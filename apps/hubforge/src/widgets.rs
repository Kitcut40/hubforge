//! Touches vivantes : l'image de la touche est redessinée en continu
//! (horloge, date, processeur, mémoire, carte graphique, volume, micro,
//! météo), avec plusieurs styles chacune (horloge à aiguilles, calendrier,
//! jauge, graphique, prévisions…).

use crate::audio::{AudioState, Meter};
use crate::render::draw_text_in;
use crate::weather::{self, Sky, Weather};
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
    ("gpu", "Carte graphique (utilisation)"),
    ("gpu_temp", "Carte graphique (température)"),
    ("vram", "Mémoire vidéo (VRAM)"),
    ("volume", "Volume général"),
    ("app_volume", "Volume d'une application"),
    ("mic", "État du micro"),
    ("weather", "Météo"),
];

/// Touches vivantes qui ont besoin des mesures de la carte graphique.
pub fn needs_gpu(kind: &str) -> bool {
    matches!(kind, "gpu" | "gpu_temp" | "vram")
}

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
    ("gpu", "bar", "Barre"),
    ("gpu", "gauge", "Jauge"),
    ("gpu", "graph", "Graphique"),
    ("gpu_temp", "bar", "Barre"),
    ("gpu_temp", "gauge", "Jauge"),
    ("gpu_temp", "graph", "Graphique"),
    ("vram", "bar", "Barre"),
    ("vram", "gauge", "Jauge"),
    ("vram", "graph", "Graphique"),
    ("volume", "bar", "Barre"),
    ("volume", "gauge", "Jauge"),
    ("app_volume", "bar", "Barre"),
    ("app_volume", "gauge", "Jauge"),
    ("weather", "now", "Maintenant"),
    ("weather", "compact", "Compacte"),
    ("weather", "forecast", "Prochains jours"),
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

/// Heure locale du PC.
pub fn local_time() -> LocalTime {
    imp::local_time()
}

/// Carte graphique (NVIDIA).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Gpu {
    pub name: String,
    /// Utilisation en %.
    pub usage: u8,
    /// Température en °C.
    pub temp: u8,
    /// Mémoire vidéo : (Go utilisés, Go au total).
    pub mem: (f32, f32),
}

impl Gpu {
    pub fn mem_pct(&self) -> u8 {
        if self.mem.1 > 0.0 {
            (self.mem.0 / self.mem.1 * 100.0).round().clamp(0.0, 100.0) as u8
        } else {
            0
        }
    }
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
    /// Carte graphique, None si pas de carte NVIDIA (ou pas demandée).
    pub gpu: Option<Gpu>,
    pub gpu_hist: Vec<u8>,
    pub gpu_temp_hist: Vec<u8>,
    pub vram_hist: Vec<u8>,
    /// Volume des applications des touches « app_volume » (cible "app:…").
    pub apps: Vec<(String, Meter)>,
}

/// Prend les mesures ; garde la mesure précédente du processeur pour
/// calculer sa charge entre deux appels, et un historique.
#[derive(Default)]
pub struct Sampler {
    cpu_last: Option<(u64, u64)>,
    cpu: Option<u8>,
    cpu_hist: VecDeque<u8>,
    ram_hist: VecDeque<u8>,
    gpu_hist: VecDeque<u8>,
    gpu_temp_hist: VecDeque<u8>,
    vram_hist: VecDeque<u8>,
    /// Mesurer la carte graphique (seulement si une touche l'affiche).
    pub want_gpu: bool,
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
        let gpu = if self.want_gpu { imp::gpu() } else { None };
        push(&mut self.gpu_hist, gpu.as_ref().map(|g| g.usage));
        push(&mut self.gpu_temp_hist, gpu.as_ref().map(|g| g.temp.min(100)));
        push(&mut self.vram_hist, gpu.as_ref().map(|g| g.mem_pct()));
        Snapshot {
            time: imp::local_time(),
            cpu: self.cpu,
            ram,
            audio,
            cpu_hist: self.cpu_hist.iter().copied().collect(),
            ram_hist: self.ram_hist.iter().copied().collect(),
            gpu,
            gpu_hist: self.gpu_hist.iter().copied().collect(),
            gpu_temp_hist: self.gpu_temp_hist.iter().copied().collect(),
            vram_hist: self.vram_hist.iter().copied().collect(),
            apps: Vec::new(),
        }
    }
}

/// Mesures partagées pour les aperçus de l'interface.
pub fn preview_snapshot(audio: AudioState) -> Snapshot {
    static SAMPLER: OnceLock<Mutex<Sampler>> = OnceLock::new();
    let m = SAMPLER.get_or_init(|| Mutex::new(Sampler { want_gpu: true, ..Default::default() }));
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
        ("gpu", "graph") => format!("{:?}{:?}", snap.gpu.as_ref().map(|g| g.usage), snap.gpu_hist),
        ("gpu", _) => format!("{:?}", snap.gpu.as_ref().map(|g| g.usage)),
        ("gpu_temp", "graph") => format!("{:?}{:?}", snap.gpu.as_ref().map(|g| g.temp), snap.gpu_temp_hist),
        ("gpu_temp", _) => format!("{:?}", snap.gpu.as_ref().map(|g| g.temp)),
        ("vram", "graph") => format!("{:?}{:?}", snap.gpu.as_ref().map(|g| (g.mem.0 * 10.0) as u32), snap.vram_hist),
        ("vram", _) => format!("{:?}", snap.gpu.as_ref().map(|g| (g.mem.0 * 10.0) as u32)),
        ("app_volume", _) => {
            let m = app_meter(def, snap);
            format!("{:?}{}{}", m.level.map(|v| (v * 100.0).round() as u32), m.muted, m.present)
        }
        ("weather", _) => format!("{:?}{}", weather::get(), t.day),
        _ => String::new(),
    };
    format!("{kind}|{style}|{content}|{}|{:?}|{:?}|{:?}", def.label, def.color, def.text_color, def.theme)
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

/// Vert, puis orange, puis rouge selon la température (°C).
fn temp_color(t: u8) -> Rgb<u8> {
    match t {
        0..=64 => Rgb([61, 220, 132]),
        65..=79 => Rgb([255, 179, 0]),
        _ => Rgb([255, 59, 48]),
    }
}

