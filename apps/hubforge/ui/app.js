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
  play_pause: "Lecture / Pause", next: "Piste suivante", previous: "Piste précédente", stop: "Stop",
  mute: "Couper le son", volume_up: "Volume +", volume_down: "Volume −", mic_mute: "Couper le micro",
};

const LIBRARY = [
  { cat: "Clavier", items: [
    { name: "Raccourci clavier", label: "Raccourci", color: "#3949AB", action: { type: "hotkey", keys: "" } },
    { name: "Taper un texte", label: "Texte", color: "#EF6C00", action: { type: "text", text: "" } },
    { name: "Copier", label: "Copier", color: "#5D4037", action: { type: "hotkey", keys: "ctrl+c" } },
    { name: "Coller", label: "Coller", color: "#5D4037", action: { type: "hotkey", keys: "ctrl+v" } },
    { name: "Couper", label: "Couper", color: "#5D4037", action: { type: "hotkey", keys: "ctrl+x" } },
    { name: "Annuler", label: "Annuler", color: "#6D4C41", action: { type: "hotkey", keys: "ctrl+z" } },
    { name: "Rétablir", label: "Rétablir", color: "#6D4C41", action: { type: "hotkey", keys: "ctrl+y" } },
    { name: "Enregistrer", label: "Enregistrer", color: "#2E7D32", action: { type: "hotkey", keys: "ctrl+s" } },
    { name: "Tout sélectionner", label: "Tout sélect.", color: "#455A64", action: { type: "hotkey", keys: "ctrl+a" } },
  ]},
  { cat: "Applications", items: [
    { name: "Lancer une application", label: "Appli", color: "#00897B", action: { type: "launch", target: "" }, kind: "program" },
    { name: "Ouvrir un site web", label: "Site web", color: "#1E88E5", action: { type: "launch", target: "https://" }, kind: "url" },
    { name: "Ouvrir un dossier ou fichier", label: "Dossier", color: "#F4B400", action: { type: "launch", target: "" }, kind: "path" },
    { name: "Explorateur", label: "Explorateur", color: "#F4B400", action: { type: "launch", target: "explorer.exe" } },
    { name: "Calculatrice", label: "Calculatrice", color: "#455A64", action: { type: "launch", target: "calc.exe" } },
    { name: "Bloc-notes", label: "Bloc-notes", color: "#0288D1", action: { type: "launch", target: "notepad.exe" } },
  ]},
  { cat: "Multimédia", items: Object.entries(MEDIA).map(([command, name]) => ({
    name, label: name.replace("Piste ", ""), color: command === "mic_mute" || command === "mute" ? "#C62828" : "#2E7D32",
    action: { type: "media", command },
  }))},
  { cat: "Système", items: [
    { name: "Verrouiller le PC", label: "Verrouiller", color: "#6A1B9A", action: { type: "lock" } },
    { name: "Capture d'écran", label: "Capture", color: "#00897B", action: { type: "hotkey", keys: "win+shift+s" } },
    { name: "Afficher le bureau", label: "Bureau", color: "#0277BD", action: { type: "hotkey", keys: "win+d" } },
    { name: "Gestionnaire des tâches", label: "Gestion. tâches", color: "#1565C0", action: { type: "hotkey", keys: "ctrl+shift+esc" } },
    { name: "Paramètres Windows", label: "Paramètres", color: "#607D8B", action: { type: "hotkey", keys: "win+i" } },
    { name: "Fermer la fenêtre", label: "Fermer", color: "#B71C1C", action: { type: "hotkey", keys: "alt+f4" } },
  ]},
  { cat: "HubForge", items: [
    { name: "Page suivante", label: "Page →", color: "#212121", action: { type: "page", to: "next" } },
    { name: "Page précédente", label: "← Page", color: "#212121", action: { type: "page", to: "previous" } },
    { name: "Aller à la page 1", label: "Accueil", color: "#212121", action: { type: "page", to: "1" } },
    { name: "Ouvrir la configuration", label: "Config", color: "#546E7A", action: { type: "open_config" } },
  ]},
];

