"use strict";
/* HubForge — Bibliothèque d'actions (colonne de gauche). */

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

/** Familles de la bibliothèque (pastilles du haut) : catégories regroupées. */
const LIB_GROUPS = [
  ["all", "Tout", []],
  ["keys", "Clavier", ["Clavier", "Fenêtres"]],
  ["apps", "Applis", ["Applications"]],
  ["sound", "Son", ["Multimédia", "Son et musique", "Soundboard", "Touches à 2 états"]],
  ["live", "Vivantes", ["Touches vivantes", "Minuteurs", "Animations"]],
  ["game", "Jeu", ["Jeu"]],
  ["ai", "IA et web", ["Connecteurs"]],
  ["system", "Système", ["Système", "Macros intelligentes", "HubForge"]],
];

/** Une ligne d'explication sous les actions les plus courantes. */
const LIB_DESC = {
  "Raccourci clavier": "Ctrl + C, Win + Maj + S…", "Taper un texte": "Une phrase, une adresse, une signature",
  "Maintenir des touches": "Tant que la touche reste enfoncée", "Macro (plusieurs actions)": "Plusieurs actions à la suite",
  "Lancer une application": "Discord, Chrome, un jeu…", "Ouvrir un site web": "Une adresse en un appui",
  "Ouvrir un dossier ou fichier": "Projets, téléchargements…", "Lecture / Pause": "Spotify, YouTube, VLC…",
  "Couper le micro": "Pour tout Windows", "Changer de sortie audio": "Casque ou enceintes",
  "Musique en cours (pochette)": "Titre et pochette en direct", "Musique en cours (titre)": "Titre et artiste en direct",
  "Chronomètre (appui long = 0)": "Départ, pause, remise à zéro", "Compte à rebours 5 min": "Clignote à la fin",
  "Compte à rebours 25 min (pomodoro)": "Pour travailler par tranches", "Jouer un son": "Ton soundboard",
  "Compteur (+1, appui long = 0)": "Affiché sur la touche", "Condition (si… alors… sinon)": "Selon l'appli, l'heure, une variable",
  "IA : corriger la sélection": "Le texte sélectionné, corrigé", "IA : traduire en anglais": "Remplace la sélection",
  "IA : traduire en français": "Remplace la sélection", "IA : style professionnel": "Réécrit la sélection",
  "IA : résumer (copié)": "Le résumé va dans le presse-papiers", "IA : consigne perso": "Ta propre consigne",
  "Requête web": "Domotique, serveur, API", "Webhook Discord": "Un message dans un salon",
  "Verrouiller le PC": "Win + L", "Capture d'écran": "Zone, fenêtre ou écran", "Micro ON / OFF": "Avec l'état sur la touche",
  "Son ON / OFF": "Avec l'état sur la touche", "Horloge": "Heure en direct", "Date": "Jour et mois",
  "Processeur (CPU)": "Charge en direct", "Mémoire (RAM)": "Utilisation en direct", "Volume général": "Niveau en direct",
  "État du micro": "Coupé ou actif", "Carte graphique (GPU)": "Charge en direct", "Température du GPU": "En degrés",
  "Volume d'une application": "Discord, Spotify…", "Météo": "Ta ville, en direct", "Réseau (débits)": "Envoi et réception",
  "Disque C:": "Espace libre", "Nouveau dossier (sous-page)": "Une page qui s'ouvre d'un appui",
  "Relecture instantanée NVIDIA": "Les dernières minutes de jeu (Alt+F10)", "Enregistrer (NVIDIA)": "Début / fin (Alt+F9)",
  "Overlay NVIDIA": "Alt+Z", "Xbox Game Bar": "Win+G", "Enregistrer les 30 dernières s (Game Bar)": "Win+Alt+G",
  "Enregistrer (Game Bar)": "Début / fin (Win+Alt+R)", "Overlay Steam": "Maj+Tab",
  "Fenêtre à gauche": "Moitié gauche de l'écran", "Fenêtre à droite": "Moitié droite de l'écran", "Vers l'autre écran": "Déplace la fenêtre",
};

