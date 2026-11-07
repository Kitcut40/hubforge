//! Touches vivantes : l'image de la touche est redessinée en continu
//! (horloge, date, processeur, mémoire, volume, micro).

use crate::audio::AudioState;
use crate::config::{parse_color, KeyDef};
use crate::render::{blend, draw_builtin_at, draw_text_sized};
use hubforge_device::KEY_IMAGE_SIZE as S;
use image::{Rgb, RgbImage, Rgba};
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

pub fn is_known(kind: &str) -> bool {
    WIDGETS.iter().any(|(k, _)| *k == kind)
}

const DAYS_SHORT: [&str; 7] = ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."];
const DAYS: [&str; 7] = ["Dimanche", "Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi"];
const MONTHS_SHORT: [&str; 12] =
    ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."];
const MONTHS: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre",
    "décembre",
];

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LocalTime {
    pub year: u16,
    pub month: u8, // 1–12
    pub day: u8,
    pub dow: u8, // 0 = dimanche
    pub hour: u8,
    pub minute: u8,
}

/// Mesures du moment.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Snapshot {
    pub time: LocalTime,
    /// Charge du processeur en %, None à la première mesure.
    pub cpu: Option<u8>,
    /// Mémoire : (% utilisé, Go utilisés, Go au total).
    pub ram: Option<(u8, f32, f32)>,
    pub audio: AudioState,
}

/// Prend les mesures ; garde la mesure précédente du processeur pour
/// calculer sa charge entre deux appels.
#[derive(Default)]
pub struct Sampler {
    cpu_last: Option<(u64, u64)>,
    cpu: Option<u8>,
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
        Snapshot { time: imp::local_time(), cpu: self.cpu, ram: imp::memory(), audio }
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
    let content = match kind {
        "clock" => format!("{:02}{:02}{}", t.hour, t.minute, t.day),
        "date" => format!("{}{}{}", t.year, t.month, t.day),
        "cpu" => format!("{:?}", snap.cpu),
        "ram" => format!("{:?}", snap.ram.map(|(p, u, _)| (p, (u * 10.0) as u32))),
        "volume" => format!("{:?}{}", snap.audio.master.map(|v| (v * 100.0).round() as u32), snap.audio.master_muted),
        "mic" => format!("{:?}", snap.audio.mic_muted),
        _ => String::new(),
    };
    format!("{kind}|{content}|{}|{:?}|{:?}", def.label, def.color, def.text_color)
}

