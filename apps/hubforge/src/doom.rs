//! Easter egg : Doom sur le MasterHUB.
//!
//! Le moteur (doomgeneric, GPL-2.0-or-later, dans `third_party/doomgeneric`)
//! tourne dans un **processus à part** : `hubforge.exe --doom <port> <wad>`.
//! Il envoie ses images à HubForge par une connexion locale (127.0.0.1) et
//! reçoit les touches par la même connexion. Si Doom plante ou qu'on le
//! quitte, HubForge continue normalement.
//!
//! HubForge découpe ensuite chaque image en 15 morceaux de 72 × 72, un par
//! touche, et n'envoie que les touches qui ont changé, au rythme que le module
//! accepte.

use image::RgbImage;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Codes de touches de Doom (doomkeys.h).
#[allow(dead_code)]
pub mod key {
    pub const RIGHT: u8 = 0xae;
    pub const LEFT: u8 = 0xac;
    pub const UP: u8 = 0xad;
    pub const DOWN: u8 = 0xaf;
    pub const STRAFE_L: u8 = 0xa0;
    pub const STRAFE_R: u8 = 0xa1;
    pub const USE: u8 = 0xa2;
    pub const FIRE: u8 = 0xa3;
    pub const ESCAPE: u8 = 27;
    pub const ENTER: u8 = 13;
    pub const TAB: u8 = 9;
    pub const RSHIFT: u8 = 0x80 + 0x36;
    pub const NEXT_WEAPON: u8 = b']';
    pub const PREV_WEAPON: u8 = b'[';
    pub const YES: u8 = b'y';
}

/// Touches du MasterHUB pendant une partie, dans l'ordre où on les voit
/// (de gauche à droite, puis de haut en bas).
pub const PAD: [&[u8]; 15] = [
    &[key::ESCAPE],
    &[key::UP],
    &[key::UP],
    &[key::UP],
    &[key::ENTER, key::YES],
    &[key::LEFT],
    &[key::USE],
    &[key::FIRE],
    &[key::NEXT_WEAPON],
    &[key::RIGHT],
    &[key::STRAFE_L],
    &[key::DOWN],
    &[key::DOWN],
    &[key::DOWN],
    &[key::STRAFE_R],
];

/// Image reçue du moteur.
#[derive(Clone)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
    /// Vie du joueur (−1 hors partie : menus, intermissions).
    pub health: i32,
    pub damage: u8,
    pub bonus: u8,
}

/// Dernière image en JPEG, pour l'afficher aussi dans la fenêtre de HubForge.
pub static VIEW: Mutex<Option<(u64, Arc<Vec<u8>>)>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// Côté HubForge : lancer et piloter le processus de Doom
// ---------------------------------------------------------------------------

pub struct Session {
    child: std::process::Child,
    stream: TcpStream,
    latest: Arc<Mutex<Option<Frame>>>,
    seq: Arc<AtomicU64>,
    alive: Arc<AtomicBool>,
}

impl Session {
    /// Lance Doom avec ce fichier WAD. `dir` reçoit sa configuration et ses sauvegardes.
    pub fn start(wad: &Path, dir: &Path) -> Result<Session, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--doom")
            .arg(port.to_string())
            .arg(wad)
            .current_dir(dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| format!("lancement de Doom impossible : {e}"))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let stream = loop {
            match listener.accept() {
                Ok((s, _)) => break s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if let Ok(Some(status)) = child.try_wait() {
                        return Err(format!("Doom s'est arrêté au démarrage ({status}). Le fichier WAD est-il valide ?"));
                    }
                    if Instant::now() > deadline {
                        let _ = child.kill();
                        return Err("Doom ne répond pas.".into());
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => {
                    let _ = child.kill();
                    return Err(e.to_string());
                }
            }
        };
        stream.set_nonblocking(false).ok();
        stream.set_nodelay(true).ok();
        let latest = Arc::new(Mutex::new(None));
        let seq = Arc::new(AtomicU64::new(0));
        let alive = Arc::new(AtomicBool::new(true));
        {
            let mut reader = stream.try_clone().map_err(|e| e.to_string())?;
            let (latest, seq, alive) = (latest.clone(), seq.clone(), alive.clone());
            std::thread::Builder::new()
                .name("doom-images".into())
                .spawn(move || {
                    while let Some(f) = read_frame(&mut reader) {
                        *latest.lock().unwrap() = Some(f);
                        seq.fetch_add(1, Ordering::Relaxed);
                    }
                    alive.store(false, Ordering::Relaxed);
                })
                .map_err(|e| e.to_string())?;
        }
        log::info!("👹 Doom lancé ({})", wad.display());
        Ok(Session { child, stream, latest, seq, alive })
    }

