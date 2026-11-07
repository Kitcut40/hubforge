//! hubforge-probe — outil de test du protocole USB du Cooler Master MasterHUB.
//!
//! Il détecte les modules, écoute ce qu'ils envoient, et teste quelques
//! commandes sans risque (luminosité, couleurs, images, LED).
//! AUCUNE commande de firmware, de numéro de série ou de réinitialisation
//! n'est envoyée.
//!
//! Tout est enregistré dans un fichier texte à côté de l'exécutable.

#![allow(dead_code)]

mod descriptor;
mod images;
mod log;
mod models;

use descriptor::ReportSizes;
use hidapi::{HidApi, HidDevice};
use models::Kind;
use std::ffi::CString;
use std::io::{self, BufRead, Write};
use std::time::{Duration, Instant};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Une interface HID d'un module MasterHUB.
struct Iface {
    kind: Kind,
    pid: u16,
    serial: String,
    product: String,
    usage_page: u16,
    usage: u16,
    interface: i32,
    path: CString,
    dev: Option<HidDevice>,
    sizes: Option<ReportSizes>,
}

impl Iface {
    fn tag(&self) -> String {
        let sn = if self.serial.len() > 6 {
            &self.serial[self.serial.len() - 6..]
        } else {
            &self.serial
        };
        format!("{}#{}/if{}", self.kind.short(), sn, self.interface)
    }

    fn is_vendor(&self) -> bool {
        self.usage_page >= 0xFF00
    }
}

// ---------------------------------------------------------------------------
// Saisie utilisateur
// ---------------------------------------------------------------------------

fn read_line() -> String {
    let mut s = String::new();
    let _ = io::stdin().lock().read_line(&mut s);
    s.trim().to_string()
}

fn pause(msg: &str) {
    print!("\n>>> {msg} (appuie sur Entrée) ");
    let _ = io::stdout().flush();
    let _ = read_line();
}

fn ask(question: &str) -> String {
    print!("\n??? {question} ");
    let _ = io::stdout().flush();
    let answer = read_line();
    note!("QUESTION: {question} -> REPONSE: {answer}");
    answer
}

fn ask_yes_no(question: &str) -> bool {
    loop {
        let a = ask(&format!("{question} [o/n]")).to_lowercase();
        match a.as_str() {
            "o" | "oui" | "y" | "yes" => return true,
            "n" | "non" | "no" => return false,
            _ => println!("Réponds par o (oui) ou n (non)."),
        }
    }
}

fn title(t: &str) {
    say!("");
    say!("==================================================================");
    say!("  {t}");
    say!("==================================================================");
}

// ---------------------------------------------------------------------------
// Détection
// ---------------------------------------------------------------------------

fn official_software_running() -> bool {
    #[cfg(windows)]
    {
        if let Ok(out) = std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq MasterHUB.exe"])
            .output()
        {
            return String::from_utf8_lossy(&out.stdout)
                .to_lowercase()
                .contains("masterhub.exe");
        }
    }
    false
}

fn enumerate(api: &mut HidApi) -> Vec<Iface> {
    if let Err(e) = api.refresh_devices() {
        say!("Attention : l'énumération USB a signalé une erreur : {e}");
    }
    let mut list = Vec::new();
    for info in api.device_list() {
        let (vid, pid) = (info.vendor_id(), info.product_id());
        if !models::is_masterhub(vid, pid) {
            continue;
        }
        list.push(Iface {
            kind: models::identify(vid, pid),
            pid,
            serial: info.serial_number().unwrap_or("").to_string(),
            product: info.product_string().unwrap_or("").to_string(),
            usage_page: info.usage_page(),
            usage: info.usage(),
            interface: info.interface_number(),
            path: info.path().to_owned(),
            dev: None,
            sizes: None,
        });
    }
    list.sort_by(|a, b| (a.kind as u8, &a.serial, a.interface).cmp(&(b.kind as u8, &b.serial, b.interface)));
    list
}

fn open_all(api: &HidApi, list: &mut [Iface]) {
    for it in list.iter_mut() {
        match api.open_path(&it.path) {
            Ok(dev) => {
                let mut buf = vec![0u8; 4096];
                match dev.get_report_descriptor(&mut buf) {
                    Ok(n) => {
                        note!("{} descripteur ({} octets): {}", it.tag(), n, log::hex(&buf[..n]));
                        it.sizes = Some(descriptor::parse(&buf[..n]));
                    }
                    Err(e) => note!("{} descripteur illisible: {e}", it.tag()),
                }
                it.dev = Some(dev);
            }
            Err(e) => note!("{} ouverture impossible: {e}", it.tag()),
        }
    }
}

