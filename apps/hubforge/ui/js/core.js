"use strict";
/* HubForge — Pont avec Rust, catalogue des actions, état, démarrage, enregistrement, pages. */

/* HubForge — interface (JavaScript sans dépendance).
   Toutes les modifications sont appliquées en direct au MasterHUB et
   enregistrées automatiquement dans config.toml. */

// ---------------------------------------------------------------------------
// Pont avec Rust (et simulation quand on ouvre la page dans un navigateur)
// ---------------------------------------------------------------------------
const TAURI = window.__TAURI__;
const invoke = TAURI ? TAURI.core.invoke : mockInvoke;
const listen = TAURI ? TAURI.event.listen : () => Promise.resolve();

// ---------------------------------------------------------------------------
// Catalogue des actions
// ---------------------------------------------------------------------------
const MEDIA = {
  play_pause: ["Lecture / Pause", "play"], next: ["Piste suivante", "skip-forward"], previous: ["Piste précédente", "skip-back"],
  stop: ["Stop", "square"], mute: ["Couper le son", "volume-x"], volume_up: ["Volume +", "volume-2"],
  volume_down: ["Volume −", "volume-1"], mic_mute: ["Couper le micro", "mic-off"],
};

/** Touches animées intégrées : [id, nom, icône de la liste]. */
const ANIMS = [["live", "En direct", "circle-dot"], ["rec", "Enregistrement", "circle-dot"], ["onair", "On Air", "radio"], ["equalizer", "Égaliseur", "activity"],
  ["wave", "Onde sonore", "activity"], ["loader", "Chargement", "refresh-cw"], ["heartbeat", "Battement de cœur", "heart"], ["radar", "Radar", "target"],
  ["bell", "Notification", "bell"], ["rainbow", "Arc-en-ciel", "sparkles"], ["sparkles", "Scintillements", "sparkles"], ["fire", "Flamme", "flame"],
  ["rocket", "Fusée", "rocket"], ["coffee", "Pause café", "coffee"], ["rain", "Pluie", "cloud"], ["matrix", "Pluie de code", "code"]];
const animName = (id) => (ANIMS.find(([v]) => v === id) || [0, id])[1];

