#![cfg_attr(not(windows), allow(dead_code))]
//! Enregistreur de macros : note les touches du clavier (et, si demandé,
//! les clics et la molette de la souris) avec leurs délais, puis les
//! transforme en étapes de macro.
//!
//! L'enregistrement s'arrête avec la touche Échap (non enregistrée), le
//! bouton « Arrêter » de l'interface, au bout de 2 minutes ou de
//! `MAX_EVENTS` évènements. Les clics dans les fenêtres de HubForge et les
//! frappes simulées (par HubForge ou un autre logiciel) sont ignorés.

use crate::config::{Action, MACRO_MAX_STEPS};
use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Durée maximale d'un enregistrement.
pub const MAX_TIME: Duration = Duration::from_secs(120);
/// Nombre maximal d'évènements notés.
pub const MAX_EVENTS: usize = 600;
/// En dessous de ce délai, aucune pause n'est ajoutée entre deux étapes.
const MIN_WAIT_MS: u64 = 20;
/// Appui et relâchement plus rapprochés que ceci : un simple appui.
const TAP_MS: u64 = 400;

#[derive(Debug, Clone, PartialEq)]
pub enum RawKind {
    KeyDown(u32),
    KeyUp(u32),
    MouseDown(&'static str, i32, i32),
    MouseUp(&'static str, i32, i32),
    Wheel(i32),
}

#[derive(Debug, Clone)]
pub struct Raw {
    pub kind: RawKind,
    pub at: Instant,
}

#[derive(Default)]
struct Rec {
    running: bool,
    mouse: bool,
    started: Option<Instant>,
    events: Vec<Raw>,
    /// Touches déjà enfoncées (pour ignorer la répétition automatique).
    down: Vec<u32>,
    /// Arrêt demandé par Échap.
    escaped: bool,
    thread_id: u32,
}

fn rec() -> &'static Mutex<Rec> {
    static R: OnceLock<Mutex<Rec>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Rec::default()))
}

/// État de l'enregistrement, pour l'interface.
#[derive(Debug, Clone, Serialize)]
pub struct RecState {
    pub recording: bool,
    pub events: usize,
    pub elapsed_ms: u64,
    /// Arrêté par Échap, la durée maximale ou le nombre d'évènements.
    pub finished: bool,
}

pub fn state() -> RecState {
    let mut r = rec().lock().unwrap();
    let elapsed = r.started.map(|s| s.elapsed()).unwrap_or_default();
    if r.running && (elapsed >= MAX_TIME || r.events.len() >= MAX_EVENTS) {
        r.escaped = true;
    }
    RecState {
        recording: r.running && !r.escaped,
        events: r.events.len(),
        elapsed_ms: elapsed.as_millis() as u64,
        finished: r.running && r.escaped,
    }
}

/// Commence un enregistrement (le précédent est oublié).
pub fn start(mouse: bool) -> Result<(), String> {
    {
        let r = rec().lock().unwrap();
        if r.running {
            return Err("un enregistrement est déjà en cours".into());
        }
    }
    {
        let mut r = rec().lock().unwrap();
        *r = Rec { running: true, mouse, started: Some(Instant::now()), ..Default::default() };
    }
    imp::start_hooks()
}

/// Arrête l'enregistrement et retourne les étapes de macro (et si la liste
/// a été coupée faute de place).
pub fn stop() -> (Vec<Action>, bool) {
    let events = {
        let mut r = rec().lock().unwrap();
        if !r.running {
            return (Vec::new(), false);
        }
        r.running = false;
        let tid = r.thread_id;
        drop(r);
        imp::stop_hooks(tid);
        std::mem::take(&mut rec().lock().unwrap().events)
    };
    to_steps(&events)
}

