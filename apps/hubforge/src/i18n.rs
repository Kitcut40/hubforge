//! Langue de HubForge (français ou anglais) pour les textes dessinés côté
//! Rust : touches vivantes (dates, météo, micro…), menu de la zone de
//! notification. L'interface, elle, se traduit dans `ui/js/i18n.js`.

use std::sync::atomic::{AtomicBool, Ordering};

static EN: AtomicBool = AtomicBool::new(false);

/// Langues proposées.
pub const LANGS: [&str; 2] = ["fr", "en"];

pub fn set(lang: &str) {
    EN.store(lang == "en", Ordering::Relaxed);
}

pub fn en() -> bool {
    EN.load(Ordering::Relaxed)
}

/// Le texte dans la langue choisie.
pub fn t(fr: &'static str, en: &'static str) -> &'static str {
    if self::en() {
        en
    } else {
        fr
    }
}

/// Langue enregistrée, ou celle du système au premier lancement.
pub fn resolve(saved: &str) -> String {
    if LANGS.contains(&saved) {
        return saved.to_string();
    }
    if system_is_french() {
        "fr".into()
    } else {
        "en".into()
    }
}

fn system_is_french() -> bool {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if !v.is_empty() {
                return v.to_lowercase().starts_with("fr");
            }
        }
    }
    imp::user_locale().map(|l| l.to_lowercase().starts_with("fr")).unwrap_or(true)
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;

    pub fn user_locale() -> Option<String> {
        let mut buf = [0u16; 85];
        let n = unsafe { GetUserDefaultLocaleName(&mut buf) };
        (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn user_locale() -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    // La langue est globale : on ne la change pas ici, d'autres tests
    // tournent en même temps et attendent le français.
    #[test]
    fn resolve_keeps_a_saved_language() {
        assert_eq!(super::resolve("en"), "en");
        assert_eq!(super::resolve("fr"), "fr");
        assert_eq!(super::t("Bonjour", "Hello"), "Bonjour");
    }
}
