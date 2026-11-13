//! État du PC pour la veille et les touches vivantes : inactivité, session
//! verrouillée, débit réseau, espace disque.

/// Secondes depuis la dernière action au clavier ou à la souris.
pub fn idle_seconds() -> u64 {
    imp::idle_seconds()
}

/// La session Windows est-elle verrouillée (écran de connexion) ?
pub fn is_locked() -> bool {
    imp::is_locked()
}

/// Octets reçus et envoyés depuis le démarrage, toutes cartes réseau actives.
pub fn net_bytes() -> Option<(u64, u64)> {
    imp::net_bytes()
}

/// Espace d'un disque : (Go utilisés, Go au total). `drive` : « C », « D:»…
pub fn disk(drive: &str) -> Option<(f32, f32)> {
    let letter = drive.trim().chars().next().filter(|c| c.is_ascii_alphabetic())?.to_ascii_uppercase();
    imp::disk(letter)
}

/// Débit en texte court : « 12,4 Mo/s », « 850 Ko/s ».
pub fn rate_text(bytes_per_s: f64) -> String {
    let en = crate::i18n::en();
    let fr = |v: f64, unit: &str| {
        let s = if v >= 100.0 { format!("{v:.0}") } else if v >= 10.0 { format!("{v:.1}") } else { format!("{v:.2}") };
        format!("{} {unit}", if en { s } else { s.replace('.', ",") })
    };
    match bytes_per_s {
        b if b >= 1e9 => fr(b / 1e9, if en { "GB/s" } else { "Go/s" }),
        b if b >= 1e6 => fr(b / 1e6, if en { "MB/s" } else { "Mo/s" }),
        b if b >= 1e3 => fr(b / 1e3, if en { "KB/s" } else { "Ko/s" }),
        b => format!("{} {}", b.round(), if en { "B/s" } else { "o/s" }),
    }
}

#[cfg(windows)]
mod imp {
    use windows::core::PCWSTR;
    use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows::Win32::System::StationsAndDesktops::{CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, UOI_NAME};
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    pub fn idle_seconds() -> u64 {
        let mut li = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        unsafe {
            if !GetLastInputInfo(&mut li).as_bool() {
                return 0;
            }
            (GetTickCount().wrapping_sub(li.dwTime) / 1000) as u64
        }
    }

    pub fn is_locked() -> bool {
        unsafe {
            // Quand la session est verrouillée, le bureau qui reçoit le clavier
            // n'est plus « Default » (ou n'est pas accessible du tout).
            let Ok(desk) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS) else { return true };
            let mut buf = [0u16; 64];
            let mut needed = 0u32;
            let ok = GetUserObjectInformationW(
                windows::Win32::Foundation::HANDLE(desk.0),
                UOI_NAME,
                Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                (buf.len() * 2) as u32,
                Some(&mut needed),
            )
            .is_ok();
            let _ = CloseDesktop(desk);
            if !ok {
                return false;
            }
            let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
            !String::from_utf16_lossy(&buf[..end]).eq_ignore_ascii_case("Default")
        }
    }

    pub fn net_bytes() -> Option<(u64, u64)> {
        unsafe {
            let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
            if GetIfTable2(&mut table).is_err() || table.is_null() {
                return None;
            }
            let t = &*table;
            let rows = std::slice::from_raw_parts(t.Table.as_ptr(), t.NumEntries as usize);
            let (mut rx, mut tx) = (0u64, 0u64);
            for r in rows {
                // 24 = boucle locale ; on ne compte que les cartes connectées et
                // matérielles (pas les adaptateurs virtuels en double).
                let hardware = r.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
                if r.Type == 24 || r.OperStatus != IfOperStatusUp || !hardware {
                    continue;
                }
                rx += r.InOctets;
                tx += r.OutOctets;
            }
            FreeMibTable(table as *const core::ffi::c_void);
            Some((rx, tx))
        }
    }

    pub fn disk(letter: char) -> Option<(f32, f32)> {
        let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain(std::iter::once(0)).collect();
        let (mut free, mut total) = (0u64, 0u64);
        unsafe { GetDiskFreeSpaceExW(PCWSTR(root.as_ptr()), None, Some(&mut total), Some(&mut free)) }.ok()?;
        let gb = 1024.0 * 1024.0 * 1024.0;
        Some(((total - free) as f32 / gb, total as f32 / gb))
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn idle_seconds() -> u64 {
        0
    }

    pub fn is_locked() -> bool {
        false
    }

    pub fn net_bytes() -> Option<(u64, u64)> {
        let text = std::fs::read_to_string("/proc/net/dev").ok()?;
        let (mut rx, mut tx) = (0u64, 0u64);
        for line in text.lines().skip(2) {
            let Some((name, rest)) = line.split_once(':') else { continue };
            if name.trim() == "lo" {
                continue;
            }
            let v: Vec<u64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if v.len() >= 9 {
                rx += v[0];
                tx += v[8];
            }
        }
        Some((rx, tx))
    }

    pub fn disk(_letter: char) -> Option<(f32, f32)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_are_readable() {
        assert_eq!(rate_text(12_400_000.0), "12,4 Mo/s");
        assert_eq!(rate_text(850_000.0), "850 Ko/s");
        assert_eq!(rate_text(1_500.0), "1,50 Ko/s");
        assert_eq!(rate_text(12.0), "12 o/s");
    }
}
