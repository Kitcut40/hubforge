//! Moteur HubForge : relie les modules, la configuration et les actions.
//! Il tourne dans son propre fil ; l'interface lui parle par messages
//! ([`EngineMsg`]) et reçoit son état ([`Status`]) à chaque changement.

use crate::actions::{self, Input};
use crate::audio::{Audio, AudioCmd};
use crate::config::{self, parse_color, Action, Config};
use crate::render::{self, KeyVisual};
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
}

/// État du moteur, envoyé à l'interface.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Status {
    pub modules: Vec<ModuleStatus>,
    pub faders: Option<[u8; FADER_COUNT]>,
    pub page: usize,
    pub pressed: Vec<bool>,
    pub config_error: Option<String>,
}

pub type Notify = Box<dyn Fn(&Status) + Send>;

struct Anim {
    frame: usize,
    due: Instant,
}

pub struct Engine {
    mgr: DeviceManager,
    events: Receiver<Event>,
    cfg: Config,
    cfg_path: PathBuf,
    cfg_dir: PathBuf,
    cfg_mtime: Option<SystemTime>,
    /// visuels[page][touche]
    visuals: Vec<Vec<KeyVisual>>,
    page: usize,
    anims: HashMap<u8, Anim>,
    pressed: [bool; KEY_COUNT],
    modules: HashMap<String, Kind>,
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
            modules: HashMap::new(),
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
            .map(|(s, k)| ModuleStatus { kind: kind_id(*k).into(), name: k.name().into(), serial: s.clone() })
            .collect();
        modules.sort_by(|a, b| a.kind.cmp(&b.kind));
        Status {
            modules,
            faders: self.faders.values().next().copied(),
            page: self.page,
            pressed: self.pressed.to_vec(),
            config_error: self.config_error.clone(),
        }
    }

    fn notify(&self) {
        (self.notify)(&self.status());
    }

    pub fn run(&mut self, stop: Arc<AtomicBool>, msgs: Receiver<EngineMsg>) {
        let mut last_scan = Instant::now() - Duration::from_secs(10);
        let mut last_cfg_check = Instant::now();
        while !stop.load(Ordering::Relaxed) {
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
        self.cfg = cfg;
        self.config_error = None;
        self.render_all();
        self.page = self.page.min(self.visuals.len().saturating_sub(1));
        let serials: Vec<(String, Kind)> = self.modules.iter().map(|(s, k)| (s.clone(), *k)).collect();
        for (s, k) in serials {
            self.setup_module(&s, k);
        }
        // Applique tout de suite les volumes selon la position actuelle des faders.
        let current: Vec<(String, [u8; FADER_COUNT])> = self.faders.drain().collect();
        for (s, v) in current {
            self.faders_changed(s, v);
        }
        self.notify();
    }

    fn render_all(&mut self) {
        let t = Instant::now();
        self.visuals = self
            .cfg
            .pages
            .iter()
            .map(|page| {
                (1..=KEY_COUNT as u8)
                    .map(|k| match page.keys.iter().find(|d| d.key == k) {
                        Some(def) => render::render_key(def, &self.cfg_dir),
                        None => render::blank(),
                    })
                    .collect()
            })
            .collect();
        if self.visuals.is_empty() {
            self.visuals.push(vec![render::blank(); KEY_COUNT]);
        }
        log::debug!("rendu des touches en {} ms", t.elapsed().as_millis());
    }

    // ------------------------------------------------------------------
    // Modules
    // ------------------------------------------------------------------

    fn setup_module(&mut self, serial: &str, kind: Kind) {
        let g = self.cfg.general.clone();
        match kind {
            Kind::Keys15 => {
                self.mgr.send(serial, Command::Feature(protocol::keys15_sw_mode(true)));
                self.mgr.send(serial, Command::Feature(protocol::keys15_brightness(g.brightness)));
                self.show_page();
            }
            Kind::Faders5 => {
                for (i, slot) in self.cfg.faders.iter().take(FADER_COUNT).enumerate() {
                    let (r, gg, b) = parse_color(&slot.color).unwrap_or((255, 255, 255));
                    self.mgr.send(serial, Command::Output(protocol::led_color(1, i as u8, r, gg, b, false)));
                }
                self.mgr.send(serial, Command::Output(protocol::led_brightness(1, 0xFF, g.led_brightness)));
                self.mgr.send(serial, Command::QueryFaders);
            }
            Kind::BaseLed => {
                let (r, gg, b) = parse_color(&g.base_color).unwrap_or((0, 200, 255));
                self.mgr.send(serial, Command::Output(protocol::led_color(0, 0, r, gg, b, false)));
                self.mgr.send(serial, Command::Output(protocol::led_brightness(0, 0, g.led_brightness)));
            }
            _ => {}
        }
    }

    fn handle(&mut self, ev: Event) {
        match ev {
            Event::Connected(info) => {
                log::info!("✓ {} connecté", info.kind.name());
                self.modules.insert(info.serial.clone(), info.kind);
                self.setup_module(&info.serial, info.kind);
            }
            Event::Disconnected { serial } => {
                if let Some(k) = self.modules.remove(&serial) {
                    log::info!("✗ {} débranché", k.name());
                }
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

    fn send_image(&self, key: u8, jpeg: &Arc<Vec<u8>>) {
        let packets = Arc::new(protocol::keys15_image_packets(key, jpeg));
        self.mgr.send_kind(Kind::Keys15, Command::Packets(packets));
    }

    fn show_page(&mut self) {
        self.anims.clear();
        let now = Instant::now();
        let page = &self.visuals[self.page];
        for (k, v) in page.iter().enumerate() {
            self.send_image(k as u8, &v.frames[0].jpeg);
            if v.is_animated() {
                self.anims.insert(k as u8, Anim { frame: 0, due: now + v.frames[0].delay });
            }
        }
        let name = self.cfg.pages.get(self.page).map(|p| p.name.as_str()).unwrap_or("");
        log::info!("page {} « {name} »", self.page + 1);
    }

    fn current_frame(&self, key: u8) -> &render::Frame {
        let v = &self.visuals[self.page][key as usize];
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
        let action = self
            .cfg
            .pages
            .get(self.page)
            .and_then(|p| p.keys.iter().find(|d| d.key == key + 1))
            .map(|d| d.action.clone())
            .unwrap_or_default();
        self.execute(&action);
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
                self.page = target;
                self.show_page();
            }
        }
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
            let v = &self.visuals[self.page][key as usize];
            let anim = self.anims.get_mut(&key).unwrap();
            anim.frame = (anim.frame + 1) % v.frames.len();
            let frame = v.frames[anim.frame].clone();
            anim.due = now + frame.delay;
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