fn step_detect(api: &mut HidApi) -> Vec<Iface> {
    title("ÉTAPE 1 — Détection des modules");
    let mut list = enumerate(api);
    if list.is_empty() {
        say!("Aucun module MasterHUB détecté.");
        say!("Vérifie que la base est branchée en USB-C et que les modules sont bien clipsés.");
        return list;
    }
    open_all(api, &mut list);
    for it in &list {
        say!("");
        say!("• {}", it.kind.name());
        say!(
            "    PID 0x{:04X} | n° série {} | interface {} | usage 0x{:04X}/0x{:04X} | « {} »",
            it.pid,
            if it.serial.is_empty() { "(aucun)" } else { &it.serial },
            it.interface,
            it.usage_page,
            it.usage,
            it.product
        );
        match (&it.dev, &it.sizes) {
            (Some(_), Some(s)) => say!("    ouvert ✓ | rapports : {}", descriptor::describe(s)),
            (Some(_), None) => say!("    ouvert ✓ | descripteur non lu"),
            (None, _) => say!("    ✗ impossible à ouvrir (normal pour une interface clavier/souris)"),
        }
    }
    let n_modules = {
        let mut keys: Vec<(u16, &str)> = list
            .iter()
            .filter(|i| i.kind != Kind::BaseHub)
            .map(|i| (i.pid, i.serial.as_str()))
            .collect();
        keys.sort();
        keys.dedup();
        keys.len()
    };
    say!("");
    say!("→ {} module(s) Cooler Master détecté(s).", n_modules);
    list
}