const ACTION_TYPES = [
  ["none", "Aucune action"],
  ["hotkey", "Raccourci clavier"],
  ["text", "Taper un texte"],
  ["launch:program", "Lancer une application"],
  ["launch:url", "Ouvrir un site web"],
  ["launch:path", "Ouvrir un dossier ou fichier"],
  ["media", "Multimédia"],
  ["page", "Changer de page"],
  ["lock", "Verrouiller le PC"],
  ["open_config", "Ouvrir la configuration"],
];

const SWATCHES = ["#FF3B30", "#FF7A1A", "#FFB300", "#FFD60A", "#3DDC84", "#1DB954", "#00C8FF", "#1E88E5",
                  "#5865F2", "#8E24AA", "#E91E63", "#FFFFFF", "#607D8B", "#37474F", "#212121", "#000000"];

const FADER_TARGETS = [["master", "Volume général"], ["mic", "Micro"], ["app", "Application(s)"], ["none", "Rien"]];
const ROLLER_MODES = [["volume", "Volume général"], ["scroll", "Défilement vertical"], ["hscroll", "Défilement horizontal"], ["zoom", "Zoom (Ctrl + molette)"], ["none", "Rien"]];

// ---------------------------------------------------------------------------
// État
// ---------------------------------------------------------------------------
const S = {
  cfg: null,
  page: 0,
  devicePage: 0,
  sel: { type: "settings" },
  previews: {},          // "page:key" -> { src, animated }
  status: { modules: [], faders: null, page: 0, pressed: [] },
  autostart: false,
  audioApps: [],
  saveTimer: null,
  capturing: false,
};

const $ = (s, el = document) => el.querySelector(s);
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
  $("#version").textContent = "v" + ini.version;
  S.configPath = ini.config_path;

  buildLibrary();
  renderAll();
  await refreshPreviews(S.page);

  listen("status", (e) => onStatus(e.payload));
  listen("autostart", (e) => { S.autostart = e.payload; if (S.sel.type === "settings") renderInspector(); });

  $("#btn-settings").onclick = () => select({ type: "settings" });
  $("#lib-search").oninput = (e) => filterLibrary(e.target.value);
  setupModuleClicks();
  document.addEventListener("keydown", onGlobalKey, true);
  fitDevice();
  new ResizeObserver(fitDevice).observe($(".stage"));
}

/** Réduit le MasterHUB dessiné pour qu'il tienne toujours dans la fenêtre. */
function fitDevice() {
  const stage = $(".stage"), dev = $("#device");
  dev.style.zoom = 1;
  const availW = stage.clientWidth - 48, availH = stage.clientHeight - 90;
  const z = Math.min(1.25, availW / dev.offsetWidth, availH / dev.offsetHeight);
  dev.style.zoom = Math.max(0.5, z).toFixed(3);
}

function normalize(cfg) {
  cfg.general = cfg.general || {};
  cfg.pages = cfg.pages && cfg.pages.length ? cfg.pages : [{ name: "Accueil", keys: [] }];
  cfg.faders = cfg.faders || [];
  while (cfg.faders.length < 5) cfg.faders.push({ target: "none", color: "#FFFFFF" });
  cfg.rollers = Object.assign({ right: "volume", left: "scroll", volume_step: 2, scroll_step: 3, invert_right: false, invert_left: false }, cfg.rollers || {});
  return cfg;
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
    // Les champs « _… » ne servent qu'à l'interface.
    const cfg = JSON.parse(JSON.stringify(S.cfg, (k, v) => (k.startsWith("_") ? undefined : v)));
    await invoke("apply_config", { config: cfg });
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
  renderKeys();
  renderFaders();
  renderRollers();
  renderBase();
  renderInspector();
  renderBanner();
}

