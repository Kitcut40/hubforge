#![cfg_attr(not(windows), allow(dead_code))]
//! Notifications sur les touches : lit les notifications de Windows (celles
//! du centre de notifications : Discord, WhatsApp, Outlook…) pour les
//! touches vivantes « Notifications », et y ajoute celles des addons.
//!
//! Windows demande l'accord de l'utilisateur (Paramètres › Confidentialité ›
//! Notifications : « Autoriser les applications à accéder à vos
//! notifications »). Rien n'est lu tant qu'aucune touche n'en affiche, et
//! rien ne sort du PC.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Une notification.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub id: String,
    /// Application (nom affiché par Windows, ou nom de l'addon).
    pub app: String,
    pub title: String,
    pub body: String,
    /// Moment de la notification (millisecondes depuis 1970).
    pub at: u64,
}

/// Accès aux notifications de Windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    /// Pas encore demandé (aucune touche n'affiche de notifications).
    #[default]
    Unknown,
    Allowed,
    Denied,
    /// Windows ne le permet pas ici (version trop ancienne, autre système).
    Unsupported,
}

#[derive(Default)]
struct Store {
    access: Access,
    windows: Vec<Item>,
    local: Vec<Item>,
    version: u64,
    merged: Arc<Vec<Item>>,
}

fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}

/// Une touche affiche des notifications : on lit celles de Windows.
static WANTED: AtomicBool = AtomicBool::new(false);
static STARTED: AtomicBool = AtomicBool::new(false);

/// Notifications des addons gardées au plus.
const LOCAL_MAX: usize = 50;
/// Notifications plus anciennes que ceci : oubliées.
const MAX_AGE: Duration = Duration::from_secs(24 * 3600);

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn rebuild(s: &mut Store) {
    let oldest = now_ms().saturating_sub(MAX_AGE.as_millis() as u64);
    let mut all: Vec<Item> = s.windows.iter().chain(s.local.iter()).filter(|i| i.at >= oldest).cloned().collect();
    all.sort_by(|a, b| b.at.cmp(&a.at));
    if *s.merged != all {
        s.merged = Arc::new(all);
        s.version += 1;
    }
}

/// Toutes les notifications (la plus récente d'abord) et leur version.
pub fn snapshot() -> (u64, Arc<Vec<Item>>) {
    let s = store().lock().unwrap();
    (s.version, s.merged.clone())
}

pub fn access() -> Access {
    store().lock().unwrap().access
}

/// Notification envoyée par un addon (programme) : elle compte pour les
/// touches « Notifications » de cet addon.
pub fn push_local(app: &str, title: &str, body: &str) {
    let mut s = store().lock().unwrap();
    let at = now_ms();
    let id = format!("local-{at}-{}", s.local.len());
    s.local.insert(0, Item { id, app: app.to_string(), title: title.to_string(), body: body.to_string(), at });
    s.local.truncate(LOCAL_MAX);
    rebuild(&mut s);
}

/// Faut-il lire les notifications de Windows (une touche en affiche) ?
pub fn set_wanted(on: bool) {
    WANTED.store(on, Ordering::Relaxed);
    if on && !STARTED.swap(true, Ordering::Relaxed) {
        std::thread::Builder::new()
            .name("notifications".into())
            .spawn(poll_loop)
            .ok();
    }
}

fn poll_loop() {
    imp::init_thread();
    loop {
        if WANTED.load(Ordering::Relaxed) {
            let (access, items) = imp::read();
            let mut s = store().lock().unwrap();
            if access != s.access {
                log::info!("notifications de Windows : {access:?}");
            }
            s.access = access;
            if let Some(items) = items {
                s.windows = items;
            }
            rebuild(&mut s);
        }
        std::thread::sleep(Duration::from_millis(1500));
    }
}

/// Demande l'accès à Windows (affiche sa fenêtre d'autorisation si besoin).
/// À appeler depuis la fenêtre de HubForge (fil de l'interface).
pub fn request_access() {
    imp::request_access();
    set_wanted(true);
}

/// La notification correspond-elle à la cible d'une touche ? Cible vide :
/// toutes ; sinon un ou plusieurs noms séparés par des virgules, comparés au
/// nom de l'application (« discord », « whatsapp »…).
pub fn matches(item: &Item, target: &str) -> bool {
    let t = target.trim().to_lowercase();
    if t.is_empty() {
        return true;
    }
    let app = item.app.to_lowercase();
    t.split(',').map(|p| p.trim().trim_end_matches(".exe")).filter(|p| !p.is_empty()).any(|p| app.contains(p) || p.contains(&app) && !app.is_empty())
}

