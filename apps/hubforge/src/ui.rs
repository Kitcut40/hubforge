//! Commandes appelées par l'interface (JavaScript → Rust).

use crate::audio::Audio;
use crate::autostart;
use crate::config::{self, Config, KeyDef};
use crate::engine::{EngineMsg, Status};
use crate::profiles::{self, SharedActive};
use crate::render;
use base64::Engine as _;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

pub struct AppState {
    /// Profil actif (partagé avec le moteur).
    pub active: SharedActive,
    pub cfg_dir: PathBuf,
    pub engine: Mutex<Sender<EngineMsg>>,
    pub status: Arc<Mutex<Status>>,
    pub audio: Audio,
}

#[derive(Serialize)]
pub struct Initial {
    config: Config,
    config_path: String,
    profile: String,
    profiles: Vec<String>,
    game_best: u32,
    status: Status,
    autostart: bool,
    version: String,
}

#[derive(Serialize)]
pub struct Preview {
    src: String,
    animated: bool,
}

#[tauri::command]
pub fn get_initial(state: State<AppState>) -> Result<Initial, String> {
    let active = state.active.lock().unwrap().clone();
    let config = config::load(&active.path).unwrap_or_else(|e| {
        log::error!("{e}");
        profiles::blank_config()
    });
    Ok(Initial {
        config,
        config_path: active.path.display().to_string(),
        profile: active.name,
        profiles: profiles::list(&state.cfg_dir),
        game_best: profiles::load_settings(&state.cfg_dir).game_best,
        status: state.status.lock().unwrap().clone(),
        autostart: autostart::is_enabled(),
        version: env!("CARGO_PKG_VERSION").into(),
    })
}

/// Enregistre la configuration et l'applique immédiatement.
#[tauri::command]
pub fn apply_config(state: State<AppState>, config: Config) -> Result<(), String> {
    let path = state.active.lock().unwrap().path.clone();
    config::save(&path, &config)?;
    let _ = state.engine.lock().unwrap().send(EngineMsg::ApplyConfig(Box::new(config)));
    Ok(())
}

// ----------------------------------------------------------------------
// Profils
// ----------------------------------------------------------------------

#[tauri::command]
pub fn list_profiles(state: State<AppState>) -> Vec<String> {
    profiles::list(&state.cfg_dir)
}

/// Le moteur charge le profil puis prévient l'interface (événement « status »).
#[tauri::command]
pub fn switch_profile(state: State<AppState>, name: String) -> Result<(), String> {
    if !profiles::list(&state.cfg_dir).contains(&name) {
        return Err(format!("profil « {name} » introuvable"));
    }
    let _ = state.engine.lock().unwrap().send(EngineMsg::SwitchProfile(name));
    Ok(())
}

/// Crée un profil (vierge ou copie de `copy_from`) et passe dessus.
#[tauri::command]
pub fn create_profile(state: State<AppState>, name: String, copy_from: Option<String>) -> Result<String, String> {
    let name = profiles::create(&state.cfg_dir, &name, copy_from.as_deref())?;
    let _ = state.engine.lock().unwrap().send(EngineMsg::SwitchProfile(name.clone()));
    Ok(name)
}

#[tauri::command]
pub fn rename_profile(state: State<AppState>, old: String, new: String) -> Result<String, String> {
    let is_active = state.active.lock().unwrap().name == old;
    let new = profiles::rename(&state.cfg_dir, &old, &new)?;
    if is_active {
        // Le fichier a changé de nom : le moteur suit.
        {
            let mut a = state.active.lock().unwrap();
            a.name = new.clone();
            a.path = profiles::path(&state.cfg_dir, &new);
        }
        let _ = state.engine.lock().unwrap().send(EngineMsg::SwitchProfile(new.clone()));
    }
    Ok(new)
}

#[tauri::command]
pub fn delete_profile(state: State<AppState>, name: String) -> Result<(), String> {
    if state.active.lock().unwrap().name == name {
        return Err("passe d'abord sur un autre profil pour supprimer celui-ci".into());
    }
    profiles::delete(&state.cfg_dir, &name)
}

/// 🎮
#[tauri::command]
pub fn start_game(state: State<AppState>) {
    let _ = state.engine.lock().unwrap().send(EngineMsg::StartGame);
}

/// Aperçus des touches (JPEG en base64) ; `None` = touche vide.
#[tauri::command]
pub async fn render_keys(state: State<'_, AppState>, keys: Vec<Option<KeyDef>>) -> Result<Vec<Option<Preview>>, String> {
    let dir = state.cfg_dir.clone();
    let has_widget = keys.iter().flatten().any(|d| d.widget.is_some());
    let snap = if has_widget {
        state.audio.send(crate::audio::AudioCmd::Poll);
        Some(crate::widgets::preview_snapshot(state.audio.state()))
    } else {
        None
    };
    Ok(keys
        .into_iter()
        .map(|k| {
            k.map(|def| {
                let (jpeg, animated) = match (&def.widget, &snap) {
                    (Some(_), Some(snap)) => (render::encode_jpeg(&crate::widgets::render(&def, snap), 90), false),
                    _ => render::preview(&def, &dir),
                };
                Preview {
                    src: format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(jpeg)),
                    animated,
                }
            })
        })
        .collect())
}

