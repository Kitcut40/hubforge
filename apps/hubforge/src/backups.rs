//! Sauvegardes automatiques des profils.
//!
//! Avant chaque enregistrement d'un profil, l'ancienne version est copiée
//! dans `backups/<profil>/<horodatage>.toml` (au plus une copie toutes les
//! 2 minutes, pour qu'une série de petites retouches ne remplisse pas tout).
//! Les 30 dernières copies sont gardées. Une copie peut être restaurée depuis
//! l'interface (la version actuelle est elle-même sauvegardée avant).

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Nombre de copies gardées par profil.
pub const KEEP: usize = 30;
/// Écart minimal entre deux copies automatiques (secondes).
const MIN_GAP_SECS: u64 = 120;

#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    /// Nom du fichier (sert d'identifiant).
    pub file: String,
    /// Date de la copie (secondes depuis 1970).
    pub time: u64,
    pub size: u64,
    /// Nombre de pages et de touches dans cette version (pour s'y retrouver).
    pub pages: usize,
    pub keys: usize,
}

pub fn dir(cfg_dir: &Path, profile: &str) -> PathBuf {
    cfg_dir.join("backups").join(profile)
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Horodatage contenu dans le nom « 1731000000.toml » (ou « 1731000000-2.toml »).
fn time_of(name: &str) -> Option<u64> {
    name.strip_suffix(".toml")?.split('-').next()?.parse().ok()
}

/// Ordre dans la même seconde : « t.toml » = 1, « t-2.toml » = 2…
fn seq_of(name: &str) -> u32 {
    name.strip_suffix(".toml").and_then(|n| n.split('-').nth(1)).and_then(|n| n.parse().ok()).unwrap_or(1)
}

/// Copies du dossier, la plus récente en premier.
fn files(d: &Path) -> Vec<(u64, PathBuf)> {
    let mut v: Vec<(u64, u32, PathBuf)> = std::fs::read_dir(d)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    time_of(&name).map(|t| (t, seq_of(&name), e.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    v.into_iter().map(|(t, _, p)| (t, p)).collect()
}

/// Copie la version actuelle du profil avant qu'elle soit remplacée.
/// `force` : ignore l'écart minimal (restauration, import…).
pub fn snapshot(cfg_dir: &Path, profile: &str, current: &Path, force: bool) -> Option<PathBuf> {
    let content = std::fs::read(current).ok()?;
    let d = dir(cfg_dir, profile);
    std::fs::create_dir_all(&d).ok()?;
    let existing = files(&d);
    if let Some((t, last)) = existing.first() {
        // Rien de neuf depuis la dernière copie.
        if std::fs::read(last).map(|c| c == content).unwrap_or(false) {
            return None;
        }
        if !force && now().saturating_sub(*t) < MIN_GAP_SECS {
            return None;
        }
    }
    let t = now();
    let mut path = d.join(format!("{t}.toml"));
    let mut n = 2;
    while path.exists() {
        path = d.join(format!("{t}-{n}.toml"));
        n += 1;
    }
    std::fs::write(&path, &content).ok()?;
    // On ne garde que les plus récentes.
    for (_, old) in files(&d).into_iter().skip(KEEP) {
        let _ = std::fs::remove_file(old);
    }
    Some(path)
}

pub fn list(cfg_dir: &Path, profile: &str) -> Vec<BackupInfo> {
    files(&dir(cfg_dir, profile))
        .into_iter()
        .map(|(time, p)| {
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            let (pages, keys) = crate::config::load(&p)
                .map(|c| (c.pages.len(), c.pages.iter().map(|pg| pg.keys.len()).sum()))
                .unwrap_or((0, 0));
            BackupInfo { file: p.file_name().unwrap_or_default().to_string_lossy().to_string(), time, size, pages, keys }
        })
        .collect()
}

/// Remet une copie à la place du profil (la version actuelle est d'abord
/// sauvegardée, pour pouvoir revenir en arrière).
pub fn restore(cfg_dir: &Path, profile: &str, file: &str, target: &Path) -> Result<(), String> {
    if file.contains(['/', '\\']) || time_of(file).is_none() {
        return Err("sauvegarde inconnue".into());
    }
    let src = dir(cfg_dir, profile).join(file);
    crate::config::load(&src).map_err(|e| format!("cette sauvegarde est illisible : {e}"))?;
    snapshot(cfg_dir, profile, target, true);
    std::fs::copy(&src, target).map_err(|e| format!("restauration impossible : {e}"))?;
    Ok(())
}

/// Suit le renommage d'un profil.
pub fn rename(cfg_dir: &Path, old: &str, new: &str) {
    let (a, b) = (dir(cfg_dir, old), dir(cfg_dir, new));
    if a.is_dir() && !b.exists() {
        let _ = std::fs::rename(a, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_and_restores() {
        let d = std::env::temp_dir().join(format!("hubforge-backups-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let profile = d.join("P.toml");
        std::fs::write(&profile, crate::config::DEFAULT_CONFIG).unwrap();

        // Première copie : toujours faite.
        assert!(snapshot(&d, "P", &profile, false).is_some());
        // Contenu identique : pas de nouvelle copie.
        assert!(snapshot(&d, "P", &profile, true).is_none());
        // Contenu différent mais trop tôt : pas de copie (sauf forcée).
        let blank = toml::to_string(&crate::profiles::blank_config()).unwrap();
        std::fs::write(&profile, &blank).unwrap();
        assert!(snapshot(&d, "P", &profile, false).is_none());
        assert!(snapshot(&d, "P", &profile, true).is_some());

        let l = list(&d, "P");
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].pages, 1); // la plus récente (vierge) en premier
        assert!(l[1].keys > 10);

        // Restaurer la plus ancienne (la config d'exemple).
        restore(&d, "P", &l[1].file, &profile).unwrap();
        assert_eq!(crate::config::load(&profile).unwrap().pages.len(), l[1].pages);
        assert!(restore(&d, "P", "../x.toml", &profile).is_err());

        // Pas plus de KEEP copies.
        for i in 0..(KEEP + 5) {
            std::fs::write(&profile, format!("{blank}\n# {i}\n")).unwrap();
            snapshot(&d, "P", &profile, true);
        }
        assert_eq!(list(&d, "P").len(), KEEP);
        let _ = std::fs::remove_dir_all(&d);
    }
}
