//! HubForge — logiciel libre pour le Cooler Master MasterHUB.
//!
//! Une seule application : le moteur (qui pilote le MasterHUB) tourne en
//! arrière-plan, l'interface s'ouvre depuis l'icône près de l'horloge.
//! Fermer la fenêtre ne coupe pas le MasterHUB ; « Quitter » arrête tout.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod actions;
mod audio;
mod autostart;
mod config;
mod engine;
mod logging;
mod render;
mod ui;

use engine::{Engine, EngineMsg, Status};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
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

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn main() {
    let dir = config::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    logging::init(&dir);
    let hidden = std::env::args().any(|a| a == "--hidden");

    // Instance unique : si HubForge tourne déjà, on lui demande d'afficher sa fenêtre.
    let listener = match TcpListener::bind(("127.0.0.1", INSTANCE_PORT)) {
        Ok(l) => l,
        Err(_) => {
            if let Ok(mut s) = TcpStream::connect(("127.0.0.1", INSTANCE_PORT)) {
                let _ = s.write_all(b"show");
            }
            return;
        }
    };

    log::info!("HubForge v{} démarre", env!("CARGO_PKG_VERSION"));
    let cfg_path = dir.join("config.toml");
    if !cfg_path.exists() {
        match std::fs::write(&cfg_path, config::DEFAULT_CONFIG) {
            Ok(()) => log::info!("configuration d'exemple créée"),
            Err(e) => log::error!("impossible de créer la configuration : {e}"),
        }
    }
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

    let status = Arc::new(Mutex::new(Status { config_error: cfg_error, ..Default::default() }));
    let audio = audio::Audio::start();
    let stop = Arc::new(AtomicBool::new(false));
    let (engine_tx, engine_rx) = mpsc::channel::<EngineMsg>();
    let engine_thread: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::new(Mutex::new(None));

    let state = ui::AppState {
        cfg_path: cfg_path.clone(),
        cfg_dir: dir.clone(),
        cfg: Mutex::new(cfg.clone()),
        engine: Mutex::new(engine_tx),
        status: status.clone(),
        audio: audio.clone(),
    };

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            ui::get_initial,
            ui::apply_config,
            ui::render_keys,
            ui::show_page,
            ui::list_audio_apps,
            ui::set_autostart,
            ui::open_config_folder,
            ui::pick_file,
            ui::pick_folder,
            ui::import_image,
            ui::import_image_data,
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
                    let t = std::thread::Builder::new()
                        .name("moteur".into())
                        .spawn(move || {
                            let notify_status = status.clone();
                            let notify_handle = handle.clone();
                            let notify: engine::Notify = Box::new(move |s: &Status| {
                                *notify_status.lock().unwrap() = s.clone();
                                let _ = notify_handle.emit("status", s);
                            });
                            match Engine::new(cfg_path, cfg, audio, notify) {
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

                // --- Instance unique : afficher la fenêtre sur demande ---
                {
                    let handle = handle.clone();
                    std::thread::spawn(move || {
                        for mut conn in listener.incoming().flatten() {
                            let mut buf = [0u8; 8];
                            let _ = conn.set_read_timeout(Some(Duration::from_millis(500)));
                            if conn.read(&mut buf).is_ok() {
                                let h = handle.clone();
                                let _ = handle.run_on_main_thread(move || show_main(&h));
                            }
                        }
                    });
                }

                // --- Icône près de l'horloge ---
                let open = MenuItem::with_id(app, "open", "Ouvrir HubForge", true, None::<&str>)?;
                let auto = CheckMenuItem::with_id(
                    app,
                    "autostart",
                    "Démarrer avec Windows",
                    cfg!(windows),
                    autostart::is_enabled(),
                    None::<&str>,
                )?;
                let quit = MenuItem::with_id(app, "quit", "Quitter HubForge", true, None::<&str>)?;
                let sep = PredefinedMenuItem::separator(app)?;
                let menu = Menu::with_items(app, &[&open, &auto, &sep, &quit])?;
                let auto_item = auto.clone();
                let stop_tray = stop.clone();
                let thread_tray = engine_thread.clone();
                TrayIconBuilder::with_id("main")
                    .icon(app.default_window_icon().cloned().ok_or("icône manquante")?)
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

                if !hidden {
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
