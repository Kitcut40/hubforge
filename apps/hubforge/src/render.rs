//! Rendu des touches : fond coloré, texte, images et GIF animés,
//! encodés en JPEG 72×72 pour le module 15 touches.

use crate::config::{parse_color, KeyDef};
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use hubforge_device::KEY_IMAGE_SIZE as S;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{AnimationDecoder, DynamicImage, Rgb, RgbImage, Rgba, RgbaImage};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.otf");
const MAX_FRAMES: usize = 240;

fn font() -> &'static FontRef<'static> {
    static F: OnceLock<FontRef<'static>> = OnceLock::new();
    F.get_or_init(|| FontRef::try_from_slice(FONT_BYTES).expect("police intégrée invalide"))
}

/// Une image prête à envoyer.
#[derive(Clone)]
pub struct Frame {
    pub jpeg: Arc<Vec<u8>>,
    /// Version assombrie (retour visuel pendant l'appui).
    pub pressed: Arc<Vec<u8>>,
    pub delay: Duration,
}

/// Visuel d'une touche : une seule image, ou plusieurs (animation).
#[derive(Clone)]
pub struct KeyVisual {
    pub frames: Vec<Frame>,
}

impl KeyVisual {
    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }
}

pub fn encode_jpeg(img: &RgbImage, quality: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let _ = JpegEncoder::new_with_quality(&mut out, quality).encode_image(img);
    out
}

fn darken(img: &RgbImage) -> RgbImage {
    let mut d = img.clone();
    for p in d.pixels_mut() {
        for c in p.0.iter_mut() {
            *c = (*c as u16 * 45 / 100) as u8;
        }
    }
    d
}

fn make_frame(img: &RgbImage, delay: Duration, quality: u8) -> Frame {
    Frame {
        jpeg: Arc::new(encode_jpeg(img, quality)),
        pressed: Arc::new(encode_jpeg(&darken(img), quality)),
        delay,
    }
}

/// Image entièrement noire (touche vide).
pub fn blank() -> KeyVisual {
    let img = RgbImage::from_pixel(S, S, Rgb([0, 0, 0]));
    KeyVisual { frames: vec![make_frame(&img, Duration::ZERO, 85)] }
}

/// Image affichée en cas d'erreur (fichier introuvable…).
pub fn error_visual(msg: &str) -> KeyVisual {
    let mut img = RgbImage::from_pixel(S, S, Rgb([120, 0, 0]));
    draw_text_block(&mut img, msg, Rgb([255, 255, 255]), 0, S);
    KeyVisual { frames: vec![make_frame(&img, Duration::ZERO, 85)] }
}

/// Aperçu pour l'interface : première image (JPEG) + animé ou non.
pub fn preview(def: &KeyDef, base_dir: &Path) -> (Vec<u8>, bool) {
    let v = render_key_opts(def, base_dir, true);
    let animated = def
        .icon
        .as_deref()
        .map(|i| {
            let l = i.to_lowercase();
            l.ends_with(".gif") || l.ends_with(".webp") || l.starts_with("anim:")
        })
        .unwrap_or(false);
    (v.frames[0].jpeg.as_ref().clone(), animated)
}

#[allow(dead_code)]
pub fn render_key(def: &KeyDef, base_dir: &Path) -> KeyVisual {
    render_key_rot(def, base_dir, 0)
}

/// Rendu pour un module de touches tourné de `rot` degrés (sens horaire) :
/// l'image est tournée dans l'autre sens pour rester lisible.
pub fn render_key_rot(def: &KeyDef, base_dir: &Path, rot: u16) -> KeyVisual {
    let mut v = render_key_opts(def, base_dir, false);
    if rot % 360 != 0 {
        for f in &mut v.frames {
            *f = rotate_frame(f, rot);
        }
    }
    v
}

