//! HubForge — logiciel libre pour le Cooler Master MasterHUB.
//!
//! Une seule application : le moteur (qui pilote le MasterHUB) tourne en
//! arrière-plan, l'interface s'ouvre depuis l'icône près de l'horloge.
//! Fermer la fenêtre ne coupe pas le MasterHUB ; « Quitter » arrête tout.
//!
//! Options de la ligne de commande :
//! - `--hidden` : démarre sans ouvrir la fenêtre (démarrage avec Windows) ;
//! - `--quit` : demande à HubForge déjà lancé de se fermer (installateur) ;
//! - `--install-addon <fichier.forge>` ou simplement `<fichier.forge>` :
//!   installe un addon (double-clic sur un fichier .forge).

#![cfg_attr(windows, windows_subsystem = "windows")]

mod actions;
mod addons;
mod anims;
mod appicon;
mod audio;
mod clip;
mod i18n;
mod connect;
mod autostart;
mod backups;
mod config;
mod doom;
mod engine;
mod foreground;
mod game;
mod icons_data;
mod lights;
mod logging;
mod official;
mod profiles;
mod render;
mod sdimport;
mod share;
mod media;
mod pcstate;
mod sound;
mod theme;
mod theme_icons;
mod vars;
mod weather;
mod topology;
mod ui;
mod update;
mod widgets;

use engine::{Engine, EngineMsg, Status};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

/// Port local utilisé pour n'avoir qu'une seule instance de HubForge.
const INSTANCE_PORT: u16 = 36360;
/// Délai entre deux vérifications automatiques des mises à jour.
const UPDATE_EVERY_SECS: u64 = 20 * 3600;

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Fichier .forge (addon) ou .hfprofile (profil) passé en argument
/// (double-clic, `--install-addon` ou `--import-profile`).
fn addon_argument() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--install-addon" || a == "--import-profile") {
        return args.get(i + 1).map(PathBuf::from);
    }
    args.iter()
        .find(|a| {
            let l = a.to_lowercase();
            l.ends_with(".forge") || l.ends_with(".hfprofile") || l.ends_with(".streamdeckprofile") || l.ends_with(".streamdeckiconpack")
        })
        .map(PathBuf::from)
}

/// Envoie un message à HubForge déjà lancé. Retourne false s'il ne tourne pas.
fn send_to_running(msg: &str) -> bool {
    match TcpStream::connect_timeout(&([127, 0, 0, 1], INSTANCE_PORT).into(), Duration::from_millis(800)) {
        Ok(mut s) => {
            let _ = s.write_all(msg.as_bytes());
            true
        }
        Err(_) => false,
    }
}

/// Installe un addon .forge (ou importe un profil .hfprofile) et prévient l'interface.
fn install_addon_file(app: &AppHandle, dir: &std::path::Path, path: &std::path::Path) {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if ["hfprofile", "streamdeckprofile", "streamdeckiconpack"].contains(&ext.as_str()) {
        let res = std::fs::read(path).map_err(|e| format!("lecture impossible : {e}")).and_then(|b| sdimport::import_any(dir, &b));
        match &res {
            Ok(r) if r.kind != "profile" => log::info!("pack d'icônes « {} » importé depuis {}", r.name, path.display()),
            Ok(r) => {
                log::info!("profil « {} » importé depuis {}", r.name, path.display());
                if let Some(st) = app.try_state::<ui::AppState>() {
                    let _ = st.engine.lock().unwrap().send(EngineMsg::SwitchProfile(r.name.clone()));
                }
            }
            Err(e) => log::error!("{} : {e}", path.display()),
        }
        show_main(app);
        let _ = app.emit("profile-imported", res);
        return;
    }
    let res = std::fs::read(path).map_err(|e| format!("lecture impossible : {e}")).and_then(|b| addons::install_bytes(dir, &b));
    match &res {
        Ok(id) => {
            official::unmark_removed(dir, id);
            log::info!("addon « {id} » installé depuis {}", path.display());
        }
        Err(e) => log::error!("{} : {e}", path.display()),
    }
    show_main(app);
    let _ = app.emit("addon-installed", res.map_err(|e| e.to_string()));
}

