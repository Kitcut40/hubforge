//! Thèmes des touches : Graphite, Néon, Plan, Console, Mono (et Classique,
//! l'ancien rendu).
//!
//! Une touche thématisée est dessinée comme un petit SVG de 72×72 (fond,
//! icône, texte), puis rendu par resvg. La couleur choisie pour la touche sert
//! de couleur d'accent : icône verte en Graphite, halo vert en Néon, témoin
//! vert en Console… Les touches qui affichent une image ou un GIF de
//! l'utilisateur ne sont pas thématisées.
//!
//! Le thème d'une touche vient, dans l'ordre : de la touche, de sa page, puis
//! du profil (`general.theme`).

use crate::config::{parse_color, KeyDef};
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use hubforge_device::KEY_IMAGE_SIZE as S;
use image::{Rgb, RgbImage};
use std::sync::{Arc, OnceLock};

/// Thèmes disponibles : (identifiant, nom affiché).
pub const THEMES: &[(&str, &str)] = &[
    ("graphite", "Graphite"),
    ("neon", "Néon"),
    ("plan", "Plan"),
    ("console", "Console"),
    ("mono", "Mono"),
    ("classic", "Classique"),
];

/// Thème des nouveaux profils.
pub const DEFAULT: &str = "graphite";

pub fn is_known(t: &str) -> bool {
    THEMES.iter().any(|(id, _)| *id == t)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Graphite,
    Neon,
    Plan,
    Console,
    Mono,
}

impl Theme {
    /// None pour « classic » (ancien rendu) ou un thème inconnu.
    pub fn parse(t: &str) -> Option<Theme> {
        match t {
            "graphite" => Some(Theme::Graphite),
            "neon" => Some(Theme::Neon),
            "plan" => Some(Theme::Plan),
            "console" => Some(Theme::Console),
            "mono" => Some(Theme::Mono),
            _ => None,
        }
    }
}

/// Thème d'une touche déjà résolu (voir `config::Config::themed`).
pub fn of(def: &KeyDef) -> Option<Theme> {
    Theme::parse(def.theme.as_deref().unwrap_or(DEFAULT))
}

// ----------------------------------------------------------------------
// Couleurs
// ----------------------------------------------------------------------

