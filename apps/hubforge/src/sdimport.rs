//! Import des fichiers du logiciel Stream Deck, pour l'interopérabilité :
//! profils (`.streamDeckProfile`) et packs d'icônes (`.streamDeckIconPack`).
//!
//! Les actions de base d'Elgato (raccourcis, texte, ouvrir, site web,
//! multimédia, sons, multi-actions, dossiers, pages) sont converties en
//! actions HubForge. Les touches qui dépendent d'un plugin tiers arrivent
//! avec leur image et leur titre, sans action, et sont listées dans le
//! compte rendu.

use crate::config::{self, Action, Config, KeyDef, Page};
use crate::share::ImportReport;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

const MAX_BYTES: u64 = 200 * 1024 * 1024;
const COLS: usize = 5;
const ROWS: usize = 3;

/// Contenu d'une archive zip, noms en minuscules → (nom réel, données).
struct Archive {
    files: HashMap<String, (String, Vec<u8>)>,
}

impl Archive {
    fn open(bytes: &[u8]) -> Result<Archive, String> {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "ce fichier n'est pas une archive lisible".to_string())?;
        let mut files = HashMap::new();
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            if f.is_dir() {
                continue;
            }
            total += f.size();
            if total > MAX_BYTES {
                return Err("archive trop grosse (200 Mo maximum)".into());
            }
            let name = f.name().replace('\\', "/");
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            files.insert(name.to_lowercase(), (name, buf));
        }
        Ok(Archive { files })
    }

    fn get(&self, path: &str) -> Option<&Vec<u8>> {
        self.files.get(&path.replace('\\', "/").to_lowercase()).map(|(_, d)| d)
    }

    fn json(&self, path: &str) -> Option<Value> {
        self.get(path).and_then(|b| serde_json::from_slice(strip_bom(b)).ok())
    }

    fn names(&self) -> impl Iterator<Item = &String> {
        self.files.values().map(|(n, _)| n)
    }
}

