//! Profils : plusieurs configurations complètes du MasterHUB (touches,
//! faders, molettes, LED), une par fichier dans `profiles/`.
//! Ex. « Benjamin », « Laura », « Création », « Streaming ».
//!
//! Les réglages communs à tous les profils (profil actif, record du
//! mini-jeu) sont dans `settings.toml`.

use crate::config::{self, Config, FaderSlot, General, Page, Rollers};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const DEFAULT_NAME: &str = "Principal";

/// Réglages communs à tous les profils.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Profil actif.
    pub profile: String,
    /// Meilleur score au mini-jeu caché.
    pub game_best: u32,
    /// Addons désactivés (leurs actions n'apparaissent plus).
    pub disabled_addons: Vec<String>,
}

/// Profil actif, partagé entre l'interface et le moteur.
#[derive(Debug, Clone)]
pub struct Active {
    pub name: String,
    pub path: PathBuf,
}

pub type SharedActive = Arc<Mutex<Active>>;

pub fn dir(cfg_dir: &Path) -> PathBuf {
    cfg_dir.join("profiles")
}

pub fn path(cfg_dir: &Path, name: &str) -> PathBuf {
    dir(cfg_dir).join(format!("{name}.toml"))
}

fn settings_path(cfg_dir: &Path) -> PathBuf {
    cfg_dir.join("settings.toml")
}

pub fn load_settings(cfg_dir: &Path) -> Settings {
    std::fs::read_to_string(settings_path(cfg_dir)).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_settings(cfg_dir: &Path, s: &Settings) {
    let text = format!(
        "# HubForge — réglages communs à tous les profils (géré par HubForge)\n\n{}",
        toml::to_string_pretty(s).unwrap_or_default()
    );
    let tmp = settings_path(cfg_dir).with_extension("toml.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, settings_path(cfg_dir));
    }
}

/// Noms des profils, triés.
pub fn list(cfg_dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir(cfg_dir))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    (p.extension().map(|x| x == "toml").unwrap_or(false))
                        .then(|| p.file_stem().map(|s| s.to_string_lossy().to_string()))
                        .flatten()
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|s| s.to_lowercase());
    v
}

/// Vérifie un nom de profil (il sert de nom de fichier).
pub fn check_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("le nom du profil est vide".into());
    }
    if n.chars().count() > 40 {
        return Err("nom trop long (40 caractères maximum)".into());
    }
    if n.chars().any(|c| "\\/:*?\"<>|".contains(c) || c.is_control()) || n.starts_with('.') {
        return Err("le nom ne peut pas contenir \\ / : * ? \" < > |".into());
    }
    Ok(n.to_string())
}

/// Prépare le dossier des profils au démarrage et retourne le profil actif.
/// Première fois : l'ancien `config.toml` devient le profil « Principal ».
pub fn init(cfg_dir: &Path) -> Active {
    let d = dir(cfg_dir);
    let _ = std::fs::create_dir_all(&d);
    if list(cfg_dir).is_empty() {
        let old = cfg_dir.join("config.toml");
        let target = path(cfg_dir, DEFAULT_NAME);
        if old.exists() {
            match std::fs::rename(&old, &target) {
                Ok(()) => log::info!("config.toml déplacé dans profiles/{DEFAULT_NAME}.toml"),
                Err(e) => {
                    log::warn!("déplacement de config.toml impossible ({e}) : copie");
                    let _ = std::fs::copy(&old, &target);
                }
            }
        } else {
            match std::fs::write(&target, config::DEFAULT_CONFIG) {
                Ok(()) => log::info!("profil d'exemple « {DEFAULT_NAME} » créé"),
                Err(e) => log::error!("impossible de créer le profil d'exemple : {e}"),
            }
        }
    }
    let names = list(cfg_dir);
    let mut settings = load_settings(cfg_dir);
    let name = if names.contains(&settings.profile) {
        settings.profile.clone()
    } else {
        names.first().cloned().unwrap_or_else(|| DEFAULT_NAME.into())
    };
    if settings.profile != name {
        settings.profile = name.clone();
        save_settings(cfg_dir, &settings);
    }
    Active { path: path(cfg_dir, &name), name }
}

