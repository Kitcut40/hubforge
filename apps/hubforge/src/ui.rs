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
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

pub struct AppState {
    /// Profil actif (partagé avec le moteur).
    pub active: SharedActive,
    pub cfg_dir: PathBuf,
    pub engine: Mutex<Sender<EngineMsg>>,
    pub status: Arc<Mutex<Status>>,
    pub audio: Audio,
    /// État des mises à jour (partagé avec la vérification en arrière-plan).
    pub update: SharedUpdate,
}

/// État des mises à jour, envoyé à l'interface.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateState {
    pub current: String,
    /// Version plus récente disponible (sauf si ignorée).
    pub available: Option<crate::update::UpdateInfo>,
    pub checking: bool,
    /// Dernier message d'erreur (pas de connexion…).
    pub error: Option<String>,
    /// Dernière vérification (secondes depuis 1970).
    pub last_check: u64,
    /// Vérification automatique activée.
    pub auto: bool,
}

pub type SharedUpdate = Arc<Mutex<UpdateState>>;

fn update_snapshot(cfg_dir: &std::path::Path, shared: &SharedUpdate) -> UpdateState {
    let st = profiles::load_settings(cfg_dir);
    let mut u = shared.lock().unwrap().clone();
    u.current = crate::update::CURRENT.into();
    u.auto = !st.no_update_check;
    u.last_check = st.last_update_check;
    if let Some(a) = &u.available {
        if a.version == st.skipped_version {
            u.available = None;
        }
    }
    u
}

/// Vérifie s'il existe une version plus récente et prévient l'interface.
pub fn run_update_check(app: &AppHandle, cfg_dir: &std::path::Path, shared: &SharedUpdate) -> UpdateState {
    shared.lock().unwrap().checking = true;
    let _ = app.emit("update-state", update_snapshot(cfg_dir, shared));
    let res = crate::update::check();
    {
        let mut u = shared.lock().unwrap();
        u.checking = false;
        match res {
            Ok(info) => {
                if let Some(i) = &info {
                    log::info!("mise à jour disponible : {} → {}", crate::update::CURRENT, i.version);
                }
                u.available = info;
                u.error = None;
            }
            Err(e) => {
                log::info!("vérification des mises à jour : {e}");
                u.error = Some(e);
            }
        }
    }
    let mut st = profiles::load_settings(cfg_dir);
    st.last_update_check = crate::update::now_secs();
    profiles::save_settings(cfg_dir, &st);
    let snap = update_snapshot(cfg_dir, shared);
    if let Some(tray) = app.tray_by_id("main") {
        let tip = match &snap.available {
            Some(a) => format!("HubForge — mise à jour {} disponible", a.version),
            None => "HubForge".to_string(),
        };
        let _ = tray.set_tooltip(Some(tip));
    }
    let _ = app.emit("update-state", snap.clone());
    snap
}

/// Notes de la version en cours, tirées du CHANGELOG intégré au programme.
fn current_release_notes() -> String {
    const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");
    let v = crate::update::CURRENT;
    let mut out = Vec::new();
    let mut inside = false;
    for line in CHANGELOG.lines() {
        if line.starts_with("## ") {
            if inside {
                break;
            }
            inside = line.contains(&format!("[{v}]")) || line.contains(&format!(" {v} ")) || line.ends_with(&format!(" {v}"));
            continue;
        }
        if inside {
            out.push(line);
        }
    }
    out.join("\n").trim().to_string()
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
    /// Notes de version à montrer (première ouverture après une mise à jour).
    whats_new: Option<String>,
    update: UpdateState,
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
        whats_new: whats_new(&state.cfg_dir),
        update: update_snapshot(&state.cfg_dir, &state.update),
    })
}

/// Nouveautés à afficher une fois, après une mise à jour.
fn whats_new(cfg_dir: &std::path::Path) -> Option<String> {
    let mut st = profiles::load_settings(cfg_dir);
    let current = crate::update::CURRENT;
    if st.last_version_run == current {
        return None;
    }
    let first_run = st.last_version_run.is_empty();
    st.last_version_run = current.to_string();
    profiles::save_settings(cfg_dir, &st);
    if first_run {
        return None;
    }
    let notes = current_release_notes();
    (!notes.is_empty()).then_some(notes)
}

// ----------------------------------------------------------------------
// Mises à jour
// ----------------------------------------------------------------------