fn rotate_frame(f: &Frame, rot: u16) -> Frame {
    let turn = |jpeg: &Arc<Vec<u8>>| -> Arc<Vec<u8>> {
        let Ok(img) = image::load_from_memory(jpeg) else { return jpeg.clone() };
        let img = img.to_rgb8();
        let img = match rot % 360 {
            90 => image::imageops::rotate270(&img),
            180 => image::imageops::rotate180(&img),
            270 => image::imageops::rotate90(&img),
            _ => img,
        };
        Arc::new(encode_jpeg(&img, 90))
    };
    Frame { jpeg: turn(&f.jpeg), pressed: turn(&f.pressed), delay: f.delay }
}

fn render_key_opts(def: &KeyDef, base_dir: &Path, first_only: bool) -> KeyVisual {
    let bg = def.color.as_deref().and_then(parse_color).unwrap_or((0, 0, 0));
    let fg = def.text_color.as_deref().and_then(parse_color).unwrap_or((255, 255, 255));
    let bg = Rgb([bg.0, bg.1, bg.2]);
    let fg = Rgb([fg.0, fg.1, fg.2]);

    // Thèmes : icônes intégrées, icônes d'addons et texte seul. Les images et
    // GIF de l'utilisateur restent telles quelles.
    if let Some(theme) = crate::theme::of(def) {
        let icon = def.icon.as_deref().unwrap_or("").trim();
        if icon.is_empty() || icon.starts_with("builtin:") {
            let img = crate::theme::render_key(theme, def, None);
            return KeyVisual { frames: vec![make_frame(&img, Duration::ZERO, 92)] };
        }
        if icon.starts_with("addon:") {
            if let Some(svg) = crate::addons::resolve_icon(base_dir, icon)
                .filter(|p| p.extension().map(|e| e.eq_ignore_ascii_case("svg")).unwrap_or(false))
                .and_then(|p| std::fs::read_to_string(p).ok())
            {
                let img = crate::theme::render_key(theme, def, Some(&svg));
                return KeyVisual { frames: vec![make_frame(&img, Duration::ZERO, 92)] };
            }
        }
    }

    if let Some(name) = def.icon.as_deref().and_then(|i| i.strip_prefix("builtin:")) {
        let mut img = RgbImage::from_pixel(S, S, bg);
        draw_builtin(&mut img, name, fg, !def.label.trim().is_empty());
        if !def.label.trim().is_empty() {
            draw_text_sized(&mut img, &def.label, fg, 44, S - 46, 13);
        }
        return KeyVisual { frames: vec![make_frame(&img, Duration::ZERO, 90)] };
    }

    let Some(icon) = def.icon.as_deref().filter(|s| !s.trim().is_empty()) else {
        let mut img = RgbImage::from_pixel(S, S, bg);
        draw_text_block(&mut img, &def.label, fg, 0, S);
        return KeyVisual { frames: vec![make_frame(&img, Duration::ZERO, 90)] };
    };

    // Touche animée intégrée.
    if let Some(id) = icon.strip_prefix("anim:") {
        let Some(bytes) = crate::anims::get(id) else { return error_visual("Animation inconnue") };
        let mut frames = match gif_frames(bytes) {
            Ok(f) if !f.is_empty() => f,
            _ => return error_visual("Animation illisible"),
        };
        if first_only {
            frames.truncate(1);
        }
        return finish_frames(def, frames, bg, fg);
    }

    let path = if icon.starts_with("addon:") {
        match crate::addons::resolve_icon(base_dir, icon) {
            Some(p) => p,
            None => return error_visual("Icône invalide"),
        }
    } else if Path::new(icon).is_absolute() {
        Path::new(icon).to_path_buf()
    } else {
        base_dir.join(icon)
    };
    // Icône SVG (addons) : dessinée comme les icônes intégrées, à la couleur du texte.
    if path.extension().map(|e| e.eq_ignore_ascii_case("svg")).unwrap_or(false) {
        let Ok(svg) = std::fs::read_to_string(&path) else {
            log::warn!("touche {} : icône introuvable {}", def.key, path.display());
            return error_visual("Image introuvable");
        };
        let mut img = RgbImage::from_pixel(S, S, bg);
        let with_label = !def.label.trim().is_empty();
        let size: u32 = if with_label { 34 } else { 46 };
        draw_svg_icon(&mut img, &svg, fg, size, if with_label { 8 } else { (S - size) / 2 });
        if with_label {
            draw_text_sized(&mut img, &def.label, fg, 44, S - 46, 13);
        }
        return KeyVisual { frames: vec![make_frame(&img, Duration::ZERO, 90)] };
    }
    let frames = match load_frames(&path).map(|mut f| {
        if first_only {
            f.truncate(1);
        }
        f
    }) {
        Ok(f) if !f.is_empty() => f,
        Ok(_) => return error_visual("Image vide"),
        Err(e) => {
            log::warn!("touche {} : {e}", def.key);
            return error_visual("Image introuvable");
        }
    };
    finish_frames(def, frames, bg, fg)
}

