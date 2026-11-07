//! Addons officiels livrés avec HubForge (intégrés au programme), installés
//! dans le dossier des addons au démarrage.
//!
//! Un addon officiel n'est réinstallé que s'il est absent ou plus ancien :
//! une version modifiée à la main par l'utilisateur est conservée tant que
//! HubForge n'apporte pas une version plus récente.

use std::path::Path;

/// (id, version, fichiers)
type Bundle = (&'static str, &'static str, &'static [(&'static str, &'static str)]);

const BUNDLES: &[Bundle] = &[(
    "fusion360",
    "1.1.0",
    &[
        ("manifest.toml", include_str!("../../../assets/addons/fusion360/manifest.toml")),
        ("icons/arc-3p.svg", include_str!("../../../assets/addons/fusion360/icons/arc-3p.svg")),
        ("icons/chamfer.svg", include_str!("../../../assets/addons/fusion360/icons/chamfer.svg")),
        ("icons/circle-2p.svg", include_str!("../../../assets/addons/fusion360/icons/circle-2p.svg")),
        ("icons/circle-3p.svg", include_str!("../../../assets/addons/fusion360/icons/circle-3p.svg")),
        ("icons/circle-center.svg", include_str!("../../../assets/addons/fusion360/icons/circle-center.svg")),
        ("icons/compute.svg", include_str!("../../../assets/addons/fusion360/icons/compute.svg")),
        ("icons/construction.svg", include_str!("../../../assets/addons/fusion360/icons/construction.svg")),
        ("icons/dimension.svg", include_str!("../../../assets/addons/fusion360/icons/dimension.svg")),
        ("icons/extrude.svg", include_str!("../../../assets/addons/fusion360/icons/extrude.svg")),
        ("icons/fillet.svg", include_str!("../../../assets/addons/fusion360/icons/fillet.svg")),
        ("icons/finish-sketch.svg", include_str!("../../../assets/addons/fusion360/icons/finish-sketch.svg")),
        ("icons/fusion.svg", include_str!("../../../assets/addons/fusion360/icons/fusion.svg")),
        ("icons/hole.svg", include_str!("../../../assets/addons/fusion360/icons/hole.svg")),
        ("icons/joint.svg", include_str!("../../../assets/addons/fusion360/icons/joint.svg")),
        ("icons/line.svg", include_str!("../../../assets/addons/fusion360/icons/line.svg")),
        ("icons/measure.svg", include_str!("../../../assets/addons/fusion360/icons/measure.svg")),
        ("icons/mirror.svg", include_str!("../../../assets/addons/fusion360/icons/mirror.svg")),
        ("icons/move.svg", include_str!("../../../assets/addons/fusion360/icons/move.svg")),
        ("icons/offset.svg", include_str!("../../../assets/addons/fusion360/icons/offset.svg")),
        ("icons/presspull.svg", include_str!("../../../assets/addons/fusion360/icons/presspull.svg")),
        ("icons/project.svg", include_str!("../../../assets/addons/fusion360/icons/project.svg")),
        ("icons/rectangle.svg", include_str!("../../../assets/addons/fusion360/icons/rectangle.svg")),
        ("icons/revolve.svg", include_str!("../../../assets/addons/fusion360/icons/revolve.svg")),
        ("icons/sketch.svg", include_str!("../../../assets/addons/fusion360/icons/sketch.svg")),
        ("icons/toolbox.svg", include_str!("../../../assets/addons/fusion360/icons/toolbox.svg")),
        ("icons/trim.svg", include_str!("../../../assets/addons/fusion360/icons/trim.svg")),
    ],
)];

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
        mark_removed(&d, "fusion360");
        std::fs::remove_dir_all(crate::addons::dir(&d).join("fusion360")).unwrap();
        install(&d);
        assert!(crate::addons::scan(&d).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Planche des pages Fusion : cargo test -p hubforge fusion_sheet -- --ignored
    #[test]
    #[ignore]
    fn fusion_sheet() {
        use hubforge_device::KEY_IMAGE_SIZE as S;
        let d = std::env::temp_dir().join("hubforge-fusion-sheet");
        let _ = std::fs::remove_dir_all(&d);
        install(&d);
        let a = crate::addons::scan(&d).remove(0).addon.unwrap();
        let pages = a.build_pages(0);
        let mut sheet = image::RgbImage::from_pixel(5 * (S + 6) + 6, 2 * (3 * (S + 6) + 12), image::Rgb([14, 15, 19]));
        for (pi, p) in pages.iter().enumerate() {
            for k in &p.keys {
                let v = crate::render::render_key(k, &d);
                let img = image::load_from_memory(&v.frames[0].jpeg).unwrap().to_rgb8();
                let i = (k.key - 1) as u32;
                let x = 6 + (i % 5) * (S + 6);
                let y = 6 + pi as u32 * (3 * (S + 6) + 12) + (i / 5) * (S + 6);
                image::imageops::overlay(&mut sheet, &img, x as i64, y as i64);
            }
        }
        let big = image::imageops::resize(&sheet, sheet.width() * 2, sheet.height() * 2, image::imageops::FilterType::Lanczos3);
        big.save("/tmp/hubforge-fusion.png").unwrap();
    }
}