/// Une mesure en pourcentage, prête à dessiner (CPU, RAM, GPU…).
struct Metric<'a> {
    title: &'a str,
    pct: Option<u8>,
    value: String,
    color: Rgb<u8>,
    hist: &'a [u8],
    sub: Option<String>,
}

fn metric<'a>(kind: &str, snap: &'a Snapshot, label: &'a str) -> Metric<'a> {
    let pick = |default: &'a str| if label.is_empty() { default } else { label };
    let pct_text = |p: Option<u8>| p.map(|p| format!("{p} %")).unwrap_or_else(|| "–".into());
    let no_gpu = || snap.gpu.is_none().then(|| "Pas de GPU NVIDIA".to_string());
    match kind {
        "ram" => Metric {
            title: pick("RAM"),
            pct: snap.ram.map(|r| r.0),
            value: pct_text(snap.ram.map(|r| r.0)),
            color: load_color(snap.ram.map(|r| r.0).unwrap_or(0)),
            hist: &snap.ram_hist,
            sub: snap.ram.map(|(_, u, tot)| format!("{} / {} Go", fr_number(u), tot.round())),
        },
        "gpu" => {
            let p = snap.gpu.as_ref().map(|g| g.usage);
            Metric { title: pick("GPU"), pct: p, value: pct_text(p), color: load_color(p.unwrap_or(0)), hist: &snap.gpu_hist, sub: no_gpu() }
        }
        "gpu_temp" => {
            let t = snap.gpu.as_ref().map(|g| g.temp);
            Metric {
                title: pick("GPU"),
                pct: t.map(|t| t.min(100)),
                value: t.map(|t| format!("{t} °C")).unwrap_or_else(|| "–".into()),
                color: temp_color(t.unwrap_or(0)),
                hist: &snap.gpu_temp_hist,
                sub: no_gpu(),
            }
        }
        "vram" => {
            let p = snap.gpu.as_ref().map(|g| g.mem_pct());
            Metric {
                title: pick("VRAM"),
                pct: p,
                value: pct_text(p),
                color: load_color(p.unwrap_or(0)),
                hist: &snap.vram_hist,
                sub: snap.gpu.as_ref().map(|g| format!("{} / {} Go", fr_number(g.mem.0), g.mem.1.round())).or_else(no_gpu),
            }
        }
        _ => Metric {
            title: pick("CPU"),
            pct: snap.cpu,
            value: pct_text(snap.cpu),
            color: load_color(snap.cpu.unwrap_or(0)),
            hist: &snap.cpu_hist,
            sub: None,
        },
    }
}

/// Cible audio d'une touche « app_volume » ("app:discord.exe,spotify.exe").
pub fn app_target(def: &KeyDef) -> Option<String> {
    let list: Vec<String> = def
        .widget_target
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    (!list.is_empty()).then(|| format!("app:{}", list.join(",")))
}

fn app_meter(def: &KeyDef, snap: &Snapshot) -> Meter {
    app_target(def).and_then(|t| snap.apps.iter().find(|(k, _)| *k == t).map(|(_, m)| *m)).unwrap_or_default()
}

/// « discord.exe » → « Discord ».
fn app_title(def: &KeyDef) -> String {
    let first = def.widget_target.as_deref().unwrap_or("").split(',').next().unwrap_or("").trim().to_string();
    let base = first.strip_suffix(".exe").unwrap_or(&first);
    let known = match base.to_lowercase().as_str() {
        "msedge" => Some("Edge"),
        "chrome" => Some("Chrome"),
        "firefox" => Some("Firefox"),
        "obs64" | "obs" => Some("OBS"),
        "vlc" => Some("VLC"),
        "steam" | "steamwebhelper" => Some("Steam"),
        "beamng.drive.x64" | "beamng.drive" => Some("BeamNG"),
        "flightsimulator" | "flightsimulator2024" => Some("MSFS"),
        _ => None,
    };
    if let Some(k) = known {
        return k.into();
    }
    let mut c = base.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => "Appli".into(),
    }
}

// ----------------------------------------------------------------------
// Météo
// ----------------------------------------------------------------------

