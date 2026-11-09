//! Exécution des actions : raccourcis, texte, lancement, médias, défilement.

use enigo::{Axis, Direction, Enigo, Key, Keyboard, Mouse, Settings};

pub struct Input {
    enigo: Option<Enigo>,
}

impl Input {
    pub fn new() -> Input {
        let enigo = match Enigo::new(&Settings::default()) {
            Ok(e) => Some(e),
            Err(e) => {
                log::error!("simulation clavier indisponible : {e}");
                None
            }
        };
        Input { enigo }
    }

    pub fn hotkey(&mut self, combo: &str) {
        let Some(e) = self.enigo.as_mut() else { return };
        let keys: Vec<Key> = match combo.split('+').map(|t| parse_key(t.trim())).collect::<Option<Vec<_>>>() {
            Some(k) if !k.is_empty() => k,
            _ => {
                log::warn!("raccourci non reconnu : « {combo} »");
                return;
            }
        };
        for k in &keys {
            let _ = e.key(*k, Direction::Press);
        }
        for k in keys.iter().rev() {
            let _ = e.key(*k, Direction::Release);
        }
    }

    /// Enfonce les touches d'une combinaison (sans les relâcher).
    pub fn press(&mut self, combo: &str) -> bool {
        let Some(e) = self.enigo.as_mut() else { return false };
        let Some(keys) = combo.split('+').map(|t| parse_key(t.trim())).collect::<Option<Vec<_>>>().filter(|k| !k.is_empty()) else {
            log::warn!("raccourci non reconnu : « {combo} »");
            return false;
        };
        for k in &keys {
            let _ = e.key(*k, Direction::Press);
        }
        true
    }

    /// Relâche les touches d'une combinaison enfoncée avec `press`.
    pub fn release(&mut self, combo: &str) {
        let Some(e) = self.enigo.as_mut() else { return };
        if let Some(keys) = combo.split('+').map(|t| parse_key(t.trim())).collect::<Option<Vec<_>>>() {
            for k in keys.iter().rev() {
                let _ = e.key(*k, Direction::Release);
            }
        }
    }

    pub fn tap(&mut self, key: Key) {
        if let Some(e) = self.enigo.as_mut() {
            let _ = e.key(key, Direction::Click);
        }
    }

    pub fn text(&mut self, text: &str) {
        if let Some(e) = self.enigo.as_mut() {
            if let Err(err) = e.text(text) {
                log::warn!("saisie de texte : {err}");
            }
        }
    }

    pub fn scroll(&mut self, amount: i32, horizontal: bool) {
        if let Some(e) = self.enigo.as_mut() {
            let axis = if horizontal { Axis::Horizontal } else { Axis::Vertical };
            let _ = e.scroll(amount, axis);
        }
    }

    pub fn zoom(&mut self, amount: i32) {
        if let Some(e) = self.enigo.as_mut() {
            let _ = e.key(Key::Control, Direction::Press);
            let _ = e.scroll(amount, Axis::Vertical);
            let _ = e.key(Key::Control, Direction::Release);
        }
    }

    pub fn media(&mut self, command: &str) {
        let key = match command {
            "play_pause" => Key::MediaPlayPause,
            "next" => Key::MediaNextTrack,
            "previous" => Key::MediaPrevTrack,
            "stop" => Key::MediaStop,
            "mute" => Key::VolumeMute,
            "volume_up" => Key::VolumeUp,
            "volume_down" => Key::VolumeDown,
            _ => {
                log::warn!("commande multimédia inconnue : « {command} »");
                return;
            }
        };
        self.tap(key);
    }
}

