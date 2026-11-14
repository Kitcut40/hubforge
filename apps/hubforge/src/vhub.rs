//! MasterHUB virtuel : une petite fenêtre qui montre les 15 touches telles
//! qu'elles sont affichées sur le module, et où un clic vaut un appui.
//! Option des réglages, désactivée par défaut.
//!
//! Le moteur note ici chaque image envoyée au module (même sans MasterHUB
//! branché) ; la fenêtre les demande avec leur numéro de version.

use base64::Engine as _;
use serde::Serialize;
use std::sync::{Arc, Mutex, OnceLock};

pub const KEYS: usize = 15;

struct Screen {
    version: u64,
    keys: Vec<(u64, Option<Arc<Vec<u8>>>)>,
    rot: u16,
    page: String,
}

fn screen() -> &'static Mutex<Screen> {
    static S: OnceLock<Mutex<Screen>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Screen { version: 0, keys: vec![(0, None); KEYS], rot: 0, page: String::new() }))
}

/// Image envoyée à une touche (déjà tournée de `rot` degrés pour le module).
pub fn set_key(key: u8, jpeg: &Arc<Vec<u8>>, rot: u16) {
    let mut s = screen().lock().unwrap();
    let Some(slot) = s.keys.get(key as usize) else { return };
    if slot.1.as_ref().is_some_and(|j| Arc::ptr_eq(j, jpeg)) && s.rot == rot {
        return;
    }
    s.version += 1;
    let v = s.version;
    s.keys[key as usize] = (v, Some(jpeg.clone()));
    s.rot = rot;
}

pub fn set_page(name: &str) {
    let mut s = screen().lock().unwrap();
    if s.page != name {
        s.page = name.to_string();
        s.version += 1;
    }
}

/// Touches changées depuis la version `since` (images en base64).
#[derive(Debug, Clone, Serialize)]
pub struct Frame {
    pub version: u64,
    pub rot: u16,
    pub page: String,
    /// (touche 0 à 14, image JPEG en base64)
    pub keys: Vec<(u8, String)>,
}

pub fn frame(since: u64) -> Frame {
    let s = screen().lock().unwrap();
    let keys = s
        .keys
        .iter()
        .enumerate()
        .filter(|(_, (v, j))| *v > since && j.is_some())
        .map(|(i, (_, j))| (i as u8, base64::engine::general_purpose::STANDARD.encode(j.as_deref().unwrap().as_slice())))
        .collect();
    Frame { version: s.version, rot: s.rot, page: s.page.clone(), keys }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_changed_keys_are_sent() {
        let a = Arc::new(vec![1u8, 2, 3]);
        set_key(2, &a, 0);
        let f = frame(0);
        assert!(f.keys.iter().any(|(k, _)| *k == 2));
        let v = f.version;
        set_key(2, &a, 0); // même image : rien ne change
        assert_eq!(frame(v).keys.len(), 0);
        set_key(3, &Arc::new(vec![4u8]), 0);
        let f2 = frame(v);
        assert_eq!(f2.keys.len(), 1);
        assert_eq!(f2.keys[0].0, 3);
    }
}