const LIBRARY = [
  { cat: "Clavier", items: [
    { name: "Raccourci clavier", label: "Raccourci", icon: "keyboard", color: "#3949AB", action: { type: "hotkey", keys: "" } },
    { name: "Taper un texte", label: "Texte", icon: "type", color: "#EF6C00", action: { type: "text", text: "" } },
    { name: "Maintenir des touches", label: "Maintenir", icon: "keyboard", color: "#283593", action: { type: "hold", keys: "" } },
    { name: "Macro (plusieurs actions)", label: "Macro", icon: "layers", color: "#6A1B9A", action: { type: "macro", steps: [{ type: "hotkey", keys: "" }] } },
    { name: "Copier", label: "Copier", icon: "copy", color: "#5D4037", action: { type: "hotkey", keys: "ctrl+c" } },
    { name: "Coller", label: "Coller", icon: "clipboard-paste", color: "#5D4037", action: { type: "hotkey", keys: "ctrl+v" } },
    { name: "Couper", label: "Couper", icon: "scissors", color: "#5D4037", action: { type: "hotkey", keys: "ctrl+x" } },
    { name: "Annuler", label: "Annuler", icon: "undo-2", color: "#6D4C41", action: { type: "hotkey", keys: "ctrl+z" } },
    { name: "Rétablir", label: "Rétablir", icon: "redo-2", color: "#6D4C41", action: { type: "hotkey", keys: "ctrl+y" } },
    { name: "Enregistrer", label: "Enregistrer", icon: "save", color: "#2E7D32", action: { type: "hotkey", keys: "ctrl+s" } },
    { name: "Tout sélectionner", label: "Tout sélect.", icon: "text-select", color: "#455A64", action: { type: "hotkey", keys: "ctrl+a" } },
  ]},
  { cat: "Applications", items: [
    { name: "Lancer une application", label: "Appli", icon: "app-window", color: "#00897B", action: { type: "launch", target: "" }, kind: "program" },
    { name: "Ouvrir un site web", label: "Site web", icon: "globe", color: "#1E88E5", action: { type: "launch", target: "https://" }, kind: "url" },
    { name: "Ouvrir un dossier ou fichier", label: "Dossier", icon: "folder-open", color: "#F4B400", action: { type: "launch", target: "" }, kind: "path" },
    { name: "Explorateur", label: "Explorateur", icon: "folder-open", color: "#F4B400", action: { type: "launch", target: "explorer.exe" }, appIcon: true },
    { name: "Calculatrice", label: "Calculatrice", icon: "calculator", color: "#455A64", action: { type: "launch", target: "calc.exe" }, appIcon: true },
    { name: "Bloc-notes", label: "Bloc-notes", icon: "notebook-pen", color: "#0288D1", action: { type: "launch", target: "notepad.exe" }, appIcon: true },
  ]},
  { cat: "Multimédia", items: Object.entries(MEDIA).map(([command, [name, icon]]) => ({
    name, label: name.replace("Piste ", ""), icon, color: command === "mic_mute" || command === "mute" ? "#C62828" : "#2E7D32",
    action: { type: "media", command },
  }))},
  { cat: "Son et musique", items: [
    { name: "Changer de sortie audio", label: "", icon: "headphones", color: "#1F2229", widget: "audio_output", action: { type: "audio_output", devices: "" } },
    { name: "Musique en cours (pochette)", label: "", icon: "music", color: "#1F2229", widget: "media", widget_style: "cover", action: { type: "media", command: "play_pause" } },
    { name: "Musique en cours (titre)", label: "", icon: "music", color: "#1DB954", widget: "media", widget_style: "text", action: { type: "media", command: "play_pause" } },
  ]},
  { cat: "Minuteurs", items: [
    { name: "Chronomètre (appui long = 0)", label: "", icon: "timer", color: "#3DA9FC", widget: "timer", widget_style: "stopwatch",
      action: { type: "timer", op: "toggle" }, long: { type: "timer", op: "reset" } },
    { name: "Compte à rebours 5 min", label: "", icon: "timer", color: "#FF8A3D", widget: "timer", widget_style: "countdown", widget_target: "5:00",
      action: { type: "timer", op: "toggle" }, long: { type: "timer", op: "reset" } },
    { name: "Compte à rebours 25 min (pomodoro)", label: "", icon: "timer", color: "#EF4444", widget: "timer", widget_style: "countdown", widget_target: "25:00",
      action: { type: "timer", op: "toggle" }, long: { type: "timer", op: "reset" } },
  ]},
  { cat: "Fenêtres", items: [
    { name: "Fenêtre à gauche", label: "Gauche", icon: "arrow-left", color: "#3949AB", action: { type: "hotkey", keys: "win+left" } },
    { name: "Fenêtre à droite", label: "Droite", icon: "arrow-right", color: "#3949AB", action: { type: "hotkey", keys: "win+right" } },
    { name: "Agrandir la fenêtre", label: "Agrandir", icon: "maximize", color: "#3949AB", action: { type: "hotkey", keys: "win+up" } },
    { name: "Réduire la fenêtre", label: "Réduire", icon: "minimize", color: "#3949AB", action: { type: "hotkey", keys: "win+down" } },
    { name: "Vers l'autre écran", label: "Autre écran", icon: "screen-share", color: "#00838F", action: { type: "hotkey", keys: "win+shift+right" } },
    { name: "Bureau virtuel suivant", label: "Bureau +", icon: "layout-dashboard", color: "#6A1B9A", action: { type: "hotkey", keys: "ctrl+win+right" } },
    { name: "Bureau virtuel précédent", label: "Bureau −", icon: "layout-dashboard", color: "#6A1B9A", action: { type: "hotkey", keys: "ctrl+win+left" } },
    { name: "Nouveau bureau virtuel", label: "Nouveau bureau", icon: "plus", color: "#6A1B9A", action: { type: "hotkey", keys: "ctrl+win+d" } },
    { name: "Fermer le bureau virtuel", label: "Fermer bureau", icon: "x", color: "#6A1B9A", action: { type: "hotkey", keys: "ctrl+win+f4" } },
    { name: "Vue des tâches", label: "Tâches", icon: "layout-dashboard", color: "#0277BD", action: { type: "hotkey", keys: "win+tab" } },
  ]},
  { cat: "Soundboard", items: [
    { name: "Jouer un son", label: "Son", icon: "music", color: "#AD1457", action: { type: "sound", file: "", volume: 100, mode: "toggle" } },
    { name: "Arrêter tous les sons", label: "Stop sons", icon: "square", color: "#B71C1C", action: { type: "stop_sounds" } },
  ]},
  { cat: "Macros intelligentes", items: [
    { name: "Compteur (+1, appui long = 0)", label: "{compteur}", icon: "plus", color: "#00695C",
      action: { type: "var", name: "compteur", op: "add", value: "1" }, long: { type: "var", name: "compteur", op: "set", value: "0" } },
    { name: "Compteur −1", label: "−1", icon: "minus", color: "#00695C", action: { type: "var", name: "compteur", op: "add", value: "-1" } },
    { name: "Interrupteur (0 / 1)", label: "{interrupteur}", icon: "toggle-left", color: "#37474F", action: { type: "var", name: "interrupteur", op: "toggle", value: "" } },
    { name: "Condition (si… alors… sinon)", label: "Si…", icon: "git-branch", color: "#4527A0",
      action: { type: "if", left: "{app}", cmp: "contains", right: "", then: [{ type: "hotkey", keys: "" }], else: [] } },
  ]},
  { cat: "Connecteurs", items: [
    { name: "IA : corriger la sélection", label: "Corriger", icon: "spell-check", color: "#7C4DFF", action: { type: "ai", preset: "fix", output: "replace" } },
    { name: "IA : traduire en anglais", label: "→ EN", icon: "languages", color: "#3949AB", action: { type: "ai", preset: "translate_en", output: "replace" } },
    { name: "IA : traduire en français", label: "→ FR", icon: "languages", color: "#3949AB", action: { type: "ai", preset: "translate_fr", output: "replace" } },
    { name: "IA : style professionnel", label: "Pro", icon: "briefcase", color: "#455A64", action: { type: "ai", preset: "formal", output: "replace" } },
    { name: "IA : résumer (copié)", label: "Résumé", icon: "file-text", color: "#00838F", action: { type: "ai", preset: "summarize", output: "clipboard" } },
    { name: "IA : consigne perso", label: "IA", icon: "wand-sparkles", color: "#8E24AA", action: { type: "ai", preset: "custom", prompt: "", output: "replace" } },
    { name: "Requête web", label: "Web", icon: "globe", color: "#0277BD", action: { type: "http", method: "GET", url: "https://" } },
    { name: "Webhook Discord", label: "Discord", icon: "webhook", color: "#5865F2",
      action: { type: "http", method: "POST", url: "https://discord.com/api/webhooks/", body: '{"content": "Message envoyé depuis HubForge"}' } },
  ]},
  { cat: "Jeu", items: [
    { name: "Relecture instantanée NVIDIA", label: "Clip", icon: "circle-dot", color: "#76B900", action: { type: "hotkey", keys: "alt+f10" } },
    { name: "Enregistrer (NVIDIA)", label: "REC", icon: "video", color: "#76B900", action: { type: "hotkey", keys: "alt+f9" } },
    { name: "Capture d'écran (NVIDIA)", label: "Capture", icon: "camera", color: "#76B900", action: { type: "hotkey", keys: "alt+f1" } },
    { name: "Overlay NVIDIA", label: "NVIDIA", icon: "layout-grid", color: "#76B900", action: { type: "hotkey", keys: "alt+z" } },
    { name: "Xbox Game Bar", label: "Game Bar", icon: "gamepad-2", color: "#107C10", action: { type: "hotkey", keys: "win+g" } },
    { name: "Enregistrer les 30 dernières s (Game Bar)", label: "30 s", icon: "circle-dot", color: "#107C10", action: { type: "hotkey", keys: "win+alt+g" } },
    { name: "Enregistrer (Game Bar)", label: "REC", icon: "video", color: "#107C10", action: { type: "hotkey", keys: "win+alt+r" } },
    { name: "Capture de jeu (Game Bar)", label: "Capture", icon: "camera", color: "#107C10", action: { type: "hotkey", keys: "win+alt+printscreen" } },
    { name: "Overlay Steam", label: "Steam", icon: "layout-grid", color: "#1B2838", action: { type: "hotkey", keys: "shift+tab" } },
    { name: "Capture d'écran Steam", label: "Steam", icon: "camera", color: "#1B2838", action: { type: "hotkey", keys: "f12" } },
  ]},
  { cat: "Système", items: [
    { name: "Verrouiller le PC", label: "Verrouiller", icon: "lock", color: "#6A1B9A", action: { type: "lock" } },
    { name: "Capture d'écran", label: "Capture", icon: "scan", color: "#00897B", action: { type: "hotkey", keys: "win+shift+s" } },
    { name: "Afficher le bureau", label: "Bureau", icon: "monitor", color: "#0277BD", action: { type: "hotkey", keys: "win+d" } },
    { name: "Gestionnaire des tâches", label: "Gestion. tâches", icon: "activity", color: "#1565C0", action: { type: "hotkey", keys: "ctrl+shift+esc" } },
    { name: "Paramètres Windows", label: "Paramètres", icon: "settings", color: "#607D8B", action: { type: "hotkey", keys: "win+i" } },
    { name: "Fermer la fenêtre", label: "Fermer", icon: "x", color: "#B71C1C", action: { type: "hotkey", keys: "alt+f4" } },
  ]},
  { cat: "Touches à 2 états", items: [
    { name: "Micro ON / OFF", label: "Micro", icon: "mic", color: "#2E7D32", action: { type: "media", command: "mic_mute" },
      toggle: { label: "Micro coupé", icon: "builtin:mic-off", color: "#C62828", action: { type: "media", command: "mic_mute" } } },
    { name: "Son ON / OFF", label: "Son", icon: "volume-2", color: "#2E7D32", action: { type: "media", command: "mute" },
      toggle: { label: "Son coupé", icon: "builtin:volume-x", color: "#C62828", action: { type: "media", command: "mute" } } },
    { name: "Lecture / Pause (2 états)", label: "Lecture", icon: "play", color: "#2E7D32", action: { type: "media", command: "play_pause" },
      toggle: { label: "Pause", icon: "builtin:pause", color: "#EF6C00", action: { type: "media", command: "play_pause" } } },
    { name: "Bascule personnalisée", label: "État 1", icon: "toggle-left", color: "#37474F", action: { type: "none" },
      toggle: { label: "État 2", icon: "builtin:toggle-right", color: "#FF7A1A", action: { type: "none" } } },
  ]},
  { cat: "Animations", items: ANIMS.map(([id, name, icon]) => ({ name, label: "", icon, color: "#1F2229", anim: id, action: { type: "none" } })) },
  { cat: "Touches vivantes", items: [
    { name: "Horloge", label: "", icon: "clock", color: "#1F2229", widget: "clock", action: { type: "none" } },
    { name: "Date", label: "", icon: "calendar", color: "#1F2229", widget: "date", action: { type: "none" } },
    { name: "Processeur (CPU)", label: "", icon: "cpu", color: "#1F2229", widget: "cpu", action: { type: "hotkey", keys: "ctrl+shift+esc" } },
    { name: "Mémoire (RAM)", label: "", icon: "hard-drive", color: "#1F2229", widget: "ram", action: { type: "hotkey", keys: "ctrl+shift+esc" } },
    { name: "Volume général", label: "", icon: "volume-2", color: "#1F2229", widget: "volume", action: { type: "media", command: "mute" } },
    { name: "État du micro", label: "", icon: "mic", color: "#2E7D32", widget: "mic", action: { type: "media", command: "mic_mute" } },
    { name: "Carte graphique (GPU)", label: "", icon: "monitor", color: "#1F2229", widget: "gpu", action: { type: "hotkey", keys: "ctrl+shift+esc" } },
    { name: "Température du GPU", label: "", icon: "monitor", color: "#1F2229", widget: "gpu_temp", action: { type: "hotkey", keys: "ctrl+shift+esc" } },
    { name: "Mémoire vidéo (VRAM)", label: "", icon: "monitor", color: "#1F2229", widget: "vram", action: { type: "hotkey", keys: "ctrl+shift+esc" } },
    { name: "Volume d'une application", label: "", icon: "volume-2", color: "#1F2229", widget: "app_volume", widget_target: "discord.exe", action: { type: "app_mute", app: "discord.exe" } },
    { name: "Météo", label: "", icon: "sun", color: "#1F2229", widget: "weather", action: { type: "none" } },
    { name: "Réseau (débits)", label: "", icon: "wifi", color: "#1F2229", widget: "net", action: { type: "none" } },
    { name: "Disque C:", label: "", icon: "hard-drive", color: "#1F2229", widget: "disk", widget_target: "C", action: { type: "none" } },
    { name: "Météo des prochains jours", label: "", icon: "sun", color: "#1F2229", widget: "weather", widget_style: "forecast", action: { type: "none" } },
  ]},
  { cat: "HubForge", items: [
    { name: "Page suivante", label: "Page suivante", icon: "arrow-right", color: "#212121", action: { type: "page", to: "next" } },
    { name: "Page précédente", label: "Page précédente", icon: "arrow-left", color: "#212121", action: { type: "page", to: "previous" } },
    { name: "Aller à la page 1", label: "Accueil", icon: "house", color: "#212121", action: { type: "page", to: "1" } },
    { name: "Nouveau dossier (sous-page)", label: "Dossier", icon: "folder", color: "#F4B400", action: { type: "folder", to: "" }, newFolder: true },
    { name: "Retour (fermer le dossier)", label: "Retour", icon: "arrow-left", color: "#212121", action: { type: "back" } },
    { name: "Profil suivant", label: "Profil", icon: "users", color: "#5865F2", action: { type: "profile", to: "next" } },
    { name: "Ouvrir la configuration", label: "Config", icon: "file-cog", color: "#546E7A", action: { type: "open_config" } },
  ]},
];

