//! Addons HubForge — niveau 2 : addons avec un programme (« service »).
//!
//! Un addon peut déclarer dans son manifest une section `[service]` : un
//! petit programme (PowerShell, Python, Node.js ou .exe) que HubForge lance
//! en arrière-plan. Ils se parlent par lignes JSON :
//!
//! - HubForge → programme (entrée standard) : `init`, `action` (une touche de
//!   l'addon a été appuyée), `settings` (réglages modifiés), `shutdown`.
//! - programme → HubForge (sortie standard) : `set_var` (variable affichée
//!   sur les touches avec `{addon.nom}`), `notify`, `ok` / `alert` (liseré de
//!   la touche), `log`.
//!
//! Un programme ne démarre qu'avec l'accord explicite de l'utilisateur, donné
//! pour une version précise de l'addon. S'il s'arrête, il est relancé
//! (5 fois au plus en 10 minutes). Format complet : docs/ADDON_GUIDE.md.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Longueur maximale d'une ligne envoyée par un programme.
const MAX_LINE: usize = 64 * 1024;
const MAX_RESTARTS: u32 = 5;
const RESTART_WINDOW: Duration = Duration::from_secs(600);

/// Section `[service]` du manifest.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ServiceDef {
    /// "powershell", "python", "node" ou "exe".
    pub run: String,
    /// Fichier à lancer, dans le dossier de l'addon.
    pub main: String,
}

/// Réglage d'un addon (`[[setting]]`), affiché dans sa fiche.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SettingDef {
    pub id: String,
    pub label: String,
    /// "text", "password", "number" ou "bool".
    pub kind: String,
    pub default: Value,
    pub help: String,
}

pub const RUNTIMES: [&str; 4] = ["powershell", "python", "node", "exe"];
const KINDS: [&str; 4] = ["text", "password", "number", "bool"];

/// Lit `[service]` et `[[setting]]` d'un manifest (déjà analysé en TOML).
pub fn parse(root: &toml::Table, addon_dir: &Path) -> Result<(Option<ServiceDef>, Vec<SettingDef>), String> {
    let service = match root.get("service") {
        None => None,
        Some(v) => {
            let t = v.as_table().ok_or("[service] doit être une section")?;
            let run = t.get("run").and_then(|v| v.as_str()).ok_or("[service] : « run » est obligatoire (powershell, python, node ou exe)")?.to_string();
            if !RUNTIMES.contains(&run.as_str()) {
                return Err(format!("[service] run = « {run} » : powershell, python, node ou exe"));
            }
            let main = t.get("main").and_then(|v| v.as_str()).ok_or("[service] : « main » est obligatoire (ex. main = \"main.ps1\")")?.to_string();
            safe_rel(&main).ok_or_else(|| format!("[service] main = « {main} » : un fichier du dossier de l'addon"))?;
            if !addon_dir.as_os_str().is_empty() && !addon_dir.join(&main).is_file() {
                return Err(format!("[service] : fichier « {main} » introuvable dans l'addon"));
            }
            Some(ServiceDef { run, main })
        }
    };
    let mut settings = Vec::new();
    if let Some(v) = root.get("setting") {
        let arr = v.as_array().ok_or("[[setting]] : liste de réglages attendue")?;
        for s in arr {
            let t = s.as_table().ok_or("[[setting]] : chaque réglage est une section")?;
            let id = t.get("id").and_then(|v| v.as_str()).ok_or("[[setting]] : « id » est obligatoire")?.to_string();
            if !crate::vars::valid_name(&id) || id.contains('.') {
                return Err(format!("[[setting]] id « {id} » : lettres, chiffres, _ et -"));
            }
            if settings.iter().any(|x: &SettingDef| x.id == id) {
                return Err(format!("[[setting]] « {id} » en double"));
            }
            let kind = t.get("type").and_then(|v| v.as_str()).unwrap_or("text").to_string();
            if !KINDS.contains(&kind.as_str()) {
                return Err(format!("[[setting]] « {id} » : type text, password, number ou bool"));
            }
            let default = t.get("default").map(toml_to_json).unwrap_or(match kind.as_str() {
                "bool" => json!(false),
                "number" => json!(0),
                _ => json!(""),
            });
            settings.push(SettingDef {
                label: t.get("label").and_then(|v| v.as_str()).unwrap_or(&id).to_string(),
                help: t.get("help").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                id,
                kind,
                default,
            });
        }
    }
    Ok((service, settings))
}

