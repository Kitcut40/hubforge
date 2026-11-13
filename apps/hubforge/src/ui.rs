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
    let (name, path) = {
        let a = state.active.lock().unwrap();
        (a.name.clone(), a.path.clone())
    };
    // Copie de sécurité de la version précédente (au plus une toutes les 2 min).
    crate::backups::snapshot(&state.cfg_dir, &name, &path, false);
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
    crate::backups::rename(&state.cfg_dir, &old, &new);
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
        let mut snap = crate::widgets::preview_snapshot(state.audio.state());
        let targets: Vec<String> = keys
            .iter()
            .flatten()
            .filter(|d| d.widget.as_deref() == Some("app_volume"))
            .filter_map(crate::widgets::app_target)
            .collect();
        if !targets.is_empty() {
            state.audio.send(crate::audio::AudioCmd::Measure(targets.clone()));
            snap.apps = targets.into_iter().filter_map(|t| state.audio.meter_opt(&t).map(|m| (t, m))).collect();
        }
        Some(snap)
    } else {
        None
    };
    let vars = crate::vars::Vars::load(&dir);
    Ok(keys
        .into_iter()
        .map(|k| {
            k.map(|def| {
                let def = crate::vars::key_with_values(&def, vars.all());
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
        "profile" => d.add_filter("Profils et packs d'icônes", &["hfprofile", "streamDeckProfile", "streamDeckIconPack"]),
        "wad" => d.add_filter("Fichier de jeu Doom (WAD)", &["wad", "WAD"]),
        "sound" => d.add_filter("Son", crate::sound::EXTENSIONS),
        "program" => d.add_filter("Programmes", &["exe", "lnk", "bat", "cmd"]).add_filter("Tous les fichiers", &["*"]),
        _ => d,
    };
    Ok(d.blocking_pick_file().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()))
}

// ----------------------------------------------------------------------
// Sauvegardes, partage de profils, diagnostic
// ----------------------------------------------------------------------

#[tauri::command]
pub fn list_backups(state: State<AppState>) -> Vec<crate::backups::BackupInfo> {
    let name = state.active.lock().unwrap().name.clone();
    crate::backups::list(&state.cfg_dir, &name)
}

/// Remet une sauvegarde du profil actif ; l'interface recharge ensuite le profil.
#[tauri::command]
pub fn restore_backup(state: State<AppState>, file: String) -> Result<(), String> {
    let (name, path) = {
        let a = state.active.lock().unwrap();
        (a.name.clone(), a.path.clone())
    };
    crate::backups::restore(&state.cfg_dir, &name, &file, &path)?;
    let cfg = config::load(&path)?;
    let _ = state.engine.lock().unwrap().send(EngineMsg::ApplyConfig(Box::new(cfg)));
    log::info!("profil « {name} » restauré depuis la sauvegarde {file}");
    Ok(())
}

/// Exporte un profil : choix de l'emplacement, puis écriture du .hfprofile.
#[tauri::command]
pub async fn export_profile(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<Option<String>, String> {
    let file_name = format!("{}.hfprofile", name.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "_"));
    let Some(dest) = app
        .dialog()
        .file()
        .add_filter("Profil HubForge", &["hfprofile"])
        .set_file_name(&file_name)
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    let dest = if dest.extension().map(|e| e == "hfprofile").unwrap_or(false) { dest } else { dest.with_extension("hfprofile") };
    let images = crate::share::export(&state.cfg_dir, &name, &dest)?;
    log::info!("profil « {name} » exporté vers {} ({images} images)", dest.display());
    Ok(Some(dest.display().to_string()))
}

fn finish_import(state: &State<AppState>, bytes: &[u8]) -> Result<crate::share::ImportReport, String> {
    let r = crate::sdimport::import_any(&state.cfg_dir, bytes)?;
    log::info!("« {} » importé ({}, {} images, {} sons)", r.name, r.kind, r.images, r.sounds);
    if r.kind == "profile" {
        let _ = state.engine.lock().unwrap().send(EngineMsg::SwitchProfile(r.name.clone()));
    }
    Ok(r)
}

#[tauri::command]
pub fn import_profile(state: State<AppState>, path: String) -> Result<crate::share::ImportReport, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("lecture impossible : {e}"))?;
    finish_import(&state, &bytes)
}