/// Notifications d'une cible arrivées après `seen` (ms) : nombre et la plus récente.
pub fn unseen<'a>(items: &'a [Item], target: &str, seen: u64) -> (usize, Option<&'a Item>) {
    let mut n = 0;
    let mut last = None;
    for i in items.iter().filter(|i| i.at > seen && matches(i, target)) {
        n += 1;
        if last.is_none() {
            last = Some(i);
        }
    }
    (n, last)
}

#[cfg(windows)]
mod imp {
    use super::{Access, Item};
    use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
    use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds};

    pub fn init_thread() {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
        }
    }

    pub fn request_access() {
        if let Ok(l) = UserNotificationListener::Current() {
            // Pas d'attente : Windows affiche sa question, la réponse est lue
            // par le fil des notifications.
            if let Err(e) = l.RequestAccessAsync() {
                log::warn!("accès aux notifications : {e}");
            }
        }
    }

    fn status() -> Access {
        let Ok(l) = UserNotificationListener::Current() else { return Access::Unsupported };
        match l.GetAccessStatus() {
            Ok(UserNotificationListenerAccessStatus::Allowed) => Access::Allowed,
            Ok(UserNotificationListenerAccessStatus::Denied) => Access::Denied,
            Ok(_) => Access::Unknown,
            Err(_) => Access::Unsupported,
        }
    }

    fn text_of(n: &windows::UI::Notifications::UserNotification) -> (String, String) {
        let texts = (|| {
            let binding = n.Notification()?.Visual()?.GetBinding(&KnownNotificationBindings::ToastGeneric()?)?;
            let els = binding.GetTextElements()?;
            let mut out = Vec::new();
            for i in 0..els.Size()? {
                out.push(els.GetAt(i)?.Text()?.to_string_lossy());
            }
            windows::core::Result::Ok(out)
        })()
        .unwrap_or_default();
        let title = texts.first().cloned().unwrap_or_default();
        let body = texts.iter().skip(1).cloned().collect::<Vec<_>>().join(" — ");
        (title, body)
    }

    /// (accès, notifications) ; None si elles n'ont pas pu être lues.
    pub fn read() -> (Access, Option<Vec<Item>>) {
        let access = status();
        if access != Access::Allowed {
            return (access, None);
        }
        let Ok(l) = UserNotificationListener::Current() else { return (Access::Unsupported, None) };
        let list = match l.GetNotificationsAsync(NotificationKinds::Toast).and_then(|op| op.join()) {
            Ok(list) => list,
            Err(e) => {
                log::debug!("lecture des notifications : {e}");
                return (access, None);
            }
        };
        let mut items = Vec::new();
        let n = list.Size().unwrap_or(0).min(200);
        for i in 0..n {
            let Ok(un) = list.GetAt(i) else { continue };
            let app = un.AppInfo().and_then(|a| a.DisplayInfo()).and_then(|d| d.DisplayName()).map(|h| h.to_string_lossy()).unwrap_or_default();
            // DateTime : centaines de nanosecondes depuis 1601.
            let at = un.CreationTime().map(|t| ((t.UniversalTime / 10_000) - 11_644_473_600_000).max(0) as u64).unwrap_or(0);
            let (title, body) = text_of(&un);
            let id = un.Id().map(|v| v.to_string()).unwrap_or_default();
            items.push(Item { id, app, title, body, at });
        }
        (access, Some(items))
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Access, Item};

    pub fn init_thread() {}

    pub fn request_access() {}

    pub fn read() -> (Access, Option<Vec<Item>>) {
        (Access::Unsupported, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(app: &str, at: u64) -> Item {
        Item { id: format!("{app}{at}"), app: app.into(), title: "t".into(), body: String::new(), at }
    }

    #[test]
    fn matching_and_counting() {
        let items = vec![it("Discord", 300), it("WhatsApp", 200), it("Discord", 100)];
        assert!(matches(&items[0], ""));
        assert!(matches(&items[0], "discord"));
        assert!(matches(&items[0], "Discord.exe"));
        assert!(matches(&items[1], "discord, whatsapp"));
        assert!(!matches(&items[1], "discord"));
        let (n, last) = unseen(&items, "discord", 0);
        assert_eq!(n, 2);
        assert_eq!(last.unwrap().at, 300);
        assert_eq!(unseen(&items, "discord", 150).0, 1);
        assert_eq!(unseen(&items, "", 0).0, 3);
    }

    #[test]
    fn local_notifications_are_merged() {
        push_local("Ping", "Connexion perdue", "");
        let (_, all) = snapshot();
        assert!(all.iter().any(|i| i.app == "Ping" && i.title == "Connexion perdue"));
    }
}