function currentPage() { return S.cfg.pages[S.page]; }
function keyDef(k, page = S.page) { return S.cfg.pages[page].keys.find((d) => d.key === k); }

function renderPages() {
  const nav = $("#pages");
  nav.innerHTML = "";
  S.cfg.pages.forEach((p, i) => {
    const tab = el("div", { class: "page-tab" + (i === S.page ? " active" : ""), title: "Double-clic pour renommer" },
      i === S.devicePage ? el("span", { class: "on-device", title: "Page affichée sur le MasterHUB" }) : null,
      el("span", {}, p.name || `Page ${i + 1}`),
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
  if (S.sel.type === "key") S.sel = { type: "key", key: S.sel.key };
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
  $("#mod-keys").classList.toggle("offline", !on.has("keys15"));
  $("#mod-faders").classList.toggle("offline", !on.has("faders5"));
  $("#mod-rollers").classList.toggle("offline", !on.has("rollers2"));
}

function renderBanner() {
  const b = $("#banner");
  if (S.status.config_error) { b.textContent = "⚠ " + S.status.config_error; b.classList.remove("hidden"); }
  else b.classList.add("hidden");
}

// ---------------------------------------------------------------------------
// Touches
// ---------------------------------------------------------------------------
function renderKeys() {
  const box = $("#keys");
  box.innerHTML = "";
  for (let k = 1; k <= 15; k++) box.append(keyElement(k));
}

function keyElement(k) {
  const def = keyDef(k);
  const prev = S.previews[`${S.page}:${k}`];
  const isSel = S.sel.type === "key" && S.sel.key === k;
  const pressed = S.devicePage === S.page && S.status.pressed && S.status.pressed[k - 1];
  const node = el("div", {
    class: "key" + (def ? "" : " empty") + (isSel ? " selected" : "") + (pressed ? " pressed" : ""),
    "data-key": k, draggable: def ? "true" : "false", title: def ? (def.label || `Touche ${k}`) : "Touche vide",
  },
    def && prev ? el("img", { src: prev.src, alt: "" }) : null,
    el("span", { class: "num" }, String(k)),
    def && prev && prev.animated ? el("span", { class: "gif" }, "GIF") : null,
  );
  node.onclick = (e) => { e.stopPropagation(); select({ type: "key", key: k }); };
  node.ondragstart = (e) => { e.dataTransfer.setData("text/hubforge-key", String(k)); e.dataTransfer.effectAllowed = "move"; node.classList.add("dragging"); };
  node.ondragend = () => node.classList.remove("dragging");
  node.ondragover = (e) => { e.preventDefault(); node.classList.add("drop"); };
  node.ondragleave = () => node.classList.remove("drop");
  node.ondrop = (e) => { e.preventDefault(); node.classList.remove("drop"); onDropOnKey(k, e.dataTransfer); };
  return node;
}

function updateKeyNode(k) {
  const old = $(`#keys .key[data-key="${k}"]`);
  if (old) old.replaceWith(keyElement(k));
}

async function refreshPreviews(page, onlyKey) {
  const keys = onlyKey ? [onlyKey] : Array.from({ length: 15 }, (_, i) => i + 1);
  const defs = keys.map((k) => keyDef(k, page) || null);
  try {
    const res = await invoke("render_keys", { keys: defs });
    keys.forEach((k, i) => { if (res[i]) S.previews[`${page}:${k}`] = res[i]; else delete S.previews[`${page}:${k}`]; });
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

async function onDropOnKey(k, dt) {
  const fromKey = dt.getData("text/hubforge-key");
  const lib = dt.getData("text/hubforge-action");
  if (fromKey) {
    const a = Number(fromKey);
    if (a === k) return;
    const da = keyDef(a) ? clone(keyDef(a)) : null;
    const db = keyDef(k) ? clone(keyDef(k)) : null;
    setKeyDef(k, da); setKeyDef(a, db);
    const pa = S.previews[`${S.page}:${a}`], pb = S.previews[`${S.page}:${k}`];
    if (pa) S.previews[`${S.page}:${k}`] = pa; else delete S.previews[`${S.page}:${k}`];
    if (pb) S.previews[`${S.page}:${a}`] = pb; else delete S.previews[`${S.page}:${a}`];
    updateKeyNode(a); updateKeyNode(k);
    select({ type: "key", key: k });
    changed();
    return;
  }
  if (lib) {
    const item = JSON.parse(lib);
    const existing = keyDef(k);
    // La touche devient cette action : texte et couleur suivent (l'image éventuelle est gardée).
    const def = existing ? clone(existing) : { key: k };
    def.action = clone(item.action);
    def.label = item.label;
    def.color = item.color;
    setKeyDef(k, def);
    select({ type: "key", key: k });
    keyChanged(k, { immediate: true });
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
// Faders, molettes, base
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

function renderFaders() {
  const box = $("#faders");
  box.innerHTML = "";
  S.cfg.faders.slice(0, 5).forEach((slot, i) => {
    const v = S.status.faders ? S.status.faders[i] : null;
    const f = el("div", { class: "fader" + (S.sel.type === "fader" && S.sel.index === i ? " selected" : ""), "data-fader": i },
      el("div", { class: "fader-value" }, v === null ? "–" : v + " %"),
      el("div", { class: "fader-track" },
        el("div", { class: "fader-led", style: `background:${slot.color};box-shadow:0 0 8px ${slot.color}` }),
        el("div", { class: "fader-thumb", style: `bottom:${v === null ? 0 : v}%` })),
      el("div", { class: "fader-name", title: slot.target }, faderName(slot)),
    );
    f.onclick = (e) => { e.stopPropagation(); select({ type: "fader", index: i }); };
    box.append(f);
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

function renderRollers() {
  const box = $("#rollers");
  box.innerHTML = "";
  const mode = (m) => (ROLLER_MODES.find(([v]) => v === m) || [0, "—"])[1].split(" (")[0].replace("Défilement ", "Défil. ");
  for (const [side, label] of [["left", "Gauche"], ["right", "Droite"]]) {
    box.append(el("div", { class: "roller" },
      el("div", { class: "roller-wheel" }),
      el("div", { class: "roller-name" }, label, el("br"), el("span", { style: "color:var(--faint)" }, mode(S.cfg.rollers[side]))),
    ));
  }
  $("#mod-rollers").classList.toggle("selected", S.sel.type === "rollers");
}

function renderBase() {
  const c = S.cfg.general.base_color || "#00C8FF";
  $("#base-led").style.cssText = `background:${c};box-shadow:0 0 14px ${c}`;
}

function setupModuleClicks() {
  $("#mod-faders").onclick = () => select({ type: "fader", index: 0 });
  $("#mod-rollers").onclick = () => select({ type: "rollers" });
  $("#base-led").onclick = (e) => { e.stopPropagation(); select({ type: "settings" }); };
}

// ---------------------------------------------------------------------------
// Sélection et inspecteur
// ---------------------------------------------------------------------------
function select(sel) {
  S.sel = sel;
  S.capturing = false;
  document.querySelectorAll(".key.selected").forEach((n) => n.classList.remove("selected"));
  if (sel.type === "key") { const n = $(`#keys .key[data-key="${sel.key}"]`); if (n) n.classList.add("selected"); }
  $("#mod-keys").classList.toggle("selected", false);
  renderFaders();
  renderRollers();
  $("#btn-settings").classList.toggle("active", sel.type === "settings");
  renderInspector();
}

function renderInspector() {
  const box = $("#inspector");
  box.innerHTML = "";
  if (S.sel.type === "key") box.append(...keyInspector(S.sel.key));
  else if (S.sel.type === "fader") box.append(...faderInspector(S.sel.index));
  else if (S.sel.type === "rollers") box.append(...rollersInspector());
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
  if (!def) {
    out.push(el("p", { class: "empty-state" }, "Touche vide.", el("br"), "Glisse une action depuis la liste de gauche, ou :"));
    out.push(el("button", { class: "btn primary", style: "width:100%", onclick: () => {
      setKeyDef(k, { key: k, label: "Nouvelle touche", color: "#3949AB", action: { type: "none" } });
      keyChanged(k, { immediate: true }); renderInspector();
    } }, "Créer une touche"));
    setTimeout(renderKeyPreviewInInspector);
    return out;
  }
  const upd = (fn, opts) => { fn(def); keyChanged(k, opts); };

  out.push(field("Texte affiché", el("input", { class: "input", value: def.label || "", placeholder: "(aucun)", oninput: (e) => upd((d) => { d.label = e.target.value; }) })));
  out.push(colorField("Couleur de fond", def.color, (v) => upd((d) => { if (v) d.color = v; else delete d.color; })));
  out.push(colorField("Couleur du texte", def.text_color || "#FFFFFF", (v) => upd((d) => { if (v) d.text_color = v; else delete d.text_color; })));

  const iconName = def.icon ? def.icon.split(/[\\/]/).pop() : null;
  out.push(field("Image ou GIF",
    el("div", { class: "row" },
      el("button", { class: "btn grow", onclick: async () => {
        const p = await invoke("pick_file", { kind: "image" });
        if (!p) return;
        try { const rel = await invoke("import_image", { path: p }); upd((d) => { d.icon = rel; }, { immediate: true }); renderInspector(); }
        catch (e) { toast(String(e), true); }
      } }, iconName ? "Changer l'image…" : "Choisir une image…"),
      iconName ? el("button", { class: "btn", title: "Retirer l'image", onclick: () => { upd((d) => { delete d.icon; }, { immediate: true }); renderInspector(); } }, "Retirer") : null),
    iconName ? el("p", { class: "hint" }, iconName) : el("p", { class: "hint" }, "Astuce : tu peux aussi glisser une image depuis l'Explorateur sur la touche."),
  ));

  out.push(el("div", { class: "sep" }));
  out.push(...actionEditor(def, k));
  out.push(el("div", { class: "sep" }));
  out.push(el("button", { class: "btn danger", style: "width:100%", onclick: async () => {
    if (!(await confirm_(`Vider la touche ${k} ?`))) return;
    setKeyDef(k, null); delete S.previews[`${S.page}:${k}`]; updateKeyNode(k); changed(); renderInspector();
  } }, "Vider la touche"));
  setTimeout(renderKeyPreviewInInspector);
  return out;
}

function renderKeyPreviewInInspector() {
  const box = $("#insp-preview");
  if (!box || S.sel.type !== "key") return;
  const k = S.sel.key;
  const prev = S.previews[`${S.page}:${k}`];
  box.innerHTML = "";
  box.append(el("div", { class: "key" + (keyDef(k) ? "" : " empty") }, keyDef(k) && prev ? el("img", { src: prev.src }) : null,
    keyDef(k) && prev && prev.animated ? el("span", { class: "gif" }, "GIF") : null));
}

function launchKind(target) {
  if (/^https?:\/\//i.test(target)) return "url";
  if (!target || /\.(exe|lnk|bat|cmd|msc)$/i.test(target)) return "program";
  return "path";
}

function actionTypeId(a) {
  if (a.type === "launch") return "launch:" + launchKind(a.target || "");
  return a.type;
}

function actionEditor(def, k) {
  const a = def.action || { type: "none" };
  const upd = (fn, opts) => { fn(def.action); keyChanged(k, opts); };
  const typeSel = el("select", { class: "input" }, ...ACTION_TYPES.map(([v, l]) => el("option", { value: v, selected: v === actionTypeId(a) }, l)));
  typeSel.onchange = () => {
    const v = typeSel.value;
    const defaults = {
      none: { type: "none" }, hotkey: { type: "hotkey", keys: "" }, text: { type: "text", text: "" },
      "launch:program": { type: "launch", target: "" }, "launch:url": { type: "launch", target: "https://" },
      "launch:path": { type: "launch", target: "" }, media: { type: "media", command: "play_pause" },
      page: { type: "page", to: "next" }, lock: { type: "lock" }, open_config: { type: "open_config" },
    };
    def.action = clone(defaults[v]);
    if (v === "launch:path") def.action._kind = "path";
    keyChanged(k);
    renderInspector();
  };
  const out = [el("h2", {}, "Action"), field("Quand j'appuie sur la touche", typeSel)];
  const t = a._kind === "path" ? "launch:path" : actionTypeId(a);

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
      el("p", { class: "hint" }, "Ex. ctrl+shift+s · win+d · alt+f4. Les combinaisons avec Windows peuvent être ajoutées avec la liste.")));
  } else if (t === "text") {
    out.push(field("Texte à taper", el("textarea", { class: "input", oninput: (e) => upd((x) => { x.text = e.target.value; }) }, a.text || "")));
  } else if (t === "launch:program") {
    const inp = el("input", { class: "input grow", value: a.target || "", placeholder: "ex. notepad.exe ou C:\\…\\appli.exe", spellcheck: "false", oninput: (e) => upd((x) => { x.target = e.target.value; }) });
    out.push(field("Application", el("div", { class: "row" }, inp, el("button", { class: "btn", onclick: async () => {
      const p = await invoke("pick_file", { kind: "program" }); if (p) { inp.value = p; upd((x) => { x.target = p; }); }
    } }, "Parcourir…"))));
    out.push(field("Arguments (facultatif)", el("input", { class: "input", value: (a.args || []).join(" "), placeholder: "ex. --nouvelle-fenetre",
      oninput: (e) => upd((x) => { const v = e.target.value.trim(); if (v) x.args = v.split(/\s+/); else delete x.args; }) })));
  } else if (t === "launch:url") {
    out.push(field("Adresse du site", el("input", { class: "input", value: a.target || "https://", spellcheck: "false", oninput: (e) => upd((x) => { x.target = e.target.value; }) })));
  } else if (t === "launch:path") {
    const inp = el("input", { class: "input", value: a.target || "", placeholder: "C:\\Users\\…", spellcheck: "false", oninput: (e) => upd((x) => { x.target = e.target.value; }) });
    out.push(field("Dossier ou fichier", inp, el("div", { class: "row", style: "margin-top:8px" },
      el("button", { class: "btn grow", onclick: async () => { const p = await invoke("pick_folder"); if (p) { inp.value = p; upd((x) => { x.target = p; }); } } }, "Choisir un dossier…"),
      el("button", { class: "btn grow", onclick: async () => { const p = await invoke("pick_file", { kind: "any" }); if (p) { inp.value = p; upd((x) => { x.target = p; }); } } }, "Choisir un fichier…"))));
  } else if (t === "media") {
    out.push(field("Commande", el("select", { class: "input", onchange: (e) => upd((x) => { x.command = e.target.value; }) },
      ...Object.entries(MEDIA).map(([v, l]) => el("option", { value: v, selected: v === a.command }, l)))));
  } else if (t === "page") {
    const opts = [["next", "Page suivante"], ["previous", "Page précédente"], ...S.cfg.pages.map((p, i) => [String(i + 1), `Page ${i + 1} — ${p.name}`])];
    out.push(field("Aller à", el("select", { class: "input", onchange: (e) => upd((x) => { x.to = e.target.value; }) },
      ...opts.map(([v, l]) => el("option", { value: v, selected: v === a.to }, l)))));
  }
  return out;
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
  if (!main) return parts.length ? null : null;
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
  sel.onchange = () => {
    slot.target = sel.value === "app" ? "app:" : sel.value;
    changed(); renderFaders(); renderInspector();
  };
  out.push(field("Ce fader contrôle", sel));

  if (kind === "app") {
    const apps = t.slice(4).split(",").map((s) => s.trim()).filter(Boolean);
    const setApps = (list) => { slot.target = "app:" + list.join(","); changed(); renderFaders(); renderInspector(); };
    const chips = el("div", { class: "chips" }, ...apps.map((a) => el("span", { class: "chip removable", title: "Retirer", onclick: () => setApps(apps.filter((x) => x !== a)) }, a, " ×")));
    const add = el("input", { class: "input grow", placeholder: "ex. discord.exe", spellcheck: "false" });
    const addIt = (name) => { name = (name || "").trim().toLowerCase(); if (!name) return; if (!/\.exe$/.test(name)) name += ".exe"; if (!apps.includes(name)) setApps([...apps, name]); };
    add.onkeydown = (e) => { if (e.key === "Enter") addIt(add.value); };
    const running = el("div", { class: "chips" }, el("span", { class: "hint", style: "margin:0" }, "Recherche des applis…"));
    invoke("list_audio_apps").then((list) => {
      running.innerHTML = "";
      const others = list.filter((x) => !apps.includes(x));
      if (!others.length) running.append(el("span", { class: "hint", style: "margin:0" }, "Aucune autre appli ne joue du son en ce moment."));
      for (const a of others) running.append(el("span", { class: "chip add", onclick: () => addIt(a) }, "+ " + a));
    });
    out.push(field("Applications", apps.length ? chips : el("p", { class: "hint", style: "margin:0" }, "Aucune pour l'instant."),
      el("div", { class: "row", style: "margin-top:10px" }, add, el("button", { class: "btn", onclick: () => addIt(add.value) }, "Ajouter"))));
    out.push(field("Applis qui jouent du son en ce moment", running,
      el("p", { class: "hint" }, "Une appli n'apparaît ici que lorsqu'elle joue (ou a joué) du son.")));
  }
  out.push(colorField("Couleur de la LED", slot.color, (v) => { if (!v) return; slot.color = v; changed(); renderFaders(); }));
  return out;
}

// --- Molettes ---
function rollersInspector() {
  const r = S.cfg.rollers;
  const out = [title("Molettes")];
  for (const [side, label] of [["left", "Molette de gauche"], ["right", "Molette de droite"]]) {
    out.push(el("h2", {}, label));
    out.push(field("Action", el("select", { class: "input", onchange: (e) => { r[side] = e.target.value; changed(); renderRollers(); } },
      ...ROLLER_MODES.map(([v, l]) => el("option", { value: v, selected: v === r[side] }, l)))));
    out.push(switchRow("Inverser le sens", r["invert_" + side], (v) => { r["invert_" + side] = v; changed(); }));
  }
  out.push(el("div", { class: "sep" }));
  out.push(rangeField("Volume par cran", r.volume_step, 1, 10, " %", (v) => { r.volume_step = v; changed(); }));
  out.push(rangeField("Lignes par cran (défilement)", r.scroll_step, 1, 10, "", (v) => { r.scroll_step = v; changed(); }));
  return out;
}

// --- Réglages généraux ---
function settingsInspector() {
  const g = S.cfg.general;
  const out = [title("Réglages généraux")];
  out.push(rangeField("Luminosité des écrans", g.brightness, 0, 100, " %", (v) => { g.brightness = v; changed(); }));
  out.push(rangeField("Luminosité des LED", g.led_brightness, 0, 100, " %", (v) => { g.led_brightness = v; changed(); }));
  out.push(colorField("Couleur des LED de la base", g.base_color, (v) => { if (!v) return; g.base_color = v; renderBase(); changed(); }));
  out.push(switchRow("Assombrir la touche pendant l'appui", g.press_feedback, (v) => { g.press_feedback = v; changed(); }));
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
      const node = el("div", { class: "lib-item", draggable: "true", "data-search": (it.name + " " + cat.cat).toLowerCase() },
        el("span", { class: "lib-swatch", style: `background:${it.color}` }, it.label.charAt(0)),
        el("span", {}, it.name));
      node.ondragstart = (e) => {
        const payload = clone(it);
        if (it.kind === "path") payload.action._kind = "path";
        e.dataTransfer.setData("text/hubforge-action", JSON.stringify(payload));
        e.dataTransfer.effectAllowed = "copy";
      };
      node.ondblclick = () => {
        if (S.sel.type !== "key") { toast("Sélectionne d'abord une touche, ou glisse l'action dessus."); return; }
        const dt = { getData: (t) => (t === "text/hubforge-action" ? JSON.stringify(Object.assign(clone(it), it.kind === "path" ? { action: Object.assign(clone(it.action), { _kind: "path" }) } : {})) : "") };
        onDropOnKey(S.sel.key, dt);
      };
      items.append(node);
    }
    const c = el("div", { class: "lib-cat" },
      el("div", { class: "lib-cat-title" }, el("span", { class: "caret" }, "▾"), cat.cat), items);
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
  const prevPressed = S.status.pressed || [];
  const modulesChanged = JSON.stringify((S.status.modules || []).map((m) => m.kind)) !== JSON.stringify(st.modules.map((m) => m.kind));
  const pageChanged = st.page !== S.devicePage;
  const errChanged = st.config_error !== S.status.config_error;
  S.status = st;
  if (modulesChanged) renderModulesChips();
  if (errChanged) renderBanner();
  updateFaderLive();
  if (pageChanged) {
    S.devicePage = st.page;
    // La page a changé depuis le MasterHUB : l'interface suit.
    if (S.page !== st.page && st.page < S.cfg.pages.length) { S.page = st.page; renderKeys(); refreshPreviews(S.page); if (S.sel.type === "key") renderInspector(); }
    renderPages();
  }
  if (S.devicePage === S.page) {
    for (let i = 0; i < 15; i++) {
      if (!!prevPressed[i] !== !!(st.pressed && st.pressed[i])) {
        const n = $(`#keys .key[data-key="${i + 1}"]`);
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
    confirm_(`Vider la touche ${k} ?`).then((ok) => { if (!ok) return; setKeyDef(k, null); delete S.previews[`${S.page}:${k}`]; updateKeyNode(k); changed(); renderInspector(); });
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
    window.__mock = { cfg };
  }
  switch (cmd) {
    case "get_initial":
      return { config: window.__mock.cfg, config_path: "C:\\Users\\benja\\AppData\\Roaming\\HubForge\\config.toml", autostart: false, version: "0.2.0",
        status: { modules: [{ kind: "keys15" }, { kind: "faders5" }, { kind: "rollers2" }, { kind: "base" }], faders: [72, 35, 60, 18, 90], page: 0, pressed: Array(15).fill(false) } };
    case "render_keys":
      return args.keys.map((d) => d && { src: mockPreview(d), animated: !!(d.icon && /\.gif$/i.test(d.icon)) });
    case "list_audio_apps": return ["chrome.exe", "discord.exe", "spotify.exe"];
    default: return null;
  }
}

function mockPreview(d) {
  const bg = d.color || "#000";
  const label = (d.label || "").replace(/&/g, "&amp;").replace(/</g, "&lt;");
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72"><rect width="72" height="72" fill="${bg}"/>` +
    (d.icon ? `<circle cx="36" cy="32" r="18" fill="none" stroke="#ff7a1a" stroke-width="7"/>` : "") +
    `<foreignObject x="3" y="${d.icon ? 50 : 3}" width="66" height="${d.icon ? 20 : 66}"><div xmlns="http://www.w3.org/1999/xhtml" style="height:100%;display:flex;align-items:center;justify-content:center;text-align:center;font:600 ${label.length > 10 ? 10 : 13}px Inter,sans-serif;color:#fff">${label}</div></foreignObject></svg>`;
  return "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
}

init().catch((e) => { document.body.innerHTML = `<pre style="padding:20px;color:#ffb3b5">Erreur au démarrage de l'interface :\n${e}</pre>`; });
