//! Construction des commandes et décodage des rapports HID.
//! Référence : docs/PROTOCOLE.md (v0.3).

/// Taille des paquets d'image du module 15 touches (Report ID inclus).
pub const KEYS15_IMAGE_PACKET: usize = 320;
/// Taille des rapports de sortie des modules de la famille M0.
pub const M0_OUTPUT_LEN: usize = 64;
/// Taille des feature reports.
pub const FEATURE_LEN: usize = 32;

pub const KEY_COUNT: usize = 15;
pub const FADER_COUNT: usize = 5;
pub const KEY_IMAGE_SIZE: u32 = 72;

/// Rapport d'entrée décodé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// État des 15 touches (true = enfoncée).
    Keys([bool; KEY_COUNT]),
    /// Position des 5 faders (0 = bas, 100 = haut).
    Faders([u8; FADER_COUNT]),
    /// Un cran de molette. `right` / `left` : None = pas de mouvement,
    /// Some(true) = sens « 01 », Some(false) = sens « 00 ».
    Rollers { right: Option<bool>, left: Option<bool> },
    /// Signal périodique de la base.
    Heartbeat,
    Unknown,
}

pub fn parse_input(report: &[u8]) -> Input {
    if report.len() < 4 || report[0] != 0x01 {
        return Input::Unknown;
    }
    let len = u16::from_le_bytes([report[2], report[3]]) as usize;
    let payload = &report[4..report.len().min(4 + len)];
    match report[1] {
        0x34 if payload.len() >= 2 + KEY_COUNT && payload[1] as usize == KEY_COUNT => {
            let mut keys = [false; KEY_COUNT];
            for (i, k) in keys.iter_mut().enumerate() {
                *k = payload[2 + i] != 0;
            }
            Input::Keys(keys)
        }
        0x2A if payload.len() >= 2 + FADER_COUNT && payload[0] == 0x02 && payload[1] == 0x05 => {
            let mut f = [0u8; FADER_COUNT];
            f.copy_from_slice(&payload[2..2 + FADER_COUNT]);
            Input::Faders(f)
        }
        0x2A if payload.len() >= 4 && payload[0] == 0x01 && payload[1] == 0x02 => {
            let dir = |b: u8| match b {
                0xFF => None,
                0x01 => Some(true),
                _ => Some(false),
            };
            Input::Rollers { right: dir(payload[2]), left: dir(payload[3]) }
        }
        0x2B => Input::Heartbeat,
        _ => Input::Unknown,
    }
}

fn feature(cmd: u8, params: &[u8]) -> Vec<u8> {
    let mut d = vec![0u8; FEATURE_LEN];
    d[0] = 0x03;
    d[1] = cmd;
    d[2..2 + params.len()].copy_from_slice(params);
    d
}

/// Mode logiciel (true = les écrans et les appuis sont gérés par le PC).
pub fn keys15_sw_mode(on: bool) -> Vec<u8> {
    feature(0x3A, &[on as u8])
}

/// Luminosité des écrans du module 15 touches (0–100).
pub fn keys15_brightness(value: u8) -> Vec<u8> {
    feature(0x34, &[value.min(100)])
}

/// Couleur unie sur une touche.
pub fn keys15_color(key: u8, r: u8, g: u8, b: u8) -> Vec<u8> {
    feature(0x37, &[key, r, g, b])
}

/// Découpe une image JPEG en paquets pour une touche (commande 0x36).
pub fn keys15_image_packets(key: u8, jpeg: &[u8]) -> Vec<Vec<u8>> {
    let chunk = KEYS15_IMAGE_PACKET - 8;
    let total = jpeg.len().div_ceil(chunk).max(1);
    let mut out = Vec::with_capacity(total);
    for index in 0..total {
        let start = index * chunk;
        let part = &jpeg[start.min(jpeg.len())..(start + chunk).min(jpeg.len())];
        let mut p = vec![0u8; KEYS15_IMAGE_PACKET];
        p[0] = 0x02;
        p[1] = 0x36;
        p[2..4].copy_from_slice(&(index as u16).to_le_bytes());
        p[4..6].copy_from_slice(&(part.len() as u16).to_le_bytes());
        p[6] = key;
        p[7] = (index + 1 == total) as u8;
        p[8..8 + part.len()].copy_from_slice(part);
        out.push(p);
    }
    out
}

/// Couleur des LED (famille M0). `index` = 0xFF pour toutes.
pub fn led_color(target: u8, index: u8, r: u8, g: u8, b: u8, save: bool) -> Vec<u8> {
    let mut d = vec![0u8; M0_OUTPUT_LEN];
    d[..8].copy_from_slice(&[0x02, 0x2D, target, index, r, g, b, save as u8]);
    d
}

/// Luminosité des LED (famille M0).
pub fn led_brightness(target: u8, index: u8, value: u8) -> Vec<u8> {
    let mut d = vec![0u8; M0_OUTPUT_LEN];
    d[..5].copy_from_slice(&[0x02, 0x2E, target, index, value.min(100)]);
    d
}

/// Demande de lecture des faders (feature report 0x33). Retourne le tampon à
/// passer à `get_feature_report`.
pub fn faders_query() -> Vec<u8> {
    let mut d = vec![0u8; FEATURE_LEN + 1];
    d[0] = 0x33;
    d
}

pub fn parse_faders_query(resp: &[u8]) -> Option<[u8; FADER_COUNT]> {
    if resp.len() >= 2 + FADER_COUNT && resp[0] == 0x33 && resp[1] as usize == FADER_COUNT {
        let mut f = [0u8; FADER_COUNT];
        f.copy_from_slice(&resp[2..2 + FADER_COUNT]);
        Some(f)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    // Rapports réellement reçus lors des tests du 07/11/2026.
    #[test]
    fn keys_report() {
        let mut r = hex("01 34 11 00 02 0F 00 01");
        r.resize(64, 0);
        let Input::Keys(k) = parse_input(&r) else { panic!() };
        assert!(!k[0] && k[1] && !k[14]);
    }

    #[test]
    fn faders_report() {
        let mut r = hex("01 2A 07 00 02 05 64 2C 2F 2E 00");
        r.resize(64, 0);
        assert_eq!(parse_input(&r), Input::Faders([100, 44, 47, 46, 0]));
    }

    #[test]
    fn rollers_report() {
        let mut r = hex("01 2A 04 00 01 02 FF 01");
        r.resize(64, 0);
        assert_eq!(parse_input(&r), Input::Rollers { right: None, left: Some(true) });
        let mut r = hex("01 2A 04 00 01 02 00 FF");
        r.resize(64, 0);
        assert_eq!(parse_input(&r), Input::Rollers { right: Some(false), left: None });
    }

    #[test]
    fn heartbeat() {
        let mut r = hex("01 2B 01");
        r.resize(64, 0);
        assert_eq!(parse_input(&r), Input::Heartbeat);
    }

    #[test]
    fn image_packets() {
        let jpeg: Vec<u8> = (0..700u32).map(|i| i as u8).collect();
        let p = keys15_image_packets(4, &jpeg);
        assert_eq!(p.len(), 3);
        assert!(p.iter().all(|x| x.len() == 320));
        assert_eq!(&p[0][..8], &[0x02, 0x36, 0, 0, 0x38, 0x01, 4, 0]);
        assert_eq!(&p[2][..8], &[0x02, 0x36, 2, 0, 76, 0, 4, 1]);
    }

    #[test]
    fn faders_query_parse() {
        assert_eq!(parse_faders_query(&hex("33 05 31 2C 2F 2E 2C")), Some([49, 44, 47, 46, 44]));
    }
}
