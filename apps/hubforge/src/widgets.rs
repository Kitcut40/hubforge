//! Touches vivantes : l'image de la touche est redessinée en continu
//! (horloge, date, processeur, mémoire, volume, micro), avec plusieurs
//! styles chacune (horloge à aiguilles, calendrier, jauge, graphique…).

use crate::audio::AudioState;
use crate::config::{parse_color, KeyDef};
use crate::render::{blend, draw_builtin_at, draw_svg, draw_text_sized};
use hubforge_device::KEY_IMAGE_SIZE as S;
use image::{Rgb, RgbImage, Rgba};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

/// Touches vivantes disponibles : (identifiant, nom affiché).
pub const WIDGETS: &[(&str, &str)] = &[
    ("clock", "Horloge"),
    ("date", "Date"),
    ("cpu", "Processeur (CPU)"),
    ("ram", "Mémoire (RAM)"),
    ("volume", "Volume général"),
    ("mic", "État du micro"),
];

/// Styles de chaque touche vivante : (widget, style, nom). Le premier est
/// le style par défaut.
pub const STYLES: &[(&str, &str, &str)] = &[
    ("clock", "digital", "Numérique"),
    ("clock", "analog", "À aiguilles"),
    ("clock", "big", "Grands chiffres"),
    ("date", "classic", "Classique"),
    ("date", "calendar", "Page de calendrier"),
    ("cpu", "bar", "Barre"),
    ("cpu", "gauge", "Jauge"),
    ("cpu", "graph", "Graphique"),
    ("ram", "bar", "Barre"),
    ("ram", "gauge", "Jauge"),
    ("ram", "graph", "Graphique"),
    ("volume", "bar", "Barre"),
    ("volume", "gauge", "Jauge"),
    ("mic", "classic", "Icône et texte"),
    ("mic", "icon", "Grande icône"),
];

pub fn is_known(kind: &str) -> bool {
    WIDGETS.iter().any(|(k, _)| *k == kind)
}

/// Style effectif (celui demandé s'il existe, sinon le style par défaut).
fn style_of(def: &KeyDef) -> &'static str {
    let kind = def.widget.as_deref().unwrap_or("");
    let wanted = def.widget_style.as_deref().unwrap_or("");
    STYLES
        .iter()
        .find(|(k, s, _)| *k == kind && *s == wanted)
        .or_else(|| STYLES.iter().find(|(k, _, _)| *k == kind))
        .map(|(_, s, _)| *s)
        .unwrap_or("")
}

const DAYS_SHORT: [&str; 7] = ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."];
const DAYS: [&str; 7] = ["Dimanche", "Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi"];
const MONTHS_SHORT: [&str; 12] =
    ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."];
const MONTHS: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre",
    "décembre",
];
/// Nombre de mesures gardées pour les graphiques (une toutes les 500 ms).
const HISTORY: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LocalTime {
    pub year: u16,
    pub month: u8, // 1–12
    pub day: u8,
    pub dow: u8, // 0 = dimanche
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// Mesures du moment.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    pub time: LocalTime,
    /// Charge du processeur en %, None à la première mesure.
    pub cpu: Option<u8>,
    /// Mémoire : (% utilisé, Go utilisés, Go au total).
    pub ram: Option<(u8, f32, f32)>,
    pub audio: AudioState,
    /// Dernières mesures (la plus ancienne d'abord), pour les graphiques.
    pub cpu_hist: Vec<u8>,
    pub ram_hist: Vec<u8>,
}

/// Prend les mesures ; garde la mesure précédente du processeur pour
/// calculer sa charge entre deux appels, et un historique.
#[derive(Default)]
pub struct Sampler {
    cpu_last: Option<(u64, u64)>,
    cpu: Option<u8>,
    cpu_hist: VecDeque<u8>,
    ram_hist: VecDeque<u8>,
}

