//! Addons HubForge — niveau 1 (déclaratif, sans code).
//!
//! Un addon est un dossier `addons/<id>/` qui contient un `manifest.toml`
//! et des icônes. Un fichier `.forge` (archive ZIP) déposé dans `addons/`
//! ou sur la fenêtre est décompressé dans ce dossier.
//!
//! Un addon apporte :
//! - des actions prêtes à l'emploi (catégorie dans la bibliothèque) ;
//! - des pages prêtes à l'emploi (ajoutées au profil en un clic) ;
//! - la détection de son application (pages automatiques).
//!
//! Format complet : docs/ADDON_GUIDE.md.

use crate::config::{parse_color, Action, KeyDef, Page};
use serde::Serialize;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Version du format de manifeste comprise par cette version de HubForge.
pub const FORMAT: i64 = 1;
const MAX_ARCHIVE_FILES: usize = 2000;
const MAX_ARCHIVE_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct Category {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddonAction {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    /// Texte affiché sur la touche.
    pub label: String,
    /// "addon:<id>/icons/x.svg", "builtin:nom" ou rien.
    pub icon: Option<String>,
    pub color: Option<String>,
    pub text_color: Option<String>,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize)]
pub struct PageKey {
    pub key: u8,
    /// Action de l'addon placée sur la touche.
    pub action: Option<String>,
    /// Ouvre une autre page de l'addon (dossier).
    pub open: Option<String>,
    /// Touche « Retour ».
    pub back: bool,
    pub label: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AddonPage {
    pub id: String,
    pub name: String,
    pub folder: bool,
    pub close_after: bool,
    /// Applications liées (par défaut : celles de [detect]).
    pub apps: Vec<String>,
    pub keys: Vec<PageKey>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Addon {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub license: String,
    pub description: String,
    /// Icône de l'addon (chemin "addon:…", "builtin:…" ou "auto").
    pub icon: Option<String>,
    pub detect: Vec<String>,
    pub categories: Vec<Category>,
    pub actions: Vec<AddonAction>,
    pub pages: Vec<AddonPage>,
}

/// Addon trouvé sur le disque : chargé, ou en erreur (message précis).
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    /// Nom du dossier.
    pub folder: String,
    pub addon: Option<Addon>,
    pub error: Option<String>,
}

pub fn dir(cfg_dir: &Path) -> PathBuf {
    cfg_dir.join("addons")
}

/// Chemin réel d'une icône "addon:<id>/<chemin>" (None si le chemin sort du dossier).
pub fn resolve_icon(cfg_dir: &Path, icon: &str) -> Option<PathBuf> {
    let rest = icon.strip_prefix("addon:")?;
    let rel = Path::new(rest);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(dir(cfg_dir).join(rel))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

// ----------------------------------------------------------------------
// Lecture du manifeste
// ----------------------------------------------------------------------

fn get_str(t: &toml::Table, key: &str) -> Option<String> {
    t.get(key).and_then(|v| v.as_str()).map(|s| s.trim().to_string())
}

fn req_str(t: &toml::Table, key: &str, ctx: &str) -> Result<String, String> {
    get_str(t, key).filter(|s| !s.is_empty()).ok_or_else(|| format!("{ctx} : le champ « {key} » est obligatoire"))
}

fn str_list(t: &toml::Table, key: &str, ctx: &str) -> Result<Vec<String>, String> {
    match t.get(key) {
        None => Ok(Vec::new()),
        Some(toml::Value::Array(a)) => a
            .iter()
            .map(|v| v.as_str().map(|s| s.trim().to_string()).ok_or_else(|| format!("{ctx} : « {key} » doit être une liste de textes")))
            .collect(),
        Some(_) => Err(format!("{ctx} : « {key} » doit être une liste, ex. {key} = [\"appli.exe\"]")),
    }
}

fn tables<'a>(root: &'a toml::Table, key: &str) -> Result<Vec<&'a toml::Table>, String> {
    match root.get(key) {
        None => Ok(Vec::new()),
        Some(toml::Value::Array(a)) => a
            .iter()
            .map(|v| v.as_table().ok_or_else(|| format!("[[{key}]] mal écrit (utilise [[{key}]] pour chaque élément)")))
            .collect(),
        Some(_) => Err(format!("« {key} » mal écrit : utilise [[{key}]] pour chaque élément")),
    }
}

