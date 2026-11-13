//! Partage de profils : un profil complet dans un seul fichier `.hfprofile`.
//!
//! C'est une archive ZIP qui contient :
//! - `profile.toml` : la configuration (touches, pages, faders, molettes, LED) ;
//! - `icons/…` : les images et GIF utilisés par les touches ;
//! - `manifest.json` : nom du profil, version de HubForge, addons utilisés.
//!
//! À l'import, les images sont rangées dans `icons/import-<profil>/` et les
//! addons manquants sont signalés.

use crate::config::{self, Config, KeyDef};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub name: String,
    pub hubforge: String,
    #[serde(default)]
    pub addons: Vec<String>,
}

/// Résultat d'un import.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ImportReport {
    /// Nom du profil créé.
    pub name: String,
    /// Addons utilisés par le profil mais absents de ce PC.
    pub missing_addons: Vec<String>,
    pub images: usize,
    /// Sons copiés.
    #[serde(default)]
    pub sounds: usize,
    /// Plugins ou actions sans équivalent (import Stream Deck).
    #[serde(default)]
    pub unsupported: Vec<String>,
    /// « profile » ou « icons » (pack d'icônes).
    pub kind: String,
}

fn key_icons(cfg: &mut Config) -> Vec<&mut Option<String>> {
    let mut v = Vec::new();
    for p in cfg.pages.iter_mut() {
        for k in p.keys.iter_mut() {
            let KeyDef { icon, toggle, .. } = k;
            v.push(icon);
            if let Some(t) = toggle {
                v.push(&mut t.icon);
            }
        }
    }
    v
}

/// Fichiers son des touches et des molettes (soundboard), y compris dans les macros, les conditions, le
/// second état, l'appui long et le double appui.
fn key_sounds(cfg: &mut Config) -> Vec<&mut String> {
    fn collect<'a>(a: &'a mut crate::config::Action, v: &mut Vec<&'a mut String>) {
        match a {
            crate::config::Action::Sound { file, .. } => v.push(file),
            other => {
                for s in other.children_mut() {
                    collect(s, v);
                }
            }
        }
    }
    let mut v = Vec::new();
    let r = &mut cfg.rollers;
    for w in [&mut r.left_actions, &mut r.right_actions] {
        collect(&mut w.up, &mut v);
        collect(&mut w.down, &mut v);
    }
    for p in cfg.pages.iter_mut() {
        for w in [p.roller_left_actions.as_mut(), p.roller_right_actions.as_mut()].into_iter().flatten() {
            collect(&mut w.up, &mut v);
            collect(&mut w.down, &mut v);
        }
        for k in p.keys.iter_mut() {
            let KeyDef { action, toggle, long, double, .. } = k;
            collect(action, &mut v);
            if let Some(t) = toggle {
                collect(&mut t.action, &mut v);
            }
            if let Some(a) = long {
                collect(a, &mut v);
            }
            if let Some(a) = double {
                collect(a, &mut v);
            }
        }
    }
    v
}

/// Addons utilisés (icônes « addon:<id>/… »).
fn used_addons(cfg: &Config) -> Vec<String> {
    let mut set = BTreeSet::new();
    for p in &cfg.pages {
        for k in &p.keys {
            for icon in [k.icon.as_deref(), k.toggle.as_ref().and_then(|t| t.icon.as_deref())].into_iter().flatten() {
                if let Some(rest) = icon.strip_prefix("addon:") {
                    if let Some(id) = rest.split('/').next() {
                        set.insert(id.to_string());
                    }
                }
            }
        }
    }
    set.into_iter().collect()
}

fn safe_file_name(s: &str) -> String {
    let n: String = s.chars().map(|c| if c.is_alphanumeric() || "._- ".contains(c) { c } else { '_' }).collect();
    let n = n.trim().trim_matches('.').to_string();
    if n.is_empty() {
        "image".into()
    } else {
        n
    }
}