const ACTION_TYPES = [
  ["none", "Aucune action"], ["hotkey", "Raccourci clavier"], ["hold", "Maintenir des touches"], ["text", "Taper un texte"],
  ["launch:program", "Lancer une application"], ["launch:url", "Ouvrir un site web"], ["launch:path", "Ouvrir un dossier ou fichier"],
  ["media", "Multimédia"], ["close_app", "Fermer une application"], ["app_mute", "Couper le son d'une application"], ["audio_output", "Changer de sortie audio"], ["timer", "Minuteur de la touche"], ["sound", "Jouer un son"], ["stop_sounds", "Arrêter tous les sons"], ["page", "Changer de page"], ["lock", "Verrouiller le PC"], ["open_config", "Ouvrir la configuration"],
  ["macro", "Macro (plusieurs actions à la suite)"], ["var", "Variable (compteur, interrupteur…)"], ["if", "Condition (si… alors… sinon)"],
  ["ai", "IA sur le texte sélectionné"], ["http", "Requête web"], ["service", "Action d'un addon"],
  ["folder", "Ouvrir un dossier"], ["back", "Retour (fermer le dossier)"],
  ["profile", "Changer de profil"],
];

const SWATCHES = ["#FF3B30", "#FF7A1A", "#FFB300", "#FFD60A", "#3DDC84", "#1DB954", "#00C8FF", "#1E88E5",
                  "#5865F2", "#8E24AA", "#E91E63", "#FFFFFF", "#607D8B", "#37474F", "#1F2229", "#000000"];