/// Configuration vierge : une page vide, faders et molettes par défaut.
pub fn blank_config() -> Config {
    let faders = ["master", "none", "none", "none", "none"]
        .iter()
        .map(|t| FaderSlot { target: t.to_string(), color: "#FFFFFF".into() })
        .collect();
    Config {
        general: General::default(),
        faders,
        rollers: Rollers::default(),
        pages: vec![Page {
            name: "Accueil".into(),
            apps: Vec::new(),
            folder: false,
            close_after: false,
            keys: Vec::new(),
        }],
    }
}

/// Crée un profil : vierge, ou copie d'un autre.
pub fn create(cfg_dir: &Path, name: &str, copy_from: Option<&str>) -> Result<String, String> {
    let name = check_name(name)?;
    let target = path(cfg_dir, &name);
    if target.exists() || list(cfg_dir).iter().any(|n| n.eq_ignore_ascii_case(&name)) {
        return Err(format!("le profil « {name} » existe déjà"));
    }
    match copy_from {
        Some(src) => {
            std::fs::copy(path(cfg_dir, src), &target).map_err(|e| format!("copie impossible : {e}"))?;
        }
        None => config::save(&target, &blank_config())?,
    }
    Ok(name)
}

pub fn rename(cfg_dir: &Path, old: &str, new: &str) -> Result<String, String> {
    let new = check_name(new)?;
    if new == old {
        return Ok(new);
    }
    if list(cfg_dir).iter().any(|n| n != old && n.eq_ignore_ascii_case(&new)) {
        return Err(format!("le profil « {new} » existe déjà"));
    }
    std::fs::rename(path(cfg_dir, old), path(cfg_dir, &new)).map_err(|e| format!("renommage impossible : {e}"))?;
    Ok(new)
}

pub fn delete(cfg_dir: &Path, name: &str) -> Result<(), String> {
    if list(cfg_dir).len() <= 1 {
        return Err("impossible de supprimer le dernier profil".into());
    }
    // Mise à la corbeille maison : on garde une copie au cas où.
    let trash = cfg_dir.join("profiles-supprimes");
    let _ = std::fs::create_dir_all(&trash);
    let _ = std::fs::copy(path(cfg_dir, name), trash.join(format!("{name}.toml")));
    std::fs::remove_file(path(cfg_dir, name)).map_err(|e| format!("suppression impossible : {e}"))
}

/// Profil suivant / précédent / par nom (insensible à la casse).
pub fn resolve(cfg_dir: &Path, current: &str, to: &str) -> Option<String> {
    let names = list(cfg_dir);
    if names.is_empty() {
        return None;
    }
    let i = names.iter().position(|n| n == current).unwrap_or(0);
    match to.trim().to_lowercase().as_str() {
        "next" | "suivant" => Some(names[(i + 1) % names.len()].clone()),
        "previous" | "prev" | "precedent" | "précédent" => Some(names[(i + names.len() - 1) % names.len()].clone()),
        other => names.iter().find(|n| n.to_lowercase() == other).cloned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("hubforge-test-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn migrates_and_manages_profiles() {
        let d = tmp();
        std::fs::write(d.join("config.toml"), config::DEFAULT_CONFIG).unwrap();
        let a = init(&d);
        assert_eq!(a.name, DEFAULT_NAME);
        assert!(!d.join("config.toml").exists());
        assert!(config::load(&a.path).is_ok());

        create(&d, "Laura", None).unwrap();
        create(&d, "Streaming", Some(DEFAULT_NAME)).unwrap();
        assert!(create(&d, "laura", None).is_err());
        assert!(create(&d, "a/b", None).is_err());
        assert_eq!(list(&d), vec!["Laura", "Principal", "Streaming"]);
        assert!(config::load(&path(&d, "Laura")).is_ok());

        assert_eq!(resolve(&d, "Principal", "next").as_deref(), Some("Streaming"));
        assert_eq!(resolve(&d, "Laura", "previous").as_deref(), Some("Streaming"));
        assert_eq!(resolve(&d, "Laura", "streaming").as_deref(), Some("Streaming"));

        rename(&d, "Streaming", "Live").unwrap();
        delete(&d, "Live").unwrap();
        assert_eq!(list(&d), vec!["Laura", "Principal"]);

        // Le profil actif est retenu.
        save_settings(&d, &Settings { profile: "Laura".into(), game_best: 12, ..Default::default() });
        assert_eq!(init(&d).name, "Laura");
        assert_eq!(load_settings(&d).game_best, 12);
        let _ = std::fs::remove_dir_all(&d);
    }
}
