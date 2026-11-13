"use strict";
/* HubForge — Le MasterHUB dessiné : base, modules, touches, faders, molettes ; glisser-déposer. */

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
    def && def.toggle ? el("span", { class: "badge", title: "Touche à 2 états" }, "2")
      : def && def.widget ? el("span", { class: "badge live", title: "Touche vivante" }, "")
      : def && def.action && def.action.type === "macro" ? el("span", { class: "badge", title: "Macro" }, "M")
      : def && prev && prev.animated ? el("span", { class: "badge", title: "Animation" }, "▶") : null,
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
    req.push([k, 0, def ? withTheme(clean(stateDef(def, 0)), page, def) : null]);
    if (def && def.toggle) req.push([k, 1, withTheme(clean(stateDef(def, 1)), page, def)]);
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
    const res = await invoke("render_keys", { keys: keys.map((k) => withTheme(clean(stateDef(keyDef(k, page), 0)), page, keyDef(k, page))) });
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

/** Touche créée à partir d'une action de la bibliothèque. */
function defFromItem(item, k) {
  const def = item.addonKey ? Object.assign({ key: k }, clone(item.addonKey))
    : { key: k, label: item.label, color: item.color, icon: "builtin:" + item.icon, action: clone(item.action) };
  if (item.toggle) def.toggle = clone(item.toggle);
  if (item.long) def.long = clone(item.long);
  if (item.widget) { def.widget = item.widget; delete def.icon; }
  if (item.anim) def.icon = "anim:" + item.anim;
  if (item.widget_style) def.widget_style = item.widget_style;
  if (item.widget_target) def.widget_target = item.widget_target;
  return def;
}

/** Action de la bibliothèque par son nom. */
function libItemByName(name) {
  for (const c of LIBRARY) { const it = c.items.find((i) => i.name === name); if (it) return it; }
  return null;
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
    const def = defFromItem(item, k);
    if (item.newFolder) {
      const idx = createFolder(nextFolderName());
      def.label = S.cfg.pages[idx].name;
      def.action = { type: "folder", to: String(idx + 1) };
    }
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
    if (/\.(mp3|wav|ogg|flac|m4a|aac)$/i.test(f.name)) {
      try {
        const rel = await invoke("import_sound_data", { name: f.name, data: await fileToBase64(f) });
        const old = keyDef(k);
        const def = old && old.action && old.action.type === "sound" ? clone(old)
          : { key: k, label: f.name.replace(/\.[^.]+$/, "").slice(0, 24), color: "#AD1457", icon: (old && old.icon) || "builtin:music", action: {} };
        def.action = { type: "sound", file: rel, volume: (old && old.action && old.action.volume) || 100, mode: (old && old.action && old.action.mode) || "toggle" };
        setKeyDef(k, def);
        S.editState = 0;
        select({ type: "key", key: k });
        keyChanged(k, { immediate: true });
        toast(`Son « ${f.name} » ajouté sur la touche ${k}.`);
      } catch (e) { toast(String(e), true); }
      return;
    }
    if (!/\.(png|jpe?g|gif|webp|bmp)$/i.test(f.name)) { toast("Dépose une image (png, jpg, gif, webp) ou un son (mp3, wav, ogg…).", true); return; }
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
  const mode = (m) => rollerKind(m) === "app" ? "Volume " + (m.slice(4).split(",")[0] || "appli").replace(/\.exe$/, "")
    : (ROLLER_MODES.find(([v]) => v === m) || [0, "—"])[1].split(" (")[0];
  return [["left", "Gauche"], ["right", "Droite"]].map(([side, label]) => {
    const own = currentPage()["roller_" + side] != null;
    return el("div", { class: "roller" + (own ? " page-own" : ""), title: `Molette ${label.toLowerCase()} : ${mode(effRoller(side))}${own ? " (propre à cette page)" : ""}` },
      el("div", { class: "roller-drum" }),
      el("div", { class: "roller-name" }, `${label} · ${mode(effRoller(side))}`));
  });
}