fn strip_bom(b: &[u8]) -> &[u8] {
    b.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(b)
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

fn join(dir: &str, rel: &str) -> String {
    if dir.is_empty() {
        rel.to_string()
    } else {
        format!("{dir}/{rel}")
    }
}

/// Nom de fichier sûr (pas de dossier, pas de caractère interdit).
fn safe(name: &str) -> String {
    let base = name.replace('\\', "/");
    let base = base.rsplit('/').next().unwrap_or("fichier");
    let s: String = base.chars().map(|c| if c.is_alphanumeric() || " ._-()".contains(c) { c } else { '_' }).collect();
    let s = s.trim_matches(|c: char| c == '.' || c == ' ').to_string();
    if s.is_empty() {
        "fichier".into()
    } else {
        s
    }
}

/// Importe un fichier Stream Deck ou HubForge : le format est reconnu au contenu.
pub fn import_any(cfg_dir: &Path, bytes: &[u8]) -> Result<ImportReport, String> {
    let ar = Archive::open(bytes)?;
    if ar.get("manifest.json").is_some() && ar.get("profile.toml").is_some() {
        return crate::share::import(cfg_dir, bytes);
    }
    if ar.names().any(|n| n.to_lowercase().contains(".sdiconpack/") || n.eq_ignore_ascii_case("icons.json")) {
        return import_icon_pack(cfg_dir, &ar);
    }
    if ar.names().any(|n| n.to_lowercase().contains(".sdprofile/")) {
        return import_profile(cfg_dir, &ar);
    }
    Err("format non reconnu : il faut un profil HubForge (.hfprofile), un profil Stream Deck (.streamDeckProfile) ou un pack d'icônes (.streamDeckIconPack)".into())
}

// ----------------------------------------------------------------------
// Profils
// ----------------------------------------------------------------------

/// Une page Stream Deck trouvée dans l'archive.
struct SdPage {
    id: String,
    /// Dossier de la page dans l'archive (pour les images).
    dir: String,
    actions: Vec<((usize, usize), Value)>,
}

fn page_actions(v: &Value) -> Vec<((usize, usize), Value)> {
    // Format récent : Controllers[0].Actions ; ancien : Actions.
    let map = v
        .get("Controllers")
        .and_then(|c| c.as_array())
        .and_then(|c| c.iter().find(|c| c.get("Type").and_then(|t| t.as_str()).unwrap_or("Keypad") == "Keypad"))
        .and_then(|c| c.get("Actions"))
        .or_else(|| v.get("Actions"))
        .and_then(|a| a.as_object());
    let mut out: Vec<((usize, usize), Value)> = map
        .map(|m| {
            m.iter()
                .filter(|(_, a)| a.is_object())
                .filter_map(|(pos, a)| {
                    let (c, r) = pos.split_once(',')?;
                    Some(((c.trim().parse().ok()?, r.trim().parse().ok()?), a.clone()))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|((c, r), _)| (*r, *c));
    out
}

struct Ctx<'a> {
    ar: &'a Archive,
    cfg_dir: &'a Path,
    slug: String,
    /// Identifiant de page Stream Deck (minuscules) → numéro de page HubForge (1…).
    page_index: HashMap<String, usize>,
    images: usize,
    sounds: usize,
    unsupported: Vec<String>,
    /// Fichiers déjà écrits (nom dans l'archive → chemin HubForge).
    written: HashMap<String, String>,
}

impl Ctx<'_> {
    /// Copie un fichier de l'archive dans le dossier de HubForge ; renvoie son chemin relatif.
    fn copy(&mut self, entry: &str, kind: &str) -> Option<String> {
        let key = entry.to_lowercase();
        if let Some(p) = self.written.get(&key) {
            return Some(p.clone());
        }
        let data = self.ar.get(entry)?;
        let (base_dir, rel_dir) = match kind {
            "sound" => ("sounds", format!("sounds/sd-{}", self.slug)),
            _ => ("icons", format!("icons/sd-{}", self.slug)),
        };
        let _ = base_dir;
        let dir = self.cfg_dir.join(&rel_dir);
        std::fs::create_dir_all(&dir).ok()?;
        let mut name = safe(entry);
        let mut n = 2;
        while dir.join(&name).exists() {
            let (stem, ext) = name.rsplit_once('.').map(|(a, b)| (a.to_string(), format!(".{b}"))).unwrap_or((name.clone(), String::new()));
            name = format!("{} ({n}){ext}", stem.trim_end_matches(|c: char| c == ')' || c.is_ascii_digit() || c == '(' || c == ' '));
            n += 1;
        }
        std::fs::write(dir.join(&name), data).ok()?;
        let rel = format!("{rel_dir}/{name}");
        self.written.insert(key, rel.clone());
        if kind == "sound" {
            self.sounds += 1;
        } else {
            self.images += 1;
        }
        Some(rel)
    }

    /// Fichier son d'une action « jouer un son ».
    fn sound_file(&mut self, a: &Value) -> Option<String> {
        // Format récent : Resources.file → Resources/manifest.json.
        if let Some(id) = a.pointer("/Resources/file").and_then(|v| v.as_str()) {
            let mapped = self
                .ar
                .json("Resources/manifest.json")
                .and_then(|m| m.pointer(&format!("/resources/{id}/file")).and_then(|f| f.as_str()).map(String::from));
            for cand in mapped.map(|f| format!("Resources/{f}")).into_iter().chain([format!("Resources/{id}")]) {
                if self.ar.get(&cand).is_some() {
                    return self.copy(&cand, "sound");
                }
                // Le dossier de la ressource contient le fichier.
                let prefix = format!("{}/", cand.to_lowercase());
                let found = self.ar.files.iter().find(|(k, _)| k.starts_with(&prefix)).map(|(_, (n, _))| n.clone());
                if let Some(f) = found {
                    return self.copy(&f, "sound");
                }
            }
        }
        // Sinon : un fichier de l'archive qui porte le même nom.
        let path = a.pointer("/Settings/path").and_then(|v| v.as_str())?;
        let base = safe(path).to_lowercase();
        let found = self.ar.files.iter().find(|(k, _)| k.rsplit('/').next() == Some(base.as_str())).map(|(_, (n, _))| n.clone())?;
        self.copy(&found, "sound")
    }

    /// Image de l'état 0 d'une touche.
    fn image(&mut self, page_dir: &str, pos: (usize, usize), a: &Value) -> Option<String> {
        let state = a.get("States").and_then(|s| s.as_array()).and_then(|s| s.first());
        let mut cands = Vec::new();
        if let Some(img) = state.and_then(|s| s.get("Image")).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            cands.push(join(page_dir, img));
        }
        // Ancien format : <col>,<row>/CustomImages/state0.png
        for name in ["state0.png", "state0.jpg", "state0.gif", "state0.svg"] {
            cands.push(join(page_dir, &format!("{},{}/CustomImages/{name}", pos.0, pos.1)));
        }
        let entry = cands.into_iter().find(|c| self.ar.get(c).is_some())?;
        self.copy(&entry, "image")
    }

    fn convert(&mut self, a: &Value, depth: usize) -> Action {
        let uuid = a.get("UUID").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
        let st = a.get("Settings").cloned().unwrap_or(Value::Null);
        let s = |k: &str| st.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        match uuid.as_str() {
            "com.elgato.streamdeck.system.hotkey" | "com.elgato.streamdeck.system.hotkeyswitch" => {
                let hk = st.get("Hotkeys").and_then(|h| h.as_array()).and_then(|h| h.iter().find(|k| k.get("VKeyCode").and_then(|v| v.as_i64()).unwrap_or(-1) > 0));
                match hk.and_then(hotkey_string) {
                    Some(keys) => Action::Hotkey { keys },
                    None => self.skip("Raccourci (vide)"),
                }
            }
            "com.elgato.streamdeck.system.text" => {
                let mut text = s("pastedText");
                if st.get("isSendingEnter").and_then(|v| v.as_bool()).unwrap_or(false) {
                    text.push('\n');
                }
                Action::Text { text }
            }
            "com.elgato.streamdeck.system.open" | "com.elgato.streamdeck.system.openapp" => {
                let target = if s("path").is_empty() { s("appPath") } else { s("path") };
                if target.is_empty() {
                    return self.skip("Ouvrir (vide)");
                }
                Action::Launch { target: target.trim_matches('"').to_string(), args: Vec::new() }
            }
            "com.elgato.streamdeck.system.website" => {
                let url = s("path");
                if url.is_empty() {
                    return self.skip("Site web (vide)");
                }
                let url = if url.contains("://") { url } else { format!("https://{url}") };
                Action::Launch { target: url, args: Vec::new() }
            }
            "com.elgato.streamdeck.system.multimedia" => {
                let cmd = match st.get("actionIdx").and_then(|v| v.as_i64()).unwrap_or(0) {
                    1 => "next",
                    2 => "previous",
                    3 => "mute",
                    4 => "volume_up",
                    5 => "volume_down",
                    _ => "play_pause",
                };
                Action::Media { command: cmd.into() }
            }
            "com.elgato.streamdeck.soundboard.playaudio" => match self.sound_file(a) {
                Some(file) => {
                    let mode = match st.get("actionType").and_then(|v| v.as_i64()).unwrap_or(0) {
                        1 => "overlap",
                        2 => "restart",
                        _ => "toggle",
                    };
                    let volume = st.get("volume").and_then(|v| v.as_u64()).unwrap_or(100).clamp(0, 100) as u8;
                    Action::Sound { file, volume, mode: mode.into() }
                }
                None => self.skip("Son introuvable dans le profil"),
            },
            "com.elgato.streamdeck.soundboard.stop" | "com.elgato.streamdeck.soundboard.stopall" => Action::StopSounds,
            "com.elgato.streamdeck.page.next" => Action::Page { to: "next".into() },
            "com.elgato.streamdeck.page.previous" => Action::Page { to: "previous".into() },
            "com.elgato.streamdeck.profile.backtoparent" => Action::Back,
            "com.elgato.streamdeck.profile.openchild" => {
                let id = s("ProfileUUID").to_lowercase();
                match self.page_index.get(&id) {
                    Some(n) => Action::Folder { to: n.to_string() },
                    None => self.skip("Dossier introuvable"),
                }
            }
            u if u.starts_with("com.elgato.streamdeck.multiactions") && depth == 0 => {
                // Multi-action : liste d'actions de l'état 0.
                let list = a
                    .get("Actions")
                    .and_then(|v| v.as_array())
                    .and_then(|v| v.first())
                    .and_then(|v| v.get("Actions"))
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let mut steps = Vec::new();
                for step in list.iter().take(config::MACRO_MAX_STEPS) {
                    let su = step.get("UUID").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                    if su == "com.elgato.streamdeck.system.delay" || su.ends_with(".delay") {
                        let ms = step.pointer("/Settings/delayValue").and_then(|v| v.as_u64()).unwrap_or(500);
                        steps.push(Action::Wait { ms: ms.min(config::WAIT_MAX_MS) });
                        continue;
                    }
                    match self.convert(step, depth + 1) {
                        Action::None => {}
                        Action::Folder { .. } | Action::Macro { .. } => {}
                        x => steps.push(x),
                    }
                }
                if steps.is_empty() {
                    self.skip("Multi-action")
                } else {
                    Action::Macro { steps }
                }
            }
            _ => {
                let plugin = a
                    .pointer("/Plugin/Name")
                    .and_then(|v| v.as_str())
                    .or_else(|| a.get("Name").and_then(|v| v.as_str()))
                    .unwrap_or(&uuid)
                    .to_string();
                self.skip(&plugin)
            }
        }
    }

    fn skip(&mut self, what: &str) -> Action {
        if !self.unsupported.iter().any(|u| u == what) {
            self.unsupported.push(what.to_string());
        }
        Action::None
    }
}

/// Raccourci Stream Deck → « ctrl+shift+a ».
fn hotkey_string(h: &Value) -> Option<String> {
    let flag = |k: &str| h.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
    let vk = h.get("VKeyCode").and_then(|v| v.as_i64())? as u32;
    let key = vk_name(vk)?;
    let mut parts = Vec::new();
    if flag("KeyCtrl") {
        parts.push("ctrl".to_string());
    }
    if flag("KeyShift") {
        parts.push("shift".into());
    }
    if flag("KeyOption") {
        parts.push("alt".into());
    }
    if flag("KeyCmd") {
        parts.push("win".into());
    }
    if !parts.iter().any(|p| *p == key) {
        parts.push(key);
    }
    Some(parts.join("+"))
}

/// Code de touche virtuelle Windows → nom HubForge.
fn vk_name(vk: u32) -> Option<String> {
    Some(match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk)?.to_ascii_lowercase().to_string(),
        0x60..=0x69 => char::from_u32(vk - 0x30)?.to_string(), // pavé numérique
        0x70..=0x87 => format!("f{}", vk - 0x6F),
        0x08 => "backspace".into(),
        0x09 => "tab".into(),
        0x0D => "enter".into(),
        0x1B => "esc".into(),
        0x20 => "space".into(),
        0x21 => "pageup".into(),
        0x22 => "pagedown".into(),
        0x23 => "end".into(),
        0x24 => "home".into(),
        0x25 => "left".into(),
        0x26 => "up".into(),
        0x27 => "right".into(),
        0x28 => "down".into(),
        0x2C => "printscreen".into(),
        0x2D => "insert".into(),
        0x2E => "delete".into(),
        0x10 => "shift".into(),
        0x11 => "ctrl".into(),
        0x12 => "alt".into(),
        0x5B | 0x5C => "win".into(),
        0x6A => "*".into(),
        0x6B | 0xBB => "plus".into(),
        0x6D | 0xBD => "-".into(),
        0xBC => ",".into(),
        0xBE | 0x6E => ".".into(),
        0xBA => ";".into(),
        0xBF | 0x6F => "/".into(),
        _ => return None,
    })
}

