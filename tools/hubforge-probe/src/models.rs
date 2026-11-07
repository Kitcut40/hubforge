//! Table des modules MasterHUB connus (voir docs/PROTOCOLE.md, section 2).

pub const VID_COOLER_MASTER: u16 = 0x2516;
pub const VID_BASE_HUB: u16 = 0x067B;
pub const PID_BASE_HUB: u16 = 0x2586;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Keys15,
    Faders5,
    Rollers2,
    Encoder,
    Knobs3,
    BaseLed,
    Keypad4,
    Screen4300,
    Screen4600,
    Bar5000,
    BaseHub,
    Unknown,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Keys15 => "15 touches à écran (M4315)",
            Kind::Faders5 => "5 faders (M005S)",
            Kind::Rollers2 => "2 molettes (M002)",
            Kind::Encoder => "Encodeur à écran (M131)",
            Kind::Knobs3 => "3 knobs (M003)",
            Kind::BaseLed => "LED de la base (M0x64)",
            Kind::Keypad4 => "Pavé 4 touches (M3704)",
            Kind::Screen4300 => "Écran (M4300)",
            Kind::Screen4600 => "Module à écran (M4600)",
            Kind::Bar5000 => "Barre à écran (M5000)",
            Kind::BaseHub => "Hub USB de la base",
            Kind::Unknown => "Inconnu",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Kind::Keys15 => "15T",
            Kind::Faders5 => "FAD",
            Kind::Rollers2 => "MOL",
            Kind::Encoder => "ENC",
            Kind::Knobs3 => "KNB",
            Kind::BaseLed => "BAS",
            Kind::Keypad4 => "K4",
            Kind::Screen4300 => "S43",
            Kind::Screen4600 => "S46",
            Kind::Bar5000 => "BAR",
            Kind::BaseHub => "HUB",
            Kind::Unknown => "???",
        }
    }

    /// Cible LED de la famille M0 (octet "CC" de la commande 0x2D/0x2E/0x2C).
    pub fn m0_led_target(self) -> Option<(u8, u8)> {
        match self {
            Kind::BaseLed => Some((0, 0)),
            Kind::Faders5 => Some((1, 0xFF)),
            Kind::Knobs3 => Some((2, 0xFF)),
            _ => None,
        }
    }
}

pub fn identify(vid: u16, pid: u16) -> Kind {
    if vid == VID_BASE_HUB && pid == PID_BASE_HUB {
        return Kind::BaseHub;
    }
    if vid != VID_COOLER_MASTER {
        return Kind::Unknown;
    }
    match pid {
        0x9997 | 0x01FE => Kind::Keys15,
        0x9991 | 0x0200 => Kind::Faders5,
        0x9995 | 0x0204 => Kind::Rollers2,
        0x9994 | 0x0206 => Kind::Encoder,
        0x9996 | 0x0202 => Kind::Knobs3,
        0x9999 | 0x01F6 => Kind::BaseLed,
        0x9989 => Kind::Keypad4,
        0x9998 => Kind::Screen4300,
        0x9993 => Kind::Screen4600,
        0x9992 => Kind::Bar5000,
        _ => Kind::Unknown,
    }
}

pub fn is_masterhub(vid: u16, pid: u16) -> bool {
    vid == VID_COOLER_MASTER || (vid == VID_BASE_HUB && pid == PID_BASE_HUB)
}