const WIDGETS = [["", "Aucune (image fixe)"], ["clock", "Horloge"], ["date", "Date"], ["weather", "Météo"], ["cpu", "Processeur (CPU)"],
  ["ram", "Mémoire (RAM)"], ["gpu", "Carte graphique (utilisation)"], ["gpu_temp", "Carte graphique (température)"], ["vram", "Mémoire vidéo (VRAM)"],
  ["volume", "Volume général"], ["app_volume", "Volume d'une application"], ["mic", "État du micro"],
  ["audio_output", "Sortie audio"], ["media", "Musique en cours"], ["timer", "Minuteur"], ["net", "Réseau"], ["disk", "Disque"]];

const WIDGET_STYLES = {
  clock: [["digital", "Numérique"], ["analog", "À aiguilles"], ["big", "Grands chiffres"]],
  date: [["classic", "Classique"], ["calendar", "Calendrier"]],
  cpu: [["bar", "Barre"], ["gauge", "Jauge"], ["graph", "Graphique"]],
  ram: [["bar", "Barre"], ["gauge", "Jauge"], ["graph", "Graphique"]],
  volume: [["bar", "Barre"], ["gauge", "Jauge"]],
  gpu: [["bar", "Barre"], ["gauge", "Jauge"], ["graph", "Graphique"]],
  gpu_temp: [["bar", "Barre"], ["gauge", "Jauge"], ["graph", "Graphique"]],
  vram: [["bar", "Barre"], ["gauge", "Jauge"], ["graph", "Graphique"]],
  app_volume: [["bar", "Barre"], ["gauge", "Jauge"]],
  weather: [["now", "Maintenant"], ["compact", "Compacte"], ["forecast", "Prochains jours"]],
  media: [["cover", "Pochette"], ["text", "Titre"]],
  timer: [["stopwatch", "Chronomètre"], ["countdown", "Compte à rebours"]],
  net: [["text", "Débits"], ["graph", "Graphique"]],
  disk: [["bar", "Barre"], ["gauge", "Jauge"]],
  mic: [["classic", "Icône et texte"], ["icon", "Grande icône"]],
};

const THEMES = [["graphite", "Graphite"], ["neon", "Néon"], ["plan", "Plan"], ["console", "Console"], ["mono", "Mono"], ["classic", "Classique"]];
const themeName = (id) => (THEMES.find(([v]) => v === id) || [0, id])[1];