    pub fn alive(&mut self) -> bool {
        self.alive.load(Ordering::Relaxed) && matches!(self.child.try_wait(), Ok(None))
    }

    pub fn key(&mut self, pressed: bool, code: u8) {
        let _ = self.stream.write_all(&[pressed as u8, code]);
    }

    /// Dernière image si elle est plus récente que `seen`.
    pub fn frame_after(&self, seen: u64) -> Option<(u64, Frame)> {
        let s = self.seq.load(Ordering::Relaxed);
        if s == seen {
            return None;
        }
        self.latest.lock().unwrap().clone().map(|f| (s, f))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        let _ = self.child.kill();
        let _ = self.child.wait();
        *VIEW.lock().unwrap() = None;
        log::info!("👹 Doom arrêté");
    }
}

/// En-tête d'une image : « HF », largeur, hauteur (u16), vie (i16), dégâts, bonus.
const HEADER: usize = 2 + 2 + 2 + 2 + 1 + 1;

fn read_frame(r: &mut impl Read) -> Option<Frame> {
    let mut h = [0u8; HEADER];
    r.read_exact(&mut h).ok()?;
    if &h[0..2] != b"HF" {
        return None;
    }
    let width = u16::from_le_bytes([h[2], h[3]]) as usize;
    let height = u16::from_le_bytes([h[4], h[5]]) as usize;
    if width == 0 || height == 0 || width > 1920 || height > 1200 {
        return None;
    }
    let mut rgb = vec![0u8; width * height * 3];
    r.read_exact(&mut rgb).ok()?;
    Some(Frame { width, height, rgb, health: i16::from_le_bytes([h[6], h[7]]) as i32, damage: h[8], bonus: h[9] })
}

fn write_frame(w: &mut impl Write, f: &Frame) -> std::io::Result<()> {
    let mut h = [0u8; HEADER];
    h[0..2].copy_from_slice(b"HF");
    h[2..4].copy_from_slice(&(f.width as u16).to_le_bytes());
    h[4..6].copy_from_slice(&(f.height as u16).to_le_bytes());
    h[6..8].copy_from_slice(&(f.health.clamp(-1, 999) as i16).to_le_bytes());
    h[8] = f.damage;
    h[9] = f.bonus;
    w.write_all(&h)?;
    w.write_all(&f.rgb)
}

// ---------------------------------------------------------------------------
// Découpage en touches
// ---------------------------------------------------------------------------

/// Touche physique (0–14) qui se trouve à la case visible (`vx`, `vy`) quand
/// le module est tourné de `rot` degrés.
pub fn physical_key(rot: u16, vx: usize, vy: usize) -> usize {
    let (px, py) = match rot % 360 {
        90 => (vy, 2 - vx),
        180 => (4 - vx, 2 - vy),
        270 => (4 - vy, vx),
        _ => (vx, vy),
    };
    py * 5 + px
}

/// Case visible (index 0–14, de gauche à droite puis de haut en bas) d'une touche physique.
pub fn visible_index(rot: u16, physical: usize) -> usize {
    let (w, h) = visible_grid(rot);
    for vy in 0..h {
        for vx in 0..w {
            if physical_key(rot, vx, vy) == physical {
                return vy * w + vx;
            }
        }
    }
    physical
}

/// Taille de la grille vue par l'utilisateur (colonnes, lignes).
pub fn visible_grid(rot: u16) -> (usize, usize) {
    if rot % 180 == 90 {
        (3, 5)
    } else {
        (5, 3)
    }
}

