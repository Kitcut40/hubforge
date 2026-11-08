//! Moteur HubForge : relie les modules, la configuration et les actions.
//! Il tourne dans son propre fil ; l'interface lui parle par messages
//! ([`EngineMsg`]) et reçoit son état ([`Status`]) à chaque changement.

use crate::actions::{self, Input};
use crate::audio::{Audio, AudioCmd};
use crate::config::{self, parse_color, Action, Config, KeyDef};
use crate::render::{self, KeyVisual};
use crate::foreground;
use crate::widgets;
use crate::game::{self, Game};
use crate::lights::{self, Rgb};
use crate::telemetry::{AcReader, Telemetry};
use crate::profiles::{self, SharedActive};
use crate::topology::{self, Placement};
use hubforge_device::{protocol, Command, DeviceManager, Event, Kind, Wheel, FADER_COUNT, KEY_COUNT};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Messages envoyés au moteur par l'interface.
pub enum EngineMsg {
    /// Nouvelle configuration (déjà enregistrée sur le disque).
    ApplyConfig(Box<Config>),
    /// Afficher une page sur le module 15 touches.
    ShowPage(usize),
    /// Passer sur un autre profil (nom exact).
    SwitchProfile(String),
    /// Lancer le mini-jeu caché.
    StartGame,
    /// Fermer et rouvrir tous les modules (ils sont redétectés tout de suite).
    Reconnect,
    /// Jouer une animation sur les LED pour l'essayer (None = arrêter).
    PreviewAnimation(Option<lights::Animation>),
    /// Orientation de la base ou positions manuelles modifiées.
    ReloadLayout,
}

/// Couleurs envoyées en ce moment aux LED (pour l'aperçu dans l'interface).
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct LedStatus {
    /// None = effet intégré de la base.
    pub base: Option<String>,
    pub faders: Vec<String>,
}

/// Jeu détecté et ses données.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct GameStatus {
    pub name: String,
    pub live: bool,
    pub rpm: i32,
    pub max_rpm: i32,
    pub flag: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleStatus {
    pub kind: String,
    pub name: String,
    pub serial: String,
    /// Chemin de ports USB dans la base (ex. "134").
    pub port: Option<String>,
    /// Position et orientation sur la base, si connues.
    pub placement: Option<Placement>,
}

/// État du moteur, envoyé à l'interface.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Status {
    pub modules: Vec<ModuleStatus>,
    pub faders: Option<[u8; FADER_COUNT]>,
    pub page: usize,
    pub pressed: Vec<bool>,
    /// État des touches à 2 états de la page affichée (true = second état).
    pub toggled: Vec<bool>,
    pub config_error: Option<String>,
    /// Profil actif.
    pub profile: String,
    /// Partie de mini-jeu en cours.
    pub game: bool,
    /// Couleurs actuelles des LED.
    pub leds: LedStatus,
    /// Jeu de course détecté (Assetto Corsa).
    pub sim: Option<GameStatus>,
    /// Orientation de la base (degrés).
    pub base_rot: u16,
}

fn hex(c: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

pub type Notify = Box<dyn Fn(&Status) + Send>;

struct Anim {
    frame: usize,
    due: Instant,
}

/// Visuels d'une touche : état normal et, pour une touche à 2 états, le second.
#[derive(Clone)]
struct KeyVisuals {
    states: Vec<KeyVisual>,
}

/// Délai entre deux étapes d'une macro (millisecondes).
const MACRO_GAP_MS: u64 = 30;

pub struct Engine {
    mgr: DeviceManager,
    events: Receiver<Event>,
    cfg: Config,
    cfg_path: PathBuf,
    cfg_dir: PathBuf,
    cfg_mtime: Option<SystemTime>,
    /// visuels[page][touche]
    visuals: Vec<Vec<KeyVisuals>>,
    page: usize,
    anims: HashMap<u8, Anim>,
    pressed: [bool; KEY_COUNT],
    /// (page, touche) → second état actif
    toggled: HashMap<(usize, u8), bool>,
    modules: HashMap<String, Kind>,
    ports: HashMap<String, String>,
    modules_changed: bool,
    /// Rotation du module de touches sur la base (degrés, sens horaire).
    keys_rot: u16,
    /// Page affichée avant un changement automatique (pour y revenir).
    auto_from: Option<usize>,
    /// Dernière application vue au premier plan.
    last_fg: Option<String>,
    /// Étapes de macro restant à exécuter, et moment de la prochaine.
    macro_queue: VecDeque<Action>,
    macro_next: Instant,
    /// Pages d'où l'on a ouvert les dossiers (pour la touche Retour).
    page_stack: Vec<usize>,
    /// Touches vivantes : mesures, prochaine mise à jour, et ce qui est
    /// affiché sur chaque touche (pour ne renvoyer que ce qui change).
    sampler: widgets::Sampler,
    widgets_due: Instant,
    widget_sig: HashMap<u8, String>,
    /// Profil actif (partagé avec l'interface).
    active: SharedActive,
    /// Mini-jeu : partie en cours, images déjà envoyées, cache des images,
    /// dernier état du chronomètre sur les LED des faders.
    game: Option<Game>,
    game_shown: [Option<game::Visual>; KEY_COUNT],
    game_cache: HashMap<game::Visual, Arc<Vec<u8>>>,
    game_leds: Option<[(u8, u8, u8); 5]>,
    faders: HashMap<String, [u8; FADER_COUNT]>,
    input: Input,
    audio: Audio,
    gif_tokens: f32,
    gif_last: Instant,
    config_error: Option<String>,
    notify: Notify,
    /// Lumières : début des animations, prochaine image, couleurs déjà
    /// envoyées à chaque module, base pilotée par HubForge (et non par son effet intégré).
    lights_t0: Instant,
    led_due: Instant,
    sent_base: HashMap<String, Rgb>,
    sent_faders: HashMap<String, [Rgb; 5]>,
    base_soft: bool,
    leds: LedStatus,
    leds_notified: Instant,
    preview: Option<(lights::Animation, Instant)>,
    /// Données d'Assetto Corsa.
    ac: AcReader,
    tel: Option<Telemetry>,
    tel_due: Instant,
    tel_acc: bool,
    tel_fg_due: Instant,
    /// Orientation de la base et positions saisies à la main.
    base_rot: u16,
    placements: HashMap<String, Placement>,
}

fn kind_id(k: Kind) -> &'static str {
    match k {
        Kind::Keys15 => "keys15",
        Kind::Faders5 => "faders5",
        Kind::Rollers2 => "rollers2",
        Kind::BaseLed => "base",
        Kind::Encoder => "encoder",
        Kind::Knobs3 => "knobs3",
    }
}

