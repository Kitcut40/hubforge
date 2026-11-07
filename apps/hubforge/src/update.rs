//! Mises à jour : HubForge regarde sur GitHub (Releases du dépôt) si une
//! version plus récente existe, et peut la télécharger puis lancer son
//! installateur en mode « mise à jour » (silencieux, HubForge redémarre seul).
//!
//! - Rien n'est installé sans l'accord de l'utilisateur.
//! - La vérification peut être désactivée (réglages) : HubForge reste
//!   entièrement utilisable hors ligne.
//! - L'installateur téléchargé est vérifié (empreinte SHA-256 publiée avec la
//!   version) avant d'être lancé.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

/// Dépôt GitHub où sont publiées les versions.
pub const REPO: &str = "Kitcut40/hubforge";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Une version disponible au téléchargement.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    /// Ex. « 0.8.1 ».
    pub version: String,
    pub current: String,
    pub name: String,
    /// Notes de version (Markdown).
    pub notes: String,
    pub published: String,
    /// Page de la version sur GitHub.
    pub page_url: String,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
    /// Empreinte attendue de l'installateur (hexadécimal), si connue d'avance.
    pub sha256: Option<String>,
    /// Fichier « .sha256 » publié à côté de l'installateur.
    pub sha256_url: Option<String>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

/// « v0.8.1 » → [0, 8, 1] (les suffixes « -beta » sont ignorés).
pub fn version_key(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches(['v', 'V'])
        .split(['-', '+'])
        .next()
        .unwrap_or("")
        .split('.')
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// `candidate` est-elle plus récente que `current` ?
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (mut a, mut b) = (version_key(candidate), version_key(current));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(&format!("HubForge/{CURRENT} (+https://github.com/{REPO})"))
        .try_proxy_from_env(true)
        .build()
}

/// L'installateur Windows d'une version : « HubForge-Setup-0.8.1.exe ».
fn is_installer(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with("hubforge-setup") && n.ends_with(".exe")
}

fn release_to_info(r: GhRelease) -> Option<UpdateInfo> {
    if r.draft || r.prerelease {
        return None;
    }
    let version = r.tag_name.trim_start_matches(['v', 'V']).to_string();
    let asset = r.assets.iter().find(|a| is_installer(&a.name))?;
    let sha256 = asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
        .map(|s| s.to_lowercase());
    let sha256_url = r
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&format!("{}.sha256", asset.name)))
        .map(|a| a.browser_download_url.clone());
    Some(UpdateInfo {
        name: r.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("HubForge {version}")),
        version,
        current: CURRENT.to_string(),
        notes: r.body.unwrap_or_default(),
        published: r.published_at.unwrap_or_default(),
        page_url: r.html_url,
        asset_name: asset.name.clone(),
        asset_url: asset.browser_download_url.clone(),
        asset_size: asset.size,
        sha256,
        sha256_url,
    })
}

/// Dernière version publiée ; `None` si HubForge est à jour.
pub fn check() -> Result<Option<UpdateInfo>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let resp = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => "aucune version publiée pour l'instant".to_string(),
            ureq::Error::Status(403, _) => "GitHub limite les vérifications : réessaie dans une heure".to_string(),
            ureq::Error::Status(code, _) => format!("GitHub a répondu {code}"),
            ureq::Error::Transport(t) => format!("pas de connexion à GitHub ({})", t.kind()),
        })?;
    let release: GhRelease = resp.into_json().map_err(|e| format!("réponse de GitHub illisible : {e}"))?;
    let Some(info) = release_to_info(release) else { return Ok(None) };
    Ok(is_newer(&info.version, CURRENT).then_some(info))
}