#[tauri::command]
pub fn get_update_state(state: State<AppState>) -> UpdateState {
    update_snapshot(&state.cfg_dir, &state.update)
}

#[tauri::command]
pub async fn check_updates_now(app: AppHandle, state: State<'_, AppState>) -> Result<UpdateState, String> {
    // Une vérification demandée à la main montre aussi une version ignorée.
    let mut st = profiles::load_settings(&state.cfg_dir);
    st.skipped_version.clear();
    profiles::save_settings(&state.cfg_dir, &st);
    Ok(run_update_check(&app, &state.cfg_dir, &state.update))
}

#[tauri::command]
pub fn set_update_auto(state: State<AppState>, on: bool) -> UpdateState {
    let mut st = profiles::load_settings(&state.cfg_dir);
    st.no_update_check = !on;
    profiles::save_settings(&state.cfg_dir, &st);
    update_snapshot(&state.cfg_dir, &state.update)
}

#[tauri::command]
pub fn skip_update(state: State<AppState>, version: String) -> UpdateState {
    let mut st = profiles::load_settings(&state.cfg_dir);
    st.skipped_version = version;
    profiles::save_settings(&state.cfg_dir, &st);
    update_snapshot(&state.cfg_dir, &state.update)
}

#[derive(Clone, Serialize)]
struct Progress {
    done: u64,
    total: u64,
}

/// Télécharge la nouvelle version, vérifie l'installateur, le lance en mode
/// mise à jour puis ferme HubForge (l'installateur le relance ensuite).
#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let info = state.update.lock().unwrap().available.clone().ok_or("aucune mise à jour disponible")?;
    let app2 = app.clone();
    let path = crate::update::download(&info, move |done, total| {
        let _ = app2.emit("update-progress", Progress { done, total });
    })?;
    crate::update::launch_installer(&path)?;
    log::info!("installateur {} lancé : fermeture de HubForge", info.version);
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(600));
        let h = app.clone();
        let _ = app.run_on_main_thread(move || h.exit(0));
    });
    Ok(())
}

/// Ouvre une adresse web (uniquement GitHub et le site de Microsoft pour WebView2).
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://github.com/") || url.starts_with("https://developer.microsoft.com/")) {
        return Err("adresse non autorisée".into());
    }
    crate::actions::launch(&url, &[]);
    Ok(())
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

// ----------------------------------------------------------------------
// Addons
// ----------------------------------------------------------------------

/// Icône prête à afficher dans l'interface : SVG (texte) ou image (data URL).
#[derive(Serialize, Default)]
pub struct IconView {
    svg: Option<String>,
    src: Option<String>,
}

/// Icône « auto » d'un addon : celle du programme installé (jamais
/// redistribuée, extraite sur le PC de l'utilisateur et gardée en cache).
fn auto_icon(cfg_dir: &std::path::Path, addon_id: &str, detect: &[String]) -> IconView {
    let cache = cfg_dir.join("icons").join(format!("addon-{addon_id}.png"));
    if !cache.is_file() {
        let Some(exe) = detect.iter().find_map(|d| crate::appicon::find_installed(d)) else { return IconView::default() };
        match crate::appicon::extract_to(&exe.to_string_lossy(), cfg_dir) {
            Ok(rel) => {
                let _ = std::fs::rename(cfg_dir.join(&rel), &cache);
            }
            Err(e) => {
                log::debug!("icône de l'addon {addon_id} : {e}");
                return IconView::default();
            }
        }
    }
    let src = std::fs::read(&cache).ok().map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)));
    IconView { svg: None, src }
}

fn icon_view(cfg_dir: &std::path::Path, icon: &Option<String>) -> IconView {
    let Some(icon) = icon else { return IconView::default() };
    if let Some(name) = icon.strip_prefix("builtin:") {
        return IconView { svg: render::builtin_svg(name).map(String::from), src: None };
    }
    let Some(path) = crate::addons::resolve_icon(cfg_dir, icon) else { return IconView::default() };
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ext == "svg" {
        return IconView { svg: std::fs::read_to_string(&path).ok(), src: None };
    }
    let mime = match ext.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };
    let src = std::fs::read(&path).ok().map(|b| format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(b)));
    IconView { svg: None, src }
}

#[derive(Serialize)]
pub struct AddonActionView {
    #[serde(flatten)]
    action: crate::addons::AddonAction,
    preview: IconView,
}