/// Découpe l'image en 15 images 72 × 72 (index = touche physique), déjà
/// tournées pour s'afficher à l'endroit.
pub fn tiles(f: &Frame, rot: u16) -> Vec<RgbImage> {
    const S: u32 = 72;
    let src = RgbImage::from_raw(f.width as u32, f.height as u32, f.rgb.clone()).unwrap_or_else(|| RgbImage::new(1, 1));
    let (gw, gh) = visible_grid(rot);
    let (cw, ch) = (gw as u32 * S, gh as u32 * S);
    // 5 × 3 touches : on remplit tout l'écran. Module vertical : bandes noires.
    let (dw, dh) = if gw > gh { (cw, ch) } else { (cw, (cw as f32 * f.height as f32 / f.width as f32) as u32) };
    let scaled = image::imageops::resize(&src, dw, dh, image::imageops::FilterType::Triangle);
    let mut canvas = RgbImage::new(cw, ch);
    image::imageops::overlay(&mut canvas, &scaled, 0, ((ch - dh) / 2) as i64);
    let mut out = vec![RgbImage::new(S, S); 15];
    for vy in 0..gh {
        for vx in 0..gw {
            let tile = image::imageops::crop_imm(&canvas, vx as u32 * S, vy as u32 * S, S, S).to_image();
            let tile = match rot % 360 {
                90 => image::imageops::rotate270(&tile),
                180 => image::imageops::rotate180(&tile),
                270 => image::imageops::rotate90(&tile),
                _ => tile,
            };
            out[physical_key(rot, vx, vy)] = tile;
        }
    }
    out
}

/// Couleur moyenne de l'image (pour la LED de la base, façon « ambilight »).
pub fn average_color(f: &Frame) -> (u8, u8, u8) {
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    // La barre d'état (en bas) fausserait la couleur : on l'ignore.
    let rows = f.height * 84 / 100;
    for y in (0..rows).step_by(4) {
        for x in (0..f.width).step_by(4) {
            let i = (y * f.width + x) * 3;
            r += f.rgb[i] as u64;
            g += f.rgb[i + 1] as u64;
            b += f.rgb[i + 2] as u64;
            n += 1;
        }
    }
    if n == 0 {
        return (0, 0, 0);
    }
    // Couleurs plus vives que la moyenne brute, souvent terne.
    let boost = |v: u64| ((v / n) as f32 * 1.6).min(255.0) as u8;
    (boost(r), boost(g), boost(b))
}

/// LED des faders : la vie du joueur (5 témoins), rouge qui clignote quand on est touché.
pub fn health_leds(health: i32, damage: u8, t: u64) -> [(u8, u8, u8); 5] {
    if health < 0 {
        return [(40, 0, 0); 5];
    }
    if damage > 0 && (t / 90) % 2 == 0 {
        return [(255, 0, 0); 5];
    }
    let lit = (health.clamp(0, 100) as f32 / 20.0).ceil() as usize;
    let c = if health > 60 { (0, 230, 60) } else if health > 30 { (255, 190, 0) } else { (255, 30, 30) };
    let mut out = [(0, 0, 0); 5];
    for (i, o) in out.iter_mut().enumerate() {
        if i < lit {
            *o = c;
        }
    }
    // Plus de 100 (méga-sphère…) : bleu.
    if health > 100 {
        out = [(40, 120, 255); 5];
    }
    out
}

// ---------------------------------------------------------------------------
// Fichiers WAD
// ---------------------------------------------------------------------------

/// Freedoom (licence BSD) : jeu libre compatible avec Doom.
pub const FREEDOOM_URL: &str = "https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip";

pub fn doom_dir(cfg_dir: &Path) -> PathBuf {
    cfg_dir.join("doom")
}

