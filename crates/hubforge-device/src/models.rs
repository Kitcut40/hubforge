//! Identification des modules MasterHUB (voir docs/PROTOCOLE.md, section 2).

pub const VID_COOLER_MASTER: u16 = 0x2516;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// 15 touches à écran (M4315)
    Keys15,
    /// 5 faders (M005S)
    Faders5,
    /// 2 molettes (M002)
    Rollers2,
    /// LED de la base (M0x64)
    BaseLed,
    /// Encodeur à écran (M131) — non testé
    Encoder,
    /// 3 knobs (M003) — non testé
    Knobs3,
}

impl Kind {
    pub fn from_pid(pid: u16) -> Option<Kind> {
        Some(match pid {
            0x01FE | 0x9997 => Kind::Keys15,
            0x0200 | 0x9991 => Kind::Faders5,
            0x0204 | 0x9995 => Kind::Rollers2,
            0x01F6 | 0x9999 => Kind::BaseLed,
            0x0206 | 0x9994 => Kind::Encoder,
            0x0202 | 0x9996 => Kind::Knobs3,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Keys15 => "15 touches",
            Kind::Faders5 => "5 faders",
            Kind::Rollers2 => "2 molettes",
            Kind::BaseLed => "LED de la base",
            Kind::Encoder => "Encodeur",
            Kind::Knobs3 => "3 knobs",
        }
    }

    /// Cible LED (famille M0) : (cible, index « toutes »).
    pub fn led_target(self) -> Option<u8> {
        match self {
            Kind::BaseLed => Some(0),
            Kind::Faders5 => Some(1),
            Kind::Knobs3 => Some(2),
            _ => None,
        }
    }
}
