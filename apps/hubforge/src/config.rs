//! Configuration de HubForge : un fichier `config.toml` lisible, rechargé
//! automatiquement à chaque enregistrement.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub general: General,
    #[serde(default)]
    pub faders: Vec<FaderSlot>,
    #[serde(default)]
    pub rollers: Rollers,
    #[serde(default)]
    pub pages: Vec<Page>,
    /// Animations des LED, couleur des faders selon leur position, jeux.
    #[serde(default)]
    pub lights: crate::lights::Lights,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct General {
    /// Luminosité des écrans des touches (0–100).
    pub brightness: u8,
    /// Luminosité des LED (0–100).
    pub led_brightness: u8,
    /// Couleur des LED de la base.
    pub base_color: String,
    /// Assombrir la touche pendant l'appui.
    pub press_feedback: bool,
    /// Débit maximal d'images envoyées pour les GIF (images par seconde).
    pub gif_max_fps: u32,
    /// Effet des LED de la base : 1 fixe, 2 respiration, 3 stroboscope,
    /// 5 cycle, 6 aléatoire, 7 vague, 8 eau, 9 arc-en-ciel, 0 éteint.
    pub base_effect: u8,
    /// Changer de page tout seul quand une application liée à une page
    /// passe au premier plan.
    pub auto_pages: bool,
    /// Durée d'un appui long (millisecondes).
    pub long_press_ms: u32,
    /// Délai maximal entre les deux appuis d'un double appui (millisecondes).
    pub double_press_ms: u32,
    /// Soundboard : volume général des sons (0–100), sortie principale
    /// (nom du périphérique, vide = sortie par défaut de Windows) et seconde
    /// sortie facultative (ex. câble audio virtuel pour envoyer les sons dans Discord).
    pub sound_volume: u8,
    pub sound_device: String,
    pub sound_device2: String,
    /// Reprise douce des faders : après un changement de page ou de profil,
    /// un fader qui contrôle autre chose n'agit qu'une fois passé par le
    /// volume actuel (pas de saut brutal).
    pub fader_pickup: bool,
    /// Ville des touches météo (vide = pas de météo, aucun accès à internet).
    pub weather_city: String,
}