/// Cherche un fichier WAD : celui choisi, Freedoom téléchargé, ou un Doom
/// installé avec Steam ou GOG.
pub fn find_wad(cfg_dir: &Path, chosen: &str) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if !chosen.is_empty() {
        candidates.push(PathBuf::from(chosen));
    }
    candidates.push(doom_dir(cfg_dir).join("freedoom1.wad"));
    for root in ["C:\\Program Files (x86)\\Steam\\steamapps\\common", "C:\\Program Files\\Steam\\steamapps\\common", "D:\\SteamLibrary\\steamapps\\common"] {
        for rel in ["Ultimate Doom\\base\\DOOM.WAD", "Ultimate Doom\\rerelease\\DOOM.WAD", "Doom 2\\base\\DOOM2.WAD", "Doom 2\\rerelease\\DOOM2.WAD"] {
            candidates.push(Path::new(root).join(rel));
        }
    }
    for root in ["C:\\GOG Games\\DOOM", "C:\\Program Files (x86)\\GOG Galaxy\\Games\\DOOM"] {
        candidates.push(Path::new(root).join("DOOM.WAD"));
    }
    candidates.into_iter().find(|p| is_iwad(p))
}

/// Vrai si le fichier commence par « IWAD » (fichier de jeu complet).
pub fn is_iwad(p: &Path) -> bool {
    let mut magic = [0u8; 4];
    std::fs::File::open(p).and_then(|mut f| f.read_exact(&mut magic)).map(|_| &magic == b"IWAD").unwrap_or(false)
}

/// Télécharge Freedoom et extrait `freedoom1.wad` dans le dossier de Doom.
pub fn download_freedoom(cfg_dir: &Path, progress: impl Fn(u64, u64)) -> Result<PathBuf, String> {
    let resp = ureq::get(FREEDOOM_URL)
        .set("User-Agent", "HubForge")
        .timeout(Duration::from_secs(120))
        .call()
        .map_err(|e| format!("téléchargement impossible : {e}"))?;
    let total: u64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut data = Vec::with_capacity(total as usize);
    let mut reader = resp.into_reader();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        progress(data.len() as u64, total);
        if data.len() > 80 * 1024 * 1024 {
            return Err("fichier trop gros".into());
        }
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| format!("archive invalide : {e}"))?;
    let dir = doom_dir(cfg_dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join("freedoom1.wad");
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().to_lowercase();
        if name.ends_with("freedoom1.wad") || name.ends_with("copying.txt") {
            let target = if name.ends_with(".wad") { dest.clone() } else { dir.join("FREEDOOM-COPYING.txt") };
            let mut out = Vec::new();
            f.read_to_end(&mut out).map_err(|e| e.to_string())?;
            std::fs::write(&target, out).map_err(|e| e.to_string())?;
        }
    }
    if !is_iwad(&dest) {
        return Err("freedoom1.wad introuvable dans l'archive".into());
    }
    log::info!("Freedoom téléchargé : {}", dest.display());
    Ok(dest)
}

// ---------------------------------------------------------------------------
// Côté Doom : processus lancé avec « --doom »
// ---------------------------------------------------------------------------

static START: OnceLock<Instant> = OnceLock::new();
static KEYS: Mutex<VecDeque<(bool, u8)>> = Mutex::new(VecDeque::new());
static OUT: Mutex<Option<TcpStream>> = Mutex::new(None);
static LAST_SENT: Mutex<Option<Instant>> = Mutex::new(None);

extern "C" {
    fn hf_doom_run(argc: i32, argv: *const *const std::os::raw::c_char);
    fn hf_player_health() -> i32;
    fn hf_player_damage() -> i32;
    fn hf_player_bonus() -> i32;
}

/// Point d'entrée du processus de Doom : `hubforge.exe --doom <port> <wad>`.
pub fn child_main(port: &str, wad: &str) -> ! {
    START.get_or_init(Instant::now);
    let Ok(stream) = TcpStream::connect(("127.0.0.1", port.parse::<u16>().unwrap_or(0))) else {
        std::process::exit(2);
    };
    stream.set_nodelay(true).ok();
    let mut reader = stream.try_clone().unwrap_or_else(|_| std::process::exit(2));
    *OUT.lock().unwrap() = Some(stream);
    std::thread::spawn(move || {
        let mut b = [0u8; 2];
        while reader.read_exact(&mut b).is_ok() {
            KEYS.lock().unwrap().push_back((b[0] != 0, b[1]));
        }
        // HubForge a fermé la connexion : on quitte.
        std::process::exit(0);
    });
    let args: Vec<std::ffi::CString> = ["hubforge-doom", "-iwad", wad]
        .iter()
        .map(|s| std::ffi::CString::new(*s).unwrap_or_default())
        .collect();
    let ptrs: Vec<*const std::os::raw::c_char> = args.iter().map(|a| a.as_ptr()).collect();
    unsafe { hf_doom_run(ptrs.len() as i32, ptrs.as_ptr()) };
    std::process::exit(0)
}

