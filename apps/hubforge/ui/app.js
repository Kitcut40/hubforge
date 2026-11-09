"use strict";
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

const LIBRARY = [
  { cat: "Clavier", items: [
    { name: "Raccourci clavier", label: "Raccourci", icon: "keyboard", color: "#3949AB", action: { type: "hotkey", keys: "" } },
    { name: "Taper un texte", label: "Texte", icon: "type", color: "#EF6C00", action: { type: "text", text: "" } },
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
  { cat: "Touches vivantes", items: [
    { name: "Horloge", label: "", icon: "clock", color: "#1F2229", widget: "clock", action: { type: "none" } },
    { name: "Date", label: "", icon: "calendar", color: "#1F2229", widget: "date", action: { type: "none" } },
    { name: "Processeur (CPU)", label: "", icon: "cpu", color: "#1F2229", widget: "cpu", action: { type: "hotkey", keys: "ctrl+shift+esc" } },
    { name: "Mémoire (RAM)", label: "", icon: "hard-drive", color: "#1F2229", widget: "ram", action: { type: "hotkey", keys: "ctrl+shift+esc" } },
    { name: "Volume général", label: "", icon: "volume-2", color: "#1F2229", widget: "volume", action: { type: "media", command: "mute" } },
    { name: "État du micro", label: "", icon: "mic", color: "#2E7D32", widget: "mic", action: { type: "media", command: "mic_mute" } },
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
  ["none", "Aucune action"], ["hotkey", "Raccourci clavier"], ["text", "Taper un texte"],
  ["launch:program", "Lancer une application"], ["launch:url", "Ouvrir un site web"], ["launch:path", "Ouvrir un dossier ou fichier"],
  ["media", "Multimédia"], ["page", "Changer de page"], ["lock", "Verrouiller le PC"], ["open_config", "Ouvrir la configuration"],
  ["macro", "Macro (plusieurs actions à la suite)"],
  ["folder", "Ouvrir un dossier"], ["back", "Retour (fermer le dossier)"],
  ["profile", "Changer de profil"],
];

const SWATCHES = ["#FF3B30", "#FF7A1A", "#FFB300", "#FFD60A", "#3DDC84", "#1DB954", "#00C8FF", "#1E88E5",
                  "#5865F2", "#8E24AA", "#E91E63", "#FFFFFF", "#607D8B", "#37474F", "#1F2229", "#000000"];

const WIDGETS = [["", "Aucune (image fixe)"], ["clock", "Horloge"], ["date", "Date"], ["cpu", "Processeur (CPU)"],
  ["ram", "Mémoire (RAM)"], ["volume", "Volume général"], ["mic", "État du micro"]];

const WIDGET_STYLES = {
  clock: [["digital", "Numérique"], ["analog", "À aiguilles"], ["big", "Grands chiffres"]],
  date: [["classic", "Classique"], ["calendar", "Calendrier"]],
  cpu: [["bar", "Barre"], ["gauge", "Jauge"], ["graph", "Graphique"]],
  ram: [["bar", "Barre"], ["gauge", "Jauge"], ["graph", "Graphique"]],
  volume: [["bar", "Barre"], ["gauge", "Jauge"]],
  mic: [["classic", "Icône et texte"], ["icon", "Grande icône"]],
};

const FADER_TARGETS = [["master", "Volume général"], ["mic", "Micro"], ["app", "Application(s)"], ["none", "Rien"]];
const ROLLER_MODES = [["volume", "Volume général"], ["scroll", "Défilement vertical"], ["hscroll", "Défilement horizontal"], ["zoom", "Zoom (Ctrl + molette)"], ["none", "Rien"]];
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
  sel: { type: "settings" },
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

  $("#btn-settings").onclick = () => select({ type: "settings" });
  $("#btn-addons").onclick = () => select({ type: "addons" });
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
  if (ini.whats_new) setTimeout(() => showWhatsNew(ini.version, ini.whats_new), 400);
  $("#lib-search").oninput = (e) => filterLibrary(e.target.value);
  document.addEventListener("keydown", onGlobalKey, true);
  fitDevice();
  new ResizeObserver(fitDevice).observe($(".stage"));
  // Aperçu des touches vivantes dans l'interface.
  setInterval(refreshLivePreviews, 2000);
}

function normalize(cfg) {
  cfg.general = Object.assign({ brightness: 80, led_brightness: 100, base_color: "#00C8FF", press_feedback: true, gif_max_fps: 28, base_effect: 1, auto_pages: true,
    long_press_ms: 500, double_press_ms: 300 }, cfg.general || {});
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
  const availW = stage.clientWidth - 40, availH = stage.clientHeight - 80;
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
      el("span", {}, p.name || `Page ${i + 1}`),
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
  const kinds = [["keys15", "Touches"], ["faders5", "Faders"], ["rollers2", "Molettes"], ["base", "Base"]];
  const on = new Set(S.status.modules.map((m) => m.kind));
  const box = $("#modules");
  box.innerHTML = "";
  for (const [k, label] of kinds) box.append(el("span", { class: "chip" + (on.has(k) ? " on" : "") }, el("span", { class: "led" }), label));
}

function renderBanner() {
  const b = $("#banner");
  if (S.status.config_error) { b.textContent = "⚠ " + S.status.config_error; b.classList.remove("hidden"); }
  else b.classList.add("hidden");
}

// ---------------------------------------------------------------------------
// La base et ses modules, à leur vraie place
// ---------------------------------------------------------------------------
function unit() { return parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--u")) || 124; }

/** Boîte occupée sur la grille (colonnes, lignes) selon la rotation. */
function footprint(kind, p) {
  const [w, h] = MODULE_SIZE[kind];
  return p.rot === 90 || p.rot === 270 ? [h, w] : [w, h];
}

function overlaps(a, b) {
  return a.col < b.col + b.w && b.col < a.col + a.w && a.row < b.row + b.h && b.row < a.row + a.h;
}

/** Modules à dessiner : position détectée, sinon position d'origine du kit. */
function layout() {
  const placed = [];
  const order = ["keys15", "faders5", "rollers2"];
  // D'abord les modules dont la position est connue, puis les autres.
  const items = order.map((kind) => {
    const m = S.status.modules.find((x) => x.kind === kind);
    return { kind, connected: !!m, placement: m && m.placement };
  }).sort((a, b) => (b.placement ? 1 : 0) - (a.placement ? 1 : 0));
  for (const it of items) {
    let p = it.placement || DEFAULT_PLACEMENT[it.kind];
    let [w, h] = footprint(it.kind, p);
    let box = { col: p.col, row: p.row, w, h };
    if (placed.some((o) => overlaps(o.box, box))) {
      // Emplacement par défaut pris : on cherche une place libre.
      let found = null;
      for (let row = 0; row <= 6 - MODULE_SIZE[it.kind][1] && !found; row++) {
        const cand = { col: 0, row, w: 4, h: MODULE_SIZE[it.kind][1] };
        if (!placed.some((o) => overlaps(o.box, cand))) found = cand;
      }
      if (!found) { if (!it.connected) continue; found = box; }
      box = found; p = { col: found.col, row: found.row, rot: 0 };
    }
    placed.push({ ...it, p, box });
  }
  return placed;
}

function renderDevice() {
  const base = $("#base");
  base.innerHTML = "";
  const U = unit();
  const rot = (S.layout && S.layout.base_rot) || 0;
  const dev = $("#device");
  dev.classList.remove("rot-90", "rot-180", "rot-270");
  if (rot) dev.classList.add("rot-" + rot);
  base.classList.toggle("selected", S.sel.type === "base");
  base.onclick = (e) => { if (!e.target.closest(".module")) select({ type: "base" }); };
  base.append(el("div", { class: "base-glow", id: "base-glow" }));
  paintBaseGlow();
  for (const m of layout()) {
    const slot = el("div", { class: "slot", style: `left:${14 + m.box.col * U}px;top:${14 + m.box.row * U}px;width:${m.box.w * U}px;height:${m.box.h * U}px` });
    const mod = moduleElement(m.kind, !m.connected);
    // Les images des touches restent droites (HubForge les tourne aussi sur le MasterHUB).
    if (m.kind === "keys15") document.documentElement.style.setProperty("--key-rot", `${-((m.p.rot || 0) + rot)}deg`);
    mod.style.transform = `translate(-50%, -50%)` + (m.p.rot ? ` rotate(${m.p.rot}deg)` : "");
    slot.append(mod);
    base.append(slot);
  }
  updateFaderLive();
}

/** Lueur de la base : couleur envoyée en ce moment par HubForge, sinon effet intégré. */
function paintBaseGlow() {
  const glow = $("#base-glow");
  if (!glow) return;
  const live = S.status.leds && S.status.leds.base;
  const g = S.cfg.general;
  const c = live || g.base_color || "#00C8FF";
  const fx = live ? 1 : g.base_effect;
  glow.className = "base-glow" + (live ? "" : ` fx-${fx}`);
  glow.style.display = fx === 0 ? "none" : "";
  glow.style.boxShadow = `0 0 0 2px ${c}${live ? "AA" : "66"}, 0 0 26px ${c}${live ? "99" : "66"}`;
}

/** Réglage du fader `i` sur la page affichée : celui de la page, sinon le réglage commun. */
function pageFader(i, page = currentPage()) {
  return (page.faders || []).find((f) => f.index === i + 1) || null;
}
function effFader(i) { return pageFader(i) || S.cfg.faders[i]; }
const DEFAULT_GRADIENT = ["0:#00E676", "60:#FFD000", "100:#FF1744"];
function faderGradient(slot) { return slot.gradient && slot.gradient.length ? slot.gradient : DEFAULT_GRADIENT; }

/** Couleur actuelle de la LED d'un fader. */
function faderLedColor(i) {
  const live = S.status.leds && S.status.leds.faders && S.status.leds.faders[i];
  if (live) return live;
  const slot = effFader(i);
  if (slot.led === "position" && S.status.faders) return gradientAt(faderGradient(slot), S.status.faders[i]);
  if (slot.led === "vu") return gradientAt(faderGradient(slot), 0);
  return slot.color || "#FFFFFF";
}

/** Couleur du dégradé à une position (même calcul que lights.rs). */
function gradientAt(stops, v) {
  const pts = stops.map((x) => { const [p, c] = x.split(":"); return [Number(p), c]; }).filter(([p, c]) => !isNaN(p) && /^#[0-9a-f]{6}$/i.test(c || "")).sort((a, b) => a[0] - b[0]);
  if (!pts.length) return "#FFFFFF";
  const rgb = (c) => [1, 3, 5].map((k) => parseInt(c.slice(k, k + 2), 16));
  if (v <= pts[0][0]) return pts[0][1];
  for (let j = 1; j < pts.length; j++) {
    if (v <= pts[j][0]) {
      const k = (v - pts[j - 1][0]) / Math.max(0.001, pts[j][0] - pts[j - 1][0]);
      const a = rgb(pts[j - 1][1]), b = rgb(pts[j][1]);
      return "#" + a.map((x, n) => Math.round(x + (b[n] - x) * k).toString(16).padStart(2, "0")).join("").toUpperCase();
    }
  }
  return pts[pts.length - 1][1];
}

function updateLedLive() {
  paintBaseGlow();
  document.querySelectorAll("#faders .fader-led").forEach((led, i) => {
    const c = faderLedColor(i);
    led.style.background = c;
    led.style.boxShadow = `0 0 8px ${c}`;
  });
}

function moduleElement(kind, ghost) {
  const selected = (kind === "faders5" && S.sel.type === "fader") || (kind === "rollers2" && S.sel.type === "rollers");
  const mod = el("div", { class: `module m-${kind.replace(/\d+$/, "")}` + (ghost ? " ghost" : "") + (selected ? " selected" : ""), "data-kind": kind });
  if (kind === "keys15") {
    mod.append(el("div", { class: "keys", id: "keys" }));
    mod.append(el("div", { class: "module-label" }, MODULE_NAMES[kind]));
    fillKeys(mod.querySelector(".keys"));
  } else if (kind === "faders5") {
    mod.append(el("div", { class: "faders", id: "faders" }, ...faderElements()));
    mod.append(el("div", { class: "module-label" }, MODULE_NAMES[kind]));
    mod.onclick = () => select({ type: "fader", index: S.sel.type === "fader" ? S.sel.index : 0 });
  } else if (kind === "rollers2") {
    mod.append(el("div", { class: "rollers", id: "rollers" }, ...rollerElements()));
    mod.append(el("div", { class: "module-label" }, MODULE_NAMES[kind]));
    mod.onclick = () => select({ type: "rollers" });
  }
  return mod;
}

// ---------------------------------------------------------------------------
// Touches
// ---------------------------------------------------------------------------
function renderKeys() {
  const box = $("#keys");
  if (box) fillKeys(box);
}

function fillKeys(box) {
  box.innerHTML = "";
  for (let k = 1; k <= 15; k++) box.append(keyElement(k));
}

function shownState(k) {
  const def = keyDef(k);
  if (!def || !def.toggle) return 0;
  return S.devicePage === S.page && S.status.toggled && S.status.toggled[k - 1] ? 1 : 0;
}

function keyElement(k) {
  const def = keyDef(k);
  const st = shownState(k);
  const prev = S.previews[`${S.page}:${k}:${st}`] || S.previews[`${S.page}:${k}:0`];
  const isSel = S.sel.type === "key" && S.sel.key === k;
  const pressed = S.devicePage === S.page && S.status.pressed && S.status.pressed[k - 1];
  const node = el("div", {
    class: "key" + (def ? "" : " empty") + (isSel ? " selected" : "") + (pressed ? " pressed" : ""),
    "data-key": k, draggable: def ? "true" : "false", title: def ? (def.label || `Touche ${k}`) : "Touche vide",
  },
    def && prev ? el("img", { src: prev.src, alt: "" }) : null,
    el("span", { class: "num" }, String(k)),
    def && def.toggle ? el("span", { class: "badge" }, "2 états")
      : def && def.widget ? el("span", { class: "badge" }, "Live")
      : def && def.action && def.action.type === "macro" ? el("span", { class: "badge" }, "Macro")
      : def && prev && prev.animated ? el("span", { class: "badge" }, "GIF") : null,
  );
  node.onclick = (e) => { e.stopPropagation(); S.editState = 0; select({ type: "key", key: k }); };
  node.ondragstart = (e) => {
    e.dataTransfer.setData("text/hubforge-key", String(k));
    e.dataTransfer.effectAllowed = "move";
    node.classList.add("dragging");
    S.draggingKey = k;
    document.body.classList.add("dragging-key");
  };
  node.ondragend = () => {
    node.classList.remove("dragging");
    S.draggingKey = null;
    document.body.classList.remove("dragging-key", "over-trash");
  };
  node.ondragover = (e) => { e.preventDefault(); node.classList.add("drop"); };
  node.ondragleave = () => node.classList.remove("drop");
  node.ondrop = (e) => { e.preventDefault(); node.classList.remove("drop"); onDropOnKey(k, e.dataTransfer); };
  return node;
}

function updateKeyNode(k) {
  const old = document.querySelector(`#keys .key[data-key="${k}"]`);
  if (old) old.replaceWith(keyElement(k));
}

async function refreshPreviews(page, onlyKey) {
  const keys = onlyKey ? [onlyKey] : Array.from({ length: 15 }, (_, i) => i + 1);
  const req = [];
  for (const k of keys) {
    const def = keyDef(k, page);
    req.push([k, 0, def ? clean(stateDef(def, 0)) : null]);
    if (def && def.toggle) req.push([k, 1, clean(stateDef(def, 1))]);
  }
  try {
    const res = await invoke("render_keys", { keys: req.map((r) => r[2]) });
    for (const k of keys) { delete S.previews[`${page}:${k}:0`]; delete S.previews[`${page}:${k}:1`]; }
    req.forEach(([k, st], i) => { if (res[i]) S.previews[`${page}:${k}:${st}`] = res[i]; });
  } catch (e) { toast("Aperçu impossible : " + e, true); }
  if (page === S.page) {
    keys.forEach(updateKeyNode);
    if (S.sel.type === "key" && keys.includes(S.sel.key)) renderKeyPreviewInInspector();
  }
}

let liveBusy = false;
async function refreshLivePreviews() {
  if (liveBusy || document.hidden) return;
  const keys = currentPage().keys.filter((d) => d.widget).map((d) => d.key);
  if (!keys.length) return;
  liveBusy = true;
  try {
    const page = S.page;
    const res = await invoke("render_keys", { keys: keys.map((k) => clean(stateDef(keyDef(k, page), 0))) });
    if (page !== S.page) return;
    keys.forEach((k, i) => {
      if (!res[i]) return;
      S.previews[`${page}:${k}:0`] = res[i];
      const img = document.querySelector(`#keys .key[data-key="${k}"] img`);
      if (img && shownState(k) === 0) img.src = res[i].src; else updateKeyNode(k);
    });
    if (S.sel.type === "key" && keys.includes(S.sel.key)) renderKeyPreviewInInspector();
  } catch (_) { /* aperçu seulement */ } finally { liveBusy = false; }
}

function setKeyDef(k, def) {
  const keys = currentPage().keys;
  const i = keys.findIndex((d) => d.key === k);
  if (def) { def.key = k; if (i >= 0) keys[i] = def; else keys.push(def); }
  else if (i >= 0) keys.splice(i, 1);
  keys.sort((a, b) => a.key - b.key);
}

let previewTimer = null;
function keyChanged(k, opts = {}) {
  changed();
  clearTimeout(previewTimer);
  previewTimer = setTimeout(() => refreshPreviews(S.page, k), opts.immediate ? 0 : 200);
}

/** Textes génériques qu'on peut remplacer par le vrai nom de l'application. */
const GENERIC_LABELS = ["", "appli", "nouvelle touche", "dossier", "fichier", "site web", "raccourci", "état 1", "état 2"];

function labelIsGeneric(st) {
  const l = (st.label || "").trim().toLowerCase();
  return GENERIC_LABELS.includes(l) || (st._autoName && st.label === st._autoName);
}

/** Nom affichable d'un site : « https://www.github.com/x » → « Github ». */
function siteName(url) {
  try {
    const host = new URL(url).hostname.replace(/^www\./, "");
    const first = host.split(".")[0];
    return first ? first.charAt(0).toUpperCase() + first.slice(1) : "";
  } catch (_) { return ""; }
}

/** Récupère l'icône et le nom d'une application et les applique à l'état donné.
    opts.quiet : pas de message d'erreur ; opts.keepLabel / opts.keepIcon : ne touche pas au texte / à l'image ;
    opts.forceLabel : remplace le texte même s'il a été personnalisé. */
async function applyAppIcon(k, st, target, opts = {}) {
  try {
    const info = await invoke("app_info", { target });
    if (!info) return false;
    let done = false;
    if (info.icon && !opts.keepIcon) {
      st.icon = info.icon;
      if (!st.color || st.color === "#00897B") st.color = "#1F2229";
      done = true;
    }
    if (info.name && !opts.keepLabel && (opts.forceLabel || labelIsGeneric(st))) {
      st.label = info.name;
      st._autoName = info.name;
      done = true;
    }
    if (!done) { if (!opts.quiet) toast("Rien trouvé pour cette application.", true); return false; }
    keyChanged(k, { immediate: true });
    if (S.sel.type === "key" && S.sel.key === k) renderInspector();
    return true;
  } catch (e) {
    if (!opts.quiet) toast("Icône introuvable : " + e, true);
    return false;
  }
}

/** Met le nom du site comme texte de la touche si le texte est générique. */
function applySiteName(k, st) {
  const n = siteName((st.action || {}).target || "");
  if (!n || !labelIsGeneric(st)) return;
  st.label = n;
  st._autoName = n;
  keyChanged(k);
  const inp = document.querySelector("#label-input");
  if (inp) inp.value = n;
}

async function onDropOnKey(k, dt) {
  const fromKey = dt.getData("text/hubforge-key");
  const lib = dt.getData("text/hubforge-action");
  if (fromKey) {
    const a = Number(fromKey);
    if (a === k) return;
    const target = keyDef(k);
    if (target && target.action && target.action.type === "folder" && keyDef(a)) { moveKeyIntoFolder(a, target); return; }
    const da = keyDef(a) ? clone(keyDef(a)) : null;
    const db = keyDef(k) ? clone(keyDef(k)) : null;
    setKeyDef(k, da); setKeyDef(a, db);
    for (const st of [0, 1]) {
      const pa = S.previews[`${S.page}:${a}:${st}`], pb = S.previews[`${S.page}:${k}:${st}`];
      if (pa) S.previews[`${S.page}:${k}:${st}`] = pa; else delete S.previews[`${S.page}:${k}:${st}`];
      if (pb) S.previews[`${S.page}:${a}:${st}`] = pb; else delete S.previews[`${S.page}:${a}:${st}`];
    }
    updateKeyNode(a); updateKeyNode(k);
    S.editState = 0;
    select({ type: "key", key: k });
    changed();
    return;
  }
  if (lib) {
    const item = JSON.parse(lib);
    const def = item.addonKey ? Object.assign({ key: k }, clone(item.addonKey))
      : { key: k, label: item.label, color: item.color, icon: "builtin:" + item.icon, action: clone(item.action) };
    if (item.newFolder) {
      const idx = createFolder(nextFolderName());
      def.label = S.cfg.pages[idx].name;
      def.action = { type: "folder", to: String(idx + 1) };
    }
    if (item.toggle) def.toggle = clone(item.toggle);
    if (item.widget) { def.widget = item.widget; delete def.icon; }
    setKeyDef(k, def);
    S.editState = 0;
    select({ type: "key", key: k });
    keyChanged(k, { immediate: true });
    if (item.appIcon) applyAppIcon(k, keyDef(k), item.action.target, { quiet: true, keepLabel: true });
    return;
  }
  if (dt.files && dt.files.length) {
    const f = dt.files[0];
    if (/\.forge$/i.test(f.name)) { installForgeFile(f); return; }
    if (!/\.(png|jpe?g|gif|webp|bmp)$/i.test(f.name)) { toast("Dépose une image (png, jpg, gif, webp).", true); return; }
    const data = await fileToBase64(f);
    try {
      const rel = await invoke("import_image_data", { name: f.name, data });
      const def = keyDef(k) ? clone(keyDef(k)) : { key: k, label: "", action: { type: "none" } };
      def.icon = rel;
      setKeyDef(k, def);
      S.editState = 0;
      select({ type: "key", key: k });
      keyChanged(k, { immediate: true });
    } catch (e) { toast(String(e), true); }
  }
}

function fileToBase64(file) {
  return new Promise((res, rej) => {
    const r = new FileReader();
    r.onload = () => res(String(r.result).split(",")[1]);
    r.onerror = rej;
    r.readAsDataURL(file);
  });
}

// ---------------------------------------------------------------------------
// Faders et molettes
// ---------------------------------------------------------------------------
function faderName(slot) {
  const t = (slot.target || "none").trim();
  if (t === "master") return "Général";
  if (t === "mic") return "Micro";
  if (t.startsWith("app:")) {
    const first = t.slice(4).split(",")[0].trim().replace(/\.exe$/i, "");
    return first ? first.charAt(0).toUpperCase() + first.slice(1) : "Appli";
  }
  return "—";
}

function faderElements() {
  return S.cfg.faders.slice(0, 5).map((_, i) => {
    const slot = effFader(i);
    const own = !!pageFader(i);
    const f = el("div", { class: "fader" + (own ? " page-own" : "") + (S.sel.type === "fader" && S.sel.index === i ? " selected" : ""), "data-fader": i, title: own ? "Réglage propre à cette page" : "" },
      el("div", { class: "fader-value" }, "–"),
      el("div", { class: "fader-track" },
        el("div", { class: "fader-led", style: `background:${faderLedColor(i)};box-shadow:0 0 8px ${faderLedColor(i)}` }),
        el("div", { class: "fader-thumb", style: "bottom:0%" })),
      el("div", { class: "fader-name", title: slot.target }, faderName(slot)),
    );
    f.onclick = (e) => { e.stopPropagation(); select({ type: "fader", index: i }); };
    return f;
  });
}

function updateFaderLive() {
  document.querySelectorAll("#faders .fader").forEach((f, i) => {
    const v = S.status.faders ? S.status.faders[i] : null;
    $(".fader-value", f).textContent = v === null ? "–" : v + " %";
    $(".fader-thumb", f).style.bottom = (v === null ? 0 : v) + "%";
  });
  if (S.sel.type === "fader") {
    const live = $("#fader-live");
    if (live && S.status.faders) live.textContent = S.status.faders[S.sel.index] + " %";
  }
}

/** Action de la molette `side` sur la page affichée (page, sinon réglage commun). */
function effRoller(side) {
  const own = currentPage()["roller_" + side];
  return own != null ? own : S.cfg.rollers[side];
}

function rollerElements() {
  const mode = (m) => (ROLLER_MODES.find(([v]) => v === m) || [0, "—"])[1].split(" (")[0];
  return [["left", "Gauche"], ["right", "Droite"]].map(([side, label]) => {
    const own = currentPage()["roller_" + side] != null;
    return el("div", { class: "roller" + (own ? " page-own" : ""), title: `Molette ${label.toLowerCase()} : ${mode(effRoller(side))}${own ? " (propre à cette page)" : ""}` },
      el("div", { class: "roller-drum" }),
      el("div", { class: "roller-name" }, `${label} · ${mode(effRoller(side))}`));
  });
}

// ---------------------------------------------------------------------------
// Sélection et inspecteur
// ---------------------------------------------------------------------------
function select(sel) {
  if (S.previewing && sel.type !== "lights") { invoke("preview_animation", { animation: null }); S.previewing = null; }
  S.sel = sel;
  S.capturing = false;
  S.iconPickerOpen = false;
  renderDevice();
  $("#btn-settings").classList.toggle("active", sel.type === "settings");
  $("#btn-addons").classList.toggle("active", sel.type === "addons");
  renderInspector();
}

function renderInspector() {
  const box = $("#inspector");
  box.innerHTML = "";
  if (S.sel.type === "key") box.append(...keyInspector(S.sel.key));
  else if (S.sel.type === "fader") box.append(...faderInspector(S.sel.index));
  else if (S.sel.type === "rollers") box.append(...rollersInspector());
  else if (S.sel.type === "page") box.append(...pageInspector());
  else if (S.sel.type === "base") box.append(...baseInspector());
  else if (S.sel.type === "addons") box.append(...addonsInspector());
  else if (S.sel.type === "lights") box.append(...lightsInspector());
  else box.append(...settingsInspector());
}

function title(text, sub) {
  return el("div", { class: "insp-title" }, el("h3", {}, text), sub ? el("span", { class: "sub" }, sub) : null);
}

function field(label, ...content) {
  return el("div", { class: "field" }, el("label", {}, label), ...content);
}

function colorField(label, value, onChange, allowEmpty) {
  const input = el("input", { type: "color", class: "color", value: value || "#000000" });
  const hex = el("input", { class: "input grow", value: value || "", placeholder: allowEmpty ? "aucune" : "#RRVVBB", spellcheck: "false" });
  const apply = (v) => {
    v = (v || "").trim();
    if (v && !/^#[0-9a-fA-F]{6}$/.test(v)) return;
    if (v) input.value = v;
    hex.value = v;
    onChange(v ? v.toUpperCase() : null);
  };
  input.oninput = () => apply(input.value);
  hex.onchange = () => apply(hex.value);
  const sw = el("div", { class: "swatches" }, ...SWATCHES.map((c) => el("span", { class: "swatch", style: `background:${c}`, title: c, onclick: () => apply(c) })));
  return field(label, el("div", { class: "row" }, input, hex), sw);
}

// --- Touche ---
function keyInspector(k) {
  const def = keyDef(k);
  const pageName = currentPage().name || `Page ${S.page + 1}`;
  const out = [title(`Touche ${k}`, pageName)];
  out.push(el("div", { class: "insp-preview", id: "insp-preview" }));
  setTimeout(renderKeyPreviewInInspector);
  if (!def) {
    out.push(el("p", { class: "empty-state" }, "Touche vide.", el("br"), "Glisse une action depuis la liste de gauche, ou :"));
    if (S.clipboard) out.push(el("button", { class: "btn", style: "width:100%;margin-bottom:8px", onclick: () => pasteKey(k) }, "Coller la touche copiée"));
    out.push(el("button", { class: "btn primary", style: "width:100%", onclick: () => {
      setKeyDef(k, { key: k, label: "Nouvelle touche", color: "#3949AB", icon: "builtin:star", action: { type: "none" } });
      keyChanged(k, { immediate: true }); renderInspector();
    } }, "Créer une touche"));
    return out;
  }

  // Touche à 2 états
  out.push(switchRow("Touche à 2 états (change à chaque appui)", !!def.toggle, (v) => {
    if (v) def.toggle = { label: (def.label || "") + " (2)", icon: def.icon, color: "#FF7A1A", text_color: def.text_color, action: clone(def.action || { type: "none" }) };
    else { delete def.toggle; S.editState = 0; }
    keyChanged(k, { immediate: true });
    renderInspector();
  }));
  if (def.toggle) {
    out.push(el("div", { class: "tabs" },
      el("button", { class: S.editState === 0 ? "on" : "", onclick: () => { S.editState = 0; renderInspector(); } }, "État 1 (normal)"),
      el("button", { class: S.editState === 1 ? "on" : "", onclick: () => { S.editState = 1; renderInspector(); } }, "État 2")));
  } else S.editState = 0;

  const st = S.editState === 1 && def.toggle ? def.toggle : def;
  const upd = (fn, opts) => { fn(st); keyChanged(k, opts); };

  const live = S.editState === 0 && def.widget;
  if (S.editState === 0) {
    const wsel = el("select", { class: "input" }, ...WIDGETS.map(([v, l]) => el("option", { value: v, selected: v === (def.widget || "") }, l)));
    wsel.onchange = () => {
      delete def.widget_style;
      if (wsel.value) { def.widget = wsel.value; if (!def.color) def.color = "#1F2229"; } else delete def.widget;
      keyChanged(k, { immediate: true });
      renderInspector();
    };
    out.push(field("Touche vivante", wsel, live ? el("p", { class: "hint" }, "L'image se met à jour toute seule. L'action ci-dessous reste active quand on appuie.") : null));
    if (live) {
      const styles = WIDGET_STYLES[def.widget] || [];
      const cur = def.widget_style || (styles[0] || [""])[0];
      const row = el("div", { class: "chips" }, ...styles.map(([v, l]) => el("span", {
        class: "chip" + (v === cur ? " on style-on" : " add"),
        onclick: () => { if (v === styles[0][0]) delete def.widget_style; else def.widget_style = v; keyChanged(k, { immediate: true }); renderInspector(); },
      }, l)));
      out.push(field("Style", row));
    }
  }
  out.push(field(live ? "Titre (facultatif)" : "Texte affiché", el("input", { id: "label-input", class: "input", value: st.label || "",
    placeholder: live ? "(titre par défaut)" : "(aucun)", oninput: (e) => upd((d) => { d.label = e.target.value; delete d._autoName; }) })));
  if (!live) out.push(iconField(k, st, upd));
  out.push(colorField("Couleur de fond", st.color, (v) => upd((d) => { if (v) d.color = v; else delete d.color; })));
  out.push(colorField("Couleur du texte et de l'icône", st.text_color || "#FFFFFF", (v) => upd((d) => { if (v) d.text_color = v; else delete d.text_color; })));

  out.push(el("div", { class: "sep" }));
  if (def.toggle) out.push(el("p", { class: "hint", style: "margin:-6px 0 12px" }, S.editState === 0
    ? "Action lancée quand la touche est dans l'état 1 ; elle passe ensuite à l'état 2."
    : "Action lancée quand la touche est dans l'état 2 ; elle revient ensuite à l'état 1."));
  out.push(...actionEditor(st, k, () => keyChanged(k)));
  if (S.editState === 0) out.push(...gestureEditors(def, k));
  out.push(el("div", { class: "sep" }));
  out.push(el("div", { class: "row", style: "margin-bottom:8px" },
    el("button", { class: "btn grow", title: "Ctrl+C", onclick: () => copyKey(k) }, "Copier"),
    el("button", { class: "btn grow", title: "Ctrl+V", disabled: !S.clipboard, onclick: () => pasteKey(k) }, "Coller ici")));
  out.push(el("button", { class: "btn danger", style: "width:100%", onclick: async () => {
    if (!(await confirm_(`Vider la touche ${k} ?`))) return;
    setKeyDef(k, null); delete S.previews[`${S.page}:${k}:0`]; delete S.previews[`${S.page}:${k}:1`]; updateKeyNode(k); changed(); renderInspector();
  } }, "Vider la touche"));
  return out;
}

/** Appui long et double appui d'une touche (actions supplémentaires). */
function gestureEditors(def, k) {
  const out = [el("div", { class: "sep" }), el("h2", {}, "Appui long et double appui")];
  for (const [prop, label, hint] of [
    ["long", "Appui long", `Garder la touche enfoncée ${(S.cfg.general.long_press_ms / 1000).toLocaleString("fr-FR")} s lance une autre action.`],
    ["double", "Double appui", `Deux appuis rapides lancent une autre action. L'appui simple attend alors ${S.cfg.general.double_press_ms} ms pour être sûr que ce n'en est pas un.`],
  ]) {
    out.push(switchRow(label, !!def[prop], (v) => {
      if (v) def[prop] = { type: "none" }; else delete def[prop];
      keyChanged(k); renderInspector();
    }));
    if (def[prop]) {
      const box = { action: def[prop] };
      out.push(el("div", { class: "gesture-box" }, ...actionEditor(box, k, () => { def[prop] = box.action; keyChanged(k); }, { extra: label === "Appui long" ? "Quand je garde la touche enfoncée" : "Quand j'appuie deux fois" }),
        el("p", { class: "hint" }, hint)));
    }
  }
  return out;
}

function renderKeyPreviewInInspector() {
  const box = $("#insp-preview");
  if (!box || S.sel.type !== "key") return;
  const k = S.sel.key;
  const def = keyDef(k);
  box.innerHTML = "";
  const one = (st, label) => {
    const prev = S.previews[`${S.page}:${k}:${st}`];
    return el("div", {},
      el("div", { class: "key" + (def ? "" : " empty") }, def && prev ? el("img", { src: prev.src }) : null,
        def && prev && prev.animated ? el("span", { class: "badge" }, "GIF") : null),
      label ? el("div", { class: "state-label" }, label) : null);
  };
  if (def && def.toggle) box.append(one(0, "État 1"), one(1, "État 2"));
  else box.append(one(0));
}

function iconField(k, st, upd) {
  const icon = st.icon || "";
  const isBuiltin = icon.startsWith("builtin:");
  const current = el("div", { class: "ic" });
  if (isBuiltin) current.innerHTML = svgIcon(icon.slice(8));
  else if (icon) current.append(el("span", { style: "font-size:10px;color:var(--muted);padding:2px;text-align:center" }, "image"));
  const name = !icon ? "Aucune" : isBuiltin ? icon.slice(8) : icon.split(/[\\/]/).pop();

  const action = st.action || {};
  const appTarget = action.type === "launch" && launchKind(action.target || "") !== "url" && (action.target || "").trim();
  const buttons = el("div", { class: "row", style: "flex-wrap:wrap;margin-top:8px" },
    el("button", { class: "btn small" + (S.iconPickerOpen ? " active" : ""), onclick: () => { S.iconPickerOpen = !S.iconPickerOpen; renderInspector(); } }, "Icônes…"),
    el("button", { class: "btn small", onclick: async () => {
      const p = await invoke("pick_file", { kind: "image" });
      if (!p) return;
      try { const rel = await invoke("import_image", { path: p }); upd((d) => { d.icon = rel; }, { immediate: true }); renderInspector(); }
      catch (e) { toast(String(e), true); }
    } }, "Image / GIF…"),
    appTarget ? el("button", { class: "btn small", title: "Reprend l'icône et le nom de l'application ou du dossier", onclick: () => applyAppIcon(k, st, appTarget, { forceLabel: true }) }, "Icône et nom de l'appli") : null,
    icon ? el("button", { class: "btn small", onclick: () => { upd((d) => { delete d.icon; }, { immediate: true }); renderInspector(); } }, "Retirer") : null);

  const out = field("Icône ou image", el("div", { class: "icon-current" }, current, el("span", { style: "color:var(--muted)" }, name)), buttons);
  if (S.iconPickerOpen) {
    const grid = el("div", { class: "icon-grid" });
    const fill = (q) => {
      grid.innerHTML = "";
      for (const ic of S.icons.filter((i) => !q || i.name.includes(q))) {
        const cell = el("div", { class: "icon-cell" + (icon === "builtin:" + ic.name ? " on" : ""), title: ic.name, html: ic.svg });
        cell.onclick = () => { upd((d) => { d.icon = "builtin:" + ic.name; }, { immediate: true }); renderInspector(); };
        grid.append(cell);
      }
    };
    const search = el("input", { class: "input", placeholder: "Rechercher une icône (en anglais : play, mic, folder…)", oninput: (e) => fill(e.target.value.trim().toLowerCase()) });
    fill("");
    out.append(el("div", { class: "icon-picker" }, search, grid));
  } else {
    out.append(el("p", { class: "hint" }, "Astuce : glisse aussi une image ou un GIF depuis l'Explorateur sur la touche."));
  }
  return out;
}

function launchKind(target) {
  if (/^https?:\/\//i.test(target)) return "url";
  if (!target || /\.(exe|lnk|bat|cmd|msc)$/i.test(target)) return "program";
  return "path";
}

function actionTypeId(a) {
  if (a.type === "launch") return a._kind === "path" ? "launch:path" : "launch:" + launchKind(a.target || "");
  return a.type;
}

/** Valeurs par défaut quand on choisit un type d'action. */
const ACTION_DEFAULTS = {
  none: { type: "none" }, hotkey: { type: "hotkey", keys: "" }, text: { type: "text", text: "" },
  "launch:program": { type: "launch", target: "" }, "launch:url": { type: "launch", target: "https://" },
  "launch:path": { type: "launch", target: "", _kind: "path" }, media: { type: "media", command: "play_pause" },
  page: { type: "page", to: "next" }, lock: { type: "lock" }, open_config: { type: "open_config" },
  macro: { type: "macro", steps: [{ type: "hotkey", keys: "" }] }, wait: { type: "wait", ms: 500 },
  folder: { type: "folder", to: "" }, back: { type: "back" },
  profile: { type: "profile", to: "next" }, game: { type: "game" },
};

/** Résumé d'une étape de macro (affiché dans son en-tête). */
function stepSummary(a) {
  switch (a.type) {
    case "hotkey": return a.keys ? `Raccourci ${a.keys}` : "Raccourci clavier";
    case "text": return a.text ? `Taper « ${a.text.length > 24 ? a.text.slice(0, 24) + "…" : a.text} »` : "Taper un texte";
    case "launch": return a.target ? `Ouvrir ${a.target.split(/[\\/]/).pop()}` : "Ouvrir…";
    case "media": return (MEDIA[a.command] || [a.command])[0];
    case "page": return a.to === "next" ? "Page suivante" : a.to === "previous" ? "Page précédente" : `Page ${a.to}`;
    case "lock": return "Verrouiller le PC";
    case "open_config": return "Ouvrir la configuration";
    case "wait": return `Pause ${(a.ms / 1000).toLocaleString("fr-FR")} s`;
    case "folder": { const p = S.cfg.pages[Number(a.to) - 1]; return `Ouvrir le dossier ${p ? "« " + p.name + " »" : a.to}`; }
    case "back": return "Retour (fermer le dossier)";
    case "profile": return a.to === "next" ? "Profil suivant" : a.to === "previous" ? "Profil précédent" : `Profil « ${a.to} »`;
    case "game": return "Mini-jeu";
    default: return "Aucune action";
  }
}

/** Éditeur d'action pour `st.action` ; `onChange` est appelé à chaque modification.
    opts.nested : étape d'une macro (pas de titre, pas d'icône ni de nom automatiques). */
function actionEditor(st, k, onChange, opts = {}) {
  if (!st.action) st.action = { type: "none" };
  const a = st.action;
  const nested = !!opts.nested || !!opts.extra;
  const upd = (fn) => { fn(st.action); onChange(); };
  let types = opts.nested
    ? [...ACTION_TYPES.filter(([v]) => v !== "macro"), ["wait", "Attendre (pause)"]]
    : ACTION_TYPES;
  if (S.gameBest > 0 || a.type === "game") types = [...types, ["game", "🎮 Mini-jeu tape-taupe"]];
  const typeSel = el("select", { class: "input" }, ...types.map(([v, l]) => el("option", { value: v, selected: v === actionTypeId(a) }, l)));
  typeSel.onchange = () => {
    st.action = clone(ACTION_DEFAULTS[typeSel.value]);
    if (st.action.type === "folder") {
      // Premier dossier existant, sinon on en crée un.
      let idx = S.cfg.pages.findIndex((p) => p.folder);
      if (idx < 0) idx = createFolder(nextFolderName());
      st.action.to = String(idx + 1);
    }
    onChange();
    renderInspector();
  };
  const out = opts.extra ? [field(opts.extra, typeSel)] : nested ? [field("Type", typeSel)] : [el("h2", {}, "Action"), field("Quand j'appuie sur la touche", typeSel)];
  const t = actionTypeId(a);
  const autoIcon = (p) => {
    if (nested) return;
    // Icône et nom automatiques (l'icône seulement si la touche n'a pas déjà sa propre image).
    const ownImage = st.icon && !st.icon.startsWith("builtin:") && !st.icon.startsWith("icons/app-");
    applyAppIcon(k, st, p, { quiet: true, keepIcon: ownImage }).then((ok) => { if (!ok) renderInspector(); });
  };

  if (t === "hotkey") {
    const cap = el("input", { class: "input capture", value: a.keys || "", placeholder: "Clique ici puis fais la combinaison", spellcheck: "false" });
    cap.onfocus = () => { S.capturing = true; cap.classList.add("listening"); cap.placeholder = "Appuie sur la combinaison…"; };
    cap.onblur = () => { S.capturing = false; cap.classList.remove("listening"); cap.placeholder = "Clique ici puis fais la combinaison"; };
    cap.onkeydown = (e) => {
      if (e.key === "Tab") return;
      e.preventDefault();
      const combo = comboFromEvent(e);
      if (combo) { cap.value = combo; upd((x) => { x.keys = combo; }); }
    };
    cap.onchange = () => upd((x) => { x.keys = cap.value.trim().toLowerCase(); });
    const extra = el("select", { class: "input" }, el("option", { value: "" }, "Ajouter une touche spéciale…"),
      ...["win", "ctrl", "alt", "shift", "esc", "enter", "tab", "space", "delete", "printscreen", "home", "end", "pageup", "pagedown",
        "up", "down", "left", "right", ...Array.from({ length: 12 }, (_, i) => "f" + (i + 1))].map((x) => el("option", { value: x }, x)));
    extra.onchange = () => {
      if (!extra.value) return;
      const v = cap.value ? cap.value + "+" + extra.value : extra.value;
      cap.value = v; upd((x) => { x.keys = v; }); extra.value = "";
    };
    out.push(field("Combinaison", cap, el("div", { style: "height:8px" }), extra,
      nested ? null : el("p", { class: "hint" }, "Ex. ctrl+shift+s · win+d · alt+f4. Les combinaisons avec la touche Windows s'ajoutent avec la liste.")));
  } else if (t === "text") {
    out.push(field("Texte à taper", el("textarea", { class: "input", oninput: (e) => upd((x) => { x.text = e.target.value; }) }, a.text || "")));
  } else if (t === "launch:program") {
    const inp = el("input", { class: "input grow", value: a.target || "", placeholder: "ex. notepad.exe ou C:\\…\\appli.exe", spellcheck: "false", oninput: (e) => upd((x) => { x.target = e.target.value; }) });
    out.push(field("Application", el("div", { class: "row" }, inp, el("button", { class: "btn", onclick: async () => {
      const p = await invoke("pick_file", { kind: "program" });
      if (!p) return;
      inp.value = p; upd((x) => { x.target = p; });
      autoIcon(p);
      if (nested) renderInspector();
    } }, "Parcourir…"))));
    out.push(field("Arguments (facultatif)", el("input", { class: "input", value: (a.args || []).join(" "), placeholder: "ex. --nouvelle-fenetre",
      oninput: (e) => upd((x) => { const v = e.target.value.trim(); if (v) x.args = v.split(/\s+/); else delete x.args; }) })));
  } else if (t === "launch:url") {
    out.push(field("Adresse du site", el("input", { class: "input", value: a.target || "https://", spellcheck: "false",
      oninput: (e) => upd((x) => { x.target = e.target.value; }), onchange: () => { if (!nested) applySiteName(k, st); } }),
      nested ? null : el("p", { class: "hint" }, "Le nom du site devient le texte de la touche s'il n'a pas été personnalisé.")));
  } else if (t === "launch:path") {
    const inp = el("input", { class: "input", value: a.target || "", placeholder: "C:\\Users\\…", spellcheck: "false", oninput: (e) => upd((x) => { x.target = e.target.value; }) });
    const choose = async (p) => {
      if (!p) return;
      inp.value = p; upd((x) => { x.target = p; });
      autoIcon(p);
      if (nested) renderInspector();
    };
    out.push(field("Dossier ou fichier", inp, el("div", { class: "row", style: "margin-top:8px" },
      el("button", { class: "btn grow", onclick: async () => choose(await invoke("pick_folder")) }, "Choisir un dossier…"),
      el("button", { class: "btn grow", onclick: async () => choose(await invoke("pick_file", { kind: "any" })) }, "Choisir un fichier…"))));
  } else if (t === "media") {
    out.push(field("Commande", el("select", { class: "input", onchange: (e) => upd((x) => { x.command = e.target.value; }) },
      ...Object.entries(MEDIA).map(([v, [l]]) => el("option", { value: v, selected: v === a.command }, l)))));
  } else if (t === "page") {
    const opts2 = [["next", "Page suivante"], ["previous", "Page précédente"], ...S.cfg.pages.map((p, i) => [String(i + 1), `Page ${i + 1} — ${p.name}`])];
    out.push(field("Aller à", el("select", { class: "input", onchange: (e) => upd((x) => { x.to = e.target.value; }) },
      ...opts2.map(([v, l]) => el("option", { value: v, selected: v === a.to }, l)))));
  } else if (t === "wait") {
    const sec = el("span", { class: "val" }, (a.ms / 1000).toLocaleString("fr-FR") + " s");
    const inp = el("input", { class: "input grow", type: "number", min: 0, max: 60000, step: 50, value: a.ms });
    inp.oninput = () => {
      const v = Math.max(0, Math.min(60000, Math.round(Number(inp.value) || 0)));
      upd((x) => { x.ms = v; });
      sec.textContent = (v / 1000).toLocaleString("fr-FR") + " s";
    };
    const quick = el("div", { class: "chips" }, ...[100, 300, 500, 1000, 2000].map((v) =>
      el("span", { class: "chip add", onclick: () => { inp.value = v; inp.oninput(); } }, v < 1000 ? `${v} ms` : `${v / 1000} s`)));
    out.push(field("Durée (millisecondes)", el("div", { class: "row" }, inp, sec), quick));
  } else if (t === "macro") {
    out.push(...macroEditor(a, k, onChange));
  } else if (t === "folder") {
    const cur = Number(a.to) - 1;
    const folders = S.cfg.pages.map((p, i) => [i, p]).filter(([i, p]) => p.folder || i === cur);
    const sel = el("select", { class: "input" },
      ...folders.map(([i, p]) => el("option", { value: String(i + 1), selected: i === cur }, `${p.name || "Page " + (i + 1)}${p.folder ? "" : " (page normale)"}`)),
      el("option", { value: "__new" }, "+ Nouveau dossier…"));
    sel.onchange = async () => {
      if (sel.value === "__new") {
        const name = await prompt_("Nom du nouveau dossier :", nextFolderName());
        if (name === null) { renderInspector(); return; }
        const idx = createFolder(name || nextFolderName());
        upd((x) => { x.to = String(idx + 1); });
        if (!nested && labelIsGeneric(st)) { st.label = S.cfg.pages[idx].name; keyChanged(k); }
      } else upd((x) => { x.to = sel.value; });
      renderInspector();
    };
    out.push(field("Dossier", sel));
    if (!nested && S.cfg.pages[cur]) {
      out.push(el("button", { class: "btn", style: "width:100%", onclick: () => goToPage(cur) }, "Ouvrir le dossier pour le remplir"));
      out.push(el("p", { class: "hint" }, "Un dossier est une page à part, absente de « page suivante / précédente ». Sa touche Retour ramène à la page d'où on l'a ouvert."));
    }
  } else if (t === "profile") {
    const opts3 = [["next", "Profil suivant"], ["previous", "Profil précédent"], ...(S.profiles || []).map((n) => [n, n])];
    if (a.to && !opts3.some(([v]) => v === a.to)) opts3.push([a.to, `${a.to} (introuvable)`]);
    out.push(field("Passer sur", el("select", { class: "input", onchange: (e) => upd((x) => { x.to = e.target.value; }) },
      ...opts3.map(([v, l]) => el("option", { value: v, selected: v === a.to }, l))),
      nested ? null : el("p", { class: "hint" }, "Change tout : touches, faders, molettes et LED. Mets cette touche dans chaque profil pour pouvoir revenir.")));
  } else if (t === "game") {
    if (!nested) out.push(el("p", { class: "hint" }, "Tape les taupes avant qu'elles repartent ! Dorée = +3, bombe = −3. Record : " + (S.gameBest || 0) + " points."));
  } else if (t === "back") {
    if (!nested) out.push(el("p", { class: "hint" }, "Revient à la page d'où le dossier a été ouvert."));
  }
  return out;
}

/** Nom libre pour un nouveau dossier : « Dossier », « Dossier 2 »… */
function nextFolderName() {
  const names = new Set(S.cfg.pages.map((p) => p.name));
  if (!names.has("Dossier")) return "Dossier";
  let n = 2;
  while (names.has(`Dossier ${n}`)) n++;
  return `Dossier ${n}`;
}

/** Crée une page dossier (avec sa touche Retour) et retourne son index. */
function createFolder(name) {
  S.cfg.pages.push({
    name, folder: true, close_after: true,
    keys: [{ key: 1, label: "Retour", icon: "builtin:arrow-left", color: "#212121", action: { type: "back" } }],
  });
  changed({ immediate: true });
  renderPages();
  return S.cfg.pages.length - 1;
}

/** Après suppression de la page `removed` (index), corrige les numéros de
    page des actions « page » et « dossier » (les liens vers elle sont retirés). */
function fixPageRefs(removed) {
  const fix = (a) => {
    if (!a) return a;
    if (a.type === "macro") { a.steps = (a.steps || []).map(fix).filter(Boolean); return a; }
    if ((a.type === "page" || a.type === "folder") && /^\d+$/.test(a.to || "")) {
      const n = Number(a.to);
      if (n === removed + 1) return a.type === "folder" ? { type: "none" } : { type: "page", to: "1" };
      if (n > removed + 1) a.to = String(n - 1);
    }
    return a;
  };
  for (const p of S.cfg.pages) for (const d of p.keys) {
    d.action = fix(d.action) || { type: "none" };
    if (d.toggle) d.toggle.action = fix(d.toggle.action) || { type: "none" };
  }
}

/** Liste des étapes d'une macro, chacune avec son propre éditeur. */
function macroEditor(a, k, onChange) {
  if (!Array.isArray(a.steps)) a.steps = [];
  const steps = a.steps;
  const changedAndRedraw = () => { onChange(); renderInspector(); };
  const list = el("div", { class: "steps" });
  steps.forEach((step, i) => {
    // L'éditeur d'action travaille sur un « porteur » qui pointe vers l'étape.
    const holder = { get action() { return steps[i]; }, set action(v) { steps[i] = v; } };
    const move = (d) => { const j = i + d; if (j < 0 || j >= steps.length) return; [steps[i], steps[j]] = [steps[j], steps[i]]; changedAndRedraw(); };
    const card = el("div", { class: "step" + (step.type === "wait" ? " wait" : "") },
      el("div", { class: "step-head" },
        el("span", { class: "step-num" }, String(i + 1)),
        el("span", { class: "step-sum" }, stepSummary(step)),
        el("button", { class: "step-btn", title: "Monter", disabled: i === 0, onclick: () => move(-1) }, "↑"),
        el("button", { class: "step-btn", title: "Descendre", disabled: i === steps.length - 1, onclick: () => move(1) }, "↓"),
        el("button", { class: "step-btn danger", title: "Supprimer l'étape", onclick: () => { steps.splice(i, 1); changedAndRedraw(); } }, "×")),
      el("div", { class: "step-body" }, ...actionEditor(holder, k, () => {
        onChange();
        const sum = card.querySelector(".step-sum");
        if (sum) sum.textContent = stepSummary(steps[i]);
      }, { nested: true })));
    list.append(card);
  });
  return [
    el("h2", { style: "margin-top:4px" }, `Étapes (${steps.length})`),
    el("p", { class: "hint", style: "margin:-4px 0 10px" }, "Exécutées dans l'ordre, de haut en bas. Ajoute des pauses pour laisser le temps à une application de s'ouvrir."),
    steps.length ? list : el("p", { class: "hint" }, "Aucune étape pour l'instant."),
    el("div", { class: "row", style: "margin-top:10px" },
      el("button", { class: "btn grow", disabled: steps.length >= 100, onclick: () => { steps.push({ type: "hotkey", keys: "" }); changedAndRedraw(); } }, "+ Étape"),
      el("button", { class: "btn grow", disabled: steps.length >= 100, onclick: () => { steps.push({ type: "wait", ms: 500 }); changedAndRedraw(); } }, "+ Pause")),
  ];
}

function comboFromEvent(e) {
  const parts = [];
  if (e.ctrlKey) parts.push("ctrl");
  if (e.altKey) parts.push("alt");
  if (e.shiftKey) parts.push("shift");
  if (e.metaKey) parts.push("win");
  let main = null;
  const c = e.code;
  if (/^Key[A-Z]$/.test(c)) main = /^[a-z]$/i.test(e.key) ? e.key.toLowerCase() : c.slice(3).toLowerCase();
  else if (/^Digit\d$/.test(c)) main = c.slice(5);
  else if (/^F\d{1,2}$/.test(c)) main = c.toLowerCase();
  else main = { Escape: "esc", Enter: "enter", Tab: "tab", Space: "space", Backspace: "backspace", Delete: "delete", Home: "home",
    End: "end", PageUp: "pageup", PageDown: "pagedown", ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right",
    PrintScreen: "printscreen", Insert: "insert" }[c] || null;
  if (!main) return null;
  parts.push(main);
  return parts.join("+");
}

// --- Fader ---
function faderInspector(i) {
  const page = currentPage();
  const own = pageFader(i);
  const slot = own || S.cfg.faders[i];
  const pageName = page.name || `Page ${S.page + 1}`;
  const out = [title(`Fader ${i + 1}`, ["le plus à gauche", "", "au milieu", "", "le plus à droite"][i] || "")];
  const live = S.status.faders ? S.status.faders[i] + " %" : "non connecté";
  out.push(el("p", { class: "hint", style: "margin:-6px 0 10px" }, "Position actuelle : ", el("b", { id: "fader-live" }, live)));

  // Réglage commun ou propre à la page
  out.push(el("div", { class: "tabs scope-tabs" },
    el("button", { class: own ? "" : "on", onclick: () => { if (!own) return; page.faders = (page.faders || []).filter((f) => f.index !== i + 1); if (!page.faders.length) delete page.faders; changed(); renderDevice(); } }, "Commun à toutes les pages"),
    el("button", { class: own ? "on" : "", onclick: () => { if (own) return; page.faders = page.faders || []; page.faders.push(Object.assign(clone(S.cfg.faders[i]), { index: i + 1 })); changed(); renderDevice(); } }, `Propre à « ${pageName} »`)));
  out.push(el("p", { class: "hint", style: "margin:-4px 0 12px" }, own
    ? `Sur cette page, ce fader a son propre réglage. Les autres pages gardent le réglage commun.`
    : `Ce réglage s'applique sur toutes les pages qui n'ont pas le leur.`));

  const t = (slot.target || "none").trim();
  const kind = t.startsWith("app:") ? "app" : t;
  const sel = el("select", { class: "input" }, ...FADER_TARGETS.map(([v, l]) => el("option", { value: v, selected: v === kind }, l)));
  sel.onchange = () => { slot.target = sel.value === "app" ? "app:" : sel.value; changed(); renderDevice(); renderInspector(); };
  out.push(field("Ce fader contrôle", sel));

  if (kind === "app") {
    const apps = t.slice(4).split(",").map((s) => s.trim()).filter(Boolean);
    const setApps = (list) => { slot.target = "app:" + list.join(","); changed(); renderDevice(); renderInspector(); };
    const chips = el("div", { class: "chips" }, ...apps.map((a) => el("span", { class: "chip removable", title: "Retirer", onclick: () => setApps(apps.filter((x) => x !== a)) }, a, " ×")));
    const add = el("input", { class: "input grow", placeholder: "ex. discord.exe", spellcheck: "false" });
    const addIt = (name) => { name = (name || "").trim().toLowerCase(); if (!name) return; if (!/\.exe$/.test(name)) name += ".exe"; if (!apps.includes(name)) setApps([...apps, name]); };
    add.onkeydown = (e) => { if (e.key === "Enter") addIt(add.value); };
    const running = el("div", { class: "chips" }, el("span", { class: "hint", style: "margin:0" }, "Recherche des applis…"));
    invoke("list_audio_apps").then((list) => {
      running.innerHTML = "";
      const others = (list || []).filter((x) => !apps.includes(x));
      if (!others.length) running.append(el("span", { class: "hint", style: "margin:0" }, "Aucune autre appli ne joue du son en ce moment."));
      for (const a of others) running.append(el("span", { class: "chip add", onclick: () => addIt(a) }, "+ " + a));
    });
    out.push(field("Applications", apps.length ? chips : el("p", { class: "hint", style: "margin:0" }, "Aucune pour l'instant."),
      el("div", { class: "row", style: "margin-top:10px" }, add, el("button", { class: "btn", onclick: () => addIt(add.value) }, "Ajouter"))));
    out.push(field("Applis qui jouent du son en ce moment", running,
      el("p", { class: "hint" }, "Une appli n'apparaît ici que lorsqu'elle joue (ou a joué) du son.")));
  }

  // LED de ce fader
  out.push(el("h2", {}, "LED de ce fader"));
  const mode = slot.led || "fixed";
  out.push(field("La LED", el("select", { class: "input", onchange: (e) => {
    slot.led = e.target.value;
    if (slot.led === "animation" && !slot.animation && S.cfg.lights.animations[0]) slot.animation = S.cfg.lights.animations[0].name;
    if ((slot.led === "position" || slot.led === "vu") && !(slot.gradient && slot.gradient.length)) slot.gradient = DEFAULT_GRADIENT.slice();
    changed(); renderInspector(); renderDevice();
  } }, ...FADER_LED_MODES.map(([v, l]) => el("option", { value: v, selected: v === mode }, l)))));
  if (mode === "position" || mode === "vu") {
    out.push(gradientEditor(slot, mode === "vu"
      ? "Couleur selon le niveau sonore : la LED s'allume plus fort quand ça joue fort."
      : "Exemple : vert en bas, jaune à 60 %, rouge en haut. La LED change de couleur en direct quand tu bouges le fader."));
  } else if (mode === "animation") {
    out.push(animationPicker(slot.animation, (v) => { slot.animation = v; changed(); }));
  } else {
    out.push(colorField(mode === "status" ? "Couleur quand le son passe" : "Couleur", slot.color, (v) => { if (!v) return; slot.color = v; changed(); renderDevice(); }));
    if (mode === "status") out.push(el("p", { class: "hint" }, kind === "app"
      ? "Rouge si l'appli est coupée, éteinte si elle n'est pas lancée."
      : "Rouge quand c'est coupé."));
  }
  if (mode === "vu" && kind === "none") out.push(el("p", { class: "hint warn-text" }, "Choisis d'abord ce que le fader contrôle : le VU-mètre mesure ce son-là."));
  out.push(el("button", { class: "btn", style: "width:100%", onclick: () => select({ type: "lights" }) }, "Gérer les animations…"));
  return out;
}

/** Éditeur du dégradé d'un fader (position ou niveau sonore → couleur). */
function gradientEditor(slot, hint) {
  const stops = () => faderGradient(slot).map((s) => { const [p, c] = s.split(":"); return { p: Number(p) || 0, c: c || "#FFFFFF" }; }).sort((a, b) => a.p - b.p);
  const save = (list) => { slot.gradient = list.sort((a, b) => a.p - b.p).map((x) => `${Math.round(x.p)}:${x.c}`); changed(); };
  const list = stops();
  const bar = el("div", { class: "gradient-bar", style: `background:linear-gradient(90deg, ${list.map((x) => `${x.c} ${x.p}%`).join(", ")})` });
  const rows = list.map((x, idx) => el("div", { class: "row gradient-row" },
    el("input", { type: "color", value: x.c, oninput: (e) => { list[idx].c = e.target.value.toUpperCase(); save(list); bar.style.background = `linear-gradient(90deg, ${list.map((y) => `${y.c} ${y.p}%`).join(", ")})`; } }),
    el("input", { class: "input", type: "number", min: 0, max: 100, value: x.p, style: "width:74px", onchange: (e) => { list[idx].p = Math.max(0, Math.min(100, Number(e.target.value) || 0)); save(list); renderInspector(); } }),
    el("span", { class: "hint", style: "margin:0" }, "%"),
    el("span", { class: "spacer" }),
    list.length > 2 ? el("button", { class: "btn small", onclick: () => { list.splice(idx, 1); save(list); renderInspector(); } }, "Retirer") : null));
  return field("Couleurs", bar, ...rows,
    el("button", { class: "btn small", style: "margin-top:6px", onclick: () => { list.push({ p: 50, c: "#FFFFFF" }); save(list); renderInspector(); } }, "+ Ajouter une couleur"),
    el("p", { class: "hint" }, hint));
}

/** Liste déroulante des animations du profil. */
function animationPicker(value, onChange) {
  const anims = S.cfg.lights.animations;
  if (!anims.length) {
    return field("Animation", el("p", { class: "hint", style: "margin:0" }, "Aucune animation pour l'instant."),
      el("button", { class: "btn small", style: "margin-top:6px", onclick: () => select({ type: "lights" }) }, "Créer une animation…"));
  }
  return field("Animation", el("select", { class: "input", onchange: (e) => onChange(e.target.value) },
    ...anims.map((a) => el("option", { value: a.name, selected: a.name === value }, a.name))));
}

// --- Molettes ---
function rollersInspector() {
  const r = S.cfg.rollers;
  const page = currentPage();
  const pageName = page.name || `Page ${S.page + 1}`;
  const out = [title("Molettes", pageName)];
  for (const [side, label] of [["left", "Molette de gauche"], ["right", "Molette de droite"]]) {
    const key = "roller_" + side;
    const own = page[key] != null;
    out.push(el("h2", {}, label));
    out.push(el("div", { class: "tabs scope-tabs" },
      el("button", { class: own ? "" : "on", onclick: () => { delete page[key]; changed(); renderDevice(); } }, "Commune"),
      el("button", { class: own ? "on" : "", onclick: () => { if (!own) { page[key] = r[side]; changed(); renderDevice(); } } }, `Propre à « ${pageName} »`)));
    out.push(field("Action", el("select", { class: "input", onchange: (e) => { if (own) page[key] = e.target.value; else r[side] = e.target.value; changed(); renderDevice(); } },
      ...ROLLER_MODES.map(([v, l]) => el("option", { value: v, selected: v === (own ? page[key] : r[side]) }, l)))));
    out.push(switchRow("Inverser le sens", r["invert_" + side], (v) => { r["invert_" + side] = v; changed(); }));
  }
  out.push(el("div", { class: "sep" }));
  out.push(rangeField("Volume par cran", r.volume_step, 1, 10, " %", (v) => { r.volume_step = v; changed(); }));
  out.push(rangeField("Lignes par cran (défilement)", r.scroll_step, 1, 10, "", (v) => { r.scroll_step = v; changed(); }));
  out.push(el("p", { class: "hint" }, "« Commune » : la même action sur toutes les pages qui n'ont pas la leur. Le sens et les pas sont communs à toutes les pages."));
  return out;
}

// --- Page ---
function pageInspector() {
  const p = currentPage();
  if (!p.apps) p.apps = [];
  const out = [title(p.folder ? "Dossier" : `Page ${S.page + 1}`, "Réglages de la page")];
  out.push(field("Nom", el("input", { class: "input", value: p.name || "", oninput: (e) => { p.name = e.target.value; changed(); renderPagesSoon(); } })));

  out.push(switchRow("Cette page est un dossier", !!p.folder, (v) => {
    if (v) { p.folder = true; p.close_after = true; } else { delete p.folder; delete p.close_after; }
    changed(); renderPages(); renderInspector();
  }));
  if (p.folder) {
    out.push(switchRow("Revenir à la page d'avant après une action", !!p.close_after, (v) => {
      if (v) p.close_after = true; else delete p.close_after;
      changed();
    }));
    out.push(el("p", { class: "hint", style: "margin-top:-4px" }, "Un dossier s'ouvre avec une touche « Ouvrir un dossier » et n'apparaît pas dans « page suivante / précédente »."));
  }
  out.push(el("div", { class: "sep" }));
  out.push(el("h2", { style: "margin-top:6px" }, "Page automatique"));
  out.push(el("p", { class: "hint", style: "margin:-4px 0 12px" },
    "Quand une de ces applications passe au premier plan, le MasterHUB affiche cette page tout seul. ",
    "En la quittant, il revient à la page d'avant."));
  const setApps = (list) => {
    p.apps = list;
    if (!list.length) delete p.apps;
    changed();
    renderPages();
    renderInspector();
  };
  const apps = p.apps || [];
  const addIt = (name) => {
    name = (name || "").trim().split(/[\\/]/).pop().toLowerCase();
    if (!name) return;
    if (!/\.exe$/.test(name)) name += ".exe";
    if (apps.includes(name)) return;
    // Une appli ne peut être liée qu'à une seule page.
    const other = S.cfg.pages.findIndex((q, i) => i !== S.page && (q.apps || []).includes(name));
    if (other >= 0) {
      S.cfg.pages[other].apps = S.cfg.pages[other].apps.filter((x) => x !== name);
      if (!S.cfg.pages[other].apps.length) delete S.cfg.pages[other].apps;
      toast(`${name} était lié à la page « ${S.cfg.pages[other].name} » : déplacé ici.`);
    }
    setApps([...apps, name]);
  };
  const chips = apps.length
    ? el("div", { class: "chips" }, ...apps.map((a) => el("span", { class: "chip removable", title: "Retirer", onclick: () => setApps(apps.filter((x) => x !== a)) }, a, " ×")))
    : el("p", { class: "hint", style: "margin:0" }, "Aucune application liée : cette page ne change qu'à la main.");
  const add = el("input", { class: "input grow", placeholder: "ex. fusion360.exe", spellcheck: "false" });
  add.onkeydown = (e) => { if (e.key === "Enter") addIt(add.value); };
  out.push(field("Applications liées", chips,
    el("div", { class: "row", style: "margin-top:10px" }, add, el("button", { class: "btn", onclick: () => addIt(add.value) }, "Ajouter")),
    el("button", { class: "btn", style: "width:100%;margin-top:8px", onclick: async () => {
      const f = await invoke("pick_file", { kind: "program" });
      if (f) addIt(f);
    } }, "Choisir un programme…")));

  const running = el("div", { class: "chips" }, el("span", { class: "hint", style: "margin:0" }, "Recherche des applications ouvertes…"));
  invoke("list_running_apps").then((list) => {
    running.innerHTML = "";
    const others = (list || []).filter((x) => !apps.includes(x));
    if (!others.length) running.append(el("span", { class: "hint", style: "margin:0" }, "Aucune autre application ouverte."));
    for (const a of others) running.append(el("span", { class: "chip add", onclick: () => addIt(a) }, "+ " + a));
  });
  out.push(field("Applications ouvertes en ce moment", running,
    el("p", { class: "hint" }, "Ouvre l'application, puis reviens ici et clique dessus.")));

  if (!S.cfg.general.auto_pages) {
    out.push(el("p", { class: "hint", style: "color:var(--warn, #ffb74d)" }, "Les pages automatiques sont désactivées dans les réglages généraux."));
  }
  out.push(el("div", { class: "sep" }));
  out.push(el("div", { class: "row" },
    el("button", { class: "btn grow", onclick: () => renamePage(S.page) }, "Renommer…"),
    el("button", { class: "btn grow", onclick: () => duplicatePage(S.page) }, "Dupliquer la page")));
  return out;
}

let pagesTimer = null;
function renderPagesSoon() { clearTimeout(pagesTimer); pagesTimer = setTimeout(renderPages, 250); }

// --- Addons ---
async function loadAddons() {
  try {
    const info = await invoke("list_addons");
    if (!info) return;
    S.addons = info.addons || [];
    S.addonsSig = info.signature;
    S.addonsFolder = info.folder;
  } catch (e) { S.addons = []; }
}

/** Rechargement à chaud : un addon ajouté, modifié ou retiré du dossier. */
async function checkAddons() {
  if (document.hidden) return;
  try {
    const sig = await invoke("addons_signature");
    if (sig === undefined || sig === null || sig === S.addonsSig) return;
    const before = new Set((S.addons || []).map((a) => a.folder));
    await loadAddons();
    buildLibrary();
    filterLibrary($("#lib-search").value || "");
    if (S.sel.type === "addons") renderInspector();
    const added = (S.addons || []).filter((a) => !before.has(a.folder));
    for (const a of added) toast(a.error ? `Addon « ${a.folder} » : ${a.error}` : `Addon « ${a.addon.name} » ajouté.`, !!a.error);
  } catch (_) {}
}

async function installForgeFile(f) {
  try {
    const data = await fileToBase64(f);
    const id = await invoke("install_addon_data", { data });
    await afterAddonInstall(id);
  } catch (e) { toast(String(e), true); }
}

async function afterAddonInstall(id) {
  await loadAddons();
  S.addonsSig = await invoke("addons_signature");
  buildLibrary();
  const ad = (S.addons || []).find((a) => a.folder === id);
  toast(ad && ad.addon ? `Addon « ${ad.addon.name} » installé : ses actions sont dans la liste de gauche.` : `Addon « ${id} » installé.`);
  select({ type: "addons" });
}

/** Fichier .forge lâché n'importe où sur la fenêtre. */
function setupForgeDrop() {
  document.addEventListener("dragover", (e) => {
    if (S.draggingKey) return;
    if (e.dataTransfer && [...(e.dataTransfer.items || [])].some((i) => i.kind === "file")) e.preventDefault();
  });
  document.addEventListener("drop", (e) => {
    if (S.draggingKey || e.target.closest("#keys")) return;
    const f = e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files[0];
    if (!f) return;
    e.preventDefault();
    if (/\.forge$/i.test(f.name)) installForgeFile(f);
    else if (/\.hfprofile$/i.test(f.name)) importProfileFile(f);
    else toast("Dépose un addon .forge ou un profil .hfprofile sur la fenêtre, ou une image sur une touche.");
  });
}

function addonIcon(ad, size) {
  const box = el("span", { class: "addon-icon", style: `width:${size}px;height:${size}px` });
  if (ad.icon && ad.icon.svg) box.innerHTML = ad.icon.svg;
  else if (ad.icon && ad.icon.src) box.append(el("img", { src: ad.icon.src, alt: "" }));
  else box.innerHTML = svgIcon("box");
  return box;
}

async function addAddonPages(ad) {
  const a = ad.addon;
  if (S.cfg.pages.some((p) => a.pages.some((q) => q.name === p.name))) {
    if (!(await confirm_(`Ce profil a déjà une page « ${a.pages[0].name} ». Ajouter quand même les pages de ${a.name} ?`))) return;
  }
  try {
    const base = S.cfg.pages.length;
    const pages = await invoke("addon_pages", { id: ad.folder, base });
    // Une application ne peut ouvrir qu'une page : on la retire des autres pages.
    const apps = new Set(pages.flatMap((p) => (p.apps || []).map((x) => x.toLowerCase())));
    for (const p of S.cfg.pages) if (p.apps) { p.apps = p.apps.filter((x) => !apps.has(x.toLowerCase())); if (!p.apps.length) delete p.apps; }
    S.cfg.pages.push(...pages);
    changed({ immediate: true });
    toast(`${pages.length} page${pages.length > 1 ? "s" : ""} ajoutée${pages.length > 1 ? "s" : ""} au profil « ${S.profile} ».`);
    goToPage(base);
  } catch (e) { toast(String(e), true); }
}

function addonsInspector() {
  const out = [title("Addons", "Niveau 1 · sans code")];
  out.push(el("div", { class: "row", style: "margin-bottom:10px" },
    el("button", { class: "btn primary grow", onclick: async () => {
      const p = await invoke("pick_file", { kind: "addon" });
      if (!p) return;
      try { await afterAddonInstall(await invoke("install_addon", { path: p })); } catch (e) { toast(String(e), true); }
    } }, "Installer un addon…"),
    el("button", { class: "btn", title: S.addonsFolder || "", onclick: () => invoke("open_addons_folder") }, "Dossier")));
  out.push(el("p", { class: "hint", style: "margin-top:0" }, "Glisse un fichier .forge sur la fenêtre pour l'installer. Un addon copié dans le dossier apparaît tout seul."));
  const list = S.addons || [];
  if (!list.length) {
    out.push(el("p", { class: "empty-state" }, "Aucun addon installé."));
  }
  for (const ad of list) {
    const a = ad.addon;
    const card = el("div", { class: "addon-card" + (ad.error ? " broken" : "") + (ad.enabled ? "" : " off") });
    const head = el("div", { class: "addon-head" }, addonIcon(ad, 34),
      el("div", { class: "grow" },
        el("div", { class: "addon-name" }, a ? a.name : ad.folder, ad.official ? el("span", { class: "addon-badge" }, "officiel") : null),
        el("div", { class: "addon-meta" }, a ? [`v${a.version}`, a.author, a.license].filter(Boolean).join(" · ") : "Addon en erreur")));
    card.append(head);
    if (ad.error) {
      card.append(el("div", { class: "addon-error" }, ad.error));
    } else {
      if (a.description) card.append(el("p", { class: "addon-desc" }, a.description));
      const counts = [`${a.actions.length} action${a.actions.length > 1 ? "s" : ""}`];
      if (a.pages.length) counts.push(`${a.pages.length} page${a.pages.length > 1 ? "s" : ""} prête${a.pages.length > 1 ? "s" : ""}`);
      if (a.detect.length) counts.push(`détecte ${a.detect.join(", ")}`);
      card.append(el("div", { class: "addon-meta", style: "margin-bottom:10px" }, counts.join(" · ")));
      card.append(switchRow("Activé (actions dans la liste de gauche)", ad.enabled, async (v) => {
        await invoke("set_addon_enabled", { id: ad.folder, on: v });
        ad.enabled = v;
        buildLibrary();
        renderInspector();
      }));
      if (a.pages.length) card.append(el("button", { class: "btn", style: "width:100%;margin-top:4px", onclick: () => addAddonPages(ad) },
        `Ajouter ${a.pages.length > 1 ? "ses pages" : "sa page"} au profil « ${S.profile} »`));
    }
    card.append(el("button", { class: "btn danger small", style: "margin-top:8px", onclick: async () => {
      if (!(await confirm_(`Supprimer l'addon « ${a ? a.name : ad.folder} » ? Les touches déjà placées gardent leur action.`))) return;
      try { await invoke("remove_addon", { id: ad.folder }); await loadAddons(); S.addonsSig = await invoke("addons_signature"); buildLibrary(); renderInspector(); }
      catch (e) { toast(String(e), true); }
    } }, "Supprimer"));
    out.push(card);
  }
  if (!list.some((x) => x.official)) {
    out.push(el("button", { class: "btn", style: "width:100%", onclick: async () => {
      await invoke("restore_official_addons"); await loadAddons(); S.addonsSig = await invoke("addons_signature"); buildLibrary(); renderInspector();
    } }, "Réinstaller les addons officiels"));
  }
  out.push(el("div", { class: "sep" }));
  out.push(el("p", { class: "hint" }, "Créer un addon : un dossier avec un manifest.toml et des icônes, zippé et renommé en .forge. Le guide complet (avec un modèle à donner à une IA) est dans docs/ADDON_GUIDE.md du dépôt GitHub."));
  return out;
}

// --- LED de la base, orientation et positions ---
function baseInspector() {
  const g = S.cfg.general, L = S.cfg.lights;
  const out = [title("Base", "Profil " + (S.profile || ""))];
  out.push(field("Orientation sur le bureau", el("select", { class: "input", onchange: async (e) => {
    const rot = Number(e.target.value);
    S.layout.base_rot = rot;
    await invoke("set_base_rot", { rot });
    renderDevice(); fitDevice();
  } }, ...BASE_ROTS.map(([v, l]) => el("option", { value: v, selected: v === ((S.layout && S.layout.base_rot) || 0) }, l))),
    el("p", { class: "hint" }, "Les images des touches sont tournées pour rester lisibles. Commun à tous les profils.")));

  out.push(el("h2", {}, "LED de la base"));
  out.push(field("Lumière", el("select", { class: "input", onchange: (e) => { L.base_mode = e.target.value; if (L.base_mode === "animation" && !L.base_animation && L.animations[0]) L.base_animation = L.animations[0].name; changed(); renderInspector(); renderDevice(); } },
    el("option", { value: "hardware", selected: L.base_mode !== "animation" }, "Effet intégré de la base"),
    el("option", { value: "animation", selected: L.base_mode === "animation" }, "Animation HubForge"))));
  if (L.base_mode === "animation") {
    out.push(animationPicker(L.base_animation, (v) => { L.base_animation = v; changed(); }));
  } else {
    const prev = el("div", { class: "led-preview" });
    const paint = () => {
      const off = g.base_effect === 0;
      prev.style.background = off ? "#0b0c10" : `linear-gradient(90deg, ${g.base_color}22, ${g.base_color}, ${g.base_color}22)`;
      prev.className = "led-preview" + (off ? "" : ` fx-${g.base_effect}`);
    };
    paint();
    out.push(prev);
    out.push(colorField("Couleur", g.base_color, (v) => { if (!v) return; g.base_color = v; paint(); renderDevice(); changed(); }));
    out.push(field("Effet", el("select", { class: "input", onchange: (e) => { g.base_effect = Number(e.target.value); paint(); renderDevice(); changed(); } },
      ...BASE_EFFECTS.map(([v, l]) => el("option", { value: v, selected: v === g.base_effect }, l)))));
  }
  out.push(rangeField("Luminosité des LED (base et faders)", g.led_brightness, 0, 100, " %", (v) => { g.led_brightness = v; changed(); }));
  out.push(el("button", { class: "btn", style: "width:100%", onclick: () => select({ type: "lights" }) }, "Animations…"));

  // Positions des modules
  out.push(el("h2", {}, "Position des modules"));
  const mods = (S.status.modules || []).filter((m) => MODULE_SIZE[m.kind]);
  if (!mods.length) out.push(el("p", { class: "hint" }, "Aucun module connecté."));
  for (const m of mods) {
    const key = `${m.kind}:${m.port}`;
    const manual = S.layout && S.layout.placements && S.layout.placements[key];
    const state = !m.port ? "port inconnu" : manual ? "position saisie à la main" : m.placement ? "détectée automatiquement ✓" : "position non reconnue";
    const box = el("div", { class: "place-card" + (!m.placement && m.port ? " warn" : "") },
      el("div", { class: "row" }, el("b", {}, MODULE_NAMES[m.kind] || m.kind), el("span", { class: "spacer" }), el("span", { class: "hint", style: "margin:0" }, state)));
    if (m.port && (!m.placement || manual)) {
      const p = Object.assign({ col: 0, row: 0, rot: 0 }, m.placement || {});
      const sel = (label, k, values) => el("label", { class: "mini" }, label, el("select", { class: "input", onchange: (e) => { p[k] = Number(e.target.value); } },
        ...values.map((v) => el("option", { value: v, selected: v === p[k] }, k === "rot" ? v + "°" : String(v + 1)))));
      box.append(el("p", { class: "hint" }, `Branchement ${m.port} inconnu de HubForge : indique où est le module (colonne et ligne de son coin haut-gauche, base vue verticale).`),
        el("div", { class: "row" }, sel("Colonne", "col", [0, 1, 2, 3]), sel("Ligne", "row", [0, 1, 2, 3, 4, 5]), sel("Rotation", "rot", [0, 90, 180, 270])),
        el("div", { class: "row", style: "margin-top:6px" },
          el("button", { class: "btn small primary", onclick: async () => { await invoke("set_placement", { kind: m.kind, code: m.port, placement: p }); S.layout = await invoke("get_layout"); toast("Position enregistrée."); } }, "Enregistrer"),
          manual ? el("button", { class: "btn small", onclick: async () => { await invoke("set_placement", { kind: m.kind, code: m.port, placement: null }); S.layout = await invoke("get_layout"); renderInspector(); } }, "Oublier") : null));
    }
    out.push(box);
  }
  out.push(el("p", { class: "hint" }, "HubForge reconnaît la position et le sens des modules d'après leur branchement dans la base. Si un branchement n'est pas encore connu, tu peux le saisir ici ; envoie-nous le rapport de diagnostic pour qu'il soit reconnu pour tout le monde."));
  return out;
}

// --- Lumières : animations et jeux ---
function presetName(id) { const p = LIGHT_PRESETS.find((x) => x[0] === id); return p ? p[1] : "Faite à la main"; }

function newAnimation(preset) {
  const L = S.cfg.lights;
  let base = preset ? presetName(preset) : "Mon animation", name = base, n = 2;
  while (L.animations.some((a) => a.name === name)) name = `${base} ${n++}`;
  const a = { name, preset: preset || "", colors: ["#00C8FF", "#FF2D9A"], speed: 1, steps: [] };
  if (!preset) a.steps = [
    { ms: 600, fade: true, base: "#FF7A1A", faders: ["#FF7A1A", "#000000", "#000000", "#000000", "#FF7A1A"] },
    { ms: 600, fade: true, base: "#2040FF", faders: ["#000000", "#2040FF", "#2040FF", "#2040FF", "#000000"] },
  ];
  L.animations.push(a);
  S.editAnim = L.animations.length - 1;
  changed();
  renderInspector();
}

function renameAnimation(a, name) {
  const L = S.cfg.lights;
  name = name.trim();
  if (!name || name === a.name) return;
  if (L.animations.some((x) => x !== a && x.name === name)) { toast("Une animation porte déjà ce nom.", true); renderInspector(); return; }
  if (L.base_animation === a.name) L.base_animation = name;
  [...S.cfg.faders, ...S.cfg.pages.flatMap((p) => p.faders || [])].forEach((f) => { if (f.animation === a.name) f.animation = name; });
  a.name = name;
  changed();
}

async function previewAnim(a) {
  if (S.previewing === a) { await invoke("preview_animation", { animation: null }); S.previewing = null; renderInspector(); return; }
  S.previewing = a;
  await invoke("preview_animation", { animation: clean(a) });
  renderInspector();
}


function lightsInspector() {
  const L = S.cfg.lights;
  const out = [title("Lumières", "Profil " + (S.profile || ""))];

  out.push(el("h2", {}, "Animations"));
  if (!L.animations.length) out.push(el("p", { class: "hint" }, "Crée une animation à partir d'un modèle ou fais-la toi-même étape par étape, puis choisis-la pour la base ou pour les faders."));
  L.animations.forEach((a, i) => {
    const faders = S.cfg.faders.map((f, n) => (f.led === "animation" && f.animation === a.name ? n + 1 : 0)).filter(Boolean);
    const used = [L.base_mode === "animation" && L.base_animation === a.name ? "base" : null, faders.length ? "fader" + (faders.length > 1 ? "s " : " ") + faders.join(", ") : null].filter(Boolean);
    out.push(el("div", { class: "anim-card" + (S.editAnim === i ? " open" : "") },
      el("div", { class: "row" },
        el("div", { class: "grow" }, el("b", {}, a.name), el("div", { class: "hint", style: "margin:2px 0 0" }, presetName(a.preset) + (used.length ? " · sur " + used.join(" et ") : ""))),
        el("button", { class: "btn small" + (S.previewing === a ? " primary" : ""), title: "Jouer sur le MasterHUB", onclick: () => previewAnim(a) }, S.previewing === a ? "■ Arrêter" : "▶ Essayer"),
        el("button", { class: "btn small", onclick: () => { S.editAnim = S.editAnim === i ? null : i; renderInspector(); } }, S.editAnim === i ? "Fermer" : "Modifier")),
      S.editAnim === i ? animationEditor(a, i) : null));
  });
  out.push(el("div", { class: "row", style: "margin:8px 0 4px" },
    el("select", { class: "input grow", id: "new-anim-preset" }, ...LIGHT_PRESETS.map(([id, name]) => el("option", { value: id }, "Modèle : " + name)), el("option", { value: "" }, "Faite à la main (étapes)")),
    el("button", { class: "btn primary", onclick: () => newAnimation($("#new-anim-preset").value) }, "Créer")));

  out.push(el("h2", {}, "Fluidité"));
  out.push(rangeField("Images par seconde envoyées aux LED", L.fps, 5, 30, " i/s", (v) => { L.fps = v; changed(); },
    "Si les LED saccadent ou retardent, baisse cette valeur."));
  return out;
}

function animationEditor(a, i) {
  const L = S.cfg.lights;
  const box = el("div", { class: "anim-editor" });
  box.append(field("Nom", el("input", { class: "input", value: a.name, onchange: (e) => renameAnimation(a, e.target.value) })));
  const restart = () => { if (S.previewing === a) invoke("preview_animation", { animation: clean(a) }); };
  if (a.preset) {
    const n = (LIGHT_PRESETS.find((x) => x[0] === a.preset) || [0, 0, 1])[2];
    a.colors = a.colors || [];
    for (let c = 0; c < n; c++) {
      box.append(colorField(n > 1 ? `Couleur ${c + 1}` : "Couleur", a.colors[c] || "#00C8FF", (v) => { if (!v) return; a.colors[c] = v; changed(); restart(); }));
    }
    box.append(rangeField("Vitesse", Math.round((a.speed || 1) * 100), 25, 400, " %", (v) => { a.speed = v / 100; changed(); restart(); }));
  } else {
    a.steps = a.steps || [];
    a.steps.forEach((st, j) => {
      st.faders = st.faders || [];
      while (st.faders.length < 5) st.faders.push("#000000");
      const colorIn = (value, set) => el("input", { type: "color", value: value || "#000000", oninput: (e) => { set(e.target.value.toUpperCase()); changed(); restart(); } });
      box.append(el("div", { class: "step-card" },
        el("div", { class: "row" },
          el("b", {}, `Étape ${j + 1}`), el("span", { class: "spacer" }),
          el("input", { class: "input", type: "number", min: 20, max: 60000, step: 50, value: st.ms, style: "width:84px", onchange: (e) => { st.ms = Math.max(20, Number(e.target.value) || 500); changed(); restart(); } }),
          el("span", { class: "hint", style: "margin:0" }, "ms")),
        el("div", { class: "row step-colors" },
          el("label", { class: "mini" }, "Base", colorIn(st.base, (v) => { st.base = v; })),
          ...st.faders.slice(0, 5).map((c, f) => el("label", { class: "mini" }, `F${f + 1}`, colorIn(c, (v) => { st.faders[f] = v; })))),
        el("div", { class: "row" },
          el("label", { class: "mini check" }, el("input", { type: "checkbox", checked: st.fade !== false, onchange: (e) => { st.fade = e.target.checked; changed(); restart(); } }), "Fondu vers l'étape suivante"),
          el("span", { class: "spacer" }),
          el("button", { class: "btn small", title: "Dupliquer", onclick: () => { a.steps.splice(j + 1, 0, clone(st)); changed(); renderInspector(); restart(); } }, "⧉"),
          a.steps.length > 1 ? el("button", { class: "btn small", title: "Supprimer", onclick: () => { a.steps.splice(j, 1); changed(); renderInspector(); restart(); } }, "✕") : null)));
    });
    box.append(el("button", { class: "btn small", onclick: () => { a.steps.push(clone(a.steps[a.steps.length - 1] || { ms: 500, fade: true, base: "#000000", faders: Array(5).fill("#000000") })); changed(); renderInspector(); } }, "+ Ajouter une étape"));
  }
  box.append(el("div", { class: "row", style: "margin-top:10px" },
    el("button", { class: "btn small", onclick: () => { L.base_mode = "animation"; L.base_animation = a.name; changed(); renderInspector(); renderDevice(); toast(`« ${a.name} » joue maintenant sur la base.`); } }, "Sur la base"),
    el("button", { class: "btn small", title: "Les 5 faders (réglage commun)", onclick: () => { S.cfg.faders.forEach((f) => { f.led = "animation"; f.animation = a.name; }); changed(); renderInspector(); renderDevice(); toast(`« ${a.name} » joue maintenant sur les 5 faders.`); } }, "Sur les faders"),
    el("span", { class: "spacer" }),
    el("button", { class: "btn small danger", onclick: async () => {
      if (!(await confirm_(`Supprimer l'animation « ${a.name} » ?`))) return;
      if (S.previewing === a) { await invoke("preview_animation", { animation: null }); S.previewing = null; }
      L.animations.splice(i, 1);
      if (L.base_animation === a.name) { L.base_animation = ""; L.base_mode = "hardware"; }
      const allFaders = [...S.cfg.faders, ...S.cfg.pages.flatMap((p) => p.faders || [])];
      allFaders.forEach((f) => { if (f.led === "animation" && f.animation === a.name) { f.led = "fixed"; f.animation = ""; } });
      S.editAnim = null;
      changed(); renderInspector(); renderDevice();
    } }, "Supprimer")));
  return box;
}

// --- Profils ---
function renderProfileButton() {
  $("#profile-name").textContent = S.profile || "Profil";
}

async function toggleProfileMenu(forceOpen) {
  const m = $("#profile-menu");
  if (!forceOpen && !m.classList.contains("hidden")) { m.classList.add("hidden"); return; }
  try { S.profiles = (await invoke("list_profiles")) || S.profiles; } catch (_) {}
  m.innerHTML = "";
  m.append(el("div", { class: "pm-title" }, "Profils"));
  for (const name of S.profiles) {
    const on = name === S.profile;
    const item = el("div", { class: "pm-item" + (on ? " on" : ""), title: on ? "Profil actif" : "Passer sur ce profil" },
      el("span", { class: "check" }, on ? "✓" : ""),
      el("span", { class: "grow" }, name),
      on ? null : el("span", { class: "del", title: "Supprimer ce profil", onclick: async (e) => {
        e.stopPropagation();
        if (!(await confirm_(`Supprimer le profil « ${name} » ? (une copie est gardée dans profiles-supprimes)`))) return;
        try { await invoke("delete_profile", { name }); toast(`Profil « ${name} » supprimé.`); toggleProfileMenu(true); }
        catch (err) { toast(String(err), true); }
      } }, "×"));
    item.onclick = () => { m.classList.add("hidden"); if (!on) switchProfile(name); };
    m.append(item);
  }
  m.append(el("div", { class: "pm-sep" }));
  const action = (label, fn) => { const it = el("div", { class: "pm-item pm-action" }, el("span", { class: "check" }, ""), el("span", { class: "grow" }, label)); it.onclick = () => { m.classList.add("hidden"); fn(); }; return it; };
  m.append(action("+ Nouveau profil vierge…", () => newProfile(null)));
  m.append(action("⧉ Dupliquer « " + (S.profile || "") + " »…", () => newProfile(S.profile)));
  m.append(action("✎ Renommer « " + (S.profile || "") + " »…", renameProfile));
  m.append(el("div", { class: "pm-sep" }));
  m.append(action("⇪ Exporter « " + (S.profile || "") + " » dans un fichier…", exportProfile));
  m.append(action("⇩ Importer un profil…", importProfile));
  m.append(action("⟲ Sauvegardes de « " + (S.profile || "") + " »…", showBackups));
  m.classList.remove("hidden");
}

async function switchProfile(name) {
  try { await invoke("switch_profile", { name }); toast(`Profil « ${name} »`); }
  catch (e) { toast(String(e), true); }
}

async function newProfile(copyFrom) {
  const def = copyFrom ? `${copyFrom} (copie)` : "Nouveau profil";
  const name = await prompt_(copyFrom ? `Nom de la copie de « ${copyFrom} » :` : "Nom du nouveau profil (ex. Laura, Streaming, Création) :", def);
  if (!name) return;
  try { const n = await invoke("create_profile", { name, copyFrom }); toast(`Profil « ${n} » créé.`); }
  catch (e) { toast(String(e), true); }
}

async function renameProfile() {
  const name = await prompt_("Nouveau nom du profil :", S.profile);
  if (!name || name === S.profile) return;
  try { await invoke("rename_profile", { old: S.profile, new: name }); }
  catch (e) { toast(String(e), true); }
}

/** Le profil a changé (depuis l'interface ou une touche) : on recharge tout. */
async function reloadProfile() {
  clearTimeout(S.saveTimer);
  const ini = await invoke("get_initial");
  S.cfg = normalize(ini.config);
  resetHistory();
  S.profile = ini.profile;
  S.profiles = ini.profiles || [];
  S.configPath = ini.config_path;
  S.previews = {};
  S.page = Math.min(ini.status.page || 0, S.cfg.pages.length - 1);
  S.devicePage = S.page;
  S.sel = { type: "settings" };
  renderProfileButton();
  renderAll();
  await refreshPreviews(S.page);
}

// --- Annuler / Rétablir ---
// Chaque modification enregistre l'état d'avant ; les modifications rapprochées
// (frappe, curseur qu'on fait glisser) sont regroupées en une seule étape.
const HISTORY_MAX = 100;
function snapshot() { return JSON.stringify(clean(S.cfg)); }
function resetHistory() {
  S.history = [];
  S.future = [];
  S.histCur = S.cfg ? snapshot() : null;
  S.histLast = 0;
  updateUndoButtons();
}
function recordHistory() {
  if (S.restoringHistory || !S.cfg) return;
  const now = snapshot();
  if (now === S.histCur) return;
  if (Date.now() - (S.histLast || 0) > 700 && S.histCur) {
    S.history.push(S.histCur);
    if (S.history.length > HISTORY_MAX) S.history.shift();
  }
  S.histCur = now;
  S.histLast = Date.now();
  S.future = [];
  updateUndoButtons();
}
function applySnapshot(snap) {
  S.restoringHistory = true;
  S.cfg = normalize(JSON.parse(snap));
  S.histCur = snap;
  S.histLast = 0;
  S.page = Math.min(S.page, S.cfg.pages.length - 1);
  S.previews = {};
  renderAll();
  refreshPreviews(S.page);
  setSaveState("saving", "Enregistrement…");
  clearTimeout(S.saveTimer);
  save();
  S.restoringHistory = false;
  updateUndoButtons();
}
function undo() {
  if (!S.history || !S.history.length) { toast("Rien à annuler."); return; }
  clearTimeout(S.saveTimer);
  recordHistory(); // la modification en cours devient annulable
  S.future.push(S.histCur);
  applySnapshot(S.history.pop());
  toast("Modification annulée (Ctrl+Y pour rétablir).");
}
function redo() {
  if (!S.future || !S.future.length) { toast("Rien à rétablir."); return; }
  S.history.push(S.histCur);
  applySnapshot(S.future.pop());
  toast("Modification rétablie.");
}
function updateUndoButtons() {
  const u = $("#btn-undo"), r = $("#btn-redo");
  if (u) u.disabled = !(S.history && S.history.length);
  if (r) r.disabled = !(S.future && S.future.length);
}

// --- Copier / coller une touche, dupliquer une page ---
function copyKey(k) {
  const def = keyDef(k);
  if (!def) { toast("Touche vide : rien à copier."); return; }
  S.clipboard = clone(def);
  toast(`Touche ${k} copiée. Sélectionne une autre touche et fais Ctrl+V.`);
}
function pasteKey(k) {
  if (!S.clipboard) { toast("Rien à coller : copie d'abord une touche (Ctrl+C)."); return; }
  const def = clone(S.clipboard);
  setKeyDef(k, def);
  S.editState = 0;
  keyChanged(k, { immediate: true });
  renderInspector();
  toast(`Touche collée sur la ${k} (Ctrl+Z pour annuler).`);
}
function duplicatePage(i) {
  const copy = clone(S.cfg.pages[i]);
  copy.name = `${copy.name || "Page " + (i + 1)} (copie)`;
  delete copy.apps; // une appli n'ouvre qu'une seule page
  S.cfg.pages.push(copy);
  changed({ immediate: true });
  movePage(S.cfg.pages.length - 1, i + 1);
  goToPage(i + 1);
  toast("Page dupliquée.");
}

// --- Export / import de profils ---
async function exportProfile() {
  try {
    const path = await invoke("export_profile", { name: S.profile });
    if (path) toast(`Profil exporté : ${path}`);
  } catch (e) { toast(String(e), true); }
}
async function importProfile() {
  const p = await invoke("pick_file", { kind: "profile" });
  if (!p) return;
  try { afterProfileImport(await invoke("import_profile", { path: p })); } catch (e) { toast(String(e), true); }
}
async function importProfileFile(f) {
  try { afterProfileImport(await invoke("import_profile_data", { data: await fileToBase64(f) })); } catch (e) { toast(String(e), true); }
}
function afterProfileImport(r) {
  if (!r) return;
  const imgs = r.images ? ` avec ${r.images} image${r.images > 1 ? "s" : ""}` : "";
  if (r.missing_addons && r.missing_addons.length) {
    toast(`Profil « ${r.name} » importé${imgs}. Addons à installer pour certaines touches : ${r.missing_addons.join(", ")}.`, true);
  } else {
    toast(`Profil « ${r.name} » importé${imgs} et activé.`);
  }
}

// --- Sauvegardes automatiques ---
function formatDateTime(secs) {
  try {
    return new Date(secs * 1000).toLocaleString("fr-FR", { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
  } catch (_) { return String(secs); }
}
async function showBackups() {
  let list = [];
  try { list = (await invoke("list_backups")) || []; } catch (e) { toast(String(e), true); return; }
  const body = el("div", { class: "backup-list" });
  if (!list.length) body.append(el("p", {}, "Pas encore de sauvegarde : HubForge en fait une automatiquement quand tu modifies ce profil (au plus une toutes les 2 minutes, les 30 dernières sont gardées)."));
  list.forEach((b, i) => {
    body.append(el("div", { class: "backup-row" },
      el("div", { class: "grow" },
        el("div", { class: "backup-date" }, formatDateTime(b.time), i === 0 ? el("span", { class: "addon-badge", style: "margin-left:8px" }, "la plus récente") : null),
        el("div", { class: "backup-meta" }, `${b.pages} page${b.pages > 1 ? "s" : ""} · ${b.keys} touche${b.keys > 1 ? "s" : ""}`)),
      el("button", { class: "btn small", onclick: async () => {
        if (!(await confirm_(`Remettre le profil « ${S.profile} » comme il était le ${formatDateTime(b.time)} ? (La version actuelle est sauvegardée avant, tu pourras y revenir.)`))) return;
        try { await invoke("restore_backup", { file: b.file }); closeDialog(); await reloadProfile(); toast("Profil restauré."); }
        catch (e) { toast(String(e), true); }
      } }, "Restaurer")));
  });
  openDialog({
    title: `Sauvegardes de « ${S.profile} »`,
    sub: "Copies automatiques avant chaque modification",
    html: "",
    actions: [el("button", { class: "btn primary", onclick: closeDialog }, "Fermer")],
    icon: `<div class="dialog-glyph">${svgIcon("refresh-cw")}</div>`,
  });
  $("#dialog-body").append(body);
}

// --- Diagnostic ---
async function showDiagnostic() {
  let d;
  try { d = await invoke("get_diagnostic"); } catch (e) { toast(String(e), true); return; }
  const kinds = { keys15: "15 touches", faders5: "5 faders", rollers2: "2 molettes", base: "LED de la base", encoder: "Encodeur", knobs3: "3 boutons" };
  const mods = (d.status.modules || []).map((m) => `${(kinds[m.kind] || m.kind).padEnd(16)} série ${m.serial || "?"}${m.port ? " · port " + m.port : ""}${m.placement ? ` · col ${m.placement.col} lig ${m.placement.row} ${m.placement.rot}°` : ""}`);
  const report = [
    `HubForge ${d.version} (${d.os})`,
    `Profil : ${d.profile}`,
    `Dossier : ${d.config_dir}`,
    "",
    "Modules :",
    ...(mods.length ? mods.map((m) => "  " + m) : ["  aucun module détecté"]),
    "",
    "Addons :",
    ...(d.addons.length ? d.addons.map((a) => "  " + a) : ["  aucun"]),
    "",
    "Journal (fin) :",
    d.log,
  ].join("\n");
  const html = `<div class="diag-grid">
      <div><b>Version</b>${d.version}</div><div><b>Système</b>${d.os}</div><div><b>Profil</b>${d.profile.replace(/</g, "&lt;")}</div>
    </div>
    <h4>Modules détectés (${mods.length})</h4>
    <pre class="diag-pre">${(mods.join("\n") || "Aucun module : vérifie le câble USB de la base.").replace(/</g, "&lt;")}</pre>
    <h4>Journal</h4>
    <pre class="diag-pre diag-log">${d.log.replace(/&/g, "&amp;").replace(/</g, "&lt;")}</pre>
    <p class="hint">Rien n'est envoyé : « Copier le rapport » met ces informations dans le presse-papiers pour que tu puisses les joindre à une demande d'aide.</p>`;
  openDialog({
    title: "Diagnostic",
    sub: "État du MasterHUB et du logiciel",
    html,
    icon: `<div class="dialog-glyph">${svgIcon("activity")}</div>`,
    actions: [
      el("button", { class: "btn small", onclick: async () => { await invoke("reconnect_modules"); toast("Modules reconnectés : ils réapparaissent dans 1 à 2 secondes."); } }, "Reconnecter les modules"),
      el("button", { class: "btn small", title: "Si un module rebranché à chaud n'est plus vu", onclick: async () => {
        toast("Réexamen de la base…");
        try { toast(await invoke("reset_base")); } catch (e) { toast(`Réexamen impossible : ${e}. Débranche puis rebranche le câble USB de la base.`, true); }
      } }, "Réinitialiser la base"),
      el("span", { class: "spacer" }),
      el("button", { class: "btn", onclick: async () => {
        try { await navigator.clipboard.writeText(report); toast("Rapport copié dans le presse-papiers."); } catch (_) { toast("Copie impossible.", true); }
      } }, "Copier le rapport"),
      el("button", { class: "btn primary", onclick: closeDialog }, "Fermer"),
    ],
  });
}

// --- Supprimer une touche en la glissant hors du MasterHUB ---
/** Page (index) ouverte par une action « dossier ». */
function folderPageIndex(to) {
  to = String(to || "").trim();
  if (/^\d+$/.test(to)) { const i = Number(to) - 1; return i >= 0 && i < S.cfg.pages.length ? i : -1; }
  return S.cfg.pages.findIndex((p) => (p.name || "").toLowerCase() === to.toLowerCase());
}

/** Glisser une touche sur une touche « dossier » la range dans ce dossier. */
function moveKeyIntoFolder(a, folderDef) {
  const idx = folderPageIndex(folderDef.action.to);
  if (idx < 0) { toast("Ce dossier n'ouvre aucune page : choisis-en une dans ses réglages.", true); return; }
  if (idx === S.page) { toast("Cette touche est déjà dans ce dossier."); return; }
  const dest = S.cfg.pages[idx];
  dest.keys = dest.keys || [];
  const used = new Set(dest.keys.map((d) => d.key));
  let free = 0;
  for (let n = 1; n <= 15; n++) if (!used.has(n)) { free = n; break; }
  if (!free) { toast(`Le dossier « ${dest.name} » est plein (15 touches).`, true); return; }
  const def = clone(keyDef(a));
  const from = S.page;
  def.key = free;
  dest.keys.push(def);
  dest.keys.sort((x, y) => x.key - y.key);
  setKeyDef(a, null);
  delete S.previews[`${from}:${a}:0`]; delete S.previews[`${from}:${a}:1`];
  updateKeyNode(a);
  if (S.sel.type === "key" && S.sel.key === a) renderInspector();
  changed();
  toast(`Touche rangée dans le dossier « ${dest.name} » (touche ${free}).`, false, { label: "Annuler", fn: () => {
    const page = S.cfg.pages[idx], back = S.cfg.pages[from];
    if (!page || !back || (back.keys || []).some((d) => d.key === a)) return;
    page.keys = (page.keys || []).filter((d) => d.key !== free);
    def.key = a;
    back.keys.push(def);
    back.keys.sort((x, y) => x.key - y.key);
    changed();
    if (S.page === from) refreshPreviews(from, a);
  } });
}

function setupTrashDrop() {
  const outside = (e) => S.draggingKey && !e.target.closest("#keys");
  document.addEventListener("dragover", (e) => {
    if (!S.draggingKey) return;
    const out = outside(e);
    document.body.classList.toggle("over-trash", out);
    if (out) { e.preventDefault(); e.dataTransfer.dropEffect = "move"; }
  });
  document.addEventListener("drop", (e) => {
    if (!outside(e)) return;
    e.preventDefault();
    const k = S.draggingKey;
    const def = keyDef(k);
    if (!def) return;
    const page = S.page;
    const backup = clone(def);
    setKeyDef(k, null);
    delete S.previews[`${page}:${k}:0`]; delete S.previews[`${page}:${k}:1`];
    updateKeyNode(k);
    if (S.sel.type === "key" && S.sel.key === k) renderInspector();
    changed();
    toast(`Touche ${k} supprimée.`, false, { label: "Annuler", fn: () => {
      const keys = S.cfg.pages[page] && S.cfg.pages[page].keys;
      if (!keys || keys.some((d) => d.key === k)) return;
      keys.push(backup);
      keys.sort((a, b) => a.key - b.key);
      changed();
      if (S.page === page) refreshPreviews(page, k);
    } });
  });
}

// --- Réordonner les onglets de pages ---
function movePage(from, to) {
  if (from === to || from < 0 || to < 0 || from >= S.cfg.pages.length || to >= S.cfg.pages.length) return;
  // Nouvel ordre : order[nouvel index] = ancien index
  const order = S.cfg.pages.map((_, i) => i);
  order.splice(to, 0, order.splice(from, 1)[0]);
  const newIndex = (old) => order.indexOf(old);
  const fix = (a) => {
    if (!a) return a;
    if (a.type === "macro") { (a.steps || []).forEach(fix); return a; }
    if ((a.type === "page" || a.type === "folder") && /^\d+$/.test(a.to || "")) {
      const old = Number(a.to) - 1;
      if (old >= 0 && old < order.length) a.to = String(newIndex(old) + 1);
    }
    return a;
  };
  S.cfg.pages = order.map((i) => S.cfg.pages[i]);
  for (const p of S.cfg.pages) for (const d of p.keys) { fix(d.action); if (d.toggle) fix(d.toggle.action); }
  // Les aperçus suivent leur page.
  const prev = S.previews; S.previews = {};
  for (const [key, v] of Object.entries(prev)) {
    const [p, ...rest] = key.split(":");
    S.previews[[newIndex(Number(p)), ...rest].join(":")] = v;
  }
  S.page = newIndex(S.page);
  S.devicePage = newIndex(S.devicePage);
  changed({ immediate: true });
  invoke("show_page", { index: S.page });
  renderPages();
}

// --- 🎮 ---
// --- Easter egg : Doom sur le MasterHUB (taper IDDQD dans la fenêtre) ---
const DOOM_KEYS = {
  ArrowUp: 0xad, z: 0xad, w: 0xad, ArrowDown: 0xaf, s: 0xaf, ArrowLeft: 0xac, ArrowRight: 0xae,
  q: 0xa0, a: 0xa0, d: 0xa1, " ": 0xa2, Control: 0xa3, f: 0xa3, Shift: 0x80 + 0x36, Escape: 27, Enter: 13, Tab: 9,
  e: 0x5d, r: 0x5b, y: 0x79, n: 0x6e, 1: 0x31, 2: 0x32, 3: 0x33, 4: 0x34, 5: 0x35, 6: 0x36, 7: 0x37,
};

function setupDoom() {
  const code = "iddqd";
  let pos = 0;
  document.addEventListener("keydown", (e) => {
    if (S.status.doom) return;
    const tag = (e.target.tagName || "").toLowerCase();
    if (tag === "input" || tag === "textarea" || S.capturing) return;
    const k = e.key.toLowerCase();
    pos = k === code[pos] ? pos + 1 : k === code[0] ? 1 : 0;
    if (pos === code.length) { pos = 0; launchDoom(); }
  });
  const send = (e, pressed) => {
    if (!S.status.doom || e.repeat) { if (S.status.doom) e.preventDefault(); return; }
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    const c = DOOM_KEYS[k];
    if (c === undefined) return;
    e.preventDefault();
    e.stopPropagation();
    invoke("doom_key", { pressed, code: c });
  };
  window.addEventListener("keydown", (e) => send(e, true), true);
  window.addEventListener("keyup", (e) => send(e, false), true);
  $("#doom-quit").onclick = () => invoke("stop_doom");
}

async function launchDoom(wad) {
  if (!S.status.modules.some((m) => m.kind === "keys15")) { toast("👹 Branche le module 15 touches pour lancer Doom…", true); return; }
  try {
    const name = await invoke("start_doom", { wad: wad || null });
    toast(`👹 Chargement de Doom (${name})…`);
  } catch (e) {
    if (String(e) !== "no_wad") { toast(String(e), true); return; }
    openDialog({
      title: "Doom a besoin d'un fichier de jeu",
      sub: "Le moteur est intégré à HubForge, mais pas les niveaux",
      html: `<p><b>Freedoom</b> est un jeu libre et gratuit compatible avec Doom (licence BSD) : HubForge peut le télécharger pour toi depuis GitHub (24 Mo, une seule fois).</p>
        <p>Tu peux aussi utiliser ton propre <b>DOOM.WAD</b> ou <b>DOOM2.WAD</b> (version Steam ou GOG de Doom / Doom II).</p>`,
      icon: `<div class="dialog-glyph doom-glyph">👹</div>`,
      actions: [
        el("button", { class: "btn ghost small", onclick: async () => { const p = await invoke("pick_file", { kind: "wad" }); if (p) { closeDialog(); launchDoom(p); } } }, "Choisir un fichier WAD…"),
        el("span", { class: "spacer" }),
        el("button", { class: "btn", onclick: closeDialog }, "Annuler"),
        el("button", { class: "btn primary", onclick: downloadFreedoom }, "Télécharger Freedoom"),
      ],
    });
  }
}

async function downloadFreedoom() {
  $("#dialog-progress").classList.remove("hidden");
  $("#dialog-bar").style.width = "0%";
  $("#dialog-progress-text").textContent = "Connexion à GitHub…";
  $("#dialog-actions").querySelectorAll("button").forEach((b) => (b.disabled = true));
  try {
    await invoke("download_freedoom");
    closeDialog();
    toast("👹 Freedoom téléchargé. Chargement…");
  } catch (e) {
    $("#dialog-progress").classList.add("hidden");
    $("#dialog-actions").querySelectorAll("button").forEach((b) => (b.disabled = false));
    toast(String(e), true);
  }
}

/** Affiche / masque la fenêtre de Doom selon l'état du moteur. */
function updateDoomView(prev, st) {
  if (st.doom_error && st.doom_error !== prev.doom_error) toast("Doom : " + st.doom_error, true);
  // Le jeu s'affiche sur les touches ; la fenêtre ne montre que les commandes.
  $("#doom").classList.toggle("hidden", !st.doom);
}

function setupEasterEgg() {
  const code = ["ArrowUp", "ArrowUp", "ArrowDown", "ArrowDown", "ArrowLeft", "ArrowRight", "ArrowLeft", "ArrowRight", "b", "a"];
  let pos = 0;
  document.addEventListener("keydown", (e) => {
    const tag = (e.target.tagName || "").toLowerCase();
    if (tag === "input" || tag === "textarea" || S.capturing) return;
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    pos = k === code[pos] ? pos + 1 : k === code[0] ? 1 : 0;
    if (pos === code.length) { pos = 0; launchGame(); }
  });
  let clicks = 0, timer = null;
  $(".brand-logo").addEventListener("click", () => {
    clicks++;
    clearTimeout(timer);
    timer = setTimeout(() => { clicks = 0; }, 1500);
    if (clicks >= 7) { clicks = 0; launchGame(); }
  });
}

function launchGame() {
  if (!S.status.modules.some((m) => m.kind === "keys15")) { toast("🎮 Branche le module 15 touches pour jouer…", true); return; }
  invoke("start_game");
  toast("🎮 Tape-taupe ! Tape les taupes sur le MasterHUB (dorée = +3, bombe = −3).");
}

// --- Réglages généraux ---
function settingsInspector() {
  const g = S.cfg.general;
  const out = [title("Réglages généraux")];
  out.push(rangeField("Luminosité des écrans", g.brightness, 0, 100, " %", (v) => { g.brightness = v; changed(); }));
  out.push(rangeField("Luminosité des LED", g.led_brightness, 0, 100, " %", (v) => { g.led_brightness = v; changed(); }));
  out.push(el("button", { class: "btn", style: "width:100%;margin-bottom:6px", onclick: () => select({ type: "base" }) }, "Base : LED, orientation, positions…"));
  out.push(el("button", { class: "btn", style: "width:100%;margin-bottom:6px", onclick: () => select({ type: "lights" }) }, "Animations…"));
  out.push(el("p", { class: "hint", style: "margin-top:0" }, "Astuce : tu peux aussi cliquer directement sur la base dans le dessin."));
  out.push(el("div", { class: "sep" }));
  out.push(switchRow("Assombrir la touche pendant l'appui", g.press_feedback, (v) => { g.press_feedback = v; changed(); }));
  out.push(rangeField("Durée d'un appui long", g.long_press_ms, 200, 1500, " ms", (v) => { g.long_press_ms = v; changed(); }));
  out.push(rangeField("Délai d'un double appui", g.double_press_ms, 150, 600, " ms", (v) => { g.double_press_ms = v; changed(); },
    "Temps maximal entre les deux appuis. Plus il est court, plus l'appui simple des touches à double appui est rapide."));
  out.push(switchRow("Pages automatiques selon l'application", g.auto_pages, (v) => { g.auto_pages = v; changed(); }));
  out.push(el("p", { class: "hint", style: "margin-top:-4px" }, "Lie une page à une application avec le bouton ⚙ de l'onglet de la page."));
  out.push(rangeField("Fluidité max. des GIF", g.gif_max_fps, 5, 40, " i/s", (v) => { g.gif_max_fps = v; changed(); },
    "Partagée entre toutes les touches animées."));
  out.push(el("div", { class: "sep" }));
  out.push(switchRow("Démarrer avec Windows", S.autostart, async (v) => {
    try { S.autostart = await invoke("set_autostart", { on: v }); } catch (e) { toast(String(e), true); }
    renderInspector();
  }));
  out.push(el("p", { class: "hint" }, "Fermer cette fenêtre ne coupe pas le MasterHUB : HubForge reste dans la zone de notification, près de l'horloge."));
  out.push(el("div", { class: "sep" }));
  out.push(el("button", { class: "btn", style: "width:100%", onclick: () => invoke("open_config_folder") }, "Ouvrir le dossier de configuration"));
  out.push(el("p", { class: "hint" }, S.configPath || ""));
  out.push(el("button", { class: "btn", style: "width:100%", onclick: showDiagnostic }, "Diagnostic et reconnexion des modules…"));
  out.push(...updateSettings());
  out.push(...aboutSection());
  return out;
}

// --- Mises à jour ---
function renderUpdatePill() {
  const pill = $("#update-pill");
  const a = S.update && S.update.available;
  pill.classList.toggle("hidden", !a);
  if (a) pill.textContent = `Mise à jour ${a.version}`;
}

function formatDate(iso) {
  if (!iso) return "";
  try { return new Date(iso).toLocaleDateString("fr-FR", { day: "numeric", month: "long", year: "numeric" }); } catch (_) { return ""; }
}

function timeAgo(secs) {
  if (!secs) return "jamais";
  const d = Math.max(0, Date.now() / 1000 - secs);
  if (d < 90) return "à l'instant";
  if (d < 3600) return `il y a ${Math.round(d / 60)} min`;
  if (d < 86400) return `il y a ${Math.round(d / 3600)} h`;
  return `il y a ${Math.round(d / 86400)} j`;
}

/** Markdown simple (titres, listes, gras, code, liens) → HTML sûr. */
function miniMarkdown(md) {
  const esc = (t) => t.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const inline = (t) => esc(t)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
    .replace(/\[([^\]]+)\]\((https:\/\/github\.com\/[^)\s]+)\)/g, '<a href="#" data-url="$2">$1</a>');
  const out = [];
  let list = false;
  for (const raw of (md || "").split(/\r?\n/)) {
    const line = raw.trimEnd();
    const li = line.match(/^\s*[-*]\s+(.*)$/);
    if (li) { if (!list) { out.push("<ul>"); list = true; } out.push(`<li>${inline(li[1])}</li>`); continue; }
    if (list) { out.push("</ul>"); list = false; }
    const h = line.match(/^#{1,6}\s+(.*)$/);
    if (h) out.push(`<h4>${inline(h[1])}</h4>`);
    else if (line.trim()) out.push(`<p>${inline(line)}</p>`);
  }
  if (list) out.push("</ul>");
  return out.join("");
}

function brandSvg() { return $(".brand-logo").outerHTML.replace('class="brand-logo"', ""); }

function openDialog({ title, sub, html, actions, icon }) {
  $("#dialog-title").textContent = title;
  $("#dialog-sub").textContent = sub || "";
  $("#dialog-icon").innerHTML = icon || brandSvg();
  const body = $("#dialog-body");
  body.innerHTML = html || "";
  body.querySelectorAll("a[data-url]").forEach((a) => { a.onclick = (e) => { e.preventDefault(); invoke("open_url", { url: a.dataset.url }); }; });
  const act = $("#dialog-actions");
  act.innerHTML = "";
  act.append(...actions);
  $("#dialog-progress").classList.add("hidden");
  $("#dialog").classList.remove("hidden");
}
function closeDialog() { if (!S.updating) $("#dialog").classList.add("hidden"); }

function showUpdateDialog() {
  const a = S.update && S.update.available;
  if (!a) return;
  const size = a.asset_size ? ` · ${(a.asset_size / 1048576).toFixed(1).replace(".", ",")} Mo` : "";
  openDialog({
    title: `HubForge ${a.version} est disponible`,
    sub: `Tu as la version ${a.current}${a.published ? " · publiée le " + formatDate(a.published) : ""}${size}`,
    html: miniMarkdown(a.notes) || "<p>Pas de notes de version.</p>",
    actions: [
      el("button", { class: "btn ghost small", onclick: async () => { S.update = await invoke("skip_update", { version: a.version }); renderUpdatePill(); closeDialog(); toast(`La version ${a.version} ne sera plus proposée.`); } }, "Ignorer cette version"),
      el("button", { class: "btn ghost small", onclick: () => invoke("open_url", { url: a.page_url }) }, "Voir sur GitHub"),
      el("span", { class: "spacer" }),
      el("button", { class: "btn", onclick: closeDialog }, "Plus tard"),
      el("button", { class: "btn primary", id: "do-update", onclick: runUpdate }, "Mettre à jour maintenant"),
    ],
  });
}

async function runUpdate() {
  S.updating = true;
  $("#dialog-progress").classList.remove("hidden");
  $("#dialog-progress-text").textContent = "Téléchargement…";
  $("#dialog-actions").querySelectorAll("button").forEach((b) => { b.disabled = true; });
  try {
    await invoke("install_update");
    $("#dialog-progress-text").textContent = "Installation… HubForge va redémarrer tout seul dans quelques secondes.";
    $("#dialog-bar").style.width = "100%";
  } catch (e) {
    S.updating = false;
    $("#dialog-progress-text").textContent = "Échec : " + e;
    $("#dialog-actions").querySelectorAll("button").forEach((b) => { b.disabled = false; });
  }
}

function updateProgress(p) {
  if (!p || !p.total) return;
  const pct = Math.min(100, Math.round((p.done / p.total) * 100));
  $("#dialog-bar").style.width = pct + "%";
  $("#dialog-progress-text").textContent = `Téléchargement… ${pct} % (${(p.done / 1048576).toFixed(1).replace(".", ",")} / ${(p.total / 1048576).toFixed(1).replace(".", ",")} Mo)`;
}

function showWhatsNew(version, notes) {
  openDialog({
    title: `Bienvenue dans HubForge ${version}`,
    sub: "Voici les nouveautés de cette version",
    html: miniMarkdown(notes),
    actions: [el("button", { class: "btn primary", onclick: closeDialog }, "C'est parti")],
  });
}

function updateSettings() {
  const u = S.update || {};
  const out = [el("div", { class: "sep" }), el("h2", {}, "Mises à jour")];
  let status;
  if (u.checking) status = el("div", { class: "update-status" }, "Vérification en cours…");
  else if (u.available) status = el("div", { class: "update-status new" }, "Version ", el("b", {}, u.available.version), " disponible (tu as la ", u.current || S.version, ").");
  else if (u.error) status = el("div", { class: "update-status" }, "Dernière vérification impossible : " + u.error);
  else status = el("div", { class: "update-status ok" }, "HubForge est à jour · vérifié " + timeAgo(u.last_check));
  out.push(status);
  out.push(el("div", { class: "row" },
    el("button", { class: "btn grow", disabled: !!u.checking, onclick: async () => {
      S.update = Object.assign({}, S.update, { checking: true }); renderInspector();
      try { S.update = await invoke("check_updates_now"); } catch (e) { toast(String(e), true); }
      renderUpdatePill(); renderInspector();
      if (S.update.available) showUpdateDialog(); else if (!S.update.error) toast("HubForge est à jour.");
    } }, "Vérifier maintenant"),
    u.available ? el("button", { class: "btn primary grow", onclick: showUpdateDialog }, "Voir la mise à jour") : null));
  out.push(el("div", { style: "height:10px" }));
  out.push(switchRow("Vérifier automatiquement (une fois par jour)", u.auto !== false, async (v) => {
    S.update = await invoke("set_update_auto", { on: v }); renderInspector();
  }));
  out.push(el("p", { class: "hint" }, "HubForge regarde seulement s'il existe une nouvelle version sur GitHub. Rien n'est installé sans ton accord, et tout fonctionne sans Internet."));
  return out;
}

function aboutSection() {
  const repo = "https://github.com/Kitcut40/hubforge";
  const link = (label, url) => el("button", { class: "btn small", onclick: () => invoke("open_url", { url }) }, label);
  const box = el("div", { class: "about", html: brandSvg() });
  box.append(el("div", {},
    el("div", { class: "about-name", html: 'Hub<span class="accent">Forge</span>' }),
    el("div", { class: "about-meta" }, `Version ${S.version || ""} · licence GPL-3.0`),
    el("div", { class: "about-meta" }, "Logiciel libre pour le Cooler Master MasterHUB")));
  return [
    el("div", { class: "sep" }),
    el("h2", {}, "À propos"),
    box,
    el("div", { class: "links" },
      link("Site du projet", repo),
      link("Nouveautés", repo + "/blob/main/CHANGELOG.md"),
      link("Guide des addons", repo + "/blob/main/docs/ADDON_GUIDE.md"),
      link("Signaler un problème", repo + "/issues")),
    el("p", { class: "hint" }, "HubForge n'est pas affilié à Cooler Master. MasterHUB est une marque de Cooler Master. Aucun firmware n'est modifié."),
  ];
}

function switchRow(label, value, onChange) {
  const input = el("input", { type: "checkbox", checked: !!value });
  input.onchange = () => onChange(input.checked);
  return el("label", { class: "switch" }, el("span", {}, label), input, el("span", { class: "knob" }));
}

function rangeField(label, value, min, max, unit, onChange, hint) {
  const val = el("span", { class: "val" }, value + unit);
  const r = el("input", { type: "range", min, max, value });
  r.oninput = () => { val.textContent = r.value + unit; onChange(Number(r.value)); };
  return field(label, el("div", { class: "range-row" }, r, val), hint ? el("p", { class: "hint" }, hint) : null);
}

// ---------------------------------------------------------------------------
// Bibliothèque
// ---------------------------------------------------------------------------
// Dossiers ouverts / fermés de la bibliothèque (retenus d'une fois sur l'autre).
const LIB_STATE_KEY = "hubforge.library.folders";
function libState() {
  if (!S.libState) {
    try { S.libState = JSON.parse(localStorage.getItem(LIB_STATE_KEY) || "{}"); } catch (_) { S.libState = {}; }
  }
  return S.libState;
}
function saveLibState() {
  try { localStorage.setItem(LIB_STATE_KEY, JSON.stringify(S.libState || {})); } catch (_) { /* rien */ }
}

/** Élément de la bibliothèque (glissable sur une touche, double-clic = touche sélectionnée). */
function libItem(it, path) {
  const sw = el("span", { class: "lib-swatch", style: `background:${it.color};color:#fff`, html: it.preview ? (it.preview.svg || "") : svgIcon(it.icon) });
  if (it.preview && it.preview.src) sw.append(el("img", { src: it.preview.src, alt: "" }));
  const node = el("div", { class: "lib-item", draggable: "true", title: it.description || "", "data-search": (it.name + " " + path).toLowerCase() }, sw, el("span", {}, it.name));
  const payload = () => {
    const p = clone(it);
    if (it.kind === "path") p.action._kind = "path";
    delete p.preview;
    return JSON.stringify(p);
  };
  node.ondragstart = (e) => { e.dataTransfer.setData("text/hubforge-action", payload()); e.dataTransfer.effectAllowed = "copy"; };
  node.ondblclick = () => {
    if (S.sel.type !== "key") { toast("Sélectionne d'abord une touche, ou glisse l'action dessus."); return; }
    onDropOnKey(S.sel.key, { getData: (t) => (t === "text/hubforge-action" ? payload() : "") });
  };
  return node;
}

/** Dossier repliable de la bibliothèque. `level` 0 = racine. */
function libFolder({ key, title, icon, count, level, defaultOpen, cls, children }) {
  const st = libState();
  const open = key in st ? st[key] : defaultOpen;
  const body = el("div", { class: "lib-folder-body" }, ...children);
  const head = el("div", { class: "lib-folder-title" },
    el("span", { class: "caret" }, "▾"),
    icon || null,
    el("span", { class: "grow" }, title),
    count ? el("span", { class: "lib-count" }, String(count)) : null);
  const f = el("div", { class: `lib-folder level-${Math.min(level, 3)}` + (cls ? " " + cls : "") + (open ? "" : " closed"), "data-key": key }, head, body);
  head.onclick = () => {
    f.classList.toggle("closed");
    libState()[key] = !f.classList.contains("closed");
    saveLibState();
  };
  return f;
}

/** Arborescence d'un addon : ses dossiers (et sous-dossiers), puis ses actions sans dossier. */
function addonFolder(ad) {
  const a = ad.addon;
  const toItem = (x) => ({
    name: x.name, color: x.color || "#37474F", preview: x.preview, description: x.description,
    addonKey: { label: x.label, icon: x.icon || undefined, color: x.color || undefined, text_color: x.text_color || undefined, action: x.action },
  });
  const known = new Set(a.categories.map((c) => c.id));
  const countIn = (cid) => {
    const subs = a.categories.filter((c) => c.parent === cid).map((c) => c.id);
    return ad.actions.filter((x) => x.category === cid).length + subs.reduce((n, sid) => n + countIn(sid), 0);
  };
  const build = (parent, level, path) => {
    const out = [];
    for (const c of a.categories.filter((c) => (c.parent || null) === parent)) {
      const n = countIn(c.id);
      if (!n) continue;
      const p = `${path} ${c.name}`;
      out.push(libFolder({
        key: `addon:${ad.folder}/${c.id}`, title: c.name, count: n, level, defaultOpen: false,
        children: [...build(c.id, level + 1, p), ...ad.actions.filter((x) => x.category === c.id).map((x) => libItem(toItem(x), p))],
      }));
    }
    return out;
  };
  const loose = ad.actions.filter((x) => !x.category || !known.has(x.category));
  return libFolder({
    key: `addon:${ad.folder}`, title: a.name, icon: addonIcon(ad, 20), count: ad.actions.length, level: 0, defaultOpen: false, cls: "addon-root",
    children: [...build(null, 1, a.name), ...loose.map((x) => libItem(toItem(x), a.name))],
  });
}

function buildLibrary() {
  const box = $("#library");
  box.innerHTML = "";
  // Un dossier par addon activé, en premier.
  const addons = (S.addons || []).filter((x) => x.enabled && x.addon && x.actions.length);
  if (addons.length) {
    box.append(el("div", { class: "lib-section" }, "Addons"));
    for (const ad of addons) box.append(addonFolder(ad));
    box.append(el("div", { class: "lib-section" }, "Actions de base"));
  }
  const cats = clone(LIBRARY);
  // Débloqué après une première partie 😉
  if (S.gameBest > 0) cats.find((c) => c.cat === "HubForge").items.push(
    { name: "Mini-jeu tape-taupe", label: "Jouer", icon: "gamepad-2", color: "#2E7D32", action: { type: "game" } });
  for (const cat of cats) {
    box.append(libFolder({
      key: `cat:${cat.cat}`, title: cat.cat, level: 0, defaultOpen: true,
      children: cat.items.map((it) => libItem(it, cat.cat)),
    }));
  }
  const q = $("#lib-search").value || "";
  if (q.trim()) filterLibrary(q);
}

/** Recherche : n'affiche que les actions trouvées et ouvre leurs dossiers ;
    sans recherche, chaque dossier retrouve son état. */
function filterLibrary(q) {
  q = q.trim().toLowerCase();
  document.querySelectorAll(".lib-item").forEach((n) => n.classList.toggle("hidden", !!q && !n.dataset.search.includes(q)));
  const folders = [...document.querySelectorAll(".lib-folder")].reverse(); // les plus profonds d'abord
  const st = libState();
  for (const f of folders) {
    if (q) {
      const any = [...f.querySelectorAll(".lib-item")].some((n) => !n.classList.contains("hidden"));
      f.classList.toggle("hidden", !any);
      f.classList.toggle("closed", !any);
    } else {
      f.classList.remove("hidden");
      const key = f.dataset.key;
      const def = !(key.startsWith("addon:"));
      f.classList.toggle("closed", !(key in st ? st[key] : def));
    }
  }
  document.querySelectorAll(".lib-section").forEach((s) => s.classList.toggle("hidden", !!q));
}

// ---------------------------------------------------------------------------
// État en direct (envoyé par le moteur)
// ---------------------------------------------------------------------------
function onStatus(st) {
  const prev = S.status;
  if (st.profile && S.profile && st.profile !== S.profile) {
    S.status = st;
    reloadProfile();
    return;
  }
  if (st.game !== prev.game) {
    const stage = $(".stage");
    const b = stage.querySelector(".game-banner");
    if (st.game && !b) stage.append(el("div", { class: "game-banner" }, "🎮 Partie de tape-taupe en cours sur le MasterHUB…"));
    if (!st.game && b) { b.remove(); invoke("get_initial").then((ini) => { if (ini.game_best > (S.gameBest || 0)) toast(`🏆 Nouveau record : ${ini.game_best} points !`); S.gameBest = ini.game_best; buildLibrary(); }); }
  }
  const sig = (s) => JSON.stringify((s.modules || []).map((m) => [m.kind, m.port]));
  const layoutChanged = sig(prev) !== sig(st);
  const pageChanged = st.page !== S.devicePage;
  const errChanged = st.config_error !== prev.config_error;
  S.status = st;
  if (layoutChanged) { renderModulesChips(); renderDevice(); fitDevice(); }
  if (errChanged) renderBanner();
  updateFaderLive();
  updateLedLive();
  updateDoomView(prev, st);
  if (S.layout && st.base_rot !== undefined && st.base_rot !== S.layout.base_rot) { S.layout.base_rot = st.base_rot; renderDevice(); fitDevice(); }
  if (pageChanged) {
    S.devicePage = st.page;
    // La page a changé depuis le MasterHUB : l'interface suit.
    if (S.page !== st.page && st.page < S.cfg.pages.length) { S.page = st.page; renderKeys(); refreshPreviews(S.page); if (S.sel.type === "key") renderInspector(); }
    renderPages();
  }
  if (S.devicePage === S.page && !layoutChanged) {
    for (let i = 0; i < 15; i++) {
      const wasT = !!(prev.toggled && prev.toggled[i]), isT = !!(st.toggled && st.toggled[i]);
      if (wasT !== isT) { updateKeyNode(i + 1); continue; }
      if (!!(prev.pressed && prev.pressed[i]) !== !!(st.pressed && st.pressed[i])) {
        const n = document.querySelector(`#keys .key[data-key="${i + 1}"]`);
        if (n) n.classList.toggle("pressed", !!st.pressed[i]);
      }
    }
  }
}

function onGlobalKey(e) {
  if (S.capturing) return;
  const tag = (e.target.tagName || "").toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return;
  const ctrl = e.ctrlKey || e.metaKey;
  const k = e.key.toLowerCase();
  if (ctrl && k === "z" && !e.shiftKey) { e.preventDefault(); undo(); return; }
  if (ctrl && (k === "y" || (k === "z" && e.shiftKey))) { e.preventDefault(); redo(); return; }
  if (ctrl && k === "c" && S.sel.type === "key") { e.preventDefault(); copyKey(S.sel.key); return; }
  if (ctrl && k === "v" && S.sel.type === "key") { e.preventDefault(); pasteKey(S.sel.key); return; }
  if (e.key === "Escape") select({ type: "settings" });
  if ((e.key === "Delete" || e.key === "Backspace") && S.sel.type === "key" && keyDef(S.sel.key)) {
    const k = S.sel.key;
    confirm_(`Vider la touche ${k} ?`).then((ok) => { if (!ok) return; setKeyDef(k, null); updateKeyNode(k); changed(); renderInspector(); });
  }
}

// ---------------------------------------------------------------------------
// Petites boîtes de dialogue
// ---------------------------------------------------------------------------
function modal(text, withInput, value) {
  return new Promise((resolve) => {
    const m = $("#modal"), input = $("#modal-input");
    $("#modal-text").textContent = text;
    input.classList.toggle("hidden", !withInput);
    input.value = value || "";
    m.classList.remove("hidden");
    if (withInput) setTimeout(() => { input.focus(); input.select(); }, 10);
    const done = (v) => { m.classList.add("hidden"); $("#modal-ok").onclick = $("#modal-cancel").onclick = input.onkeydown = null; resolve(v); };
    $("#modal-ok").onclick = () => done(withInput ? input.value.trim() : true);
    $("#modal-cancel").onclick = () => done(withInput ? null : false);
    input.onkeydown = (e) => { if (e.key === "Enter") done(input.value.trim()); if (e.key === "Escape") done(null); };
  });
}
const prompt_ = (t, v) => modal(t, true, v);
const confirm_ = (t) => modal(t, false);

let toastTimer = null;
function toast(msg, isError, action) {
  const t = $("#toast");
  t.innerHTML = "";
  t.append(document.createTextNode(msg));
  if (action) t.append(el("span", { class: "toast-action", onclick: () => { t.classList.add("hidden"); action.fn(); } }, action.label));
  t.className = "toast" + (isError ? " error" : "");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add("hidden"), isError || action ? 6000 : 2500);
}

// ---------------------------------------------------------------------------
// Simulation (ouverture dans un navigateur, pour le développement)
// ---------------------------------------------------------------------------
window.__mockUpdate = () => ({ current: "0.8.0", checking: false, auto: true, last_check: Date.now() / 1000 - 600, error: null,
  available: location.search.includes("update") ? { version: "0.8.1", current: "0.8.0", name: "HubForge 0.8.1", published: "2026-11-08T10:00:00Z",
    page_url: "https://github.com/Kitcut40/hubforge/releases/tag/v0.8.1", asset_name: "HubForge-Setup-0.8.1.exe", asset_size: 6340000,
    notes: "### Nouveautés\n- **Appui long** sur les touches\n- Intégration `OBS` avec retour d'état\n\n### Corrections\n- Le module 15 touches revient seul après un rebranchement" } : null });

async function mockInvoke(cmd, args) {
  if (!window.__mock) {
    const cfg = await fetch("mock-config.json").then((r) => r.json()).catch(() => ({ pages: [{ name: "Accueil", keys: [] }] }));
    const icons = await fetch("mock-icons.json").then((r) => r.json()).catch(() => []);
    window.__mock = { cfg, icons };
  }
  const rotated = location.search.includes("layout=2");
  const mods = rotated
    ? [{ kind: "keys15", port: "134", placement: { col: 1, row: 0, rot: 270 } }, { kind: "faders5", port: "122", placement: { col: 0, row: 4, rot: 0 } },
       { kind: "rollers2", port: "141", placement: { col: 0, row: 0, rot: 270 } }, { kind: "base" }]
    : [{ kind: "keys15", port: "24", placement: { col: 0, row: 0, rot: 0 } }, { kind: "faders5", port: "41", placement: { col: 0, row: 3, rot: 0 } },
       { kind: "rollers2", port: "121", placement: { col: 0, row: 5, rot: 0 } }, { kind: "base" }];
  switch (cmd) {
    case "get_layout": return window.__mockLayout || (window.__mockLayout = { base_rot: location.search.includes("rot=90") ? 90 : 0, placements: {} });
    case "set_base_rot": window.__mockLayout.base_rot = args.rot; return null;
    case "set_placement": return null;
    case "preview_animation": return null;
    case "start_doom": if (location.search.includes("doom")) { setTimeout(() => onStatus(Object.assign({}, S.status, { doom: true })), 300); return "freedoom1.wad"; } throw "no_wad";
    case "download_freedoom": return null;
    case "stop_doom": onStatus(Object.assign({}, S.status, { doom: false })); return null;
    case "list_profiles": return ["Benjamin", "Laura", "Streaming"];
    case "list_backups": return [0, 1, 2].map((i) => ({ file: `${1794000000 - i * 3600}.toml`, time: Date.now() / 1000 - i * 5400 - 300, size: 4000, pages: 3, keys: 30 - i * 2 }));
    case "get_diagnostic": return { version: "0.8.1", os: "windows x86_64", profile: "Benjamin", config_dir: "C:\\Users\\benja\\AppData\\Roaming\\HubForge",
      status: { modules: [{ kind: "keys15", serial: "MHKD15AA431251602923", port: "24", placement: { col: 0, row: 0, rot: 0 } }, { kind: "faders5", serial: "MHFD05AA421251700387", port: "41", placement: { col: 0, row: 3, rot: 0 } }] },
      addons: ["fusion360 1.1.0", "discord 1.0.0"], log: "[20:12:01] HubForge v0.8.1 démarre\n[20:12:02] ✓ 15 touches connecté\n[20:12:02] ✓ 5 faders connecté" };
    case "check_updates_now": case "get_update_state": return window.__mockUpdate();
    case "set_update_auto": return Object.assign(window.__mockUpdate(), { auto: args.on });
    case "skip_update": return Object.assign(window.__mockUpdate(), { available: null });
    case "list_addons": return await fetch("mock-addons.json").then((r) => r.json()).catch(() => ({ addons: [], signature: "", folder: "" }));
    case "addons_signature": return "x";
    case "addon_pages": return [{ name: "Fusion 360", apps: ["Fusion360.exe"], keys: [{ key: 1, label: "Esquisse", color: "#1565C0", action: { type: "hotkey", keys: "s" } }] }];
    case "get_initial":
      return { update: window.__mockUpdate(), whats_new: location.search.includes("new") ? "### Nouveautés\n- **Installateur Windows** : menu Démarrer, désinstallation propre.\n- **Mises à jour depuis GitHub** en un clic.\n- Double-clic sur un fichier `.forge`." : null, profile: "Benjamin", profiles: ["Benjamin", "Laura", "Streaming"], game_best: 0, config: window.__mock.cfg, config_path: "C:\\Users\\benja\\AppData\\Roaming\\HubForge\\config.toml", autostart: false, version: "0.8.1",
        status: { modules: mods, faders: [72, 35, 60, 18, 90], page: 0, pressed: Array(15).fill(false), toggled: Array(15).fill(false),
          leds: location.search.includes("leds") ? { base: "#FFC800", faders: ["#00E676", "#00E676", "#FFC800", "#FFC800", "#000000"] } : { base: null, faders: [] },
          sim: location.search.includes("leds") ? { name: "Assetto Corsa", live: true, rpm: 7120, max_rpm: 8000, flag: 2 } : null,
          base_rot: location.search.includes("rot=90") ? 90 : 0 } };
    case "render_keys": return args.keys.map((d) => d && { src: mockPreview(d), animated: !!(d.icon && /\.gif$/i.test(d.icon)) });
    case "list_icons": return window.__mock.icons;
    case "list_audio_apps": return ["chrome.exe", "discord.exe", "spotify.exe"];
    case "list_running_apps": return ["chrome.exe", "explorer.exe", "fusion360.exe", "obs64.exe", "prusa-slicer.exe"];
    case "app_info": return { icon: null, name: (args.target.split(/[\\/]/).pop() || "").replace(/\.exe$/i, "") };
    default: return null;
  }
}

function mockPreview(d) {
  const bg = d.color || "#000";
  const demo = { clock: "15:49", date: "Samedi 7", cpu: "CPU 23 %", ram: "RAM 41 %", volume: "45 %", mic: "Micro" };
  const label = (d.widget ? demo[d.widget] : d.label || "").replace(/&/g, "&amp;").replace(/</g, "&lt;");
  let icon = "";
  if (d.icon && d.icon.startsWith("builtin:")) {
    const svg = svgIcon(d.icon.slice(8)).replace(/currentColor/g, d.text_color || "#fff").replace(/width="24"/, 'width="34"').replace(/height="24"/, 'height="34"');
    icon = `<g transform="translate(19,${label ? 6 : 19})">${svg.replace(/<\?xml[^>]*>/, "")}</g>`;
  }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72"><rect width="72" height="72" fill="${bg}"/>${icon}` +
    `<foreignObject x="3" y="${icon ? 44 : 3}" width="66" height="${icon ? 26 : 66}"><div xmlns="http://www.w3.org/1999/xhtml" style="height:100%;display:flex;align-items:center;justify-content:center;text-align:center;font:600 ${icon ? 10 : 13}px Inter,sans-serif;color:#fff">${label}</div></foreignObject></svg>`;
  return "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
}

init().catch((e) => { document.body.innerHTML = `<pre style="padding:20px;color:#ffb3b5">Erreur au démarrage de l'interface :\n${e}\n${e.stack || ""}</pre>`; });