/// Note un évènement (appelé par les hooks). Échap arrête l'enregistrement.
fn push(kind: RawKind) {
    let mut r = rec().lock().unwrap();
    if !r.running || r.escaped {
        return;
    }
    if r.started.is_some_and(|s| s.elapsed() >= MAX_TIME) || r.events.len() >= MAX_EVENTS {
        r.escaped = true;
        return;
    }
    match &kind {
        RawKind::KeyDown(vk) => {
            if *vk == VK_ESCAPE {
                r.escaped = true;
                return;
            }
            if r.down.contains(vk) {
                return; // répétition automatique
            }
            r.down.push(*vk);
        }
        RawKind::KeyUp(vk) => {
            if *vk == VK_ESCAPE {
                return;
            }
            if !r.down.contains(vk) {
                return; // enfoncée avant le début de l'enregistrement
            }
            r.down.retain(|k| k != vk);
        }
        RawKind::MouseDown(..) | RawKind::MouseUp(..) | RawKind::Wheel(_) => {
            if !r.mouse {
                return;
            }
        }
    }
    r.events.push(Raw { kind, at: Instant::now() });
}

const VK_ESCAPE: u32 = 0x1B;

/// Nom HubForge d'une touche virtuelle Windows (« ctrl », « a », « f5 »…).
pub fn key_name(vk: u32) -> Option<String> {
    Some(match vk {
        0x10 | 0xA0 | 0xA1 => "shift".into(),
        0x11 | 0xA2 | 0xA3 => "ctrl".into(),
        0x12 | 0xA4 | 0xA5 => "alt".into(),
        0x5B | 0x5C => "win".into(),
        0x0D => "enter".into(),
        0x09 => "tab".into(),
        0x20 => "space".into(),
        0x08 => "backspace".into(),
        0x2E => "delete".into(),
        0x2D => "insert".into(),
        0x24 => "home".into(),
        0x23 => "end".into(),
        0x21 => "pageup".into(),
        0x22 => "pagedown".into(),
        0x25 => "left".into(),
        0x26 => "up".into(),
        0x27 => "right".into(),
        0x28 => "down".into(),
        0x2C => "printscreen".into(),
        0x41..=0x5A => ((b'a' + (vk - 0x41) as u8) as char).to_string(),
        0x30..=0x39 => ((b'0' + (vk - 0x30) as u8) as char).to_string(),
        0x60..=0x69 => ((b'0' + (vk - 0x60) as u8) as char).to_string(),
        0x70..=0x87 => format!("f{}", vk - 0x70 + 1),
        _ => return imp::char_of(vk).filter(|c| !c.is_whitespace() && *c != '+').map(|c| c.to_string()),
    })
}