/** Style effectif d'une touche : le sien, sinon celui de la page, sinon celui du profil. */
function effTheme(def, pageIdx) {
  const p = S.cfg.pages[pageIdx] || {};
  return (def && def.theme) || p.theme || S.cfg.general.theme || "graphite";
}
/** Ajoute le style effectif à une touche envoyée pour l'aperçu. */
function withTheme(d, pageIdx, src) {
  if (d) d.theme = effTheme(src, pageIdx);
  return d;
}
/** La touche affiche une image ou une animation à elle : le style ne s'applique pas. */
function ownVisual(st) {
  return !!(st && st.icon && !st.icon.startsWith("builtin:") && !st.icon.startsWith("addon:"));
}
/** Après un changement de style : tous les aperçus sont à refaire. */
function rethemed() {
  S.previews = {};
  changed({ immediate: true });
  refreshPreviews(S.page);
}
/** Pastilles de choix de style. `inherit` : libellé de l'option « comme au-dessus » (ou null). */
function themeChips(current, inherit, onPick) {
  const opts = inherit ? [["", inherit], ...THEMES] : THEMES;
  return el("div", { class: "chips" }, ...opts.map(([v, l]) => el("span", {
    class: "chip" + (v === (current || "") ? " on style-on" : " add"),
    onclick: () => onPick(v || null),
  }, l)));
}

const FADER_TARGETS = [["master", "Volume général"], ["mic", "Micro"], ["app", "Application(s)"], ["none", "Rien"]];
const ROLLER_MODES = [["volume", "Volume général"], ["mic", "Volume du micro"], ["app", "Volume d'applications"], ["scroll", "Défilement vertical"],
  ["hscroll", "Défilement horizontal"], ["zoom", "Zoom (Ctrl + molette)"], ["actions", "Actions (une par cran)"], ["none", "Rien"]];
const exeList = (v) => v.split(",").map((x) => x.trim().toLowerCase()).filter(Boolean).map((x) => /\.exe$/.test(x) ? x : x + ".exe").join(",");
const rollerKind = (m) => (m || "").startsWith("app:") ? "app" : (m || "none");
const CMPS = [["==", "est égal à", "="], ["!=", "est différent de", "≠"], [">", "est plus grand que", ">"], ["<", "est plus petit que", "<"],
  [">=", "est plus grand ou égal à", "≥"], ["<=", "est plus petit ou égal à", "≤"], ["contains", "contient", "contient"]];
const VAR_SPECIALS = [["{app}", "appli au premier plan"], ["{page}", "numéro de page"], ["{profile}", "profil"], ["{hour}", "heure"], ["{minute}", "minute"], ["{weekday}", "jour (1 = lundi)"]];

/** Actions contenues dans une action (étapes de macro, branches d'une condition). */
function childActions(a) {
  if (!a) return [];
  if (a.type === "macro") return a.steps || [];
  if (a.type === "if") return [...(a.then || []), ...(a["else"] || [])];
  return [];
}

/** Champ « variable où ranger le résultat » (requête web, IA). */
function varTargetField(a, upd, placeholder, label = "Variable") {
  const listId = "vars-" + Math.random().toString(36).slice(2);
  return field(label, el("input", { class: "input", value: a.save_to || "", list: listId, spellcheck: "false", placeholder,
    oninput: (e) => upd((x) => { x.save_to = e.target.value.trim(); if (!x.save_to) delete x.pick; }), onchange: () => renderInspector() }),
    el("datalist", { id: listId }, ...knownVars().map((n) => el("option", { value: n }))));
}

/** Réglages de l'IA (communs à tous les profils, jamais exportés). */
function aiSettingsBlock() {
  const box = el("div", { id: "ai-settings" });
  (async () => {
    let st;
    try { st = await invoke("get_ai_settings"); } catch (_) { st = null; }
    st = Object.assign({ provider: "", url: "", model: "", key: "" }, st || {});
    const draw = () => {
      box.innerHTML = "";
      const prov = AI_PROVIDERS.find(([v]) => v === st.provider) || AI_PROVIDERS[0];
      const save = async () => { try { await invoke("set_ai_settings", { settings: st }); } catch (e) { toast(String(e), true); } };
      box.append(field("Fournisseur", el("select", { class: "input", onchange: (e) => {
        const p = AI_PROVIDERS.find(([v]) => v === e.target.value);
        st.provider = p[0]; st.url = ""; st.model = p[3]; save(); draw();
      } }, ...AI_PROVIDERS.map(([v, l]) => el("option", { value: v, selected: v === st.provider }, l)))));
      if (!st.provider) {
        box.append(el("p", { class: "hint" }, "Les touches « IA » corrigent, traduisent ou résument le texte sélectionné. Choisis Ollama pour une IA gratuite qui tourne sur ton PC (à installer depuis ollama.com), ou un service en ligne avec ta clé API."));
        return;
      }
      box.append(field("Modèle", el("input", { class: "input mono", value: st.model, spellcheck: "false", placeholder: prov[3], onchange: (e) => { st.model = e.target.value.trim(); save(); } }),
        st.provider === "ollama" ? el("p", { class: "hint" }, "Le nom d'un modèle téléchargé dans Ollama (commande « ollama pull llama3.2 »).") : null));
      if (st.provider !== "ollama") box.append(field("Clé API", el("input", { class: "input mono", type: "password", value: st.key, spellcheck: "false", autocomplete: "off",
        onchange: (e) => { st.key = e.target.value.trim(); save(); } }),
        el("p", { class: "hint" }, "Gardée sur ce PC seulement : elle n'est jamais incluse quand tu exportes un profil.")));
      const adv = el("details", { class: "adv" }, el("summary", {}, "Adresse du serveur"),
        el("input", { class: "input mono", value: st.url, spellcheck: "false", placeholder: prov[2], onchange: (e) => { st.url = e.target.value.trim(); save(); } }),
        el("p", { class: "hint" }, st.provider === "openai" ? "À changer pour un service compatible OpenAI (Mistral, Groq, OpenRouter, LM Studio…)." : "Laisse vide pour l'adresse par défaut."));
      if (st.url) adv.open = true;
      box.append(adv);
      const res = el("p", { class: "hint" });
      box.append(el("div", { class: "row", style: "gap:6px;align-items:center" }, el("button", { class: "btn small", onclick: async () => {
        res.textContent = "Essai en cours…";
        try { res.textContent = "Réponse : " + await invoke("test_ai", { settings: st }); } catch (e) { res.textContent = "Erreur : " + e; }
      } }, "Essayer"), res));
    };
    draw();
  })();
  return box;
}