/// Interfaces "principales" (une par module) : celles qui ont des rapports
/// de sortie, de préférence sur une page d'usage constructeur (0xFF00+).
fn main_ifaces(list: &[Iface], kind: Kind) -> Vec<usize> {
    let mut serials: Vec<&str> = list
        .iter()
        .filter(|i| i.kind == kind)
        .map(|i| i.serial.as_str())
        .collect();
    serials.sort();
    serials.dedup();
    let mut out = Vec::new();
    for sn in serials {
        let candidates: Vec<usize> = (0..list.len())
            .filter(|&k| list[k].kind == kind && list[k].serial == sn && list[k].dev.is_some())
            .collect();
        let best = candidates
            .iter()
            .copied()
            .max_by_key(|&k| {
                let it = &list[k];
                let has_out = it.sizes.as_ref().map(|s| !s.output.is_empty()).unwrap_or(false);
                (it.is_vendor() as u8) * 2 + has_out as u8
            });
        if let Some(b) = best {
            out.push(b);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Écoute des contrôles
// ---------------------------------------------------------------------------

fn listen(list: &[Iface], seconds: u64, consigne: &str) {
    say!("");
    say!("{consigne}");
    pause(&format!("Prêt ? L'écoute dure {seconds} secondes"));
    say!("--- écoute démarrée ({seconds} s) ---");
    let open: Vec<&Iface> = list.iter().filter(|i| i.dev.is_some()).collect();
    let mut last: Vec<Vec<u8>> = vec![Vec::new(); open.len()];
    let mut count = vec![0usize; open.len()];
    let start = Instant::now();
    let mut buf = vec![0u8; 2048];
    let mut printed = 0usize;
    while start.elapsed() < Duration::from_secs(seconds) {
        for (k, it) in open.iter().enumerate() {
            let dev = it.dev.as_ref().unwrap();
            match dev.read_timeout(&mut buf, 4) {
                Ok(n) if n > 0 => {
                    count[k] += 1;
                    let data = &buf[..n];
                    note!("RX {} ({} o): {}", it.tag(), n, log::hex(data));
                    if data != last[k].as_slice() {
                        if printed < 400 {
                            println!(
                                "[{:5.2}s] {} : {}",
                                start.elapsed().as_secs_f32(),
                                it.tag(),
                                log::hex_trim(data)
                            );
                        }
                        printed += 1;
                        last[k] = data.to_vec();
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    note!("RX {} erreur: {e}", it.tag());
                }
            }
        }
    }
    say!("--- écoute terminée ---");
    for (k, it) in open.iter().enumerate() {
        say!("  {} : {} rapport(s) reçu(s)", it.tag(), count[k]);
    }
}

// ---------------------------------------------------------------------------
// Envoi de commandes
// ---------------------------------------------------------------------------

/// Envoie un rapport de sortie, complété à la bonne taille si on la connaît.
fn send(it: &Iface, mut data: Vec<u8>) -> bool {
    let Some(dev) = it.dev.as_ref() else {
        return false;
    };
    if let Some(len) = it.sizes.as_ref().and_then(|s| s.output_len_with_id(data[0])) {
        if data.len() < len {
            data.resize(len, 0);
        }
    }
    match dev.write(&data) {
        Ok(n) => {
            note!("TX {} ({} o, écrit {}): {}", it.tag(), data.len(), n, log::hex_trim(&data));
            true
        }
        Err(e) => {
            say!("  ✗ envoi refusé sur {} : {e}", it.tag());
            note!("TX {} ECHEC: {}", it.tag(), log::hex_trim(&data));
            false
        }
    }
}

/// Commande courte du module 15 touches : feature report 32 octets
/// [03, cmd, params...]. Si refusé, essai avec le Report ID = cmd.
fn cmd3(it: &Iface, cmd: u8, params: &[u8]) -> bool {
    let Some(dev) = it.dev.as_ref() else { return false };
    let mut d = vec![0u8; 32];
    d[0] = 0x03;
    d[1] = cmd;
    d[2..2 + params.len()].copy_from_slice(params);
    match dev.send_feature_report(&d) {
        Ok(()) => {
            note!("FEAT {} OK: {}", it.tag(), log::hex_trim(&d));
            return true;
        }
        Err(e) => note!("FEAT {} ECHEC ({e}): {}", it.tag(), log::hex_trim(&d)),
    }
    let mut d2 = vec![0u8; 32];
    d2[0] = cmd;
    d2[1..1 + params.len()].copy_from_slice(params);
    match dev.send_feature_report(&d2) {
        Ok(()) => {
            note!("FEAT(alt) {} OK: {}", it.tag(), log::hex_trim(&d2));
            say!("  (accepté avec la variante « Report ID = commande »)");
            true
        }
        Err(e) => {
            note!("FEAT(alt) {} ECHEC ({e}): {}", it.tag(), log::hex_trim(&d2));
            say!("  ✗ commande refusée par le module : {e}");
            false
        }
    }
}

/// Envoie une image en paquets (commande 0x36 = touche, 0x35 = écran).
fn send_image(it: &Iface, cmd: u8, key: u8, jpeg: &[u8]) -> bool {
    let Some(packet_len) = it.sizes.as_ref().and_then(|s| s.max_output_len()) else {
        say!("  ✗ taille des paquets inconnue (descripteur non lu) : image non envoyée");
        return false;
    };
    if packet_len <= 8 {
        say!("  ✗ taille de paquet trop petite ({packet_len})");
        return false;
    }
    let chunk = packet_len - 8;
    let total = jpeg.len().div_ceil(chunk);
    for (index, part) in jpeg.chunks(chunk).enumerate() {
        let mut p = vec![0u8; packet_len];
        p[0] = 0x02;
        p[1] = cmd;
        p[2..4].copy_from_slice(&(index as u16).to_le_bytes());
        p[4..6].copy_from_slice(&(part.len() as u16).to_le_bytes());
        p[6] = key;
        p[7] = (index + 1 == total) as u8;
        p[8..8 + part.len()].copy_from_slice(part);
        if !send(it, p) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Tests des modules
// ---------------------------------------------------------------------------

fn step_keys15(list: &[Iface]) {
    let idx = main_ifaces(list, Kind::Keys15);
    title("ÉTAPE 3 — Module 15 touches");
    if idx.is_empty() {
        say!("Module 15 touches non détecté (ou impossible à ouvrir) : étape sautée.");
        return;
    }
    for &k in &idx {
        let it = &list[k];
        say!("Interface utilisée : {}", it.tag());

        say!("\n[3.1] Activation du « mode logiciel » (commande 03 3A 01)");
        cmd3(it, 0x3A, &[1]);
        std::thread::sleep(Duration::from_millis(300));
        ask_yes_no("Quelque chose a-t-il changé sur les écrans des touches ?");

        listen(
            list,
            15,
            "Pendant l'écoute : appuie sur la touche EN HAUT À GAUCHE, puis celle juste à sa droite,\n\
             puis la touche EN BAS À DROITE. Lentement, une par une.",
        );

        say!("\n[3.2] Luminosité à 10 % (commande 03 34 0A)");
        cmd3(it, 0x34, &[10]);
        let dim = ask_yes_no("Les écrans sont-ils devenus plus sombres ?");
        say!("      Luminosité à 100 % (commande 03 34 64)");
        cmd3(it, 0x34, &[100]);
        if dim {
            ask_yes_no("Les écrans sont-ils revenus lumineux ?");
        }

        say!("\n[3.3] Couleur unie sur chaque touche (commande 03 37)");
        for key in 0..15u8 {
            let (r, g, b) = images::key_color(key);
            cmd3(it, 0x37, &[key, r, g, b]);
            std::thread::sleep(Duration::from_millis(40));
        }
        ask(
            "Que vois-tu ? (ex. « 15 couleurs différentes », « rien », « seulement certaines touches »)",
        );

        say!("\n[3.4] Image JPEG 72x72 avec un numéro sur chaque touche (commande 02 36)");
        let mut ok = true;
        for key in 0..15u8 {
            let jpeg = images::to_jpeg(&images::key_test_image(key), 80);
            note!("image touche {key}: {} octets JPEG", jpeg.len());
            if !send_image(it, 0x36, key, &jpeg) {
                ok = false;
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        if ok {
            ask(
                "Les numéros 1 à 15 sont-ils affichés ? Si oui, dans quel ordre ?\n    \
                 (ex. « 1 à 5 sur la première ligne de gauche à droite », « à l'envers », « rien »)",
            );
            ask("L'image est-elle à l'endroit (petit coin blanc en haut à gauche de chaque touche) ?");
        }
    }
}

fn step_keys15_v2(list: &[Iface]) {
    let idx = main_ifaces(list, Kind::Keys15);
    title("TEST — Commandes courtes du module 15 touches");
    if idx.is_empty() {
        say!("Module 15 touches non détecté : test impossible.");
        return;
    }
    for &k in &idx {
        let it = &list[k];
        say!("Interface utilisée : {}", it.tag());

        say!("\n[A] Luminosité des écrans à 10 %");
        cmd3(it, 0x34, &[10]);
        let dim = ask_yes_no("Les écrans des touches sont-ils devenus plus sombres ?");
        say!("    Luminosité à 100 %");
        cmd3(it, 0x34, &[100]);
        if dim {
            ask_yes_no("Sont-ils revenus lumineux ?");
        }

        say!("\n[B] Couleur unie sur chaque touche");
        for key in 0..15u8 {
            let (r, g, b) = images::key_color(key);
            cmd3(it, 0x37, &[key, r, g, b]);
            std::thread::sleep(Duration::from_millis(40));
        }
        ask("Que vois-tu ? (« 15 couleurs unies sans numéro », « rien de changé », autre)");

        say!("\n[C] Mode logiciel DÉSACTIVÉ (03 3A 00)");
        cmd3(it, 0x3A, &[0]);
        std::thread::sleep(Duration::from_millis(300));
        ask("Quelque chose a changé sur les touches ? (décris, ou « non »)");
        listen(list, 10, "Appuie sur 3 touches au hasard, lentement.");

        say!("\n[D] Mode logiciel ACTIVÉ (03 3A 01)");
        cmd3(it, 0x3A, &[1]);
        std::thread::sleep(Duration::from_millis(300));
        ask("Quelque chose a changé sur les touches ? (décris, ou « non »)");
        listen(list, 10, "Appuie de nouveau sur 3 touches au hasard, lentement.");

        say!("\n[E] Remise des images numérotées sur les touches");
        for key in 0..15u8 {
            let jpeg = images::to_jpeg(&images::key_test_image(key), 80);
            if !send_image(it, 0x36, key, &jpeg) {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        ask_yes_no("Les numéros 1 à 15 sont-ils de nouveau affichés ?");
    }
}

fn step_m0_leds(list: &[Iface]) {
    title("ÉTAPE 4 — LED des faders et de la base");
    let mut tested = false;
    for kind in [Kind::Faders5, Kind::Knobs3, Kind::BaseLed] {
        let Some((target, index)) = kind.m0_led_target() else { continue };
        for k in main_ifaces(list, kind) {
            tested = true;
            let it = &list[k];
            say!("\n{} ({})", kind.name(), it.tag());
            for (nom, r, g, b) in [("ROUGE", 255u8, 0u8, 0u8), ("VERT", 0, 255, 0), ("BLEU", 0, 0, 255)] {
                say!("  Couleur {nom} (commande 02 2D {target:02X} {index:02X} …, sans sauvegarde)");
                send(it, vec![0x02, 0x2D, target, index, r, g, b, 0]);
                std::thread::sleep(Duration::from_millis(200));
                ask(&format!("De quelle couleur sont les LED maintenant ? (attendu : {nom})"));
            }
            say!("  Luminosité LED 20 (commande 02 2E {target:02X} {index:02X} 14)");
            send(it, vec![0x02, 0x2E, target, index, 20]);
            ask("Les LED ont-elles baissé ? (oui / non / éteintes)");
            say!("  Luminosité LED 100 (commande 02 2E {target:02X} {index:02X} 64)");
            send(it, vec![0x02, 0x2E, target, index, 100]);
            ask("Et maintenant ? (plus fort / pareil / autre)");
        }
    }
    if !tested {
        say!("Aucun module à LED détecté : étape sautée.");
    } else {
        say!("\nRemarque : les couleurs testées ne sont pas sauvegardées dans les modules.");
        say!("Débrancher / rebrancher la base ou relancer le logiciel officiel rétablit tes réglages.");
    }
}

fn step_faders_feature(list: &[Iface]) {
    title("ÉTAPE 5 — Lecture directe des faders (feature report 0x33)");
    let idx = main_ifaces(list, Kind::Faders5);
    if idx.is_empty() {
        say!("Module faders non détecté : étape sautée.");
        return;
    }
    pause("Mets le fader 1 (le plus à gauche) TOUT EN HAUT et les 4 autres TOUT EN BAS");
    for &k in &idx {
        let it = &list[k];
        for attempt in 0..2 {
            let mut buf = vec![0u8; 33];
            buf[0] = 0x33;
            match it.dev.as_ref().unwrap().get_feature_report(&mut buf) {
                Ok(n) => {
                    say!("  {} → {} octets : {}", it.tag(), n, log::hex_trim(&buf[..n]));
                    break;
                }
                Err(e) => {
                    say!("  {} → lecture impossible (essai {}) : {e}", it.tag(), attempt + 1);
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }
    pause("Maintenant mets les 5 faders À MI-HAUTEUR");
    for &k in &idx {
        let it = &list[k];
        let mut buf = vec![0u8; 33];
        buf[0] = 0x33;
        match it.dev.as_ref().unwrap().get_feature_report(&mut buf) {
            Ok(n) => say!("  {} → {}", it.tag(), log::hex_trim(&buf[..n])),
            Err(e) => say!("  {} → lecture impossible : {e}", it.tag()),
        }
    }
}

// ---------------------------------------------------------------------------

fn setup_console() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleOutputCP(65001);
        windows_sys::Win32::System::Console::SetConsoleCP(65001);
    }
}

fn main() {
    setup_console();
    let log_path = log::init();
    title(&format!("HubForge — outil de test du MasterHUB (v{VERSION})"));
    say!("Ce programme détecte tes modules et teste le protocole USB.");
    say!("Il n'envoie AUCUNE commande de firmware ni de réinitialisation.");
    say!("Journal enregistré dans : {}", log_path.display());
    note!("OS: {} {}", std::env::consts::OS, std::env::consts::ARCH);

    if official_software_running() {
        say!("");
        say!("⚠  Le logiciel officiel MasterHUB est OUVERT.");
        say!("   Ferme-le complètement (aussi dans la zone de notification, près de l'horloge),");
        say!("   sinon il va se battre avec cet outil pour l'accès aux modules.");
        pause("Quand c'est fait");
        if official_software_running() {
            say!("   Il semble encore ouvert. On continue quand même, mais les résultats peuvent être faussés.");
        }
    }

    let mut api = match HidApi::new() {
        Ok(a) => a,
        Err(e) => {
            say!("Erreur : impossible d'initialiser l'accès HID : {e}");
            pause("Fin");
            return;
        }
    };

    let list = step_detect(&mut api);
    if list.iter().any(|i| i.kind == Kind::Keys15) {
        step_keys15_v2(&list);
    }

    title("TERMINÉ — merci !");
    say!("Envoie-moi ce fichier dans la conversation :");
    say!("  {}", log::path().map(|p| p.display().to_string()).unwrap_or_default());
    say!("Tu peux maintenant rouvrir le logiciel officiel si tu veux retrouver tes réglages.");
    pause("Fermer");
}
