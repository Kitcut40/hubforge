//! Lumières de HubForge : animations des LED de la base et des faders, et
//! couleur d'un fader selon sa position ou le niveau sonore.
//!
//! Tout est calculé ici, sans matériel : une animation donne, pour un instant
//! `t` (millisecondes), la couleur de la base et celle des 5 faders. Le moteur
//! envoie ensuite ces couleurs aux modules (uniquement quand elles changent).

use crate::config::parse_color;
use serde::{Deserialize, Serialize};

pub type Rgb = (u8, u8, u8);

/// Couleurs d'un instant : base et 5 faders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub base: Rgb,
    pub faders: [Rgb; 5],
}

/// Réglages des lumières d'un profil (`[lights]` dans le profil). Les LED de
/// chaque fader se règlent dans `[[faders]]` (champs `led`, `gradient`, `animation`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Lights {
    /// LED de la base : "hardware" (effet intégré : couleur + effet de
    /// `[general]`) ou "animation" (animation HubForge).
    pub base_mode: String,
    pub base_animation: String,
    /// Images par seconde au maximum envoyées aux LED.
    pub fps: u8,
    /// Animations du profil.
    pub animations: Vec<Animation>,
    /// Anciens réglages (0.9), repris par `config::migrate` puis oubliés.
    #[serde(skip_serializing)]
    pub faders_mode: String,
    #[serde(skip_serializing)]
    pub faders_animation: String,
    #[serde(skip_serializing)]
    pub gradient: Vec<String>,
    #[serde(skip_serializing)]
    pub game: Option<toml::Value>,
}

impl Default for Lights {
    fn default() -> Self {
        Lights {
            base_mode: "hardware".into(),
            base_animation: String::new(),
            fps: 15,
            animations: Vec::new(),
            faders_mode: String::new(),
            faders_animation: String::new(),
            gradient: Vec::new(),
            game: None,
        }
    }
}

/// Dégradé par défaut des faders : vert, jaune, rouge.
pub fn default_gradient() -> Vec<String> {
    vec!["0:#00E676".into(), "60:#FFD000".into(), "100:#FF1744".into()]
}

/// Une animation : un modèle tout prêt (`preset`) avec ses couleurs et sa
/// vitesse, ou une suite d'étapes faite à la main (`preset` vide).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Animation {
    pub name: String,
    /// "", "breathe", "rainbow", "wave", "comet", "heartbeat", "police",
    /// "fire", "fill", "strobe".
    pub preset: String,
    /// Couleurs du modèle (1 ou 2 selon le modèle).
    pub colors: Vec<String>,
    /// Vitesse : 1 = normale, 2 = deux fois plus vite.
    pub speed: f32,
    /// Étapes d'une animation faite à la main (jouées en boucle).
    pub steps: Vec<Step>,
}

impl Default for Animation {
    fn default() -> Self {
        Animation { name: String::new(), preset: String::new(), colors: Vec::new(), speed: 1.0, steps: Vec::new() }
    }
}

/// Étape d'une animation faite à la main.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Step {
    /// Durée de l'étape (millisecondes).
    pub ms: u32,
    /// Fondu progressif vers l'étape suivante (sinon changement net).
    pub fade: bool,
    pub base: String,
    /// Couleur de chaque fader (5).
    pub faders: Vec<String>,
}

impl Default for Step {
    fn default() -> Self {
        Step { ms: 500, fade: true, base: "#000000".into(), faders: vec!["#000000".into(); 5] }
    }
}

/// Modèles d'animation proposés : (identifiant, nom affiché, nombre de couleurs).
pub const PRESETS: &[(&str, &str, usize)] = &[
    ("breathe", "Respiration", 1),
    ("rainbow", "Arc-en-ciel", 0),
    ("wave", "Vague", 2),
    ("comet", "Comète", 1),
    ("heartbeat", "Battement de cœur", 1),
    ("police", "Gyrophare", 2),
    ("fire", "Feu de camp", 0),
    ("fill", "Remplissage", 1),
    ("strobe", "Stroboscope", 1),
];

const BLACK: Rgb = (0, 0, 0);

fn color(list: &[String], i: usize, default: Rgb) -> Rgb {
    list.get(i).and_then(|s| parse_color(s)).unwrap_or(default)
}