/// Vérifie une icône de l'addon et la transforme en référence utilisable.
fn icon_ref(addon_id: &str, addon_dir: &Path, icon: Option<String>, ctx: &str) -> Result<Option<String>, String> {
    let Some(icon) = icon.filter(|s| !s.is_empty()) else { return Ok(None) };
    if icon.starts_with("builtin:") || icon == "auto" {
        return Ok(Some(icon));
    }
    let rel = Path::new(&icon);
    if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(format!("{ctx} : l'icône « {icon} » doit être un chemin dans l'addon (ex. icons/extrude.svg)"));
    }
    let ext = rel.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !matches!(ext.as_str(), "svg" | "png" | "jpg" | "jpeg" | "gif" | "webp") {
        return Err(format!("{ctx} : icône « {icon} » : formats acceptés svg, png, jpg, gif, webp"));
    }
    if !addon_dir.join(rel).is_file() {
        return Err(format!("{ctx} : icône introuvable « {icon} »"));
    }
    Ok(Some(format!("addon:{addon_id}/{}", icon.replace('\\', "/"))))
}

fn check_color(c: &Option<String>, ctx: &str) -> Result<(), String> {
    match c {
        Some(c) if parse_color(c).is_none() => Err(format!("{ctx} : couleur invalide « {c} » (format #RRVVBB)")),
        _ => Ok(()),
    }
}