#[tauri::command]
pub fn show_page(state: State<AppState>, index: usize) {
    let _ = state.engine.lock().unwrap().send(EngineMsg::ShowPage(index));
}

#[tauri::command]
pub async fn list_audio_apps(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(state.audio.list_apps())
}

#[tauri::command]
pub fn set_autostart(on: bool) -> Result<bool, String> {
    autostart::set(on)?;
    Ok(autostart::is_enabled())
}

#[tauri::command]
pub fn open_config_folder(state: State<AppState>) {
    crate::actions::launch(&state.cfg_dir.to_string_lossy(), &[]);
}

/// Ouvre une boîte de dialogue « Ouvrir un fichier ».
/// kind = "image", "program" ou "any". Retourne le chemin choisi.
#[tauri::command]
pub async fn pick_file(app: AppHandle, kind: String) -> Result<Option<String>, String> {
    let mut d = app.dialog().file();
    d = match kind.as_str() {
        "image" => d.add_filter("Images et GIF", &["png", "jpg", "jpeg", "gif", "webp", "bmp"]),
        "program" => d.add_filter("Programmes", &["exe", "lnk", "bat", "cmd"]).add_filter("Tous les fichiers", &["*"]),
        _ => d,
    };
    Ok(d.blocking_pick_file().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()))
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    Ok(app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()))
}

/// Copie une image dans le dossier de configuration (icons/) et retourne
/// son chemin relatif, pour que la configuration reste portable.
#[tauri::command]
pub fn import_image(state: State<AppState>, path: String) -> Result<String, String> {
    let src = PathBuf::from(&path);
    let name = src.file_name().ok_or("nom de fichier invalide")?.to_string_lossy().to_string();
    let icons = state.cfg_dir.join("icons");
    std::fs::create_dir_all(&icons).map_err(|e| e.to_string())?;
    let mut dest = icons.join(&name);
    if dest.exists() && std::fs::read(&dest).ok() != std::fs::read(&src).ok() {
        let stem = src.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let ext = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let mut i = 2;
        while dest.exists() {
            dest = icons.join(format!("{stem}-{i}{ext}"));
            i += 1;
        }
    }
    if !dest.exists() {
        std::fs::copy(&src, &dest).map_err(|e| format!("copie impossible : {e}"))?;
    }
    Ok(format!("icons/{}", dest.file_name().unwrap().to_string_lossy()))
}

/// Variante de `import_image` pour une image glissée depuis l'Explorateur
/// (l'interface n'a que le contenu du fichier, pas son chemin).
#[tauri::command]
pub fn import_image_data(state: State<AppState>, name: String, data: String) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).map_err(|e| e.to_string())?;
    let safe: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let tmp = std::env::temp_dir().join(format!("hubforge-import-{safe}"));
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    let res = import_image(state, tmp.display().to_string());
    let _ = std::fs::remove_file(&tmp);
    res
}

#[derive(Serialize)]
pub struct IconInfo {
    name: String,
    svg: String,
}

/// Icônes intégrées (pour le sélecteur de l'interface).
#[tauri::command]
pub fn list_icons() -> Vec<IconInfo> {
    crate::icons_data::ICONS.iter().map(|(n, s)| IconInfo { name: n.to_string(), svg: s.to_string() }).collect()
}

/// Récupère l'icône d'une application / d'un dossier et retourne son chemin relatif.
#[tauri::command]
pub async fn extract_app_icon(state: State<'_, AppState>, target: String) -> Result<String, String> {
    let dir = state.cfg_dir.clone();
    crate::appicon::extract_to(&target, &dir)
}

/// Icône (chemin relatif, si trouvée) et nom lisible d'une application.
#[derive(Serialize)]
pub struct AppInfo {
    icon: Option<String>,
    name: Option<String>,
}

#[tauri::command]
pub async fn app_info(state: State<'_, AppState>, target: String) -> Result<AppInfo, String> {
    let dir = state.cfg_dir.clone();
    let icon = crate::appicon::extract_to(&target, &dir).ok();
    let name = crate::appicon::app_name(&target);
    Ok(AppInfo { icon, name })
}

/// Programmes qui ont une fenêtre ouverte (pour lier une page à une appli).
#[tauri::command]
pub async fn list_running_apps() -> Vec<String> {
    crate::foreground::running_apps()
}
