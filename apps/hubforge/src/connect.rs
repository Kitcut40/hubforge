//! Connecteurs intégrés : requêtes web et IA sur la sélection.
//!
//! Ces fonctions bloquent (réseau) : le moteur les appelle dans un fil à part
//! et reçoit le résultat par un canal.

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use std::time::Duration;

const MAX_RESPONSE: u64 = 1024 * 1024;
/// Longueur maximale gardée dans une variable.
pub const MAX_VAR_LEN: usize = 500;

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(timeout).user_agent(concat!("HubForge/", env!("CARGO_PKG_VERSION"))).build()
}

fn read_body(resp: ureq::Response) -> Result<String, String> {
    let mut buf = Vec::new();
    resp.into_reader().take(MAX_RESPONSE).read_to_end(&mut buf).map_err(|e| format!("réponse illisible : {e}"))?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn net_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = read_body(resp).unwrap_or_default();
            let detail: String = body.trim().chars().take(160).collect();
            if detail.is_empty() {
                format!("le serveur a répondu {code}")
            } else {
                format!("le serveur a répondu {code} : {detail}")
            }
        }
        ureq::Error::Transport(t) => format!("connexion impossible : {t}"),
    }
}

// ----------------------------------------------------------------------
// Requête web
// ----------------------------------------------------------------------

/// Envoie la requête et renvoie le corps de la réponse.
pub fn http(method: &str, url: &str, body: &str, headers: &str) -> Result<String, String> {
    let url = url.trim();
    if !url.to_lowercase().starts_with("http://") && !url.to_lowercase().starts_with("https://") {
        return Err(format!("adresse invalide : « {url} »"));
    }
    let mut req = agent(Duration::from_secs(10)).request(&method.to_uppercase(), url);
    let mut has_type = false;
    for line in headers.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            if k.is_empty() {
                continue;
            }
            has_type |= k.eq_ignore_ascii_case("content-type");
            req = req.set(k, v.trim());
        }
    }
    let resp = if body.is_empty() || method.eq_ignore_ascii_case("GET") {
        req.call()
    } else {
        if !has_type {
            let t = body.trim_start();
            let ct = if t.starts_with('{') || t.starts_with('[') { "application/json" } else { "text/plain; charset=utf-8" };
            req = req.set("Content-Type", ct);
        }
        req.send_string(body)
    };
    read_body(resp.map_err(net_error)?)
}

/// Valeur à afficher : un champ d'une réponse JSON (« a.b.0.c »), sinon le texte.
pub fn pick(body: &str, path: &str) -> Result<String, String> {
    let path = path.trim();
    let text = if path.is_empty() {
        // Réponse JSON réduite à une valeur simple : on la déballe.
        match serde_json::from_str::<serde_json::Value>(body) {
            Ok(serde_json::Value::String(s)) => s,
            Ok(v @ (serde_json::Value::Number(_) | serde_json::Value::Bool(_))) => v.to_string(),
            _ => body.trim().to_string(),
        }
    } else {
        let v: serde_json::Value = serde_json::from_str(body).map_err(|_| "la réponse n'est pas du JSON".to_string())?;
        let pointer = if path.starts_with('/') { path.to_string() } else { format!("/{}", path.replace('.', "/")) };
        match v.pointer(&pointer) {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Null) | None => return Err(format!("champ « {path} » absent de la réponse")),
            Some(x) => x.to_string(),
        }
    };
    Ok(clip(&text))
}

/// Coupe un texte trop long pour une variable.
pub fn clip(text: &str) -> String {
    let t = text.trim();
    if t.chars().count() > MAX_VAR_LEN {
        let mut s: String = t.chars().take(MAX_VAR_LEN).collect();
        s.push('…');
        s
    } else {
        t.to_string()
    }
}

// ----------------------------------------------------------------------
// IA
// ----------------------------------------------------------------------

/// Réglages de l'IA, communs à tous les profils (fichier `ai.json`, jamais
/// exporté avec un profil puisqu'il peut contenir une clé).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiSettings {
    /// "ollama", "openai" (et services compatibles) ou "anthropic".
    pub provider: String,
    /// Adresse du serveur (Ollama, service compatible OpenAI).
    pub url: String,
    pub model: String,
    pub key: String,
}

impl AiSettings {
    pub fn is_configured(&self) -> bool {
        !self.provider.is_empty() && !self.model.trim().is_empty() && (self.provider == "ollama" || !self.key.trim().is_empty())
    }
}

fn settings_path(cfg_dir: &Path) -> std::path::PathBuf {
    cfg_dir.join("ai.json")
}