impl Sampler {
    pub fn sample(&mut self, audio: AudioState) -> Snapshot {
        if let Some((idle, total)) = imp::cpu_times() {
            if let Some((li, lt)) = self.cpu_last {
                let dt = total.saturating_sub(lt);
                let di = idle.saturating_sub(li);
                if dt > 0 {
                    self.cpu = Some((100 * dt.saturating_sub(di) / dt).min(100) as u8);
                }
            }
            self.cpu_last = Some((idle, total));
        }
        let ram = imp::memory();
        let push = |h: &mut VecDeque<u8>, v: Option<u8>| {
            if let Some(v) = v {
                if h.len() == HISTORY {
                    h.pop_front();
                }
                h.push_back(v);
            }
        };
        push(&mut self.cpu_hist, self.cpu);
        push(&mut self.ram_hist, ram.map(|r| r.0));
        Snapshot {
            time: imp::local_time(),
            cpu: self.cpu,
            ram,
            audio,
            cpu_hist: self.cpu_hist.iter().copied().collect(),
            ram_hist: self.ram_hist.iter().copied().collect(),
        }
    }
}

/// Mesures partagées pour les aperçus de l'interface.
pub fn preview_snapshot(audio: AudioState) -> Snapshot {
    static SAMPLER: OnceLock<Mutex<Sampler>> = OnceLock::new();
    let m = SAMPLER.get_or_init(|| Mutex::new(Sampler::default()));
    m.lock().map(|mut s| s.sample(audio)).unwrap_or_default()
}

/// Ce que la touche affiche, sous forme de texte : on ne renvoie l'image au
/// module que si ceci change.
pub fn signature(def: &KeyDef, snap: &Snapshot) -> String {
    let t = &snap.time;
    let kind = def.widget.as_deref().unwrap_or("");
    let style = style_of(def);
    let content = match (kind, style) {
        ("clock", "analog") => format!("{:02}{:02}{:02}", t.hour, t.minute, t.second),
        ("clock", _) => format!("{:02}{:02}{}", t.hour, t.minute, t.day),
        ("date", _) => format!("{}{}{}", t.year, t.month, t.day),
        ("cpu", "graph") => format!("{:?}{:?}", snap.cpu, snap.cpu_hist),
        ("cpu", _) => format!("{:?}", snap.cpu),
        ("ram", "graph") => format!("{:?}{:?}", snap.ram.map(|r| r.0), snap.ram_hist),
        ("ram", _) => format!("{:?}", snap.ram.map(|(p, u, _)| (p, (u * 10.0) as u32))),
        ("volume", _) => format!("{:?}{}", snap.audio.master.map(|v| (v * 100.0).round() as u32), snap.audio.master_muted),
        ("mic", _) => format!("{:?}", snap.audio.mic_muted),
        _ => String::new(),
    };
    format!("{kind}|{style}|{content}|{}|{:?}|{:?}", def.label, def.color, def.text_color)
}

// ----------------------------------------------------------------------
// Outils de dessin
// ----------------------------------------------------------------------

fn color_or(c: &Option<String>, default: (u8, u8, u8)) -> Rgb<u8> {
    let (r, g, b) = c.as_deref().and_then(parse_color).unwrap_or(default);
    Rgb([r, g, b])
}

fn hex(c: Rgb<u8>) -> String {
    format!("#{:02X}{:02X}{:02X}", c.0[0], c.0[1], c.0[2])
}

/// Mélange deux couleurs (t = part de `a`).
fn mix(a: Rgb<u8>, b: Rgb<u8>, t: f32) -> Rgb<u8> {
    let m = |x: u8, y: u8| (x as f32 * t + y as f32 * (1.0 - t)).round() as u8;
    Rgb([m(a.0[0], b.0[0]), m(a.0[1], b.0[1]), m(a.0[2], b.0[2])])
}

fn rect(img: &mut RgbImage, x0: u32, y0: u32, w: u32, h: u32, c: Rgb<u8>) {
    for y in y0..(y0 + h).min(S) {
        for x in x0..(x0 + w).min(S) {
            blend(img, x, y, Rgba([c.0[0], c.0[1], c.0[2], 255]));
        }
    }
}

