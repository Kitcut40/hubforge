//! Mini-jeu caché « Tape-taupe » sur les 15 touches.
//! Des taupes sortent au hasard : il faut les taper avant qu'elles ne
//! repartent. Taupe dorée = 3 points, bombe = -3. Une partie dure 30 s ;
//! les LED des faders servent de chronomètre.

use crate::render::{blend, draw_text_sized};
use hubforge_device::KEY_IMAGE_SIZE as S;
use image::{Rgb, RgbImage, Rgba};
use std::time::{Duration, Instant};

pub const KEYS: usize = 15;
pub const DURATION: Duration = Duration::from_secs(30);
const COUNTDOWN_STEP: Duration = Duration::from_millis(700);
const OVER_DURATION: Duration = Duration::from_secs(5);

/// Ce qu'affiche une touche (sert aussi de clé de cache pour les images).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visual {
    Empty,
    Mole,
    Gold,
    Bomb,
    Hit(i8),
    Miss,
    Countdown(u8),
    Go,
    Title(u8), // écran de fin : 0 = « Fin ! », 1 = « Score », 2 = « Record », 3 = « Nouveau record ! »
    Number(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Cell {
    Empty,
    Mole { until: Instant },
    Gold { until: Instant },
    Bomb { until: Instant },
    Flash { until: Instant, v: Visual },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Countdown { start: Instant },
    Play { start: Instant },
    Over { start: Instant },
    Done,
}

pub struct Game {
    phase: Phase,
    cells: [Cell; KEYS],
    pub score: i32,
    pub best: u32,
    pub new_record: bool,
    next_spawn: Instant,
    rng: u64,
}

impl Game {
    pub fn new(best: u32, now: Instant, seed: u64) -> Game {
        Game {
            phase: Phase::Countdown { start: now },
            cells: [Cell::Empty; KEYS],
            score: 0,
            best,
            new_record: false,
            next_spawn: now,
            rng: seed | 1,
        }
    }

    fn rand(&mut self) -> u64 {
        // xorshift64
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    pub fn is_done(&self) -> bool {
        self.phase == Phase::Done
    }

    pub fn is_over(&self) -> bool {
        matches!(self.phase, Phase::Over { .. } | Phase::Done)
    }

    /// Temps restant de la partie (1.0 au début, 0.0 à la fin).
    pub fn time_left(&self, now: Instant) -> f32 {
        match self.phase {
            Phase::Countdown { .. } => 1.0,
            Phase::Play { start } => 1.0 - now.duration_since(start).as_secs_f32() / DURATION.as_secs_f32(),
            _ => 0.0,
        }
        .clamp(0.0, 1.0)
    }

    /// Avance le jeu ; à appeler souvent (toutes les ~10 ms).
    pub fn tick(&mut self, now: Instant) {
        match self.phase {
            Phase::Countdown { start } => {
                if now.duration_since(start) >= COUNTDOWN_STEP * 4 {
                    self.phase = Phase::Play { start: now };
                    self.next_spawn = now + Duration::from_millis(200);
                }
            }
            Phase::Play { start } => {
                let elapsed = now.duration_since(start);
                if elapsed >= DURATION {
                    self.finish(now);
                    return;
                }
                // Les taupes repartent, les éclairs s'éteignent.
                for c in self.cells.iter_mut() {
                    let gone = match *c {
                        Cell::Mole { until } | Cell::Gold { until } | Cell::Bomb { until } | Cell::Flash { until, .. } => now >= until,
                        Cell::Empty => false,
                    };
                    if gone {
                        *c = Cell::Empty;
                    }
                }
                // De plus en plus vite.
                let p = elapsed.as_secs_f32() / DURATION.as_secs_f32();
                if now >= self.next_spawn {
                    let max_up = 1 + (p * 3.0) as usize; // 1 → 3 taupes à la fois
                    let up = self.cells.iter().filter(|c| matches!(c, Cell::Mole { .. } | Cell::Gold { .. })).count();
                    if up < max_up {
                        self.spawn(now, p);
                    }
                    let gap = 650.0 - 380.0 * p + (self.rand() % 200) as f32;
                    self.next_spawn = now + Duration::from_millis(gap as u64);
                }
            }
            Phase::Over { start } => {
                if now.duration_since(start) >= OVER_DURATION {
                    self.phase = Phase::Done;
                }
            }
            Phase::Done => {}
        }
    }

    fn spawn(&mut self, now: Instant, p: f32) {
        let free: Vec<usize> = (0..KEYS).filter(|&i| self.cells[i] == Cell::Empty).collect();
        if free.is_empty() {
            return;
        }
        let i = free[(self.rand() % free.len() as u64) as usize];
        let life = Duration::from_millis((1150.0 - 550.0 * p) as u64);
        let roll = self.rand() % 100;
        self.cells[i] = if roll < 9 {
            Cell::Gold { until: now + life.mul_f32(0.8) }
        } else if p > 0.3 && roll < 22 {
            Cell::Bomb { until: now + life.mul_f32(1.3) }
        } else {
            Cell::Mole { until: now + life }
        };
    }

    fn finish(&mut self, now: Instant) {
        self.cells = [Cell::Empty; KEYS];
        let s = self.score.max(0) as u32;
        if s > self.best {
            self.best = s;
            self.new_record = true;
        }
        self.phase = Phase::Over { start: now };
    }

    /// Appui sur une touche (0 à 14).
    pub fn press(&mut self, key: usize, now: Instant) {
        if key >= KEYS {
            return;
        }
        match self.phase {
            Phase::Play { .. } => {
                let flash = |v| Cell::Flash { until: now + Duration::from_millis(260), v };
                self.cells[key] = match self.cells[key] {
                    Cell::Mole { .. } => {
                        self.score += 1;
                        flash(Visual::Hit(1))
                    }
                    Cell::Gold { .. } => {
                        self.score += 3;
                        flash(Visual::Hit(3))
                    }
                    Cell::Bomb { .. } => {
                        self.score -= 3;
                        flash(Visual::Hit(-3))
                    }
                    Cell::Empty => flash(Visual::Miss),
                    c @ Cell::Flash { .. } => c,
                };
            }
            // Après 1,5 s d'écran de fin, un appui quitte.
            Phase::Over { start } if now.duration_since(start) > Duration::from_millis(1500) => self.phase = Phase::Done,
            _ => {}
        }
    }

    /// Ce que chaque touche doit afficher maintenant.
    pub fn visuals(&self, now: Instant) -> [Visual; KEYS] {
        let mut out = [Visual::Empty; KEYS];
        match self.phase {
            Phase::Countdown { start } => {
                let step = (now.duration_since(start).as_millis() / COUNTDOWN_STEP.as_millis()) as u8;
                let v = if step >= 3 { Visual::Go } else { Visual::Countdown(3 - step) };
                out[7] = v; // touche du milieu
            }
            Phase::Play { .. } => {
                for (i, c) in self.cells.iter().enumerate() {
                    out[i] = match c {
                        Cell::Empty => Visual::Empty,
                        Cell::Mole { .. } => Visual::Mole,
                        Cell::Gold { .. } => Visual::Gold,
                        Cell::Bomb { .. } => Visual::Bomb,
                        Cell::Flash { v, .. } => *v,
                    };
                }
            }
            Phase::Over { .. } | Phase::Done => {
                out[2] = Visual::Title(0);
                out[6] = Visual::Title(1);
                out[7] = Visual::Number(self.score.max(0) as u32);
                out[11] = Visual::Title(if self.new_record { 3 } else { 2 });
                out[12] = Visual::Number(self.best);
            }
        }
        out
    }
}

// ----------------------------------------------------------------------
// Dessin
// ----------------------------------------------------------------------

fn svg_image(svg: &str, bg: Rgb<u8>) -> RgbImage {
    let mut img = RgbImage::from_pixel(S, S, bg);
    let Ok(tree) = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()) else { return img };
    let Some(mut pm) = resvg::tiny_skia::Pixmap::new(S, S) else { return img };
    let ts = tree.size();
    let scale = S as f32 / ts.width().max(ts.height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    for (i, px) in pm.pixels().iter().enumerate() {
        if px.alpha() == 0 {
            continue;
        }
        let c = px.demultiply();
        blend(&mut img, i as u32 % S, i as u32 / S, Rgba([c.red(), c.green(), c.blue(), px.alpha()]));
    }
    img
}

/// Taupe dessinée en SVG (72×72) : `fur` = couleur du pelage.
fn mole_svg(fur: &str, dark: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">
  <ellipse cx="36" cy="66" rx="30" ry="9" fill="#3b2a1c"/>
  <ellipse cx="36" cy="64" rx="24" ry="5" fill="#241810"/>
  <path d="M14 66 C14 34 22 14 36 14 C50 14 58 34 58 66 Z" fill="{fur}"/>
  <path d="M22 66 C22 46 28 34 36 34 C44 34 50 46 50 66 Z" fill="{dark}" opacity="0.35"/>
  <circle cx="28" cy="31" r="4.2" fill="#111"/><circle cx="44" cy="31" r="4.2" fill="#111"/>
  <circle cx="29.4" cy="29.6" r="1.4" fill="#fff"/><circle cx="45.4" cy="29.6" r="1.4" fill="#fff"/>
  <ellipse cx="36" cy="39" rx="5.5" ry="4" fill="#f48fb1"/>
  <rect x="33" y="43" width="6" height="5" rx="1" fill="#fff"/>
  <line x1="36" y1="43" x2="36" y2="48" stroke="#ccc" stroke-width="0.8"/>
  <path d="M23 40 L12 37 M23 42 L12 43 M49 40 L60 37 M49 42 L60 43" stroke="{dark}" stroke-width="1.2" stroke-linecap="round"/>
</svg>"##
    )
}

const BOMB_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">
  <circle cx="34" cy="42" r="20" fill="#1b1b1f"/>
  <circle cx="27" cy="35" r="5" fill="#55555f"/>
  <rect x="40" y="16" width="10" height="9" rx="2" transform="rotate(35 45 20)" fill="#3a3a44"/>
  <path d="M48 16 C52 10 58 12 60 7" stroke="#c8a165" stroke-width="2.4" fill="none" stroke-linecap="round"/>
  <circle cx="60" cy="7" r="4.5" fill="#ffb300"/><circle cx="60" cy="7" r="2.2" fill="#fff59d"/>
</svg>"##;

const HOLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72" viewBox="0 0 72 72">
  <ellipse cx="36" cy="58" rx="26" ry="9" fill="#2a1d13"/>
  <ellipse cx="36" cy="57" rx="20" ry="5.5" fill="#120c07"/>
</svg>"##;

const GRASS: Rgb<u8> = Rgb([0x1d, 0x3b, 0x22]);

/// Image d'une case du jeu.
pub fn render(v: Visual) -> RgbImage {
    let white = Rgb([255, 255, 255]);
    match v {
        Visual::Empty => svg_image(HOLE_SVG, GRASS),
        Visual::Mole => svg_image(&mole_svg("#8d6e63", "#4e342e"), GRASS),
        Visual::Gold => svg_image(&mole_svg("#ffca28", "#ff8f00"), Rgb([0x2a, 0x44, 0x1e])),
        Visual::Bomb => svg_image(BOMB_SVG, Rgb([0x4a, 0x14, 0x14])),
        Visual::Hit(p) => {
            let bg = if p > 0 { Rgb([0x2e, 0x7d, 0x32]) } else { Rgb([0xc6, 0x28, 0x28]) };
            let mut img = RgbImage::from_pixel(S, S, bg);
            let t = if p > 0 { format!("+{p}") } else { format!("{p}") };
            draw_text_sized(&mut img, &t, white, 8, S - 16, 32);
            img
        }
        Visual::Miss => {
            let mut img = svg_image(HOLE_SVG, Rgb([0x3a, 0x22, 0x22]));
            draw_text_sized(&mut img, "Raté", Rgb([255, 180, 180]), 6, 26, 14);
            img
        }
        Visual::Countdown(n) => {
            let mut img = RgbImage::from_pixel(S, S, Rgb([0xff, 0x7a, 0x1a]));
            draw_text_sized(&mut img, &n.to_string(), white, 4, S - 8, 46);
            img
        }
        Visual::Go => {
            let mut img = RgbImage::from_pixel(S, S, Rgb([0x2e, 0x7d, 0x32]));
            draw_text_sized(&mut img, "GO !", white, 8, S - 16, 26);
            img
        }
        Visual::Title(n) => {
            let (text, bg) = match n {
                0 => ("Fin !", Rgb([0xff, 0x7a, 0x1a])),
                1 => ("Score", Rgb([0x1f, 0x22, 0x29])),
                3 => ("Nouveau record !", Rgb([0xff, 0xb3, 0x00])),
                _ => ("Record", Rgb([0x1f, 0x22, 0x29])),
            };
            let mut img = RgbImage::from_pixel(S, S, bg);
            draw_text_sized(&mut img, text, white, 4, S - 8, 18);
            img
        }
        Visual::Number(n) => {
            let mut img = RgbImage::from_pixel(S, S, Rgb([0x1f, 0x22, 0x29]));
            draw_text_sized(&mut img, &n.to_string(), white, 4, S - 8, 36);
            img
        }
    }
}

/// Couleurs des 5 LED de faders pour le temps restant (chronomètre).
pub fn timer_leds(left: f32) -> [(u8, u8, u8); 5] {
    let color = if left > 0.5 {
        (0, 220, 90)
    } else if left > 0.2 {
        (255, 160, 0)
    } else {
        (255, 30, 20)
    };
    let lit = (left * 5.0).ceil() as usize;
    let mut out = [(0, 0, 0); 5];
    for (i, c) in out.iter_mut().enumerate() {
        if i < lit {
            *c = color;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_a_full_game() {
        let t0 = Instant::now();
        let mut g = Game::new(5, t0, 42);
        assert_eq!(g.visuals(t0)[7], Visual::Countdown(3));
        let mut now = t0;
        let mut hits = 0;
        // Simulation : on tape toutes les taupes visibles.
        while !g.is_over() {
            now += Duration::from_millis(20);
            g.tick(now);
            for (i, v) in g.visuals(now).iter().enumerate() {
                if matches!(v, Visual::Mole | Visual::Gold) {
                    g.press(i, now);
                    hits += 1;
                }
            }
        }
        assert!(hits > 10, "{hits} taupes seulement");
        assert!(g.score > 10);
        assert!(g.new_record && g.best == g.score as u32);
        assert_eq!(g.visuals(now)[7], Visual::Number(g.score as u32));
        // Un appui trop tôt ne quitte pas, plus tard oui.
        g.press(0, now + Duration::from_millis(100));
        assert!(!g.is_done());
        g.press(0, now + Duration::from_secs(2));
        assert!(g.is_done());
    }

    #[test]
    fn bombs_cost_points() {
        let t0 = Instant::now();
        let mut g = Game::new(0, t0, 7);
        g.phase = Phase::Play { start: t0 };
        g.cells[3] = Cell::Bomb { until: t0 + Duration::from_secs(1) };
        g.press(3, t0);
        assert_eq!(g.score, -3);
        g.press(4, t0); // case vide : pas de pénalité
        assert_eq!(g.score, -3);
    }

    #[test]
    fn renders_and_timer() {
        for v in [Visual::Empty, Visual::Mole, Visual::Gold, Visual::Bomb, Visual::Hit(3), Visual::Miss, Visual::Go, Visual::Number(42)] {
            assert_eq!(render(v).dimensions(), (S, S));
        }
        assert_eq!(timer_leds(1.0).iter().filter(|c| **c != (0, 0, 0)).count(), 5);
        assert_eq!(timer_leds(0.3).iter().filter(|c| **c != (0, 0, 0)).count(), 2);
        assert_eq!(timer_leds(0.0).iter().filter(|c| **c != (0, 0, 0)).count(), 0);
    }

    /// Planche : cargo test -p hubforge game_sheet -- --ignored
    #[test]
    #[ignore]
    fn game_sheet() {
        let vs = [Visual::Empty, Visual::Mole, Visual::Gold, Visual::Bomb, Visual::Hit(1), Visual::Hit(-3), Visual::Miss, Visual::Countdown(3), Visual::Title(3), Visual::Number(27)];
        let mut sheet = RgbImage::from_pixel(vs.len() as u32 * (S + 8) + 8, S + 16, Rgb([14, 15, 19]));
        for (i, v) in vs.iter().enumerate() {
            image::imageops::overlay(&mut sheet, &render(*v), (8 + i as u32 * (S + 8)) as i64, 8);
        }
        let big = image::imageops::resize(&sheet, sheet.width() * 3, sheet.height() * 3, image::imageops::FilterType::Nearest);
        big.save("/tmp/hubforge-game.png").unwrap();
    }
}
