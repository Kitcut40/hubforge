//! Journal : affiché dans la console et écrit dans `hubforge.log`.

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
#[cfg(not(windows))]
use std::time::{SystemTime, UNIX_EPOCH};

struct Logger {
    file: Mutex<Option<File>>,
}

impl Log for Logger {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= Level::Info || m.target().starts_with("hubforge")
    }

    fn log(&self, r: &Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let (h, m, s) = local_time();
        let tag = match r.level() {
            Level::Error => "ERREUR",
            Level::Warn => "ATTENTION",
            Level::Info => "",
            _ => "debug",
        };
        let line = if tag.is_empty() {
            format!("[{h:02}:{m:02}:{s:02}] {}", r.args())
        } else {
            format!("[{h:02}:{m:02}:{s:02}] {tag} : {}", r.args())
        };
        if r.level() <= Level::Info {
            println!("{line}");
        }
        if let Some(f) = self.file.lock().unwrap().as_mut() {
            let _ = writeln!(f, "{line}");
        }
    }

    fn flush(&self) {
        if let Some(f) = self.file.lock().unwrap().as_mut() {
            let _ = f.flush();
        }
    }
}

/// Heure locale (heures, minutes, secondes).
fn local_time() -> (u16, u16, u16) {
    #[cfg(windows)]
    {
        let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        (t.wHour, t.wMinute, t.wSecond)
    }
    #[cfg(not(windows))]
    {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        (((secs / 3600) % 24) as u16, ((secs / 60) % 60) as u16, (secs % 60) as u16)
    }
}

pub fn init(dir: &Path) {
    let path = dir.join("hubforge.log");
    // On repart d'un journal neuf s'il devient trop gros.
    if std::fs::metadata(&path).map(|m| m.len() > 2_000_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    let file = OpenOptions::new().create(true).append(true).open(&path).ok();
    let logger = Box::leak(Box::new(Logger { file: Mutex::new(file) }));
    let _ = log::set_logger(logger);
    log::set_max_level(LevelFilter::Debug);
}
