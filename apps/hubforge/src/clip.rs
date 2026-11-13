//! Presse-papiers de Windows (texte seulement), pour l'action « IA sur la
//! sélection » : on copie la sélection, on la lit, puis on colle le résultat.

/// Numéro qui change à chaque modification du presse-papiers.
pub fn sequence() -> u32 {
    imp::sequence()
}

/// Texte du presse-papiers (None s'il n'y a pas de texte).
pub fn get_text() -> Option<String> {
    imp::get_text()
}

/// Remplace le contenu du presse-papiers par ce texte.
pub fn set_text(text: &str) -> bool {
    imp::set_text(text)
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    const CF_UNICODETEXT: u32 = 13;

    /// Ouvre le presse-papiers (quelques essais : une autre appli peut l'avoir).
    fn open() -> bool {
        for _ in 0..10 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    }

    pub fn sequence() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }

    pub fn get_text() -> Option<String> {
        if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) }.is_err() || !open() {
            return None;
        }
        let out = unsafe {
            match GetClipboardData(CF_UNICODETEXT) {
                Ok(h) if !h.is_invalid() => {
                    let g = HGLOBAL(h.0);
                    let p = GlobalLock(g) as *const u16;
                    if p.is_null() {
                        None
                    } else {
                        let mut len = 0usize;
                        while *p.add(len) != 0 && len < 8 * 1024 * 1024 {
                            len += 1;
                        }
                        let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
                        let _ = GlobalUnlock(g);
                        Some(s)
                    }
                }
                _ => None,
            }
        };
        unsafe {
            let _ = CloseClipboard();
        }
        out
    }

    pub fn set_text(text: &str) -> bool {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        if !open() {
            return false;
        }
        let ok = unsafe {
            let _ = EmptyClipboard();
            match GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) {
                Ok(g) => {
                    let p = GlobalLock(g) as *mut u16;
                    if p.is_null() {
                        let _ = GlobalFree(Some(g));
                        false
                    } else {
                        std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                        let _ = GlobalUnlock(g);
                        if SetClipboardData(CF_UNICODETEXT, Some(HANDLE(g.0))).is_ok() {
                            true
                        } else {
                            let _ = GlobalFree(Some(g));
                            false
                        }
                    }
                }
                Err(_) => false,
            }
        };
        unsafe {
            let _ = CloseClipboard();
        }
        ok
    }
}

#[cfg(not(windows))]
mod imp {
    use std::sync::Mutex;

    static CLIP: Mutex<(u32, Option<String>)> = Mutex::new((0, None));

    pub fn sequence() -> u32 {
        CLIP.lock().unwrap().0
    }

    pub fn get_text() -> Option<String> {
        CLIP.lock().unwrap().1.clone()
    }

    pub fn set_text(text: &str) -> bool {
        let mut c = CLIP.lock().unwrap();
        c.0 += 1;
        c.1 = Some(text.to_string());
        true
    }
}
