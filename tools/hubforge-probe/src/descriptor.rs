//! Analyse minimale d'un descripteur de rapport HID :
//! calcule la taille (en octets) de chaque rapport d'entrée, de sortie
//! et de "feature", par Report ID.

use std::collections::BTreeMap;

#[derive(Default, Debug, Clone)]
pub struct ReportSizes {
    /// Report ID -> taille en octets (SANS l'octet de Report ID)
    pub input: BTreeMap<u8, usize>,
    pub output: BTreeMap<u8, usize>,
    pub feature: BTreeMap<u8, usize>,
    pub uses_report_ids: bool,
    pub usage_pages: Vec<u16>,
}

impl ReportSizes {
    /// Taille totale d'un rapport de sortie, Report ID inclus (comme
    /// "OutputReportByteLength" sous Windows).
    pub fn output_len_with_id(&self, id: u8) -> Option<usize> {
        self.output.get(&id).map(|b| b + 1)
    }

    /// Plus grande taille de rapport de sortie, Report ID inclus.
    pub fn max_output_len(&self) -> Option<usize> {
        self.output.values().max().map(|b| b + 1)
    }
}

pub fn parse(desc: &[u8]) -> ReportSizes {
    let mut out = ReportSizes::default();
    let mut bits_in: BTreeMap<u8, usize> = BTreeMap::new();
    let mut bits_out: BTreeMap<u8, usize> = BTreeMap::new();
    let mut bits_feat: BTreeMap<u8, usize> = BTreeMap::new();

    let mut report_size: usize = 0;
    let mut report_count: usize = 0;
    let mut report_id: u8 = 0;
    let mut stack: Vec<(usize, usize, u8)> = Vec::new();

    let mut i = 0;
    while i < desc.len() {
        let prefix = desc[i];
        if prefix == 0xFE {
            // Long item : on le saute.
            if i + 2 >= desc.len() {
                break;
            }
            let len = desc[i + 1] as usize;
            i += 3 + len;
            continue;
        }
        let size = match prefix & 0x03 {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 4,
        };
        if i + 1 + size > desc.len() {
            break;
        }
        let mut value: u32 = 0;
        for k in 0..size {
            value |= (desc[i + 1 + k] as u32) << (8 * k);
        }
        let tag = prefix & 0xFC;
        match tag {
            // Main items
            0x80 => *bits_in.entry(report_id).or_default() += report_size * report_count,
            0x90 => *bits_out.entry(report_id).or_default() += report_size * report_count,
            0xB0 => *bits_feat.entry(report_id).or_default() += report_size * report_count,
            // Global items
            0x04 => {
                let page = value as u16;
                if !out.usage_pages.contains(&page) {
                    out.usage_pages.push(page);
                }
            }
            0x74 => report_size = value as usize,
            0x94 => report_count = value as usize,
            0x84 => {
                report_id = value as u8;
                out.uses_report_ids = true;
            }
            0xA4 => stack.push((report_size, report_count, report_id)),
            0xB4 => {
                if let Some((s, c, id)) = stack.pop() {
                    report_size = s;
                    report_count = c;
                    report_id = id;
                }
            }
            _ => {}
        }
        i += 1 + size;
    }

    let to_bytes = |m: BTreeMap<u8, usize>| -> BTreeMap<u8, usize> {
        m.into_iter().map(|(k, v)| (k, v.div_ceil(8))).collect()
    };
    out.input = to_bytes(bits_in);
    out.output = to_bytes(bits_out);
    out.feature = to_bytes(bits_feat);
    out
}

pub fn describe(s: &ReportSizes) -> String {
    let fmt = |m: &BTreeMap<u8, usize>| -> String {
        if m.is_empty() {
            return "aucun".into();
        }
        m.iter()
            .map(|(id, n)| format!("ID 0x{id:02X} = {} octets", n + 1))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "entrée [{}] | sortie [{}] | feature [{}]",
        fmt(&s.input),
        fmt(&s.output),
        fmt(&s.feature)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vendor_descriptor() {
        // Usage Page 0xFF00, Report ID 2 : 63 octets en sortie,
        // Report ID 1 : 63 octets en entrée.
        let d = [
            0x06, 0x00, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0x85, 0x01, 0x75, 0x08, 0x95, 0x3F, 0x09,
            0x01, 0x81, 0x02, 0x85, 0x02, 0x95, 0x3F, 0x09, 0x01, 0x91, 0x02, 0xC0,
        ];
        let s = parse(&d);
        assert_eq!(s.input.get(&1), Some(&63));
        assert_eq!(s.output.get(&2), Some(&63));
        assert_eq!(s.max_output_len(), Some(64));
        assert_eq!(s.usage_pages, vec![0xFF00]);
    }
}
