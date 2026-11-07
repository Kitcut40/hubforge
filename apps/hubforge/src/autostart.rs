//! Démarrage automatique avec Windows (clé « Run » de l'utilisateur, sans
//! droits administrateur).

#[cfg(windows)]
mod imp {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
    };

    const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    const NAME: PCWSTR = w!("HubForge");

    fn open(write: bool) -> Option<HKEY> {
        let mut key = HKEY::default();
        let access = if write { KEY_SET_VALUE | KEY_QUERY_VALUE } else { KEY_QUERY_VALUE };
        let r = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN, Some(0), access, &mut key) };
        (r == ERROR_SUCCESS).then_some(key)
    }

    pub fn is_enabled() -> bool {
        let Some(key) = open(false) else { return false };
        let r = unsafe { RegQueryValueExW(key, NAME, None, None, None, None) };
        unsafe {
            let _ = RegCloseKey(key);
        }
        r == ERROR_SUCCESS
    }

    pub fn set(enabled: bool) -> Result<(), String> {
        let key = open(true).ok_or("accès au registre impossible")?;
        let r = if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let cmd = format!("\"{}\" --hidden", exe.display());
            let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2) };
            unsafe { RegSetValueExW(key, NAME, Some(0), REG_SZ, Some(bytes)) }
        } else {
            let r = unsafe { RegDeleteValueW(key, NAME) };
            if r == windows::Win32::Foundation::ERROR_FILE_NOT_FOUND {
                ERROR_SUCCESS
            } else {
                r
            }
        };
        unsafe {
            let _ = RegCloseKey(key);
        }
        if r == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("erreur registre {}", r.0))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn is_enabled() -> bool {
        false
    }
    pub fn set(_enabled: bool) -> Result<(), String> {
        Err("démarrage automatique pas encore disponible sur cette plateforme".into())
    }
}

pub use imp::{is_enabled, set};
