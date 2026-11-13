"use strict";
/* HubForge — Profils, annuler / rétablir, copier-coller, import / export, packs d'icônes, sauvegardes, diagnostic. */

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
  m.append(action("⚡ Applications de « " + (S.profile || "") + " » (mode dynamique)…", () => select({ type: "profile" })));
  m.append(el("div", { class: "pm-sep" }));
  m.append(action("⇪ Exporter « " + (S.profile || "") + " » dans un fichier…", exportProfile));
  m.append(action("⇩ Importer un profil ou un pack d'icônes…", importProfile));
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
  S.sel = { type: "none" };
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
  const plural = (n, w) => `${n} ${w}${n > 1 ? "s" : ""}`;
  if (r.kind === "icons") {
    S.iconPacks = null;
    S.iconPickerOpen = true;
    toast(`Pack d'icônes « ${r.name} » ajouté : ${plural(r.images, "icône")}. Tu le trouves dans « Icônes… » de chaque touche.`);
    if (S.sel && S.sel.type === "key") renderInspector();
    return;
  }
  const parts = [];
  if (r.images) parts.push(plural(r.images, "image"));
  if (r.sounds) parts.push(plural(r.sounds, "son"));
  const extra = parts.length ? ` avec ${parts.join(" et ")}` : "";
  if (r.unsupported && r.unsupported.length) {
    toast(`Profil « ${r.name} » importé${extra}. Touches sans action (plugins Stream Deck sans équivalent, en gris) : ${r.unsupported.join(", ")}.`, true);
  } else if (r.missing_addons && r.missing_addons.length) {
    toast(`Profil « ${r.name} » importé${extra}. Addons à installer pour certaines touches : ${r.missing_addons.join(", ")}.`, true);
  } else {
    toast(`Profil « ${r.name} » importé${extra} et activé.`);
  }
}

// --- Packs d'icônes (Stream Deck) ---
async function loadIconPacks() {
  if (S.iconPacks) return S.iconPacks;
  try { S.iconPacks = await invoke("list_icon_packs"); } catch (_) { S.iconPacks = []; }
  return S.iconPacks;
}
async function packThumbs(folder) {
  S.packThumbs = S.packThumbs || {};
  if (!S.packThumbs[folder]) {
    try { S.packThumbs[folder] = await invoke("icon_pack_thumbs", { folder }); } catch (_) { S.packThumbs[folder] = []; }
  }
  return S.packThumbs[folder];
}
function iconPacksBlock(icon, upd) {
  const box = el("div", {});
  (async () => {
    const packs = await loadIconPacks();
    const head = el("div", { class: "step-branch" }, "Packs d'icônes");
    const importBtn = el("button", { class: "btn small", onclick: importProfile }, "Importer un pack…");
    if (!packs.length) {
      box.append(head, el("p", { class: "hint" }, "Ajoute un pack d'icônes Stream Deck (.streamDeckIconPack) : il apparaîtra ici pour toutes les touches. ", importBtn));
      return;
    }
    if (!S.openPack || !packs.some((p) => p.folder === S.openPack)) S.openPack = packs[0].folder;
    const grid = el("div", { class: "icon-grid pack-grid" });
    const tabs = el("div", { class: "chips" }, ...packs.map((p) => el("span", {
      class: "chip" + (p.folder === S.openPack ? " on style-on" : " add"),
      title: p.author ? `par ${p.author}` : "",
      onclick: () => { S.openPack = p.folder; renderInspector(); },
    }, `${p.name} (${p.icons.length})`)));
    const pack = packs.find((p) => p.folder === S.openPack);
    const search = el("input", { class: "input", placeholder: `Rechercher dans « ${pack.name} »` });
    const thumbs = await packThumbs(pack.folder);
    const fill = (q) => {
      grid.innerHTML = "";
      for (const [path, name, src] of thumbs.filter(([, n]) => !q || n.toLowerCase().includes(q)).slice(0, 400)) {
        const cell = el("div", { class: "icon-cell" + (icon === path ? " on" : ""), title: name }, el("img", { src, alt: name }));
        cell.onclick = () => { upd((d) => { d.icon = path; }, { immediate: true }); renderInspector(); };
        grid.append(cell);
      }
    };
    search.oninput = (e) => fill(e.target.value.trim().toLowerCase());
    fill("");
    const remove = el("button", { class: "btn small", onclick: async () => {
      if (!confirm(`Retirer le pack « ${pack.name} » ? Les touches qui utilisent ses icônes n'auront plus d'image.`)) return;
      try { await invoke("remove_icon_pack", { folder: pack.folder }); S.iconPacks = null; delete S.packThumbs[pack.folder]; renderInspector(); }
      catch (e) { toast(String(e), true); }
    } }, "Retirer ce pack");
    box.append(head, tabs, search, grid, el("div", { class: "row", style: "gap:6px;margin-top:6px" }, importBtn, remove));
  })();
  return box;
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
    if (a.type === "macro" || a.type === "if") { childActions(a).forEach(fix); return a; }
    if ((a.type === "page" || a.type === "folder") && /^\d+$/.test(a.to || "")) {
      const old = Number(a.to) - 1;
      if (old >= 0 && old < order.length) a.to = String(newIndex(old) + 1);
    }
    return a;
  };
  S.cfg.pages = order.map((i) => S.cfg.pages[i]);
  for (const p of S.cfg.pages) for (const d of p.keys) { fix(d.action); if (d.toggle) fix(d.toggle.action); fix(d.long); fix(d.double); }
  for (const w of wheelActionSets()) { fix(w.up); fix(w.down); }
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
