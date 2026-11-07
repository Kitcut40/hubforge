//! Position des modules sur la base.
//!
//! Chaque module est branché sur un port USB interne de la base. Le chemin
//! de ports (ex. « 1-3-4 ») donne sa position et son orientation, comme le
//! fait le logiciel officiel. On lit ce chemin dans Windows
//! (propriété « LocationPaths » du périphérique USB).

use serde::Serialize;
use std::collections::HashMap;

/// Emplacement d'un module sur la grille de la base (4 colonnes × 6 lignes,
/// base vue avec les modules dans leur sens « normal »).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Placement {
    pub col: u8,
    pub row: u8,
    /// Rotation en degrés (0, 90, 180, 270), sens horaire.
    pub rot: u16,
}

/// Table « chemin de ports → emplacement » par type de module.
/// Déduite de la géométrie de la base (interopérabilité).
pub fn placement(kind: &str, code: &str) -> Option<Placement> {
    let table: &[(&str, u8, u8, u16)] = match kind {
        "keys15" => &[
            ("24", 0, 0, 0), ("42", 0, 1, 0), ("41", 0, 2, 0), ("122", 0, 3, 0),
            ("141", 0, 0, 180), ("33", 0, 2, 180),
            ("132", 0, 0, 270), ("134", 1, 0, 270), ("112", 1, 1, 270), ("22", 0, 2, 270), ("23", 1, 2, 270),
        ],
        "faders5" => &[
            ("113", 0, 0, 0), ("24", 0, 1, 0), ("42", 0, 2, 0), ("41", 0, 3, 0), ("122", 0, 4, 0),
            ("141", 0, 0, 180), ("33", 0, 2, 180),
            ("143", 0, 0, 270), ("132", 1, 0, 270), ("134", 2, 0, 270), ("32", 2, 0, 270), ("112", 2, 1, 270),
            ("111", 0, 2, 270), ("22", 1, 2, 270), ("23", 2, 2, 270), ("31", 2, 2, 270),
        ],
        "rollers2" => &[
            ("134", 0, 0, 0), ("112", 0, 1, 0), ("23", 0, 2, 0), ("43", 0, 3, 0), ("44", 0, 4, 0), ("121", 0, 5, 0),
            ("31", 1, 0, 90), ("42", 3, 0, 90), ("41", 3, 1, 90), ("123", 1, 2, 90), ("122", 3, 2, 90),
            ("141", 0, 0, 270), ("142", 1, 0, 270), ("131", 2, 0, 270), ("124", 2, 0, 270), ("133", 3, 0, 270),
            ("33", 0, 2, 270), ("32", 1, 2, 270), ("21", 2, 2, 270), ("114", 3, 2, 270),
        ],
        _ => return None,
    };
    table.iter().find(|(c, ..)| *c == code).map(|&(_, col, row, rot)| Placement { col, row, rot })
}

/// Chemins de ports des modules : n° de série → code (ex. "134").
pub fn port_codes() -> HashMap<String, String> {
    imp::port_codes()
}

/// Extrait les numéros de port d'un chemin Windows
/// « PCIROOT(0)#PCI(1400)#USBROOT(0)#USB(2)#USB(1)#USB(3) » → (préfixe racine, [2, 1, 3]).
fn parse_location(path: &str) -> Option<(String, Vec<u32>)> {
    let idx = path.find("#USBROOT(")?;
    let root = path[..idx].to_string();
    let ports = path[idx..]
        .split('#')
        .filter_map(|seg| seg.strip_prefix("USB(").and_then(|s| s.strip_suffix(')')))
        .filter_map(|n| n.parse().ok())
        .collect::<Vec<u32>>();
    Some((root, ports))
}

/// Calcule les codes de position à partir des chemins des hubs de la base et
/// des modules (fonction pure, testable).
fn compute(hubs: &[(String, Vec<u32>)], modules: &[(String, String, Vec<u32>)]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for (serial, root, ports) in modules {
        // Hub de la base le plus haut dans l'arborescence qui contient ce module.
        let best = hubs
            .iter()
            .filter(|(hroot, hports)| hroot == root && hports.len() < ports.len() && ports.starts_with(hports))
            .min_by_key(|(_, hports)| hports.len());
        if let Some((_, hports)) = best {
            let code: String = ports[hports.len()..].iter().map(|p| p.to_string()).collect();
            out.insert(serial.clone(), code);
        }
    }
    out
}

