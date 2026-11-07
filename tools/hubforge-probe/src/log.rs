//! Journal : tout ce qui s'affiche est aussi écrit dans un fichier texte
//! placé à côté de l'exécutable, pour pouvoir l'envoyer facilement.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static LOG: Mutex<Option<Logger>> = Mutex::new(None);

struct Logger {
    file: Option<File>,
    path: PathBuf,
    start: Instant,
}

/// Initialise le journal. Retourne le chemin du fichier créé.
pub fn init() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let path = dir.join(format!("hubforge-probe-log-{stamp}.txt"));
    let file = File::create(&path).ok();
    *LOG.lock().unwrap() = Some(Logger {
        file,
        path: path.clone(),
        start: Instant::now(),
    });
    path
}

pub fn path() -> Option<PathBuf> {
    LOG.lock().unwrap().as_ref().map(|l| l.path.clone())
}

/// Écrit une ligne à l'écran et dans le fichier.
pub fn line(msg: &str) {
    println!("{msg}");
    file_only(msg);
}

/// Écrit une ligne uniquement dans le fichier (détails techniques).
pub fn file_only(msg: &str) {
    if let Some(l) = LOG.lock().unwrap().as_mut() {
        let t = l.start.elapsed().as_secs_f64();
        if let Some(f) = l.file.as_mut() {
            let _ = writeln!(f, "[{t:9.3}] {msg}");
            let _ = f.flush();
        }
    }
}

/// Formate des octets en hexadécimal lisible.
pub fn hex(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Hexadécimal en coupant les zéros de fin (plus lisible à l'écran).
pub fn hex_trim(data: &[u8]) -> String {
    let end = data.iter().rposition(|&b| b != 0).map(|i| i + 1).unwrap_or(1);
    let mut s = hex(&data[..end.max(1)]);
    if end < data.len() {
        s.push_str(&format!(" … ({} octets au total)", data.len()));
    }
    s
}

#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => { $crate::log::line(&format!($($arg)*)) };
}

#[macro_export]
macro_rules! note {
    ($($arg:tt)*) => { $crate::log::file_only(&format!($($arg)*)) };
}
