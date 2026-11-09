//! Variables des macros : compteurs, interrupteurs, textes.
//!
//! Elles sont gardées dans `variables.json` (dossier de HubForge) et survivent
//! donc à un redémarrage. Dans un texte, `{nom}` est remplacé par la valeur de
//! la variable ; quelques noms spéciaux donnent l'état du PC (`{app}`,
//! `{page}`, `{profile}`, `{hour}`, `{minute}`, `{weekday}`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Nombre maximal de variables et longueur maximale d'une valeur.
pub const MAX_VARS: usize = 500;
pub const MAX_VALUE_LEN: usize = 200;

/// Noms réservés, calculés à la demande.
pub const SPECIALS: [&str; 6] = ["app", "page", "profile", "hour", "minute", "weekday"];

pub struct Vars {
    map: BTreeMap<String, String>,
    path: PathBuf,
    dirty: bool,
    changed_at: Instant,
}

/// Nom de variable : lettres, chiffres, « _ » et « - », 1 à 40 caractères.
pub fn valid_name(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty() && n.chars().count() <= 40 && n.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Nom entre accolades : sans espaces autour (« { rien } » reste du texte).
fn placeholder(name: &str) -> bool {
    name == name.trim() && valid_name(name)
}

fn key(name: &str) -> String {
    name.trim().to_lowercase()
}

impl Vars {
    pub fn load(dir: &Path) -> Vars {
        let path = dir.join("variables.json");
        let map = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<BTreeMap<String, String>>(&t).ok())
            .unwrap_or_default();
        Vars { map, path, dirty: false, changed_at: Instant::now() }
    }

    #[cfg(test)]
    pub fn in_memory() -> Vars {
        Vars { map: BTreeMap::new(), path: PathBuf::new(), dirty: false, changed_at: Instant::now() }
    }

    pub fn get(&self, name: &str) -> Option<&String> {
        self.map.get(&key(name))
    }

    pub fn all(&self) -> &BTreeMap<String, String> {
        &self.map
    }

    /// Applique une opération ; retourne vrai si la valeur a changé.
    /// `value` doit déjà être développée ({variables} remplacées).
    pub fn apply(&mut self, name: &str, op: &str, value: &str) -> bool {
        if !valid_name(name) {
            return false;
        }
        let k = key(name);
        let cur = self.map.get(&k).cloned().unwrap_or_default();
        let new = match op {
            "add" => {
                let a = number(&cur).unwrap_or(0.0);
                let b = number(value).unwrap_or(0.0);
                format_number(a + b)
            }
            "toggle" => {
                if truthy(&cur) {
                    "0".into()
                } else {
                    "1".into()
                }
            }
            _ => value.chars().take(MAX_VALUE_LEN).collect(),
        };
        if new == cur && self.map.contains_key(&k) {
            return false;
        }
        if !self.map.contains_key(&k) && self.map.len() >= MAX_VARS {
            log::warn!("trop de variables ({MAX_VARS}) : « {name} » ignorée");
            return false;
        }
        log::info!("variable {k} = {new}");
        self.map.insert(k, new);
        self.dirty = true;
        self.changed_at = Instant::now();
        true
    }

    /// Remet toutes les variables à zéro (efface le fichier).
    pub fn clear(&mut self) {
        self.map.clear();
        self.dirty = true;
        self.changed_at = Instant::now() - Duration::from_secs(10);
    }

    /// Enregistre si besoin, au plus une fois par seconde (une molette peut
    /// changer un compteur des dizaines de fois par seconde).
    pub fn save_if_due(&mut self) {
        if self.dirty && self.changed_at.elapsed() >= Duration::from_secs(1) {
            self.save_now();
        }
    }

    pub fn save_now(&mut self) {
        if !self.dirty || self.path.as_os_str().is_empty() {
            return;
        }
        self.dirty = false;
        let text = serde_json::to_string_pretty(&self.map).unwrap_or_else(|_| "{}".into());
        let tmp = self.path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &self.path)).is_err() {
            log::warn!("variables : enregistrement impossible");
        }
    }
}

