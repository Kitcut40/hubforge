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
  { cat: "HubForge", items: [
    { name: "Page suivante", label: "Page suivante", icon: "arrow-right", color: "#212121", action: { type: "page", to: "next" } },
    { name: "Page précédente", label: "Page précédente", icon: "arrow-left", color: "#212121", action: { type: "page", to: "previous" } },
    { name: "Aller à la page 1", label: "Accueil", icon: "house", color: "#212121", action: { type: "page", to: "1" } },
    { name: "Ouvrir la configuration", label: "Config", icon: "file-cog", color: "#546E7A", action: { type: "open_config" } },
  ]},
];

const ACTION_TYPES = [
  ["none", "Aucune action"], ["hotkey", "Raccourci clavier"], ["text", "Taper un texte"],
  ["launch:program", "Lancer une application"], ["launch:url", "Ouvrir un site web"], ["launch:path", "Ouvrir un dossier ou fichier"],
  ["media", "Multimédia"], ["page", "Changer de page"], ["lock", "Verrouiller le PC"], ["open_config", "Ouvrir la configuration"],
  ["macro", "Macro (plusieurs actions à la suite)"],
];

const SWATCHES = ["#FF3B30", "#FF7A1A", "#FFB300", "#FFD60A", "#3DDC84", "#1DB954", "#00C8FF", "#1E88E5",
                  "#5865F2", "#8E24AA", "#E91E63", "#FFFFFF", "#607D8B", "#37474F", "#1F2229", "#000000"];

const FADER_TARGETS = [["master", "Volume général"], ["mic", "Micro"], ["app", "Application(s)"], ["none", "Rien"]];
const ROLLER_MODES = [["volume", "Volume général"], ["scroll", "Défilement vertical"], ["hscroll", "Défilement horizontal"], ["zoom", "Zoom (Ctrl + molette)"], ["none", "Rien"]];
const BASE_EFFECTS = [[1, "Fixe"], [2, "Respiration"], [3, "Stroboscope"], [5, "Cycle de couleurs"], [6, "Aléatoire"], [7, "Vague"], [8, "Eau"], [9, "Arc-en-ciel"], [0, "Éteint"]];

/* Taille des modules sur la base (en unités), dans leur sens normal. */
const MODULE_SIZE = { keys15: [4, 3], faders5: [4, 2], rollers2: [4, 1] };
/* Disposition d'origine du kit (touches en haut, faders, molettes en bas). */
const DEFAULT_PLACEMENT = { keys15: { col: 0, row: 0, rot: 0 }, faders5: { col: 0, row: 3, rot: 0 }, rollers2: { col: 0, row: 5, rot: 0 } };
const MODULE_NAMES = { keys15: "15xKey", faders5: "5xFader", rollers2: "2xRoller" };

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
  $("#version").textContent = "v" + ini.version;
  S.icons = (await invoke("list_icons")) || [];

  buildLibrary();
  renderAll();
  await refreshPreviews(S.page);

  listen("status", (e) => onStatus(e.payload));
  listen("autostart", (e) => { S.autostart = e.payload; if (S.sel.type === "settings") renderInspector(); });

  $("#btn-settings").onclick = () => select({ type: "settings" });
  $("#lib-search").oninput = (e) => filterLibrary(e.target.value);
  document.addEventListener("keydown", onGlobalKey, true);
  fitDevice();
  new ResizeObserver(fitDevice).observe($(".stage"));
}