fn hex(c: (u8, u8, u8)) -> String {
    format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

/// Mélange vers le blanc (f > 0) ou vers le noir (f < 0).
pub fn shade(c: (u8, u8, u8), f: f32) -> (u8, u8, u8) {
    let m = |v: u8| -> u8 {
        let v = v as f32;
        (if f >= 0.0 { v + (255.0 - v) * f } else { v * (1.0 + f) }).round().clamp(0.0, 255.0) as u8
    };
    (m(c.0), m(c.1), m(c.2))
}

fn rgb_to_hsl((r, g, b): (u8, u8, u8)) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    if mx == mn {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let f = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (f(r), f(g), f(b))
}

/// Couleur d'accent par défaut selon l'icône (quand la touche n'a pas de
/// couleur, ou une couleur trop sombre ou grise pour servir d'accent).
fn default_accent(icon: &str) -> (u8, u8, u8) {
    let name = icon.strip_prefix("builtin:").unwrap_or(icon);
    let c = match name {
        "mic" | "mic-off" | "check" | "battery" | "activity" => "#2ECC71",
        "lock" | "power" | "x" | "trash-2" | "circle-dot" | "heart" | "flame" => "#EF4444",
        "folder" | "folder-open" | "star" | "zap" | "sun" | "sparkles" => "#F5B700",
        "copy" | "clipboard-paste" | "scissors" | "file" | "file-cog" | "coffee" => "#FF8A3D",
        "layers" | "box" | "code" | "terminal" | "git-branch" | "palette" | "brush" | "pen-tool" => "#A35BFF",
        "arrow-right" | "arrow-left" | "house" | "undo-2" | "redo-2" | "rotate-ccw" | "settings" => "#94A3B8",
        "gamepad-2" | "rocket" | "target" | "crosshair" => "#FF4FA3",
        "music" | "headphones" | "play" | "pause" | "skip-forward" | "skip-back" => "#1DB954",
        _ => "#3DA9FC",
    };
    parse_color(c).unwrap_or((61, 169, 252))
}

/// Couleur d'accent d'une touche : sa couleur, rendue assez vive et lumineuse
/// pour ressortir sur un fond sombre.
pub fn accent(def: &KeyDef, fallback_icon: &str) -> (u8, u8, u8) {
    accent_with(def, default_accent(fallback_icon))
}

/// Comme `accent`, avec la couleur à prendre si la touche n'en a pas d'utile.
pub fn accent_with(def: &KeyDef, fallback: (u8, u8, u8)) -> (u8, u8, u8) {
    let Some(c) = def.color.as_deref().and_then(parse_color) else { return fallback };
    let (h, s, l) = rgb_to_hsl(c);
    if s < 0.18 || (l < 0.16 && s < 0.5) {
        // Gris ou presque noir : la couleur ne dit rien, accent par défaut.
        return fallback;
    }
    hsl_to_rgb(h, s.max(0.55), l.clamp(0.55, 0.68))
}

pub fn hex_of(c: (u8, u8, u8)) -> String {
    hex(c)
}

// ----------------------------------------------------------------------
// Styles des thèmes
// ----------------------------------------------------------------------

/// Épaisseur d'icône : indice dans `theme_icons::THEME_ICONS`.
#[derive(Clone, Copy, PartialEq)]
pub enum Weight {
    Fill = 0,
    Bold = 1,
    Light = 2,
    Thin = 3,
}

const CREAM: (u8, u8, u8) = (0xF4, 0xE6, 0xC8);

/// Réglages de dessin d'un thème pour une couleur d'accent.
pub struct Style {
    pub theme: Theme,
    pub accent: (u8, u8, u8),
    pub weight: Weight,
    /// Couleur des icônes.
    pub ic: String,
    /// Texte principal, texte secondaire, valeurs.
    pub fg: String,
    pub soft: String,
    pub val: String,
    /// Couleur des barres, jauges et courbes.
    pub line: String,
    /// Opacité du rail des barres et jauges.
    pub track_op: f32,
    pub upper: bool,
    pub spacing: f32,
    pub label_size: f32,
    pub label_weight: u16,
    pub value_weight: u16,
    /// Épaisseur des jauges.
    pub stroke: f32,
    /// Ligne de base du titre.
    pub label_y: f32,
}

pub fn style(theme: Theme, accent: (u8, u8, u8)) -> Style {
    let a = hex(accent);
    match theme {
        Theme::Graphite => Style {
            theme,
            accent,
            weight: Weight::Fill,
            ic: a.clone(),
            fg: "#F2F4F8".into(),
            soft: "#9AA3B2".into(),
            val: "#FFFFFF".into(),
            line: a,
            track_op: 0.12,
            upper: false,
            spacing: 0.0,
            label_size: 11.0,
            label_weight: 600,
            value_weight: 700,
            stroke: 5.0,
            label_y: 64.0,
        },
        Theme::Neon => Style {
            theme,
            accent,
            weight: Weight::Fill,
            ic: a.clone(),
            fg: hex(shade(accent, 0.55)),
            soft: hex(shade(accent, 0.25)),
            val: "#FFFFFF".into(),
            line: a,
            track_op: 0.12,
            upper: false,
            spacing: 0.0,
            label_size: 11.0,
            label_weight: 600,
            value_weight: 700,
            stroke: 5.0,
            label_y: 63.0,
        },
        Theme::Plan => Style {
            theme,
            accent,
            weight: Weight::Light,
            ic: "#FFFFFF".into(),
            fg: "#FFFFFF".into(),
            soft: "#BFD3F2".into(),
            val: "#FFFFFF".into(),
            line: "#FFFFFF".into(),
            track_op: 0.22,
            upper: true,
            spacing: 0.8,
            label_size: 8.5,
            label_weight: 600,
            value_weight: 700,
            stroke: 2.2,
            label_y: 64.0,
        },
        Theme::Console => Style {
            theme,
            accent,
            weight: Weight::Bold,
            ic: hex(CREAM),
            fg: hex(CREAM),
            soft: "#B9AE97".into(),
            val: hex(CREAM),
            line: a,
            track_op: 0.15,
            upper: true,
            spacing: 0.6,
            label_size: 8.0,
            label_weight: 700,
            value_weight: 700,
            stroke: 4.5,
            label_y: 62.0,
        },
        Theme::Mono => Style {
            theme,
            accent,
            weight: Weight::Thin,
            ic: "#FFFFFF".into(),
            fg: "#E8E8E8".into(),
            soft: "#8A8A8A".into(),
            val: "#FFFFFF".into(),
            line: "#FFFFFF".into(),
            track_op: 0.16,
            upper: true,
            spacing: 1.2,
            label_size: 8.5,
            label_weight: 400,
            value_weight: 400,
            stroke: 2.0,
            label_y: 64.0,
        },
    }
}

/// Fond, cadre et filtres du thème (début du SVG, avant le contenu).
pub fn background(st: &Style) -> String {
    let a = hex(st.accent);
    let glow = r#"<filter id="glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="2.4" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>"#;
    let soft_glow = r#"<filter id="glow2" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="1.5" result="b"/><feMerge><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge></filter>"#;
    match st.theme {
        Theme::Graphite => format!(
            r##"<defs>{glow}{soft_glow}<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#30343D"/><stop offset="1" stop-color="#16181D"/></linearGradient></defs><rect width="72" height="72" fill="url(#bg)"/><rect width="72" height="1" fill="#FFFFFF" fill-opacity="0.10"/>"##
        ),
        Theme::Neon => format!(
            r##"<defs>{glow}{soft_glow}<radialGradient id="bg" cx="0.5" cy="0.38" r="0.75"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="#050608"/></radialGradient></defs><rect width="72" height="72" fill="url(#bg)"/><rect x="2.5" y="2.5" width="67" height="67" rx="9" fill="none" stroke="{a}" stroke-opacity="0.55" stroke-width="1.5"/>"##,
            hex(shade(st.accent, -0.78))
        ),
        Theme::Plan => {
            let mut grid = String::new();
            for x in (0..=72).step_by(6) {
                let op = if x % 18 == 0 { 0.16 } else { 0.07 };
                grid += &format!(
                    r##"<line x1="{x}" y1="0" x2="{x}" y2="72" stroke="#FFFFFF" stroke-opacity="{op}" stroke-width="0.6"/><line x1="0" y1="{x}" x2="72" y2="{x}" stroke="#FFFFFF" stroke-opacity="{op}" stroke-width="0.6"/>"##
                );
            }
            format!(
                r##"<defs>{glow}{soft_glow}<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#245A9E"/><stop offset="1" stop-color="#173C6E"/></linearGradient></defs><rect width="72" height="72" fill="url(#bg)"/>{grid}<rect x="4" y="4" width="6" height="6" fill="{a}"/><rect x="1.5" y="1.5" width="69" height="69" fill="none" stroke="#FFFFFF" stroke-opacity="0.5" stroke-width="0.8"/>"##
            )
        }
        Theme::Console => format!(
            r##"<defs>{glow}{soft_glow}<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3A3B3F"/><stop offset="1" stop-color="#232427"/></linearGradient><radialGradient id="lit" cx="0.5" cy="0.45" r="0.6"><stop offset="0" stop-color="#4A4335"/><stop offset="1" stop-color="#2A2A2C"/></radialGradient></defs><rect width="72" height="72" fill="#141517"/><rect x="3" y="3" width="66" height="66" rx="6" fill="url(#bg)"/><rect x="6" y="6" width="60" height="60" rx="4" fill="url(#lit)" stroke="#000000" stroke-opacity="0.6"/><rect x="6.5" y="6.5" width="59" height="1" fill="#FFFFFF" fill-opacity="0.12"/><rect x="24" y="6.5" width="24" height="6.4" rx="3" fill="{a}" fill-opacity="0.25"/><rect x="26" y="8.5" width="20" height="2.4" rx="1.2" fill="{a}"/>"##
        ),
        Theme::Mono => format!(r##"<defs>{glow}{soft_glow}</defs><rect width="72" height="72" fill="#000000"/>"##),
    }
}

/// Filtre à appliquer aux icônes et courbes (halo en Néon et Console).
pub fn fx(st: &Style) -> &'static str {
    match st.theme {
        Theme::Neon => r#"filter="url(#glow)""#,
        Theme::Console => r#"filter="url(#glow2)""#,
        _ => "",
    }
}

pub fn svg_doc(st: &Style, body: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">{}{body}</svg>"#, background(st))
}

// ----------------------------------------------------------------------
// Icônes
// ----------------------------------------------------------------------

fn inner(svg: &str) -> &str {
    let start = svg.find('>').map(|i| i + 1).unwrap_or(0);
    let end = svg.rfind("</svg>").unwrap_or(svg.len());
    &svg[start..end.max(start)]
}

/// SVG d'une icône de thème (Phosphor) de cette épaisseur, ou None.
pub fn theme_icon(name: &str, w: Weight) -> Option<&'static str> {
    crate::theme_icons::THEME_ICONS.iter().find(|(n, _)| *n == name).map(|(_, v)| v[w as usize])
}