/// Lit et vérifie un manifeste. `folder` = nom du dossier de l'addon.
pub fn parse_manifest(text: &str, addon_dir: &Path) -> Result<Addon, String> {
    let root: toml::Table = toml::from_str(text).map_err(|e| format!("manifest.toml : {e}"))?;
    let a = root.get("addon").and_then(|v| v.as_table()).ok_or("manifest.toml : la section [addon] est obligatoire")?;
    let id = req_str(a, "id", "[addon]")?;
    if !valid_id(&id) {
        return Err(format!("[addon] id « {id} » : uniquement minuscules, chiffres, - et _ (ex. fusion360)"));
    }
    let format = a.get("format").and_then(|v| v.as_integer()).unwrap_or(1);
    if format > FORMAT {
        return Err(format!("cet addon demande le format {format} : mets HubForge à jour"));
    }
    let name = req_str(a, "name", "[addon]")?;
    let icon = icon_ref(&id, addon_dir, get_str(a, "icon"), "[addon]")?;

    let detect = match root.get("detect").and_then(|v| v.as_table()) {
        Some(d) => str_list(d, "windows", "[detect]")?,
        None => Vec::new(),
    };

    let mut categories = Vec::new();
    for c in tables(&root, "category")? {
        let cid = req_str(c, "id", "[[category]]")?;
        let cname = get_str(c, "name").unwrap_or_else(|| cid.clone());
        categories.push(Category { id: cid, name: cname });
    }

    let mut actions: Vec<AddonAction> = Vec::new();
    let mut seen = HashSet::new();
    for t in tables(&root, "action")? {
        let aid = req_str(t, "id", "[[action]]")?;
        let ctx = format!("action « {aid} »");
        if !seen.insert(aid.clone()) {
            return Err(format!("{ctx} : identifiant en double"));
        }
        let aname = req_str(t, "name", &ctx)?;
        let category = get_str(t, "category").unwrap_or_default();
        if !category.is_empty() && !categories.iter().any(|c| c.id == category) {
            return Err(format!("{ctx} : catégorie « {category} » inconnue (ajoute un [[category]] avec id = \"{category}\")"));
        }
        // Les champs restants décrivent l'action elle-même (comme dans config.toml).
        let mut act = t.clone();
        for meta in ["id", "name", "category", "description", "label", "icon", "color", "text_color"] {
            act.remove(meta);
        }
        if !act.contains_key("type") {
            return Err(format!("{ctx} : le champ « type » est obligatoire (hotkey, text, launch, media, macro…)"));
        }
        let action: Action = toml::Value::Table(act).try_into().map_err(|e| format!("{ctx} : {e}"))?;
        if matches!(action, Action::Game | Action::Profile { .. } | Action::OpenConfig) {
            return Err(format!("{ctx} : ce type d'action n'est pas autorisé dans un addon"));
        }
        let color = get_str(t, "color");
        let text_color = get_str(t, "text_color");
        check_color(&color, &ctx)?;
        check_color(&text_color, &ctx)?;
        actions.push(AddonAction {
            label: get_str(t, "label").unwrap_or_else(|| aname.clone()),
            icon: icon_ref(&id, addon_dir, get_str(t, "icon"), &ctx)?,
            id: aid,
            name: aname,
            category,
            description: get_str(t, "description").unwrap_or_default(),
            color,
            text_color,
            action,
        });
    }

    let page_tables = tables(&root, "page")?;
    let page_ids: Vec<String> = page_tables.iter().filter_map(|p| get_str(p, "id")).collect();
    let mut pages = Vec::new();
    for p in page_tables {
        let pid = req_str(p, "id", "[[page]]")?;
        let ctx = format!("page « {pid} »");
        let pname = req_str(p, "name", &ctx)?;
        let folder = p.get("folder").and_then(|v| v.as_bool()).unwrap_or(false);
        let close_after = p.get("close_after").and_then(|v| v.as_bool()).unwrap_or(false);
        let apps = if p.contains_key("apps") {
            str_list(p, "apps", &ctx)?
        } else if folder {
            Vec::new()
        } else {
            detect.clone()
        };
        let mut keys = Vec::new();
        let mut used = HashSet::new();
        let key_tables: Vec<&toml::Table> = match p.get("keys") {
            None => Vec::new(),
            Some(toml::Value::Array(a)) => {
                a.iter().map(|v| v.as_table().ok_or_else(|| format!("{ctx} : keys = [ {{ key = 1, action = \"…\" }}, … ]"))).collect::<Result<_, _>>()?
            }
            Some(_) => return Err(format!("{ctx} : keys doit être une liste")),
        };
        for k in key_tables {
            let key = k.get("key").and_then(|v| v.as_integer()).ok_or_else(|| format!("{ctx} : chaque touche a besoin de « key » (1 à 15)"))?;
            if !(1..=15).contains(&key) || !used.insert(key) {
                return Err(format!("{ctx} : touche {key} invalide ou en double (1 à 15)"));
            }
            let action = get_str(k, "action");
            let open = get_str(k, "open");
            let back = k.get("back").and_then(|v| v.as_bool()).unwrap_or(false);
            if let Some(a) = &action {
                if !actions.iter().any(|x| &x.id == a) {
                    return Err(format!("{ctx} touche {key} : action « {a} » inconnue"));
                }
            }
            if let Some(o) = &open {
                if !page_ids.contains(o) {
                    return Err(format!("{ctx} touche {key} : page « {o} » inconnue"));
                }
            }
            if action.is_none() && open.is_none() && !back {
                return Err(format!("{ctx} touche {key} : indique action = \"…\", open = \"…\" ou back = true"));
            }
            let color = get_str(k, "color");
            check_color(&color, &format!("{ctx} touche {key}"))?;
            keys.push(PageKey { key: key as u8, action, open, back, label: get_str(k, "label"), color });
        }
        pages.push(AddonPage { id: pid, name: pname, folder, close_after, apps, keys });
    }

    Ok(Addon {
        id,
        name,
        version: get_str(a, "version").unwrap_or_else(|| "1.0.0".into()),
        author: get_str(a, "author").unwrap_or_default(),
        license: get_str(a, "license").unwrap_or_default(),
        description: get_str(a, "description").unwrap_or_default(),
        icon,
        detect,
        categories,
        actions,
        pages,
    })
}