function normalize(cfg) {
  cfg.general = Object.assign({ brightness: 80, led_brightness: 100, base_color: "#00C8FF", press_feedback: true, gif_max_fps: 28, base_effect: 1, auto_pages: true }, cfg.general || {});
  cfg.pages = cfg.pages && cfg.pages.length ? cfg.pages : [{ name: "Accueil", keys: [] }];
  cfg.faders = cfg.faders || [];
  while (cfg.faders.length < 5) cfg.faders.push({ target: "none", color: "#FFFFFF" });
  cfg.rollers = Object.assign({ right: "volume", left: "scroll", volume_step: 2, scroll_step: 3, invert_right: false, invert_left: false }, cfg.rollers || {});
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
    const tab = el("div", { class: "page-tab" + (i === S.page ? " active" : ""), title: "Double-clic pour renommer" },
      i === S.devicePage ? el("span", { class: "on-device", title: "Page affichée sur le MasterHUB" }) : null,
      el("span", {}, p.name || `Page ${i + 1}`),
      apps.length ? el("span", { class: "auto", title: "S'affiche toute seule avec : " + apps.join(", "), html: svgIcon("app-window") }) : null,
      i === S.page ? el("span", { class: "opts", title: "Réglages de la page", html: svgIcon("settings"),
        onclick: (e) => { e.stopPropagation(); select({ type: "page" }); } }) : null,
      S.cfg.pages.length > 1 ? el("span", { class: "close", title: "Supprimer la page", onclick: (e) => { e.stopPropagation(); deletePage(i); } }, "×") : null,
    );
    tab.onclick = () => goToPage(i);
    tab.ondblclick = () => renamePage(i);
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
  const c = S.cfg.general.base_color || "#00C8FF";
  base.append(el("div", { class: "base-glow", style: `box-shadow:0 0 0 2px ${c}55, 0 0 26px ${c}55` }));
  base.append(el("div", { class: "base-led-hit", title: "LED de la base", onclick: (e) => { e.stopPropagation(); select({ type: "settings" }); } }));
  for (const m of layout()) {
    const slot = el("div", { class: "slot", style: `left:${14 + m.box.col * U}px;top:${14 + m.box.row * U}px;width:${m.box.w * U}px;height:${m.box.h * U}px` });
    const mod = moduleElement(m.kind, !m.connected);
    // Les images des touches restent droites (HubForge les tourne aussi sur le MasterHUB).
    if (m.kind === "keys15") document.documentElement.style.setProperty("--key-rot", `${-(m.p.rot || 0)}deg`);
    mod.style.transform = `translate(-50%, -50%)` + (m.p.rot ? ` rotate(${m.p.rot}deg)` : "");
    slot.append(mod);
    base.append(slot);
  }
  updateFaderLive();
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
      : def && def.action && def.action.type === "macro" ? el("span", { class: "badge" }, "Macro")
      : def && prev && prev.animated ? el("span", { class: "badge" }, "GIF") : null,
  );
  node.onclick = (e) => { e.stopPropagation(); S.editState = 0; select({ type: "key", key: k }); };
  node.ondragstart = (e) => { e.dataTransfer.setData("text/hubforge-key", String(k)); e.dataTransfer.effectAllowed = "move"; node.classList.add("dragging"); };
  node.ondragend = () => node.classList.remove("dragging");
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
    const def = { key: k, label: item.label, color: item.color, icon: "builtin:" + item.icon, action: clone(item.action) };
    if (item.toggle) def.toggle = clone(item.toggle);
    setKeyDef(k, def);
    S.editState = 0;
    select({ type: "key", key: k });
    keyChanged(k, { immediate: true });
    if (item.appIcon) applyAppIcon(k, keyDef(k), item.action.target, { quiet: true, keepLabel: true });
    return;
  }
  if (dt.files && dt.files.length) {
    const f = dt.files[0];
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
  return S.cfg.faders.slice(0, 5).map((slot, i) => {
    const f = el("div", { class: "fader" + (S.sel.type === "fader" && S.sel.index === i ? " selected" : ""), "data-fader": i },
      el("div", { class: "fader-value" }, "–"),
      el("div", { class: "fader-track" },
        el("div", { class: "fader-led", style: `background:${slot.color};box-shadow:0 0 8px ${slot.color}` }),
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

function rollerElements() {
  const mode = (m) => (ROLLER_MODES.find(([v]) => v === m) || [0, "—"])[1].split(" (")[0];
  return [["left", "Gauche"], ["right", "Droite"]].map(([side, label]) =>
    el("div", { class: "roller", title: `Molette ${label.toLowerCase()} : ${mode(S.cfg.rollers[side])}` },
      el("div", { class: "roller-drum" }),
      el("div", { class: "roller-name" }, `${label} · ${mode(S.cfg.rollers[side])}`)));
}

// ---------------------------------------------------------------------------
// Sélection et inspecteur
// ---------------------------------------------------------------------------
function select(sel) {
  S.sel = sel;
  S.capturing = false;
  S.iconPickerOpen = false;
  renderDevice();
  $("#btn-settings").classList.toggle("active", sel.type === "settings");
  renderInspector();
}

function renderInspector() {
  const box = $("#inspector");
  box.innerHTML = "";
  if (S.sel.type === "key") box.append(...keyInspector(S.sel.key));
  else if (S.sel.type === "fader") box.append(...faderInspector(S.sel.index));
  else if (S.sel.type === "rollers") box.append(...rollersInspector());
  else if (S.sel.type === "page") box.append(...pageInspector());
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

  out.push(field("Texte affiché", el("input", { id: "label-input", class: "input", value: st.label || "", placeholder: "(aucun)", oninput: (e) => upd((d) => { d.label = e.target.value; delete d._autoName; }) })));
  out.push(iconField(k, st, upd));
  out.push(colorField("Couleur de fond", st.color, (v) => upd((d) => { if (v) d.color = v; else delete d.color; })));
  out.push(colorField("Couleur du texte et de l'icône", st.text_color || "#FFFFFF", (v) => upd((d) => { if (v) d.text_color = v; else delete d.text_color; })));

  out.push(el("div", { class: "sep" }));
  if (def.toggle) out.push(el("p", { class: "hint", style: "margin:-6px 0 12px" }, S.editState === 0
    ? "Action lancée quand la touche est dans l'état 1 ; elle passe ensuite à l'état 2."
    : "Action lancée quand la touche est dans l'état 2 ; elle revient ensuite à l'état 1."));
  out.push(...actionEditor(st, k, () => keyChanged(k)));
  out.push(el("div", { class: "sep" }));
  out.push(el("button", { class: "btn danger", style: "width:100%", onclick: async () => {
    if (!(await confirm_(`Vider la touche ${k} ?`))) return;
    setKeyDef(k, null); delete S.previews[`${S.page}:${k}:0`]; delete S.previews[`${S.page}:${k}:1`]; updateKeyNode(k); changed(); renderInspector();
  } }, "Vider la touche"));
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
    default: return "Aucune action";
  }
}

/** Éditeur d'action pour `st.action` ; `onChange` est appelé à chaque modification.
    opts.nested : étape d'une macro (pas de titre, pas d'icône ni de nom automatiques). */
function actionEditor(st, k, onChange, opts = {}) {
  if (!st.action) st.action = { type: "none" };
  const a = st.action;
  const nested = !!opts.nested;
  const upd = (fn) => { fn(st.action); onChange(); };
  const types = nested
    ? [...ACTION_TYPES.filter(([v]) => v !== "macro"), ["wait", "Attendre (pause)"]]
    : ACTION_TYPES;
  const typeSel = el("select", { class: "input" }, ...types.map(([v, l]) => el("option", { value: v, selected: v === actionTypeId(a) }, l)));
  typeSel.onchange = () => {
    st.action = clone(ACTION_DEFAULTS[typeSel.value]);
    onChange();
    renderInspector();
  };
  const out = nested ? [field("Type", typeSel)] : [el("h2", {}, "Action"), field("Quand j'appuie sur la touche", typeSel)];
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
  }
  return out;
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
        el("button", { class: "icon-btn", title: "Monter", disabled: i === 0, onclick: () => move(-1) }, "↑"),
        el("button", { class: "icon-btn", title: "Descendre", disabled: i === steps.length - 1, onclick: () => move(1) }, "↓"),
        el("button", { class: "icon-btn danger", title: "Supprimer l'étape", onclick: () => { steps.splice(i, 1); changedAndRedraw(); } }, "×")),
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
  const slot = S.cfg.faders[i];
  const out = [title(`Fader ${i + 1}`, ["le plus à gauche", "", "au milieu", "", "le plus à droite"][i] || "")];
  const live = S.status.faders ? S.status.faders[i] + " %" : "non connecté";
  out.push(el("p", { class: "hint", style: "margin:-6px 0 14px" }, "Position actuelle : ", el("b", { id: "fader-live" }, live)));
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
  out.push(colorField("Couleur de la LED", slot.color, (v) => { if (!v) return; slot.color = v; changed(); renderDevice(); }));
  return out;
}

// --- Molettes ---
function rollersInspector() {
  const r = S.cfg.rollers;
  const out = [title("Molettes")];
  for (const [side, label] of [["left", "Molette de gauche"], ["right", "Molette de droite"]]) {
    out.push(el("h2", {}, label));
    out.push(field("Action", el("select", { class: "input", onchange: (e) => { r[side] = e.target.value; changed(); renderDevice(); } },
      ...ROLLER_MODES.map(([v, l]) => el("option", { value: v, selected: v === r[side] }, l)))));
    out.push(switchRow("Inverser le sens", r["invert_" + side], (v) => { r["invert_" + side] = v; changed(); }));
  }
  out.push(el("div", { class: "sep" }));
  out.push(rangeField("Volume par cran", r.volume_step, 1, 10, " %", (v) => { r.volume_step = v; changed(); }));
  out.push(rangeField("Lignes par cran (défilement)", r.scroll_step, 1, 10, "", (v) => { r.scroll_step = v; changed(); }));
  return out;
}

// --- Page ---
function pageInspector() {
  const p = currentPage();
  if (!p.apps) p.apps = [];
  const out = [title(`Page ${S.page + 1}`, "Réglages de la page")];
  out.push(field("Nom", el("input", { class: "input", value: p.name || "", oninput: (e) => { p.name = e.target.value; changed(); renderPagesSoon(); } })));

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
  out.push(el("button", { class: "btn", style: "width:100%", onclick: () => renamePage(S.page) }, "Renommer…"));
  return out;
}

let pagesTimer = null;
function renderPagesSoon() { clearTimeout(pagesTimer); pagesTimer = setTimeout(renderPages, 250); }

// --- Réglages généraux ---
function settingsInspector() {
  const g = S.cfg.general;
  const out = [title("Réglages généraux")];
  out.push(rangeField("Luminosité des écrans", g.brightness, 0, 100, " %", (v) => { g.brightness = v; changed(); }));
  out.push(rangeField("Luminosité des LED", g.led_brightness, 0, 100, " %", (v) => { g.led_brightness = v; changed(); }));
  out.push(el("h2", { style: "margin-top:6px" }, "LED de la base"));
  out.push(colorField("Couleur", g.base_color, (v) => { if (!v) return; g.base_color = v; renderDevice(); changed(); }));
  out.push(field("Effet", el("select", { class: "input", onchange: (e) => { g.base_effect = Number(e.target.value); changed(); } },
    ...BASE_EFFECTS.map(([v, l]) => el("option", { value: v, selected: v === g.base_effect }, l))),
    el("p", { class: "hint" }, "Les effets animés ne fonctionnent que sur la base ; les faders restent en couleur fixe.")));
  out.push(el("div", { class: "sep" }));
  out.push(switchRow("Assombrir la touche pendant l'appui", g.press_feedback, (v) => { g.press_feedback = v; changed(); }));
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
  return out;
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
function buildLibrary() {
  const box = $("#library");
  box.innerHTML = "";
  for (const cat of LIBRARY) {
    const items = el("div", { class: "lib-items" });
    for (const it of cat.items) {
      const sw = el("span", { class: "lib-swatch", style: `background:${it.color};color:#fff`, html: svgIcon(it.icon) });
      const node = el("div", { class: "lib-item", draggable: "true", "data-search": (it.name + " " + cat.cat).toLowerCase() }, sw, el("span", {}, it.name));
      const payload = () => {
        const p = clone(it);
        if (it.kind === "path") p.action._kind = "path";
        return JSON.stringify(p);
      };
      node.ondragstart = (e) => { e.dataTransfer.setData("text/hubforge-action", payload()); e.dataTransfer.effectAllowed = "copy"; };
      node.ondblclick = () => {
        if (S.sel.type !== "key") { toast("Sélectionne d'abord une touche, ou glisse l'action dessus."); return; }
        onDropOnKey(S.sel.key, { getData: (t) => (t === "text/hubforge-action" ? payload() : "") });
      };
      items.append(node);
    }
    const c = el("div", { class: "lib-cat" }, el("div", { class: "lib-cat-title" }, el("span", { class: "caret" }, "▾"), cat.cat), items);
    $(".lib-cat-title", c).onclick = () => c.classList.toggle("closed");
    box.append(c);
  }
}

function filterLibrary(q) {
  q = q.trim().toLowerCase();
  document.querySelectorAll(".lib-item").forEach((n) => n.classList.toggle("hidden", q && !n.dataset.search.includes(q)));
  document.querySelectorAll(".lib-cat").forEach((c) => {
    c.classList.remove("closed");
    const any = [...c.querySelectorAll(".lib-item")].some((n) => !n.classList.contains("hidden"));
    c.classList.toggle("hidden", !any);
  });
}

// ---------------------------------------------------------------------------
// État en direct (envoyé par le moteur)
// ---------------------------------------------------------------------------
function onStatus(st) {
  const prev = S.status;
  const sig = (s) => JSON.stringify((s.modules || []).map((m) => [m.kind, m.port]));
  const layoutChanged = sig(prev) !== sig(st);
  const pageChanged = st.page !== S.devicePage;
  const errChanged = st.config_error !== prev.config_error;
  S.status = st;
  if (layoutChanged) { renderModulesChips(); renderDevice(); fitDevice(); }
  if (errChanged) renderBanner();
  updateFaderLive();
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
function toast(msg, isError) {
  const t = $("#toast");
  t.textContent = msg;
  t.className = "toast" + (isError ? " error" : "");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add("hidden"), isError ? 6000 : 2500);
}

// ---------------------------------------------------------------------------
// Simulation (ouverture dans un navigateur, pour le développement)
// ---------------------------------------------------------------------------
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
    case "get_initial":
      return { config: window.__mock.cfg, config_path: "C:\\Users\\benja\\AppData\\Roaming\\HubForge\\config.toml", autostart: false, version: "0.4.1",
        status: { modules: mods, faders: [72, 35, 60, 18, 90], page: 0, pressed: Array(15).fill(false), toggled: Array(15).fill(false) } };
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
  const label = (d.label || "").replace(/&/g, "&amp;").replace(/</g, "&lt;");
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