/// Images d'une animation ou d'une image → visuel (avec le bandeau du titre).
fn finish_frames(def: &KeyDef, frames: Vec<(RgbaImage, Duration)>, bg: Rgb<u8>, fg: Rgb<u8>) -> KeyVisual {
    let animated = frames.len() > 1;
    let quality = if animated { 72 } else { 90 };
    let out = frames
        .into_iter()
        .map(|(rgba, delay)| {
            let mut img = RgbImage::from_pixel(S, S, bg);
            paste_fit(&mut img, &rgba);
            if !def.label.trim().is_empty() {
                draw_label_band(&mut img, &def.label, fg);
            }
            make_frame(&img, delay, quality)
        })
        .collect();
    KeyVisual { frames: out }
}

/// Images d'un GIF en mémoire.
fn gif_frames(bytes: &[u8]) -> Result<Vec<(RgbaImage, Duration)>, String> {
    let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let frames = dec.into_frames().collect_frames().map_err(|e| e.to_string())?;
    Ok(frames
        .into_iter()
        .take(MAX_FRAMES)
        .map(|f| {
            let (n, d) = f.delay().numer_denom_ms();
            let ms = if d == 0 { 100 } else { (n / d).max(20) };
            (f.into_buffer(), Duration::from_millis(ms as u64))
        })
        .collect())
}

fn load_frames(path: &Path) -> Result<Vec<(RgbaImage, Duration)>, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let read = || std::fs::File::open(path).map(std::io::BufReader::new).map_err(|e| format!("{} : {e}", path.display()));
    let to_frames = |frames: Vec<image::Frame>| -> Vec<(RgbaImage, Duration)> {
        frames
            .into_iter()
            .take(MAX_FRAMES)
            .map(|f| {
                let (n, d) = f.delay().numer_denom_ms();
                let ms = if d == 0 { 100 } else { (n / d).max(20) };
                (f.into_buffer(), Duration::from_millis(ms as u64))
            })
            .collect()
    };
    if ext == "gif" {
        let dec = image::codecs::gif::GifDecoder::new(read()?).map_err(|e| e.to_string())?;
        let frames = dec.into_frames().collect_frames().map_err(|e| e.to_string())?;
        return Ok(to_frames(frames));
    }
    if ext == "webp" {
        let dec = image::codecs::webp::WebPDecoder::new(read()?).map_err(|e| e.to_string())?;
        if dec.has_animation() {
            let frames = dec.into_frames().collect_frames().map_err(|e| e.to_string())?;
            return Ok(to_frames(frames));
        }
    }
    let img = image::open(path).map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(vec![(img.to_rgba8(), Duration::ZERO)])
}

/// Colle une image en la redimensionnant pour tenir dans 72×72 (sans déformer).
fn paste_fit(dst: &mut RgbImage, src: &RgbaImage) {
    let (w, h) = src.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    let scale = (S as f32 / w as f32).min(S as f32 / h as f32);
    let (nw, nh) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
    let resized = if (nw, nh) == (w, h) {
        src.clone()
    } else {
        DynamicImage::ImageRgba8(src.clone()).resize_exact(nw, nh, FilterType::Lanczos3).to_rgba8()
    };
    let (ox, oy) = ((S - nw) / 2, (S - nh) / 2);
    for (x, y, p) in resized.enumerate_pixels() {
        blend(dst, ox + x, oy + y, *p);
    }
}