fn sun(cx: f32, cy: f32, r: f32) -> String {
    let mut s = format!(r##"<circle cx="{cx}" cy="{cy}" r="{r}" fill="#FFC531"/>"##);
    for i in 0..8 {
        let (x1, y1) = polar(cx, cy, r + 1.8, i as f32 * 45.0);
        let (x2, y2) = polar(cx, cy, r + 3.6, i as f32 * 45.0);
        s += &format!(r##"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="#FFC531" stroke-width="1.6" stroke-linecap="round"/>"##);
    }
    s
}

const CLOUD: &str = "M7 19a5 5 0 0 1-.6-9.96A6.5 6.5 0 0 1 18.6 9.6 4.75 4.75 0 0 1 18 19z";

/// Icône météo dessinée (dans un carré de 24 unités, placée en `cx`, `cy`).
fn weather_icon(code: u8, is_day: bool, cx: f32, cy: f32, size: f32) -> String {
    let k = size / 24.0;
    let body = match weather::sky(code) {
        Sky::Clear if is_day => sun(12.0, 12.0, 5.0),
        Sky::Clear => r##"<path d="M15 3a8.5 8.5 0 1 0 6.5 13.6A7 7 0 0 1 15 3z" fill="#E8EEF7"/>"##.to_string(),
        Sky::Partly => {
            let back = if is_day {
                sun(8.5, 8.0, 3.6)
            } else {
                r##"<path d="M9 2.5a5.5 5.5 0 1 0 5 8A4.5 4.5 0 0 1 9 2.5z" fill="#E8EEF7"/>"##.to_string()
            };
            format!(r##"{back}<path d="{CLOUD}" transform="translate(2.4 2.6) scale(0.9)" fill="#F4F7FB"/>"##)
        }
        Sky::Cloudy => format!(
            r##"<path d="{CLOUD}" transform="translate(-1.5 -2.5) scale(0.8)" fill="#B9C4D2"/><path d="{CLOUD}" transform="translate(1 1)" fill="#F4F7FB"/>"##
        ),
        Sky::Fog => format!(
            r##"<path d="{CLOUD}" transform="translate(0 -3)" fill="#DDE3EA"/><g stroke="#DDE3EA" stroke-width="1.6" stroke-linecap="round"><line x1="4" y1="19.5" x2="20" y2="19.5"/><line x1="6" y1="22.5" x2="18" y2="22.5"/></g>"##
        ),
        Sky::Drizzle | Sky::Rain => {
            let drops = if weather::sky(code) == Sky::Rain { [7.5, 11.5, 15.5].as_slice() } else { [9.5, 14.0].as_slice() };
            let mut s = format!(r##"<path d="{CLOUD}" transform="translate(0 -4)" fill="#E3E9F1"/>"##);
            for x in drops {
                s += &format!(r##"<line x1="{x}" y1="17.5" x2="{}" y2="21.5" stroke="#5EB8FF" stroke-width="1.8" stroke-linecap="round"/>"##, x - 1.2);
            }
            s
        }
        Sky::Snow => format!(
            r##"<path d="{CLOUD}" transform="translate(0 -4)" fill="#E3E9F1"/><g fill="#FFFFFF"><circle cx="8" cy="19" r="1.3"/><circle cx="12" cy="21.5" r="1.3"/><circle cx="16" cy="19" r="1.3"/></g>"##
        ),
        Sky::Storm => format!(
            r##"<path d="{CLOUD}" transform="translate(0 -4)" fill="#AEB8C6"/><path d="M13 13l-3.5 5.5h3l-1.5 5 5-7h-3l1.5-3.5z" fill="#FFC531"/>"##
        ),
    };
    format!(r#"<g transform="translate({:.2} {:.2}) scale({k:.3})">{body}</g>"#, cx - size / 2.0, cy - size / 2.0)
}

/// Fond automatique selon le temps (dégradé).
fn weather_bg(code: u8, is_day: bool) -> (&'static str, &'static str) {
    match (weather::sky(code), is_day) {
        (_, false) => ("#24365C", "#0F1A33"),
        (Sky::Clear | Sky::Partly, true) => ("#4FA8F8", "#2468D0"),
        (Sky::Snow, true) => ("#9DB2D0", "#6D82A3"),
        (Sky::Rain | Sky::Storm, true) => ("#5A6880", "#323C4E"),
        _ => ("#8593A8", "#56637A"),
    }
}

fn deg(v: f32) -> String {
    format!("{}°", v.round() as i32)
}

fn render_weather(def: &KeyDef, style: &str, fg: Rgb<u8>, label: &str) -> RgbImage {
    let custom = def.color.as_deref().filter(|c| !c.eq_ignore_ascii_case("#1F2229")).and_then(parse_color);
    let w: Result<Weather, String> = weather::get();
    let (code, is_day) = w.as_ref().map(|w| (w.code, w.is_day)).unwrap_or((3, true));
    let mut img = RgbImage::from_pixel(S, S, Rgb([0x1F, 0x22, 0x29]));
    match custom {
        Some((r, g, b)) => img = RgbImage::from_pixel(S, S, Rgb([r, g, b])),
        None => {
            let (top, bottom) = if w.is_ok() { weather_bg(code, is_day) } else { ("#3A4150", "#262B35") };
            draw_svg(
                &mut img,
                &format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient></defs><rect width="72" height="72" fill="url(#g)"/></svg>"#
                ),
            );
        }
    }
    let soft = Rgb([(fg.0[0] as u16 * 4 / 5) as u8, (fg.0[1] as u16 * 4 / 5) as u8, (fg.0[2] as u16 * 4 / 5) as u8]);
    let svg = |inner: String| format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">{inner}</svg>"#);
    let w = match w {
        Ok(w) => w,
        Err(msg) => {
            draw_svg(&mut img, &svg(weather_icon(3, true, 36.0, 22.0, 26.0)));
            draw_text_sized(&mut img, &msg, fg, 38, 32, 10);
            return img;
        }
    };
    let city = if label.is_empty() { w.city.as_str() } else { label };
    let today = w.days.first();
    match style {
        "compact" => {
            draw_svg(&mut img, &svg(weather_icon(w.code, w.is_day, 51.0, 22.0, 30.0)));
            draw_text_in(&mut img, &deg(w.temp), fg, 21.0, 38.0, 6, 32, 26);
            let day = DAYS_SHORT[today.map(|d| d.dow).unwrap_or(0) as usize % 7];
            draw_text_in(&mut img, day, soft, 21.0, 38.0, 40, 14, 11);
            draw_text_sized(&mut img, weather::sky_name(w.code), soft, 55, 14, 10);
        }
        "forecast" => {
            draw_svg(&mut img, r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72"><line x1="36" y1="8" x2="36" y2="64" stroke="#FFFFFF" stroke-opacity="0.25" stroke-width="1"/></svg>"##);
            for (i, d) in w.days.iter().skip(1).take(2).enumerate() {
                let cx = 18.0 + i as f32 * 36.0;
                draw_text_in(&mut img, DAYS_SHORT[d.dow as usize % 7], soft, cx, 32.0, 3, 13, 10);
                draw_svg(&mut img, &svg(weather_icon(d.code, true, cx, 28.0, 22.0)));
                draw_text_in(&mut img, &deg(d.max), fg, cx, 32.0, 41, 15, 13);
                draw_text_in(&mut img, &deg(d.min), soft, cx, 32.0, 55, 13, 10);
            }
        }
        _ => {
            draw_text_sized(&mut img, city, soft, 2, 14, 10);
            draw_svg(&mut img, &svg(weather_icon(w.code, w.is_day, 20.0, 33.0, 26.0)));
            draw_text_in(&mut img, &deg(w.temp), fg, 50.0, 40.0, 19, 28, 24);
            if let Some(d) = today {
                draw_text_sized(&mut img, &format!("{} / {}", deg(d.max), deg(d.min)), soft, 54, 15, 10);
            }
        }
    }
    img
}

// ----------------------------------------------------------------------
// Rendu
// ----------------------------------------------------------------------

/// Dessine la touche vivante.
pub fn render(def: &KeyDef, snap: &Snapshot) -> RgbImage {
    if let Some(theme) = crate::theme::of(def) {
        if let Some(img) = themed::render(theme, def, snap) {
            return img;
        }
    }
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
        "cpu" | "ram" | "gpu" | "gpu_temp" | "vram" => {
            let bg = color_or(&def.color, dark);
            let mut img = RgbImage::from_pixel(S, S, bg);
            let Metric { title, pct, value, color, hist, sub } = metric(kind, snap, label);
            let track = mix(fg, bg, 0.16);
            match style {
                "gauge" => {
                    gauge(&mut img, pct.unwrap_or(0) as f32 / 100.0, color, track);
                    draw_text_sized(&mut img, &value, fg, 24, 20, 15);
                    draw_text_sized(&mut img, title, mix(fg, bg, 0.65), 55, 15, 10);
                }
                "graph" => {
                    graph(&mut img, hist, color, mix(fg, bg, 0.14));
                    draw_text_sized(&mut img, &format!("{title} {value}"), fg, 3, 18, 12);
                }
                _ => {
                    draw_text_sized(&mut img, title, mix(fg, bg, 0.65), 3, 16, 11);
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
        "app_volume" => {
            let bg = color_or(&def.color, dark);
            let mut img = RgbImage::from_pixel(S, S, bg);
            let m = app_meter(def, snap);
            let title = if label.is_empty() { app_title(def) } else { label.to_string() };
            let v = m.level.unwrap_or(0.0);
            let (icon, text, fill) = if def.widget_target.as_deref().unwrap_or("").trim().is_empty() {
                ("volume-x", "Choisis l'appli".to_string(), mix(fg, bg, 0.35))
            } else if !m.present {
                ("volume-x", "Aucun son".to_string(), mix(fg, bg, 0.35))
            } else if m.muted {
                ("volume-x", "Muet".to_string(), mix(fg, bg, 0.35))
            } else {
                (if v < 0.5 { "volume-1" } else { "volume-2" }, format!("{} %", (v * 100.0).round()), VOLUME_BLUE)
            };
            if style == "gauge" {
                gauge(&mut img, if m.present { v } else { 0.0 }, fill, mix(fg, bg, 0.16));
                draw_builtin_at(&mut img, icon, mix(fg, bg, 0.8), 16, 18);
                draw_text_sized(&mut img, &text, fg, 34, 16, 13);
                draw_text_sized(&mut img, &title, mix(fg, bg, 0.65), 55, 15, 10);
            } else {
                draw_text_sized(&mut img, &title, mix(fg, bg, 0.7), 3, 14, 11);
                draw_builtin_at(&mut img, icon, fg, 18, 17);
                draw_text_sized(&mut img, &text, fg, 36, 18, 15);
                bar(&mut img, 58, if m.present { v } else { 0.0 }, fill, mix(fg, bg, 0.18));
            }
            img
        }
        "weather" => render_weather(def, style, fg, label),
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

/// Touches vivantes dans les thèmes (Graphite, Néon, Plan, Console, Mono).
mod themed {
    use super::*;
    use crate::theme::{self as th, Style, Theme};

    fn rgb(h: &str) -> (u8, u8, u8) {
        parse_color(h).unwrap_or((61, 169, 252))
    }

    /// Accent par défaut de chaque touche vivante.
    fn default_accent(def: &KeyDef, kind: &str, snap: &Snapshot) -> (u8, u8, u8) {
        rgb(match kind {
            "clock" => "#FF7A1A",
            "date" => "#EF4444",
            "cpu" => "#3DA9FC",
            "ram" => "#A35BFF",
            "gpu" => "#2ECC71",
            "gpu_temp" => "#FFB020",
            "vram" => "#14B8A6",
            "weather" => "#3B82F6",
            "mic" => {
                if snap.audio.mic_muted == Some(false) {
                    "#2ECC71"
                } else {
                    "#EF4444"
                }
            }
            "app_volume" => match brand(def) {
                Some("discord") => "#5865F2",
                Some("spotify") => "#1DB954",
                Some("steam") => "#66C0F4",
                Some("twitch") => "#9146FF",
                Some("youtube") => "#FF3B30",
                Some("chrome") => "#FBBC04",
                _ => "#3DA9FC",
            },
            _ => "#3DA9FC",
        })
    }

    /// Logo de l'application d'une touche « volume d'une application ».
    fn brand(def: &KeyDef) -> Option<&'static str> {
        let t = def.widget_target.as_deref().unwrap_or("").to_lowercase();
        ["discord", "spotify", "steam", "twitch", "youtube", "chrome"].into_iter().find(|b| t.contains(b))
    }

    fn doc(st: &Style, body: String) -> RgbImage {
        th::rasterize(&th::svg_doc(st, &body))
    }

    /// Décalage vertical du contenu (le bouton de Console a un témoin en haut).
    fn dy(st: &Style) -> f32 {
        if st.theme == Theme::Console {
            5.0
        } else {
            0.0
        }
    }

    /// Zone horizontale utile (Console : dans le bouton ; Néon : dans le cadre).
    fn span(st: &Style) -> (f32, f32) {
        match st.theme {
            Theme::Console => (8.0, 64.0),
            Theme::Neon => (4.0, 68.0),
            Theme::Plan => (2.0, 70.0),
            _ => (0.0, 72.0),
        }
    }

    fn val(st: &Style, x: f32, y: f32, s: &str, size: f32) -> String {
        th::text(x, y, s, size, &st.val, st.value_weight, 0.0, 64.0, 1.0)
    }

    fn small(st: &Style, x: f32, y: f32, s: &str, size: f32, max_w: f32) -> String {
        let s = if st.upper { s.to_uppercase() } else { s.to_string() };
        th::text(x, y, &s, size, &st.soft, 600, st.spacing.min(0.8), max_w, 0.95)
    }

    fn bar(st: &Style, y: f32, frac: f32, color: &str) -> String {
        let (x, w) = (12.0, 48.0);
        let mut s = format!(
            r##"<rect x="{x}" y="{y}" width="{w}" height="4" rx="2" fill="#FFFFFF" fill-opacity="{}"/>"##,
            st.track_op
        );
        let fw = w * frac.clamp(0.0, 1.0);
        if fw > 0.5 {
            s += &format!(r#"<rect x="{x}" y="{y}" width="{fw:.1}" height="4" rx="2" fill="{color}" {}/>"#, th::fx(st));
        }
        s
    }

    fn gauge(st: &Style, cx: f32, cy: f32, r: f32, frac: f32, color: &str) -> String {
        let pt = |deg: f32| {
            let a = deg.to_radians();
            (cx + r * a.cos(), cy + r * a.sin())
        };
        let (x1, y1) = pt(135.0);
        let (x2, y2) = pt(405.0);
        let frac = frac.clamp(0.0, 1.0);
        let (xe, ye) = pt(135.0 + 270.0 * frac);
        let mut s = format!(
            r##"<path d="M{x1:.2} {y1:.2} A{r} {r} 0 1 1 {x2:.2} {y2:.2}" stroke="#FFFFFF" stroke-opacity="{}" stroke-width="{}" fill="none" stroke-linecap="round"/>"##,
            st.track_op, st.stroke
        );
        if frac > 0.005 {
            let large = if 270.0 * frac > 180.0 { 1 } else { 0 };
            s += &format!(
                r#"<path d="M{x1:.2} {y1:.2} A{r} {r} 0 {large} 1 {xe:.2} {ye:.2}" stroke="{color}" stroke-width="{}" fill="none" stroke-linecap="round" {}/>"#,
                st.stroke,
                th::fx(st)
            );
        }
        s
    }

    fn graph(st: &Style, hist: &[u8], y0: f32, y1: f32, color: &str) -> String {
        let (x0, x1) = span(st);
        let n = HISTORY.max(2) as f32 - 1.0;
        if hist.len() < 2 {
            return String::new();
        }
        let pts: Vec<(f32, f32)> = hist
            .iter()
            .enumerate()
            .map(|(i, v)| (x1 - (hist.len() - 1 - i) as f32 * (x1 - x0) / n, y1 - (*v as f32 / 100.0) * (y1 - y0)))
            .collect();
        let line: String = pts.iter().map(|(x, y)| format!("{x:.1},{y:.1} ")).collect();
        let mut s = String::new();
        if !matches!(st.theme, Theme::Plan | Theme::Mono) {
            s += &format!(r#"<polygon points="{:.1},{y1} {line}{:.1},{y1}" fill="{color}" fill-opacity="0.22"/>"#, pts[0].0, pts[pts.len() - 1].0);
        }
        let w = if matches!(st.theme, Theme::Plan | Theme::Mono) { 1.2 } else { 1.8 };
        s += &format!(r#"<polyline points="{line}" fill="none" stroke="{color}" stroke-width="{w}" stroke-linejoin="round" {}/>"#, th::fx(st));
        s
    }

    /// Couleur d'une mesure : l'accent, puis orange et rouge quand ça chauffe.
    fn level_color(st: &Style, pct: u8, hot: (u8, u8)) -> String {
        if matches!(st.theme, Theme::Plan | Theme::Mono) {
            return st.line.clone();
        }
        if pct >= hot.1 {
            "#FF3B30".into()
        } else if pct >= hot.0 {
            "#FFB020".into()
        } else {
            st.line.clone()
        }
    }

    fn weather_icon_name(code: u8, is_day: bool) -> &'static str {
        match (weather::sky(code), is_day) {
            (Sky::Clear, true) => "sun-full",
            (Sky::Clear, false) => "moon-full",
            (Sky::Partly, true) => "cloud-sun",
            (Sky::Partly, false) => "cloud-moon",
            (Sky::Fog, _) => "cloud-fog",
            (Sky::Drizzle | Sky::Rain, _) => "cloud-rain",
            (Sky::Snow, _) => "cloud-snow",
            (Sky::Storm, _) => "cloud-lightning",
            _ => "cloud",
        }
    }

    fn weather_color(st: &Style, code: u8) -> String {
        match st.theme {
            Theme::Graphite => match weather::sky(code) {
                Sky::Clear | Sky::Partly => "#FFD15C".into(),
                _ => "#CFE3FF".into(),
            },
            Theme::Neon => th::hex_of(st.accent),
            _ => st.ic.clone(),
        }
    }

    pub fn render(theme: Theme, def: &KeyDef, snap: &Snapshot) -> Option<RgbImage> {
        let kind = def.widget.as_deref().unwrap_or("");
        let style = style_of(def);
        let st = th::style(theme, th::accent_with(def, default_accent(def, kind, snap)));
        let label = def.label.trim();
        let t = &snap.time;
        let d = dy(&st);
        let fx = th::fx(&st);
        let ly = st.label_y;
        let mut b = String::new();
        match kind {
            "cpu" | "ram" | "gpu" | "gpu_temp" | "vram" => {
                let Metric { title, pct, value, hist, sub, .. } = metric(kind, snap, label);
                let hot = if kind == "gpu_temp" { (65, 80) } else { (60, 85) };
                let color = level_color(&st, pct.unwrap_or(0), hot);
                let icon = match kind {
                    "cpu" => "cpu",
                    "ram" => "hard-drive",
                    "gpu_temp" => "thermometer",
                    _ => "graphics-card",
                };
                let caption = if matches!(kind, "ram" | "vram") || snap.gpu.is_none() && kind != "cpu" && kind != "ram" {
                    sub.clone().unwrap_or_else(|| title.to_string())
                } else {
                    title.to_string()
                };
                match style {
                    "gauge" => {
                        b += &gauge(&st, 36.0, 31.0 + d, 21.0, pct.unwrap_or(0) as f32 / 100.0, &color);
                        b += &th::text(36.0, 36.0 + d, &value, 13.0, &st.val, st.value_weight, 0.0, 30.0, 1.0);
                        b += &th::label(&st, title, ly);
                    }
                    "graph" => {
                        b += &graph(&st, hist, 32.0 + d, if st.theme == Theme::Console { 58.0 } else { 66.0 }, &color);
                        b += &small(&st, 36.0, 11.5 + d, title, 8.5, 60.0);
                        b += &val(&st, 36.0, 29.0 + d, &value, 17.0);
                    }
                    _ => {
                        b += &th::icon(icon, st.weight, 36.0, 17.0 + d, 20.0, &st.ic, fx);
                        b += &val(&st, 36.0, 40.0 + d * 0.7, &value, 16.0);
                        b += &bar(&st, 46.0 + d * 0.6, pct.unwrap_or(0) as f32 / 100.0, &color);
                        b += &th::label(&st, &caption, ly);
                    }
                }
            }
            "volume" | "app_volume" => {
                let (level, muted, present, title) = if kind == "volume" {
                    let a = &snap.audio;
                    (a.master, a.master_muted, a.master.is_some(), if label.is_empty() { "Volume".to_string() } else { label.to_string() })
                } else {
                    let m = app_meter(def, snap);
                    (m.level, m.muted, m.present, if label.is_empty() { app_title(def) } else { label.to_string() })
                };
                let unset = kind == "app_volume" && def.widget_target.as_deref().unwrap_or("").trim().is_empty();
                let v = level.unwrap_or(0.0);
                let text = if unset {
                    "Choisis l'appli".to_string()
                } else if !present {
                    "Aucun son".to_string()
                } else if muted {
                    "Muet".to_string()
                } else {
                    format!("{} %", (v * 100.0).round())
                };
                let off = unset || !present || muted;
                let color = if off { st.soft.clone() } else { st.line.clone() };
                let icon = match (kind, brand(def)) {
                    ("app_volume", Some(b)) => b,
                    _ if off => "volume-x",
                    _ if v < 0.5 => "volume-1",
                    _ => "volume-2",
                };
                let ic = if off { st.soft.clone() } else { st.ic.clone() };
                let shown = if present && !muted { v } else { 0.0 };
                if style == "gauge" {
                    b += &gauge(&st, 36.0, 31.0 + d, 21.0, shown, &color);
                    b += &th::icon(icon, st.weight, 36.0, 24.0 + d, 13.0, &ic, fx);
                    b += &val(&st, 36.0, 43.0 + d, &text, if text.len() > 6 { 10.0 } else { 13.0 });
                    b += &th::label(&st, &title, ly);
                } else {
                    b += &th::icon(icon, st.weight, 36.0, 17.0 + d, 22.0, &ic, fx);
                    b += &val(&st, 36.0, 40.0 + d * 0.7, &text, if text.len() > 6 { 11.0 } else { 16.0 });
                    b += &bar(&st, 46.0 + d * 0.6, shown, &color);
                    b += &th::label(&st, &title, ly);
                }
            }
            "mic" => {
                let (icon, text) = match snap.audio.mic_muted {
                    Some(true) => ("mic-off", "Coupé"),
                    Some(false) => ("mic", if label.is_empty() { "Micro" } else { label }),
                    None => ("mic-off", "Pas de micro"),
                };
                if style == "icon" {
                    b += &format!(
                        r#"<circle cx="36" cy="{}" r="27" fill="none" stroke="{}" stroke-opacity="0.45" stroke-width="2.5" {fx}/>"#,
                        36.0 + d,
                        th::hex_of(st.accent)
                    );
                    b += &th::icon(icon, st.weight, 36.0, 36.0 + d, 32.0, &st.ic, fx);
                } else {
                    b += &th::icon(icon, st.weight, 36.0, 29.0 + d * 0.7, 32.0, &st.ic, fx);
                    b += &th::label(&st, text, ly);
                }
            }
            "clock" => {
                let date = format!("{} {} {}", DAYS_SHORT[t.dow as usize % 7], t.day, MONTHS_SHORT[(t.month as usize + 11) % 12]);
                match style {
                    "analog" => {
                        let (cx, cy) = (36.0, 36.0 + d * 0.5);
                        let r = if st.theme == Theme::Console { 25.0 } else { 29.0 };
                        b += &format!(r##"<circle cx="{cx}" cy="{cy}" r="{r}" fill="#FFFFFF" fill-opacity="0.05" stroke="{}" stroke-opacity="0.35" stroke-width="1.5"/>"##, st.fg);
                        for i in 0..12 {
                            let a = (i as f32 * 30.0 - 90.0).to_radians();
                            let long = i % 3 == 0;
                            let (r1, r2) = (if long { r - 7.0 } else { r - 4.5 }, r - 2.5);
                            b += &format!(
                                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-opacity="{}" stroke-width="{}" stroke-linecap="round"/>"#,
                                cx + r1 * a.cos(),
                                cy + r1 * a.sin(),
                                cx + r2 * a.cos(),
                                cy + r2 * a.sin(),
                                st.fg,
                                if long { 1.0 } else { 0.5 },
                                if long { 2.0 } else { 1.2 }
                            );
                        }
                        let minute = t.minute as f32 + t.second as f32 / 60.0;
                        let hour = (t.hour % 12) as f32 + minute / 60.0;
                        let hand = |deg: f32, len: f32, w: f32, c: &str| {
                            let a = (deg - 90.0).to_radians();
                            format!(
                                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{c}" stroke-width="{w}" stroke-linecap="round"/>"#,
                                cx - 4.0 * a.cos(),
                                cy - 4.0 * a.sin(),
                                cx + len * a.cos(),
                                cy + len * a.sin()
                            )
                        };
                        b += &hand(hour * 30.0, r * 0.52, 3.4, &st.val);
                        b += &hand(minute * 6.0, r * 0.78, 2.4, &st.val);
                        let sec = th::hex_of(st.accent);
                        b += &hand(t.second as f32 * 6.0, r * 0.86, 1.2, &sec);
                        b += &format!(r#"<circle cx="{cx}" cy="{cy}" r="2.4" fill="{sec}"/>"#);
                    }
                    "big" => {
                        b += &th::text(36.0, 33.0 + d, &format!("{:02}", t.hour), 27.0, &st.val, st.value_weight.max(600), 0.0, 64.0, 1.0);
                        b += &th::text(36.0, 62.0 + d * 0.3, &format!("{:02}", t.minute), 27.0, &th::hex_of(st.accent), st.value_weight.max(600), 0.0, 64.0, 1.0);
                    }
                    _ => {
                        b += &val(&st, 36.0, 39.0 + d, &format!("{:02}:{:02}", t.hour, t.minute), 21.0);
                        b += &th::label(&st, if label.is_empty() { &date } else { label }, ly);
                    }
                }
            }
            "date" => {
                let day = if label.is_empty() { DAYS[t.dow as usize % 7] } else { label };
                let month = MONTHS[(t.month as usize + 11) % 12];
                if style == "calendar" {
                    let a = th::hex_of(st.accent);
                    let (x, y, w, h) = if st.theme == Theme::Console { (12.0, 15.0, 48.0, 46.0) } else { (9.0, 7.0, 54.0, 56.0) };
                    b += &format!(r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="7" fill="#FFFFFF" fill-opacity="0.07" stroke="{a}" stroke-opacity="0.5"/>"##);
                    b += &format!(r#"<path d="M{x} {} a7 7 0 0 1 7 -7 h{} a7 7 0 0 1 7 7 v7 h-{w} z" fill="{a}"/>"#, y + 7.0, w - 14.0);
                    b += &th::text(36.0, y + 10.5, &MONTHS_SHORT[(t.month as usize + 11) % 12].to_uppercase(), 8.5, "#FFFFFF", 700, 0.6, w - 6.0, 1.0);
                    b += &val(&st, 36.0, y + h - 12.0, &t.day.to_string(), 24.0);
                    b += &small(&st, 36.0, y + h - 3.0, DAYS_SHORT[t.dow as usize % 7], 8.0, w - 6.0);
                } else {
                    b += &small(&st, 36.0, 15.0 + d, day, 10.0, 62.0);
                    b += &val(&st, 36.0, 45.0 + d * 0.6, &t.day.to_string(), 30.0);
                    b += &th::label(&st, month, ly);
                }
            }
            "weather" => {
                let w = match weather::get() {
                    Ok(w) => w,
                    Err(msg) => {
                        b += &th::icon("cloud", st.weight, 36.0, 24.0 + d, 26.0, &st.soft, "");
                        b += &th::text(36.0, 50.0 + d, &msg, 9.0, &st.fg, 600, 0.0, 64.0, 0.95);
                        return Some(doc(&st, b));
                    }
                };
                let city = if label.is_empty() { w.city.as_str() } else { label };
                let wc = weather_color(&st, w.code);
                match style {
                    "compact" => {
                        b += &th::icon(weather_icon_name(w.code, w.is_day), st.weight, 51.0, 23.0 + d, 28.0, &wc, fx);
                        b += &th::text(22.0, 31.0 + d, &deg(w.temp), 22.0, &st.val, st.value_weight, 0.0, 36.0, 1.0);
                        let day = DAYS_SHORT[w.days.first().map(|x| x.dow).unwrap_or(t.dow) as usize % 7];
                        b += &small(&st, 22.0, 44.0 + d, day, 9.5, 34.0);
                        b += &th::label(&st, weather::sky_name(w.code), ly);
                    }
                    "forecast" => {
                        let (x0, x1) = if st.theme == Theme::Console { (21.0, 51.0) } else { (19.0, 53.0) };
                        for (i, dd) in w.days.iter().skip(1).take(2).enumerate() {
                            let cx = if i == 0 { x0 } else { x1 };
                            b += &small(&st, cx, 13.0 + d, DAYS_SHORT[dd.dow as usize % 7], 8.5, 30.0);
                            b += &th::icon(weather_icon_name(dd.code, true), st.weight, cx, 28.0 + d, 18.0, &weather_color(&st, dd.code), fx);
                            b += &th::text(cx, 48.0 + d * 0.5, &deg(dd.max), 12.5, &st.val, st.value_weight, 0.0, 32.0, 1.0);
                            b += &th::text(cx, 59.0, &deg(dd.min), 9.0, &st.soft, 600, 0.0, 32.0, 0.9);
                        }
                        b += &format!(r##"<rect x="35.5" y="{}" width="1" height="46" fill="#FFFFFF" fill-opacity="0.15"/>"##, 9.0 + d);
                    }
                    _ => {
                        b += &small(&st, 36.0, 12.5 + d, city, 9.0, 60.0);
                        b += &th::icon(weather_icon_name(w.code, w.is_day), st.weight, 21.0, 33.0 + d, 26.0, &wc, fx);
                        b += &th::text(51.0, 41.0 + d, &deg(w.temp), 19.0, &st.val, st.value_weight, 0.0, 34.0, 1.0);
                        if let Some(dd) = w.days.first() {
                            b += &th::label(&st, &format!("{} / {}", deg(dd.max), deg(dd.min)), ly);
                        }
                    }
                }
            }
            _ => return None,
        }
        Some(doc(&st, b))
    }
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

    // --- Carte graphique NVIDIA, via la bibliothèque NVML du pilote ---

    use std::ffi::c_void;
    use std::sync::OnceLock;
    use windows::core::{s, w, PCSTR};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

    type Dev = *mut c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Util {
        gpu: u32,
        memory: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Mem {
        total: u64,
        free: u64,
        used: u64,
    }

    struct Nvml {
        dev: Dev,
        name: String,
        temp: unsafe extern "C" fn(Dev, u32, *mut u32) -> i32,
        util: unsafe extern "C" fn(Dev, *mut Util) -> i32,
        mem: unsafe extern "C" fn(Dev, *mut Mem) -> i32,
    }
    // Les pointeurs viennent d'une bibliothèque chargée une fois pour toutes.
    unsafe impl Send for Nvml {}
    unsafe impl Sync for Nvml {}

    fn load() -> Option<Nvml> {
        unsafe {
            let lib = LoadLibraryW(w!("nvml.dll"))
                .or_else(|_| LoadLibraryW(w!("C:\\Program Files\\NVIDIA Corporation\\NVSMI\\nvml.dll")))
                .ok()?;
            let f = |n: PCSTR| GetProcAddress(lib, n);
            let init: unsafe extern "C" fn() -> i32 = std::mem::transmute(f(s!("nvmlInit_v2"))?);
            let handle: unsafe extern "C" fn(u32, *mut Dev) -> i32 = std::mem::transmute(f(s!("nvmlDeviceGetHandleByIndex_v2"))?);
            let name_fn: unsafe extern "C" fn(Dev, *mut u8, u32) -> i32 = std::mem::transmute(f(s!("nvmlDeviceGetName"))?);
            if init() != 0 {
                return None;
            }
            let mut dev: Dev = std::ptr::null_mut();
            if handle(0, &mut dev) != 0 || dev.is_null() {
                return None;
            }
            let mut buf = [0u8; 96];
            let name = if name_fn(dev, buf.as_mut_ptr(), buf.len() as u32) == 0 {
                let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
                String::from_utf8_lossy(&buf[..end]).to_string()
            } else {
                "NVIDIA".into()
            };
            log::info!("carte graphique : {name}");
            Some(Nvml {
                dev,
                name,
                temp: std::mem::transmute(f(s!("nvmlDeviceGetTemperature"))?),
                util: std::mem::transmute(f(s!("nvmlDeviceGetUtilizationRates"))?),
                mem: std::mem::transmute(f(s!("nvmlDeviceGetMemoryInfo"))?),
            })
        }
    }

    pub fn gpu() -> Option<super::Gpu> {
        static NVML: OnceLock<Option<Nvml>> = OnceLock::new();
        let n = NVML.get_or_init(load).as_ref()?;
        let (mut t, mut u, mut m) = (0u32, Util::default(), Mem::default());
        unsafe {
            if (n.util)(n.dev, &mut u) != 0 {
                return None;
            }
            let _ = (n.temp)(n.dev, 0, &mut t);
            let _ = (n.mem)(n.dev, &mut m);
        }
        let gb = 1024.0 * 1024.0 * 1024.0;
        Some(super::Gpu {
            name: n.name.clone(),
            usage: u.gpu.min(100) as u8,
            temp: t.min(255) as u8,
            mem: (m.used as f32 / gb, m.total as f32 / gb),
        })
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

    pub fn gpu() -> Option<super::Gpu> {
        None
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
            widget_target: (widget == "app_volume").then(|| "discord.exe".into()),
            long: None,
            double: None,
            theme: None,
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
            gpu: Some(Gpu { name: "RTX 2060 SUPER".into(), usage: 23, temp: 35, mem: (2.4, 8.0) }),
            gpu_hist: vec![5, 8, 23, 40, 61, 55, 30, 23],
            gpu_temp_hist: vec![33, 34, 35, 38, 41, 40, 37, 35],
            vram_hist: vec![28, 29, 30, 30, 31, 30],
            apps: vec![("app:discord.exe".into(), Meter { present: true, muted: false, peak: 0.2, level: Some(0.81) })],
        }
    }

    fn fake_weather() {
        weather::inject(
            "Toulouse",
            Weather {
                city: "Toulouse".into(),
                temp: 5.4,
                code: 2,
                is_day: true,
                days: vec![
                    weather::Day { dow: 1, code: 2, max: 8.2, min: 2.0 },
                    weather::Day { dow: 2, code: 61, max: 8.0, min: 0.4 },
                    weather::Day { dow: 3, code: 3, max: 9.1, min: 3.0 },
                ],
            },
        );
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

    /// Planche des thèmes : cargo test -p hubforge theme_sheet -- --ignored
    #[test]
    #[ignore]
    fn theme_sheet() {
        fake_weather();
        let mut actions = vec![
            ("builtin:copy", "Copier", Some("#FF8A3D")),
            ("builtin:layers", "Macro", Some("#A35BFF")),
            ("builtin:mic", "Micro", Some("#8BC34A")),
            ("builtin:volume-2", "Son", Some("#8BC34A")),
            ("builtin:lock", "Verrouiller", None),
            ("", "Texte seul", Some("#5865F2")),
        ];
        let widgets: Vec<(&str, &str)> = vec![
            ("cpu", "bar"), ("gpu_temp", "gauge"), ("gpu", "graph"), ("ram", "bar"), ("app_volume", "bar"), ("volume", "gauge"),
            ("mic", "classic"), ("clock", "digital"), ("clock", "analog"), ("clock", "big"), ("date", "classic"), ("date", "calendar"),
            ("weather", "now"), ("weather", "compact"), ("weather", "forecast"),
        ];
        let cols = actions.len() + widgets.len();
        let themes = ["graphite", "neon", "plan", "console", "mono", "classic"];
        let mut sheet = RgbImage::from_pixel(cols as u32 * (S + 6) + 6, themes.len() as u32 * (S + 6) + 6, Rgb([14, 15, 19]));
        for (r, th) in themes.iter().enumerate() {
            let mut c = 0;
            for (icon, label, color) in actions.iter_mut() {
                let mut k = key("cpu", None);
                k.widget = None;
                k.icon = (!icon.is_empty()).then(|| icon.to_string());
                k.label = label.to_string();
                k.color = color.map(String::from);
                k.theme = Some(th.to_string());
                let v = crate::render::render_key(&k, std::path::Path::new("/tmp"));
                let img = image::load_from_memory(&v.frames[0].jpeg).unwrap().to_rgb8();
                image::imageops::overlay(&mut sheet, &img, (6 + c * (S + 6)) as i64, (6 + r as u32 * (S + 6)) as i64);
                c += 1;
            }
            for (w, st) in &widgets {
                let mut k = key(w, Some(st));
                k.color = Some("#1F2229".into());
                k.theme = Some(th.to_string());
                let img = render(&k, &snap());
                image::imageops::overlay(&mut sheet, &img, (6 + c * (S + 6)) as i64, (6 + r as u32 * (S + 6)) as i64);
                c += 1;
            }
        }
        let big = image::imageops::resize(&sheet, sheet.width() * 2, sheet.height() * 2, image::imageops::FilterType::Nearest);
        big.save("/tmp/hubforge-themes.png").unwrap();
    }

    /// Planche d'aperçu : cargo test -p hubforge widget_sheet -- --ignored
    #[test]
    #[ignore]
    fn widget_sheet() {
        fake_weather();
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