/** Noms des variables utilisées dans le profil et de celles qui ont une valeur. */
function knownVars() {
  const names = new Set(Object.keys((S.status && S.status.vars) || {}));
  const walk = (a) => {
    if (!a) return;
    if (a.type === "var" && a.name) names.add(a.name.trim().toLowerCase());
    if ((a.type === "http" || a.type === "ai") && a.save_to) names.add(a.save_to.trim().toLowerCase());
    childActions(a).forEach(walk);
  };
  for (const p of S.cfg.pages) for (const d of p.keys) { walk(d.action); if (d.toggle) walk(d.toggle.action); walk(d.long); walk(d.double); }
  return [...names].sort();
}
// Effets intégrés de la base. Le stroboscope intégré (3) clignote mal : il est
// remplacé par l'animation HubForge « Stroboscope ».
const BASE_EFFECTS = [[1, "Fixe"], [2, "Respiration"], [5, "Cycle de couleurs"], [6, "Aléatoire"], [7, "Vague"], [8, "Eau"], [9, "Arc-en-ciel"], [0, "Éteint"]];

/* Taille des modules sur la base (en unités), dans leur sens normal. */
const MODULE_SIZE = { keys15: [4, 3], faders5: [4, 2], rollers2: [4, 1] };
/* Disposition d'origine du kit (touches en haut, faders, molettes en bas). */
const DEFAULT_PLACEMENT = { keys15: { col: 0, row: 0, rot: 0 }, faders5: { col: 0, row: 3, rot: 0 }, rollers2: { col: 0, row: 5, rot: 0 } };
const MODULE_NAMES = { keys15: "15xKey", faders5: "5xFader", rollers2: "2xRoller" };
// Modèles d'animation (identiques à lights.rs) : identifiant, nom, nombre de couleurs.
const LIGHT_PRESETS = [["breathe", "Respiration", 1], ["rainbow", "Arc-en-ciel", 0], ["wave", "Vague", 2], ["comet", "Comète", 1],
  ["heartbeat", "Battement de cœur", 1], ["police", "Gyrophare", 2], ["fire", "Feu de camp", 0], ["fill", "Remplissage", 1], ["strobe", "Stroboscope", 1]];
const FADER_LED_MODES = [["fixed", "Couleur fixe"], ["position", "Couleur selon la position du fader"], ["status", "Lumière d'état (coupé, lancé)"], ["vu", "VU-mètre (niveau sonore)"], ["animation", "Animation"]];
const BASE_ROTS = [[0, "Verticale (comme dans le kit)"], [90, "Horizontale, tournée vers la droite"], [270, "Horizontale, tournée vers la gauche"], [180, "Verticale, à l'envers"]];

// ---------------------------------------------------------------------------
// État
// ---------------------------------------------------------------------------
const S = {
  cfg: null,
  page: 0,
  devicePage: 0,
  sel: { type: "none" },
  editState: 0,           // 0 = état normal, 1 = second état (touches à 2 états)
  previews: {},           // "page:key:state" -> { src, animated }
  status: { modules: [], faders: null, page: 0, pressed: [], toggled: [] },
  autostart: false,
  icons: [],              // icônes intégrées { name, svg }
  iconPickerOpen: false,
  saveTimer: null,
  capturing: false,
};

const $ = (s, root = document) => root.querySelector(s);
const el = (tag, attrs = {}, ...kids) => {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") e.className = v;
    else if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else if (k === "html") e.innerHTML = v;
    else if (v !== undefined && v !== null && v !== false) e.setAttribute(k, v === true ? "" : v);
  }
  for (const kid of kids.flat()) if (kid !== null && kid !== undefined && kid !== false) e.append(kid.nodeType ? kid : document.createTextNode(kid));
  return e;
};
const clone = (o) => JSON.parse(JSON.stringify(o));
/** Retire les champs « _… » qui ne servent qu'à l'interface. */
const clean = (o) => JSON.parse(JSON.stringify(o, (k, v) => (k.startsWith("_") ? undefined : v)));
const svgIcon = (name) => { const ic = S.icons.find((i) => i.name === name); return ic ? ic.svg : ""; };

