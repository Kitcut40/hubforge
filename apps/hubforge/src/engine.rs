//! Moteur HubForge : relie les modules, la configuration et les actions.
//! Il tourne dans son propre fil ; l'interface lui parle par messages
//! ([`EngineMsg`]) et reçoit son état ([`Status`]) à chaque changement.

use crate::actions::{self, Input};
use crate::audio::{Audio, AudioCmd};
use crate::config::{self, parse_color, Action, Config, KeyDef, WheelActions};
use crate::render::{self, KeyVisual};
use crate::foreground;
use crate::widgets;
use crate::game::{self, Game};
use crate::doom;
use crate::sound::{SoundCmd, Sounds};
use crate::vars::{self, Vars};
use crate::{clip, connect};
use crate::lights::{self, Rgb};
use crate::profiles::{self, SharedActive};
use crate::topology::{self, Placement};
use hubforge_device::{protocol, Command, DeviceManager, Event, Kind, Wheel, FADER_COUNT, KEY_COUNT};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
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
    /// Essayer un son (bouton « Écouter » de l'interface).
    TestSound { file: String, volume: u8 },
    /// Easter egg : lancer Doom avec ce fichier WAD.
    StartDoom(std::path::PathBuf),
    StopDoom,
    /// Touche de Doom depuis le clavier du PC (fenêtre de HubForge).
    DoomKey { pressed: bool, code: u8 },
    /// Remettre toutes les variables à zéro.
    ClearVars,
    /// La langue a changé : redessiner les touches (dates, météo…).
    Redraw,
    /// Addons avec programme : activation, accord ou réglages modifiés.
    SyncServices,
}

/// Résultat d'une action qui tourne en arrière-plan (requête web, IA).
struct JobDone {
    /// Touche qui l'a lancée (page, touche), pour son liseré d'état.
    at: Option<(usize, u8)>,
    result: Result<JobOut, String>,
}

enum JobOut {
    Nothing,
    /// Donner une valeur à une variable.
    Var(String, String),
    /// Coller ce texte à la place de la sélection, puis remettre l'ancien
    /// contenu du presse-papiers.
    Paste { text: String, restore: Option<String> },
}

/// Liseré affiché autour d'une touche : en cours, réussi, échec.
const MARK_BUSY: [u8; 3] = [255, 166, 0];
const MARK_OK: [u8; 3] = [48, 209, 88];
const MARK_ERR: [u8; 3] = [255, 59, 48];
/// Liseré « appuie encore pour confirmer ».
const MARK_CONFIRM: [u8; 3] = [255, 214, 10];
/// Délai pour confirmer une action sensible.
const CONFIRM_WINDOW: Duration = Duration::from_secs(3);
/// Répétition tant que la touche est enfoncée : premier délai, puis cadence.
const REPEAT_FIRST: Duration = Duration::from_millis(400);
const REPEAT_EVERY: Duration = Duration::from_millis(90);

/// Minuteur d'une touche (chronomètre ou compte à rebours).
#[derive(Debug, Clone, Copy, Default)]
struct TimerState {
    acc: Duration,
    since: Option<Instant>,
}

impl TimerState {
    fn elapsed(&self, now: Instant) -> Duration {
        self.acc + self.since.map(|s| now.saturating_duration_since(s)).unwrap_or_default()
    }
}

/// Reprise douce d'un fader : il n'agit qu'une fois passé par le volume actuel.
#[derive(Clone, Copy)]
struct Pickup {
    /// Côté où se trouvait le fader au premier mouvement (-1 en dessous, +1 au-dessus).
    side: Option<i8>,
    since: Instant,
}

/// Appui sur une touche qui a un appui long ou un double appui : on attend
/// de savoir lequel c'est avant d'agir.
struct Gesture {
    key: u8,
    page: usize,
    down_at: Instant,
    /// Touche relâchée : on attend un éventuel second appui.
    up_at: Option<Instant>,
    /// Appui long ou double appui déjà déclenché : plus rien à faire.
    done: bool,
}

/// Partie de Doom en cours.
struct DoomPlay {
    sess: doom::Session,
    seen: u64,
    tile_hash: [u64; KEY_COUNT],
    /// Touches de Doom maintenues (nombre de touches du MasterHUB qui les tiennent).
    held: HashMap<u8, u8>,
    /// Touches relâchées automatiquement (crans des molettes).
    pulses: Vec<(Instant, u8)>,
    esc_since: Option<Instant>,
    leds_due: Instant,
}

