//! Touches dessinées par les addons, et géométrie du module 15 touches.
//!
//! Le programme d'un addon envoie un dessin SVG pour un « canevas »
//! (`{"type":"draw","canvas":"progress","svg":"<svg…>"}`). Les touches dont
//! `canvas = "prusa.progress"` affichent ce dessin, découpé si la touche
//! couvre plusieurs cases (`span`).
//!
//! Le dessin d'une zone de c × r touches fait `c·72 + (c−1)·écart` pixels de
//! large : l'écart est la largeur (en pixels d'écran) des bords du module entre
//! deux touches. Il se règle dans HubForge avec une mire, et il est donné au
//! programme de l'addon (`layout`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Taille d'une touche, en pixels.
pub const KEY: u32 = 72;
/// Écart par défaut entre deux touches (pixels d'écran équivalents).
pub const DEFAULT_GAP: u8 = 14;
/// Taille maximale d'un dessin SVG.
pub const MAX_SVG: usize = 1_500_000;

static GAP: AtomicU8 = AtomicU8::new(DEFAULT_GAP);
static VERSION: AtomicU64 = AtomicU64::new(0);

pub fn gap() -> u32 {
    GAP.load(Ordering::Relaxed) as u32
}

pub fn set_gap(g: u8) {
    GAP.store(g.min(60), Ordering::Relaxed);
}

/// Taille en pixels d'une zone de `cols` × `rows` touches (écarts compris).
pub fn zone_size(cols: u8, rows: u8) -> (u32, u32) {
    let g = gap();
    let c = cols.max(1) as u32;
    let r = rows.max(1) as u32;
    (c * KEY + (c - 1) * g, r * KEY + (r - 1) * g)
}

/// Un dessin d'addon.
#[derive(Clone)]
pub struct Drawing {
    pub svg: Arc<String>,
    /// Dossier de l'addon : images et polices du dessin y sont cherchées.
    pub dir: PathBuf,
}

fn store() -> &'static Mutex<HashMap<String, Drawing>> {
    static S: OnceLock<Mutex<HashMap<String, Drawing>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Nom complet d'un canevas : « addon.nom ».
pub fn full_name(addon: &str, canvas: &str) -> String {
    format!("{addon}.{canvas}")
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 40 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Enregistre un dessin ; retourne false s'il n'a pas changé.
pub fn set(name: &str, svg: String, dir: &Path) -> bool {
    let mut m = store().lock().unwrap();
    if m.get(name).is_some_and(|d| *d.svg == svg) {
        return false;
    }
    VERSION.fetch_add(1, Ordering::Relaxed);
    m.insert(name.to_string(), Drawing { svg: Arc::new(svg), dir: dir.to_path_buf() });
    true
}

pub fn get(name: &str) -> Option<Drawing> {
    store().lock().unwrap().get(name).cloned()
}

/// Version globale : change à chaque nouveau dessin (aperçus de l'interface).
pub fn version() -> u64 {
    VERSION.load(Ordering::Relaxed)
}

// ----------------------------------------------------------------------
// Rendu SVG des dessins d'addons
// ----------------------------------------------------------------------

/// Options de rendu pour un addon : ses polices (dossier `fonts/`) en plus
/// de celles de HubForge, et son dossier pour les images.
fn options(dir: &Path) -> Arc<resvg::usvg::Options<'static>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<resvg::usvg::Options<'static>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(o) = cache.lock().unwrap().get(dir) {
        return o.clone();
    }
    let base = crate::theme::font_db();
    let mut db = (*base).clone();
    if let Ok(rd) = std::fs::read_dir(dir.join("fonts")) {
        for e in rd.flatten() {
            let p = e.path();
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if ext == "ttf" || ext == "otf" {
                match std::fs::read(&p) {
                    Ok(data) => db.load_font_data(data),
                    Err(err) => log::warn!("police {} : {err}", p.display()),
                }
            }
        }
    }
    let mut o = resvg::usvg::Options { font_family: "Inter".into(), resources_dir: Some(dir.to_path_buf()), ..Default::default() };
    o.fontdb = Arc::new(db);
    let o = Arc::new(o);
    cache.lock().unwrap().insert(dir.to_path_buf(), o.clone());
    o
}

/// Dessine un SVG d'addon sur une image de `w` × `h` pixels (fond noir). Le
/// SVG est mis à l'échelle de l'image (sa taille ou son viewBox).
pub fn render(d: &Drawing, w: u32, h: u32) -> Option<image::RgbaImage> {
    let opts = options(&d.dir);
    let tree = match resvg::usvg::Tree::from_str(&d.svg, &opts) {
        Ok(t) => t,
        Err(e) => {
            log::warn!("dessin d'addon illisible : {e}");
            return None;
        }
    };
    let mut pm = resvg::tiny_skia::Pixmap::new(w, h)?;
    pm.fill(resvg::tiny_skia::Color::BLACK);
    let size = tree.size();
    let sx = w as f32 / size.width();
    let sy = h as f32 / size.height();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(sx, sy), &mut pm.as_mut());
    image::RgbaImage::from_raw(w, h, pm.take())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_sizes_follow_the_gap() {
        set_gap(10);
        assert_eq!(zone_size(1, 1), (72, 72));
        assert_eq!(zone_size(5, 1), (72 * 5 + 40, 72));
        assert_eq!(zone_size(2, 2), (154, 154));
        set_gap(DEFAULT_GAP);
    }

    #[test]
    fn drawings_render() {
        let dir = std::env::temp_dir();
        assert!(set("t.a", r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72"><rect x="0" y="0" width="36" height="72" fill="#FA6831"/></svg>"##.into(), &dir));
        assert!(!set("t.a", r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72"><rect x="0" y="0" width="36" height="72" fill="#FA6831"/></svg>"##.into(), &dir));
        let img = render(&get("t.a").unwrap(), 144, 72).unwrap();
        assert_eq!(img.get_pixel(10, 10).0[..3], [250, 104, 49]);
        assert_eq!(img.get_pixel(100, 10).0[..3], [0, 0, 0]);
    }
}