// ---------------------------------------------------------------------------
// Démarrage
// ---------------------------------------------------------------------------
async function init() {
  const ini = await invoke("get_initial");
  S.cfg = normalize(ini.config);
  S.status = ini.status;
  S.autostart = ini.autostart;
  S.page = Math.min(ini.status.page || 0, S.cfg.pages.length - 1);
  S.devicePage = S.page;
  S.configPath = ini.config_path;
  S.profile = ini.profile;
  S.profiles = ini.profiles || [];
  S.gameBest = ini.game_best || 0;
  S.update = ini.update || {};
  S.version = ini.version;
  S.uiMode = ini.ui_mode || "advanced";
  S.lang = ini.lang === "en" ? "en" : "fr";
  setupLang(S.lang);
  S.autoProfiles = ini.auto_profiles !== false;
  $("#version").textContent = "v" + ini.version;
  S.icons = (await invoke("list_icons")) || [];
  S.layout = (await invoke("get_layout")) || { base_rot: 0, placements: {} };

  buildLibrary();
  renderAll();
  await refreshPreviews(S.page);

  listen("status", (e) => onStatus(e.payload));
  listen("autostart", (e) => { S.autostart = e.payload; if (S.sel.type === "settings") renderInspector(); });
  listen("update-state", (e) => { S.update = e.payload || {}; renderUpdatePill(); if (S.sel.type === "settings") renderInspector(); });
  listen("update-progress", (e) => updateProgress(e.payload));
  listen("doom-download", (e) => updateProgress(e.payload));
  listen("addon-installed", async (e) => {
    const r = e.payload;
    if (r && r.Err) { toast(`Addon non installé : ${r.Err}`, true); return; }
    const id = r && (r.Ok || r);
    await afterAddonInstall(id);
  });

  $("#btn-settings").onclick = () => (S.sel.type === "settings" ? deselect() : select({ type: "settings" }));
  $("#btn-addons").onclick = () => (S.sel.type === "addons" ? deselect() : select({ type: "addons" }));
  $("#btn-undo").onclick = undo;
  $("#btn-redo").onclick = redo;
  resetHistory();
  listen("profile-imported", (e) => {
    const r = e.payload;
    if (r && r.Err) { toast(`Profil non importé : ${r.Err}`, true); return; }
    afterProfileImport(r && (r.Ok || r));
  });
  await loadAddons();
  buildLibrary();
  setInterval(checkAddons, 3000);
  setupForgeDrop();
  $("#profile-btn").onclick = (e) => { e.stopPropagation(); toggleProfileMenu(); };
  document.addEventListener("click", (e) => { if (!e.target.closest(".profile-wrap")) $("#profile-menu").classList.add("hidden"); });
  renderProfileButton();
  setupEasterEgg();
  setupDoom();
  setupTrashDrop();
  renderUpdatePill();
  $("#update-pill").onclick = () => showUpdateDialog();
  $("#dialog-close").onclick = closeDialog;
  $("#dialog").addEventListener("click", (e) => { if (e.target.id === "dialog" && !S.updating) closeDialog(); });
  let reopen = null;
  try { reopen = JSON.parse(sessionStorage.getItem("hf-reopen") || "null"); sessionStorage.removeItem("hf-reopen"); } catch (_) {}
  if (reopen) {
    if (reopen.setSec) S.setSec = reopen.setSec;
    if (reopen.wizard) setTimeout(() => openWizard(), 300);
    else if (reopen.sel && reopen.sel.type === "settings") setTimeout(() => select({ type: "settings" }), 50);
  } else if (ini.show_wizard) setTimeout(() => openWizard(), 300);
  else if (ini.whats_new) setTimeout(() => showWhatsNew(ini.version, ini.whats_new), 400);
  $("#lib-search").oninput = () => buildLibrary();
  document.addEventListener("keydown", onGlobalKey, true);
  fitDevice();
  new ResizeObserver(fitDevice).observe($(".stage"));
  // Aperçu des touches vivantes dans l'interface.
  setInterval(refreshLivePreviews, 2000);
}

function normalize(cfg) {
  cfg.general = Object.assign({ brightness: 80, led_brightness: 100, base_color: "#00C8FF", press_feedback: true, gif_max_fps: 28, base_effect: 1, auto_pages: true,
    long_press_ms: 500, double_press_ms: 300, sound_volume: 80, sound_device: "", sound_device2: "", fader_pickup: true, weather_city: "", theme: "graphite", sleep_after: 0, sleep_locked: true, vars_files: false }, cfg.general || {});
  cfg.pages = cfg.pages && cfg.pages.length ? cfg.pages : [{ name: "Accueil", keys: [] }];
  cfg.faders = cfg.faders || [];
  while (cfg.faders.length < 5) cfg.faders.push({ target: "none", color: "#FFFFFF" });
  cfg.rollers = Object.assign({ right: "volume", left: "scroll", volume_step: 2, scroll_step: 3, invert_right: false, invert_left: false }, cfg.rollers || {});
  cfg.lights = Object.assign({ base_mode: "hardware", base_animation: "", fps: 15, animations: [] }, cfg.lights || {});
  for (const k of ["faders_mode", "faders_animation", "gradient", "game"]) delete cfg.lights[k];
  return cfg;
}

/** Réduit la base dessinée pour qu'elle tienne toujours dans la fenêtre. */
function fitDevice() {
  const stage = $(".stage"), dev = $("#device");
  dev.style.zoom = 1;
  // Place prise par les onglets de pages et la phrase d'aide (plus les marges).
  const extra = [".pages", ".stage-hint"].reduce((h, sel) => { const n = $(sel); return h + (n ? n.offsetHeight + 28 : 0); }, 0);
  const availW = stage.clientWidth - 48, availH = stage.clientHeight - extra - 56;
  const z = Math.min(1.2, availW / dev.offsetWidth, availH / dev.offsetHeight);
  dev.style.zoom = Math.max(0.4, z).toFixed(3);
}

// ---------------------------------------------------------------------------
// Enregistrement automatique
// ---------------------------------------------------------------------------
function changed(opts = {}) {
  recordHistory();
  setSaveState("saving", "Enregistrement…");
  clearTimeout(S.saveTimer);
  S.saveTimer = setTimeout(save, opts.immediate ? 0 : 350);
}

async function save() {
  try {
    await invoke("apply_config", { config: clean(S.cfg) });
    setSaveState("ok", "Enregistré");
  } catch (e) {
    setSaveState("error", "Erreur");
    toast(String(e), true);
  }
}