fn toml_to_json(v: &toml::Value) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// Chemin relatif sûr (pas de « .. », pas de chemin absolu).
fn safe_rel(p: &str) -> Option<PathBuf> {
    let path = Path::new(p);
    if p.is_empty() || path.is_absolute() || path.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(path.to_path_buf())
}

// ----------------------------------------------------------------------
// Réglages enregistrés et accord de l'utilisateur
// ----------------------------------------------------------------------

fn settings_file(cfg_dir: &Path, id: &str) -> PathBuf {
    cfg_dir.join("addon-settings").join(format!("{id}.json"))
}

/// Valeurs des réglages d'un addon (valeurs par défaut pour ceux jamais réglés).
pub fn load_values(cfg_dir: &Path, id: &str, defs: &[SettingDef]) -> serde_json::Map<String, Value> {
    let saved: serde_json::Map<String, Value> =
        std::fs::read_to_string(settings_file(cfg_dir, id)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    defs.iter().map(|d| (d.id.clone(), saved.get(&d.id).cloned().unwrap_or_else(|| d.default.clone()))).collect()
}

pub fn save_values(cfg_dir: &Path, id: &str, values: &serde_json::Map<String, Value>) -> Result<(), String> {
    let path = settings_file(cfg_dir, id);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, serde_json::to_string_pretty(values).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Clé d'accord : l'accord est donné pour une version précise de l'addon.
pub fn trust_key(id: &str, version: &str) -> String {
    format!("{id}@{version}")
}

// ----------------------------------------------------------------------
// Programmes en cours
// ----------------------------------------------------------------------

/// Message d'un programme pour le moteur.
#[derive(Debug, Clone)]
pub enum Incoming {
    SetVar { name: String, value: String },
    Notify { addon: String, text: String, error: bool },
    Mark { page: usize, key: u8, ok: bool },
}

/// État d'un programme, affiché dans la fiche de l'addon.
#[derive(Debug, Clone, Serialize, Default)]
pub struct State {
    /// "running", "stopped", "waiting" (attend l'accord), "error".
    pub state: String,
    pub message: String,
}

struct Proc {
    child: Child,
    stdin: ChildStdin,
}

struct Wanted {
    dir: PathBuf,
    def: ServiceDef,
    settings: serde_json::Map<String, Value>,
}

#[derive(Default)]
struct Host {
    procs: HashMap<String, Proc>,
    wanted: HashMap<String, Wanted>,
    states: HashMap<String, State>,
    crashes: HashMap<String, Vec<Instant>>,
    retry_at: HashMap<String, Instant>,
    tx: Option<Sender<(String, Value)>>,
    rx: Option<Receiver<(String, Value)>>,
}

fn host() -> &'static Mutex<Host> {
    static H: OnceLock<Mutex<Host>> = OnceLock::new();
    H.get_or_init(|| {
        let (tx, rx) = channel();
        Mutex::new(Host { tx: Some(tx), rx: Some(rx), ..Default::default() })
    })
}

/// Addon avec programme, tel que la configuration le décrit.
pub struct Desired {
    pub id: String,
    pub dir: PathBuf,
    pub def: ServiceDef,
    pub settings: serde_json::Map<String, Value>,
    /// Activé et approuvé par l'utilisateur.
    pub allowed: bool,
    /// Pas encore approuvé (mais activé).
    pub needs_trust: bool,
}

/// Démarre, arrête ou redémarre les programmes pour suivre la configuration.
pub fn sync(desired: Vec<Desired>) {
    let mut h = host().lock().unwrap();
    let ids: Vec<String> = desired.iter().map(|d| d.id.clone()).collect();
    // Programmes qui ne sont plus voulus.
    let stale: Vec<String> = h.procs.keys().filter(|id| !desired.iter().any(|d| &d.id == *id && d.allowed)).cloned().collect();
    for id in stale {
        stop_locked(&mut h, &id);
        h.states.insert(id, State { state: "stopped".into(), message: String::new() });
    }
    h.states.retain(|id, _| ids.contains(id));
    h.wanted.clear();
    for d in desired {
        if !d.allowed {
            h.states.insert(d.id.clone(), State { state: if d.needs_trust { "waiting" } else { "stopped" }.into(), message: String::new() });
            continue;
        }
        // Programme ou réglages changés : on relance.
        let changed = h.procs.contains_key(&d.id) && h.wanted_snapshot(&d.id).map(|(def, s)| def != d.def || s != d.settings).unwrap_or(false);
        h.wanted.insert(d.id.clone(), Wanted { dir: d.dir, def: d.def, settings: d.settings });
        if changed {
            stop_locked(&mut h, &d.id);
        }
    }
    let now = Instant::now();
    let todo: Vec<String> = h.wanted.keys().filter(|id| !h.procs.contains_key(*id) && h.retry_at.get(*id).map(|t| now >= *t).unwrap_or(true)).cloned().collect();
    for id in todo {
        if h.states.get(&id).map(|s| s.state == "error").unwrap_or(false) && h.retry_at.get(&id).is_none() {
            continue;
        }
        start_locked(&mut h, &id);
    }
}

impl Host {
    fn wanted_snapshot(&self, id: &str) -> Option<(ServiceDef, serde_json::Map<String, Value>)> {
        self.wanted.get(id).map(|w| (w.def.clone(), w.settings.clone()))
    }
}

fn command_for(dir: &Path, def: &ServiceDef) -> Result<Command, String> {
    let main = dir.join(safe_rel(&def.main).ok_or("fichier invalide")?);
    let mut cmd = match def.run.as_str() {
        "powershell" => {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"]).arg(&main);
            c
        }
        "python" => {
            let mut c = Command::new(if cfg!(windows) { "python" } else { "python3" });
            c.arg("-u").arg(&main);
            c
        }
        "node" => {
            let mut c = Command::new("node");
            c.arg(&main);
            c
        }
        _ => Command::new(&main),
    };
    cmd.current_dir(dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // pas de fenêtre de console
    }
    Ok(cmd)
}

fn start_locked(h: &mut Host, id: &str) {
    let Some(w) = h.wanted.get(id) else { return };
    let spawned = command_for(&w.dir, &w.def).and_then(|mut c| c.spawn().map_err(|e| e.to_string()));
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            let hint = match w.def.run.as_str() {
                "python" => " (Python est-il installé ?)",
                "node" => " (Node.js est-il installé ?)",
                _ => "",
            };
            log::warn!("addon {id} : programme impossible à lancer : {e}");
            h.states.insert(id.into(), State { state: "error".into(), message: format!("impossible à lancer : {e}{hint}") });
            h.retry_at.remove(id);
            return;
        }
    };
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let tx = h.tx.clone().unwrap();
    let addon = id.to_string();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.by_ref().take(MAX_LINE as u64 + 1).read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            if line.len() > MAX_LINE {
                log::warn!("addon {addon} : ligne trop longue ignorée");
                // Fin de la ligne trop longue.
                let mut rest = Vec::new();
                let _ = reader.read_until(b'\n', &mut rest);
                continue;
            }
            let text = String::from_utf8_lossy(&line);
            // Windows PowerShell peut écrire une marque UTF-8 (BOM) en tête.
            let text = text.trim_start_matches('\u{feff}').trim();
            if text.is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(text) {
                Ok(v) => {
                    if tx.send((addon.clone(), v)).is_err() {
                        break;
                    }
                }
                Err(_) => log::info!("addon {addon} : {}", text.chars().take(300).collect::<String>()),
            }
        }
    });
    let addon = id.to_string();
    std::thread::spawn(move || {
        for l in BufReader::new(stderr).lines().map_while(Result::ok).take(10_000) {
            log::warn!("addon {addon} (erreur) : {}", l.chars().take(300).collect::<String>());
        }
    });
    log::info!("addon {id} : programme lancé ({})", w.def.main);
    let init = json!({
        "type": "init",
        "addon": id,
        "hubforge": env!("CARGO_PKG_VERSION"),
        "lang": if crate::i18n::en() { "en" } else { "fr" },
        "settings": w.settings,
    });
    let mut p = Proc { child, stdin };
    let _ = writeln!(p.stdin, "{init}");
    let _ = p.stdin.flush();
    h.procs.insert(id.into(), p);
    h.states.insert(id.into(), State { state: "running".into(), message: String::new() });
}