#[tauri::command]
pub fn import_profile_data(state: State<AppState>, data: String) -> Result<crate::share::ImportReport, String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.as_bytes()).map_err(|e| e.to_string())?;
    finish_import(&state, &bytes)
}

#[tauri::command]
pub fn list_icon_packs(state: State<AppState>) -> Vec<crate::sdimport::IconPack> {
    crate::sdimport::list_icon_packs(&state.cfg_dir)
}

/// Vignettes d'un pack d'icônes : (chemin, nom, image en data URL).
#[tauri::command]
pub fn icon_pack_thumbs(state: State<AppState>, folder: String) -> Vec<(String, String, String)> {
    use base64::Engine as _;
    let Some(pack) = crate::sdimport::list_icon_packs(&state.cfg_dir).into_iter().find(|p| p.folder == folder) else { return Vec::new() };
    pack.icons
        .into_iter()
        .filter_map(|i| {
            let data = std::fs::read(state.cfg_dir.join(&i.path)).ok()?;
            if data.len() > 2 * 1024 * 1024 {
                return None;
            }
            let ext = i.path.rsplit('.').next().unwrap_or("png").to_lowercase();
            let mime = match ext.as_str() {
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "svg" => "image/svg+xml",
                _ => "image/png",
            };
            Some((i.path, i.name, format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(data))))
        })
        .collect()
}

#[tauri::command]
pub fn remove_icon_pack(state: State<AppState>, folder: String) -> Result<(), String> {
    crate::sdimport::remove_icon_pack(&state.cfg_dir, &folder)
}

#[derive(Serialize)]
pub struct Diagnostic {
    version: String,
    os: String,
    profile: String,
    config_dir: String,
    status: Status,
    addons: Vec<String>,
    log: String,
}

/// Informations utiles pour une demande d'aide (rien n'est envoyé : l'utilisateur copie le rapport s'il le souhaite).
#[tauri::command]
pub fn get_diagnostic(state: State<AppState>) -> Diagnostic {
    let log_text = std::fs::read_to_string(state.cfg_dir.join("hubforge.log")).unwrap_or_default();
    let lines: Vec<&str> = log_text.lines().collect();
    let tail = lines[lines.len().saturating_sub(150)..].join("\n");
    Diagnostic {
        version: crate::update::CURRENT.into(),
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        profile: state.active.lock().unwrap().name.clone(),
        config_dir: state.cfg_dir.display().to_string(),
        status: state.status.lock().unwrap().clone(),
        addons: crate::addons::scan(&state.cfg_dir)
            .into_iter()
            .map(|e| match (e.addon, e.error) {
                (Some(a), _) => format!("{} {}", a.id, a.version),
                (None, err) => format!("{} (erreur : {})", e.folder, err.unwrap_or_default()),
            })
            .collect(),
        log: tail,
    }
}

/// Ferme et rouvre tous les modules (ils réapparaissent en 1 à 2 secondes).
#[tauri::command]
pub fn reconnect_modules(state: State<AppState>) {
    let _ = state.engine.lock().unwrap().send(EngineMsg::Reconnect);
}

