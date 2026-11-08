//! Addons officiels livrés avec HubForge (intégrés au programme), installés
//! dans le dossier des addons au démarrage.
//!
//! Un addon officiel n'est réinstallé que s'il est absent ou plus ancien :
//! une version modifiée à la main par l'utilisateur est conservée tant que
//! HubForge n'apporte pas une version plus récente.

use std::path::Path;

/// (id, version, fichiers)
type Bundle = (&'static str, &'static str, &'static [(&'static str, &'static str)]);

/// Aucun addon n'est livré avec HubForge pour l'instant : ils seront
/// proposés dans un catalogue à télécharger.
const BUNDLES: &[Bundle] = &[];

/// Ids des addons officiels.
pub fn ids() -> Vec<&'static str> {
    BUNDLES.iter().map(|b| b.0).collect()
}

/// « 1.2.10 » → [1, 2, 10] pour comparer des versions.
fn version_key(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect()
}

fn installed_version(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("manifest.toml")).ok()?;
    let t: toml::Table = toml::from_str(&text).ok()?;
    t.get("addon")?.get("version")?.as_str().map(String::from)
}

/// Installe ou met à jour les addons officiels.
pub fn install(cfg_dir: &Path) {
    let base = crate::addons::dir(cfg_dir);
    for (id, version, files) in BUNDLES {
        let d = base.join(id);
        // Supprimé volontairement par l'utilisateur : on ne le remet pas.
        if base.join(format!(".{id}.supprime")).exists() {
            continue;
        }
        let needed = match installed_version(&d) {
            None => true,
            Some(v) => version_key(&v) < version_key(version),
        };
        if !needed {
            continue;
        }
        let ok = files.iter().all(|(rel, content)| {
            let p = d.join(rel);
            p.parent().map(|par| std::fs::create_dir_all(par).is_ok()).unwrap_or(true) && std::fs::write(&p, content).is_ok()
        });
        if ok {
            log::info!("addon officiel « {id} » {version} installé");
        } else {
            log::error!("installation de l'addon officiel « {id} » impossible");
        }
    }
}

/// Retient qu'un addon officiel a été supprimé (pour ne pas le réinstaller).
pub fn mark_removed(cfg_dir: &Path, id: &str) {
    if ids().contains(&id) {
        let _ = std::fs::write(crate::addons::dir(cfg_dir).join(format!(".{id}.supprime")), "");
    }
}

/// Annule `mark_removed` (réinstallation demandée).
pub fn unmark_removed(cfg_dir: &Path, id: &str) {
    let _ = std::fs::remove_file(crate::addons::dir(cfg_dir).join(format!(".{id}.supprime")));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_addons_are_valid() {
        let d = std::env::temp_dir().join(format!("hubforge-official-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        install(&d);
        let list = crate::addons::scan(&d);
        assert_eq!(list.len(), BUNDLES.len());
        for e in &list {
            assert!(e.error.is_none(), "{} : {:?}", e.folder, e.error);
            let a = e.addon.as_ref().unwrap();
            let v = BUNDLES.iter().find(|b| b.0 == a.id).unwrap().1;
            assert_eq!(a.version, v, "version du manifeste différente de BUNDLES");
            // Toutes les pages se construisent et chaque icône se dessine.
            let pages = a.build_pages(0);
            assert!(!pages.is_empty());
            for act in &a.actions {
                let v = crate::render::render_key(&act.key_def(1), &d);
                assert_eq!(v.frames.len(), 1, "{}", act.id);
            }
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