// ----------------------------------------------------------------------
// Liste, installation, suppression
// ----------------------------------------------------------------------

/// Addons du dossier (les archives .forge déposées à la main sont d'abord installées).
pub fn scan(cfg_dir: &Path) -> Vec<Entry> {
    let d = dir(cfg_dir);
    let _ = std::fs::create_dir_all(&d);
    // .forge déposés directement dans le dossier : on les installe.
    if let Ok(rd) = std::fs::read_dir(&d) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().map(|x| x.eq_ignore_ascii_case("forge")).unwrap_or(false) {
                match std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| install_bytes(cfg_dir, &b)) {
                    Ok(id) => {
                        log::info!("addon « {id} » installé depuis {}", p.display());
                        let _ = std::fs::remove_file(&p);
                    }
                    Err(err) => {
                        log::error!("{} : {err}", p.display());
                        let _ = std::fs::rename(&p, p.with_extension("forge.erreur"));
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&d) {
        for e in rd.flatten() {
            let p = e.path();
            let manifest = p.join("manifest.toml");
            if !p.is_dir() || !manifest.is_file() {
                continue;
            }
            let folder = e.file_name().to_string_lossy().to_string();
            let res = std::fs::read_to_string(&manifest)
                .map_err(|e| format!("lecture de manifest.toml impossible : {e}"))
                .and_then(|t| parse_manifest(&t, &p))
                .and_then(|a| {
                    if a.id == folder {
                        Ok(a)
                    } else {
                        Err(format!("le dossier doit s'appeler comme l'id de l'addon (« {} »)", a.id))
                    }
                });
            match res {
                Ok(a) => out.push(Entry { folder, addon: Some(a), error: None }),
                Err(e) => out.push(Entry { folder, addon: None, error: Some(e) }),
            }
        }
    }
    out.sort_by_key(|e| e.addon.as_ref().map(|a| a.name.to_lowercase()).unwrap_or_else(|| e.folder.clone()));
    out
}

/// Empreinte du dossier des addons : change quand on ajoute, retire ou
/// modifie un addon (rechargement à chaud de l'interface).
pub fn signature(cfg_dir: &Path) -> String {
    let mut parts = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir(cfg_dir)) {
        for e in rd.flatten() {
            let p = e.path();
            let m = if p.is_dir() { p.join("manifest.toml") } else { p.clone() };
            let t = std::fs::metadata(&m).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
            parts.push(format!("{}:{}", e.file_name().to_string_lossy(), t.map(|d| d.as_millis()).unwrap_or(0)));
        }
    }
    parts.sort();
    parts.join("|")
}

