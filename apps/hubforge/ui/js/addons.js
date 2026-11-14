"use strict";
/* HubForge — Addons .forge. */

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
    else if (/\.(hfprofile|streamDeckProfile|streamDeckIconPack)$/i.test(f.name)) importProfileFile(f);
    else toast("Dépose sur la fenêtre un addon .forge, un profil (.hfprofile ou Stream Deck) ou un pack d'icônes Stream Deck, ou une image sur une touche.");
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
  const out = [title("Addons", "Actions, pages et programmes")];
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
        el("div", { class: "addon-name", translate: "no" }, a ? a.name : ad.folder, ad.official ? el("span", { class: "addon-badge" }, "officiel") : null),
        el("div", { class: "addon-meta" }, a ? [`v${a.version}`, a.author, a.license].filter(Boolean).join(" · ") : "Addon en erreur")));
    card.append(head);
    if (ad.error) {
      card.append(el("div", { class: "addon-error" }, ad.error));
    } else {
      if (a.description) card.append(el("p", { class: "addon-desc", translate: "no" }, a.description));
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
      if (a.service || (a.settings && a.settings.length)) card.append(...addonServiceBlock(ad));
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

const RUNTIME_NAMES = { powershell: "PowerShell", python: "Python", node: "Node.js", exe: "un programme .exe" };

/** Programme d'un addon (niveau 2) : accord, état, réglages. */
function addonServiceBlock(ad) {
  const a = ad.addon;
  const out = [];
  const refresh = async () => { await loadAddons(); renderInspector(); };
  if (a.service) {
    if (!ad.trusted) {
      out.push(el("div", { class: "addon-trust" },
        el("b", {}, "Cet addon contient un programme"),
        el("p", {}, `Il lance ${RUNTIME_NAMES[a.service.run] || a.service.run} (${a.service.main}) en arrière-plan, avec les mêmes droits que toi sur ce PC. N'autorise que les addons dont tu connais la source.`),
        el("button", { class: "btn primary small", onclick: async () => {
          try { await invoke("trust_addon_service", { id: ad.folder, on: true }); toast("Programme autorisé : il démarre dans quelques secondes."); setTimeout(refresh, 1500); }
          catch (e) { toast(String(e), true); }
        } }, "Autoriser ce programme")));
    } else {
      const st = ad.service || { state: "stopped", message: "" };
      const label = { running: "Programme en marche", stopped: ad.enabled ? "Programme en cours de démarrage…" : "Programme arrêté (addon désactivé)", waiting: "En attente de ton accord", error: "Programme en erreur" }[st.state] || st.state;
      out.push(el("div", { class: "addon-svc " + (st.state || "") },
        el("span", { class: "dot" }), el("span", { class: "grow" }, label, st.message ? el("small", {}, " — " + st.message) : null),
        st.state === "error" ? el("button", { class: "btn small", onclick: async () => { await invoke("restart_addon_service", { id: a.id }); setTimeout(refresh, 1500); } }, "Relancer") : null,
        el("button", { class: "btn ghost small", title: "Le programme ne sera plus lancé", onclick: async () => {
          await invoke("trust_addon_service", { id: ad.folder, on: false }); setTimeout(refresh, 800);
        } }, "Retirer l'autorisation")));
    }
  }
  if (a.settings && a.settings.length) {
    // Les textes des réglages viennent de l'addon : on ne les traduit pas.
    const first = out.length;
    const values = Object.assign({}, ad.settings || {});
    const save = async () => { try { await invoke("save_addon_settings", { id: ad.folder, values }); ad.settings = values; } catch (e) { toast(String(e), true); } };
    for (const d of a.settings) {
      let input;
      if (d.kind === "bool") {
        out.push(switchRow(d.label, !!values[d.id], (v) => { values[d.id] = v; save(); }, d.help || undefined));
        continue;
      }
      input = el("input", { class: "input" + (d.kind === "password" ? " mono" : ""), type: d.kind === "password" ? "password" : d.kind === "number" ? "number" : "text",
        value: values[d.id] ?? "", spellcheck: "false", autocomplete: "off",
        onchange: (e) => { values[d.id] = d.kind === "number" ? Number(e.target.value) : e.target.value; save(); } });
      out.push(field(d.label, input, d.help ? el("p", { class: "hint" }, d.help) : null));
    }
    for (const n of out.slice(first)) n.setAttribute("translate", "no");
  }
  return out;
}