/// Couleurs envoyées en ce moment aux LED (pour l'aperçu dans l'interface).
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct LedStatus {
    /// None = effet intégré de la base.
    pub base: Option<String>,
    pub faders: Vec<String>,
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
    /// Orientation de la base (degrés).
    pub base_rot: u16,
    /// Doom tourne sur le MasterHUB.
    pub doom: bool,
    /// Dernière erreur au lancement de Doom.
    pub doom_error: Option<String>,
    /// Variables des macros.
    pub vars: BTreeMap<String, String>,
    /// Faders en attente de reprise douce.
    pub pickup: Vec<bool>,
    /// MasterHUB en veille.
    pub asleep: bool,
    /// Dernier message d'un addon à afficher : (numéro, addon, texte, erreur).
    pub notice: Option<(u64, String, String, bool)>,
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
    /// Soundboard.
    sounds: Sounds,
    /// Touches du PC maintenues par une touche du MasterHUB (action « maintenir »).
    held: HashMap<u8, String>,
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
    /// Appuis longs / doubles en cours d'analyse.
    gesture: Option<Gesture>,
    /// Orientation de la base et positions saisies à la main.
    base_rot: u16,
    placements: HashMap<String, Placement>,
    doom: Option<DoomPlay>,
    doom_error: Option<String>,
    /// Variables des macros, et touches dont le texte en affiche une.
    vars: Vars,
    var_keys: Vec<(usize, u8)>,
    /// Une macro est en train d'être déroulée (les conditions insèrent alors
    /// leurs étapes juste après l'étape en cours).
    in_macro: bool,
    /// Touche dont l'action est en cours (pour l'action « minuteur »).
    acting: Option<(usize, u8)>,
    /// Minuteurs des touches (page, touche).
    timers: HashMap<(usize, u8), TimerState>,
    /// Veille : écrans et LED éteints après inactivité ou verrouillage.
    asleep: bool,
    last_device_input: Instant,
    sleep_check: Instant,
    /// Touche dont l'appui a seulement réveillé le MasterHUB.
    wake_swallow: Option<u8>,
    /// Reprise douce : faders en attente, et cible de chaque fader au dernier changement.
    pickup: [Option<Pickup>; FADER_COUNT],
    fader_targets: [String; FADER_COUNT],
    /// Actions en arrière-plan : canal des résultats.
    job_tx: Sender<JobDone>,
    job_rx: Receiver<JobDone>,
    /// Liserés d'état : touche → (page, couleur, fin).
    marks: HashMap<u8, (usize, [u8; 3], Instant)>,
    /// Ancien contenu du presse-papiers à remettre après un collage de l'IA.
    clip_restore: Option<(String, Instant)>,
    /// Action sensible en attente de confirmation : (page, touche, fin).
    confirm_pending: Option<(usize, u8, Instant)>,
    /// Touche qui répète son action : (page, touche, prochaine répétition).
    repeating: Option<(usize, u8, Instant)>,
    /// Mode nuit en cours (luminosité réduite).
    night: bool,
    /// Touches « Lancer » dont l'application est ouverte (point vert).
    run_dots: HashSet<u8>,
    run_check: Instant,
    /// Mode dynamique : applications de chaque profil, et profil d'avant.
    profile_apps: Vec<(String, Vec<String>)>,
    profile_mtimes: HashMap<String, std::time::SystemTime>,
    profiles_check: Instant,
    auto_profiles: bool,
    auto_profile_from: Option<String>,
    /// Addons avec programme : dernière synchronisation, signature des addons.
    services_check: Instant,
    services_sig: String,
    services_last_full: Instant,
    notice: Option<(u64, String, String, bool)>,
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
        let cfg_dir_for_vars = cfg_dir.clone();
        let (job_tx, job_rx) = std::sync::mpsc::channel();
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
            sounds: Sounds::new(),
            held: HashMap::new(),
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
            gesture: None,
            base_rot: 0,
            placements: HashMap::new(),
            doom: None,
            doom_error: None,
            vars: Vars::load(&cfg_dir_for_vars),
            var_keys: Vec::new(),
            in_macro: false,
            acting: None,
            timers: HashMap::new(),
            asleep: false,
            last_device_input: Instant::now(),
            sleep_check: Instant::now(),
            wake_swallow: None,
            pickup: [None; FADER_COUNT],
            fader_targets: Default::default(),
            job_tx,
            job_rx,
            marks: HashMap::new(),
            clip_restore: None,
            confirm_pending: None,
            repeating: None,
            night: false,
            run_dots: HashSet::new(),
            run_check: Instant::now(),
            profile_apps: Vec::new(),
            profile_mtimes: HashMap::new(),
            profiles_check: Instant::now() - Duration::from_secs(60),
            auto_profiles: true,
            auto_profile_from: None,
            services_check: Instant::now() - Duration::from_secs(60),
            services_sig: String::new(),
            services_last_full: Instant::now() - Duration::from_secs(60),
            notice: None,
        };
        e.sync_fader_targets();
        crate::weather::set_city(&e.cfg.general.weather_city);
        e.load_layout();
        e.apply_sound_devices();
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
            notice: self.notice.clone(),
            config_error: self.config_error.clone(),
            profile: self.active.lock().map(|a| a.name.clone()).unwrap_or_default(),
            game: self.game.is_some(),
            leds: self.leds.clone(),
            base_rot: self.base_rot,
            doom: self.doom.is_some(),
            doom_error: self.doom_error.clone(),
            vars: self.vars.all().clone(),
            pickup: self.pickup.iter().map(|p| p.is_some()).collect(),
            asleep: self.asleep,
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
            self.poll_jobs();
            self.vars.save_if_due();
            if self.sleep_check.elapsed() >= Duration::from_secs(1) {
                self.sleep_check = Instant::now();
                self.check_sleep();
                self.check_night();
            }
            if self.run_check.elapsed() >= Duration::from_secs(2) {
                self.run_check = Instant::now();
                self.check_running_apps();
            }
            if self.profiles_check.elapsed() >= Duration::from_secs(5) {
                self.profiles_check = Instant::now();
                self.load_profile_apps();
            }
            self.tick_repeat();
            self.tick_services();
            if self.doom.is_some() {
                self.tick_doom();
            } else if self.game.is_some() {
                self.tick_game();
            } else if Instant::now() >= self.widgets_due {
                self.update_widgets(false);
            }
            let busy_lights = !self.marks.is_empty()
                || self.repeating.is_some()
                || self.clip_restore.is_some()
                || self.preview.is_some()
                || self.base_soft
                || self.gesture.is_some()
                || self.pickup.iter().any(|p| p.is_some())
                || (0..FADER_COUNT).any(|i| self.fader_slot(i).led != "fixed");
            let timeout = if self.anims.is_empty() && self.macro_queue.is_empty() && self.game.is_none() && self.doom.is_none() && !busy_lights { 30 } else { 5 };
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
        crate::services::stop_all();
        self.release_held();
        self.vars.save_now();
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
            EngineMsg::SyncServices => {
                self.services_check = Instant::now() - Duration::from_secs(60);
                self.services_last_full = Instant::now() - Duration::from_secs(60);
            }
            EngineMsg::Redraw => {
                self.render_all();
                self.show_page();
            }
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
            EngineMsg::TestSound { file, volume } => {
                self.play_sound("test", &file, volume, "restart");
            }
            EngineMsg::ClearVars => {
                self.vars.clear();
                self.vars.save_now();
                let _ = std::fs::remove_dir_all(self.cfg_dir.join("variables"));
                self.refresh_var_keys();
                self.notify();
            }
            EngineMsg::StartDoom(wad) => self.start_doom(&wad),
            EngineMsg::StopDoom => self.end_doom(),
            EngineMsg::DoomKey { pressed, code } => {
                if let Some(d) = self.doom.as_mut() {
                    d.sess.key(pressed, code);
                }
            }
            EngineMsg::ShowPage(p) => {
                if p < self.visuals.len() && p != self.page {
                    self.auto_from = None;
                    self.page_stack.clear();
                    self.change_page(p);
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
        crate::weather::set_city(&self.cfg.general.weather_city);
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
            let leds_changed = old.general.base_color != self.cfg.general.base_color
                || old.general.base_effect != self.cfg.general.base_effect
                || old.general.led_brightness != self.cfg.general.led_brightness
                || old.lights != self.cfg.lights;
            if k == Kind::Keys15 || leds_changed {
                self.setup_module(&s, k);
            }
        }
        if old.general.sound_device != self.cfg.general.sound_device || old.general.sound_device2 != self.cfg.general.sound_device2 {
            self.apply_sound_devices();
        }
        if self.cfg.general.vars_files && !old.general.vars_files {
            self.vars.export(&self.cfg_dir.join("variables"), None);
        }
        if old.general.sleep_after != self.cfg.general.sleep_after || old.general.sleep_locked != self.cfg.general.sleep_locked {
            self.last_device_input = Instant::now();
        }
        if old.lights.animations != self.cfg.lights.animations {
            self.lights_t0 = Instant::now();
        }
        // Faders qui contrôlent maintenant autre chose : reprise douce.
        self.arm_pickups();
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
        let vars = self.vars.all().clone();
        let mut var_keys = Vec::new();
        let cfg = &self.cfg;
        self.visuals = cfg
            .pages
            .iter()
            .enumerate()
            .map(|(p, page)| {
                (1..=KEY_COUNT as u8)
                    .map(|k| match page.keys.iter().find(|d| d.key == k) {
                        Some(def) => {
                            if key_mentions_vars(def) {
                                var_keys.push((p, k - 1));
                            }
                            render_key_visuals(&cfg.themed(p, def), &vars, &dir, rot)
                        }
                        None => KeyVisuals { states: vec![render::blank()] },
                    })
                    .collect()
            })
            .collect();
        self.var_keys = var_keys;
        if self.visuals.is_empty() {
            self.visuals.push(vec![KeyVisuals { states: vec![render::blank()] }; KEY_COUNT]);
        }
        log::debug!("rendu des touches en {} ms", t.elapsed().as_millis());
    }

    /// Redessine les touches qui affichent une variable (après un changement).
    fn refresh_var_keys(&mut self) {
        let vars = self.vars.all().clone();
        let dir = self.cfg_dir.clone();
        for (p, k) in self.var_keys.clone() {
            let Some(def) = self.cfg.pages.get(p).and_then(|pg| pg.keys.iter().find(|d| d.key == k + 1)).map(|d| self.cfg.themed(p, d)) else {
                continue;
            };
            let v = render_key_visuals(&def, &vars, &dir, self.keys_rot);
            if let Some(slot) = self.visuals.get_mut(p).and_then(|pg| pg.get_mut(k as usize)) {
                *slot = v;
            }
            if p == self.page && self.game.is_none() {
                self.widget_sig.remove(&k);
                let f = self.visual(k).frames[0].clone();
                let jpeg = if self.pressed[k as usize] && self.cfg.general.press_feedback { f.pressed } else { f.jpeg };
                self.send_image(k, &jpeg);
                self.restart_anim(k);
            }
        }
    }

    /// Valeur d'un nom entre accolades : variable, ou état du PC.
    fn lookup(&self, name: &str) -> Option<String> {
        match name {
            "app" => Some(foreground::foreground_exe().unwrap_or_default()),
            "page" => Some((self.page + 1).to_string()),
            "profile" => Some(self.active.lock().map(|a| a.name.clone()).unwrap_or_default()),
            "hour" => Some(widgets::local_time().hour.to_string()),
            "minute" => Some(widgets::local_time().minute.to_string()),
            // 1 = lundi … 7 = dimanche
            "weekday" => Some(match widgets::local_time().dow {
                0 => 7,
                d => d,
            }
            .to_string()),
            _ => self.vars.get(name).cloned(),
        }
    }

    /// Résultats des requêtes web et de l'IA, liserés et presse-papiers à remettre.
    fn poll_jobs(&mut self) {
        while let Ok(job) = self.job_rx.try_recv() {
            match job.result {
                Ok(out) => {
                    self.set_mark(job.at, MARK_OK, Duration::from_millis(900));
                    match out {
                        JobOut::Nothing => {}
                        JobOut::Var(name, value) => {
                            if self.vars.apply(&name, "set", &value) {
                                if self.cfg.general.vars_files {
                                    self.vars.export(&self.cfg_dir.join("variables"), Some(&name));
                                }
                                self.refresh_var_keys();
                                self.notify();
                            }
                        }
                        JobOut::Paste { text, restore } => {
                            if clip::set_text(&text) {
                                self.input.hotkey("ctrl+v");
                                self.clip_restore = restore.map(|r| (r, Instant::now() + Duration::from_millis(700)));
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("action en arrière-plan : {e}");
                    self.set_mark(job.at, MARK_ERR, Duration::from_millis(2500));
                }
            }
        }
        if let Some((_, due)) = &self.clip_restore {
            if Instant::now() >= *due {
                let (text, _) = self.clip_restore.take().unwrap();
                clip::set_text(&text);
            }
        }
        if !self.marks.is_empty() {
            let now = Instant::now();
            let done: Vec<(u8, usize)> = self.marks.iter().filter(|(_, (_, _, end))| now >= *end).map(|(k, (p, _, _))| (*k, *p)).collect();
            for (key, page) in done {
                self.marks.remove(&key);
                if page == self.page && self.game.is_none() {
                    let f = self.current_frame(key).jpeg.clone();
                    self.send_image(key, &f);
                }
            }
        }
    }

    /// Entoure une touche d'un liseré de couleur pendant `dur`.
    fn set_mark(&mut self, at: Option<(usize, u8)>, color: [u8; 3], dur: Duration) {
        let Some((page, key)) = at else { return };
        self.marks.insert(key, (page, color, Instant::now() + dur));
        if page == self.page && self.game.is_none() && (key as usize) < KEY_COUNT {
            let f = self.current_frame(key).jpeg.clone();
            let marked = Arc::new(render::with_border(&f, color));
            self.send_image(key, &marked);
        }
    }

    /// Remplace les {noms} connus ; `blank_unknown` : les variables inconnues
    /// valent "" (conditions) au lieu de rester telles quelles (textes).
    fn expand(&self, text: &str, blank_unknown: bool) -> String {
        vars::expand(text, &|n| self.lookup(n).or_else(|| if blank_unknown { Some(String::new()) } else { None }))
    }

    // ------------------------------------------------------------------
    // Modules
    // ------------------------------------------------------------------

    /// Couleur(s), puis mode, puis luminosité, avec de courtes pauses :
    /// le module n'applique une couleur qu'après la commande de mode.
    /// Luminosité des LED (0 en veille).
    fn led_level(&self) -> u8 {
        if self.asleep {
            0
        } else {
            self.dimmed(self.cfg.general.led_brightness)
        }
    }

    /// Luminosité des écrans des touches (0 en veille, réduite la nuit).
    fn screen_level(&self) -> u8 {
        if self.asleep {
            0
        } else {
            self.dimmed(self.cfg.general.brightness)
        }
    }

    fn dimmed(&self, level: u8) -> u8 {
        if self.night {
            let pct = self.cfg.general.night_level.clamp(5, 100) as u32;
            ((level as u32 * pct + 50) / 100).max(1).min(100) as u8
        } else {
            level
        }
    }

    /// Mode nuit : entre l'heure de début et l'heure de fin (qui peut être le lendemain).
    fn check_night(&mut self) {
        let g = &self.cfg.general;
        let want = g.night_dim && {
            let h = widgets::local_time().hour as u8;
            let (from, to) = (g.night_from.min(23), g.night_to.min(23));
            if from == to {
                false
            } else if from < to {
                h >= from && h < to
            } else {
                h >= from || h < to
            }
        };
        if want != self.night {
            log::info!("{}", if want { "mode nuit : luminosité réduite" } else { "fin du mode nuit" });
            self.night = want;
            let serials: Vec<(String, Kind)> = self.modules.iter().map(|(s, k)| (s.clone(), *k)).collect();
            for (s, k) in serials {
                if k == Kind::Keys15 {
                    self.mgr.send(&s, Command::Feature(protocol::keys15_brightness(self.screen_level())));
                } else {
                    self.setup_module(&s, k);
                }
            }
        }
    }

    /// Veille : après X minutes sans rien toucher (PC et MasterHUB), ou
    /// quand la session est verrouillée.
    fn check_sleep(&mut self) {
        let g = &self.cfg.general;
        let idle = crate::pcstate::idle_seconds().min(self.last_device_input.elapsed().as_secs());
        let by_idle = g.sleep_after > 0 && idle >= g.sleep_after as u64 * 60;
        let by_lock = g.sleep_locked && crate::pcstate::is_locked();
        let want = (by_idle || by_lock) && self.doom.is_none();
        if want != self.asleep {
            self.set_asleep(want);
        }
    }

    fn set_asleep(&mut self, asleep: bool) {
        log::info!("{}", if asleep { "MasterHUB en veille" } else { "MasterHUB réveillé" });
        self.asleep = asleep;
        self.release_held();
        let serials: Vec<(String, Kind)> = self.modules.iter().map(|(s, k)| (s.clone(), *k)).collect();
        for (s, k) in serials {
            self.setup_module(&s, k);
        }
        self.notify();
    }

    /// Un module a été touché : sort de veille. Vrai si l'appui ne doit
    /// servir qu'à réveiller.
    fn device_touched(&mut self) -> bool {
        self.last_device_input = Instant::now();
        if self.asleep && !(self.cfg.general.sleep_locked && crate::pcstate::is_locked()) {
            self.set_asleep(false);
            return true;
        }
        false
    }

    fn setup_leds(&self, serial: &str, target: u8, colors: &[(u8, (u8, u8, u8))], mode: u8) {
        for (index, (r, gg, b)) in colors {
            self.mgr.send(serial, Command::Output(protocol::led_color(target, *index, *r, *gg, *b, false)));
            self.mgr.send(serial, Command::Sleep(30));
        }
        let all = if target == 0 { 0 } else { 0xFF };
        self.mgr.send(serial, Command::Output(protocol::led_mode(target, all, mode, false)));
        self.mgr.send(serial, Command::Sleep(30));
        self.mgr.send(serial, Command::Output(protocol::led_brightness(target, all, self.led_level())));
    }

    fn setup_module(&mut self, serial: &str, kind: Kind) {
        let g = self.cfg.general.clone();
        match kind {
            Kind::Keys15 => {
                self.mgr.send(serial, Command::Feature(protocol::keys15_sw_mode(true)));
                self.mgr.send(serial, Command::Feature(protocol::keys15_brightness(self.screen_level())));
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
                if self.modules.get(&serial) == Some(&Kind::Keys15) {
                    self.release_held();
                }
                self.sent_base.remove(&serial);
                self.sent_faders.remove(&serial);
                if let Some(k) = self.modules.remove(&serial) {
                    log::info!("✗ {} débranché", k.name());
                }
                self.modules_changed = true;
                self.faders.remove(&serial);
            }
            Event::KeyDown { key, .. } => {
                // Un appui qui réveille le MasterHUB ne lance pas d'action.
                if self.device_touched() {
                    self.wake_swallow = Some(key);
                    return;
                }
                self.key_down(key)
            }
            Event::KeyUp { key, .. } => {
                if self.wake_swallow == Some(key) {
                    self.wake_swallow = None;
                    return;
                }
                self.key_up(key)
            }
            Event::Faders { serial, values } => {
                self.device_touched();
                self.faders_changed(serial, values)
            }
            Event::Roller { wheel, step, .. } => {
                self.device_touched();
                self.acting = None;
                self.roller(wheel, step)
            }
        }
    }

    // ------------------------------------------------------------------
    // Touches
    // ------------------------------------------------------------------

    fn is_toggled(&self, key: u8) -> bool {
        if let Some(name) = self.key_def(key).and_then(|d| d.state_var.as_deref()).filter(|n| !n.is_empty()) {
            return truthy(self.vars.all().get(name).map(|s| s.as_str()).unwrap_or(""));
        }
        self.toggled.get(&(self.page, key)).copied().unwrap_or(false)
    }

    /// Addons avec programme : démarrage / arrêt, et messages reçus.
    fn tick_services(&mut self) {
        if self.services_check.elapsed() >= Duration::from_secs(4) {
            self.services_check = Instant::now();
            let sig = crate::addons::signature(&self.cfg_dir);
            // Rescan complet si les addons ont changé (ou sur demande), sinon
            // toutes les 30 s pour relancer ce qui doit l'être.
            if sig != self.services_sig || self.services_last_full.elapsed() >= Duration::from_secs(30) {
                self.services_sig = sig;
                self.services_last_full = Instant::now();
                crate::services::sync(crate::addons::desired_services(&self.cfg_dir));
            }
        }
        let msgs = crate::services::poll();
        if msgs.is_empty() {
            return;
        }
        let mut vars_changed: Vec<String> = Vec::new();
        for m in msgs {
            match m {
                crate::services::Incoming::SetVar { name, value } => {
                    if self.vars.all().get(&name) != Some(&value) && self.vars.apply(&name, "set", &value) {
                        if self.cfg.general.vars_files {
                            self.vars.export(&self.cfg_dir.join("variables"), Some(&name));
                        }
                        vars_changed.push(name);
                    }
                }
                crate::services::Incoming::Notify { addon, text, error } => {
                    let id = self.notice.as_ref().map(|n| n.0 + 1).unwrap_or(1);
                    if error {
                        log::warn!("addon {addon} : {text}");
                    } else {
                        log::info!("addon {addon} : {text}");
                    }
                    self.notice = Some((id, addon, text, error));
                    self.notify();
                }
                crate::services::Incoming::Mark { page, key, ok } => {
                    if (key as usize) < KEY_COUNT {
                        self.set_mark(Some((page, key)), if ok { MARK_OK } else { MARK_ERR }, Duration::from_millis(if ok { 900 } else { 2500 }));
                    }
                }
            }
        }
        if !vars_changed.is_empty() {
            self.refresh_var_keys();
            // Touches à 2 états qui suivent une de ces variables.
            let keys: Vec<u8> = self
                .cfg
                .pages
                .get(self.page)
                .map(|p| p.keys.iter().filter(|d| d.state_var.as_ref().map(|n| vars_changed.contains(n)).unwrap_or(false)).map(|d| d.key - 1).collect())
                .unwrap_or_default();
            if self.game.is_none() {
                for k in keys.into_iter().filter(|k| (*k as usize) < KEY_COUNT) {
                    let f = self.visual(k).frames[0].clone();
                    self.send_image(k, &f.jpeg);
                    self.restart_anim(k);
                }
            }
            self.notify();
        }
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
        // Pendant Doom, c'est lui qui dessine sur les touches.
        if self.doom.is_some() {
            return;
        }
        if self.run_dots.contains(&key) && self.game.is_none() {
            let dotted = Arc::new(render::with_dot(jpeg, self.keys_rot));
            self.send_image_raw(key, &dotted);
            return;
        }
        self.send_image_raw(key, jpeg);
    }

    /// Point vert sur les touches « Lancer » dont l'application est ouverte.
    fn check_running_apps(&mut self) {
        if self.game.is_some() || self.doom.is_some() {
            return;
        }
        let Some(page) = self.cfg.pages.get(self.page) else { return };
        let targets: Vec<(u8, String)> = page
            .keys
            .iter()
            .filter(|d| d.toggle.is_none() && d.widget.is_none())
            .filter_map(|d| match &d.action {
                Action::Launch { target, .. } => foreground::exe_of_target(target).map(|e| (d.key.saturating_sub(1), e)),
                _ => None,
            })
            .filter(|(k, _)| (*k as usize) < KEY_COUNT)
            .collect();
        let want: HashSet<u8> = if targets.is_empty() {
            HashSet::new()
        } else {
            let running: HashSet<String> = foreground::running_apps().into_iter().collect();
            targets.into_iter().filter(|(_, e)| running.contains(e)).map(|(k, _)| k).collect()
        };
        if want == self.run_dots {
            return;
        }
        let changed: Vec<u8> = want.symmetric_difference(&self.run_dots).copied().collect();
        self.run_dots = want;
        for k in changed {
            let f = self.current_frame(k).clone();
            let jpeg = if self.pressed[k as usize] && self.cfg.general.press_feedback { f.pressed } else { f.jpeg };
            self.send_image(k, &jpeg);
        }
    }

    fn send_image_raw(&self, key: u8, jpeg: &Arc<Vec<u8>>) {
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
        let Some(defs) = self.cfg.pages.get(page).map(|p| p.keys.iter().map(|d| self.cfg.themed(page, d)).collect::<Vec<_>>()) else {
            return Vec::new();
        };
        if !defs.iter().any(|d| d.widget.is_some()) {
            return Vec::new();
        }
        self.sampler.want_gpu = defs.iter().any(|d| d.widget.as_deref().map(widgets::needs_gpu).unwrap_or(false));
        self.sampler.want_net = defs.iter().any(|d| d.widget.as_deref() == Some("net"));
        let mut snap = self.sampler.sample(self.audio.state());
        let now = Instant::now();
        for d in defs.iter().filter(|d| d.widget.as_deref() == Some("timer")) {
            let t = self.timers.get(&(page, d.key.saturating_sub(1))).copied().unwrap_or_default();
            snap.timers.insert(d.key.saturating_sub(1), (t.elapsed(now).as_millis() as u64, t.since.is_some()));
        }
        let targets: Vec<String> =
            defs.iter().filter(|d| d.widget.as_deref() == Some("app_volume")).filter_map(widgets::app_target).collect();
        if !targets.is_empty() {
            // Résultat au tour suivant (toutes les 500 ms).
            self.audio.send(AudioCmd::Measure(targets.clone()));
            snap.apps = targets.into_iter().filter_map(|t| self.audio.meter_opt(&t).map(|m| (t, m))).collect();
        }
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
        if self.doom.is_some() {
            self.doom_pad(key as usize, true);
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
        let now = Instant::now();
        // Second appui d'un double appui ?
        if let Some(g) = self.gesture.as_mut() {
            if g.key == key && g.page == self.page && g.up_at.is_some() && !g.done {
                g.done = true;
                g.up_at = None;
                if let Some(a) = def.double.clone() {
                    self.acting = Some((self.page, key));
                    self.run_extra_action(&a);
                }
                return;
            }
        }
        // Une autre touche attendait un double appui : c'était un appui simple.
        self.flush_gesture();
        if def.long.is_some() || def.double.is_some() {
            self.gesture = Some(Gesture { key, page: self.page, down_at: now, up_at: None, done: false });
            return;
        }
        self.press_key_action(key, def);
    }

    /// Action « appui simple » d'une touche (avec ses deux états éventuels).
    fn press_key_action(&mut self, key: u8, def: KeyDef) {
        self.acting = Some((self.page, key));
        // Action sensible : il faut appuyer une seconde fois pour confirmer.
        if def.confirm {
            let now = Instant::now();
            match self.confirm_pending {
                Some((p, k, until)) if p == self.page && k == key && now < until => self.confirm_pending = None,
                _ => {
                    self.confirm_pending = Some((self.page, key, now + CONFIRM_WINDOW));
                    self.set_mark(Some((self.page, key)), MARK_CONFIRM, CONFIRM_WINDOW);
                    return;
                }
            }
            self.marks.remove(&key);
        }
        let state = if def.toggle.is_some() && self.is_toggled(key) { 1 } else { 0 };
        let action = def.state(state).action;
        if def.toggle.is_some() && def.state_var.is_none() {
            let page = self.page;
            self.toggled.insert((page, key), state == 0);
        }
        // « Maintenir » : les touches restent enfoncées jusqu'au relâchement.
        if let Action::Hold { keys } = &action {
            if self.pressed[key as usize] && !self.held.contains_key(&key) {
                if self.input.press(keys) {
                    self.held.insert(key, keys.clone());
                }
                return;
            }
        }
        // Son d'une touche : identifié par la page et la touche.
        if let Action::Sound { file, volume, mode } = &action {
            let id = format!("{}:{}", self.page, key);
            self.play_sound(&id, file, *volume, mode);
            self.nudge_widgets();
            return;
        }
        let page_before = self.page;
        self.execute(&action);
        self.nudge_widgets();
        if def.repeat && def.toggle.is_none() && repeatable(&action) && self.pressed[key as usize] && self.page == page_before {
            self.repeating = Some((self.page, key, Instant::now() + REPEAT_FIRST));
        }
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

    /// Répète l'action de la touche maintenue.
    fn tick_repeat(&mut self) {
        let Some((page, key, due)) = self.repeating else { return };
        if page != self.page || !self.pressed.get(key as usize).copied().unwrap_or(false) || self.game.is_some() {
            self.repeating = None;
            return;
        }
        if Instant::now() < due {
            return;
        }
        let Some(action) = self.key_def(key).map(|d| d.action.clone()) else {
            self.repeating = None;
            return;
        };
        self.acting = Some((page, key));
        self.execute(&action);
        self.nudge_widgets();
        self.repeating = Some((page, key, Instant::now() + REPEAT_EVERY));
    }

    fn key_up(&mut self, key: u8) {
        if matches!(self.repeating, Some((_, k, _)) if k == key) {
            self.repeating = None;
        }
        if let Some(combo) = self.held.remove(&key) {
            self.input.release(&combo);
        }
        if (key as usize) < KEY_COUNT && self.doom.is_some() {
            self.doom_pad(key as usize, false);
            return;
        }
        if (key as usize) >= KEY_COUNT || self.game.is_some() {
            return;
        }
        self.pressed[key as usize] = false;
        if self.cfg.general.press_feedback {
            let jpeg = self.current_frame(key).jpeg.clone();
            self.send_image(key, &jpeg);
        }
        let has_double = self.key_def(key).map(|d| d.double.is_some()).unwrap_or(false);
        if let Some(g) = self.gesture.as_mut() {
            if g.key == key {
                if g.done {
                    self.gesture = None;
                } else if has_double {
                    // On attend un éventuel second appui.
                    g.up_at = Some(Instant::now());
                } else {
                    self.gesture = None;
                    if let Some(def) = self.key_def(key).cloned() {
                        self.press_key_action(key, def);
                    }
                }
            }
        }
    }

    /// Joue un son de la soundboard (volume du son × volume général).
    fn play_sound(&mut self, id: &str, file: &str, volume: u8, mode: &str) {
        let path = std::path::Path::new(file);
        let path = if path.is_absolute() { path.to_path_buf() } else { self.cfg_dir.join(path) };
        let v = volume as f32 / 100.0 * self.cfg.general.sound_volume.min(100) as f32 / 100.0;
        self.sounds.send(SoundCmd::Play { id: id.to_string(), path, volume: v, mode: mode.to_string() });
    }

    fn apply_sound_devices(&self) {
        let g = &self.cfg.general;
        self.sounds.send(SoundCmd::Devices { main: g.sound_device.clone(), second: g.sound_device2.clone() });
    }

    /// Relâche toutes les touches maintenues (changement de page, fin…).
    fn release_held(&mut self) {
        for (_, combo) in std::mem::take(&mut self.held) {
            self.input.release(&combo);
        }
    }

    fn execute(&mut self, action: &Action) {
        match action {
            Action::None => {}
            Action::Hotkey { keys } => self.input.hotkey(keys),
            // Dans une macro ou un appui long : simple appui-relâché.
            Action::Hold { keys } => self.input.hotkey(keys),
            Action::Sound { file, volume, mode } => {
                let id = format!("{}:{}", self.page, file);
                self.play_sound(&id, file, *volume, mode);
            }
            Action::StopSounds => self.sounds.send(SoundCmd::StopAll),
            Action::CloseApp { target, force } => foreground::close_app(target, *force),
            Action::Service { addon, event, data } => {
                let key = self.acting.map(|(p, k)| serde_json::json!({ "page": p + 1, "key": k + 1 })).unwrap_or(serde_json::Value::Null);
                let msg = serde_json::json!({
                    "type": "action",
                    "event": event,
                    "data": serde_json::to_value(data).unwrap_or(serde_json::Value::Null),
                    "key": key,
                });
                if !crate::services::send(addon, &msg) {
                    log::warn!("addon {addon} : son programme ne tourne pas (activé et autorisé dans sa fiche ?)");
                    self.set_mark(self.acting, MARK_ERR, Duration::from_millis(1500));
                }
            }
            Action::Text { text } => {
                let t = self.expand(text, false);
                self.input.text(&t);
            }
            Action::Var { name, op, value } => {
                let v = self.expand(value, true);
                if self.vars.apply(name, op, &v) {
                    if self.cfg.general.vars_files {
                        self.vars.export(&self.cfg_dir.join("variables"), Some(name));
                    }
                    self.refresh_var_keys();
                    self.notify();
                }
            }
            Action::Http { method, url, body, headers, save_to, pick } => {
                let url = self.expand(url, false);
                let body = self.expand(body, false);
                let headers = self.expand(headers, false);
                let (method, save, pick) = (method.clone(), save_to.trim().to_string(), pick.clone());
                let at = self.acting;
                self.set_mark(at, MARK_BUSY, Duration::from_secs(30));
                let tx = self.job_tx.clone();
                std::thread::spawn(move || {
                    let result = connect::http(&method, &url, &body, &headers).and_then(|resp| {
                        if save.is_empty() {
                            Ok(JobOut::Nothing)
                        } else {
                            connect::pick(&resp, &pick).map(|v| JobOut::Var(save, v))
                        }
                    });
                    let _ = tx.send(JobDone { at, result });
                });
            }
            Action::Ai { preset, prompt, output, save_to } => {
                let at = self.acting;
                let settings = connect::load_settings(&self.cfg_dir);
                if !settings.is_configured() {
                    log::warn!("IA : pas encore réglée (Réglages › Intelligence artificielle)");
                    self.set_mark(at, MARK_ERR, Duration::from_secs(2));
                    return;
                }
                // Copie de la sélection : on garde l'ancien presse-papiers.
                let original = clip::get_text();
                let seq = clip::sequence();
                self.input.hotkey("ctrl+c");
                self.set_mark(at, MARK_BUSY, Duration::from_secs(95));
                let instruction = connect::instruction(preset, prompt);
                let (output, save) = (output.clone(), save_to.trim().to_string());
                let tx = self.job_tx.clone();
                std::thread::spawn(move || {
                    let result = (|| {
                        let t0 = Instant::now();
                        while clip::sequence() == seq && t0.elapsed() < Duration::from_millis(800) {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        if clip::sequence() == seq {
                            return Err("aucun texte sélectionné".to_string());
                        }
                        let text = clip::get_text().filter(|t| !t.trim().is_empty()).ok_or("aucun texte sélectionné")?;
                        if text.chars().count() > 20_000 {
                            return Err("sélection trop longue pour l'IA (20 000 caractères maximum)".into());
                        }
                        let answer = connect::ask(&settings, &instruction, &text);
                        match (output.as_str(), answer) {
                            ("replace", Ok(a)) => Ok(JobOut::Paste { text: a, restore: original }),
                            (_, Err(e)) => {
                                if let Some(o) = &original {
                                    clip::set_text(o);
                                }
                                Err(e)
                            }
                            ("var", Ok(a)) => {
                                if let Some(o) = &original {
                                    clip::set_text(o);
                                }
                                Ok(JobOut::Var(save, connect::clip(&a)))
                            }
                            (_, Ok(a)) => {
                                clip::set_text(&a);
                                Ok(JobOut::Nothing)
                            }
                        }
                    })();
                    let _ = tx.send(JobDone { at, result });
                });
            }
            Action::AudioOutput { devices } => {
                let list: Vec<String> = devices.split('|').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                self.audio.send(AudioCmd::SetOutput(list));
                self.audio.send(AudioCmd::Poll);
                self.widgets_due = self.widgets_due.min(Instant::now() + Duration::from_millis(300));
            }
            Action::Timer { op } => {
                let Some((page, key)) = self.acting else { return };
                let now = Instant::now();
                let total = self.cfg.pages.get(page).and_then(|p| p.keys.iter().find(|d| d.key == key + 1)).and_then(widgets::timer_total);
                let t = self.timers.entry((page, key)).or_default();
                match op.as_str() {
                    "reset" => *t = TimerState::default(),
                    _ => {
                        let finished = total.map(|tot| t.elapsed(now) >= tot).unwrap_or(false);
                        if finished {
                            *t = TimerState { acc: Duration::ZERO, since: Some(now) };
                        } else if let Some(s) = t.since.take() {
                            t.acc += now.saturating_duration_since(s);
                        } else {
                            t.since = Some(now);
                        }
                    }
                }
                self.widgets_due = Instant::now();
            }
            Action::If { left, cmp, right, then_steps, else_steps } => {
                let (l, r) = (self.expand(left, true), self.expand(right, true));
                let ok = vars::compare(&l, cmp, &r);
                log::debug!("condition « {l} » {cmp} « {r} » → {}", if ok { "oui" } else { "non" });
                let branch: Vec<Action> = if ok { then_steps } else { else_steps }
                    .iter()
                    .filter(|s| !matches!(s, Action::Macro { .. }))
                    .cloned()
                    .collect();
                if self.macro_queue.len() + branch.len() > 4 * config::MACRO_MAX_STEPS {
                    log::warn!("trop d'étapes en attente : condition ignorée");
                    return;
                }
                if self.in_macro {
                    // Dans une macro : la branche passe avant les étapes suivantes.
                    for s in branch.into_iter().rev() {
                        self.macro_queue.push_front(s);
                    }
                } else {
                    self.macro_queue.extend(branch);
                    self.run_macro();
                }
            }
            Action::Launch { target, args } => actions::launch(target, args),
            Action::Media { command } if command == "mic_mute" => self.audio.send(AudioCmd::ToggleMicMute),
            Action::Media { command } => self.input.media(command),
            Action::AppMute { app } => {
                let names: Vec<String> = app.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
                if !names.is_empty() {
                    self.audio.send(AudioCmd::ToggleAppMute(names));
                    // La touche « volume d'une application » se met à jour tout de suite.
                    self.widgets_due = self.widgets_due.min(Instant::now() + Duration::from_millis(120));
                }
            }
            Action::Lock => actions::lock_session(),
            Action::Macro { steps } => {
                if self.macro_queue.len() + steps.len() > 4 * config::MACRO_MAX_STEPS {
                    log::warn!("trop de macros en attente : appui ignoré");
                    return;
                }
                // Les étapes s'ajoutent à la suite d'une macro déjà en cours.
                self.macro_queue.extend(steps.iter().filter(|s| !matches!(s, Action::Macro { .. })).cloned());
                if !self.in_macro {
                    self.run_macro();
                }
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
        // Choisi à la main : le mode dynamique ne ramène plus à l'ancien profil.
        self.auto_profile_from = None;
        self.switch_profile_with(name, true);
    }

    /// `remember` : profil retenu pour le prochain démarrage (pas pour un
    /// changement automatique du mode dynamique).
    fn switch_profile_with(&mut self, name: &str, remember: bool) {
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
        if remember {
            let mut st = profiles::load_settings(&self.cfg_dir);
            st.profile = name.to_string();
            profiles::save_settings(&self.cfg_dir, &st);
        }
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
    // Easter egg : Doom
    // ------------------------------------------------------------------

    fn start_doom(&mut self, wad: &std::path::Path) {
        if self.doom.is_some() {
            return;
        }
        if self.game.is_some() {
            self.game = None;
        }
        self.anims.clear();
        self.macro_queue.clear();
        self.preview = None;
        match doom::Session::start(wad, &doom::doom_dir(&self.cfg_dir)) {
            Ok(sess) => {
                let now = Instant::now();
                self.doom = Some(DoomPlay {
                    sess,
                    seen: 0,
                    tile_hash: [0; KEY_COUNT],
                    held: HashMap::new(),
                    pulses: Vec::new(),
                    esc_since: None,
                    leds_due: now,
                });
                self.doom_error = None;
                self.pressed = [false; KEY_COUNT];
            }
            Err(e) => {
                log::error!("Doom : {e}");
                self.doom_error = Some(e);
            }
        }
        self.notify();
    }

    fn end_doom(&mut self) {
        if self.doom.take().is_none() {
            return;
        }
        self.pressed = [false; KEY_COUNT];
        self.show_page();
        self.restore_leds();
        self.led_due = Instant::now();
        self.notify();
    }

    /// Appui ou relâchement d'une touche du MasterHUB pendant Doom.
    fn doom_pad(&mut self, physical: usize, down: bool) {
        let rot = self.keys_rot;
        let Some(d) = self.doom.as_mut() else { return };
        let idx = doom::visible_index(rot, physical).min(KEY_COUNT - 1);
        let codes = doom::PAD[idx];
        if codes.contains(&doom::key::ESCAPE) {
            d.esc_since = if down { Some(Instant::now()) } else { None };
        }
        for &c in codes {
            let n = d.held.entry(c).or_insert(0);
            if down {
                *n += 1;
                if *n == 1 {
                    d.sess.key(true, c);
                }
            } else if *n > 0 {
                *n -= 1;
                if *n == 0 {
                    d.held.remove(&c);
                    d.sess.key(false, c);
                }
            }
        }
    }

    fn tick_doom(&mut self) {
        let now = Instant::now();
        let rot = self.keys_rot;
        let keys_busy = self.mgr.pending_kind(Kind::Keys15);
        let leds_busy = self.mgr.pending_kind(Kind::Faders5) + self.mgr.pending_kind(Kind::BaseLed);
        let Some(d) = self.doom.as_mut() else { return };
        if !d.sess.alive() {
            self.end_doom();
            return;
        }
        // Appui long sur Échap : on quitte Doom.
        if d.esc_since.map(|t| now.saturating_duration_since(t) > Duration::from_millis(1500)).unwrap_or(false) {
            self.end_doom();
            return;
        }
        // Crans des molettes : relâcher les touches arrivées à échéance.
        let mut released = Vec::new();
        d.pulses.retain(|(t, k)| {
            if now >= *t {
                released.push(*k);
                false
            } else {
                true
            }
        });
        for k in released {
            if !d.held.contains_key(&k) {
                d.sess.key(false, k);
            }
        }

        let Some((seq, frame)) = d.sess.frame_after(d.seen) else { return };
        // L'image n'est affichée que sur les touches du MasterHUB.
        // MasterHUB : seulement quand le module a fini d'afficher l'image précédente.
        if keys_busy == 0 {
            d.seen = seq;
            let tiles = doom::tiles(&frame, rot);
            let mut to_send = Vec::new();
            for (k, t) in tiles.iter().enumerate() {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                t.as_raw().hash(&mut h);
                let hash = h.finish();
                if d.tile_hash[k] != hash {
                    d.tile_hash[k] = hash;
                    to_send.push((k as u8, Arc::new(render::encode_jpeg(t, 55))));
                }
            }
            for (k, jpeg) in to_send {
                self.send_image_raw(k, &jpeg);
            }
        }
        let Some(d) = self.doom.as_mut() else { return };
        // LED : vie sur les faders, couleur de l'action sur la base.
        if now >= d.leds_due && leds_busy == 0 {
            d.leds_due = now + Duration::from_millis(110);
            let t = now.saturating_duration_since(self.lights_t0).as_millis() as u64;
            let faders = doom::health_leds(frame.health, frame.damage, t);
            let base = if frame.damage > 0 {
                (255, 0, 0)
            } else if frame.bonus > 0 {
                (255, 200, 40)
            } else {
                doom::average_color(&frame)
            };
            self.send_live_leds(Some(base), Some(faders));
        }
    }

    /// Envoie des couleurs aux LED en direct (seulement ce qui a changé).
    fn send_live_leds(&mut self, base: Option<Rgb>, faders: Option<[Rgb; 5]>) {
        let mods: Vec<(String, Kind)> = self.modules.iter().map(|(s, k)| (s.clone(), *k)).collect();
        for (s, k) in mods {
            match (k, base, faders) {
                (Kind::BaseLed, Some(c), _) if self.sent_base.get(&s) != Some(&c) => {
                    self.mgr.send(&s, Command::Output(protocol::led_color(0, 0, c.0, c.1, c.2, false)));
                    self.mgr.send(&s, Command::Sleep(5));
                    self.mgr.send(&s, Command::Output(protocol::led_mode(0, 0, 1, false)));
                    self.sent_base.insert(s.clone(), c);
                    self.base_soft = true;
                }
                (Kind::Faders5, _, Some(target)) => {
                    let prev = self.sent_faders.get(&s).copied();
                    if prev == Some(target) {
                        continue;
                    }
                    for (i, c) in target.iter().enumerate() {
                        if prev.map(|p| p[i] != *c).unwrap_or(true) {
                            self.mgr.send(&s, Command::Output(protocol::led_color(1, i as u8, c.0, c.1, c.2, false)));
                            self.mgr.send(&s, Command::Sleep(4));
                        }
                    }
                    self.mgr.send(&s, Command::Output(protocol::led_mode(1, 0xFF, 1, false)));
                    self.sent_faders.insert(s.clone(), target);
                }
                _ => {}
            }
        }
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
    /// Action d'un appui long ou d'un double appui.
    fn run_extra_action(&mut self, action: &Action) {
        let page_before = self.page;
        self.execute(action);
        self.nudge_widgets();
        let closes = !matches!(action, Action::None | Action::Folder { .. } | Action::Back | Action::Page { .. });
        if closes && self.page == page_before && self.is_closing_folder(self.page) {
            self.go_back();
        }
    }

    /// Une touche attendait un double appui qui n'est pas venu : appui simple.
    fn flush_gesture(&mut self) {
        let Some(g) = self.gesture.take() else { return };
        if g.done || g.up_at.is_none() || g.page != self.page {
            return;
        }
        if let Some(def) = self.key_def(g.key).cloned() {
            self.press_key_action(g.key, def);
        }
    }

    /// Appui long atteint, ou délai du double appui écoulé.
    fn tick_gesture(&mut self, now: Instant) {
        let Some(g) = self.gesture.as_ref() else { return };
        let (key, down_at, up_at, done) = (g.key, g.down_at, g.up_at, g.done);
        if done {
            return;
        }
        let def = self.key_def(key).cloned();
        match up_at {
            None => {
                let long_ms = self.cfg.general.long_press_ms.clamp(150, 5000) as u64;
                if now.saturating_duration_since(down_at) >= Duration::from_millis(long_ms) {
                    if let Some(a) = def.and_then(|d| d.long) {
                        if let Some(g) = self.gesture.as_mut() {
                            g.done = true;
                        }
                        self.acting = Some((self.page, key));
                        self.run_extra_action(&a);
                    }
                }
            }
            Some(up) => {
                let double_ms = self.cfg.general.double_press_ms.clamp(100, 1500) as u64;
                if now.saturating_duration_since(up) >= Duration::from_millis(double_ms) {
                    self.gesture = None;
                    if let Some(def) = def {
                        self.press_key_action(key, def);
                    }
                }
            }
        }
    }

    fn change_page(&mut self, p: usize) {
        self.release_held();
        self.pressed = [false; KEY_COUNT];
        self.gesture = None;
        self.confirm_pending = None;
        self.repeating = None;
        self.run_dots.clear();
        self.run_check = Instant::now() - Duration::from_secs(10);
        self.page = p;
        self.show_page();
        // Les faders et molettes peuvent avoir leurs propres réglages sur cette page.
        self.led_due = Instant::now();
        self.arm_pickups();
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
        if self.in_macro {
            return;
        }
        self.in_macro = true;
        let page_before = self.page;
        // Garde-fou : une suite de conditions et de variables sans pause
        // s'exécute d'un coup, mais pas indéfiniment.
        let mut instant_steps = 0;
        while !self.macro_queue.is_empty() && Instant::now() >= self.macro_next {
            let Some(step) = self.macro_queue.pop_front() else { break };
            match step {
                Action::Wait { ms } => {
                    self.macro_next = Instant::now() + Duration::from_millis(ms.min(config::WAIT_MAX_MS));
                }
                // Variables et conditions : rien à attendre côté Windows.
                step @ (Action::Var { .. } | Action::If { .. } | Action::None) => {
                    self.execute(&step);
                    instant_steps += 1;
                    if instant_steps >= 4 * config::MACRO_MAX_STEPS {
                        self.macro_next = Instant::now() + Duration::from_millis(MACRO_GAP_MS);
                    }
                }
                other => {
                    self.execute(&other);
                    // Petit délai entre deux étapes, pour que Windows suive.
                    self.macro_next = Instant::now() + Duration::from_millis(MACRO_GAP_MS);
                }
            }
        }
        self.in_macro = false;
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

    /// Mode dynamique : passe au profil lié à l'application au premier plan,
    /// et revient au profil d'avant en la quittant. Vrai si le profil a changé.
    fn check_auto_profile(&mut self, exe: &str) -> bool {
        if !self.auto_profiles || self.profile_apps.is_empty() {
            return false;
        }
        let current = self.active.lock().map(|a| a.name.clone()).unwrap_or_default();
        let target = self.profile_apps.iter().find(|(_, apps)| apps.iter().any(|a| foreground::normalize(a) == exe)).map(|(n, _)| n.clone());
        match target {
            Some(name) if !name.eq_ignore_ascii_case(&current) => {
                if self.auto_profile_from.is_none() {
                    self.auto_profile_from = Some(current);
                }
                log::info!("{exe} au premier plan → profil « {name} »");
                let from = self.auto_profile_from.clone();
                self.switch_profile_with(&name, false);
                self.auto_profile_from = from;
                true
            }
            Some(_) => false,
            None => {
                // On quitte l'application : retour au profil d'avant, si on est
                // toujours dans un profil d'application.
                let in_app_profile = self.profile_apps.iter().any(|(n, apps)| n.eq_ignore_ascii_case(&current) && !apps.is_empty());
                match self.auto_profile_from.take() {
                    Some(prev) if in_app_profile && !prev.eq_ignore_ascii_case(&current) && profiles::list(&self.cfg_dir).contains(&prev) => {
                        log::info!("retour au profil « {prev} »");
                        self.switch_profile_with(&prev, false);
                        true
                    }
                    _ => false,
                }
            }
        }
    }

    /// Relit les applications liées à chaque profil (seulement les fichiers modifiés).
    fn load_profile_apps(&mut self) {
        self.auto_profiles = profiles::load_settings(&self.cfg_dir).auto_profiles.unwrap_or(true);
        let names = profiles::list(&self.cfg_dir);
        self.profile_apps.retain(|(n, _)| names.contains(n));
        self.profile_mtimes.retain(|n, _| names.contains(n));
        for name in names {
            let path = profiles::path(&self.cfg_dir, &name);
            let Ok(mtime) = std::fs::metadata(&path).and_then(|m| m.modified()) else { continue };
            if self.profile_mtimes.get(&name) == Some(&mtime) {
                continue;
            }
            self.profile_mtimes.insert(name.clone(), mtime);
            let apps = config::load(&path).map(|c| c.general.profile_apps).unwrap_or_default();
            self.profile_apps.retain(|(n, _)| n != &name);
            if !apps.is_empty() {
                self.profile_apps.push((name, apps));
            }
        }
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
        if self.check_auto_profile(&exe) {
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

    /// Réglage effectif du fader `i` : celui de la page, sinon le réglage commun.
    fn fader_slot(&self, i: usize) -> crate::config::FaderSlot {
        let page = self.cfg.pages.get(self.page);
        page.and_then(|p| p.faders.iter().find(|f| f.index as usize == i + 1))
            .or_else(|| self.cfg.faders.get(i))
            .cloned()
            .unwrap_or_else(|| crate::config::FaderSlot {
                index: 0,
                target: "none".into(),
                color: "#FFFFFF".into(),
                led: "fixed".into(),
                gradient: Vec::new(),
                animation: String::new(),
            })
    }

    /// Couleur voulue pour la base (None = effet intégré de la base).
    fn base_target(&self, now: Instant) -> Option<Rgb> {
        if let Some((a, t0)) = &self.preview {
            return Some(lights::frame(a, now.saturating_duration_since(*t0).as_millis() as u64).base);
        }
        let l = &self.cfg.lights;
        if l.base_mode == "animation" {
            if let Some(a) = lights::find(l, &l.base_animation) {
                return Some(lights::frame(a, self.anim_t(now)).base);
            }
        }
        None
    }

    /// Couleurs voulues pour les 5 faders d'un module : chaque fader a son propre réglage.
    fn fader_target(&self, serial: &str, now: Instant) -> [Rgb; 5] {
        if let Some((a, t0)) = &self.preview {
            return lights::frame(a, now.saturating_duration_since(*t0).as_millis() as u64).faders;
        }
        let values = self.faders.get(serial).copied().unwrap_or([0; FADER_COUNT]);
        let mut out = [(0, 0, 0); 5];
        for (i, c) in out.iter_mut().enumerate() {
            let slot = self.fader_slot(i);
            let color = parse_color(&slot.color).unwrap_or((255, 255, 255));
            let grad = if slot.gradient.is_empty() { lights::default_gradient() } else { slot.gradient.clone() };
            *c = match slot.led.as_str() {
                "position" => lights::gradient_color(&grad, values[i]),
                "animation" => match lights::find(&self.cfg.lights, &slot.animation) {
                    Some(a) => lights::frame(a, self.anim_t(now)).faders[i],
                    None => color,
                },
                "status" => {
                    let m = self.audio.meter(&slot.target);
                    if slot.target.trim() == "none" || !m.present {
                        (0, 0, 0)
                    } else if m.muted {
                        (255, 25, 25)
                    } else {
                        color
                    }
                }
                "vu" => {
                    let m = self.audio.meter(&slot.target);
                    let level = (m.peak.clamp(0.0, 1.0).sqrt() * 100.0).round() as u8;
                    let k = 0.12 + 0.88 * (level as f32 / 100.0);
                    let c = lights::gradient_color(&grad, level);
                    ((c.0 as f32 * k) as u8, (c.1 as f32 * k) as u8, (c.2 as f32 * k) as u8)
                }
                _ => color,
            };
            // En attente de reprise douce : la LED pulse doucement.
            if self.pickup[i].is_some() {
                let t = self.anim_t(now) as f32 / 1000.0;
                let k = 0.15 + 0.35 * (0.5 + 0.5 * (t * std::f32::consts::TAU / 1.2).sin());
                *c = ((c.0 as f32 * k) as u8, (c.1 as f32 * k) as u8, (c.2 as f32 * k) as u8);
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

    /// Met à jour les LED de la base et des faders (animations, position, jeux).
    fn tick_lights(&mut self) {
        let now = Instant::now();
        self.tick_gesture(now);
        if self.game.is_some() || self.doom.is_some() || now < self.led_due {
            return;
        }
        let fps = self.cfg.lights.fps.clamp(1, 30) as u64;
        self.led_due = now + Duration::from_millis(1000 / fps);
        // Mesures audio pour les LED « état » et « VU-mètre » (utilisées à l'image suivante).
        let measured = self.measure_targets();
        if !measured.is_empty() {
            self.audio.send(AudioCmd::Measure(measured));
        }
        self.expire_pickups(now);

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
                            self.mgr.send(s, Command::Output(protocol::led_brightness(0, 0, self.led_level())));
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

    /// Cibles à mesurer : LED « état » et « VU-mètre », faders en reprise douce.
    fn measure_targets(&self) -> Vec<String> {
        let mut v: Vec<String> = (0..FADER_COUNT)
            .map(|i| (i, self.fader_slot(i)))
            .filter(|(i, s)| s.led == "status" || s.led == "vu" || self.pickup[*i].is_some())
            .map(|(_, s)| s.target)
            .collect();
        v.sort();
        v.dedup();
        v
    }

    fn norm_target(t: &str) -> String {
        t.trim().to_lowercase()
    }

    /// Note la cible actuelle de chaque fader, sans reprise douce (démarrage).
    fn sync_fader_targets(&mut self) {
        for i in 0..FADER_COUNT {
            self.fader_targets[i] = Self::norm_target(&self.fader_slot(i).target);
        }
    }

    /// Après un changement de page ou de configuration : chaque fader qui
    /// contrôle maintenant autre chose attend d'être passé par le volume actuel.
    fn arm_pickups(&mut self) {
        let now = Instant::now();
        let mut armed = false;
        for i in 0..FADER_COUNT {
            let t = Self::norm_target(&self.fader_slot(i).target);
            if t == self.fader_targets[i] {
                continue;
            }
            self.fader_targets[i] = t.clone();
            let useful = self.cfg.general.fader_pickup && t != "none" && !t.is_empty() && !self.faders.is_empty();
            self.pickup[i] = if useful { Some(Pickup { side: None, since: now }) } else { None };
            armed |= useful;
        }
        if armed {
            self.audio.send(AudioCmd::Measure(self.measure_targets()));
            self.led_due = now;
        }
        if !self.cfg.general.fader_pickup {
            self.pickup = [None; FADER_COUNT];
        }
    }

    /// Une reprise douce dont la cible ne peut pas être mesurée est abandonnée.
    fn expire_pickups(&mut self, now: Instant) {
        for i in 0..FADER_COUNT {
            if let Some(p) = self.pickup[i] {
                let m = self.audio.meter_opt(&self.fader_slot(i).target);
                let gone = match m {
                    Some(m) => !m.present || m.level.is_none(),
                    None => now.saturating_duration_since(p.since) > Duration::from_secs(2),
                };
                if gone {
                    self.pickup[i] = None;
                }
            }
        }
    }

    /// Reprise douce : vrai si le fader `i` (à la position `v`) doit encore attendre.
    fn pickup_waits(&mut self, i: usize, v: u8) -> bool {
        let Some(p) = self.pickup[i] else { return false };
        let target = self.fader_slot(i).target;
        let Some(m) = self.audio.meter_opt(&target) else {
            // Pas encore mesuré : on attend un peu.
            return p.since.elapsed() < Duration::from_secs(2) || {
                self.pickup[i] = None;
                false
            };
        };
        let Some(level) = m.level.filter(|_| m.present) else {
            self.pickup[i] = None;
            return false;
        };
        let diff = v as i32 - (level * 100.0).round() as i32;
        let crossed = p.side.map(|s| diff == 0 || diff.signum() != s as i32).unwrap_or(false);
        if diff.abs() <= 3 || crossed {
            log::debug!("fader {} repris à {v} %", i + 1);
            self.pickup[i] = None;
            self.led_due = Instant::now();
            return false;
        }
        if p.side.is_none() {
            self.pickup[i] = Some(Pickup { side: Some(diff.signum() as i8), since: p.since });
        }
        true
    }

    fn faders_changed(&mut self, serial: String, values: [u8; FADER_COUNT]) {
        let previous = self.faders.insert(serial, values);
        if (0..FADER_COUNT).any(|i| self.fader_slot(i).led == "position") {
            self.led_due = self.led_due.min(Instant::now());
        }
        for (i, &v) in values.iter().enumerate() {
            if previous.map(|p| p[i] == v).unwrap_or(false) {
                continue;
            }
            if self.pickup_waits(i, v) {
                continue;
            }
            let slot = self.fader_slot(i);
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
        if let Some(d) = self.doom.as_mut() {
            // Molette gauche : tourner. Molette droite : avancer / reculer.
            let (code, ms) = match (wheel, step > 0) {
                (Wheel::Left, true) => (doom::key::RIGHT, 90),
                (Wheel::Left, false) => (doom::key::LEFT, 90),
                (Wheel::Right, true) => (doom::key::UP, 160),
                (Wheel::Right, false) => (doom::key::DOWN, 160),
            };
            let until = Instant::now() + Duration::from_millis(ms);
            if let Some(p) = d.pulses.iter_mut().find(|(_, k)| *k == code) {
                p.0 = until;
            } else {
                if !d.held.contains_key(&code) {
                    d.sess.key(true, code);
                }
                d.pulses.push((until, code));
            }
            return;
        }
        let r = &self.cfg.rollers;
        // Réglage propre à la page, sinon réglage commun.
        let page = self.cfg.pages.get(self.page);
        let (mode, invert, actions) = match wheel {
            Wheel::Right => (
                page.and_then(|p| p.roller_right.clone()).unwrap_or_else(|| r.right.clone()),
                r.invert_right,
                page.and_then(|p| p.roller_right_actions.clone()).unwrap_or_else(|| r.right_actions.clone()),
            ),
            Wheel::Left => (
                page.and_then(|p| p.roller_left.clone()).unwrap_or_else(|| r.left.clone()),
                r.invert_left,
                page.and_then(|p| p.roller_left_actions.clone()).unwrap_or_else(|| r.left_actions.clone()),
            ),
        };
        // Sens « 01 » = vers le bas (-1) : on le ramène à « vers le haut = + ».
        let mut dir = -(step as i32);
        if invert {
            dir = -dir;
        }
        let vstep = dir as f32 * r.volume_step as f32 / 100.0;
        let scroll = r.scroll_step;
        let mode = mode.trim().to_lowercase();
        match mode.as_str() {
            "volume" => {
                self.audio.send(AudioCmd::StepMaster(vstep));
                self.widgets_due = self.widgets_due.min(Instant::now() + Duration::from_millis(80));
            }
            "mic" => self.audio.send(AudioCmd::StepMic(vstep)),
            "scroll" => self.input.scroll(-dir * scroll, false),
            "hscroll" => self.input.scroll(dir * scroll, true),
            "zoom" => self.input.zoom(-dir),
            "actions" => self.wheel_action(&actions, dir > 0),
            m => {
                if let Some(apps) = m.strip_prefix("app:") {
                    let names: Vec<String> = apps.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                    if !names.is_empty() {
                        self.audio.send(AudioCmd::StepApps(names, vstep));
                    }
                }
            }
        }
    }

    /// Mode « actions » : l'action du cran vers le haut ou vers le bas.
    fn wheel_action(&mut self, w: &WheelActions, up: bool) {
        let a = if up { &w.up } else { &w.down };
        if *a != Action::None {
            self.run_extra_action(&a.clone());
        }
    }
}

/// La touche affiche-t-elle une variable dans son texte ?
fn key_mentions_vars(def: &KeyDef) -> bool {
    vars::mentions(&def.label) || def.toggle.as_ref().map(|t| vars::mentions(&t.label)).unwrap_or(false)
}

/// Visuels d'une touche, avec les {variables} de son texte remplacées.
fn render_key_visuals(def: &KeyDef, vars: &BTreeMap<String, String>, dir: &std::path::Path, rot: u16) -> KeyVisuals {
    let def = if key_mentions_vars(def) { vars::key_with_values(def, vars) } else { def.clone() };
    let mut states = vec![render::render_key_rot(&def.state(0), dir, rot)];
    if def.toggle.is_some() {
        states.push(render::render_key_rot(&def.state(1), dir, rot));
    }
    KeyVisuals { states }
}

/// Actions qu'on peut répéter tant que la touche est enfoncée.
fn repeatable(a: &Action) -> bool {
    matches!(a, Action::Hotkey { .. } | Action::Text { .. } | Action::Media { .. } | Action::Var { .. })
}

/// Valeur « vraie » d'une variable pour une touche à 2 états.
fn truthy(v: &str) -> bool {
    let v = v.trim().to_lowercase();
    !matches!(v.as_str(), "" | "0" | "false" | "off" | "non" | "no" | "faux")
}