pub fn load_settings(cfg_dir: &Path) -> AiSettings {
    std::fs::read_to_string(settings_path(cfg_dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_settings(cfg_dir: &Path, s: &AiSettings) -> Result<(), String> {
    if !["", "ollama", "openai", "anthropic"].contains(&s.provider.as_str()) {
        return Err(format!("fournisseur d'IA inconnu « {} »", s.provider));
    }
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(settings_path(cfg_dir), text).map_err(|e| e.to_string())
}

/// Consigne envoyée à l'IA pour chaque préréglage.
pub fn instruction(preset: &str, custom: &str) -> String {
    let base = match preset {
        "translate_en" => "Traduis le texte en anglais.",
        "translate_fr" => "Traduis le texte en français.",
        "summarize" => "Résume le texte en quelques phrases, dans sa langue d'origine.",
        "formal" => "Réécris le texte dans un style plus professionnel et poli, dans sa langue d'origine, sans rien ajouter.",
        "custom" if !custom.trim().is_empty() => custom.trim(),
        _ => "Corrige l'orthographe, la grammaire et la ponctuation du texte, sans changer son sens, son ton ni sa langue.",
    };
    format!("{base}\nRéponds uniquement avec le texte obtenu, sans introduction, sans commentaire et sans guillemets autour.")
}

fn base_url(s: &AiSettings, default: &str) -> String {
    let u = s.url.trim().trim_end_matches('/');
    if u.is_empty() {
        default.to_string()
    } else {
        u.to_string()
    }
}

/// Envoie le texte à l'IA avec la consigne ; renvoie sa réponse.
pub fn ask(s: &AiSettings, instruction: &str, text: &str) -> Result<String, String> {
    if !s.is_configured() {
        return Err("l'IA n'est pas réglée (Réglages › Intelligence artificielle)".into());
    }
    let ag = agent(Duration::from_secs(90));
    let answer = match s.provider.as_str() {
        "ollama" => {
            let url = format!("{}/api/chat", base_url(s, "http://localhost:11434"));
            let body = serde_json::json!({
                "model": s.model.trim(),
                "stream": false,
                "messages": [{ "role": "system", "content": instruction }, { "role": "user", "content": text }],
            });
            let v: serde_json::Value = ag.post(&url).send_json(body).map_err(net_error)?.into_json().map_err(|e| e.to_string())?;
            v.pointer("/message/content").and_then(|c| c.as_str()).map(String::from)
        }
        "anthropic" => {
            let url = format!("{}/v1/messages", base_url(s, "https://api.anthropic.com"));
            let body = serde_json::json!({
                "model": s.model.trim(),
                "max_tokens": 4096,
                "system": instruction,
                "messages": [{ "role": "user", "content": text }],
            });
            let v: serde_json::Value = ag
                .post(&url)
                .set("x-api-key", s.key.trim())
                .set("anthropic-version", "2023-06-01")
                .send_json(body)
                .map_err(net_error)?
                .into_json()
                .map_err(|e| e.to_string())?;
            v.pointer("/content/0/text").and_then(|c| c.as_str()).map(String::from)
        }
        _ => {
            let url = format!("{}/chat/completions", base_url(s, "https://api.openai.com/v1"));
            let body = serde_json::json!({
                "model": s.model.trim(),
                "messages": [{ "role": "system", "content": instruction }, { "role": "user", "content": text }],
            });
            let mut req = ag.post(&url);
            if !s.key.trim().is_empty() {
                req = req.set("Authorization", &format!("Bearer {}", s.key.trim()));
            }
            let v: serde_json::Value = req.send_json(body).map_err(net_error)?.into_json().map_err(|e| e.to_string())?;
            v.pointer("/choices/0/message/content").and_then(|c| c.as_str()).map(String::from)
        }
    };
    let a = answer.ok_or("réponse de l'IA inattendue")?;
    let a = a.trim();
    // Certains modèles entourent quand même leur réponse de guillemets.
    let a = a.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(a);
    if a.is_empty() {
        return Err("l'IA a renvoyé une réponse vide".into());
    }
    Ok(a.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_fields() {
        let b = r#"{"data":{"temp":21.5,"name":"Salon","list":[{"v":"a"},{"v":"b"}]}}"#;
        assert_eq!(pick(b, "data.temp").unwrap(), "21.5");
        assert_eq!(pick(b, "data.name").unwrap(), "Salon");
        assert_eq!(pick(b, "data.list.1.v").unwrap(), "b");
        assert_eq!(pick(b, "/data/name").unwrap(), "Salon");
        assert!(pick(b, "data.absent").is_err());
        assert_eq!(pick("  ok \n", "").unwrap(), "ok");
        assert_eq!(pick("\"bonjour\"", "").unwrap(), "bonjour");
        assert!(pick("pas du json", "a").is_err());
        assert_eq!(clip(&"x".repeat(600)).chars().count(), MAX_VAR_LEN + 1);
    }

    #[test]
    fn settings_round_trip_and_checks() {
        let d = std::env::temp_dir().join(format!("hubforge-ai-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert_eq!(load_settings(&d), AiSettings::default());
        let s = AiSettings { provider: "ollama".into(), url: String::new(), model: "llama3.2".into(), key: String::new() };
        assert!(s.is_configured());
        save_settings(&d, &s).unwrap();
        assert_eq!(load_settings(&d), s);
        assert!(!AiSettings { provider: "openai".into(), model: "x".into(), ..Default::default() }.is_configured());
        assert!(save_settings(&d, &AiSettings { provider: "pirate".into(), ..Default::default() }).is_err());
        assert!(ask(&AiSettings::default(), "x", "y").is_err());
        assert!(instruction("custom", "").contains("Corrige"));
        assert!(instruction("custom", "Mets en majuscules").starts_with("Mets en majuscules"));
    }

    #[test]
    fn http_rejects_bad_urls() {
        assert!(http("GET", "ftp://x", "", "").is_err());
        assert!(http("GET", "localhost", "", "").is_err());
    }
}