pub(crate) fn blend(img: &mut RgbImage, x: u32, y: u32, p: Rgba<u8>) {
    if x >= img.width() || y >= img.height() {
        return;
    }
    let a = p.0[3] as u32;
    let d = img.get_pixel_mut(x, y);
    for c in 0..3 {
        d.0[c] = ((p.0[c] as u32 * a + d.0[c] as u32 * (255 - a)) / 255) as u8;
    }
}

/// Bandeau semi-transparent en bas avec le libellé (sur les images).
fn draw_label_band(img: &mut RgbImage, label: &str, fg: Rgb<u8>) {
    let band = 20;
    for y in S - band..S {
        for x in 0..S {
            blend(img, x, y, Rgba([0, 0, 0, 150]));
        }
    }
    draw_text_sized(img, label, fg, S - band, band, 13);
}

/// Découpe le texte en lignes qui tiennent dans `max_w` pixels.
fn wrap(text: &str, scale: PxScale, max_w: f32) -> Vec<String> {
    let f = font().as_scaled(scale);
    let width = |s: &str| -> f32 { s.chars().map(|c| f.h_advance(f.glyph_id(c))).sum() };
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if width(&candidate) <= max_w || line.is_empty() {
                line = candidate;
            } else {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            }
        }
        lines.push(line);
    }
    lines
}

/// Dessine un texte centré dans la zone verticale [top, top+height),
/// en choisissant la plus grande taille qui tient.
fn draw_text_block(img: &mut RgbImage, text: &str, color: Rgb<u8>, top: u32, height: u32) {
    draw_text_sized(img, text, color, top, height, 22);
}

/// Comme `draw_text_block`, avec une taille de police maximale.
pub(crate) fn draw_text_sized(img: &mut RgbImage, text: &str, color: Rgb<u8>, top: u32, height: u32, max_size: u32) {
    draw_text_in(img, text, color, S as f32 / 2.0, S as f32 - 8.0, top, height, max_size);
}

/// Texte centré sur `cx`, dans une colonne de largeur `max_w`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_text_in(img: &mut RgbImage, text: &str, color: Rgb<u8>, cx: f32, max_w: f32, top: u32, height: u32, max_size: u32) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let mut chosen = None;
    for size in (9..=max_size.max(9)).rev() {
        let scale = PxScale::from(size as f32);
        let lines = wrap(text, scale, max_w);
        let f = font().as_scaled(scale);
        let line_h = f.height() * 0.95;
        let total_h = line_h * lines.len() as f32;
        let widest = lines
            .iter()
            .map(|l| l.chars().map(|c| f.h_advance(f.glyph_id(c))).sum::<f32>())
            .fold(0.0, f32::max);
        if total_h <= height as f32 - 2.0 && widest <= max_w {
            chosen = Some((scale, lines, line_h, total_h));
            break;
        }
    }
    let (scale, lines, line_h, total_h) = chosen.unwrap_or_else(|| {
        let scale = PxScale::from(9.0);
        let lines = wrap(text, scale, max_w);
        let lh = font().as_scaled(scale).height() * 0.95;
        let th = lh * lines.len() as f32;
        (scale, lines, lh, th)
    });
    let f = font().as_scaled(scale);
    let mut y = top as f32 + (height as f32 - total_h) / 2.0;
    for line in &lines {
        let w: f32 = line.chars().map(|c| f.h_advance(f.glyph_id(c))).sum();
        let mut x = cx - w / 2.0;
        let baseline = y + f.ascent();
        for c in line.chars() {
            let id = f.glyph_id(c);
            let glyph = id.with_scale_and_position(scale, ab_glyph::point(x, baseline));
            if let Some(outline) = font().outline_glyph(glyph) {
                let b = outline.px_bounds();
                outline.draw(|gx, gy, cov| {
                    let px = b.min.x as i32 + gx as i32;
                    let py = b.min.y as i32 + gy as i32;
                    if px >= 0 && py >= 0 {
                        let a = (cov.clamp(0.0, 1.0) * 255.0) as u8;
                        blend(img, px as u32, py as u32, Rgba([color.0[0], color.0[1], color.0[2], a]));
                    }
                });
            }
            x += f.h_advance(id);
        }
        y += line_h;
    }
}

