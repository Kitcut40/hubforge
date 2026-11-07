//! HubForge — logiciel libre pour le Cooler Master MasterHUB.
//!
//! Version 0.1 (phase 1) : service en console, configuration dans un
//! fichier `config.toml` rechargé à chaud.

mod actions;
mod audio;
mod config;
mod engine;
mod logging;
mod render;

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn pause() {
    print!("\nAppuie sur Entrée pour fermer…");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
}

fn main() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::Console::SetConsoleOutputCP(65001);
        let _ = windows::Win32::System::Console::SetConsoleTitleW(&windows::core::HSTRING::from("HubForge"));
    }

    let dir = config::config_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("Impossible de créer {} : {e}", dir.display());
        pause();
        return;
    }
    logging::init(&dir);

    println!("==============================================================");
    println!("  HubForge v{VERSION} — logiciel libre pour le MasterHUB");
    println!("==============================================================");

    // Une seule instance à la fois (sinon deux programmes se disputent les modules).
    let _lock = match std::net::TcpListener::bind("127.0.0.1:36360") {
        Ok(l) => l,
        Err(_) => {
            log::error!("HubForge est déjà lancé.");
            pause();
            return;
        }
    };

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

    let cfg = match config::load(&cfg_path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("{e}");
            log::error!("→ corrige le fichier et enregistre-le : HubForge le rechargera tout seul.");
            toml::from_str("").expect("configuration vide")
        }
    };

    println!("Configuration : {}", cfg_path.display());
    println!("Modifie ce fichier et enregistre-le : les touches se mettent à jour toutes seules.");
    println!("Pour quitter : Ctrl+C ou ferme cette fenêtre.\n");

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        let _ = ctrlc::set_handler(move || stop.store(true, Ordering::Relaxed));
    }

    let mut engine = match engine::Engine::new(cfg_path, cfg) {
        Ok(e) => e,
        Err(e) => {
            log::error!("accès USB impossible : {e}");
            pause();
            return;
        }
    };
    log::info!("en attente des modules…");
    engine.run(stop);
    log::info!("au revoir !");
}