/// Barre de progression (fraction 0–1).
fn bar(img: &mut RgbImage, y: u32, frac: f32, fill: Rgb<u8>, track: Rgb<u8>) {
    let (x0, w, h) = (9u32, S - 18, 5u32);
    rect(img, x0, y, w, h, track);
    let fw = (w as f32 * frac.clamp(0.0, 1.0)).round() as u32;
    if fw > 0 {
        rect(img, x0, y, fw, h, fill);
    }
}

/// Vert, puis orange, puis rouge selon la charge.
fn load_color(pct: u8) -> Rgb<u8> {
    match pct {
        0..=59 => Rgb([61, 220, 132]),
        60..=84 => Rgb([255, 179, 0]),
        _ => Rgb([255, 59, 48]),
    }
}

const VOLUME_BLUE: Rgb<u8> = Rgb([0, 200, 255]);

fn fr_number(v: f32) -> String {
    format!("{v:.1}").replace('.', ",")
}

/// Point sur un cercle (angle en degrés, 0 = droite, sens horaire).
fn polar(cx: f32, cy: f32, r: f32, deg: f32) -> (f32, f32) {
    let a = deg.to_radians();
    (cx + r * a.cos(), cy + r * a.sin())
}

/// Arc de cercle SVG de `from` à `to` degrés (sens horaire).
fn arc_path(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> String {
    let (x1, y1) = polar(cx, cy, r, from);
    let (x2, y2) = polar(cx, cy, r, to);
    let large = if to - from > 180.0 { 1 } else { 0 };
    format!("M{x1:.2} {y1:.2} A{r} {r} 0 {large} 1 {x2:.2} {y2:.2}")
}

/// Jauge circulaire (270°, ouverte en bas).
fn gauge(img: &mut RgbImage, frac: f32, fill: Rgb<u8>, track: Rgb<u8>) {
    let (cx, cy, r) = (36.0, 34.0, 25.0);
    let start = 135.0;
    let end = start + 270.0 * frac.clamp(0.0, 1.0);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72"><path d="{}" stroke="{}" stroke-width="7" fill="none" stroke-linecap="round"/>"#,
        arc_path(cx, cy, r, start, start + 270.0),
        hex(track)
    );
    if frac > 0.005 {
        svg += &format!(
            r#"<path d="{}" stroke="{}" stroke-width="7" fill="none" stroke-linecap="round"/>"#,
            arc_path(cx, cy, r, start, end.max(start + 1.0)),
            hex(fill)
        );
    }
    svg += "</svg>";
    draw_svg(img, &svg);
}

/// Graphique de l'historique (zone remplie + ligne), en bas de la touche.
fn graph(img: &mut RgbImage, hist: &[u8], color: Rgb<u8>, grid: Rgb<u8>) {
    let (x0, x1, y0, y1) = (4.0f32, 68.0f32, 24.0f32, 68.0f32);
    let n = HISTORY.max(2) as f32 - 1.0;
    let pts: Vec<(f32, f32)> = hist
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = x1 - (hist.len() - 1 - i) as f32 * (x1 - x0) / n;
            let y = y1 - (*v as f32 / 100.0) * (y1 - y0);
            (x, y)
        })
        .collect();
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72"><path d="M{x0} {y0} H{x1} M{x0} {ym} H{x1} M{x0} {y1} H{x1}" stroke="{g}" stroke-width="1" stroke-dasharray="2 3"/>"#,
        ym = (y0 + y1) / 2.0,
        g = hex(grid)
    );
    if pts.len() >= 2 {
        let line: String = pts.iter().map(|(x, y)| format!("{x:.1},{y:.1} ")).collect();
        let first = pts[0];
        let last = pts[pts.len() - 1];
        svg += &format!(
            r#"<polygon points="{first_x:.1},{y1} {line}{last_x:.1},{y1}" fill="{c}" fill-opacity="0.28"/><polyline points="{line}" stroke="{c}" stroke-width="2" fill="none" stroke-linejoin="round"/><circle cx="{lx:.1}" cy="{ly:.1}" r="2.6" fill="{c}"/>"#,
            first_x = first.0,
            last_x = last.0,
            c = hex(color),
            lx = last.0,
            ly = last.1
        );
    }
    svg += "</svg>";
    draw_svg(img, &svg);
}