fn stop_locked(h: &mut Host, id: &str) {
    if let Some(mut p) = h.procs.remove(id) {
        let _ = writeln!(p.stdin, "{}", json!({ "type": "shutdown" }));
        let _ = p.stdin.flush();
        drop(p.stdin);
        // Une seconde pour s'arrêter proprement.
        for _ in 0..10 {
            if matches!(p.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = p.child.kill();
        let _ = p.child.wait();
    }
}

/// Arrête tous les programmes (fermeture de HubForge).
pub fn stop_all() {
    let mut h = host().lock().unwrap();
    let ids: Vec<String> = h.procs.keys().cloned().collect();
    for id in ids {
        stop_locked(&mut h, &id);
    }
    h.wanted.clear();
}

/// Envoie un message à un programme. Faux s'il ne tourne pas.
pub fn send(id: &str, msg: &Value) -> bool {
    let mut h = host().lock().unwrap();
    let Some(p) = h.procs.get_mut(id) else { return false };
    let ok = writeln!(p.stdin, "{msg}").and_then(|_| p.stdin.flush()).is_ok();
    ok
}

/// Messages reçus des programmes, et surveillance des programmes arrêtés.
pub fn poll() -> Vec<Incoming> {
    let mut h = host().lock().unwrap();
    // Programmes arrêtés tout seuls : on relance, avec une limite.
    let dead: Vec<(String, Option<i32>)> =
        h.procs.iter_mut().filter_map(|(id, p)| match p.child.try_wait() { Ok(Some(st)) => Some((id.clone(), st.code())), _ => None }).collect();
    let now = Instant::now();
    for (id, code) in dead {
        h.procs.remove(&id);
        let crashes = h.crashes.entry(id.clone()).or_default();
        crashes.retain(|t| now.duration_since(*t) < RESTART_WINDOW);
        crashes.push(now);
        let n = crashes.len() as u32;
        if n > MAX_RESTARTS {
            log::warn!("addon {id} : programme arrêté trop souvent, il n'est plus relancé");
            h.states.insert(id.clone(), State { state: "error".into(), message: format!("arrêté {n} fois en 10 minutes (code {code:?}) : relance l'addon depuis sa fiche") });
            h.retry_at.remove(&id);
        } else {
            let delay = Duration::from_secs(2u64.pow(n.min(5)));
            log::warn!("addon {id} : programme arrêté (code {code:?}), relance dans {} s", delay.as_secs());
            h.states.insert(id.clone(), State { state: "error".into(), message: format!("arrêté (code {code:?}), relance dans {} s", delay.as_secs()) });
            h.retry_at.insert(id, now + delay);
        }
    }
    let due: Vec<String> =
        h.retry_at.iter().filter(|(id, t)| now >= **t && !h.procs.contains_key(*id) && h.wanted.contains_key(*id)).map(|(id, _)| id.clone()).collect();
    for id in due {
        h.retry_at.remove(&id);
        start_locked(&mut h, &id);
    }
    let mut out = Vec::new();
    if let Some(rx) = &h.rx {
        while let Ok((addon, v)) = rx.try_recv() {
            if let Some(m) = interpret(&addon, &v) {
                out.push(m);
            }
            if out.len() > 500 {
                break;
            }
        }
    }
    out
}

/// Traduit un message JSON d'un programme.
fn interpret(addon: &str, v: &Value) -> Option<Incoming> {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
        "set_var" => {
            let name = s("name");
            if !crate::vars::valid_name(&name) || name.contains('.') {
                log::warn!("addon {addon} : nom de variable invalide « {name} »");
                return None;
            }
            let value = match v.get("value") {
                Some(Value::String(x)) => x.clone(),
                Some(Value::Null) | None => String::new(),
                Some(x) => x.to_string(),
            };
            Some(Incoming::SetVar { name: format!("{addon}.{name}"), value: crate::connect::clip(&value) })
        }
        "notify" => Some(Incoming::Notify { addon: addon.into(), text: s("text").chars().take(300).collect(), error: s("level") == "error" }),
        "ok" | "alert" => {
            let k = v.get("key")?;
            let page = k.get("page")?.as_u64()? as usize;
            let key = k.get("key")?.as_u64()?;
            (1..=15).contains(&key).then(|| Incoming::Mark { page: page.saturating_sub(1), key: key as u8 - 1, ok: s("type") == "ok" })
        }
        "log" => {
            log::info!("addon {addon} : {}", s("text").chars().take(300).collect::<String>());
            None
        }
        other => {
            log::warn!("addon {addon} : message inconnu « {other} »");
            None
        }
    }
}

/// États des programmes, pour la fiche des addons.
pub fn states() -> HashMap<String, State> {
    host().lock().unwrap().states.clone()
}

/// Relancer un programme arrêté en erreur (bouton de la fiche).
pub fn restart(id: &str) {
    let mut h = host().lock().unwrap();
    h.crashes.remove(id);
    h.retry_at.remove(id);
    stop_locked(&mut h, id);
    if h.wanted.contains_key(id) {
        start_locked(&mut h, id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_service_and_settings() {
        let t: toml::Table = toml::from_str(
            r#"
[service]
run = "powershell"
main = "main.ps1"

[[setting]]
id = "host"
label = "Adresse"
default = "localhost"

[[setting]]
id = "port"
type = "number"
default = 4455

[[setting]]
id = "secret"
type = "password"
"#,
        )
        .unwrap();
        let (svc, set) = parse(&t, Path::new("")).unwrap();
        assert_eq!(svc.unwrap().run, "powershell");
        assert_eq!(set.len(), 3);
        assert_eq!(set[1].default, json!(4455));
        assert_eq!(set[2].default, json!(""));
        let bad: toml::Table = toml::from_str("[service]\nrun = \"bash\"\nmain = \"x.sh\"").unwrap();
        assert!(parse(&bad, Path::new("")).is_err());
        let escape: toml::Table = toml::from_str("[service]\nrun = \"exe\"\nmain = \"../evil.exe\"").unwrap();
        assert!(parse(&escape, Path::new("")).is_err());
    }

    #[test]
    fn interprets_messages() {
        match interpret("obs", &json!({"type": "set_var", "name": "scene", "value": "Jeu"})) {
            Some(Incoming::SetVar { name, value }) => assert_eq!((name.as_str(), value.as_str()), ("obs.scene", "Jeu")),
            _ => panic!(),
        }
        match interpret("obs", &json!({"type": "set_var", "name": "live", "value": true})) {
            Some(Incoming::SetVar { value, .. }) => assert_eq!(value, "true"),
            _ => panic!(),
        }
        assert!(interpret("obs", &json!({"type": "set_var", "name": "a b"})).is_none());
        match interpret("obs", &json!({"type": "alert", "key": {"page": 2, "key": 4}})) {
            Some(Incoming::Mark { page, key, ok }) => assert_eq!((page, key, ok), (1, 3, false)),
            _ => panic!(),
        }
        assert!(interpret("obs", &json!({"type": "ok", "key": {"page": 1, "key": 99}})).is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn runs_a_program_and_talks_json() {
        let d = std::env::temp_dir().join(format!("hubforge-svc-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        // Programme d'essai : répond à chaque action par une variable.
        std::fs::write(
            d.join("main.py"),
            "import sys, json\nfor line in sys.stdin:\n    m = json.loads(line)\n    if m['type'] == 'init':\n        print(json.dumps({'type': 'set_var', 'name': 'hello', 'value': m['settings']['who']}), flush=True)\n    elif m['type'] == 'action':\n        print(json.dumps({'type': 'set_var', 'name': 'count', 'value': m['data']['n']}), flush=True)\n    elif m['type'] == 'shutdown':\n        break\n",
        )
        .unwrap();
        let mut settings = serde_json::Map::new();
        settings.insert("who".into(), json!("Benjamin"));
        sync(vec![Desired {
            id: "demo".into(),
            dir: d.clone(),
            def: ServiceDef { run: "python".into(), main: "main.py".into() },
            settings,
            allowed: true,
            needs_trust: false,
        }]);
        assert!(send("demo", &json!({"type": "action", "event": "x", "data": {"n": 7}})));
        let mut got = Vec::new();
        for _ in 0..50 {
            for m in poll() {
                if let Incoming::SetVar { name, value } = m {
                    got.push(format!("{name}={value}"));
                }
            }
            if got.len() >= 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(got, vec!["demo.hello=Benjamin".to_string(), "demo.count=7".to_string()]);
        assert_eq!(states()["demo"].state, "running");
        stop_all();
    }
}