#[no_mangle]
pub extern "C" fn hf_draw_frame(pixels: *const u32, width: i32, height: i32) {
    // 30 images par seconde suffisent largement.
    {
        let mut last = LAST_SENT.lock().unwrap();
        if last.map(|t| t.elapsed() < Duration::from_millis(33)).unwrap_or(false) {
            return;
        }
        *last = Some(Instant::now());
    }
    let (w, h) = (width.max(0) as usize, height.max(0) as usize);
    let src = unsafe { std::slice::from_raw_parts(pixels, w * h) };
    let mut rgb = Vec::with_capacity(w * h * 3);
    for p in src {
        rgb.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
    }
    let f = unsafe {
        Frame {
            width: w,
            height: h,
            rgb,
            health: hf_player_health(),
            damage: hf_player_damage().clamp(0, 255) as u8,
            bonus: hf_player_bonus().clamp(0, 255) as u8,
        }
    };
    let mut out = OUT.lock().unwrap();
    if let Some(s) = out.as_mut() {
        if write_frame(s, &f).is_err() {
            std::process::exit(0);
        }
    }
}

#[no_mangle]
pub extern "C" fn hf_sleep_ms(ms: u32) {
    std::thread::sleep(Duration::from_millis(ms as u64));
}

#[no_mangle]
pub extern "C" fn hf_ticks_ms() -> u32 {
    START.get_or_init(Instant::now).elapsed().as_millis() as u32
}

#[no_mangle]
pub extern "C" fn hf_get_key(pressed: *mut i32, key: *mut u8) -> i32 {
    match KEYS.lock().unwrap().pop_front() {
        Some((p, k)) => {
            unsafe {
                *pressed = p as i32;
                *key = k;
            }
            1
        }
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_roundtrip() {
        let f = Frame { width: 4, height: 2, rgb: (0..24).collect(), health: 87, damage: 3, bonus: 0 };
        let mut buf = Vec::new();
        write_frame(&mut buf, &f).unwrap();
        let g = read_frame(&mut std::io::Cursor::new(buf)).unwrap();
        assert_eq!((g.width, g.height, g.health, g.damage), (4, 2, 87, 3));
        assert_eq!(g.rgb, f.rgb);
    }

    #[test]
    fn key_mapping_is_a_permutation_for_every_rotation() {
        for rot in [0u16, 90, 180, 270] {
            let (w, h) = visible_grid(rot);
            let mut seen = [false; 15];
            for vy in 0..h {
                for vx in 0..w {
                    let k = physical_key(rot, vx, vy);
                    assert!(!seen[k], "rot {rot}");
                    seen[k] = true;
                    assert_eq!(visible_index(rot, k), vy * w + vx);
                }
            }
        }
        assert_eq!(physical_key(0, 4, 2), 14);
    }

    #[test]
    fn tiles_cover_the_whole_frame() {
        let f = Frame { width: 320, height: 200, rgb: vec![200; 320 * 200 * 3], health: 100, damage: 0, bonus: 0 };
        for rot in [0u16, 90] {
            let t = tiles(&f, rot);
            assert_eq!(t.len(), 15);
            assert!(t.iter().all(|i| i.width() == 72 && i.height() == 72));
        }
        assert_eq!(health_leds(100, 0, 0), [(0, 230, 60); 5]);
        assert_eq!(health_leds(25, 0, 0)[1], (255, 30, 30));
        assert_eq!(health_leds(25, 0, 0)[2], (0, 0, 0));
    }
}