/// Exporte un profil vers `dest` (fichier .hfprofile).
pub fn export(cfg_dir: &Path, profile_name: &str, dest: &Path) -> Result<usize, String> {
    let src = crate::profiles::path(cfg_dir, profile_name);
    let mut cfg = config::load(&src)?;
    let addons = used_addons(&cfg);

    // Images : chemins relatifs à la config ou absolus → icons/<nom> dans l'archive.
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let mut renamed: HashMap<String, String> = HashMap::new();
    for icon in key_icons(&mut cfg) {
        let Some(path) = icon.clone() else { continue };
        if path.starts_with("builtin:") || path.starts_with("addon:") || path.trim().is_empty() {
            continue;
        }
        if let Some(new) = renamed.get(&path) {
            *icon = Some(new.clone());
            continue;
        }
        let real = if Path::new(&path).is_absolute() { PathBuf::from(&path) } else { cfg_dir.join(&path) };
        if !real.is_file() {
            continue; // image déjà absente : la touche affichera « introuvable » de toute façon
        }
        let base = safe_file_name(&real.file_name().unwrap_or_default().to_string_lossy());
        let mut name = format!("icons/{base}");
        let mut n = 2;
        while files.iter().any(|(f, _)| f == &name) {
            name = format!("icons/{n}-{base}");
            n += 1;
        }
        files.push((name.clone(), real));
        renamed.insert(path, name.clone());
        *icon = Some(name);
    }

    // Sons : → sounds/<nom> dans l'archive.
    for file in key_sounds(&mut cfg) {
        let path = file.clone();
        if path.trim().is_empty() {
            continue;
        }
        if let Some(new) = renamed.get(&path) {
            *file = new.clone();
            continue;
        }
        let real = if Path::new(&path).is_absolute() { PathBuf::from(&path) } else { cfg_dir.join(&path) };
        if !real.is_file() {
            continue;
        }
        let base = safe_file_name(&real.file_name().unwrap_or_default().to_string_lossy());
        let mut name = format!("sounds/{base}");
        let mut n = 2;
        while files.iter().any(|(f, _)| f == &name) {
            name = format!("sounds/{n}-{base}");
            n += 1;
        }
        files.push((name.clone(), real));
        renamed.insert(path, name.clone());
        *file = name;
    }

    let manifest = Manifest { format: 1, name: profile_name.to_string(), hubforge: crate::update::CURRENT.to_string(), addons };
    let body = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;

    let tmp = dest.with_extension("hfprofile.tmp");
    let file = std::fs::File::create(&tmp).map_err(|e| format!("écriture impossible : {e}"))?;
    let mut z = zip::ZipWriter::new(file);
    let o = zip::write::SimpleFileOptions::default();
    let res = (|| -> Result<(), String> {
        z.start_file("manifest.json", o).map_err(|e| e.to_string())?;
        z.write_all(serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
        z.start_file("profile.toml", o).map_err(|e| e.to_string())?;
        z.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
        for (name, real) in &files {
            let data = std::fs::read(real).map_err(|e| format!("{} : {e}", real.display()))?;
            z.start_file(name.as_str(), o).map_err(|e| e.to_string())?;
            z.write_all(&data).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(e) = res {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, dest).map_err(|e| format!("écriture impossible : {e}"))?;
    Ok(files.len())
}

/// Importe un profil (contenu d'un .hfprofile) sous un nom libre.
pub fn import(cfg_dir: &Path, bytes: &[u8]) -> Result<ImportReport, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "ce fichier n'est pas un profil HubForge (.hfprofile)".to_string())?;
    let mut read = |name: &str| -> Option<Vec<u8>> {
        let mut f = zip.by_name(name).ok()?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).ok()?;
        Some(buf)
    };
    let manifest: Manifest = read("manifest.json")
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or("manifest.json manquant ou illisible : ce n'est pas un profil HubForge")?;
    if manifest.format > 1 {
        return Err("ce profil vient d'une version plus récente de HubForge : mets HubForge à jour".into());
    }
    let text = String::from_utf8(read("profile.toml").ok_or("profile.toml manquant")?).map_err(|_| "profile.toml illisible")?;
    let mut cfg: Config = toml::from_str(&text).map_err(|e| format!("profile.toml : {e}"))?;
    config::validate(&cfg)?;

    // Nom libre : « Laura », « Laura (2) »…
    let wanted = crate::profiles::check_name(&manifest.name).unwrap_or_else(|_| "Profil importé".into());
    let existing = crate::profiles::list(cfg_dir);
    let mut name = wanted.clone();
    let mut n = 2;
    while existing.iter().any(|e| e.eq_ignore_ascii_case(&name)) {
        name = format!("{wanted} ({n})");
        n += 1;
    }

    // Images : extraites dans icons/import-<profil>/.
    let slug = safe_file_name(&name).replace(' ', "-").to_lowercase();
    let icons_dir = cfg_dir.join("icons").join(format!("import-{slug}"));
    let mut images = 0;
    let mut total = 0u64;
    let names: Vec<String> = (0..zip.len()).filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string())).collect();
    for entry in names.iter().filter(|n| n.starts_with("icons/") && !n.ends_with('/')) {
        let base = safe_file_name(entry.trim_start_matches("icons/").rsplit('/').next().unwrap_or(""));
        let mut f = zip.by_name(entry).map_err(|e| e.to_string())?;
        total += f.size();
        if total > MAX_BYTES {
            return Err("profil trop gros (100 Mo maximum)".into());
        }
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&icons_dir).map_err(|e| e.to_string())?;
        std::fs::write(icons_dir.join(&base), buf).map_err(|e| e.to_string())?;
        let new_ref = format!("icons/import-{slug}/{base}");
        for icon in key_icons(&mut cfg) {
            if icon.as_deref() == Some(entry.as_str()) {
                *icon = Some(new_ref.clone());
            }
        }
        images += 1;
    }

    // Sons : extraits dans sounds/import-<profil>/.
    let sounds_dir = cfg_dir.join("sounds").join(format!("import-{slug}"));
    let mut sounds = 0;
    for entry in names.iter().filter(|n| n.starts_with("sounds/") && !n.ends_with('/')) {
        let base = safe_file_name(entry.trim_start_matches("sounds/").rsplit('/').next().unwrap_or(""));
        let mut f = zip.by_name(entry).map_err(|e| e.to_string())?;
        total += f.size();
        if total > MAX_BYTES {
            return Err("profil trop gros (100 Mo maximum)".into());
        }
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&sounds_dir).map_err(|e| e.to_string())?;
        std::fs::write(sounds_dir.join(&base), buf).map_err(|e| e.to_string())?;
        sounds += 1;
        let new_ref = format!("sounds/import-{slug}/{base}");
        for file in key_sounds(&mut cfg) {
            if file.as_str() == entry.as_str() {
                *file = new_ref.clone();
            }
        }
    }

    crate::profiles::create(cfg_dir, &name, None)?;
    config::save(&crate::profiles::path(cfg_dir, &name), &cfg)?;

    let installed: Vec<String> = crate::addons::scan(cfg_dir).into_iter().filter(|e| e.addon.is_some()).map(|e| e.folder).collect();
    let missing_addons = used_addons(&cfg).into_iter().filter(|a| !installed.contains(a)).collect();
    Ok(ImportReport { name, missing_addons, images, sounds, unsupported: Vec::new(), kind: "profile".into() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_then_import() {
        let d = std::env::temp_dir().join(format!("hubforge-share-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("icons")).unwrap();
        crate::profiles::init(&d);
        // Une image locale, une image absolue, une icône d'addon.
        std::fs::write(d.join("icons/logo.png"), b"PNG").unwrap();
        let abs = d.join("ailleurs.gif");
        std::fs::write(&abs, b"GIF").unwrap();
        let p = crate::profiles::path(&d, crate::profiles::DEFAULT_NAME);
        let mut cfg = config::load(&p).unwrap();
        cfg.pages[0].keys[0].icon = Some("icons/logo.png".into());
        cfg.pages[0].keys[1].icon = Some(abs.to_string_lossy().to_string());
        cfg.pages[0].keys[2].icon = Some("addon:discord/icons/x.svg".into());
        config::save(&p, &cfg).unwrap();

        let out = d.join("partage.hfprofile");
        assert_eq!(export(&d, crate::profiles::DEFAULT_NAME, &out).unwrap(), 2);

        let r = import(&d, &std::fs::read(&out).unwrap()).unwrap();
        assert_eq!(r.name, format!("{} (2)", crate::profiles::DEFAULT_NAME));
        assert_eq!(r.images, 2);
        assert_eq!(r.missing_addons, vec!["discord"]);
        let back = config::load(&crate::profiles::path(&d, &r.name)).unwrap();
        let i0 = back.pages[0].keys[0].icon.clone().unwrap();
        assert!(i0.starts_with("icons/import-"), "{i0}");
        assert!(d.join(&i0).is_file());
        assert!(d.join(back.pages[0].keys[1].icon.clone().unwrap()).is_file());
        assert!(import(&d, b"pas un zip").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