function setSaveState(kind, label) {
  const s = $("#save-state");
  s.className = "save-state" + (kind === "saving" ? " saving" : kind === "error" ? " error" : "");
  $(".label", s).textContent = label;
}

// ---------------------------------------------------------------------------
// Rendu général
// ---------------------------------------------------------------------------
function renderAll() {
  renderPages();
  renderModulesChips();
  renderDevice();
  renderInspector();
  renderBanner();
}

function currentPage() { return S.cfg.pages[S.page]; }
function keyDef(k, page = S.page) { return S.cfg.pages[page].keys.find((d) => d.key === k); }

/** Définition d'un état d'une touche (0 = normal, 1 = second état). */
function stateDef(def, n) {
  if (n === 1 && def.toggle) {
    const t = def.toggle;
    return { key: def.key, label: t.label || "", icon: t.icon, color: t.color, text_color: t.text_color, action: t.action || { type: "none" } };
  }
  const d = clone(def);
  delete d.toggle;
  return d;
}

function renderPages() {
  const nav = $("#pages");
  nav.innerHTML = "";
  S.cfg.pages.forEach((p, i) => {
    const apps = p.apps || [];
    const tab = el("div", { class: "page-tab" + (i === S.page ? " active" : "") + (p.folder ? " is-folder" : ""), title: "Double-clic pour renommer" },
      i === S.devicePage ? el("span", { class: "on-device", title: "Page affichée sur le MasterHUB" }) : null,
      p.folder ? el("span", { class: "auto folder-mark", title: "Dossier", html: svgIcon("folder") }) : null,
      p.name ? el("span", { translate: "no" }, p.name) : el("span", {}, `Page ${i + 1}`),
      apps.length ? el("span", { class: "auto", title: "S'affiche toute seule avec : " + apps.join(", "), html: svgIcon("app-window") }) : null,
      i === S.page ? el("span", { class: "opts", title: "Réglages de la page", html: svgIcon("settings"),
        onclick: (e) => { e.stopPropagation(); select({ type: "page" }); } }) : null,
      S.cfg.pages.length > 1 ? el("span", { class: "close", title: "Supprimer la page", onclick: (e) => { e.stopPropagation(); deletePage(i); } }, "×") : null,
    );
    tab.onclick = () => goToPage(i);
    tab.ondblclick = () => renamePage(i);
    tab.draggable = true;
    tab.ondragstart = (e) => { e.dataTransfer.setData("text/hubforge-page", String(i)); e.dataTransfer.effectAllowed = "move"; tab.classList.add("dragging"); };
    tab.ondragend = () => tab.classList.remove("dragging");
    tab.ondragover = (e) => {
      if (!e.dataTransfer.types.includes("text/hubforge-page")) return;
      e.preventDefault();
      const r = tab.getBoundingClientRect();
      const left = e.clientX < r.left + r.width / 2;
      tab.classList.toggle("drag-over-left", left);
      tab.classList.toggle("drag-over-right", !left);
    };
    tab.ondragleave = () => tab.classList.remove("drag-over-left", "drag-over-right");
    tab.ondrop = (e) => {
      const from = Number(e.dataTransfer.getData("text/hubforge-page"));
      if (Number.isNaN(from) || !e.dataTransfer.types.includes("text/hubforge-page")) return;
      e.preventDefault();
      const left = tab.classList.contains("drag-over-left");
      tab.classList.remove("drag-over-left", "drag-over-right");
      let to = left ? i : i + 1;
      if (from < to) to--;
      movePage(from, to);
    };
    nav.append(tab);
  });
  nav.append(el("button", { class: "page-add", title: "Ajouter une page", onclick: addPage }, "+"));
}

async function goToPage(i) {
  S.page = i;
  invoke("show_page", { index: i });
  S.devicePage = i;
  renderPages();
  renderKeys();
  renderInspector();
  await refreshPreviews(i);
}

async function addPage() {
  const name = await prompt_("Nom de la nouvelle page :", `Page ${S.cfg.pages.length + 1}`);
  if (name === null) return;
  S.cfg.pages.push({ name: name || `Page ${S.cfg.pages.length + 1}`, keys: [] });
  changed({ immediate: true });
  goToPage(S.cfg.pages.length - 1);
}

async function renamePage(i) {
  const name = await prompt_("Nouveau nom de la page :", S.cfg.pages[i].name);
  if (name === null) return;
  S.cfg.pages[i].name = name;
  renderPages();
  changed();
}

async function deletePage(i) {
  if (!(await confirm_(`Supprimer la page « ${S.cfg.pages[i].name} » et ses touches ?`))) return;
  S.cfg.pages.splice(i, 1);
  fixPageRefs(i);
  S.previews = {};
  S.page = Math.min(S.page, S.cfg.pages.length - 1);
  changed({ immediate: true });
  goToPage(S.page);
}

function renderModulesChips() {
  const names = { keys15: "15 touches", faders5: "5 faders", rollers2: "2 molettes", base: "base" };
  const mods = S.status.modules.map((m) => names[m.kind]).filter(Boolean);
  const box = $("#modules");
  const on = mods.length > 0;
  box.className = "status-pill" + (on ? " on" : "");
  box.title = on ? "Branché : " + mods.join(", ") + ". Clique pour le diagnostic." : "Aucun module détecté. Clique pour le diagnostic.";
  box.innerHTML = "";
  box.append(el("span", { class: "dot" }), on ? "MasterHUB connecté" : "MasterHUB non détecté");
  box.onclick = showDiagnostic;
}

function renderBanner() {
  const b = $("#banner");
  if (S.status.config_error) { b.textContent = "⚠ " + S.status.config_error; b.classList.remove("hidden"); }
  else b.classList.add("hidden");
}