/// Installe une archive .forge (contenu en mémoire) ; retourne l'id.
pub fn install_bytes(cfg_dir: &Path, bytes: &[u8]) -> Result<String, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "ce fichier n'est pas un addon .forge valide (archive ZIP attendue)".to_string())?;
    if zip.len() > MAX_ARCHIVE_FILES {
        return Err("archive trop grande (trop de fichiers)".into());
    }
    // Le manifeste peut être à la racine ou dans un unique dossier.
    let mut manifest_name = None;
    for i in 0..zip.len() {
        let f = zip.by_index(i).map_err(|e| e.to_string())?;
        let n = f.name().replace('\\', "/");
        if n == "manifest.toml" || (n.ends_with("/manifest.toml") && n.matches('/').count() == 1) {
            if manifest_name.as_ref().map(|m: &String| n.len() < m.len()).unwrap_or(true) {
                manifest_name = Some(n);
            }
        }
    }
    let manifest_name = manifest_name.ok_or("manifest.toml introuvable dans l'archive")?;
    let prefix = manifest_name.trim_end_matches("manifest.toml").to_string();

    // Extraction dans un dossier temporaire, vérification, puis remplacement.
    let d = dir(cfg_dir);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let tmp = d.join(format!(".installation-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = (|| -> Result<String, String> {
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            let Some(path) = f.enclosed_name() else { return Err(format!("chemin dangereux dans l'archive : {}", f.name())) };
            let name = path.to_string_lossy().replace('\\', "/");
            let Some(rel) = name.strip_prefix(&prefix) else { continue };
            if rel.is_empty() {
                continue;
            }
            let out = tmp.join(rel);
            if f.is_dir() {
                std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
                continue;
            }
            total += f.size();
            if total > MAX_ARCHIVE_BYTES {
                return Err("archive trop grande (50 Mo maximum)".into());
            }
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut buf = Vec::with_capacity(f.size() as usize);
            f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            std::fs::write(&out, buf).map_err(|e| e.to_string())?;
        }
        let text = std::fs::read_to_string(tmp.join("manifest.toml")).map_err(|e| e.to_string())?;
        let addon = parse_manifest(&text, &tmp)?;
        let target = d.join(&addon.id);
        if target.exists() {
            std::fs::remove_dir_all(&target).map_err(|e| format!("remplacement de l'ancienne version impossible : {e}"))?;
        }
        std::fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
        Ok(addon.id)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

pub fn remove(cfg_dir: &Path, id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("addon inconnu".into());
    }
    let p = dir(cfg_dir).join(id);
    if !p.is_dir() {
        return Err("addon introuvable".into());
    }
    std::fs::remove_dir_all(&p).map_err(|e| format!("suppression impossible : {e}"))
}

// ----------------------------------------------------------------------
// Conversion en touches et pages de la configuration
// ----------------------------------------------------------------------

impl AddonAction {
    pub fn key_def(&self, key: u8) -> KeyDef {
        KeyDef {
            key,
            label: self.label.clone(),
            icon: self.icon.clone(),
            color: self.color.clone(),
            text_color: self.text_color.clone(),
            action: self.action.clone(),
            toggle: None,
            widget: None,
            widget_style: None,
        }
    }
}