impl Engine {
    pub fn new(cfg_dir: PathBuf, active: SharedActive, cfg: Config, audio: Audio, notify: Notify) -> Result<Engine, String> {
        let (mgr, events) = DeviceManager::new()?;
        let cfg_path = active.lock().unwrap().path.clone();
        let cfg_mtime = std::fs::metadata(&cfg_path).and_then(|m| m.modified()).ok();
        let mut e = Engine {
            mgr,
            events,
            cfg,
            cfg_path,
            cfg_dir,
            cfg_mtime,
            visuals: Vec::new(),
            page: 0,
            anims: HashMap::new(),
            pressed: [false; KEY_COUNT],
            toggled: HashMap::new(),
            modules: HashMap::new(),
            ports: HashMap::new(),
            modules_changed: false,
            keys_rot: 0,
            auto_from: None,
            last_fg: None,
            macro_queue: VecDeque::new(),
            macro_next: Instant::now(),
            page_stack: Vec::new(),
            sampler: widgets::Sampler::default(),
            widgets_due: Instant::now(),
            widget_sig: HashMap::new(),
            active,
            game: None,
            game_shown: [None; KEY_COUNT],
            game_cache: HashMap::new(),
            game_leds: None,
            faders: HashMap::new(),
            input: Input::new(),
            audio,
            gif_tokens: 0.0,
            gif_last: Instant::now(),
            config_error: None,
            notify,
            lights_t0: Instant::now(),
            led_due: Instant::now(),
            sent_base: HashMap::new(),
            sent_faders: HashMap::new(),
            base_soft: false,
            leds: LedStatus::default(),
            leds_notified: Instant::now(),
            preview: None,
            ac: AcReader::default(),
            tel: None,
            tel_due: Instant::now(),
            tel_acc: false,
            tel_fg_due: Instant::now(),
            base_rot: 0,
            placements: HashMap::new(),
        };
        e.load_layout();
        e.render_all();
        Ok(e)
    }

    pub fn status(&self) -> Status {
        let mut modules: Vec<ModuleStatus> = self
            .modules
            .iter()
            .map(|(s, k)| {
                let port = self.ports.get(s).cloned();
                let placement = port.as_deref().and_then(|p| self.placement_of(*k, p));
                ModuleStatus { kind: kind_id(*k).into(), name: k.name().into(), serial: s.clone(), port, placement }
            })
            .collect();
        modules.sort_by(|a, b| a.kind.cmp(&b.kind));
        Status {
            modules,
            faders: self.faders.values().next().copied(),
            page: self.page,
            pressed: self.pressed.to_vec(),
            toggled: (0..KEY_COUNT as u8).map(|k| self.is_toggled(k)).collect(),
            config_error: self.config_error.clone(),
            profile: self.active.lock().map(|a| a.name.clone()).unwrap_or_default(),
            game: self.game.is_some(),
            leds: self.leds.clone(),
            sim: self.tel.map(|t| GameStatus {
                name: if self.tel_acc { "Assetto Corsa Competizione".into() } else { "Assetto Corsa".into() },
                live: t.live,
                rpm: t.rpm,
                max_rpm: t.max_rpm,
                flag: t.flag,
            }),
            base_rot: self.base_rot,
        }
    }

    /// Position d'un module : saisie à la main, sinon table de la base.
    fn placement_of(&self, kind: Kind, code: &str) -> Option<Placement> {
        self.placements.get(&format!("{}:{code}", kind_id(kind))).copied().or_else(|| topology::placement(kind_id(kind), code))
    }

    fn load_layout(&mut self) {
        let st = profiles::load_settings(&self.cfg_dir);
        self.base_rot = st.base_rot % 360 / 90 * 90;
        self.placements = st.placements;
    }

    fn notify(&self) {
        (self.notify)(&self.status());
    }