fn import_profile(cfg_dir: &Path, ar: &Archive) -> Result<ImportReport, String> {
    // Manifeste du profil : le plus court chemin « …x.sdProfile/manifest.json ».
    let mut tops: Vec<&String> = ar
        .names()
        .filter(|n| {
            let l = n.to_lowercase();
            l.ends_with(".sdprofile/manifest.json")
        })
        .collect();
    tops.sort_by_key(|n| n.matches('/').count());
    let top_path = tops.first().ok_or("profil Stream Deck sans manifeste")?.to_string();
    let top_dir = dir_of(&top_path).to_string();
    let top = ar.json(&top_path).ok_or("manifeste du profil illisible")?;
    let profile_name = top.get("Name").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()).unwrap_or("Profil Stream Deck").to_string();

    // Pages : format récent (Profiles/<id>/manifest.json) ou ancien (actions
    // dans le manifeste, dossiers dans Profiles/<id>.sdProfile/).
    let mut pages: Vec<SdPage> = Vec::new();
    let sub_prefix = format!("{}/profiles/", top_dir.to_lowercase());
    let mut subs: Vec<(String, String)> = ar
        .names()
        .filter(|n| {
            let l = n.to_lowercase();
            l.starts_with(&sub_prefix) && l.ends_with("/manifest.json") && l[sub_prefix.len()..].matches('/').count() == 1
        })
        .map(|n| {
            let id = n[sub_prefix.len()..].split('/').next().unwrap_or("").trim_end_matches(".sdProfile").trim_end_matches(".sdprofile").to_lowercase();
            (id, n.clone())
        })
        .collect();
    subs.sort();
    // Ordre : pages listées, page par défaut, puis les autres (dossiers).
    let mut order: Vec<String> = top
        .pointer("/Pages/Pages")
        .and_then(|p| p.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_lowercase())).collect())
        .unwrap_or_default();
    if let Some(d) = top.pointer("/Pages/Default").and_then(|v| v.as_str()) {
        order.push(d.to_lowercase());
    }
    let top_actions = page_actions(&top);
    if !top_actions.is_empty() {
        pages.push(SdPage { id: "__top".into(), dir: top_dir.clone(), actions: top_actions });
    }
    let mut seen = std::collections::HashSet::new();
    for id in order.iter().chain(subs.iter().map(|(i, _)| i)) {
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some((_, path)) = subs.iter().find(|(i, _)| i == id) else { continue };
        let Some(v) = ar.json(path) else { continue };
        let actions = page_actions(&v);
        // Une page par défaut vide n'est pas recopiée.
        if actions.is_empty() && order.last() == Some(id) && !order[..order.len() - 1].contains(id) {
            continue;
        }
        pages.push(SdPage { id: id.clone(), dir: dir_of(path).to_string(), actions });
    }
    if pages.is_empty() {
        return Err("ce profil Stream Deck ne contient aucune touche".into());
    }

    // Nom libre.
    let wanted = crate::profiles::check_name(&profile_name).unwrap_or_else(|_| "Profil Stream Deck".into());
    let existing = crate::profiles::list(cfg_dir);
    let mut name = wanted.clone();
    let mut n = 2;
    while existing.iter().any(|e| e.eq_ignore_ascii_case(&name)) {
        name = format!("{wanted} ({n})");
        n += 1;
    }
    let slug: String = safe(&name).to_lowercase().replace(' ', "-");

    // Numéros de pages : les touches en trop (Stream Deck XL…) vont sur des
    // pages supplémentaires, donc on compte d'abord.
    let mut layout: Vec<(usize, Vec<((usize, usize), Value)>)> = Vec::new(); // (page sd, touches d'une page HubForge)
    for (pi, p) in pages.iter().enumerate() {
        let mut slots: Vec<Option<((usize, usize), Value)>> = vec![None; COLS * ROWS];
        let mut overflow = Vec::new();
        for (pos, a) in &p.actions {
            if pos.0 < COLS && pos.1 < ROWS {
                slots[pos.1 * COLS + pos.0] = Some((*pos, a.clone()));
            } else {
                overflow.push((*pos, a.clone()));
            }
        }
        // Les touches en trop remplissent d'abord les cases vides.
        for slot in slots.iter_mut().filter(|s| s.is_none()) {
            if overflow.is_empty() {
                break;
            }
            *slot = Some(overflow.remove(0));
        }
        layout.push((pi, slots.into_iter().flatten().collect()));
        while !overflow.is_empty() {
            let take = overflow.len().min(COLS * ROWS);
            layout.push((pi, overflow.drain(..take).collect()));
        }
    }
    let mut ctx = Ctx {
        ar,
        cfg_dir,
        slug,
        page_index: HashMap::new(),
        images: 0,
        sounds: 0,
        unsupported: Vec::new(),
        written: HashMap::new(),
    };
    for (i, (pi, _)) in layout.iter().enumerate() {
        ctx.page_index.entry(pages[*pi].id.clone()).or_insert(i + 1);
    }

    let mut cfg: Config = toml::from_str("").or_else(|_| toml::from_str(config::DEFAULT_CONFIG)).map_err(|e| e.to_string())?;
    cfg.pages.clear();
    for (i, (pi, keys)) in layout.iter().enumerate() {
        let p = &pages[*pi];
        let mut page: Page = toml::from_str("").map_err(|e| e.to_string())?;
        page.name = if layout.len() > 1 { format!("{} {}", name, i + 1) } else { name.clone() };
        // Places : dans l'ordre de la grille d'origine, sinon dans l'ordre libre.
        let mut used = [false; COLS * ROWS];
        let mut placed: Vec<(usize, &((usize, usize), Value))> = Vec::new();
        for item in keys {
            let (c, r) = item.0;
            if c < COLS && r < ROWS && !used[r * COLS + c] {
                used[r * COLS + c] = true;
                placed.push((r * COLS + c, item));
            }
        }
        for item in keys {
            if placed.iter().any(|(_, it)| std::ptr::eq(*it, item)) {
                continue;
            }
            if let Some(free) = used.iter().position(|u| !u) {
                used[free] = true;
                placed.push((free, item));
            }
        }
        placed.sort_by_key(|(slot, _)| *slot);
        for (slot, (pos, a)) in placed {
            let action = ctx.convert(a, 0);
            let image = ctx.image(&p.dir, *pos, a);
            let state = a.get("States").and_then(|s| s.as_array()).and_then(|s| s.first());
            let show_title = state.and_then(|s| s.get("ShowTitle")).and_then(|v| v.as_bool()).unwrap_or(true);
            let title = state.and_then(|s| s.get("Title")).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            let mut label = if show_title { title } else { String::new() };
            if image.is_none() && label.is_empty() {
                label = a.get("Name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            }
            let unsupported = action == Action::None;
            page.keys.push(KeyDef {
                key: slot as u8 + 1,
                label: label.chars().take(40).collect(),
                icon: image,
                color: Some(if unsupported { "#3A3D45".into() } else { "#1F2229".into() }),
                text_color: None,
                action,
                toggle: None,
                widget: None,
                widget_style: None,
                widget_target: None,
                long: None,
                double: None,
                theme: None,
                confirm: false,
                repeat: false,
                state_var: None,
            });
        }
        cfg.pages.push(page);
    }
    // Les pages ouvertes par une touche « dossier » deviennent des dossiers
    // (hors du défilement page suivante / précédente).
    let targets: Vec<usize> = cfg
        .pages
        .iter()
        .flat_map(|p| p.keys.iter())
        .filter_map(|k| match &k.action {
            Action::Folder { to } => to.parse::<usize>().ok(),
            _ => None,
        })
        .collect();
    for t in targets {
        if t > 1 {
            if let Some(p) = cfg.pages.get_mut(t - 1) {
                p.folder = true;
            }
        }
    }
    config::validate(&cfg)?;
    crate::profiles::create(cfg_dir, &name, None)?;
    config::save(&crate::profiles::path(cfg_dir, &name), &cfg)?;
    log::info!(
        "profil Stream Deck importé : {name} ({} pages, {} images, {} sons, non pris en charge : {:?})",
        cfg.pages.len(),
        ctx.images,
        ctx.sounds,
        ctx.unsupported
    );
    Ok(ImportReport {
        name,
        missing_addons: Vec::new(),
        images: ctx.images,
        sounds: ctx.sounds,
        unsupported: ctx.unsupported,
        kind: "profile".into(),
    })
}

// ----------------------------------------------------------------------
// Packs d'icônes
// ----------------------------------------------------------------------

/// Un pack d'icônes installé (dossier `icon-packs/<nom>/`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IconPack {
    pub name: String,
    #[serde(default)]
    pub author: String,
    pub folder: String,
    pub icons: Vec<PackIcon>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PackIcon {
    /// Chemin à mettre dans la touche (relatif au dossier de HubForge).
    pub path: String,
    pub name: String,
}

fn is_image(n: &str) -> bool {
    let l = n.to_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg"].iter().any(|e| l.ends_with(e))
}

fn import_icon_pack(cfg_dir: &Path, ar: &Archive) -> Result<ImportReport, String> {
    let manifest_path = ar
        .names()
        .filter(|n| n.to_lowercase().ends_with("manifest.json"))
        .min_by_key(|n| n.matches('/').count())
        .cloned();
    let root = manifest_path.as_deref().map(dir_of).unwrap_or("").to_string();
    let manifest = manifest_path.as_deref().and_then(|p| ar.json(p)).unwrap_or(Value::Null);
    let pack_name = manifest.get("Name").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()).unwrap_or("Pack d'icônes").to_string();
    let author = manifest.get("Author").and_then(|v| v.as_str()).unwrap_or("").to_string();
    // Noms lisibles depuis icons.json s'il existe.
    let mut names: HashMap<String, String> = HashMap::new();
    if let Some(list) = ar.json(&join(&root, "icons.json")).and_then(|v| v.as_array().cloned()) {
        for it in list {
            if let (Some(p), Some(n)) = (it.get("path").and_then(|v| v.as_str()), it.get("name").and_then(|v| v.as_str())) {
                names.insert(safe(p).to_lowercase(), n.to_string());
            }
        }
    }
    let icons_prefix = join(&root, "icons/").to_lowercase();
    let mut entries: Vec<String> = ar.names().filter(|n| n.to_lowercase().starts_with(&icons_prefix) && is_image(n)).cloned().collect();
    if entries.is_empty() {
        // Pack sans dossier « icons » : toutes les images sauf l'icône du pack.
        entries = ar.names().filter(|n| is_image(n) && dir_of(n) != root).cloned().collect();
    }
    if entries.is_empty() {
        return Err("ce pack ne contient aucune image".into());
    }
    entries.sort();
    let folder = {
        let base: String = safe(&pack_name).to_lowercase().replace(' ', "-");
        let mut f = base.clone();
        let mut n = 2;
        while cfg_dir.join("icon-packs").join(&f).exists() {
            f = format!("{base}-{n}");
            n += 1;
        }
        f
    };
    let dir = cfg_dir.join("icon-packs").join(&folder);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut icons = Vec::new();
    for e in &entries {
        let Some(data) = ar.get(e) else { continue };
        let file = safe(e);
        if dir.join(&file).exists() {
            continue;
        }
        std::fs::write(dir.join(&file), data).map_err(|e| e.to_string())?;
        let stem = file.rsplit_once('.').map(|(a, _)| a).unwrap_or(&file).replace(['_', '-'], " ");
        let name = names.get(&file.to_lowercase()).cloned().unwrap_or(stem);
        icons.push(PackIcon { path: format!("icon-packs/{folder}/{file}"), name });
    }
    let pack = IconPack { name: pack_name.clone(), author, folder: folder.clone(), icons };
    std::fs::write(dir.join("pack.json"), serde_json::to_string_pretty(&pack).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    log::info!("pack d'icônes importé : {pack_name} ({} icônes)", pack.icons.len());
    Ok(ImportReport {
        name: pack_name,
        missing_addons: Vec::new(),
        images: pack.icons.len(),
        sounds: 0,
        unsupported: Vec::new(),
        kind: "icons".into(),
    })
}

/// Packs d'icônes installés.
pub fn list_icon_packs(cfg_dir: &Path) -> Vec<IconPack> {
    let mut out: Vec<IconPack> = std::fs::read_dir(cfg_dir.join("icon-packs"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read_to_string(e.path().join("pack.json")).ok())
        .filter_map(|t| serde_json::from_str(&t).ok())
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Supprime un pack d'icônes (les touches qui l'utilisent affichent alors une erreur d'image).
pub fn remove_icon_pack(cfg_dir: &Path, folder: &str) -> Result<(), String> {
    let folder = safe(folder);
    std::fs::remove_dir_all(cfg_dir.join("icon-packs").join(folder)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut z = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let o = zip::write::SimpleFileOptions::default();
            for (n, d) in files {
                z.start_file(*n, o).unwrap();
                z.write_all(d).unwrap();
            }
            z.finish().unwrap();
        }
        buf
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("hubforge-sd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("profiles")).unwrap();
        d
    }

    #[test]
    fn hotkeys_and_keys() {
        let h: Value = serde_json::from_str(r#"{"KeyCtrl":true,"KeyShift":true,"KeyOption":false,"KeyCmd":false,"VKeyCode":83}"#).unwrap();
        assert_eq!(hotkey_string(&h).unwrap(), "ctrl+shift+s");
        let h: Value = serde_json::from_str(r#"{"KeyCmd":true,"VKeyCode":37}"#).unwrap();
        assert_eq!(hotkey_string(&h).unwrap(), "win+left");
        assert_eq!(vk_name(0x75).unwrap(), "f6");
    }

    #[test]
    fn imports_recent_profile_with_sounds_folders_and_plugins() {
        let png = {
            let img = image::RgbImage::from_pixel(144, 144, image::Rgb([200, 30, 30]));
            let mut b = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
            b
        };
        let page_main = r#"{"Controllers":[{"Type":"Keypad","Actions":{
            "0,0":{"UUID":"com.elgato.streamdeck.soundboard.playaudio","Resources":{"file":"aaa"},
                   "Settings":{"actionType":1,"path":"C:\\x\\Boom.mp3","volume":40},
                   "States":[{"Image":"Images/boom.png","ShowTitle":true,"Title":""}]},
            "1,0":{"UUID":"com.elgato.streamdeck.system.hotkey","Settings":{"Hotkeys":[{"KeyCtrl":true,"VKeyCode":67}]},
                   "States":[{"Title":"Copier"}]},
            "2,0":{"UUID":"com.elgato.streamdeck.profile.openchild","Settings":{"ProfileUUID":"CHILD-1"},"States":[{"Title":"Dossier"}]},
            "3,0":{"UUID":"com.barraider.spotify.play","Name":"Lecture","Plugin":{"Name":"Spotify Integration"},"States":[{"Title":"Spotify"}]},
            "4,0":{"UUID":"com.elgato.streamdeck.multiactions","Actions":[{"Actions":[
                {"UUID":"com.elgato.streamdeck.system.text","Settings":{"pastedText":"gg","isSendingEnter":true}},
                {"UUID":"com.elgato.streamdeck.system.delay","Settings":{"delayValue":300}},
                {"UUID":"com.elgato.streamdeck.system.website","Settings":{"path":"hubforge.example"}}]}],
                "States":[{"Title":"Multi"}]},
            "7,3":{"UUID":"com.elgato.streamdeck.system.multimedia","Settings":{"actionIdx":1},"States":[{"Title":"Suivant"}]}
        }}]}"#;
        let page_child = r#"{"Controllers":[{"Type":"Keypad","Actions":{
            "0,0":{"UUID":"com.elgato.streamdeck.profile.backtoparent","States":[{"Title":"Retour"}]}}}]}"#;
        let top = r#"{"Name":"Mon Deck","Pages":{"Current":"x","Default":"DEF","Pages":["main-1"]},"Version":"3.0"}"#;
        let res = r#"{"resources":{"aaa":{"file":"aaa/Boom.mp3"}}}"#;
        let bytes = zip_of(&[
            ("package.json", b"{}"),
            ("Profiles/P1.sdProfile/manifest.json", top.as_bytes()),
            ("Profiles/P1.sdProfile/Profiles/MAIN-1/manifest.json", page_main.as_bytes()),
            ("Profiles/P1.sdProfile/Profiles/MAIN-1/Images/boom.png", &png),
            ("Profiles/P1.sdProfile/Profiles/CHILD-1/manifest.json", page_child.as_bytes()),
            ("Profiles/P1.sdProfile/Profiles/DEF/manifest.json", br#"{"Controllers":[{"Type":"Keypad","Actions":{}}]}"#),
            ("Resources/manifest.json", res.as_bytes()),
            ("Resources/aaa/Boom.mp3", b"ID3fake"),
        ]);
        let d = tmp("profile");
        let r = import_any(&d, &bytes).unwrap();
        assert_eq!(r.kind, "profile");
        assert_eq!(r.name, "Mon Deck");
        assert_eq!(r.sounds, 1);
        assert_eq!(r.images, 1);
        assert_eq!(r.unsupported, vec!["Spotify Integration".to_string()]);
        let cfg = config::load(&crate::profiles::path(&d, &r.name)).unwrap();
        assert_eq!(cfg.pages.len(), 2, "page principale + dossier (la page par défaut vide est ignorée)");
        let p = &cfg.pages[0];
        let k = |n: u8| p.keys.iter().find(|k| k.key == n).unwrap();
        assert!(matches!(&k(1).action, Action::Sound { volume: 40, mode, file } if mode == "overlap" && file.ends_with("Boom.mp3")));
        assert!(d.join(match &k(1).action { Action::Sound { file, .. } => file.clone(), _ => unreachable!() }).exists());
        assert_eq!(k(1).icon.as_deref().map(|i| d.join(i).exists()), Some(true));
        assert_eq!(k(2).action, Action::Hotkey { keys: "ctrl+c".into() });
        assert_eq!(k(2).label, "Copier");
        assert_eq!(k(3).action, Action::Folder { to: "2".into() });
        assert_eq!(k(4).action, Action::None);
        assert!(matches!(&k(5).action, Action::Macro { steps } if steps.len() == 3));
        // La touche 7,3 (Stream Deck XL) trouve une place libre.
        assert!(p.keys.iter().any(|k| k.action == Action::Media { command: "next".into() }));
        assert_eq!(cfg.pages[1].keys[0].action, Action::Back);
    }

    #[test]
    fn imports_old_profile_format() {
        let man = r#"{"Name":"Ancien","Actions":{"0,0":{"UUID":"com.elgato.streamdeck.system.text","Settings":{"pastedText":"salut"},"States":[{"Title":"Salut"}]}}}"#;
        let png = {
            let mut b = Vec::new();
            image::RgbImage::from_pixel(72, 72, image::Rgb([0, 0, 200])).write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
            b
        };
        let bytes = zip_of(&[("ABC.sdProfile/manifest.json", man.as_bytes()), ("ABC.sdProfile/0,0/CustomImages/state0.png", &png)]);
        let d = tmp("old");
        let r = import_any(&d, &bytes).unwrap();
        let cfg = config::load(&crate::profiles::path(&d, &r.name)).unwrap();
        assert_eq!(cfg.pages[0].keys[0].action, Action::Text { text: "salut".into() });
        assert!(cfg.pages[0].keys[0].icon.is_some());
    }

    #[test]
    fn imports_icon_pack() {
        let png = {
            let mut b = Vec::new();
            image::RgbImage::from_pixel(144, 144, image::Rgb([0, 120, 0])).write_to(&mut std::io::Cursor::new(&mut b), image::ImageFormat::Png).unwrap();
            b
        };
        let bytes = zip_of(&[
            ("com.test.pack.sdIconPack/manifest.json", br#"{"Name":"Pack Test","Author":"Moi"}"#),
            ("com.test.pack.sdIconPack/icons.json", br#"[{"path":"rocket.png","name":"Fusee","tags":[]}]"#),
            ("com.test.pack.sdIconPack/icons/rocket.png", &png),
            ("com.test.pack.sdIconPack/icons/star.png", &png),
            ("com.test.pack.sdIconPack/icon.png", &png),
        ]);
        let d = tmp("pack");
        let r = import_any(&d, &bytes).unwrap();
        assert_eq!(r.kind, "icons");
        assert_eq!(r.images, 2);
        let packs = list_icon_packs(&d);
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].author, "Moi");
        assert!(packs[0].icons.iter().any(|i| i.name == "Fusee"));
        assert!(d.join(&packs[0].icons[0].path).exists());
        remove_icon_pack(&d, &packs[0].folder).unwrap();
        assert!(list_icon_packs(&d).is_empty());
    }

    #[test]
    fn real_profile_if_present() {
        // Le profil d'essai de l'utilisateur, s'il est là (pas dans le dépôt).
        let Ok(bytes) = std::fs::read("/tmp/sdp/p.zip") else { return };
        let d = tmp("real");
        let r = import_any(&d, &bytes).unwrap();
        assert_eq!(r.sounds, 15);
        assert!(r.images >= 15);
        assert!(r.unsupported.is_empty(), "{:?}", r.unsupported);
    }
}