// ----------------------------------------------------------------------
// Rendu
// ----------------------------------------------------------------------

/// Dessine la touche vivante.
pub fn render(def: &KeyDef, snap: &Snapshot) -> RgbImage {
    let kind = def.widget.as_deref().unwrap_or("");
    let style = style_of(def);
    let fg = color_or(&def.text_color, (255, 255, 255));
    let label = def.label.trim();
    let t = &snap.time;
    let dark = (0x1F, 0x22, 0x29);

    match kind {
        "clock" => {
            let bg = color_or(&def.color, dark);
            let mut img = RgbImage::from_pixel(S, S, bg);
            match style {
                "analog" => draw_analog(&mut img, t, fg, bg),
                "big" => {
                    draw_text_sized(&mut img, &format!("{:02}", t.hour), fg, 2, 36, 34);
                    draw_text_sized(&mut img, &format!("{:02}", t.minute), Rgb([0xFF, 0x7A, 0x1A]), 34, 36, 34);
                }
                _ => {
                    draw_text_sized(&mut img, &format!("{:02}:{:02}", t.hour, t.minute), fg, 10, 32, 26);
                    let sub = if label.is_empty() {
                        format!("{} {} {}", DAYS_SHORT[t.dow as usize % 7], t.day, MONTHS_SHORT[(t.month as usize + 11) % 12])
                    } else {
                        label.to_string()
                    };
                    draw_text_sized(&mut img, &sub, mix(fg, bg, 0.65), 44, 20, 11);
                }
            }
            img
        }
        "date" => {
            if style == "calendar" {
                let bg = color_or(&def.color, dark);
                let mut img = RgbImage::from_pixel(S, S, bg);
                draw_svg(
                    &mut img,
                    r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">
  <rect x="7" y="8" width="58" height="58" rx="8" fill="#F5F5F7"/>
  <path d="M7 16 a8 8 0 0 1 8 -8 h42 a8 8 0 0 1 8 8 v8 h-58 z" fill="#E53935"/>
  <rect x="19" y="4" width="4" height="9" rx="2" fill="#B0B0B8"/><rect x="49" y="4" width="4" height="9" rx="2" fill="#B0B0B8"/>
</svg>"##,
                );
                draw_text_sized(&mut img, &MONTHS_SHORT[(t.month as usize + 11) % 12].to_uppercase(), Rgb([255, 255, 255]), 12, 13, 10);
                draw_text_sized(&mut img, &t.day.to_string(), Rgb([0x1F, 0x22, 0x29]), 25, 28, 28);
                let low = if label.is_empty() { DAYS_SHORT[t.dow as usize % 7] } else { label };
                draw_text_sized(&mut img, low, Rgb([0x6B, 0x70, 0x7C]), 52, 13, 10);
                img
            } else {
                let bg = color_or(&def.color, dark);
                let mut img = RgbImage::from_pixel(S, S, bg);
                let top = if label.is_empty() { DAYS[t.dow as usize % 7] } else { label };
                draw_text_sized(&mut img, top, mix(fg, bg, 0.7), 3, 16, 11);
                draw_text_sized(&mut img, &t.day.to_string(), fg, 17, 34, 30);
                draw_text_sized(&mut img, MONTHS[(t.month as usize + 11) % 12], mix(fg, bg, 0.7), 51, 18, 11);
                img
            }
        }
        "cpu" | "ram" => {
            let bg = color_or(&def.color, dark);
            let mut img = RgbImage::from_pixel(S, S, bg);
            let title = if !label.is_empty() {
                label
            } else if kind == "cpu" {
                "CPU"
            } else {
                "RAM"
            };
            let pct = if kind == "cpu" { snap.cpu } else { snap.ram.map(|r| r.0) };
            let value = pct.map(|p| format!("{p} %")).unwrap_or_else(|| "–".into());
            let color = load_color(pct.unwrap_or(0));
            let track = mix(fg, bg, 0.16);
            match style {
                "gauge" => {
                    gauge(&mut img, pct.unwrap_or(0) as f32 / 100.0, color, track);
                    draw_text_sized(&mut img, &value, fg, 24, 20, 15);
                    draw_text_sized(&mut img, title, mix(fg, bg, 0.65), 55, 15, 10);
                }
                "graph" => {
                    let hist = if kind == "cpu" { &snap.cpu_hist } else { &snap.ram_hist };
                    graph(&mut img, hist, color, mix(fg, bg, 0.14));
                    draw_text_sized(&mut img, &format!("{title} {value}"), fg, 3, 18, 12);
                }
                _ => {
                    draw_text_sized(&mut img, title, mix(fg, bg, 0.65), 3, 16, 11);
                    let sub = if kind == "ram" {
                        snap.ram.map(|(_, u, tot)| format!("{} / {} Go", fr_number(u), tot.round()))
                    } else {
                        None
                    };
                    let has_sub = sub.is_some();
                    draw_text_sized(&mut img, &value, fg, 17, if has_sub { 28 } else { 32 }, 22);
                    let bar_y = if has_sub { 48 } else { 54 };
                    bar(&mut img, bar_y, pct.unwrap_or(0) as f32 / 100.0, color, track);
                    if let Some(sub) = sub {
                        draw_text_sized(&mut img, &sub, mix(fg, bg, 0.6), 55, 16, 9);
                    }
                }
            }
            img
        }
        "volume" => {
            let bg = color_or(&def.color, dark);
            let mut img = RgbImage::from_pixel(S, S, bg);
            let a = &snap.audio;
            let muted = a.master_muted || a.master.is_none();
            let v = a.master.unwrap_or(0.0);
            let icon = if muted {
                "volume-x"
            } else if v < 0.5 {
                "volume-1"
            } else {
                "volume-2"
            };
            let text = if a.master.is_none() {
                "—".to_string()
            } else if a.master_muted {
                "Muet".to_string()
            } else {
                format!("{} %", (v * 100.0).round())
            };
            let fill = if muted { mix(fg, bg, 0.35) } else { VOLUME_BLUE };
            if style == "gauge" {
                gauge(&mut img, v, fill, mix(fg, bg, 0.16));
                draw_builtin_at(&mut img, icon, mix(fg, bg, 0.8), 16, 18);
                draw_text_sized(&mut img, &text, fg, 34, 16, 13);
                if !label.is_empty() {
                    draw_text_sized(&mut img, label, mix(fg, bg, 0.65), 55, 15, 10);
                }
            } else {
                draw_builtin_at(&mut img, icon, fg, 26, 6);
                draw_text_sized(&mut img, &text, fg, 33, 20, 16);
                bar(&mut img, 57, v, fill, mix(fg, bg, 0.18));
            }
            img
        }
        "mic" => {
            let (bg, icon, text) = match snap.audio.mic_muted {
                Some(true) => (Rgb([0xC6, 0x28, 0x28]), "mic-off", "Micro coupé".to_string()),
                Some(false) => (
                    color_or(&def.color, (0x2E, 0x7D, 0x32)),
                    "mic",
                    if label.is_empty() { "Micro".to_string() } else { label.to_string() },
                ),
                None => (Rgb([0x37, 0x47, 0x4F]), "mic-off", "Pas de micro".to_string()),
            };
            let mut img = RgbImage::from_pixel(S, S, bg);
            if style == "icon" {
                // Halo autour de l'icône.
                draw_svg(
                    &mut img,
                    &format!(
                        r#"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72"><circle cx="36" cy="36" r="29" fill="none" stroke="{}" stroke-width="3" stroke-opacity="0.45"/></svg>"#,
                        hex(fg)
                    ),
                );
                draw_builtin_at(&mut img, icon, fg, 38, 17);
            } else {
                draw_builtin_at(&mut img, icon, fg, 34, 8);
                draw_text_sized(&mut img, &text, fg, 44, S - 46, 13);
            }
            img
        }
        _ => {
            let mut img = RgbImage::from_pixel(S, S, Rgb([80, 0, 0]));
            draw_text_sized(&mut img, "Touche vivante inconnue", fg, 0, S, 12);
            img
        }
    }
}