    pub fn run(&mut self, stop: Arc<AtomicBool>, msgs: Receiver<EngineMsg>) {
        let mut last_scan = Instant::now() - Duration::from_secs(10);
        let mut last_cfg_check = Instant::now();
        let mut last_fg_check = Instant::now();
        while !stop.load(Ordering::Relaxed) {
            if last_fg_check.elapsed() >= Duration::from_millis(250) {
                self.check_foreground();
                last_fg_check = Instant::now();
            }
            if last_scan.elapsed() >= Duration::from_secs(2) {
                self.mgr.scan();
                last_scan = Instant::now();
            }
            if last_cfg_check.elapsed() >= Duration::from_secs(1) {
                self.check_config();
                last_cfg_check = Instant::now();
            }
            while let Ok(m) = msgs.try_recv() {
                self.message(m);
            }
            self.run_macro();
            if self.game.is_some() {
                self.tick_game();
            } else if Instant::now() >= self.widgets_due {
                self.update_widgets(false);
            }
            let busy_lights = self.preview.is_some() || self.base_soft || self.cfg.lights.faders_mode == "animation" || self.tel.map(|t| t.live).unwrap_or(false);
            let timeout = if self.anims.is_empty() && self.macro_queue.is_empty() && self.game.is_none() && !busy_lights { 30 } else { 5 };
            match self.events.recv_timeout(Duration::from_millis(timeout)) {
                Ok(ev) => {
                    self.handle(ev);
                    while let Ok(ev) = self.events.try_recv() {
                        self.handle(ev);
                    }
                    if self.modules_changed {
                        self.modules_changed = false;
                        self.ports = topology::port_codes();
                        let rot = self.detected_keys_rot();
                        if rot != self.keys_rot {
                            log::info!("module de touches tourné de {rot}°");
                            self.keys_rot = rot;
                            self.render_all();
                            self.show_page();
                        }
                    }
                    self.notify();
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            self.tick_animations();
            self.tick_lights();
        }
        self.shutdown();
    }

    fn shutdown(&mut self) {
        log::info!("arrêt de HubForge…");
        // Le module 15 touches retrouve son animation du logo.
        self.mgr.send_kind(Kind::Keys15, Command::Feature(protocol::keys15_sw_mode(false)));
        self.mgr.shutdown();
    }

    fn message(&mut self, m: EngineMsg) {
        match m {
            EngineMsg::ApplyConfig(cfg) => {
                self.cfg_mtime = std::fs::metadata(&self.cfg_path).and_then(|m| m.modified()).ok();
                self.apply_config(*cfg);
            }
            EngineMsg::SwitchProfile(name) => self.switch_profile(&name),
            EngineMsg::StartGame => self.start_game(),
            EngineMsg::Reconnect => {
                log::info!("reconnexion des modules demandée");
                self.mgr.shutdown();
                self.modules.clear();
                self.faders.clear();
                self.ports.clear();
                self.pressed = [false; KEY_COUNT];
                self.notify();
                self.mgr.scan();
                if self.modules_changed {
                    self.modules_changed = false;
                    self.ports = topology::port_codes();
                }
                self.notify();
            }
            EngineMsg::PreviewAnimation(a) => {
                self.preview = a.map(|a| (a, Instant::now()));
                self.led_due = Instant::now();
                if self.preview.is_none() {
                    self.restore_leds();
                }
            }
            EngineMsg::ReloadLayout => {
                self.load_layout();
                let rot = self.detected_keys_rot();
                if rot != self.keys_rot {
                    self.keys_rot = rot;
                    self.render_all();
                    self.show_page();
                }
                self.notify();
            }
            EngineMsg::ShowPage(p) => {
                if p < self.visuals.len() && p != self.page {
                    self.auto_from = None;
                    self.page_stack.clear();
                    self.page = p;
                    self.pressed = [false; KEY_COUNT];
                    self.show_page();
                    self.notify();
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Configuration
    // ------------------------------------------------------------------

    fn check_config(&mut self) {
        let mtime = std::fs::metadata(&self.cfg_path).and_then(|m| m.modified()).ok();
        // Fichier absent (profil renommé à l'instant) : on garde la configuration.
        if mtime.is_none() || mtime == self.cfg_mtime {
            return;
        }
        self.cfg_mtime = mtime;
        // Laisse l'éditeur finir d'écrire le fichier.
        std::thread::sleep(Duration::from_millis(150));
        match config::load(&self.cfg_path) {
            Ok(cfg) => {
                log::info!("configuration rechargée depuis le fichier ✓");
                self.apply_config(cfg);
            }
            Err(e) => {
                log::error!("{e}\n→ la configuration précédente reste active.");
                self.config_error = Some(e);
                self.notify();
            }
        }
    }

    fn apply_config(&mut self, cfg: Config) {
        let old = std::mem::replace(&mut self.cfg, cfg);
        self.config_error = None;
        self.render_all();
        self.page = self.page.min(self.visuals.len().saturating_sub(1));
        // Les touches qui ne sont plus à 2 états reviennent à l'état normal.
        let cfg = &self.cfg;
        self.toggled.retain(|(p, k), _| {
            cfg.pages.get(*p).and_then(|pg| pg.keys.iter().find(|d| d.key == k + 1)).map(|d| d.toggle.is_some()).unwrap_or(false)
        });
        let serials: Vec<(String, Kind)> = self.modules.iter().map(|(s, k)| (s.clone(), *k)).collect();
        for (s, k) in serials {
            // On ne renvoie les LED que si elles ont changé (évite un clignotement).
            let leds_changed = old.faders.iter().map(|f| &f.color).ne(self.cfg.faders.iter().map(|f| &f.color))
                || old.general.base_color != self.cfg.general.base_color
                || old.general.base_effect != self.cfg.general.base_effect
                || old.general.led_brightness != self.cfg.general.led_brightness
                || old.lights != self.cfg.lights;
            if k == Kind::Keys15 || leds_changed {
                self.setup_module(&s, k);
            }
        }
        if old.lights.animations != self.cfg.lights.animations {
            self.lights_t0 = Instant::now();
        }
        // Applique tout de suite les volumes selon la position actuelle des faders.
        let current: Vec<(String, [u8; FADER_COUNT])> = self.faders.drain().collect();
        for (s, v) in current {
            self.faders_changed(s, v);
        }
        self.notify();
    }

    fn detected_keys_rot(&self) -> u16 {
        self.modules
            .iter()
            .find(|(_, k)| matches!(k, Kind::Keys15))
            .and_then(|(s, _)| self.ports.get(s))
            .and_then(|p| self.placement_of(Kind::Keys15, p))
            .map(|p| (p.rot + self.base_rot) % 360)
            .unwrap_or(self.base_rot)
    }

    fn render_all(&mut self) {
        let t = Instant::now();
        let dir = self.cfg_dir.clone();
        let rot = self.keys_rot;
        self.widget_sig.clear();
        self.game_cache.clear();
        self.visuals = self
            .cfg
            .pages
            .iter()
            .map(|page| {
                (1..=KEY_COUNT as u8)
                    .map(|k| match page.keys.iter().find(|d| d.key == k) {
                        Some(def) => {
                            let mut states = vec![render::render_key_rot(&def.state(0), &dir, rot)];
                            if def.toggle.is_some() {
                                states.push(render::render_key_rot(&def.state(1), &dir, rot));
                            }
                            KeyVisuals { states }
                        }
                        None => KeyVisuals { states: vec![render::blank()] },
                    })
                    .collect()
            })
            .collect();
        if self.visuals.is_empty() {
            self.visuals.push(vec![KeyVisuals { states: vec![render::blank()] }; KEY_COUNT]);
        }
        log::debug!("rendu des touches en {} ms", t.elapsed().as_millis());
    }

    // ------------------------------------------------------------------
    // Modules
    // ------------------------------------------------------------------

    /// Couleur(s), puis mode, puis luminosité, avec de courtes pauses :
    /// le module n'applique une couleur qu'après la commande de mode.
    fn setup_leds(&self, serial: &str, target: u8, colors: &[(u8, (u8, u8, u8))], mode: u8) {
        let g = &self.cfg.general;
        for (index, (r, gg, b)) in colors {
            self.mgr.send(serial, Command::Output(protocol::led_color(target, *index, *r, *gg, *b, false)));
            self.mgr.send(serial, Command::Sleep(30));
        }
        let all = if target == 0 { 0 } else { 0xFF };
        self.mgr.send(serial, Command::Output(protocol::led_mode(target, all, mode, false)));
        self.mgr.send(serial, Command::Sleep(30));
        self.mgr.send(serial, Command::Output(protocol::led_brightness(target, all, g.led_brightness)));
    }

    fn setup_module(&mut self, serial: &str, kind: Kind) {
        let g = self.cfg.general.clone();
        match kind {
            Kind::Keys15 => {
                self.mgr.send(serial, Command::Feature(protocol::keys15_sw_mode(true)));
                self.mgr.send(serial, Command::Feature(protocol::keys15_brightness(g.brightness)));
                self.show_page();
            }
            Kind::Faders5 => {
                // Les effets intégrés ne fonctionnent pas sur les faders : HubForge
                // envoie lui-même chaque couleur (fixe, selon la position ou animée).
                let now = Instant::now();
                let target = self.fader_target(serial, now);
                let colors: Vec<(u8, (u8, u8, u8))> = target.iter().enumerate().map(|(i, c)| (i as u8, *c)).collect();
                self.setup_leds(serial, 1, &colors, 1);
                self.sent_faders.insert(serial.to_string(), target);
                self.mgr.send(serial, Command::QueryFaders);
            }
            Kind::BaseLed => {
                let now = Instant::now();
                match self.base_target(now) {
                    Some(c) => {
                        self.setup_leds(serial, 0, &[(0, c)], 1);
                        self.sent_base.insert(serial.to_string(), c);
                        self.base_soft = true;
                    }
                    None => {
                        let c = parse_color(&g.base_color).unwrap_or((0, 200, 255));
                        self.setup_leds(serial, 0, &[(0, c)], g.base_effect);
                        self.sent_base.remove(serial);
                        self.base_soft = false;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle(&mut self, ev: Event) {
        match ev {
            Event::Connected(info) => {
                log::info!("✓ {} connecté", info.kind.name());
                self.modules.insert(info.serial.clone(), info.kind);
                self.modules_changed = true;
                self.setup_module(&info.serial, info.kind);
            }
            Event::Disconnected { serial } => {
                self.sent_base.remove(&serial);
                self.sent_faders.remove(&serial);
                if let Some(k) = self.modules.remove(&serial) {
                    log::info!("✗ {} débranché", k.name());
                }
                self.modules_changed = true;
                self.faders.remove(&serial);
            }
            Event::KeyDown { key, .. } => self.key_down(key),
            Event::KeyUp { key, .. } => self.key_up(key),
            Event::Faders { serial, values } => self.faders_changed(serial, values),
            Event::Roller { wheel, step, .. } => self.roller(wheel, step),
        }
    }

    // ------------------------------------------------------------------
    // Touches
    // ------------------------------------------------------------------

    fn is_toggled(&self, key: u8) -> bool {
        self.toggled.get(&(self.page, key)).copied().unwrap_or(false)
    }

    fn visual(&self, key: u8) -> &KeyVisual {
        let kv = &self.visuals[self.page][key as usize];
        let i = if self.is_toggled(key) { 1 } else { 0 };
        &kv.states[i.min(kv.states.len() - 1)]
    }

    fn key_def(&self, key: u8) -> Option<&KeyDef> {
        self.cfg.pages.get(self.page).and_then(|p| p.keys.iter().find(|d| d.key == key + 1))
    }

    fn send_image(&self, key: u8, jpeg: &Arc<Vec<u8>>) {
        let packets = Arc::new(protocol::keys15_image_packets(key, jpeg));
        self.mgr.send_kind(Kind::Keys15, Command::Packets(packets));
    }

    fn restart_anim(&mut self, key: u8) {
        self.anims.remove(&key);
        let v = self.visual(key);
        if v.is_animated() {
            let due = Instant::now() + v.frames[0].delay;
            self.anims.insert(key, Anim { frame: 0, due });
        }
    }

    fn show_page(&mut self) {
        // Pendant le mini-jeu, c'est lui qui dessine (tout sera renvoyé).
        if self.game.is_some() {
            self.game_shown = [None; KEY_COUNT];
            return;
        }
        self.anims.clear();
        // Touches vivantes à jour avant l'envoi de la page.
        self.widget_sig.clear();
        self.render_widgets();
        for k in 0..KEY_COUNT as u8 {
            let jpeg = self.visual(k).frames[0].jpeg.clone();
            self.send_image(k, &jpeg);
            self.restart_anim(k);
        }
        let name = self.cfg.pages.get(self.page).map(|p| p.name.as_str()).unwrap_or("");
        log::info!("page {} « {name} »", self.page + 1);
    }

    // ------------------------------------------------------------------
    // Touches vivantes
    // ------------------------------------------------------------------

    /// Redessine les touches vivantes de la page dont le contenu a changé ;
    /// retourne les touches redessinées.
    fn render_widgets(&mut self) -> Vec<u8> {
        let page = self.page;
        let Some(defs) = self.cfg.pages.get(page).map(|p| p.keys.clone()) else { return Vec::new() };
        if !defs.iter().any(|d| d.widget.is_some()) {
            return Vec::new();
        }
        let snap = self.sampler.sample(self.audio.state());
        let mut changed = Vec::new();
        for def in defs.iter().filter(|d| d.widget.is_some() && (1..=KEY_COUNT as u8).contains(&d.key)) {
            let key = def.key - 1;
            // Une touche à 2 états montre son second état tel quel.
            if def.toggle.is_some() && self.is_toggled(key) {
                continue;
            }
            let sig = widgets::signature(def, &snap);
            if self.widget_sig.get(&key) == Some(&sig) {
                continue;
            }
            let img = widgets::render(def, &snap);
            let visual = render::visual_from_image(&img, self.keys_rot);
            if let Some(v) = self.visuals.get_mut(page).and_then(|p| p.get_mut(key as usize)) {
                if let Some(first) = v.states.first_mut() {
                    *first = visual;
                }
            }
            self.widget_sig.insert(key, sig);
            changed.push(key);
        }
        changed
    }

    /// Mise à jour régulière (toutes les 500 ms) : n'envoie que les touches
    /// dont l'affichage a changé.
    fn update_widgets(&mut self, force: bool) {
        self.widgets_due = Instant::now() + Duration::from_millis(500);
        // Demande l'état du son pour la prochaine fois (volume, micro).
        self.audio.send(AudioCmd::Poll);
        if force {
            self.widget_sig.clear();
        }
        for key in self.render_widgets() {
            let f = self.current_frame(key);
            let jpeg = if self.pressed[key as usize] && self.cfg.general.press_feedback { f.pressed.clone() } else { f.jpeg.clone() };
            self.send_image(key, &jpeg);
        }
    }

    /// Après une action qui peut changer le volume ou le micro : mise à jour
    /// rapide des touches vivantes.
    fn nudge_widgets(&mut self) {
        self.audio.send(AudioCmd::Poll);
        self.widgets_due = self.widgets_due.min(Instant::now() + Duration::from_millis(120));
    }

    fn current_frame(&self, key: u8) -> &render::Frame {
        let v = self.visual(key);
        let i = self.anims.get(&key).map(|a| a.frame).unwrap_or(0);
        &v.frames[i.min(v.frames.len() - 1)]
    }

    fn key_down(&mut self, key: u8) {
        if (key as usize) >= KEY_COUNT {
            return;
        }
        if let Some(g) = self.game.as_mut() {
            g.press(key as usize, Instant::now());
            return;
        }
        self.pressed[key as usize] = true;
        if self.cfg.general.press_feedback {
            let jpeg = self.current_frame(key).pressed.clone();
            self.send_image(key, &jpeg);
        }
        let Some(def) = self.key_def(key).cloned() else { return };
        let state = if def.toggle.is_some() && self.is_toggled(key) { 1 } else { 0 };
        let action = def.state(state).action;
        if def.toggle.is_some() {
            let page = self.page;
            self.toggled.insert((page, key), state == 0);
        }
        let page_before = self.page;
        self.execute(&action);
        self.nudge_widgets();
        // Dossier « refermé après une action » : retour à la page d'avant.
        let closes = !matches!(action, Action::None | Action::Folder { .. } | Action::Back | Action::Page { .. });
        if closes && self.page == page_before && self.is_closing_folder(self.page) {
            self.go_back();
            return;
        }
        // Touche à 2 états : affiche tout de suite le nouvel état.
        if def.toggle.is_some() && self.page == page_before {
            self.restart_anim(key);
            if !self.cfg.general.press_feedback {
                let jpeg = self.visual(key).frames[0].jpeg.clone();
                self.send_image(key, &jpeg);
            }
        }
    }

    fn key_up(&mut self, key: u8) {
        if (key as usize) >= KEY_COUNT || self.game.is_some() {
            return;
        }
        self.pressed[key as usize] = false;
        if self.cfg.general.press_feedback {
            let jpeg = self.current_frame(key).jpeg.clone();
            self.send_image(key, &jpeg);
        }
    }

    fn execute(&mut self, action: &Action) {
        match action {
            Action::None => {}
            Action::Hotkey { keys } => self.input.hotkey(keys),
            Action::Text { text } => self.input.text(text),
            Action::Launch { target, args } => actions::launch(target, args),
            Action::Media { command } if command == "mic_mute" => self.audio.send(AudioCmd::ToggleMicMute),
            Action::Media { command } => self.input.media(command),
            Action::Lock => actions::lock_session(),
            Action::Macro { steps } => {
                if self.macro_queue.len() + steps.len() > 4 * config::MACRO_MAX_STEPS {
                    log::warn!("trop de macros en attente : appui ignoré");
                    return;
                }
                // Les étapes s'ajoutent à la suite d'une macro déjà en cours.
                self.macro_queue.extend(steps.iter().filter(|s| !matches!(s, Action::Macro { .. })).cloned());
                self.run_macro();
            }
            Action::Wait { .. } => {} // n'a de sens que dans une macro
            Action::OpenConfig => actions::launch(&self.cfg_path.to_string_lossy(), &[]),
            Action::Page { to } => {
                let Some(target) = self.resolve_page(to, true) else { return };
                // Choix manuel : pas de retour automatique, dossiers refermés.
                self.auto_from = None;
                self.page_stack.clear();
                self.change_page(target);
            }
            Action::Folder { to } => {
                let Some(target) = self.resolve_page(to, false) else { return };
                if target == self.page {
                    return;
                }
                if self.page_stack.len() >= 16 {
                    self.page_stack.remove(0);
                }
                self.page_stack.push(self.page);
                self.change_page(target);
            }
            Action::Back => self.go_back(),
            Action::Profile { to } => {
                let current = self.active.lock().map(|a| a.name.clone()).unwrap_or_default();
                match profiles::resolve(&self.cfg_dir, &current, to) {
                    Some(name) => self.switch_profile(&name),
                    None => log::warn!("profil inconnu : « {to} »"),
                }
            }
            Action::Game => self.start_game(),
        }
    }

    // ------------------------------------------------------------------
    // Profils
    // ------------------------------------------------------------------

    /// Charge un autre profil : touches, faders, molettes et LED changent.
    fn switch_profile(&mut self, name: &str) {
        let path = profiles::path(&self.cfg_dir, name);
        let cfg = match config::load(&path) {
            Ok(c) => c,
            Err(e) => {
                log::error!("profil « {name} » : {e}");
                self.config_error = Some(format!("profil « {name} » : {e}"));
                self.notify();
                return;
            }
        };
        if let Ok(mut a) = self.active.lock() {
            a.name = name.to_string();
            a.path = path.clone();
        }
        let mut st = profiles::load_settings(&self.cfg_dir);
        st.profile = name.to_string();
        profiles::save_settings(&self.cfg_dir, &st);
        log::info!("profil « {name} »");
        self.cfg_path = path;
        self.cfg_mtime = std::fs::metadata(&self.cfg_path).and_then(|m| m.modified()).ok();
        self.page = 0;
        self.toggled.clear();
        self.page_stack.clear();
        self.auto_from = None;
        self.last_fg = None;
        self.macro_queue.clear();
        // Toutes les LED sont renvoyées (couleurs du nouveau profil).
        self.cfg.general.base_color.clear();
        self.apply_config(cfg);
        self.show_page();
        self.notify();
    }

    // ------------------------------------------------------------------
    // Mini-jeu caché
    // ------------------------------------------------------------------

    fn start_game(&mut self) {
        if self.game.is_some() || !self.modules.values().any(|k| *k == Kind::Keys15) {
            return;
        }
        let best = profiles::load_settings(&self.cfg_dir).game_best;
        let seed = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        log::info!("🎮 mini-jeu lancé (record : {best})");
        self.anims.clear();
        self.macro_queue.clear();
        self.game = Some(Game::new(best, Instant::now(), seed));
        self.game_shown = [None; KEY_COUNT];
        self.game_leds = None;
        if self.game_cache.is_empty() {
            log::debug!("préparation des images du jeu");
        }
        self.notify();
    }

    fn game_image(&mut self, v: game::Visual) -> Arc<Vec<u8>> {
        if let Some(j) = self.game_cache.get(&v) {
            return j.clone();
        }
        let img = game::render(v);
        let jpeg = render::visual_from_image(&img, self.keys_rot).frames[0].jpeg.clone();
        self.game_cache.insert(v, jpeg.clone());
        jpeg
    }

    fn tick_game(&mut self) {
        let now = Instant::now();
        let Some(g) = self.game.as_mut() else { return };
        let was_over = g.is_over();
        g.tick(now);
        let visuals = g.visuals(now);
        let left = g.time_left(now);
        let (done, over, new_record, score, best) = (g.is_done(), g.is_over(), g.new_record, g.score, g.best);

        for (k, v) in visuals.iter().enumerate() {
            if self.game_shown[k] != Some(*v) {
                let jpeg = self.game_image(*v);
                self.send_image(k as u8, &jpeg);
                self.game_shown[k] = Some(*v);
            }
        }
        // Chronomètre sur les LED des faders.
        let leds = game::timer_leds(left);
        if self.game_leds != Some(leds) {
            self.game_leds = Some(leds);
            let colors: Vec<(u8, (u8, u8, u8))> = leds.iter().enumerate().map(|(i, c)| (i as u8, *c)).collect();
            let faders: Vec<String> = self.modules.iter().filter(|(_, k)| **k == Kind::Faders5).map(|(s, _)| s.clone()).collect();
            for s in faders {
                self.setup_leds(&s, 1, &colors, 1);
            }
        }
        if over && !was_over {
            log::info!("🎮 fin de partie : {score} points (record : {best})");
            if new_record {
                let mut st = profiles::load_settings(&self.cfg_dir);
                st.game_best = best;
                profiles::save_settings(&self.cfg_dir, &st);
                // Arc-en-ciel sur la base pour fêter ça.
                let bases: Vec<String> = self.modules.iter().filter(|(_, k)| **k == Kind::BaseLed).map(|(s, _)| s.clone()).collect();
                for s in bases {
                    self.setup_leds(&s, 0, &[(0, (255, 255, 255))], 9);
                }
            }
        }
        if done {
            self.game = None;
            // Retour à la normale : LED du profil et page affichée.
            let serials: Vec<(String, Kind)> = self.modules.iter().map(|(s, k)| (s.clone(), *k)).collect();
            for (s, k) in serials {
                if matches!(k, Kind::Faders5 | Kind::BaseLed) {
                    self.setup_module(&s, k);
                }
            }
            self.show_page();
            self.notify();
        }
    }

    /// Change de page en relâchant les touches « enfoncées ».
    fn change_page(&mut self, p: usize) {
        self.pressed = [false; KEY_COUNT];
        self.page = p;
        self.show_page();
    }

    fn is_folder(&self, page: usize) -> bool {
        self.cfg.pages.get(page).map(|p| p.folder).unwrap_or(false)
    }

    fn is_closing_folder(&self, page: usize) -> bool {
        self.cfg.pages.get(page).map(|p| p.folder && p.close_after).unwrap_or(false)
    }

    /// Revient à la page d'où le dossier a été ouvert (sinon à la première
    /// page normale).
    fn go_back(&mut self) {
        let target = loop {
            match self.page_stack.pop() {
                Some(p) if p < self.visuals.len() && p != self.page => break Some(p),
                Some(_) => continue,
                None => break (0..self.visuals.len()).find(|&p| !self.is_folder(p) && p != self.page),
            }
        };
        if let Some(p) = target {
            self.change_page(p);
        }
    }

    /// « next », « previous » ou un numéro (1 = première page).
    /// `skip_folders` : suivante / précédente ignorent les dossiers.
    fn resolve_page(&self, to: &str, skip_folders: bool) -> Option<usize> {
        let n = self.visuals.len();
        let step = match to {
            "next" | "suivante" => 1,
            "previous" | "prev" | "precedente" | "précédente" => n - 1,
            num => {
                return match num.trim().parse::<usize>() {
                    Ok(p) if p >= 1 && p <= n => Some(p - 1),
                    _ => {
                        log::warn!("page inconnue : « {num} »");
                        None
                    }
                }
            }
        };
        let mut p = self.page;
        for _ in 0..n {
            p = (p + step) % n;
            if !skip_folders || !self.is_folder(p) {
                return Some(p);
            }
        }
        None
    }

    /// Exécute les étapes de macro dont le moment est venu (sans bloquer :
    /// les touches, GIF et faders restent réactifs pendant une pause).
    fn run_macro(&mut self) {
        let page_before = self.page;
        while !self.macro_queue.is_empty() && Instant::now() >= self.macro_next {
            let Some(step) = self.macro_queue.pop_front() else { break };
            match step {
                Action::Wait { ms } => {
                    self.macro_next = Instant::now() + Duration::from_millis(ms.min(config::WAIT_MAX_MS));
                }
                other => {
                    self.execute(&other);
                    // Petit délai entre deux étapes, pour que Windows suive.
                    self.macro_next = Instant::now() + Duration::from_millis(MACRO_GAP_MS);
                }
            }
        }
        if self.page != page_before {
            self.notify();
        }
    }

    // ------------------------------------------------------------------
    // Pages automatiques
    // ------------------------------------------------------------------

    /// Page liée à un programme (champ `apps` des pages).
    fn page_for_app(&self, exe: &str) -> Option<usize> {
        self.cfg
            .pages
            .iter()
            .position(|p| p.apps.iter().any(|a| foreground::normalize(a) == exe))
            .filter(|&i| i < self.visuals.len())
    }

    fn page_has_apps(&self, page: usize) -> bool {
        self.cfg.pages.get(page).map(|p| !p.apps.is_empty()).unwrap_or(false)
    }

    /// Affiche la page de l'application au premier plan ; en sortant de
    /// l'application, revient à la page affichée avant.
    fn check_foreground(&mut self) {
        if !self.cfg.general.auto_pages {
            self.auto_from = None;
            return;
        }
        let fg = foreground::foreground_exe();
        if fg == self.last_fg {
            return;
        }
        self.last_fg = fg.clone();
        let Some(exe) = fg else { return };
        // La fenêtre de HubForge ne change rien : on peut éditer tranquillement.
        if exe == "hubforge.exe" {
            return;
        }
        match self.page_for_app(&exe) {
            Some(p) if p != self.page => {
                if self.auto_from.is_none() {
                    self.auto_from = Some(self.page);
                }
                log::info!("{exe} au premier plan → page {}", p + 1);
                self.set_page(p);
            }
            Some(_) => {}
            None => {
                if let Some(prev) = self.auto_from.take() {
                    // Seulement si on est encore sur une page d'application
                    // (sinon, l'utilisateur a choisi une autre page entre-temps).
                    if self.page_has_apps(self.page) && prev < self.visuals.len() && prev != self.page {
                        log::info!("retour à la page {}", prev + 1);
                        self.set_page(prev);
                    }
                }
            }
        }
    }

    fn set_page(&mut self, p: usize) {
        self.page_stack.clear();
        self.change_page(p);
        self.notify();
    }

    fn tick_animations(&mut self) {
        if self.anims.is_empty() {
            return;
        }
        let now = Instant::now();
        let fps = self.cfg.general.gif_max_fps.clamp(1, 60) as f32;
        self.gif_tokens = (self.gif_tokens + self.gif_last.elapsed().as_secs_f32() * fps).min(3.0);
        self.gif_last = now;
        let mut due: Vec<(Instant, u8)> = self
            .anims
            .iter()
            .filter(|(k, a)| a.due <= now && !self.pressed[**k as usize])
            .map(|(k, a)| (a.due, *k))
            .collect();
        due.sort();
        for (_, key) in due {
            if self.gif_tokens < 1.0 {
                break;
            }
            self.gif_tokens -= 1.0;
            let len = self.visual(key).frames.len();
            let next = (self.anims[&key].frame + 1) % len;
            let frame = self.visual(key).frames[next].clone();
            if let Some(anim) = self.anims.get_mut(&key) {
                anim.frame = next;
                anim.due = now + frame.delay;
            }
            self.send_image(key, &frame.jpeg);
        }
    }

    // ------------------------------------------------------------------
    // Lumières
    // ------------------------------------------------------------------

    fn anim_t(&self, now: Instant) -> u64 {
        now.saturating_duration_since(self.lights_t0).as_millis() as u64
    }

    fn overlay(&self, now: Instant) -> lights::GameOverlay {
        match self.tel {
            Some(t) => lights::game_overlay(&self.cfg.lights.game, &t, self.anim_t(now)),
            None => lights::GameOverlay::default(),
        }
    }

    /// Couleur voulue pour la base (None = effet intégré de la base).
    fn base_target(&self, now: Instant) -> Option<Rgb> {
        if let Some((a, t0)) = &self.preview {
            return Some(lights::frame(a, now.saturating_duration_since(*t0).as_millis() as u64).base);
        }
        if let Some(c) = self.overlay(now).base {
            return Some(c);
        }
        let l = &self.cfg.lights;
        if l.base_mode == "animation" {
            if let Some(a) = lights::find(l, &l.base_animation) {
                return Some(lights::frame(a, self.anim_t(now)).base);
            }
        }
        None
    }

    /// Couleurs voulues pour les 5 faders d'un module.
    fn fader_target(&self, serial: &str, now: Instant) -> [Rgb; 5] {
        if let Some((a, t0)) = &self.preview {
            return lights::frame(a, now.saturating_duration_since(*t0).as_millis() as u64).faders;
        }
        if let Some(f) = self.overlay(now).faders {
            return f;
        }
        let l = &self.cfg.lights;
        match l.faders_mode.as_str() {
            "animation" => {
                if let Some(a) = lights::find(l, &l.faders_animation) {
                    return lights::frame(a, self.anim_t(now)).faders;
                }
            }
            "position" => {
                let values = self.faders.get(serial).copied().unwrap_or([0; FADER_COUNT]);
                let mut out = [(0, 0, 0); 5];
                for (i, c) in out.iter_mut().enumerate() {
                    *c = lights::gradient_color(&l.gradient, values[i]);
                }
                return out;
            }
            _ => {}
        }
        let mut out = [(255, 255, 255); 5];
        for (i, c) in out.iter_mut().enumerate() {
            if let Some(slot) = self.cfg.faders.get(i) {
                *c = parse_color(&slot.color).unwrap_or((255, 255, 255));
            }
        }
        out
    }

    /// Remet les LED dans leur état habituel (fin d'un essai d'animation).
    fn restore_leds(&mut self) {
        let serials: Vec<(String, Kind)> = self.modules.iter().map(|(s, k)| (s.clone(), *k)).collect();
        for (s, k) in serials {
            if matches!(k, Kind::Faders5 | Kind::BaseLed) {
                self.setup_module(&s, k);
            }
        }
    }

    /// Lit les données du jeu de course, s'il tourne.
    fn poll_telemetry(&mut self, now: Instant) {
        if now < self.tel_due {
            return;
        }
        let live = self.tel.map(|t| t.live).unwrap_or(false);
        self.tel_due = now + Duration::from_millis(if live { 30 } else { 500 });
        if self.tel.is_some() && now >= self.tel_fg_due {
            self.tel_fg_due = now + Duration::from_secs(2);
            if let Some(exe) = foreground::foreground_exe() {
                if exe.starts_with("ac2-") {
                    self.tel_acc = true;
                } else if exe == "acs.exe" || exe == "acs_x86.exe" {
                    self.tel_acc = false;
                }
            }
        }
        let before = self.tel;
        self.tel = if self.cfg.lights.game.enabled { self.ac.poll(self.tel_acc) } else { None };
        let changed = match (before, self.tel) {
            (Some(a), Some(b)) => a.live != b.live || a.flag != b.flag || a.max_rpm != b.max_rpm,
            (a, b) => a.is_some() != b.is_some(),
        };
        if changed {
            if let Some(t) = self.tel {
                if before.is_none() {
                    log::info!("🏁 Assetto Corsa détecté");
                }
                if t.live != before.map(|b| b.live).unwrap_or(false) {
                    log::info!("Assetto Corsa : {}", if t.live { "en piste" } else { "hors piste" });
                }
            }
            self.led_due = now;
            self.notify();
        }
    }

    /// Met à jour les LED de la base et des faders (animations, position, jeux).
    fn tick_lights(&mut self) {
        let now = Instant::now();
        self.poll_telemetry(now);
        if self.game.is_some() || now < self.led_due {
            return;
        }
        let fps = self.cfg.lights.fps.clamp(1, 30) as u64;
        self.led_due = now + Duration::from_millis(1000 / fps);

        let mut changed = false;
        let base = self.base_target(now);
        let bases: Vec<String> = self.modules.iter().filter(|(_, k)| **k == Kind::BaseLed).map(|(s, _)| s.clone()).collect();
        for s in &bases {
            match base {
                Some(c) => {
                    if self.sent_base.get(s) != Some(&c) {
                        self.mgr.send(s, Command::Output(protocol::led_color(0, 0, c.0, c.1, c.2, false)));
                        self.mgr.send(s, Command::Sleep(6));
                        self.mgr.send(s, Command::Output(protocol::led_mode(0, 0, 1, false)));
                        if !self.base_soft {
                            self.mgr.send(s, Command::Sleep(6));
                            self.mgr.send(s, Command::Output(protocol::led_brightness(0, 0, self.cfg.general.led_brightness)));
                        }
                        self.sent_base.insert(s.clone(), c);
                        changed = true;
                    }
                }
                None if self.base_soft => {
                    // Retour à l'effet intégré de la base.
                    self.setup_module(s, Kind::BaseLed);
                    changed = true;
                }
                None => {}
            }
        }
        self.base_soft = base.is_some() && !bases.is_empty();

        let faders: Vec<String> = self.modules.iter().filter(|(_, k)| **k == Kind::Faders5).map(|(s, _)| s.clone()).collect();
        let mut first: Option<[Rgb; 5]> = None;
        for s in &faders {
            let target = self.fader_target(s, now);
            first.get_or_insert(target);
            let prev = self.sent_faders.get(s).copied();
            if prev == Some(target) {
                continue;
            }
            for (i, c) in target.iter().enumerate() {
                if prev.map(|p| p[i] != *c).unwrap_or(true) {
                    self.mgr.send(s, Command::Output(protocol::led_color(1, i as u8, c.0, c.1, c.2, false)));
                    self.mgr.send(s, Command::Sleep(5));
                }
            }
            self.mgr.send(s, Command::Output(protocol::led_mode(1, 0xFF, 1, false)));
            self.sent_faders.insert(s.clone(), target);
            changed = true;
        }

        let status = LedStatus {
            base: base.map(hex),
            faders: first.map(|f| f.iter().map(|c| hex(*c)).collect()).unwrap_or_default(),
        };
        if changed || status != self.leds {
            self.leds = status;
            // L'interface n'a pas besoin de plus de 8 images par seconde.
            if now.saturating_duration_since(self.leds_notified) >= Duration::from_millis(120) {
                self.leds_notified = now;
                self.notify();
            }
        }
    }

    // ------------------------------------------------------------------
    // Faders et molettes
    // ------------------------------------------------------------------

    fn faders_changed(&mut self, serial: String, values: [u8; FADER_COUNT]) {
        let previous = self.faders.insert(serial, values);
        if self.cfg.lights.faders_mode == "position" {
            self.led_due = self.led_due.min(Instant::now());
        }
        for (i, &v) in values.iter().enumerate() {
            if previous.map(|p| p[i] == v).unwrap_or(false) {
                continue;
            }
            let Some(slot) = self.cfg.faders.get(i) else { continue };
            let level = v as f32 / 100.0;
            let target = slot.target.trim().to_lowercase();
            if target == "master" {
                self.audio.send(AudioCmd::SetMaster(level));
                self.widgets_due = self.widgets_due.min(Instant::now() + Duration::from_millis(80));
            } else if target == "mic" {
                self.audio.send(AudioCmd::SetMic(level));
            } else if let Some(apps) = target.strip_prefix("app:") {
                let names = apps.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                self.audio.send(AudioCmd::SetApps(names, level));
            }
        }
    }

    fn roller(&mut self, wheel: Wheel, step: i8) {
        let r = &self.cfg.rollers;
        let (mode, invert) = match wheel {
            Wheel::Right => (r.right.clone(), r.invert_right),
            Wheel::Left => (r.left.clone(), r.invert_left),
        };
        // Sens « 01 » = vers le bas (-1) : on le ramène à « vers le haut = + ».
        let mut dir = -(step as i32);
        if invert {
            dir = -dir;
        }
        match mode.as_str() {
            "volume" => {
                self.audio.send(AudioCmd::StepMaster(dir as f32 * r.volume_step as f32 / 100.0));
                self.widgets_due = self.widgets_due.min(Instant::now() + Duration::from_millis(80));
            }
            "scroll" => self.input.scroll(-dir * r.scroll_step, false),
            "hscroll" => self.input.scroll(dir * r.scroll_step, true),
            "zoom" => self.input.zoom(-dir),
            _ => {}
        }
    }
}