/** Élément de la bibliothèque (glissable sur une touche, double-clic = touche sélectionnée). */
function libItem(it, path) {
  const c = it.color || "#5d6374";
  const sw = el("span", { class: "lib-swatch", style: `background:${c}26;color:${libTint(c)}`, html: it.preview ? (it.preview.svg || "") : svgIcon(it.icon) });
  if (it.preview && it.preview.src) sw.append(el("img", { src: it.preview.src, alt: "" }));
  const desc = it.description || LIB_DESC[it.name] || "";
  const node = el("div", { class: "lib-item", draggable: "true", title: desc ? `${it.name} — ${desc}` : it.name, "data-search": (it.name + " " + path + " " + desc + " " + tr(it.name) + " " + tr(desc)).toLowerCase() },
    sw, el("span", { class: "lib-text" }, el("span", { class: "lib-name" }, it.name), desc ? el("span", { class: "lib-desc" }, desc) : null));
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
    addonKey: { label: x.label, icon: x.icon || undefined, color: x.color || undefined, text_color: x.text_color || undefined, action: x.action,
      ...(x.state_var ? { state_var: x.state_var, toggle: x.on } : {}) },
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

/** Couleur d'icône lisible sur fond sombre (les couleurs trop foncées sont éclaircies). */
function libTint(hex) {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex || "");
  if (!m) return "#c9ccd6";
  let [r, g, b] = [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16));
  const l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  if (l < 120) { const k = 120 / Math.max(l, 1); [r, g, b] = [r, g, b].map((v) => Math.min(255, Math.round(v * k + 40))); }
  return `rgb(${r},${g},${b})`;
}

function buildLibGroups(hasAddons) {
  const box = $("#lib-groups");
  if (!box) return;
  const groups = hasAddons ? [...LIB_GROUPS, ["addons", "Addons", []]] : LIB_GROUPS;
  if (!groups.some(([id]) => id === S.libGroup)) S.libGroup = "all";
  box.innerHTML = "";
  box.append(...groups.map(([id, label]) => el("button", { class: "pill" + (id === S.libGroup ? " on" : ""),
    onclick: () => { S.libGroup = id; buildLibrary(); } }, label)));
}

function buildLibrary() {
  const box = $("#library");
  box.innerHTML = "";
  const addonsOn = (S.addons || []).filter((x) => x.enabled && x.addon && x.actions.length);
  buildLibGroups(addonsOn.length > 0);
  const group = LIB_GROUPS.find(([id]) => id === S.libGroup);
  const q = ($("#lib-search").value || "").trim();
  // Une famille choisie : ses actions à plat, sous de petits intertitres.
  if (!q && S.libGroup === "addons") {
    for (const ad of addonsOn) box.append(addonFolder(ad));
    return;
  }
  if (!q && group && group[0] !== "all") {
    let cats = clone(LIBRARY).filter((c) => group[2].includes(c.cat));
    if (S.gameBest > 0 && group[2].includes("HubForge")) cats.find((c) => c.cat === "HubForge").items.push(
      { name: "Mini-jeu tape-taupe", label: "Jouer", icon: "gamepad-2", color: "#2E7D32", action: { type: "game" } });
    for (const cat of cats) {
      if (!isAdv()) {
        if (SIMPLE_HIDDEN_CATS.includes(cat.cat)) continue;
        cat.items = cat.items.filter((it) => !SIMPLE_HIDDEN_ITEMS.includes(it.name));
      }
      if (!cat.items.length) continue;
      if (cats.length > 1) box.append(el("div", { class: "lib-section" }, cat.cat));
      box.append(...cat.items.map((it) => libItem(it, cat.cat)));
    }
    return;
  }
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
    if (!isAdv()) {
      if (SIMPLE_HIDDEN_CATS.includes(cat.cat)) continue;
      cat.items = cat.items.filter((it) => !SIMPLE_HIDDEN_ITEMS.includes(it.name));
    }
    box.append(libFolder({
      key: `cat:${cat.cat}`, title: cat.cat, level: 0, defaultOpen: true,
      children: cat.items.map((it) => libItem(it, cat.cat)),
    }));
  }
  if (q) filterLibrary(q);
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