/// Horloge à aiguilles (avec trotteuse orange).
fn draw_analog(img: &mut RgbImage, t: &LocalTime, fg: Rgb<u8>, bg: Rgb<u8>) {
    let (cx, cy) = (36.0, 36.0);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72"><circle cx="{cx}" cy="{cy}" r="31" fill="{face}" stroke="{ring}" stroke-width="2"/>"#,
        face = hex(mix(fg, bg, 0.07)),
        ring = hex(mix(fg, bg, 0.3))
    );
    for i in 0..12 {
        let deg = i as f32 * 30.0 - 90.0;
        let long = i % 3 == 0;
        let (x1, y1) = polar(cx, cy, if long { 22.0 } else { 25.0 }, deg);
        let (x2, y2) = polar(cx, cy, 28.0, deg);
        svg += &format!(
            r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{c}" stroke-width="{w}" stroke-linecap="round"/>"#,
            c = hex(if long { fg } else { mix(fg, bg, 0.5) }),
            w = if long { 2.4 } else { 1.4 }
        );
    }
    let minute = t.minute as f32 + t.second as f32 / 60.0;
    let hour = (t.hour % 12) as f32 + minute / 60.0;
    let hand = |deg: f32, len: f32, w: f32, c: Rgb<u8>| {
        let (x, y) = polar(cx, cy, len, deg - 90.0);
        let (bx, by) = polar(cx, cy, -4.0, deg - 90.0);
        format!(r#"<line x1="{bx:.2}" y1="{by:.2}" x2="{x:.2}" y2="{y:.2}" stroke="{}" stroke-width="{w}" stroke-linecap="round"/>"#, hex(c))
    };
    svg += &hand(hour * 30.0, 15.0, 3.6, fg);
    svg += &hand(minute * 6.0, 23.0, 2.6, fg);
    svg += &hand(t.second as f32 * 6.0, 25.0, 1.2, Rgb([0xFF, 0x7A, 0x1A]));
    svg += &format!(r##"<circle cx="{cx}" cy="{cy}" r="2.6" fill="#FF7A1A"/></svg>"##);
    draw_svg(img, &svg);
}

#[cfg(windows)]
mod imp {
    use super::LocalTime;
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::SystemInformation::{GetLocalTime, GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows::Win32::System::Threading::GetSystemTimes;

    pub fn local_time() -> LocalTime {
        let t = unsafe { GetLocalTime() };
        LocalTime {
            year: t.wYear,
            month: t.wMonth as u8,
            day: t.wDay as u8,
            dow: t.wDayOfWeek as u8,
            hour: t.wHour as u8,
            minute: t.wMinute as u8,
            second: t.wSecond as u8,
        }
    }

    fn ft(f: &FILETIME) -> u64 {
        ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
    }

    /// (temps d'inactivité, temps total) cumulés.
    pub fn cpu_times() -> Option<(u64, u64)> {
        let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
        unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }.ok()?;
        // Le temps « noyau » comprend déjà le temps d'inactivité.
        Some((ft(&idle), ft(&kernel) + ft(&user)))
    }

    pub fn memory() -> Option<(u8, f32, f32)> {
        let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        unsafe { GlobalMemoryStatusEx(&mut m) }.ok()?;
        let gb = 1024.0 * 1024.0 * 1024.0;
        let total = m.ullTotalPhys as f32 / gb;
        let used = (m.ullTotalPhys - m.ullAvailPhys) as f32 / gb;
        Some((m.dwMemoryLoad as u8, used, total))
    }
}

#[cfg(not(windows))]
mod imp {
    use super::LocalTime;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Heure UTC (suffisant hors Windows, en attendant la version Linux).
    pub fn local_time() -> LocalTime {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let days = (secs / 86_400) as i64;
        // Algorithme de Howard Hinnant : jours depuis 1970 → date.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = (yoe + era * 400 + if month <= 2 { 1 } else { 0 }) as u16;
        LocalTime {
            year,
            month,
            day,
            dow: ((days + 4).rem_euclid(7)) as u8,
            hour: ((secs / 3600) % 24) as u8,
            minute: ((secs / 60) % 60) as u8,
            second: (secs % 60) as u8,
        }
    }

    pub fn cpu_times() -> Option<(u64, u64)> {
        let text = std::fs::read_to_string("/proc/stat").ok()?;
        let line = text.lines().next()?;
        let v: Vec<u64> = line.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
        if v.len() < 4 {
            return None;
        }
        let idle = v[3] + v.get(4).copied().unwrap_or(0);
        Some((idle, v.iter().sum()))
    }

    pub fn memory() -> Option<(u8, f32, f32)> {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let get = |k: &str| -> Option<f32> {
            text.lines().find(|l| l.starts_with(k))?.split_whitespace().nth(1)?.parse().ok()
        };
        let total = get("MemTotal:")?;
        let avail = get("MemAvailable:")?;
        let gb = 1024.0 * 1024.0;
        Some((((total - avail) / total * 100.0).round() as u8, (total - avail) / gb, total / gb))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Action;

    fn key(widget: &str, style: Option<&str>) -> KeyDef {
        KeyDef {
            key: 1,
            label: String::new(),
            icon: None,
            color: None,
            text_color: None,
            action: Action::None,
            toggle: None,
            widget: Some(widget.into()),
            widget_style: style.map(String::from),
            long: None,
            double: None,
        }
    }

    fn snap() -> Snapshot {
        Snapshot {
            time: LocalTime { year: 2026, month: 11, day: 7, dow: 6, hour: 15, minute: 49, second: 12 },
            cpu: Some(23),
            ram: Some((41, 6.5, 16.0)),
            audio: AudioState { master: Some(0.45), master_muted: false, mic_muted: Some(true) },
            cpu_hist: vec![12, 18, 25, 40, 33, 22, 28, 60, 72, 45, 30, 23],
            ram_hist: vec![40, 40, 41, 41, 42, 41, 41],
        }
    }

    #[test]
    fn renders_every_style() {
        for (w, st, _) in STYLES {
            let img = render(&key(w, Some(st)), &snap());
            assert_eq!(img.dimensions(), (S, S));
        }
        // Style inconnu : style par défaut.
        assert_eq!(style_of(&key("cpu", Some("nimporte"))), "bar");
        assert_eq!(style_of(&key("clock", None)), "digital");
    }

    #[test]
    fn signature_changes_with_content() {
        let k = key("clock", None);
        let mut s = snap();
        let a = signature(&k, &s);
        s.time.minute = 50;
        assert_ne!(a, signature(&k, &s));
        s.cpu = Some(99); // ne concerne pas l'horloge
        let b = signature(&k, &s);
        s.cpu = Some(1);
        assert_eq!(b, signature(&k, &s));
        // L'horloge à aiguilles bouge chaque seconde.
        let an = key("clock", Some("analog"));
        let c = signature(&an, &s);
        s.time.second = 13;
        assert_ne!(c, signature(&an, &s));
    }

    #[test]
    fn sampler_keeps_history() {
        let mut sm = Sampler::default();
        for _ in 0..(HISTORY + 5) {
            let s = sm.sample(AudioState::default());
            assert!(s.time.month >= 1 && s.time.month <= 12);
            assert!(s.ram_hist.len() <= HISTORY);
        }
    }

    /// Planche d'aperçu : cargo test -p hubforge widget_sheet -- --ignored
    #[test]
    #[ignore]
    fn widget_sheet() {
        let n = STYLES.len() as u32;
        let mut sheet = RgbImage::from_pixel(n * (S + 8) + 8, S + 16, Rgb([14, 15, 19]));
        for (i, (w, st, _)) in STYLES.iter().enumerate() {
            let img = render(&key(w, Some(st)), &snap());
            image::imageops::overlay(&mut sheet, &img, (8 + i as u32 * (S + 8)) as i64, 8);
        }
        let big = image::imageops::resize(&sheet, sheet.width() * 2, sheet.height() * 2, image::imageops::FilterType::Lanczos3);
        big.save("/tmp/hubforge-widgets.png").unwrap();
    }
}