/// Transforme les évènements en étapes : pauses, appuis simples (« Raccourci »),
/// touches enfoncées / relâchées, clics.
pub fn to_steps(events: &[Raw]) -> (Vec<Action>, bool) {
    let mut steps: Vec<Action> = Vec::new();
    let mut last: Option<Instant> = None;
    let mut i = 0;
    let mut cut = false;
    let wait = |steps: &mut Vec<Action>, last: &mut Option<Instant>, at: Instant| {
        if let Some(l) = *last {
            let ms = at.saturating_duration_since(l).as_millis() as u64;
            let ms = (ms + 5) / 10 * 10;
            if ms >= MIN_WAIT_MS {
                steps.push(Action::Wait { ms: ms.min(crate::config::WAIT_MAX_MS) });
            }
        }
        *last = Some(at);
    };
    while i < events.len() {
        if steps.len() >= MACRO_MAX_STEPS - 1 {
            cut = true;
            break;
        }
        let e = &events[i];
        let next = events.get(i + 1);
        match &e.kind {
            RawKind::KeyDown(vk) => {
                let Some(name) = key_name(*vk) else {
                    i += 1;
                    continue;
                };
                wait(&mut steps, &mut last, e.at);
                let quick_up = next.filter(|n| n.kind == RawKind::KeyUp(*vk) && n.at.duration_since(e.at).as_millis() as u64 <= TAP_MS);
                if quick_up.is_some() {
                    steps.push(Action::Hotkey { keys: name });
                    i += 2;
                    continue;
                }
                steps.push(Action::Key { keys: name, op: "down".into() });
            }
            RawKind::KeyUp(vk) => {
                if let Some(name) = key_name(*vk) {
                    wait(&mut steps, &mut last, e.at);
                    steps.push(Action::Key { keys: name, op: "up".into() });
                }
            }
            RawKind::MouseDown(b, x, y) => {
                wait(&mut steps, &mut last, e.at);
                let quick_up = next.filter(|n| matches!(n.kind, RawKind::MouseUp(nb, ..) if nb == *b) && n.at.duration_since(e.at).as_millis() as u64 <= TAP_MS);
                let op = if quick_up.is_some() { "click" } else { "down" };
                steps.push(Action::Mouse { op: op.into(), button: b.to_string(), x: Some(*x), y: Some(*y), amount: 0 });
                if quick_up.is_some() {
                    i += 2;
                    continue;
                }
            }
            RawKind::MouseUp(b, x, y) => {
                wait(&mut steps, &mut last, e.at);
                steps.push(Action::Mouse { op: "up".into(), button: b.to_string(), x: Some(*x), y: Some(*y), amount: 0 });
            }
            RawKind::Wheel(delta) => {
                // Crans consécutifs regroupés.
                let mut total = -delta / 120;
                let mut j = i + 1;
                while let Some(n) = events.get(j) {
                    match n.kind {
                        RawKind::Wheel(d) if n.at.duration_since(events[j - 1].at) < Duration::from_millis(250) && (d < 0) == (*delta < 0) => {
                            total += -d / 120;
                            j += 1;
                        }
                        _ => break,
                    }
                }
                if total != 0 {
                    wait(&mut steps, &mut last, e.at);
                    steps.push(Action::Mouse { op: "scroll".into(), button: "left".into(), x: None, y: None, amount: total.clamp(-100, 100) });
                    last = Some(events[j - 1].at);
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    // Touches encore enfoncées à la fin : relâchées par la macro elle-même.
    (steps, cut)
}

#[cfg(windows)]
mod imp {
    use super::{push, rec, RawKind};
    use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VK_TO_CHAR};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, GetWindowThreadProcessId, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
        WindowFromPoint, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN,
        WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
        WM_SYSKEYUP,
    };

    /// Frappe ou clic simulé (par un logiciel) : ignoré.
    const LLKHF_INJECTED: u32 = 0x10;
    const LLMHF_INJECTED: u32 = 0x01;

    unsafe extern "system" fn kb_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let k = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            if k.flags.0 & LLKHF_INJECTED == 0 {
                match wparam.0 as u32 {
                    WM_KEYDOWN | WM_SYSKEYDOWN => push(RawKind::KeyDown(k.vkCode)),
                    WM_KEYUP | WM_SYSKEYUP => push(RawKind::KeyUp(k.vkCode)),
                    _ => {}
                }
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    /// Clic dans une fenêtre de HubForge (bouton « Arrêter »…) ?
    fn in_hubforge(pt: POINT) -> bool {
        unsafe {
            let w = WindowFromPoint(pt);
            if w.is_invalid() {
                return false;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(w, Some(&mut pid));
            pid == GetCurrentProcessId()
        }
    }

    unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let m = &*(lparam.0 as *const MSLLHOOKSTRUCT);
            if m.flags & LLMHF_INJECTED == 0 && !in_hubforge(m.pt) {
                let (x, y) = (m.pt.x, m.pt.y);
                match wparam.0 as u32 {
                    WM_LBUTTONDOWN => push(RawKind::MouseDown("left", x, y)),
                    WM_LBUTTONUP => push(RawKind::MouseUp("left", x, y)),
                    WM_RBUTTONDOWN => push(RawKind::MouseDown("right", x, y)),
                    WM_RBUTTONUP => push(RawKind::MouseUp("right", x, y)),
                    WM_MBUTTONDOWN => push(RawKind::MouseDown("middle", x, y)),
                    WM_MBUTTONUP => push(RawKind::MouseUp("middle", x, y)),
                    WM_MOUSEWHEEL => push(RawKind::Wheel(((m.mouseData >> 16) as u16 as i16) as i32)),
                    _ => {}
                }
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }

    pub fn start_hooks() -> Result<(), String> {
        let (tx, rx) = std::sync::mpsc::channel::<Result<u32, String>>();
        std::thread::Builder::new()
            .name("enregistreur".into())
            .spawn(move || unsafe {
                let hmod: HINSTANCE = GetModuleHandleW(None).map(|h| h.into()).unwrap_or_default();
                let kb = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_proc), Some(hmod), 0) {
                    Ok(h) => h,
                    Err(e) => {
                        let _ = tx.send(Err(format!("écoute du clavier impossible : {e}")));
                        return;
                    }
                };
                let ms = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), Some(hmod), 0).ok();
                let tid = GetCurrentThreadId();
                rec().lock().unwrap().thread_id = tid;
                let _ = tx.send(Ok(tid));
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
                let _ = UnhookWindowsHookEx(kb);
                if let Some(m) = ms {
                    let _ = UnhookWindowsHookEx(m);
                }
            })
            .map_err(|e| e.to_string())?;
        match rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => {
                rec().lock().unwrap().running = false;
                Err(e)
            }
            Err(_) => {
                rec().lock().unwrap().running = false;
                Err("l'enregistreur n'a pas démarré".into())
            }
        }
    }

    pub fn stop_hooks(tid: u32) {
        if tid != 0 {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// Caractère produit par une touche (ponctuation du clavier actuel).
    pub fn char_of(vk: u32) -> Option<char> {
        let c = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_CHAR) } & 0x7FFF_FFFF;
        char::from_u32(c).filter(|c| *c != '\0').map(|c| c.to_lowercase().next().unwrap_or(c))
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn start_hooks() -> Result<(), String> {
        super::rec().lock().unwrap().running = false;
        Err("l'enregistrement de macros n'existe que sous Windows".into())
    }

    pub fn stop_hooks(_tid: u32) {}

    pub fn char_of(_vk: u32) -> Option<char> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(kind: RawKind, ms: u64, t0: Instant) -> Raw {
        Raw { kind, at: t0 + Duration::from_millis(ms) }
    }

    #[test]
    fn taps_combos_and_clicks() {
        let t0 = Instant::now();
        let events = vec![
            ev(RawKind::KeyDown(0x11), 0, t0),    // ctrl enfoncé
            ev(RawKind::KeyDown(0x43), 100, t0),  // c
            ev(RawKind::KeyUp(0x43), 160, t0),
            ev(RawKind::KeyUp(0x11), 300, t0),
            ev(RawKind::MouseDown("left", 10, 20), 800, t0),
            ev(RawKind::MouseUp("left", 10, 20), 850, t0),
            ev(RawKind::Wheel(-120), 1000, t0),
            ev(RawKind::Wheel(-120), 1100, t0),
            ev(RawKind::KeyDown(0x41), 1500, t0), // a maintenu longtemps
            ev(RawKind::KeyUp(0x41), 2500, t0),
        ];
        let (steps, cut) = to_steps(&events);
        assert!(!cut);
        assert_eq!(
            steps,
            vec![
                Action::Key { keys: "ctrl".into(), op: "down".into() },
                Action::Wait { ms: 100 },
                Action::Hotkey { keys: "c".into() },
                Action::Wait { ms: 200 },
                Action::Key { keys: "ctrl".into(), op: "up".into() },
                Action::Wait { ms: 500 },
                Action::Mouse { op: "click".into(), button: "left".into(), x: Some(10), y: Some(20), amount: 0 },
                Action::Wait { ms: 200 },
                Action::Mouse { op: "scroll".into(), button: "left".into(), x: None, y: None, amount: 2 },
                Action::Wait { ms: 400 },
                Action::Key { keys: "a".into(), op: "down".into() },
                Action::Wait { ms: 1000 },
                Action::Key { keys: "a".into(), op: "up".into() },
            ]
        );
        // Les étapes produites sont valides.
        let m = Action::Macro { steps };
        crate::config::validate_action(&m, 0).unwrap();
    }

    #[test]
    fn long_recordings_are_cut() {
        let t0 = Instant::now();
        let events: Vec<Raw> = (0..400).flat_map(|i| [ev(RawKind::KeyDown(0x41), i * 100, t0), ev(RawKind::KeyUp(0x41), i * 100 + 30, t0)]).collect();
        let (steps, cut) = to_steps(&events);
        assert!(cut);
        assert!(steps.len() < MACRO_MAX_STEPS);
    }
}