impl Default for General {
    fn default() -> Self {
        General {
            brightness: 80,
            led_brightness: 100,
            base_color: "#00C8FF".into(),
            press_feedback: true,
            gif_max_fps: 28,
            base_effect: 1,
            auto_pages: true,
            long_press_ms: 500,
            double_press_ms: 300,
            sound_volume: 80,
            sound_device: String::new(),
            sound_device2: String::new(),
            fader_pickup: true,
            weather_city: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FaderSlot {
    /// Dans une page : numéro du fader remplacé (1 à 5). Inutilisé dans `[[faders]]`.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub index: u8,
    /// "master", "mic", "app:nom.exe" (plusieurs séparés par des virgules), "none"
    pub target: String,
    /// Couleur de la LED (mode « fixed ») ; couleur « allumé » du mode « status ».
    #[serde(default = "default_fader_color")]
    pub color: String,
    /// LED : "fixed" (couleur fixe), "position" (selon la position du fader),
    /// "animation", "status" (rouge si coupé, éteinte si l'appli ne tourne
    /// pas), "vu" (niveau sonore de ce que le fader contrôle).
    #[serde(default = "default_led_mode", skip_serializing_if = "is_fixed")]
    pub led: String,
    /// Dégradé « position → couleur » (modes « position » et « vu ») : "0:#00FF00", "100:#FF0000".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gradient: Vec<String>,
    /// Animation (mode « animation »).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub animation: String,
}

fn default_led_mode() -> String {
    "fixed".into()
}

fn is_fixed(s: &String) -> bool {
    s == "fixed"
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

fn default_fader_color() -> String {
    "#FFFFFF".into()
}

/// Molettes. Modes : "volume" (volume général), "mic", "app:nom.exe"
/// (volume d'applications), "scroll", "hscroll", "zoom", "actions"
/// (une action par cran, dans chaque sens) ou "none".
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Rollers {
    pub right: String,
    pub left: String,
    pub volume_step: u8,
    pub scroll_step: i32,
    pub invert_right: bool,
    pub invert_left: bool,
    /// Actions du mode « actions ».
    #[serde(skip_serializing_if = "WheelActions::is_empty")]
    pub left_actions: WheelActions,
    #[serde(skip_serializing_if = "WheelActions::is_empty")]
    pub right_actions: WheelActions,
}

impl Default for Rollers {
    fn default() -> Self {
        Rollers {
            right: "volume".into(),
            left: "scroll".into(),
            volume_step: 2,
            scroll_step: 3,
            invert_right: false,
            invert_left: false,
            left_actions: WheelActions::default(),
            right_actions: WheelActions::default(),
        }
    }
}

/// Actions d'une molette en mode « actions » : un cran vers le haut, un cran vers le bas.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WheelActions {
    #[serde(default)]
    pub up: Action,
    #[serde(default)]
    pub down: Action,
}

impl WheelActions {
    pub fn is_empty(&self) -> bool {
        self.up == Action::None && self.down == Action::None
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    #[serde(default)]
    pub name: String,
    /// Applications (ex. "fusion360.exe") qui affichent cette page
    /// automatiquement quand elles passent au premier plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<String>,
    /// Page « dossier » : ouverte par une touche (action folder), absente
    /// du défilement page suivante / précédente.
    #[serde(default, skip_serializing_if = "is_false")]
    pub folder: bool,
    /// Dossier : revenir à la page d'avant après une action.
    #[serde(default, skip_serializing_if = "is_false")]
    pub close_after: bool,
    #[serde(default)]
    pub keys: Vec<KeyDef>,
    /// Faders propres à cette page (les autres gardent le réglage commun).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faders: Vec<FaderSlot>,
    /// Molettes propres à cette page (None = réglage commun).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roller_left: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roller_right: Option<String>,
    /// Actions des molettes propres à cette page (mode « actions »).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roller_left_actions: Option<WheelActions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roller_right_actions: Option<WheelActions>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyDef {
    /// Numéro de la touche, de 1 (en haut à gauche) à 15 (en bas à droite).
    pub key: u8,
    #[serde(default)]
    pub label: String,
    /// Image (png, jpg, gif, webp), chemin relatif au dossier de config ou absolu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Couleur de fond (si pas d'image).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Couleur du texte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(default)]
    pub action: Action,
    /// Second état : la touche alterne entre son état normal et celui-ci
    /// à chaque appui (texte, image, couleur et action propres).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toggle: Option<KeyState>,
    /// Touche vivante : "clock", "date", "cpu", "ram", "gpu", "gpu_temp",
    /// "vram", "volume", "app_volume", "mic", "weather".
    /// L'image est redessinée en continu (le texte sert de titre).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget: Option<String>,
    /// Style de la touche vivante (ex. "analog" pour l'horloge, "gauge",
    /// "graph" pour CPU et RAM). Vide = style par défaut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget_style: Option<String>,
    /// Ce que montre la touche vivante, quand il faut le préciser : les
    /// applications d'un widget « app_volume » ("discord.exe,spotify.exe").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget_target: Option<String>,
    /// Action d'un appui long (None = pas d'appui long).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<Action>,
    /// Action d'un double appui (None = pas de double appui).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double: Option<Action>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct KeyState {
    #[serde(default)]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(default)]
    pub action: Action,
}

impl KeyDef {
    /// Définition de l'état n° `state` (0 = normal, 1 = second état).
    pub fn state(&self, state: usize) -> KeyDef {
        match (&self.toggle, state) {
            (Some(t), 1) => KeyDef {
                key: self.key,
                label: t.label.clone(),
                icon: t.icon.clone(),
                color: t.color.clone(),
                text_color: t.text_color.clone(),
                action: t.action.clone(),
                toggle: None,
                widget: None,
                widget_style: None,
                widget_target: None,
                long: self.long.clone(),
                double: self.double.clone(),
            },
            _ => KeyDef { toggle: None, ..self.clone() },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    #[default]
    None,
    /// Raccourci clavier, ex. "ctrl+shift+esc"
    Hotkey { keys: String },
    /// Touches maintenues tant que la touche du MasterHUB est enfoncée
    /// (klaxon, accélérateur, « appuyer pour parler »…).
    Hold { keys: String },
    /// Joue un son (soundboard). `file` : chemin absolu ou relatif au dossier
    /// de configuration ; `mode` : "toggle" (rappuyer arrête), "restart"
    /// (rappuyer recommence) ou "overlap" (les sons se superposent).
    Sound {
        file: String,
        #[serde(default = "default_volume")]
        volume: u8,
        #[serde(default = "default_sound_mode")]
        mode: String,
    },
    /// Arrête tous les sons en cours
    StopSounds,
    /// Tape un texte
    Text { text: String },
    /// Lance une application, ouvre un fichier, un dossier ou une URL
    Launch {
        target: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
    },
    /// Commande multimédia : play_pause, next, previous, stop, mute,
    /// volume_up, volume_down, mic_mute
    Media { command: String },
    /// Couper / rétablir le son d'applications ("discord.exe,spotify.exe").
    AppMute { app: String },
    /// Change de page : "next", "previous" ou un numéro (1, 2…)
    Page { to: String },
    /// Verrouille la session
    Lock,
    /// Ouvre le fichier de configuration
    OpenConfig,
    /// Ouvre une page dossier (numéro de page, 1 = la première)
    Folder { to: String },
    /// Ferme le dossier : revient à la page d'avant
    Back,
    /// Change de profil : "next", "previous" ou le nom du profil
    Profile { to: String },
    /// Lance le mini-jeu caché
    Game,
    /// Suite d'actions exécutées l'une après l'autre (macro)
    Macro {
        #[serde(default)]
        steps: Vec<Action>,
    },
    /// Pause dans une macro, en millisecondes
    Wait { ms: u64 },
    /// Variable : "set" (donne la valeur), "add" (ajoute la valeur, négative
    /// pour retirer), "toggle" (0 ↔ 1). La valeur peut contenir des {variables}.
    Var {
        name: String,
        #[serde(default = "default_var_op")]
        op: String,
        #[serde(default)]
        value: String,
    },
    /// Condition : compare `left` et `right` (textes qui peuvent contenir des
    /// {variables}, ou {app}, {page}, {profile}, {hour}, {minute}, {weekday}),
    /// puis exécute `then` ou `else`.
    If {
        left: String,
        #[serde(default = "default_cmp")]
        cmp: String,
        #[serde(default)]
        right: String,
        #[serde(default, rename = "then", skip_serializing_if = "Vec::is_empty")]
        then_steps: Vec<Action>,
        #[serde(default, rename = "else", skip_serializing_if = "Vec::is_empty")]
        else_steps: Vec<Action>,
    },
}

impl Action {
    /// Vrai si cette action, ou une action qu'elle contient (macro, condition), vérifie `f`.
    pub fn any(&self, f: &dyn Fn(&Action) -> bool) -> bool {
        f(self)
            || match self {
                Action::Macro { steps } => steps.iter().any(|s| s.any(f)),
                Action::If { then_steps, else_steps, .. } => then_steps.iter().chain(else_steps).any(|s| s.any(f)),
                _ => false,
            }
    }

    /// Actions contenues (étapes d'une macro, branches d'une condition).
    pub fn children_mut(&mut self) -> Vec<&mut Action> {
        match self {
            Action::Macro { steps } => steps.iter_mut().collect(),
            Action::If { then_steps, else_steps, .. } => then_steps.iter_mut().chain(else_steps.iter_mut()).collect(),
            _ => Vec::new(),
        }
    }
}

/// Opérations et comparaisons possibles.
pub const VAR_OPS: [&str; 3] = ["set", "add", "toggle"];
pub const CMPS: [&str; 7] = ["==", "!=", ">", "<", ">=", "<=", "contains"];
/// Profondeur maximale des conditions imbriquées.
pub const IF_MAX_DEPTH: usize = 4;

fn default_var_op() -> String {
    "set".into()
}

fn default_cmp() -> String {
    "==".into()
}

/// Nombre maximal d'étapes dans une macro et durée maximale d'une pause.
pub const MACRO_MAX_STEPS: usize = 100;
pub const WAIT_MAX_MS: u64 = 60_000;

fn default_volume() -> u8 {
    100
}

fn default_sound_mode() -> String {
    "toggle".into()
}

pub fn validate_action(a: &Action, depth: usize) -> Result<(), String> {
    match a {
        Action::Macro { steps } => {
            if depth > 0 {
                return Err("une macro ne peut pas contenir une autre macro".into());
            }
            if steps.len() > MACRO_MAX_STEPS {
                return Err(format!("macro trop longue ({} étapes, maximum {MACRO_MAX_STEPS})", steps.len()));
            }
            steps.iter().try_for_each(|s| validate_action(s, depth + 1))
        }
        Action::Wait { ms } if *ms > WAIT_MAX_MS => Err(format!("pause trop longue ({ms} ms, maximum {WAIT_MAX_MS} ms)")),
        Action::Var { name, op, .. } => {
            if !crate::vars::valid_name(name) {
                return Err(format!("nom de variable invalide « {name} » (lettres, chiffres, _ et -)"));
            }
            if crate::vars::SPECIALS.contains(&name.trim().to_lowercase().as_str()) {
                return Err(format!("« {name} » est un nom réservé (app, page, profile, hour, minute, weekday)"));
            }
            if !VAR_OPS.contains(&op.as_str()) {
                return Err(format!("opération inconnue « {op} » (set, add ou toggle)"));
            }
            Ok(())
        }
        Action::If { cmp, then_steps, else_steps, .. } => {
            if !CMPS.contains(&cmp.as_str()) {
                return Err(format!("comparaison inconnue « {cmp} »"));
            }
            if depth >= IF_MAX_DEPTH {
                return Err(format!("trop de conditions imbriquées (maximum {IF_MAX_DEPTH})"));
            }
            if then_steps.len() + else_steps.len() > MACRO_MAX_STEPS {
                return Err(format!("condition trop longue (maximum {MACRO_MAX_STEPS} étapes)"));
            }
            then_steps.iter().chain(else_steps).try_for_each(|s| validate_action(s, depth + 1))
        }
        _ => Ok(()),
    }
}

/// Actions des molettes (réglage commun puis pages), pour la validation.
fn wheel_actions(cfg: &Config) -> Vec<(String, &Action)> {
    let mut v = Vec::new();
    let r = &cfg.rollers;
    for (side, w) in [("gauche", &r.left_actions), ("droite", &r.right_actions)] {
        v.push((format!("molette {side}"), &w.up));
        v.push((format!("molette {side}"), &w.down));
    }
    for (p, page) in cfg.pages.iter().enumerate() {
        for (side, w) in [("gauche", &page.roller_left_actions), ("droite", &page.roller_right_actions)] {
            if let Some(w) = w {
                v.push((format!("page {} molette {side}", p + 1), &w.up));
                v.push((format!("page {} molette {side}", p + 1), &w.down));
            }
        }
    }
    v
}

pub fn parse_color(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// Enregistre la configuration (écriture atomique : fichier temporaire puis
/// renommage, pour ne jamais laisser un fichier à moitié écrit).
pub fn save(path: &Path, cfg: &Config) -> Result<(), String> {
    validate(cfg)?;
    let body = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let text = format!(
        "# HubForge — configuration\n# Modifiée par l'interface de HubForge. Tu peux aussi l'éditer à la main :\n# enregistre le fichier et HubForge le rechargera tout seul.\n\n{body}"
    );
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("écriture impossible : {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("écriture impossible : {e}"))?;
    Ok(())
}

pub fn config_dir() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    base.unwrap_or_else(|| PathBuf::from(".")).join("HubForge")
}

pub fn load(path: &Path) -> Result<Config, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("lecture impossible : {e}"))?;
    let mut cfg: Config = toml::from_str(&text).map_err(|e| format!("erreur dans config.toml :\n{e}"))?;
    migrate(&mut cfg);
    validate(&cfg)?;
    Ok(cfg)
}

/// Reprend les réglages des anciennes versions (0.9 : LED réglées pour les
/// 5 faders à la fois ; elles se règlent maintenant fader par fader).
pub fn migrate(cfg: &mut Config) {
    let l = &mut cfg.lights;
    if l.faders_mode == "position" || l.faders_mode == "animation" {
        for slot in cfg.faders.iter_mut().filter(|s| s.led == "fixed") {
            slot.led = l.faders_mode.clone();
            slot.gradient = l.gradient.clone();
            slot.animation = l.faders_animation.clone();
        }
    }
    l.faders_mode.clear();
    l.faders_animation.clear();
    l.gradient.clear();
    l.game = None;
}

pub fn validate(cfg: &Config) -> Result<(), String> {
    for (p, page) in cfg.pages.iter().enumerate() {
        for k in &page.keys {
            if !(1..=15).contains(&k.key) {
                return Err(format!("page {} : la touche {} n'existe pas (1 à 15)", p + 1, k.key));
            }
            if let Some(w) = &k.widget {
                if !crate::widgets::is_known(w) {
                    return Err(format!("page {} touche {} : touche vivante inconnue « {w} »", p + 1, k.key));
                }
            }
            let alt = k.toggle.as_ref();
            for a in std::iter::once(&k.action).chain(alt.map(|t| &t.action)).chain(k.long.iter()).chain(k.double.iter()) {
                validate_action(a, 0).map_err(|e| format!("page {} touche {} : {e}", p + 1, k.key))?;
            }
            for c in [&k.color, &k.text_color, &alt.and_then(|t| t.color.clone()), &alt.and_then(|t| t.text_color.clone())]
                .into_iter()
                .flatten()
            {
                if parse_color(c).is_none() {
                    return Err(format!("page {} touche {} : couleur invalide « {c} » (format #RRVVBB)", p + 1, k.key));
                }
            }
        }
    }
    for (ctx, a) in wheel_actions(cfg) {
        validate_action(a, 0).map_err(|e| format!("{ctx} : {e}"))?;
    }
    for (i, f) in cfg.faders.iter().enumerate() {
        if parse_color(&f.color).is_none() {
            return Err(format!("fader {} : couleur invalide « {} »", i + 1, f.color));
        }
    }
    if parse_color(&cfg.general.base_color).is_none() {
        return Err(format!("couleur de la base invalide « {} »", cfg.general.base_color));
    }
    Ok(())
}

/// Configuration créée au premier lancement.
pub const DEFAULT_CONFIG: &str = r##"# =====================================================================
#  HubForge — configuration
#  Ce fichier est relu automatiquement à chaque enregistrement :
#  modifie-le, sauvegarde (Ctrl+S), et ton MasterHUB se met à jour.
#  Couleurs au format "#RRVVBB" (ex. "#FF0000" = rouge).
# =====================================================================

[general]
brightness = 80          # luminosité des écrans des touches (0 à 100)
led_brightness = 100     # luminosité des LED des faders et de la base (0 à 100)
base_color = "#00C8FF"   # couleur des LED de la base
base_effect = 1          # effet de la base : 1 fixe, 2 respiration, 3 stroboscope, 5 cycle,
                         # 6 aléatoire, 7 vague, 8 eau, 9 arc-en-ciel, 0 éteint
press_feedback = true    # la touche s'assombrit pendant l'appui
gif_max_fps = 28         # images par seconde max pour les GIF (toutes touches confondues)
auto_pages = true        # afficher tout seul la page liée à l'application au premier plan

# ---------------------------------------------------------------------
#  Faders, de gauche à droite.
#  target = "master"            volume général
#           "mic"               volume du micro
#           "app:discord.exe"   volume d'une application (plusieurs : "app:chrome.exe,msedge.exe")
#           "none"              rien
# ---------------------------------------------------------------------
[[faders]]
target = "master"
color = "#00C8FF"

[[faders]]
target = "app:discord.exe"
color = "#5865F2"

[[faders]]
target = "app:spotify.exe"
color = "#1DB954"

[[faders]]
target = "app:chrome.exe,msedge.exe,firefox.exe"
color = "#FFB300"

[[faders]]
target = "mic"
color = "#FF3B30"

# ---------------------------------------------------------------------
#  Molettes : "volume", "mic", "app:discord.exe", "scroll", "hscroll",
#  "zoom", "actions" (une action par cran : left_actions / right_actions)
#  ou "none"
# ---------------------------------------------------------------------
[rollers]
right = "volume"
left = "scroll"
volume_step = 2          # % de volume par cran
scroll_step = 3          # lignes par cran
invert_right = false     # inverser le sens si besoin
invert_left = false

# ---------------------------------------------------------------------
#  Pages de touches. Touche 1 = en haut à gauche, 15 = en bas à droite.
#  Types d'action :
#    { type = "hotkey", keys = "ctrl+shift+esc" }
#    { type = "text", text = "Bonjour !" }
#    { type = "launch", target = "notepad.exe" }        (appli, fichier, dossier ou https://…)
#    { type = "media", command = "play_pause" }        (play_pause, next, previous, stop,
#                                                       mute, volume_up, volume_down, mic_mute)
#    { type = "page", to = "next" }                    ("next", "previous" ou un numéro)
#    { type = "lock" }                                 (verrouiller le PC)
#    { type = "open_config" }                          (ouvrir ce fichier)
#  icon = "mon_image.png" : image (png, jpg, gif animé, webp) dans le dossier de ce fichier
#  icon = "builtin:play"   : icône intégrée (voir la liste dans l'interface)
#  toggle = { … }          : second état (la touche alterne à chaque appui)
# ---------------------------------------------------------------------
[[pages]]
name = "Accueil"

[[pages.keys]]
key = 1
label = "Explorateur"
icon = "builtin:folder-open"
color = "#F4B400"
action = { type = "launch", target = "explorer.exe" }

[[pages.keys]]
key = 2
label = "Calculatrice"
icon = "builtin:calculator"
color = "#455A64"
action = { type = "launch", target = "calc.exe" }

[[pages.keys]]
key = 3
label = "HubForge GitHub"
icon = "builtin:globe"
color = "#24292F"
action = { type = "launch", target = "https://github.com/Kitcut40/hubforge" }

[[pages.keys]]
key = 4
label = "Gestion. tâches"
icon = "builtin:activity"
color = "#1565C0"
action = { type = "hotkey", keys = "ctrl+shift+esc" }

[[pages.keys]]
key = 5
label = "Capture"
icon = "builtin:scan"
color = "#00897B"
action = { type = "hotkey", keys = "win+shift+s" }

[[pages.keys]]
key = 6
label = "Précédent"
icon = "builtin:skip-back"
color = "#37474F"
action = { type = "media", command = "previous" }

[[pages.keys]]
key = 7
label = "Lecture / Pause"
icon = "builtin:play"
color = "#2E7D32"
action = { type = "media", command = "play_pause" }

[[pages.keys]]
key = 8
label = "Suivant"
icon = "builtin:skip-forward"
color = "#37474F"
action = { type = "media", command = "next" }

[[pages.keys]]
key = 9
label = "Muet"
icon = "builtin:volume-x"
color = "#C62828"
action = { type = "media", command = "mute" }

[[pages.keys]]
key = 10
label = "Verrouiller"
icon = "builtin:lock"
color = "#6A1B9A"
action = { type = "lock" }

[[pages.keys]]
key = 11
label = "Bonjour"
icon = "builtin:type"
color = "#EF6C00"
action = { type = "text", text = "Bonjour depuis HubForge !" }

[[pages.keys]]
key = 12
label = "Bureau"
icon = "builtin:monitor"
color = "#0277BD"
action = { type = "hotkey", keys = "win+d" }

[[pages.keys]]
key = 13
label = "GIF"
icon = "demo.gif"
action = { type = "none" }

[[pages.keys]]
key = 14
label = "Config"
icon = "builtin:settings"
color = "#546E7A"
action = { type = "open_config" }

[[pages.keys]]
key = 15
label = "Page 2"
icon = "builtin:arrow-right"
color = "#212121"
action = { type = "page", to = "next" }

[[pages]]
name = "Page 2"
# apps = ["notepad.exe"]   # décommente : cette page s'affiche quand le Bloc-notes est au premier plan

[[pages.keys]]
key = 1
label = "Bloc-notes"
icon = "builtin:notebook-pen"
color = "#0288D1"
action = { type = "launch", target = "notepad.exe" }

[[pages.keys]]
key = 2
label = "Paramètres"
icon = "builtin:settings"
color = "#607D8B"
action = { type = "hotkey", keys = "win+i" }

[[pages.keys]]
key = 3
label = "Copier"
icon = "builtin:copy"
color = "#5D4037"
action = { type = "hotkey", keys = "ctrl+c" }

[[pages.keys]]
key = 4
label = "Coller"
icon = "builtin:clipboard-paste"
color = "#5D4037"
action = { type = "hotkey", keys = "ctrl+v" }

[[pages.keys]]
key = 5
label = "Annuler"
icon = "builtin:undo-2"
color = "#5D4037"
action = { type = "hotkey", keys = "ctrl+z" }

[[pages.keys]]
key = 6
label = "Micro"
icon = "builtin:mic"
color = "#2E7D32"
action = { type = "media", command = "mic_mute" }
toggle = { label = "Micro coupé", icon = "builtin:mic-off", color = "#C62828", action = { type = "media", command = "mic_mute" } }

# Macro : plusieurs actions à la suite, avec des pauses (ms).
[[pages.keys]]
key = 7
label = "Macro démo"
icon = "builtin:layers"
color = "#6A1B9A"
action = { type = "macro", steps = [
    { type = "launch", target = "notepad.exe" },
    { type = "wait", ms = 1000 },
    { type = "text", text = "Bonjour depuis HubForge !" },
] }

# Dossier : ouvre la page « Applis » (page 3) ; la touche Retour la referme.
[[pages.keys]]
key = 8
label = "Applis"
icon = "builtin:folder"
color = "#F4B400"
action = { type = "folder", to = "3" }

# Touches vivantes : l'image se met à jour toute seule.
# widget = "clock", "date", "weather", "cpu", "ram", "gpu", "gpu_temp",
# "vram", "volume", "app_volume" (avec widget_target = "discord.exe") ou "mic".
[[pages.keys]]
key = 11
widget = "clock"
color = "#1F2229"

[[pages.keys]]
key = 12
widget = "cpu"
color = "#1F2229"
action = { type = "hotkey", keys = "ctrl+shift+esc" }

[[pages.keys]]
key = 13
widget = "ram"
color = "#1F2229"
action = { type = "hotkey", keys = "ctrl+shift+esc" }

[[pages.keys]]
key = 14
widget = "volume"
color = "#1F2229"
action = { type = "media", command = "mute" }

[[pages.keys]]
key = 15
label = "Accueil"
icon = "builtin:house"
color = "#212121"
action = { type = "page", to = "1" }

# ---------------------------------------------------------------------
#  Dossier : page ouverte par une touche. folder = true la retire de
#  « page suivante / précédente » ; close_after = true revient à la page
#  d'avant après chaque action.
# ---------------------------------------------------------------------
[[pages]]
name = "Applis"
folder = true
close_after = true

[[pages.keys]]
key = 1
label = "Retour"
icon = "builtin:arrow-left"
color = "#212121"
action = { type = "back" }

[[pages.keys]]
key = 2
label = "Explorateur"
icon = "builtin:folder-open"
color = "#F4B400"
action = { type = "launch", target = "explorer.exe" }

[[pages.keys]]
key = 3
label = "Calculatrice"
icon = "builtin:calculator"
color = "#455A64"
action = { type = "launch", target = "calc.exe" }

[[pages.keys]]
key = 4
label = "Bloc-notes"
icon = "builtin:notebook-pen"
color = "#0288D1"
action = { type = "launch", target = "notepad.exe" }

[[pages.keys]]
key = 5
label = "Paramètres"
icon = "builtin:settings"
color = "#607D8B"
action = { type = "hotkey", keys = "win+i" }
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macros_parse_and_validate() {
        let cfg: Config = toml::from_str(
            r#"
            [[pages]]
            name = "Test"
            [[pages.keys]]
            key = 1
            action = { type = "macro", steps = [
                { type = "hotkey", keys = "ctrl+s" },
                { type = "wait", ms = 300 },
                { type = "text", text = "ok" },
            ] }
            "#,
        )
        .unwrap();
        validate(&cfg).unwrap();
        match &cfg.pages[0].keys[0].action {
            Action::Macro { steps } => assert_eq!(steps.len(), 3),
            a => panic!("{a:?}"),
        }
        // Enregistrement puis relecture : la macro est identique.
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.pages[0].keys[0].action, cfg.pages[0].keys[0].action);
        let nested: Config = toml::from_str(
            r#"
            [[pages]]
            [[pages.keys]]
            key = 1
            action = { type = "macro", steps = [ { type = "macro", steps = [] } ] }
            "#,
        )
        .unwrap();
        assert!(validate(&nested).is_err());
    }

    #[test]
    fn default_config_is_valid() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        validate(&cfg).unwrap();
        assert_eq!(cfg.pages.len(), 3);
        assert!(cfg.pages[2].folder && cfg.pages[2].close_after);
        assert!(cfg.pages[1].keys.iter().any(|k| k.widget.is_some()));
        assert!(cfg.pages[1].keys.iter().any(|k| matches!(k.action, Action::Folder { .. })));
        assert!(cfg.pages[1].keys.iter().any(|k| k.toggle.is_some()));
        assert!(cfg.pages[1].keys.iter().any(|k| matches!(k.action, Action::Macro { .. })));
        assert_eq!(cfg.faders.len(), 5);
        assert_eq!(
            cfg.pages[0].keys[3].action,
            Action::Hotkey { keys: "ctrl+shift+esc".into() }
        );
    }

    #[test]
    fn save_roundtrip() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        let dir = std::env::temp_dir().join("hubforge-cfg-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.toml");
        save(&p, &cfg).unwrap();
        let back = load(&p).unwrap();
        assert_eq!(back.pages.len(), cfg.pages.len());
        assert_eq!(back.pages[0].keys[3].action, cfg.pages[0].keys[3].action);
        assert_eq!(back.faders[1].target, "app:discord.exe");
    }

    #[test]
    fn variables_conditions_and_wheel_actions() {
        let cfg: Config = toml::from_str(
            r#"
            [rollers]
            left = "actions"
            left_actions = { up = { type = "hotkey", keys = "ctrl+y" }, down = { type = "hotkey", keys = "ctrl+z" } }

            [[pages]]
            name = "A"
            roller_right = "actions"
            roller_right_actions = { up = { type = "var", name = "compteur", op = "add", value = "1" } }
            [[pages.keys]]
            key = 1
            label = "{compteur}"
            action = { type = "if", left = "{compteur}", cmp = ">=", right = "10", then = [
                { type = "var", name = "compteur", value = "0" },
            ], else = [
                { type = "var", name = "compteur", op = "add", value = "1" },
                { type = "wait", ms = 100 },
            ] }
            "#,
        )
        .unwrap();
        validate(&cfg).unwrap();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.pages[0].keys[0].action, cfg.pages[0].keys[0].action);
        assert_eq!(back.rollers.left_actions, cfg.rollers.left_actions);
        assert!(cfg.pages[0].keys[0].action.any(&|a| matches!(a, Action::Wait { .. })));
        let bad: Config = toml::from_str(
            r#"
            [[pages]]
            [[pages.keys]]
            key = 1
            action = { type = "var", name = "a b" }
            "#,
        )
        .unwrap();
        assert!(validate(&bad).is_err());
        let bad: Config = toml::from_str(
            r#"
            [rollers]
            right_actions = { up = { type = "macro", steps = [ { type = "macro" } ] } }
            "#,
        )
        .unwrap();
        assert!(validate(&bad).is_err());
    }