/// SVG d'une icône intégrée (Lucide), ou None si inconnue.
pub fn builtin_svg(name: &str) -> Option<&'static str> {
    crate::icons_data::ICONS.iter().find(|(n, _)| *n == name).map(|(_, svg)| *svg)
}

/// Dessine une icône intégrée : en haut si un texte suit, sinon centrée.
fn draw_builtin(img: &mut RgbImage, name: &str, color: Rgb<u8>, with_label: bool) {
    if builtin_svg(name).is_none() {
        draw_text_block(img, "?", color, 0, S);
        return;
    }
    let size: u32 = if with_label { 34 } else { 46 };
    let oy = if with_label { 8 } else { (S - size) / 2 };
    draw_builtin_at(img, name, color, size, oy);
}

/// Dessine une icône intégrée de `size` pixels, centrée horizontalement, à la hauteur `oy`.
pub(crate) fn draw_builtin_at(img: &mut RgbImage, name: &str, color: Rgb<u8>, size: u32, oy: u32) {
    let Some(svg) = builtin_svg(name) else { return };
    draw_svg_icon(img, svg, color, size, oy);
}

/// Dessine une icône SVG (style Lucide : `currentColor` = couleur du texte).
pub(crate) fn draw_svg_icon(img: &mut RgbImage, svg: &str, color: Rgb<u8>, size: u32, oy: u32) {
    let hex = format!("#{:02X}{:02X}{:02X}", color.0[0], color.0[1], color.0[2]);
    let svg = svg.replace("currentColor", &hex);
    let Ok(tree) = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()) else { return };
    let Some(mut pm) = resvg::tiny_skia::Pixmap::new(size, size) else { return };
    let ts = tree.size();
    let scale = size as f32 / ts.width().max(ts.height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    let ox = S.saturating_sub(size) / 2;
    for (i, px) in pm.pixels().iter().enumerate() {
        let a = px.alpha();
        if a == 0 {
            continue;
        }
        // Pixels prémultipliés → couleur réelle
        let c = px.demultiply();
        let (x, y) = (i as u32 % size, i as u32 / size);
        if ox + x < S && oy + y < S {
            blend(img, ox + x, oy + y, Rgba([c.red(), c.green(), c.blue(), a]));
        }
    }
}

/// Dessine un SVG de 72×72 par-dessus l'image (formes des touches vivantes).
pub(crate) fn draw_svg(img: &mut RgbImage, svg: &str) {
    let Ok(tree) = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()) else {
        log::debug!("SVG invalide");
        return;
    };
    let Some(mut pm) = resvg::tiny_skia::Pixmap::new(S, S) else { return };
    let ts = tree.size();
    let scale = S as f32 / ts.width().max(ts.height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    for (i, px) in pm.pixels().iter().enumerate() {
        if px.alpha() == 0 {
            continue;
        }
        let c = px.demultiply();
        blend(img, i as u32 % S, i as u32 / S, Rgba([c.red(), c.green(), c.blue(), px.alpha()]));
    }
}

/// Visuel d'une seule image (touches vivantes), tourné comme le module.
pub(crate) fn visual_from_image(img: &RgbImage, rot: u16) -> KeyVisual {
    let f = make_frame(img, Duration::ZERO, 90);
    let f = if rot % 360 != 0 { rotate_frame(&f, rot) } else { f };
    KeyVisual { frames: vec![f] }
}