#[cfg(windows)]
mod imp {
    use super::{compute, parse_location};
    use std::collections::HashMap;
    use windows::core::PCWSTR;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        CM_Get_DevNode_PropertyW, CM_Get_Device_ID_ListW, CM_Get_Device_ID_List_SizeW, CM_Locate_DevNodeW,
        CM_GETIDLIST_FILTER_ENUMERATOR, CM_GETIDLIST_FILTER_PRESENT, CM_LOCATE_DEVNODE_NORMAL, CR_SUCCESS,
    };
    use windows::Win32::Devices::Properties::{DEVPKEY_Device_LocationPaths, DEVPROPTYPE};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn device_ids() -> Vec<String> {
        let filter = wide("USB");
        let flags = CM_GETIDLIST_FILTER_ENUMERATOR | CM_GETIDLIST_FILTER_PRESENT;
        let mut len = 0u32;
        unsafe {
            if CM_Get_Device_ID_List_SizeW(&mut len, PCWSTR(filter.as_ptr()), flags) != CR_SUCCESS {
                return Vec::new();
            }
            let mut buf = vec![0u16; len as usize];
            if CM_Get_Device_ID_ListW(PCWSTR(filter.as_ptr()), &mut buf, flags) != CR_SUCCESS {
                return Vec::new();
            }
            buf.split(|&c| c == 0).filter(|s| !s.is_empty()).map(String::from_utf16_lossy).collect()
        }
    }

    fn location(id: &str) -> Option<String> {
        let w = wide(id);
        let mut inst = 0u32;
        unsafe {
            if CM_Locate_DevNodeW(&mut inst, PCWSTR(w.as_ptr()), CM_LOCATE_DEVNODE_NORMAL) != CR_SUCCESS {
                return None;
            }
            let mut ty = DEVPROPTYPE::default();
            let mut size = 0u32;
            let _ = CM_Get_DevNode_PropertyW(inst, &DEVPKEY_Device_LocationPaths, &mut ty, None, &mut size, 0);
            if size == 0 {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            if CM_Get_DevNode_PropertyW(inst, &DEVPKEY_Device_LocationPaths, &mut ty, Some(buf.as_mut_ptr()), &mut size, 0)
                != CR_SUCCESS
            {
                return None;
            }
            let wide: Vec<u16> = buf.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            wide.split(|&c| c == 0)
                .filter(|s| !s.is_empty())
                .map(String::from_utf16_lossy)
                .find(|s| s.contains("#USBROOT("))
        }
    }

    pub fn port_codes() -> HashMap<String, String> {
        let mut hubs = Vec::new();
        let mut modules = Vec::new();
        for id in device_ids() {
            let up = id.to_uppercase();
            // Les sous-interfaces (&MI_xx) ne nous intéressent pas.
            if up.contains("&MI_") {
                continue;
            }
            let is_hub = up.starts_with("USB\\VID_067B&PID_2586\\");
            let is_module = up.starts_with("USB\\VID_2516&");
            if !is_hub && !is_module {
                continue;
            }
            let Some(loc) = location(&id) else { continue };
            let Some((root, ports)) = parse_location(&loc) else { continue };
            if is_hub {
                hubs.push((root, ports));
            } else if let Some(serial) = id.rsplit('\\').next() {
                modules.push((serial.to_string(), root, ports));
            }
        }
        let codes = compute(&hubs, &modules);
        log::debug!("positions des modules : {codes:?}");
        codes
    }
}

#[cfg(not(windows))]
mod imp {
    use std::collections::HashMap;
    pub fn port_codes() -> HashMap<String, String> {
        HashMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_location_path() {
        let (root, ports) = parse_location("PCIROOT(0)#PCI(1400)#USBROOT(0)#USB(2)#USB(1)#USB(3)#USB(4)").unwrap();
        assert_eq!(root, "PCIROOT(0)#PCI(1400)");
        assert_eq!(ports, vec![2, 1, 3, 4]);
    }

    #[test]
    fn computes_codes_from_top_hub() {
        let r = "PCIROOT(0)#PCI(1400)".to_string();
        let hubs = vec![(r.clone(), vec![2]), (r.clone(), vec![2, 1])];
        let modules = vec![("SN1".to_string(), r.clone(), vec![2, 1, 3, 4]), ("SN2".to_string(), r.clone(), vec![2, 4, 1])];
        let c = compute(&hubs, &modules);
        assert_eq!(c["SN1"], "134");
        assert_eq!(c["SN2"], "41");
    }

    #[test]
    fn default_layout_positions() {
        assert_eq!(placement("keys15", "24"), Some(Placement { col: 0, row: 0, rot: 0 }));
        assert_eq!(placement("rollers2", "121"), Some(Placement { col: 0, row: 5, rot: 0 }));
        assert_eq!(placement("faders5", "999"), None);
    }
}