#[derive(Serialize)]
pub struct AddonView {
    folder: String,
    enabled: bool,
    official: bool,
    error: Option<String>,
    addon: Option<crate::addons::Addon>,
    icon: IconView,
    actions: Vec<AddonActionView>,
}

#[derive(Serialize)]
pub struct AddonsInfo {
    addons: Vec<AddonView>,
    signature: String,
    folder: String,
}

#[tauri::command]
pub async fn list_addons(state: State<'_, AppState>) -> Result<AddonsInfo, String> {
    let dir = state.cfg_dir.clone();
    let disabled = profiles::load_settings(&dir).disabled_addons;
    let official = crate::official::ids();
    let addons = crate::addons::scan(&dir)
        .into_iter()
        .map(|e| {
            let (icon, actions) = match &e.addon {
                Some(a) => (
                    if a.icon.as_deref() == Some("auto") { auto_icon(&dir, &a.id, &a.detect) } else { icon_view(&dir, &a.icon) },
                    a.actions
                        .iter()
                        .map(|x| AddonActionView { preview: icon_view(&dir, &x.icon), action: x.clone() })
                        .collect(),
                ),
                None => (IconView::default(), Vec::new()),
            };
            AddonView {
                enabled: !disabled.contains(&e.folder),
                official: official.contains(&e.folder.as_str()),
                folder: e.folder,
                error: e.error,
                addon: e.addon,
                icon,
                actions,
            }
        })
        .collect();
    Ok(AddonsInfo { addons, signature: crate::addons::signature(&dir), folder: crate::addons::dir(&dir).display().to_string() })
}

/// Empreinte du dossier des addons (l'interface recharge la liste si elle change).
#[tauri::command]
pub fn addons_signature(state: State<AppState>) -> String {
    crate::addons::signature(&state.cfg_dir)
}

fn after_install(state: &State<AppState>, id: &str) {
    crate::official::unmark_removed(&state.cfg_dir, id);
    log::info!("addon « {id} » installé");
}

/// Installe un fichier .forge choisi sur le disque.
#[tauri::command]
pub fn install_addon(state: State<AppState>, path: String) -> Result<String, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("lecture impossible : {e}"))?;
    let id = crate::addons::install_bytes(&state.cfg_dir, &bytes)?;
    after_install(&state, &id);
    Ok(id)
}

/// Installe un fichier .forge glissé sur la fenêtre (contenu en base64).
#[tauri::command]
pub fn install_addon_data(state: State<AppState>, data: String) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).map_err(|e| e.to_string())?;
    let id = crate::addons::install_bytes(&state.cfg_dir, &bytes)?;
    after_install(&state, &id);
    Ok(id)
}

#[tauri::command]
pub fn remove_addon(state: State<AppState>, id: String) -> Result<(), String> {
    crate::addons::remove(&state.cfg_dir, &id)?;
    crate::official::mark_removed(&state.cfg_dir, &id);
    log::info!("addon « {id} » supprimé");
    Ok(())
}

/// Réinstalle les addons officiels supprimés.
#[tauri::command]
pub fn restore_official_addons(state: State<AppState>) {
    for id in crate::official::ids() {
        crate::official::unmark_removed(&state.cfg_dir, id);
    }
    crate::official::install(&state.cfg_dir);
}

#[tauri::command]
pub fn set_addon_enabled(state: State<AppState>, id: String, on: bool) {
    let mut st = profiles::load_settings(&state.cfg_dir);
    st.disabled_addons.retain(|x| x != &id);
    if !on {
        st.disabled_addons.push(id);
    }
    profiles::save_settings(&state.cfg_dir, &st);
}

/// Pages prêtes à l'emploi d'un addon, numérotées pour être ajoutées
/// après les `base` pages existantes.
#[tauri::command]
pub fn addon_pages(state: State<AppState>, id: String, base: usize) -> Result<Vec<config::Page>, String> {
    let entry = crate::addons::scan(&state.cfg_dir).into_iter().find(|e| e.folder == id).ok_or("addon introuvable")?;
    let addon = entry.addon.ok_or_else(|| entry.error.unwrap_or_default())?;
    Ok(addon.build_pages(base))
}

#[tauri::command]
pub fn open_addons_folder(state: State<AppState>) {
    let d = crate::addons::dir(&state.cfg_dir);
    let _ = std::fs::create_dir_all(&d);
    crate::actions::launch(&d.to_string_lossy(), &[]);
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
        "addon" => d.add_filter("Addon HubForge", &["forge"]),
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
