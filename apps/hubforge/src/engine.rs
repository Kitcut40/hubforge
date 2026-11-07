//! Moteur HubForge : relie les modules, la configuration et les actions.
//! Il tourne dans son propre fil ; l'interface lui parle par messages
//! ([`EngineMsg`]) et reçoit son état ([`Status`]) à chaque changement.

use crate::actions::{self, Input};
use crate::audio::{Audio, AudioCmd};
use crate::config::{self, parse_color, Action, Config, KeyDef};
use crate::render::{self, KeyVisual};
use crate::foreground;
use crate::topology::{self, Placement};
use hubforge_device::{protocol, Command, DeviceManager, Event, Kind, Wheel, FADER_COUNT, KEY_COUNT};
use serde::Serialize;
use std::collections::HashMap;
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
    faders: HashMap<String, [u8; FADER_COUNT]>,
    input: Input,
    audio: Audio,
    gif_tokens: f32,
    gif_last: Instant,
    config_error: Option<String>,
    notify: Notify,
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
    pub fn new(cfg_path: PathBuf, cfg: Config, audio: Audio, notify: Notify) -> Result<Engine, String> {
        let (mgr, events) = DeviceManager::new()?;
        let cfg_dir = cfg_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
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
            faders: HashMap::new(),
            input: Input::new(),
            audio,
            gif_tokens: 0.0,
            gif_last: Instant::now(),
            config_error: None,
            notify,
        };
        e.render_all();
        Ok(e)
    }

    pub fn status(&self) -> Status {
        let mut modules: Vec<ModuleStatus> = self
            .modules
            .iter()
            .map(|(s, k)| {
                let port = self.ports.get(s).cloned();
                let placement = port.as_deref().and_then(|p| topology::placement(kind_id(*k), p));
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
        }
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
            let timeout = if self.anims.is_empty() { 30 } else { 5 };
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
            EngineMsg::ShowPage(p) => {
                if p < self.visuals.len() && p != self.page {
                    self.auto_from = None;
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
        if mtime == self.cfg_mtime {
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
                || old.general.led_brightness != self.cfg.general.led_brightness;
            if k == Kind::Keys15 || leds_changed {
                self.setup_module(&s, k);
            }
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
            .and_then(|p| topology::placement("keys15", p))
            .map(|p| p.rot)
            .unwrap_or(0)
    }

    fn render_all(&mut self) {
        let t = Instant::now();
        let dir = self.cfg_dir.clone();
        let rot = self.keys_rot;
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
                let colors: Vec<(u8, (u8, u8, u8))> = self
                    .cfg
                    .faders
                    .iter()
                    .take(FADER_COUNT)
                    .enumerate()
                    .map(|(i, slot)| (i as u8, parse_color(&slot.color).unwrap_or((255, 255, 255))))
                    .collect();
                // Les effets animés ne fonctionnent pas sur les faders : couleur fixe.
                self.setup_leds(serial, 1, &colors, 1);
                self.mgr.send(serial, Command::QueryFaders);
            }
            Kind::BaseLed => {
                let c = parse_color(&g.base_color).unwrap_or((0, 200, 255));
                self.setup_leds(serial, 0, &[(0, c)], g.base_effect);
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
        self.anims.clear();
        for k in 0..KEY_COUNT as u8 {
            let jpeg = self.visual(k).frames[0].jpeg.clone();
            self.send_image(k, &jpeg);
            self.restart_anim(k);
        }
        let name = self.cfg.pages.get(self.page).map(|p| p.name.as_str()).unwrap_or("");
        log::info!("page {} « {name} »", self.page + 1);
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
        if (key as usize) >= KEY_COUNT {
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
            Action::OpenConfig => actions::launch(&self.cfg_path.to_string_lossy(), &[]),
            Action::Page { to } => {
                let n = self.visuals.len();
                let target = match to.as_str() {
                    "next" | "suivante" => (self.page + 1) % n,
                    "previous" | "prev" | "precedente" | "précédente" => (self.page + n - 1) % n,
                    num => match num.parse::<usize>() {
                        Ok(p) if p >= 1 && p <= n => p - 1,
                        _ => {
                            log::warn!("page inconnue : « {num} »");
                            return;
                        }
                    },
                };
                // Évite de laisser une touche « enfoncée » assombrie.
                self.pressed = [false; KEY_COUNT];
                // Choix manuel : pas de retour automatique.
                self.auto_from = None;
                self.page = target;
                self.show_page();
            }
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
        self.pressed = [false; KEY_COUNT];
        self.page = p;
        self.show_page();
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
    // Faders et molettes
    // ------------------------------------------------------------------

    fn faders_changed(&mut self, serial: String, values: [u8; FADER_COUNT]) {
        let previous = self.faders.insert(serial, values);
        for (i, &v) in values.iter().enumerate() {
            if previous.map(|p| p[i] == v).unwrap_or(false) {
                continue;
            }
            let Some(slot) = self.cfg.faders.get(i) else { continue };
            let level = v as f32 / 100.0;
            let target = slot.target.trim().to_lowercase();
            if target == "master" {
                self.audio.send(AudioCmd::SetMaster(level));
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
            "volume" => self.audio.send(AudioCmd::StepMaster(dir as f32 * r.volume_step as f32 / 100.0)),
            "scroll" => self.input.scroll(-dir * r.scroll_step, false),
            "hscroll" => self.input.scroll(dir * r.scroll_step, true),
            "zoom" => self.input.zoom(-dir),
            _ => {}
        }
    }
}