/// Télécharge l'installateur dans le dossier temporaire et vérifie son
/// empreinte. `progress(reçu, total)` est appelé pendant le téléchargement.
pub fn download(info: &UpdateInfo, progress: impl Fn(u64, u64)) -> Result<PathBuf, String> {
    if !info.asset_url.starts_with("https://github.com/") {
        return Err("adresse de téléchargement inattendue".into());
    }
    let ag = agent();
    // Empreinte attendue : fournie par GitHub, sinon fichier .sha256.
    let expected = match (&info.sha256, &info.sha256_url) {
        (Some(h), _) => h.clone(),
        (None, Some(u)) => {
            let text = ag.get(u).call().map_err(|e| format!("empreinte introuvable : {e}"))?.into_string().map_err(|e| e.to_string())?;
            text.split_whitespace().next().unwrap_or("").to_lowercase()
        }
        (None, None) => return Err("cette version ne publie pas d'empreinte SHA-256 : mise à jour refusée par sécurité".into()),
    };
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("empreinte SHA-256 invalide".into());
    }

    let resp = ag.get(&info.asset_url).call().map_err(|e| format!("téléchargement impossible : {e}"))?;
    let total = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(info.asset_size);
    let mut reader = resp.into_reader().take(200 * 1024 * 1024);
    let dir = std::env::temp_dir().join("HubForge-mise-a-jour");
    let _ = std::fs::create_dir_all(&dir);
    let safe_name: String = info.asset_name.chars().filter(|c| c.is_ascii_alphanumeric() || "._-".contains(*c)).collect();
    let path = dir.join(safe_name);
    let mut file = std::fs::File::create(&path).map_err(|e| format!("écriture impossible : {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    let mut last = std::time::Instant::now();
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("téléchargement interrompu : {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| format!("écriture impossible : {e}"))?;
        done += n as u64;
        if last.elapsed() > Duration::from_millis(120) {
            progress(done, total);
            last = std::time::Instant::now();
        }
    }
    drop(file);
    progress(done, total);
    let got = format!("{:x}", hasher.finalize());
    if got != expected {
        let _ = std::fs::remove_file(&path);
        return Err("le fichier téléchargé est corrompu (empreinte différente) : réessaie plus tard".into());
    }
    log::info!("mise à jour {} téléchargée et vérifiée ({} octets)", info.version, done);
    Ok(path)
}

/// Lance l'installateur en mode mise à jour (silencieux ; il ferme
/// HubForge, installe la nouvelle version et la relance).
pub fn launch_installer(path: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let file = wide(&path.to_string_lossy());
        let args = wide("/S /UPDATE");
        let verb = wide("open");
        let r = unsafe { ShellExecuteW(None, PCWSTR(verb.as_ptr()), PCWSTR(file.as_ptr()), PCWSTR(args.as_ptr()), PCWSTR::null(), SW_SHOWNORMAL) };
        if (r.0 as isize) <= 32 {
            return Err(format!("lancement de l'installateur impossible (code {})", r.0 as isize));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("mise à jour automatique disponible uniquement sous Windows pour l'instant".into())
    }
}

/// Secondes depuis 1970 (pour espacer les vérifications).
pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.8.1", "0.8.0"));
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("1.0", "0.99.99"));
        assert!(!is_newer("0.8.0", "0.8.0"));
        assert!(!is_newer("0.7.9", "0.8.0"));
        assert!(!is_newer("0.8.0-beta", "0.8.0"));
    }

    #[test]
    fn reads_github_release() {
        let json = r#"{
            "tag_name": "v9.9.9", "name": "HubForge 9.9.9", "body": "- nouveautés",
            "html_url": "https://github.com/Kitcut40/hubforge/releases/tag/v9.9.9",
            "published_at": "2026-11-07T17:00:00Z", "draft": false, "prerelease": false,
            "assets": [
              { "name": "HubForge-Setup-9.9.9.exe", "browser_download_url": "https://github.com/Kitcut40/hubforge/releases/download/v9.9.9/HubForge-Setup-9.9.9.exe", "size": 123, "digest": "sha256:ABCDEF0000000000000000000000000000000000000000000000000000000000" },
              { "name": "HubForge-Setup-9.9.9.exe.sha256", "browser_download_url": "https://github.com/x.sha256", "size": 64 }
            ]
        }"#;
        let r: GhRelease = serde_json::from_str(json).unwrap();
        let i = release_to_info(r).unwrap();
        assert_eq!(i.version, "9.9.9");
        assert_eq!(i.asset_name, "HubForge-Setup-9.9.9.exe");
        assert!(i.sha256.unwrap().starts_with("abcdef"));
        assert!(i.sha256_url.is_some());
        // Version sans installateur Windows : ignorée.
        let r: GhRelease = serde_json::from_str(r#"{ "tag_name": "v1.0.0", "html_url": "x", "assets": [] }"#).unwrap();
        assert!(release_to_info(r).is_none());
    }
}