impl Addon {
    /// Pages prêtes à l'emploi, à ajouter à la fin du profil : `base` = nombre
    /// de pages actuel (pour numéroter les dossiers).
    pub fn build_pages(&self, base: usize) -> Vec<Page> {
        let index = |pid: &str| self.pages.iter().position(|p| p.id == pid).map(|i| base + i + 1);
        self.pages
            .iter()
            .map(|p| Page {
                name: p.name.clone(),
                apps: p.apps.clone(),
                folder: p.folder,
                close_after: p.close_after,
                keys: p
                    .keys
                    .iter()
                    .map(|k| {
                        let mut def = if let Some(a) = k.action.as_ref().and_then(|a| self.actions.iter().find(|x| &x.id == a)) {
                            a.key_def(k.key)
                        } else if k.back {
                            KeyDef {
                                key: k.key,
                                label: "Retour".into(),
                                icon: Some("builtin:arrow-left".into()),
                                color: Some("#212121".into()),
                                text_color: None,
                                action: Action::Back,
                                toggle: None,
                                widget: None,
                                widget_style: None,
                            }
                        } else {
                            let target = k.open.as_deref().and_then(|o| self.pages.iter().find(|p| p.id == o));
                            KeyDef {
                                key: k.key,
                                label: target.map(|t| t.name.clone()).unwrap_or_default(),
                                icon: Some("builtin:folder".into()),
                                color: Some("#F4B400".into()),
                                text_color: None,
                                action: Action::Folder { to: k.open.as_deref().and_then(index).map(|n| n.to_string()).unwrap_or_default() },
                                toggle: None,
                                widget: None,
                                widget_style: None,
                            }
                        };
                        if let Some(l) = &k.label {
                            def.label = l.clone();
                        }
                        if let Some(c) = &k.color {
                            def.color = Some(c.clone());
                        }
                        def
                    })
                    .collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hubforge-addon-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const MANIFEST: &str = r##"
[addon]
id = "demo"
name = "Démo"
version = "1.2.0"
license = "MIT"

[detect]
windows = ["Demo.exe"]

[[category]]
id = "base"
name = "Base"

[[action]]
id = "save"
name = "Enregistrer"
category = "base"
icon = "builtin:save"
color = "#2E7D32"
type = "hotkey"
keys = "ctrl+s"

[[action]]
id = "hello"
name = "Bonjour"
type = "macro"
steps = [ { type = "text", text = "Bonjour" }, { type = "wait", ms = 100 } ]

[[page]]
id = "main"
name = "Démo"
keys = [ { key = 1, action = "save" }, { key = 15, open = "more" } ]

[[page]]
id = "more"
name = "Démo +"
folder = true
keys = [ { key = 1, back = true }, { key = 2, action = "hello", label = "Salut" } ]
"##;

    #[test]
    fn parses_and_builds_pages() {
        let d = tmp("parse");
        let a = parse_manifest(MANIFEST, &d).unwrap();
        assert_eq!(a.id, "demo");
        assert_eq!(a.actions.len(), 2);
        assert_eq!(a.actions[0].action, Action::Hotkey { keys: "ctrl+s".into() });
        assert!(matches!(a.actions[1].action, Action::Macro { .. }));
        assert_eq!(a.pages[0].apps, vec!["Demo.exe"]);
        assert!(a.pages[1].apps.is_empty());

        let pages = a.build_pages(2);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].keys[1].action, Action::Folder { to: "4".into() });
        assert_eq!(pages[1].keys[0].action, Action::Back);
        assert_eq!(pages[1].keys[1].label, "Salut");
        assert!(pages[1].folder);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn clear_errors() {
        let d = tmp("errors");
        let err = |m: &str| parse_manifest(m, &d).unwrap_err();
        assert!(err("x = 1").contains("[addon]"));
        assert!(err("[addon]\nid = \"Fusion 360\"\nname = \"x\"").contains("minuscules"));
        assert!(err("[addon]\nid = \"a\"\nname = \"x\"\n[[action]]\nid = \"b\"\nname = \"B\"").contains("type"));
        assert!(err("[addon]\nid = \"a\"\nname = \"x\"\n[[action]]\nid = \"b\"\nname = \"B\"\ntype = \"hotkey\"").contains("action « b »"));
        assert!(err("[addon]\nid = \"a\"\nname = \"x\"\n[[action]]\nid = \"b\"\nname = \"B\"\ntype = \"none\"\nicon = \"../x.png\"").contains("chemin"));
        assert!(err("[addon]\nid = \"a\"\nname = \"x\"\nformat = 99").contains("mets HubForge à jour"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn installs_forge_archive() {
        use std::io::Write;
        let d = tmp("install");
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("demo/manifest.toml", o).unwrap();
            z.write_all(MANIFEST.as_bytes()).unwrap();
            z.start_file("demo/icons/x.svg", o).unwrap();
            z.write_all(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
            z.finish().unwrap();
        }
        let id = install_bytes(&d, buf.get_ref()).unwrap();
        assert_eq!(id, "demo");
        assert!(dir(&d).join("demo/icons/x.svg").is_file());
        let list = scan(&d);
        assert_eq!(list.len(), 1);
        assert!(list[0].error.is_none());
        assert!(resolve_icon(&d, "addon:demo/icons/x.svg").unwrap().is_file());
        assert!(resolve_icon(&d, "addon:../../etc/passwd").is_none());
        remove(&d, "demo").unwrap();
        assert!(scan(&d).is_empty());
        assert!(install_bytes(&d, b"pas un zip").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