fn scale(c: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0.0, 1.0);
    ((c.0 as f32 * k).round() as u8, (c.1 as f32 * k).round() as u8, (c.2 as f32 * k).round() as u8)
}

pub fn mix(a: Rgb, b: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Teinte (0–360) → couleur saturée.
fn hue(h: f32) -> Rgb {
    let h = h.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Petit bruit déterministe (0–1) pour le feu.
fn noise(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    (x & 0xFFFF) as f32 / 65535.0
}

/// Couleurs de l'animation à l'instant `t` (millisecondes depuis son début).
pub fn frame(anim: &Animation, t: u64) -> Frame {
    let speed = if anim.speed.is_finite() && anim.speed > 0.05 { anim.speed.min(10.0) } else { 1.0 };
    let t = (t as f64 * speed as f64) as u64;
    let tf = t as f32;
    let c1 = color(&anim.colors, 0, (0, 200, 255));
    let c2 = color(&anim.colors, 1, (255, 0, 160));
    let all = |c: Rgb| Frame { base: c, faders: [c; 5] };
    match anim.preset.as_str() {
        "" => steps_frame(&anim.steps, t),
        "breathe" => {
            let k = 0.08 + 0.92 * (0.5 - 0.5 * (tf / 3000.0 * std::f32::consts::TAU).cos());
            all(scale(c1, k))
        }
        "rainbow" => {
            let base = tf / 8000.0 * 360.0;
            let mut faders = [BLACK; 5];
            for (i, f) in faders.iter_mut().enumerate() {
                *f = hue(base + i as f32 * 40.0);
            }
            Frame { base: hue(base), faders }
        }
        "wave" => {
            let mut faders = [BLACK; 5];
            for (i, f) in faders.iter_mut().enumerate() {
                let k = 0.5 + 0.5 * ((tf / 2000.0 - i as f32 / 5.0) * std::f32::consts::TAU).sin();
                *f = mix(c1, c2, k);
            }
            Frame { base: mix(c1, c2, 0.5 + 0.5 * (tf / 2000.0 * std::f32::consts::TAU).sin()), faders }
        }
        "comet" => {
            // Une lumière fait l'aller-retour sur les faders avec une traînée.
            let period = 1600.0;
            let p = (tf % period) / period * 8.0; // 0 → 4 → 0 sur 8 pas
            let pos = if p <= 4.0 { p } else { 8.0 - p };
            let mut faders = [BLACK; 5];
            for (i, f) in faders.iter_mut().enumerate() {
                let d = (i as f32 - pos).abs();
                *f = scale(c1, (1.0 - d / 1.6).max(0.0));
            }
            Frame { base: scale(c1, 0.25), faders }
        }
        "heartbeat" => {
            // Deux battements rapprochés, puis une pause.
            let ph = t % 1200;
            let pulse = |start: u64| -> f32 {
                if ph >= start && ph < start + 180 {
                    let x = (ph - start) as f32 / 180.0;
                    (x * std::f32::consts::PI).sin()
                } else {
                    0.0
                }
            };
            let k = 0.06 + 0.94 * pulse(0).max(pulse(260) * 0.8);
            all(scale(c1, k))
        }
        "police" => {
            let half = (t / 120) % 8; // 4 éclairs d'un côté, 4 de l'autre
            let on = half % 2 == 0;
            let left = half < 4;
            let mut faders = [BLACK; 5];
            if on {
                for (i, f) in faders.iter_mut().enumerate() {
                    if (left && i < 2) || (!left && i > 2) {
                        *f = if left { c1 } else { c2 };
                    }
                }
            }
            let base = if on { if left { c1 } else { c2 } } else { BLACK };
            Frame { base, faders }
        }
        "fire" => {
            let step = (t / 70) as u32;
            let flame = |i: u32| {
                let k = 0.55 + 0.45 * noise(step.wrapping_mul(7).wrapping_add(i * 131));
                scale(mix((255, 40, 0), (255, 170, 20), noise(step.wrapping_add(i * 977))), k)
            };
            let mut faders = [BLACK; 5];
            for (i, f) in faders.iter_mut().enumerate() {
                *f = flame(i as u32 + 1);
            }
            Frame { base: flame(0), faders }
        }
        "fill" => {
            // Les faders s'allument un par un, restent allumés, puis tout s'éteint.
            let n = ((t / 250) % 7) as usize; // 0..6
            let mut faders = [BLACK; 5];
            for (i, f) in faders.iter_mut().enumerate() {
                if i < n {
                    *f = c1;
                }
            }
            Frame { base: if n >= 5 { c1 } else { scale(c1, 0.2) }, faders }
        }
        "strobe" => all(if (t / 60) % 2 == 0 { c1 } else { BLACK }),
        _ => all(c1),
    }
}

fn steps_frame(steps: &[Step], t: u64) -> Frame {
    if steps.is_empty() {
        return Frame { base: BLACK, faders: [BLACK; 5] };
    }
    let total: u64 = steps.iter().map(|s| s.ms.max(20) as u64).sum();
    let mut at = t % total.max(1);
    for (i, s) in steps.iter().enumerate() {
        let d = s.ms.max(20) as u64;
        if at < d {
            let cur = step_colors(s);
            if !s.fade {
                return cur;
            }
            let next = step_colors(&steps[(i + 1) % steps.len()]);
            let k = at as f32 / d as f32;
            let mut faders = [BLACK; 5];
            for (j, f) in faders.iter_mut().enumerate() {
                *f = mix(cur.faders[j], next.faders[j], k);
            }
            return Frame { base: mix(cur.base, next.base, k), faders };
        }
        at -= d;
    }
    step_colors(&steps[0])
}

fn step_colors(s: &Step) -> Frame {
    let mut faders = [BLACK; 5];
    for (i, f) in faders.iter_mut().enumerate() {
        *f = color(&s.faders, i, BLACK);
    }
    Frame { base: parse_color(&s.base).unwrap_or(BLACK), faders }
}

/// Couleur d'un fader selon sa position (0–100) et le dégradé « 0:#…, 100:#… ».
pub fn gradient_color(stops: &[String], value: u8) -> Rgb {
    let mut pts: Vec<(f32, Rgb)> = stops
        .iter()
        .filter_map(|s| {
            let (p, c) = s.split_once(':')?;
            Some((p.trim().parse::<f32>().ok()?.clamp(0.0, 100.0), parse_color(c)?))
        })
        .collect();
    if pts.is_empty() {
        return (255, 255, 255);
    }
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let v = value.min(100) as f32;
    if v <= pts[0].0 {
        return pts[0].1;
    }
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if v <= b.0 {
            let span = (b.0 - a.0).max(0.001);
            return mix(a.1, b.1, (v - a.0) / span);
        }
    }
    pts[pts.len() - 1].1
}

/// Trouve une animation du profil par son nom.
pub fn find<'a>(lights: &'a Lights, name: &str) -> Option<&'a Animation> {
    lights.animations.iter().find(|a| a.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_interpolates_between_stops() {
        let stops = vec!["0:#000000".to_string(), "100:#FFFFFF".to_string()];
        assert_eq!(gradient_color(&stops, 0), (0, 0, 0));
        assert_eq!(gradient_color(&stops, 100), (255, 255, 255));
        assert_eq!(gradient_color(&stops, 50), (128, 128, 128));
        let three = vec!["100:#FF0000".to_string(), "0:#00FF00".to_string(), "50:#FFFF00".to_string()];
        assert_eq!(gradient_color(&three, 50), (255, 255, 0));
        assert_eq!(gradient_color(&three, 75), (255, 128, 0));
    }

    #[test]
    fn steps_loop_and_fade() {
        let anim = Animation {
            steps: vec![
                Step { ms: 100, fade: true, base: "#000000".into(), faders: vec!["#000000".into(); 5] },
                Step { ms: 100, fade: false, base: "#FFFFFF".into(), faders: vec!["#FF0000".into(); 5] },
            ],
            ..Animation::default()
        };
        assert_eq!(frame(&anim, 0).base, (0, 0, 0));
        assert_eq!(frame(&anim, 50).base, (128, 128, 128));
        assert_eq!(frame(&anim, 150).faders[2], (255, 0, 0));
        assert_eq!(frame(&anim, 200).base, (0, 0, 0)); // boucle
    }

    #[test]
    fn every_preset_renders() {
        for (id, ..) in PRESETS {
            let a = Animation { preset: id.to_string(), colors: vec!["#FF8800".into(), "#0044FF".into()], ..Animation::default() };
            for t in [0, 37, 1000, 123_456] {
                let _ = frame(&a, t);
            }
        }
    }
}