/// Crée le GIF de démonstration (anneau arc-en-ciel qui tourne).
pub fn write_demo_gif(path: &Path) -> Result<(), String> {
    use image::codecs::gif::{GifEncoder, Repeat};
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = GifEncoder::new_with_speed(file, 10);
    enc.set_repeat(Repeat::Infinite).map_err(|e| e.to_string())?;
    let n = 24;
    for i in 0..n {
        let mut img = RgbaImage::from_pixel(S, S, Rgba([10, 10, 20, 255]));
        let c = S as f32 / 2.0;
        for y in 0..S {
            for x in 0..S {
                let (dx, dy) = (x as f32 - c + 0.5, y as f32 - c + 0.5);
                let r = (dx * dx + dy * dy).sqrt();
                if (18.0..30.0).contains(&r) {
                    let ang = dy.atan2(dx).to_degrees() + 180.0 + i as f32 * 360.0 / n as f32;
                    let (cr, cg, cb) = hsv(ang % 360.0);
                    img.put_pixel(x, y, Rgba([cr, cg, cb, 255]));
                }
            }
        }
        let frame = image::Frame::from_parts(img, 0, 0, image::Delay::from_numer_denom_ms(50, 1));
        enc.encode_frame(frame).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn hsv(h: f32) -> (u8, u8, u8) {
    let c = 1.0;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Action, KeyDef};

    fn key(label: &str, icon: Option<&str>) -> KeyDef {
        KeyDef {
            key: 1,
            label: label.into(),
            icon: icon.map(String::from),
            color: Some("#1565C0".into()),
            text_color: None,
            action: Action::None,
            toggle: None,
            widget: None,
            widget_style: None,
            widget_target: None,
            long: None,
            double: None,
            theme: None,
        }
    }

    #[test]
    fn renders_text_key() {
        let v = render_key(&key("Gestion. tâches", None), Path::new("."));
        assert_eq!(v.frames.len(), 1);
        assert_eq!(&v.frames[0].jpeg[..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn renders_demo_gif() {
        let dir = std::env::temp_dir().join("hubforge-test");
        std::fs::create_dir_all(&dir).unwrap();
        write_demo_gif(&dir.join("demo.gif")).unwrap();
        let v = render_key(&key("GIF", Some("demo.gif")), &dir);
        assert_eq!(v.frames.len(), 24);
        assert!(v.is_animated());
    }

    #[test]
    fn renders_builtin_icon() {
        let mut d = key("Copier", None);
        d.icon = Some("builtin:copy".into());
        let v = render_key(&d, Path::new("."));
        assert_eq!(v.frames.len(), 1);
        assert!(builtin_svg("copy").is_some());
        assert!(builtin_svg("nexistepas").is_none());
    }

    #[test]
    fn missing_image_gives_error_visual() {
        let v = render_key(&key("x", Some("nexiste_pas.png")), Path::new("."));
        assert_eq!(v.frames.len(), 1);
    }
}

#[cfg(test)]
mod preview {
    use super::*;
    use crate::config::{Config, DEFAULT_CONFIG};

    /// Génère un aperçu des pages de la config d'exemple :
    /// `cargo test -p hubforge preview -- --ignored`
    #[test]
    #[ignore]
    fn preview_sheet() {
        let dir = std::env::temp_dir().join("hubforge-preview");
        std::fs::create_dir_all(&dir).unwrap();
        write_demo_gif(&dir.join("demo.gif")).unwrap();
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        for (p, page) in cfg.pages.iter().enumerate() {
            let mut sheet = RgbImage::from_pixel(5 * 80 + 8, 3 * 80 + 8, Rgb([30, 30, 30]));
            for k in 1..=15u8 {
                let v = match page.keys.iter().find(|d| d.key == k) {
                    Some(d) => render_key(d, &dir),
                    None => blank(),
                };
                let img = image::load_from_memory(&v.frames[0].jpeg).unwrap().to_rgb8();
                let i = (k - 1) as u32;
                image::imageops::overlay(&mut sheet, &img, (8 + (i % 5) * 80) as i64, (8 + (i / 5) * 80) as i64);
            }
            sheet.save(format!("/tmp/hubforge-page{}.png", p + 1)).unwrap();
        }
    }
}

#[cfg(test)]
mod anim_tests {
    use super::*;

    #[test]
    fn builtin_animations_render() {
        for (id, _, _) in crate::anims::ANIMS {
            let k: crate::config::KeyDef = toml::from_str(&format!("key = 1\nicon = \"anim:{id}\"")).unwrap();
            let v = render_key(&k, Path::new("/tmp"));
            assert!(v.frames.len() > 1, "{id} n'est pas animée");
            let (_, animated) = preview(&k, Path::new("/tmp"));
            assert!(animated);
        }
    }
}