/// Remplace chaque `{nom}` dont `lookup` connaît la valeur. Les autres
/// accolades restent telles quelles (un texte ordinaire n'est pas abîmé).
pub fn expand(text: &str, lookup: &dyn Fn(&str) -> Option<String>) -> String {
    if !text.contains('{') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) if placeholder(&after[..end]) => {
                let name = &after[..end];
                match lookup(&key(name)) {
                    Some(v) => out.push_str(&v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Texte d'une touche : chaque {variable} est remplacée par sa valeur
/// (0 si elle n'a pas encore de valeur).
pub fn expand_label(text: &str, vars: &BTreeMap<String, String>) -> String {
    expand(text, &|n| Some(vars.get(n).cloned().unwrap_or_else(|| "0".into())))
}

/// Une touche dont le texte affiche des variables, avec leurs valeurs.
pub fn key_with_values(def: &crate::config::KeyDef, vars: &BTreeMap<String, String>) -> crate::config::KeyDef {
    let mut def = def.clone();
    def.label = expand_label(&def.label, vars);
    if let Some(t) = def.toggle.as_mut() {
        t.label = expand_label(&t.label, vars);
    }
    def
}

/// Vrai si le texte cite au moins une variable (`{nom}`).
pub fn mentions(text: &str) -> bool {
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        if let Some(end) = after.find('}') {
            if placeholder(&after[..end]) {
                return true;
            }
        }
        rest = after;
    }
    false
}

fn number(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn format_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn truthy(s: &str) -> bool {
    let t = s.trim().to_lowercase();
    !(t.is_empty() || t == "0" || t == "false" || t == "non" || t == "off")
}

/// Compare deux valeurs : en nombres si les deux en sont (un côté vide
/// compte alors pour 0), sinon en texte sans tenir compte des majuscules.
pub fn compare(left: &str, cmp: &str, right: &str) -> bool {
    if cmp == "contains" {
        return left.to_lowercase().contains(&right.trim().to_lowercase());
    }
    let (ln, rn) = (number(left), number(right));
    let nums = match (ln, rn) {
        (Some(a), Some(b)) => Some((a, b)),
        (None, Some(b)) if left.trim().is_empty() => Some((0.0, b)),
        (Some(a), None) if right.trim().is_empty() => Some((a, 0.0)),
        _ => None,
    };
    let ord = match nums {
        Some((a, b)) => a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal),
        None => left.trim().to_lowercase().cmp(&right.trim().to_lowercase()),
    };
    use std::cmp::Ordering::*;
    match cmp {
        "!=" => ord != Equal,
        ">" => ord == Greater,
        "<" => ord == Less,
        ">=" => ord != Less,
        "<=" => ord != Greater,
        _ => ord == Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_and_toggles() {
        let mut v = Vars::in_memory();
        assert!(v.apply("Compteur", "add", "1"));
        assert!(v.apply("compteur", "add", "2"));
        assert_eq!(v.get("COMPTEUR").unwrap(), "3");
        assert!(v.apply("compteur", "add", "-0.5"));
        assert_eq!(v.get("compteur").unwrap(), "2.5");
        assert!(v.apply("live", "toggle", ""));
        assert_eq!(v.get("live").unwrap(), "1");
        v.apply("live", "toggle", "");
        assert_eq!(v.get("live").unwrap(), "0");
        assert!(!v.apply("live", "set", "0"));
        assert!(!v.apply("pas bon", "set", "1"));
    }

    #[test]
    fn expansion_keeps_unknown_braces() {
        let look = |n: &str| if n == "a" { Some("42".to_string()) } else { None };
        assert_eq!(expand("x={a} y={b} {pas bon} {", &look), "x=42 y={b} {pas bon} {");
        assert_eq!(expand("{A}{a}", &look), "4242");
        assert!(mentions("Morts : {morts}"));
        assert!(!mentions("{ rien }"));
    }

    #[test]
    fn comparisons() {
        assert!(compare("10", ">=", "9"));
        assert!(!compare("10", "<", "9"));
        assert!(compare("", "==", "0"));
        assert!(compare("obs64.exe", "==", "OBS64.EXE"));
        assert!(compare("obs64.exe", "contains", "obs"));
        assert!(compare("2,5", ">", "2"));
        assert!(compare("abc", "!=", "abd"));
    }
}