fn color_or(c: &Option<String>, default: (u8, u8, u8)) -> Rgb<u8> {
    let (r, g, b) = c.as_deref().and_then(parse_color).unwrap_or(default);
    Rgb([r, g, b])
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

fn fr_number(v: f32) -> String {
    format!("{v:.1}").replace('.', ",")
}

/// Dessine la touche vivante.
pub fn render(def: &KeyDef, snap: &Snapshot) -> RgbImage {
    let kind = def.widget.as_deref().unwrap_or("");
    let fg = color_or(&def.text_color, (255, 255, 255));
    let label = def.label.trim();
    let t = &snap.time;

    match kind {
        "clock" => {
            let bg = color_or(&def.color, (0x1F, 0x22, 0x29));
            let mut img = RgbImage::from_pixel(S, S, bg);
            draw_text_sized(&mut img, &format!("{:02}:{:02}", t.hour, t.minute), fg, 10, 32, 26);
            let sub = if label.is_empty() {
                format!("{} {} {}", DAYS_SHORT[t.dow as usize % 7], t.day, MONTHS_SHORT[(t.month as usize + 11) % 12])
            } else {
                label.to_string()
            };
            draw_text_sized(&mut img, &sub, mix(fg, bg, 0.65), 44, 20, 11);
            img
        }
        "date" => {
            let bg = color_or(&def.color, (0x1F, 0x22, 0x29));
            let mut img = RgbImage::from_pixel(S, S, bg);
            let top = if label.is_empty() { DAYS[t.dow as usize % 7] } else { label };
            draw_text_sized(&mut img, top, mix(fg, bg, 0.7), 3, 16, 11);
            draw_text_sized(&mut img, &t.day.to_string(), fg, 17, 34, 30);
            draw_text_sized(&mut img, MONTHS[(t.month as usize + 11) % 12], mix(fg, bg, 0.7), 51, 18, 11);
            img
        }
        "cpu" | "ram" => {
            let bg = color_or(&def.color, (0x1F, 0x22, 0x29));
            let mut img = RgbImage::from_pixel(S, S, bg);
            let title = if !label.is_empty() {
                label
            } else if kind == "cpu" {
                "CPU"
            } else {
                "RAM"
            };
            draw_text_sized(&mut img, title, mix(fg, bg, 0.65), 3, 16, 11);
            let (pct, sub) = if kind == "cpu" {
                (snap.cpu, None)
            } else {
                (snap.ram.map(|r| r.0), snap.ram.map(|(_, u, tot)| format!("{} / {} Go", fr_number(u), tot.round())))
            };
            let value = pct.map(|p| format!("{p} %")).unwrap_or_else(|| "–".into());
            let has_sub = sub.is_some();
            draw_text_sized(&mut img, &value, fg, 17, if has_sub { 28 } else { 32 }, 22);
            let bar_y = if has_sub { 48 } else { 54 };
            bar(&mut img, bar_y, pct.unwrap_or(0) as f32 / 100.0, load_color(pct.unwrap_or(0)), mix(fg, bg, 0.18));
            if let Some(sub) = sub {
                draw_text_sized(&mut img, &sub, mix(fg, bg, 0.6), 55, 16, 9);
            }
            img
        }
        "volume" => {
            let bg = color_or(&def.color, (0x1F, 0x22, 0x29));
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
            draw_builtin_at(&mut img, icon, fg, 26, 6);
            let text = if a.master.is_none() {
                "—".to_string()
            } else if a.master_muted {
                "Muet".to_string()
            } else {
                format!("{} %", (v * 100.0).round())
            };
            draw_text_sized(&mut img, &text, fg, 33, 20, 16);
            let fill = if muted { mix(fg, bg, 0.35) } else { Rgb([0, 200, 255]) };
            bar(&mut img, 57, v, fill, mix(fg, bg, 0.18));
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
            draw_builtin_at(&mut img, icon, fg, 34, 8);
            draw_text_sized(&mut img, &text, fg, 44, S - 46, 13);
            img
        }
        _ => {
            let mut img = RgbImage::from_pixel(S, S, Rgb([80, 0, 0]));
            draw_text_sized(&mut img, "Touche vivante inconnue", fg, 0, S, 12);
            img
        }
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

    fn key(widget: &str) -> KeyDef {
        KeyDef {
            key: 1,
            label: String::new(),
            icon: None,
            color: None,
            text_color: None,
            action: Action::None,
            toggle: None,
            widget: Some(widget.into()),
        }
    }

    fn snap() -> Snapshot {
        Snapshot {
            time: LocalTime { year: 2026, month: 11, day: 7, dow: 6, hour: 15, minute: 49 },
            cpu: Some(23),
            ram: Some((41, 6.5, 16.0)),
            audio: AudioState { master: Some(0.45), master_muted: false, mic_muted: Some(true) },
        }
    }

    #[test]
    fn renders_every_widget() {
        for (w, _) in WIDGETS {
            let img = render(&key(w), &snap());
            assert_eq!(img.dimensions(), (S, S));
        }
    }

    #[test]
    fn signature_changes_with_content() {
        let k = key("clock");
        let mut s = snap();
        let a = signature(&k, &s);
        s.time.minute = 50;
        assert_ne!(a, signature(&k, &s));
        s.cpu = Some(99); // ne concerne pas l'horloge
        let b = signature(&k, &s);
        s.cpu = Some(1);
        assert_eq!(b, signature(&k, &s));
    }

    #[test]
    fn sampler_works() {
        let mut sm = Sampler::default();
        let s = sm.sample(AudioState::default());
        assert!(s.time.month >= 1 && s.time.month <= 12);
    }

    /// Planche d'aperçu : cargo test -p hubforge widget_sheet -- --ignored
    #[test]
    #[ignore]
    fn widget_sheet() {
        let n = WIDGETS.len() as u32;
        let mut sheet = RgbImage::from_pixel(n * (S + 8) + 8, S + 16, Rgb([14, 15, 19]));
        for (i, (w, _)) in WIDGETS.iter().enumerate() {
            let img = render(&key(w), &snap());
            image::imageops::overlay(&mut sheet, &img, (8 + i as u32 * (S + 8)) as i64, 8);
        }
        let big = image::imageops::resize(&sheet, sheet.width() * 3, sheet.height() * 3, image::imageops::FilterType::Nearest);
        big.save("/tmp/hubforge-widgets.png").unwrap();
    }
}