fn main() {
    let dir = config::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let args: Vec<String> = std::env::args().collect();

    // --quit : fermer HubForge déjà lancé (utilisé par l'installateur) et attendre.
    if args.iter().any(|a| a == "--quit") {
        if send_to_running("quit") {
            for _ in 0..80 {
                std::thread::sleep(Duration::from_millis(100));
                if TcpStream::connect_timeout(&([127, 0, 0, 1], INSTANCE_PORT).into(), Duration::from_millis(100)).is_err() {
                    break;
                }
            }
        }
        return;
    }

    // --doom : processus du moteur de Doom (easter egg), lancé par HubForge lui-même.
    if args.get(1).map(|a| a == "--doom").unwrap_or(false) && args.len() >= 4 {
        doom::child_main(&args[2], &args[3]);
    }

    logging::init(&dir);
    i18n::set(&i18n::resolve(&profiles::load_settings(&dir).lang));
    let hidden = args.iter().any(|a| a == "--hidden");
    let addon_to_install = addon_argument();

    // Instance unique : si HubForge tourne déjà, on lui transmet la demande.
    let listener = match TcpListener::bind(("127.0.0.1", INSTANCE_PORT)) {
        Ok(l) => l,
        Err(_) => {
            match &addon_to_install {
                Some(p) => send_to_running(&format!("addon:{}", p.display())),
                None => send_to_running("show"),
            };
            return;
        }
    };

    log::info!("HubForge v{} démarre", update::CURRENT);
    official::install(&dir);
    let active = profiles::init(&dir);
    log::info!("profil « {} »", active.name);
    let cfg_path = active.path.clone();
    let active: profiles::SharedActive = Arc::new(Mutex::new(active));
    let demo = dir.join("demo.gif");
    if !demo.exists() {
        if let Err(e) = render::write_demo_gif(&demo) {
            log::warn!("GIF de démonstration non créé : {e}");
        }
    }
    let (cfg, cfg_error) = match config::load(&cfg_path) {
        Ok(c) => (c, None),
        Err(e) => {
            log::error!("{e}");
            (toml::from_str("").expect("configuration vide"), Some(e))
        }
    };

    let status = Arc::new(Mutex::new(Status {
        config_error: cfg_error,
        profile: active.lock().unwrap().name.clone(),
        ..Default::default()
    }));
    let audio = audio::Audio::start();
    let stop = Arc::new(AtomicBool::new(false));
    let (engine_tx, engine_rx) = mpsc::channel::<EngineMsg>();
    let engine_thread: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));
    let update_state: ui::SharedUpdate = Arc::new(Mutex::new(ui::UpdateState::default()));

    let state = ui::AppState {
        active: active.clone(),
        cfg_dir: dir.clone(),
        engine: Mutex::new(engine_tx),
        status: status.clone(),
        audio: audio.clone(),
        update: update_state.clone(),
    };

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            ui::get_initial,
            ui::list_profiles,
            ui::switch_profile,
            ui::create_profile,
            ui::rename_profile,
            ui::delete_profile,
            ui::start_game,
            ui::list_backups,
            ui::restore_backup,
            ui::export_profile,
            ui::import_profile,
            ui::import_profile_data,
            ui::list_icon_packs,
            ui::remove_icon_pack,
            ui::icon_pack_thumbs,
            ui::get_ai_settings,
            ui::set_ai_settings,
            ui::test_ai,
            ui::test_http,
            ui::set_ui_mode,
            ui::set_auto_profiles,
            ui::set_lang,
            ui::finish_wizard,
            ui::create_starter_profile,
            ui::get_diagnostic,
            ui::reconnect_modules,
            ui::light_presets,
            ui::preview_animation,
            ui::get_layout,
            ui::set_base_rot,
            ui::set_placement,
            ui::list_sound_devices,
            ui::import_sound,
            ui::import_sound_data,
            ui::test_sound,
            ui::clear_vars,
            ui::list_audio_outputs,
            ui::open_vars_folder,
            ui::start_doom,
            ui::download_freedoom,
            ui::stop_doom,
            ui::doom_key,
            ui::reset_base,
            ui::list_addons,
            ui::addons_signature,
            ui::install_addon,
            ui::install_addon_data,
            ui::remove_addon,
            ui::restore_official_addons,
            ui::set_addon_enabled,
            ui::addon_pages,
            ui::open_addons_folder,
            ui::app_info,
            ui::list_running_apps,
            ui::apply_config,
            ui::render_keys,
            ui::show_page,
            ui::list_audio_apps,
            ui::set_autostart,
            ui::open_config_folder,
            ui::open_url,
            ui::pick_file,
            ui::pick_folder,
            ui::import_image,
            ui::import_image_data,
            ui::list_icons,
            ui::extract_app_icon,
            ui::get_update_state,
            ui::check_updates_now,
            ui::set_update_auto,
            ui::skip_update,
            ui::install_update,
        ])
        .setup({
            let stop = stop.clone();
            let engine_thread = engine_thread.clone();
            move |app| {
                let handle = app.handle().clone();

                // --- Moteur ---
                {
                    let handle = handle.clone();
                    let status = status.clone();
                    let stop = stop.clone();
                    let engine_dir = dir.clone();
                    let engine_active = active.clone();
                    let t = std::thread::Builder::new()
                        .name("moteur".into())
                        .spawn(move || {
                            let notify_status = status.clone();
                            let notify_handle = handle.clone();
                            let notify: engine::Notify = Box::new(move |s: &Status| {
                                *notify_status.lock().unwrap() = s.clone();
                                let _ = notify_handle.emit("status", s);
                            });
                            match Engine::new(engine_dir, engine_active, cfg, audio, notify) {
                                Ok(mut e) => {
                                    log::info!("en attente des modules…");
                                    e.run(stop, engine_rx);
                                }
                                Err(e) => {
                                    log::error!("accès USB impossible : {e}");
                                    let mut s = status.lock().unwrap();
                                    s.config_error = Some(format!("Accès USB impossible : {e}"));
                                    let _ = handle.emit("status", &*s);
                                }
                            }
                        })?;
                    *engine_thread.lock().unwrap() = Some(t);
                }

                // --- Instance unique : messages d'une autre instance ---
                {
                    let handle = handle.clone();
                    let dir = dir.clone();
                    std::thread::spawn(move || {
                        for mut conn in listener.incoming().flatten() {
                            let mut buf = vec![0u8; 4096];
                            let _ = conn.set_read_timeout(Some(Duration::from_millis(800)));
                            let Ok(n) = conn.read(&mut buf) else { continue };
                            let msg = String::from_utf8_lossy(&buf[..n]).to_string();
                            let h = handle.clone();
                            let d = dir.clone();
                            if msg == "quit" {
                                log::info!("fermeture demandée (installation d'une mise à jour)");
                                let _ = handle.run_on_main_thread(move || h.exit(0));
                            } else if let Some(path) = msg.strip_prefix("addon:") {
                                let path = PathBuf::from(path);
                                std::thread::spawn(move || install_addon_file(&h, &d, &path));
                            } else {
                                let _ = handle.run_on_main_thread(move || show_main(&h));
                            }
                        }
                    });
                }

                // --- Icône près de l'horloge ---
                let open = MenuItem::with_id(app, "open", i18n::t("Ouvrir HubForge", "Open HubForge"), true, None::<&str>)?;
                let auto = CheckMenuItem::with_id(
                    app,
                    "autostart",
                    i18n::t("Démarrer avec Windows", "Start with Windows"),
                    cfg!(windows),
                    autostart::is_enabled(),
                    None::<&str>,
                )?;
                let quit = MenuItem::with_id(app, "quit", i18n::t("Quitter HubForge", "Quit HubForge"), true, None::<&str>)?;
                let sep = PredefinedMenuItem::separator(app)?;
                let menu = Menu::with_items(app, &[&open, &auto, &sep, &quit])?;
                let auto_item = auto.clone();
                let stop_tray = stop.clone();
                let thread_tray = engine_thread.clone();
                TrayIconBuilder::with_id("main")
                    .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                    .tooltip("HubForge")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(move |app, ev| match ev.id().as_ref() {
                        "open" => show_main(app),
                        "autostart" => {
                            let want = !autostart::is_enabled();
                            if let Err(e) = autostart::set(want) {
                                log::error!("démarrage automatique : {e}");
                            }
                            let _ = auto_item.set_checked(autostart::is_enabled());
                            let _ = app.emit("autostart", autostart::is_enabled());
                        }
                        "quit" => {
                            stop_tray.store(true, Ordering::Relaxed);
                            if let Some(t) = thread_tray.lock().unwrap().take() {
                                let _ = t.join();
                            }
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, ev| {
                        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                            show_main(tray.app_handle());
                        }
                    })
                    .build(app)?;

                // --- Mises à jour : vérification discrète en arrière-plan ---
                {
                    let handle = handle.clone();
                    let dir = dir.clone();
                    let update_state = update_state.clone();
                    std::thread::Builder::new().name("mises-a-jour".into()).spawn(move || {
                        std::thread::sleep(Duration::from_secs(20));
                        loop {
                            let st = profiles::load_settings(&dir);
                            if !st.no_update_check && update::now_secs().saturating_sub(st.last_update_check) >= UPDATE_EVERY_SECS {
                                ui::run_update_check(&handle, &dir, &update_state);
                            }
                            std::thread::sleep(Duration::from_secs(3600));
                        }
                    })?;
                }

                // --- Addon passé en argument (double-clic sur un .forge) ---
                if let Some(path) = addon_to_install.clone() {
                    let h = handle.clone();
                    let d = dir.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(1500));
                        install_addon_file(&h, &d, &path);
                    });
                }

                if !hidden || addon_to_install.is_some() {
                    show_main(&handle);
                }
                Ok(())
            }
        })
        .on_window_event(|window, event| {
            // Fermer la fenêtre la cache seulement : le MasterHUB continue de fonctionner.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("impossible de démarrer l'interface");

    app.run(move |_app, event| match event {
        RunEvent::ExitRequested { code: None, api, .. } => {
            // Toutes les fenêtres sont cachées : on reste dans la zone de notification.
            api.prevent_exit();
        }
        RunEvent::Exit => {
            stop.store(true, Ordering::Relaxed);
            if let Some(t) = engine_thread.lock().unwrap().take() {
                let _ = t.join();
            }
            log::info!("au revoir !");
        }
        _ => {}
    });
}
