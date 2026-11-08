//! Données des jeux de course, pour faire réagir les LED.
//!
//! Assetto Corsa publie l'état de la voiture dans trois zones de mémoire
//! partagée (« acpmf_physics », « acpmf_graphics », « acpmf_static »),
//! prévues pour les applications externes (tableaux de bord, simulateurs…).
//! HubForge les lit seulement ; il n'écrit rien et ne touche pas au jeu.

/// Ce qu'on retient du jeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Telemetry {
    /// En piste (ni menu, ni pause, ni ralenti).
    pub live: bool,
    pub rpm: i32,
    pub max_rpm: i32,
    /// Rapport engagé : 0 = marche arrière, 1 = point mort, 2 = première…
    pub gear: i32,
    /// Drapeau : 0 aucun, 1 bleu, 2 jaune, 3 noir, 4 blanc, 5 damier, 6 pénalité.
    pub flag: i32,
}

/// Emplacements (octets) des champs utiles dans les zones partagées
/// d'Assetto Corsa (structures alignées sur 4 octets).
pub mod layout {
    pub const PHYSICS_GEAR: usize = 16;
    pub const PHYSICS_RPM: usize = 20;
    pub const GRAPHICS_STATUS: usize = 4;
    pub const GRAPHICS_FLAG: usize = 268;
    pub const STATIC_MAX_RPM: usize = 412;
    /// Statut « en piste ».
    pub const STATUS_LIVE: i32 = 2;
}

fn read_i32(buf: &[u8], at: usize) -> i32 {
    buf.get(at..at + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]])).unwrap_or(0)
}

/// Décode les trois zones (copies brutes) en [`Telemetry`]. `flags` = false
/// pour les jeux dont la zone « graphics » est organisée autrement (ACC).
pub fn decode(physics: &[u8], graphics: &[u8], stat: &[u8], flags: bool) -> Telemetry {
    let status = read_i32(graphics, layout::GRAPHICS_STATUS);
    let max_rpm = read_i32(stat, layout::STATIC_MAX_RPM);
    Telemetry {
        live: status == layout::STATUS_LIVE,
        rpm: read_i32(physics, layout::PHYSICS_RPM),
        max_rpm: if (500..30_000).contains(&max_rpm) { max_rpm } else { 0 },
        gear: read_i32(physics, layout::PHYSICS_GEAR),
        flag: if flags { read_i32(graphics, layout::GRAPHICS_FLAG).clamp(0, 6) } else { 0 },
    }
}

/// Lecteur des données d'Assetto Corsa. Ouvre les zones quand le jeu tourne,
/// les referme quand il s'arrête.
#[derive(Default)]
pub struct AcReader {
    imp: imp::Maps,
    next_try: Option<std::time::Instant>,
    /// Régime le plus haut vu (si le jeu ne donne pas le maximum).
    seen_max: i32,
}

impl AcReader {
    /// Lit l'état actuel. `acc` = le jeu au premier plan est ACC (drapeaux
    /// non pris en charge). None = jeu fermé.
    pub fn poll(&mut self, acc: bool) -> Option<Telemetry> {
        let now = std::time::Instant::now();
        if !self.imp.is_open() {
            if self.next_try.map(|t| now < t).unwrap_or(false) {
                return None;
            }
            self.next_try = Some(now + std::time::Duration::from_secs(2));
            if !self.imp.open() {
                return None;
            }
            self.seen_max = 0;
        }
        let (p, g, s) = self.imp.read()?;
        let mut tel = decode(&p, &g, &s, !acc);
        if tel.live {
            self.seen_max = self.seen_max.max(tel.rpm);
        }
        if tel.max_rpm == 0 && self.seen_max > 2000 {
            tel.max_rpm = self.seen_max;
        }
        Some(tel)
    }
}

#[cfg(windows)]
mod imp {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Memory::{MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS};

    /// Tailles lues : assez pour les champs utilisés.
    const SIZES: [usize; 3] = [64, 512, 512];
    const NAMES: [&str; 3] = ["Local\\acpmf_physics", "Local\\acpmf_graphics", "Local\\acpmf_static"];

    struct View {
        handle: HANDLE,
        addr: MEMORY_MAPPED_VIEW_ADDRESS,
        size: usize,
    }

    #[derive(Default)]
    pub struct Maps {
        views: Vec<View>,
    }

    // Les vues ne sont utilisées que par le fil du moteur.
    unsafe impl Send for Maps {}

    impl Maps {
        pub fn is_open(&self) -> bool {
            self.views.len() == 3
        }

        pub fn open(&mut self) -> bool {
            self.close();
            for (name, size) in NAMES.iter().zip(SIZES) {
                let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
                unsafe {
                    let Ok(h) = OpenFileMappingW(FILE_MAP_READ.0, false, PCWSTR(wide.as_ptr())) else {
                        self.close();
                        return false;
                    };
                    let addr = MapViewOfFile(h, FILE_MAP_READ, 0, 0, size);
                    if addr.Value.is_null() {
                        let _ = CloseHandle(h);
                        self.close();
                        return false;
                    }
                    self.views.push(View { handle: h, addr, size });
                }
            }
            true
        }

        pub fn read(&mut self) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
            if !self.is_open() {
                return None;
            }
            let copy = |v: &View| unsafe { std::slice::from_raw_parts(v.addr.Value as *const u8, v.size).to_vec() };
            Some((copy(&self.views[0]), copy(&self.views[1]), copy(&self.views[2])))
        }

        fn close(&mut self) {
            for v in self.views.drain(..) {
                unsafe {
                    let _ = UnmapViewOfFile(v.addr);
                    let _ = CloseHandle(v.handle);
                }
            }
        }
    }

    impl Drop for Maps {
        fn drop(&mut self) {
            self.close();
        }
    }
}

#[cfg(not(windows))]
mod imp {
    #[derive(Default)]
    pub struct Maps;
    impl Maps {
        pub fn is_open(&self) -> bool {
            false
        }
        pub fn open(&mut self) -> bool {
            false
        }
        pub fn read(&mut self) -> Option<(Vec<u8>, Vec<u8>, Vec<u8>)> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_fields_at_their_offsets() {
        let mut p = vec![0u8; 64];
        let mut g = vec![0u8; 512];
        let mut s = vec![0u8; 512];
        p[layout::PHYSICS_RPM..layout::PHYSICS_RPM + 4].copy_from_slice(&7350i32.to_le_bytes());
        p[layout::PHYSICS_GEAR..layout::PHYSICS_GEAR + 4].copy_from_slice(&4i32.to_le_bytes());
        g[layout::GRAPHICS_STATUS..layout::GRAPHICS_STATUS + 4].copy_from_slice(&2i32.to_le_bytes());
        g[layout::GRAPHICS_FLAG..layout::GRAPHICS_FLAG + 4].copy_from_slice(&2i32.to_le_bytes());
        s[layout::STATIC_MAX_RPM..layout::STATIC_MAX_RPM + 4].copy_from_slice(&8000i32.to_le_bytes());
        let t = decode(&p, &g, &s, true);
        assert_eq!(t, Telemetry { live: true, rpm: 7350, max_rpm: 8000, gear: 4, flag: 2 });
        assert_eq!(decode(&p, &g, &s, false).flag, 0);
    }
}