/// Convertit un nom de touche (« ctrl », « s », « f5 »…) en touche.
pub fn parse_key(name: &str) -> Option<Key> {
    let n = name.to_lowercase();
    Some(match n.as_str() {
        "ctrl" | "control" | "ctl" => Key::Control,
        "shift" | "maj" => Key::Shift,
        "alt" => Key::Alt,
        "win" | "windows" | "super" | "meta" | "cmd" => Key::Meta,
        "enter" | "return" | "entree" | "entrée" => Key::Return,
        "esc" | "escape" | "echap" | "échap" => Key::Escape,
        "tab" => Key::Tab,
        "space" | "espace" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" | "del" | "suppr" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdn" => Key::PageDown,
        "up" | "haut" => Key::UpArrow,
        "down" | "bas" => Key::DownArrow,
        "left" | "gauche" => Key::LeftArrow,
        "right" | "droite" => Key::RightArrow,
        "printscreen" | "prtsc" | "impr" => Key::PrintScr,
        "insert" | "ins" | "inser" => insert_key(),
        "plus" => Key::Unicode('+'),
        _ => {
            if let Some(num) = n.strip_prefix('f').and_then(|d| d.parse::<u32>().ok()) {
                return function_key(num);
            }
            let mut chars = n.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            return Some(char_key(c));
        }
    })
}

#[cfg(windows)]
fn function_key(n: u32) -> Option<Key> {
    (1..=24).contains(&n).then(|| Key::Other(0x70 + n - 1))
}

#[cfg(not(windows))]
fn function_key(n: u32) -> Option<Key> {
    Some(match n {
        1 => Key::F1,
        2 => Key::F2,
        3 => Key::F3,
        4 => Key::F4,
        5 => Key::F5,
        6 => Key::F6,
        7 => Key::F7,
        8 => Key::F8,
        9 => Key::F9,
        10 => Key::F10,
        11 => Key::F11,
        12 => Key::F12,
        _ => return None,
    })
}

#[cfg(windows)]
fn insert_key() -> Key {
    Key::Other(0x2D) // VK_INSERT
}

#[cfg(not(windows))]
fn insert_key() -> Key {
    Key::Insert
}

/// Lettres et chiffres : sous Windows on utilise le code de touche virtuel,
/// identique en AZERTY et QWERTY (Ctrl+S reste Ctrl+S). Ponctuation (« , »,
/// « . », « / »…) : la touche qui produit ce caractère sur le clavier
/// actuel, pour que les jeux la reconnaissent (ils ignorent souvent les
/// caractères envoyés directement).
#[cfg(windows)]
fn char_key(c: char) -> Key {
    match c {
        'a'..='z' => Key::Other(0x41 + (c as u32 - 'a' as u32)),
        '0'..='9' => Key::Other(0x30 + (c as u32 - '0' as u32)),
        _ => {
            let mut buf = [0u16; 2];
            let units = c.encode_utf16(&mut buf);
            if units.len() == 1 {
                let r = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::VkKeyScanW(units[0]) };
                let vk = (r as u16) & 0xFF;
                // Seulement si le caractère s'obtient sans Maj/Ctrl/Alt.
                if r != -1 && (r as u16) >> 8 == 0 && vk != 0 {
                    return Key::Other(vk as u32);
                }
            }
            Key::Unicode(c)
        }
    }
}

#[cfg(not(windows))]
fn char_key(c: char) -> Key {
    Key::Unicode(c)
}

/// Lance une application, un fichier, un dossier ou une URL.
pub fn launch(target: &str, args: &[String]) {
    log::info!("lancement : {target}");
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let params = args
            .iter()
            .map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() })
            .collect::<Vec<_>>()
            .join(" ");
        let r = unsafe {
            ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(target), &HSTRING::from(params), None, SW_SHOWNORMAL)
        };
        if (r.0 as isize) <= 32 {
            log::warn!("impossible de lancer « {target} » (code {})", r.0 as isize);
        }
    }
    #[cfg(not(windows))]
    {
        let res = if args.is_empty() {
            std::process::Command::new("xdg-open").arg(target).spawn()
        } else {
            std::process::Command::new(target).args(args).spawn()
        };
        if let Err(e) = res {
            log::warn!("impossible de lancer « {target} » : {e}");
        }
    }
}

pub fn lock_session() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::Shutdown::LockWorkStation();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("loginctl").arg("lock-session").spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hotkeys() {
        for combo in ["ctrl+shift+esc", "win+shift+s", "ctrl+c", "alt+f4", "f5"] {
            assert!(combo.split('+').all(|t| parse_key(t).is_some()), "{combo}");
        }
        assert!(parse_key("nimportequoi").is_none());
    }
}
