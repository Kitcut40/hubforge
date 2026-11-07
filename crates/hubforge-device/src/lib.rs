//! hubforge-device — communication avec les modules du Cooler Master MasterHUB.
//!
//! Chaque module est un périphérique HID distinct. Le [`DeviceManager`]
//! détecte les modules et lance, pour chacun, un fil d'exécution qui :
//! - lit en continu les rapports d'entrée et les transforme en [`Event`] ;
//! - exécute les [`Command`] envoyées (images, luminosité, LED…).

pub mod models;
pub mod protocol;

pub use models::Kind;
pub use protocol::{FADER_COUNT, KEY_COUNT, KEY_IMAGE_SIZE};

use hidapi::{HidApi, HidDevice};
use protocol::Input;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInfo {
    pub serial: String,
    pub kind: Kind,
    pub pid: u16,
    pub product: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wheel {
    Right,
    Left,
}

/// Événement remonté par un module.
#[derive(Debug, Clone)]
pub enum Event {
    Connected(ModuleInfo),
    Disconnected { serial: String },
    KeyDown { serial: String, key: u8 },
    KeyUp { serial: String, key: u8 },
    /// Position de tous les faders (envoyée à chaque changement).
    Faders { serial: String, values: [u8; FADER_COUNT] },
    /// Un cran de molette : +1 ou -1.
    Roller { serial: String, wheel: Wheel, step: i8 },
}

/// Commande bas niveau envoyée à un module.
#[derive(Debug, Clone)]
pub enum Command {
    /// Feature report (32 octets).
    Feature(Vec<u8>),
    /// Rapport de sortie.
    Output(Vec<u8>),
    /// Suite de rapports de sortie (image découpée en paquets).
    Packets(Arc<Vec<Vec<u8>>>),
    /// Demande la position actuelle des faders (répond par Event::Faders).
    QueryFaders,
    /// Pause (millisecondes) entre deux commandes, pour laisser le module les traiter.
    Sleep(u64),
    /// Arrête le fil du module.
    Stop,
}

struct Handle {
    info: ModuleInfo,
    tx: Sender<Command>,
    alive: Arc<AtomicBool>,
}

pub struct DeviceManager {
    api: HidApi,
    modules: HashMap<String, Handle>,
    events: Sender<Event>,
}

impl DeviceManager {
    /// Crée le gestionnaire. Les événements arrivent sur le `Receiver` retourné.
    pub fn new() -> Result<(DeviceManager, Receiver<Event>), String> {
        let api = HidApi::new().map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel();
        Ok((DeviceManager { api, modules: HashMap::new(), events: tx }, rx))
    }

    /// Détecte les modules branchés et démarre ceux qui sont nouveaux.
    pub fn scan(&mut self) {
        self.modules.retain(|_, h| h.alive.load(Ordering::Relaxed));
        if let Err(e) = self.api.refresh_devices() {
            log::warn!("énumération USB : {e}");
        }
        let mut found = Vec::new();
        for d in self.api.device_list() {
            if d.vendor_id() != models::VID_COOLER_MASTER {
                continue;
            }
            let Some(kind) = Kind::from_pid(d.product_id()) else { continue };
            let serial = d.serial_number().unwrap_or("").to_string();
            if serial.is_empty() || self.modules.contains_key(&serial) {
                continue;
            }
            found.push((
                ModuleInfo {
                    serial,
                    kind,
                    pid: d.product_id(),
                    product: d.product_string().unwrap_or("").to_string(),
                },
                d.path().to_owned(),
            ));
        }
        for (info, path) in found {
            match self.api.open_path(&path) {
                Ok(dev) => self.start(info, dev),
                Err(e) => log::warn!("ouverture de {} impossible : {e}", info.serial),
            }
        }
    }

    fn start(&mut self, info: ModuleInfo, dev: HidDevice) {
        let (tx, rx) = mpsc::channel();
        let alive = Arc::new(AtomicBool::new(true));
        let events = self.events.clone();
        let worker_info = info.clone();
        let worker_alive = alive.clone();
        log::info!("module connecté : {} ({})", info.kind.name(), info.serial);
        let _ = self.events.send(Event::Connected(info.clone()));
        thread::Builder::new()
            .name(format!("module-{}", info.serial))
            .spawn(move || {
                worker(worker_info, dev, rx, events);
                worker_alive.store(false, Ordering::Relaxed);
            })
            .ok();
        self.modules.insert(info.serial.clone(), Handle { info, tx, alive });
    }

    pub fn modules(&self) -> impl Iterator<Item = &ModuleInfo> {
        self.modules.values().filter(|h| h.alive.load(Ordering::Relaxed)).map(|h| &h.info)
    }

    pub fn send(&self, serial: &str, cmd: Command) -> bool {
        self.modules.get(serial).map(|h| h.tx.send(cmd).is_ok()).unwrap_or(false)
    }

    /// Envoie une commande à tous les modules d'un type.
    pub fn send_kind(&self, kind: Kind, cmd: Command) {
        for h in self.modules.values().filter(|h| h.info.kind == kind) {
            let _ = h.tx.send(cmd.clone());
        }
    }

    /// Arrête proprement tous les modules (attend la fin des envois).
    pub fn shutdown(&mut self) {
        for h in self.modules.values() {
            let _ = h.tx.send(Command::Stop);
        }
        for _ in 0..100 {
            if self.modules.values().all(|h| !h.alive.load(Ordering::Relaxed)) {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

fn worker(info: ModuleInfo, dev: HidDevice, rx: Receiver<Command>, events: Sender<Event>) {
    let serial = info.serial.clone();
    let mut buf = [0u8; 128];
    let mut keys = [false; KEY_COUNT];
    let mut faders: Option<[u8; FADER_COUNT]> = None;

    loop {
        // 1) Commandes en attente
        loop {
            match rx.try_recv() {
                Ok(Command::Stop) => return,
                Ok(cmd) => {
                    if let Err(e) = execute(&dev, &cmd, &serial, &events) {
                        log::warn!("{serial} : commande échouée : {e}");
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        // 2) Lecture (attente courte pour rester réactif aux commandes)
        match dev.read_timeout(&mut buf, 8) {
            Ok(0) => {}
            Ok(n) => match protocol::parse_input(&buf[..n]) {
                Input::Keys(state) => {
                    for (i, (&now, was)) in state.iter().zip(keys.iter_mut()).enumerate() {
                        if now != *was {
                            *was = now;
                            let key = i as u8;
                            let s = serial.clone();
                            let _ = events.send(if now {
                                Event::KeyDown { serial: s, key }
                            } else {
                                Event::KeyUp { serial: s, key }
                            });
                        }
                    }
                }
                Input::Faders(values) => {
                    if faders != Some(values) {
                        faders = Some(values);
                        let _ = events.send(Event::Faders { serial: serial.clone(), values });
                    }
                }
                Input::Rollers { right, left } => {
                    for (wheel, dir) in [(Wheel::Right, right), (Wheel::Left, left)] {
                        if let Some(d) = dir {
                            let step = if d { 1 } else { -1 };
                            let _ = events.send(Event::Roller { serial: serial.clone(), wheel, step });
                        }
                    }
                }
                Input::Heartbeat | Input::Unknown => {}
            },
            Err(e) => {
                log::info!("module déconnecté : {} ({e})", serial);
                let _ = events.send(Event::Disconnected { serial });
                return;
            }
        }
    }
}

fn execute(dev: &HidDevice, cmd: &Command, serial: &str, events: &Sender<Event>) -> Result<(), String> {
    match cmd {
        Command::Feature(d) => dev.send_feature_report(d).map_err(|e| e.to_string()),
        Command::Output(d) => dev.write(d).map(|_| ()).map_err(|e| e.to_string()),
        Command::Packets(list) => {
            for p in list.iter() {
                dev.write(p).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        Command::QueryFaders => {
            let mut b = protocol::faders_query();
            let n = dev.get_feature_report(&mut b).map_err(|e| e.to_string())?;
            if let Some(values) = protocol::parse_faders_query(&b[..n]) {
                let _ = events.send(Event::Faders { serial: serial.to_string(), values });
            }
            Ok(())
        }
        Command::Sleep(ms) => {
            thread::sleep(std::time::Duration::from_millis(*ms));
            Ok(())
        }
        Command::Stop => Ok(()),
    }
}