/// Icône `name` centrée en (`cx`, `cy`), de `size` pixels. Icône de thème si
/// elle existe, sinon l'icône intégrée à traits (Lucide), sinon rien.
pub fn icon(name: &str, w: Weight, cx: f32, cy: f32, size: f32, color: &str, extra: &str) -> String {
    if let Some(svg) = theme_icon(name, w) {
        let k = size / 256.0;
        return format!(
            r#"<g transform="translate({:.2} {:.2}) scale({k:.4})" fill="{color}" {extra}>{}</g>"#,
            cx - size / 2.0,
            cy - size / 2.0,
            inner(svg).replace("currentColor", color)
        );
    }
    if let Some(svg) = crate::render::builtin_svg(name) {
        let k = size / 24.0;
        let sw = match w {
            Weight::Fill | Weight::Bold => 2.4,
            Weight::Light => 1.5,
            Weight::Thin => 1.1,
        };
        return format!(
            r#"<g transform="translate({:.2} {:.2}) scale({k:.4})" fill="none" stroke="{color}" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round" {extra}>{}</g>"#,
            cx - size / 2.0,
            cy - size / 2.0,
            inner(svg).replace("currentColor", color).replace("stroke-width=\"2\"", "")
        );
    }
    String::new()
}

// ----------------------------------------------------------------------
// Texte
// ----------------------------------------------------------------------

const REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.otf");
const SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.otf");
const BOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-Bold.otf");

fn measure_font(weight: u16) -> &'static FontRef<'static> {
    static R: OnceLock<FontRef<'static>> = OnceLock::new();
    static M: OnceLock<FontRef<'static>> = OnceLock::new();
    static B: OnceLock<FontRef<'static>> = OnceLock::new();
    match weight {
        0..=450 => R.get_or_init(|| FontRef::try_from_slice(REGULAR).expect("police")),
        451..=650 => M.get_or_init(|| FontRef::try_from_slice(SEMIBOLD).expect("police")),
        _ => B.get_or_init(|| FontRef::try_from_slice(BOLD).expect("police")),
    }
}

/// Largeur d'un texte en pixels.
pub fn text_width(s: &str, size: f32, weight: u16, spacing: f32) -> f32 {
    let f = measure_font(weight).as_scaled(PxScale::from(size));
    let n = s.chars().count() as f32;
    s.chars().map(|c| f.h_advance(f.glyph_id(c))).sum::<f32>() + spacing * (n - 1.0).max(0.0)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Texte centré en `x`, ligne de base `y`. La taille diminue (jusqu'à 7 px)
/// pour tenir dans `max_w`.
#[allow(clippy::too_many_arguments)]
pub fn text(x: f32, y: f32, s: &str, size: f32, color: &str, weight: u16, spacing: f32, max_w: f32, opacity: f32) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let mut size = size;
    while size > 7.0 && text_width(s, size, weight, spacing) > max_w {
        size -= 0.5;
    }
    format!(
        r#"<text x="{x:.1}" y="{y:.1}" font-family="Inter" font-weight="{weight}" font-size="{size:.1}" fill="{color}" fill-opacity="{opacity}" text-anchor="middle" letter-spacing="{spacing}">{}</text>"#,
        esc(s)
    )
}

/// Titre d'une touche selon le thème (majuscules, espacement…).
pub fn label(st: &Style, s: &str, y: f32) -> String {
    let s = if st.upper { s.to_uppercase() } else { s.to_string() };
    text(36.0, y, &s, st.label_size, &st.fg, st.label_weight, st.spacing, 64.0, 0.95)
}

/// Découpe un texte en 1 à 3 lignes qui tiennent dans `max_w` à cette taille.
fn wrap(s: &str, size: f32, weight: u16, max_w: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        match lines.last_mut() {
            Some(l) if text_width(&format!("{l} {word}"), size, weight, 0.0) <= max_w => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

// ----------------------------------------------------------------------
// Rendu SVG → image
// ----------------------------------------------------------------------

fn options() -> &'static resvg::usvg::Options<'static> {
    static O: OnceLock<resvg::usvg::Options<'static>> = OnceLock::new();
    O.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        for f in [REGULAR, SEMIBOLD, BOLD] {
            db.load_font_data(f.to_vec());
        }
        let mut o = resvg::usvg::Options { font_family: "Inter".into(), ..Default::default() };
        o.fontdb = Arc::new(db);
        o
    })
}

/// Rend un SVG de 72×72 en image.
pub fn rasterize(svg: &str) -> RgbImage {
    let mut img = RgbImage::from_pixel(S, S, Rgb([0, 0, 0]));
    let tree = match resvg::usvg::Tree::from_str(svg, options()) {
        Ok(t) => t,
        Err(e) => {
            log::warn!("thème : SVG invalide ({e})");
            return img;
        }
    };
    let Some(mut pm) = resvg::tiny_skia::Pixmap::new(S, S) else { return img };
    let ts = tree.size();
    let scale = S as f32 / ts.width().max(ts.height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    for (i, px) in pm.pixels().iter().enumerate() {
        let c = px.demultiply();
        let a = px.alpha() as u32;
        let p = img.get_pixel_mut(i as u32 % S, i as u32 / S);
        p.0 = [(c.red() as u32 * a / 255) as u8, (c.green() as u32 * a / 255) as u8, (c.blue() as u32 * a / 255) as u8];
    }
    img
}

// ----------------------------------------------------------------------
// Touches d'action
// ----------------------------------------------------------------------

/// Dessine une touche d'action thématisée : icône intégrée et/ou texte.
/// `addon_svg` : icône SVG d'un addon (dessinée à la couleur de l'icône).
pub fn render_key(theme: Theme, def: &KeyDef, addon_svg: Option<&str>) -> RgbImage {
    let icon_name = def.icon.as_deref().and_then(|i| i.strip_prefix("builtin:")).unwrap_or("");
    let st = style(theme, accent(def, icon_name));
    let label_text = def.label.trim();
    let has_icon = !icon_name.is_empty() || addon_svg.is_some();
    let fx = fx(&st);
    let mut body = String::new();

    if has_icon {
        let (cy, size) = match (label_text.is_empty(), theme) {
            (true, _) => (36.0 + if theme == Theme::Console { 3.0 } else { 0.0 }, 38.0),
            (false, Theme::Console) => (32.0, 28.0),
            (false, _) => (29.0, 32.0),
        };
        if let Some(svg) = addon_svg {
            // Icône d'addon (style à traits) : couleur de l'icône du thème.
            let k = size / 24.0;
            body += &format!(
                r#"<g transform="translate({:.2} {:.2}) scale({k:.4})" color="{c}" {fx}>{}</g>"#,
                36.0 - size / 2.0,
                cy - size / 2.0,
                inner(svg).replace("currentColor", &st.ic),
                c = st.ic
            );
        } else {
            body += &icon(icon_name, st.weight, 36.0, cy, size, &st.ic, fx);
        }
        body += &label(&st, label_text, st.label_y);
    } else if !label_text.is_empty() {
        // Texte seul : grand et centré, sur 1 à 3 lignes.
        let shown = if st.upper { label_text.to_uppercase() } else { label_text.to_string() };
        let w = if theme == Theme::Mono { 500 } else { 700 };
        let max_w = if theme == Theme::Console { 54.0 } else { 62.0 };
        let mut size = 18.0;
        let mut lines = wrap(&shown, size, w, max_w);
        while size > 9.0 && (lines.len() > 3 || lines.iter().any(|l| text_width(l, size, w, 0.0) > max_w)) {
            size -= 1.0;
            lines = wrap(&shown, size, w, max_w);
        }
        let lh = size * 1.15;
        let mid = if theme == Theme::Console { 39.0 } else { 37.0 };
        let y0 = mid - lh * (lines.len() as f32 - 1.0) / 2.0 + size * 0.36;
        for (i, l) in lines.iter().enumerate() {
            body += &text(36.0, y0 + i as f32 * lh, l, size, &st.val, w, 0.0, max_w + 6.0, 1.0);
        }
    }
    rasterize(&svg_doc(&st, &body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Action;

    pub fn key(icon: Option<&str>, label: &str, color: Option<&str>, theme: &str) -> KeyDef {
        KeyDef {
            key: 1,
            label: label.into(),
            icon: icon.map(String::from),
            color: color.map(String::from),
            text_color: None,
            action: Action::None,
            toggle: None,
            widget: None,
            widget_style: None,
            widget_target: None,
            long: None,
            double: None,
            theme: Some(theme.into()),
        }
    }

    #[test]
    fn accents_are_bright_enough() {
        let k = key(Some("builtin:mic"), "Micro", Some("#1F2229"), "graphite");
        assert_eq!(accent(&k, "mic"), parse_color("#2ECC71").unwrap());
        let k = key(Some("builtin:copy"), "Copier", Some("#5D4037"), "graphite");
        let (_, _, l) = rgb_to_hsl(accent(&k, "copy"));
        assert!(l >= 0.54);
    }

    #[test]
    fn every_theme_renders() {
        for (id, _) in THEMES {
            let Some(t) = Theme::parse(id) else { continue };
            for k in [
                key(Some("builtin:copy"), "Copier", Some("#FF8A3D"), id),
                key(Some("builtin:mic"), "", None, id),
                key(None, "Un texte assez long sur la touche", Some("#5865F2"), id),
                key(Some("builtin:inconnue"), "?", None, id),
            ] {
                let img = render_key(t, &k, None);
                assert_eq!(img.dimensions(), (S, S));
                // Pas une image vide.
                assert!(img.pixels().any(|p| p.0.iter().any(|c| *c > 60)));
            }
        }
    }

    #[test]
    fn every_builtin_icon_has_a_theme_icon() {
        for (name, _) in crate::icons_data::ICONS {
            assert!(theme_icon(name, Weight::Fill).is_some(), "pas d'icône de thème pour {name}");
        }
    }
}