    #[test]
    fn colors() {
        assert_eq!(parse_color("#FF8000"), Some((255, 128, 0)));
        assert_eq!(parse_color("nope"), None);
    }

    #[test]
    fn old_fader_lights_are_migrated_per_fader() {
        let dir = std::env::temp_dir().join("hubforge-migrate-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("p.toml");
        std::fs::write(
            &path,
            r##"
[[faders]]
target = "master"
color = "#FF0000"

[[faders]]
target = "mic"

[lights]
faders_mode = "position"
gradient = ["0:#000000", "100:#FFFFFF"]

[lights.game]
enabled = true

[[pages]]
name = "A"

[[pages.faders]]
index = 2
target = "app:discord.exe"
led = "vu"

[[pages.keys]]
key = 1
action = { type = "media", command = "play_pause" }
long = { type = "lock" }
double = { type = "media", command = "next" }
"##,
        )
        .unwrap();
        let cfg = load(&path).unwrap();
        assert_eq!(cfg.faders[0].led, "position");
        assert_eq!(cfg.faders[1].gradient, vec!["0:#000000".to_string(), "100:#FFFFFF".to_string()]);
        assert!(cfg.lights.faders_mode.is_empty() && cfg.lights.game.is_none());
        assert_eq!(cfg.pages[0].faders[0].index, 2);
        assert!(cfg.pages[0].keys[0].long.is_some() && cfg.pages[0].keys[0].double.is_some());
        let text = toml::to_string(&cfg).unwrap();
        assert!(!text.contains("faders_mode") && !text.contains("[lights.game]"));
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.faders[0].led, "position");
    }
}