/// Demande à Windows de réexaminer la base, puis reconnecte les modules.
#[tauri::command]
pub async fn reset_base(state: State<'_, AppState>) -> Result<String, String> {
    let r = crate::topology::reenumerate_base();
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let _ = state.engine.lock().unwrap().send(EngineMsg::Reconnect);
    match r {
        Ok(n) => {
            log::info!("base réexaminée ({n} hub)");
            Ok(format!("Base réexaminée ({n} hub). Les modules réapparaissent dans quelques secondes."))
        }
        Err(e) => {
            log::warn!("réexamen de la base : {e}");
            Err(e)
        }
    }
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

// ---------------------------------------------------------------------------
// Lumières et disposition
// ---------------------------------------------------------------------------

/// Modèles d'animation : (identifiant, nom, nombre de couleurs).
#[tauri::command]
pub fn light_presets() -> Vec<(String, String, usize)> {
    crate::lights::PRESETS.iter().map(|(id, name, n)| (id.to_string(), name.to_string(), *n)).collect()
}

/// Joue une animation sur les LED pour l'essayer (None = arrêter l'essai).
#[tauri::command]
pub fn preview_animation(state: State<AppState>, animation: Option<crate::lights::Animation>) {
    let _ = state.engine.lock().unwrap().send(EngineMsg::PreviewAnimation(animation));
}

#[derive(serde::Serialize)]
pub struct LayoutSettings {
    pub base_rot: u16,
    pub placements: std::collections::HashMap<String, crate::topology::Placement>,
}

#[tauri::command]
pub fn get_layout(state: State<AppState>) -> LayoutSettings {
    let st = crate::profiles::load_settings(&state.cfg_dir);
    LayoutSettings { base_rot: st.base_rot, placements: st.placements }
}

/// Orientation de la base sur le bureau : 0, 90, 180 ou 270 degrés.
#[tauri::command]
pub fn set_base_rot(state: State<AppState>, rot: u16) {
    let mut st = crate::profiles::load_settings(&state.cfg_dir);
    st.base_rot = rot % 360 / 90 * 90;
    crate::profiles::save_settings(&state.cfg_dir, &st);
    let _ = state.engine.lock().unwrap().send(EngineMsg::ReloadLayout);
}

/// Position saisie à la main pour un branchement non reconnu (None = l'oublier).
#[tauri::command]
pub fn set_placement(state: State<AppState>, kind: String, code: String, placement: Option<crate::topology::Placement>) {
    let mut st = crate::profiles::load_settings(&state.cfg_dir);
    let key = format!("{kind}:{code}");
    match placement {
        Some(p) => {
            log::info!("position saisie à la main : {key} → col {} lig {} {}°", p.col, p.row, p.rot);
            st.placements.insert(key, p);
        }
        None => {
            st.placements.remove(&key);
        }
    }
    crate::profiles::save_settings(&state.cfg_dir, &st);
    let _ = state.engine.lock().unwrap().send(EngineMsg::ReloadLayout);
}

// ---------------------------------------------------------------------------
// Easter egg : Doom
// ---------------------------------------------------------------------------

/// Lance Doom sur le MasterHUB. `wad` : fichier choisi par l'utilisateur
/// (sinon celui retenu, Freedoom ou un Doom installé). Erreur « no_wad » si
/// aucun fichier de jeu n'est trouvé.
#[tauri::command]
pub fn start_doom(state: State<AppState>, wad: Option<String>) -> Result<String, String> {
    let mut st = crate::profiles::load_settings(&state.cfg_dir);
    if let Some(w) = wad {
        if !crate::doom::is_iwad(std::path::Path::new(&w)) {
            return Err("Ce fichier n'est pas un WAD de jeu complet (IWAD) : choisis DOOM.WAD, DOOM2.WAD ou freedoom1.wad.".into());
        }
        st.doom_wad = w;
        crate::profiles::save_settings(&state.cfg_dir, &st);
    }
    let path = crate::doom::find_wad(&state.cfg_dir, &st.doom_wad).ok_or_else(|| "no_wad".to_string())?;
    let _ = state.engine.lock().unwrap().send(EngineMsg::StartDoom(path.clone()));
    Ok(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
}

/// Télécharge Freedoom (jeu libre compatible Doom, ~24 Mo) puis lance Doom.
#[tauri::command]
pub async fn download_freedoom(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let app2 = app.clone();
    let path = crate::doom::download_freedoom(&state.cfg_dir, move |done, total| {
        let _ = app2.emit("doom-download", Progress { done, total });
    })?;
    let _ = state.engine.lock().unwrap().send(EngineMsg::StartDoom(path));
    Ok(())
}

#[tauri::command]
pub fn stop_doom(state: State<AppState>) {
    let _ = state.engine.lock().unwrap().send(EngineMsg::StopDoom);
}

/// Touche du clavier du PC pendant Doom (code de touche de Doom).
#[tauri::command]
pub fn doom_key(state: State<AppState>, pressed: bool, code: u8) {
    let _ = state.engine.lock().unwrap().send(EngineMsg::DoomKey { pressed, code });
}

// ---------------------------------------------------------------------------
// Soundboard
// ---------------------------------------------------------------------------

/// Sorties audio disponibles (pour la soundboard).
#[tauri::command]
pub fn list_sound_devices() -> Vec<String> {
    use rodio::cpal::traits::HostTrait;
    use rodio::DeviceTrait;
    rodio::cpal::default_host()
        .output_devices()
        .map(|ds| ds.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

/// Copie un son dans le dossier de HubForge (`sounds/`) et renvoie son chemin
/// relatif : le profil reste valable même si le fichier d'origine est déplacé.
fn store_sound(cfg_dir: &std::path::Path, name: &str, bytes: &[u8]) -> Result<String, String> {
    let name = std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let ext = std::path::Path::new(&name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !crate::sound::EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!("format de son non pris en charge ({})", crate::sound::EXTENSIONS.join(", ")));
    }
    if bytes.len() > 50 * 1024 * 1024 {
        return Err("son trop gros (50 Mo maximum)".into());
    }
    let dir = cfg_dir.join("sounds");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem: String = std::path::Path::new(&name)
        .file_stem()
        .map(|s| s.to_string_lossy().chars().map(|c| if c.is_alphanumeric() || "-_ ".contains(c) { c } else { '_' }).collect())
        .unwrap_or_else(|| "son".into());
    let mut file = format!("{stem}.{ext}");
    let mut n = 2;
    while dir.join(&file).exists() {
        if std::fs::read(dir.join(&file)).map(|b| b == bytes).unwrap_or(false) {
            return Ok(format!("sounds/{file}"));
        }
        file = format!("{stem} ({n}).{ext}");
        n += 1;
    }
    std::fs::write(dir.join(&file), bytes).map_err(|e| e.to_string())?;
    Ok(format!("sounds/{file}"))
}

#[tauri::command]
pub fn import_sound(state: State<AppState>, path: String) -> Result<String, String> {
    let bytes = std::fs::read(&path).map_err(|e| format!("lecture impossible : {e}"))?;
    store_sound(&state.cfg_dir, &path, &bytes)
}

/// Son déposé sur une touche (contenu en base64).
#[tauri::command]
pub fn import_sound_data(state: State<AppState>, name: String, data: String) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).map_err(|e| e.to_string())?;
    store_sound(&state.cfg_dir, &name, &bytes)
}

#[tauri::command]
pub fn test_sound(state: State<AppState>, file: String, volume: u8) {
    let _ = state.engine.lock().unwrap().send(EngineMsg::TestSound { file, volume });
}

#[tauri::command]
pub fn clear_vars(state: State<AppState>) {
    let _ = state.engine.lock().unwrap().send(EngineMsg::ClearVars);
}

/// Sorties audio actives (pour l'action « changer de sortie audio »).
#[tauri::command]
pub fn list_audio_outputs(state: State<AppState>) -> Vec<String> {
    state.audio.list_outputs()
}

/// Ouvre le dossier des variables (fichiers texte pour OBS) dans l'explorateur.
#[tauri::command]
pub fn open_vars_folder(state: State<AppState>) -> Result<String, String> {
    let dir = state.cfg_dir.join("variables");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
    Ok(dir.display().to_string())
}
