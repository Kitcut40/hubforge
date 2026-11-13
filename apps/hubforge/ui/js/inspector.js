"use strict";
/* HubForge — Sélection, panneau de droite, réglage d'une touche et éditeurs d'actions. */

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
  if (sel.type === "settings" && sel.focus) S.setSec = sel.focus === "ai" ? "connect" : sel.focus;
  renderInspector();
}

/** Ferme le panneau de droite (ou les réglages). */
function deselect() { select({ type: "none" }); }

function renderInspector() {
  const box = $("#inspector");
  const layout = $(".layout");
  const t = S.sel.type;
  layout.classList.toggle("settings-mode", t === "settings");
  layout.classList.toggle("no-insp", t === "none" || t === "settings");
  box.innerHTML = "";
  const wasHidden = layout.dataset.cols !== layout.className;
  layout.dataset.cols = layout.className;
  if (wasHidden) requestAnimationFrame(fitDevice);
  if (t === "settings") { renderSettingsPage(); return; }
  if (t === "none") return;
  if (t === "key") { box.append(...keyInspector(S.sel.key)); return; }
  const make = { profile: profileInspector, fader: () => faderInspector(S.sel.index), rollers: rollersInspector, page: pageInspector, base: baseInspector, addons: addonsInspector, lights: lightsInspector }[t];
  if (!make) return;
  const items = make().filter(Boolean);
  const head = items.length && items[0].classList && items[0].classList.contains("insp-head") ? items.shift() : null;
  if (head) box.append(head);
  box.append(el("div", { class: "insp-body" }, ...items));
}

const CLOSE_SVG = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18"/></svg>';
function closeBtn() {
  return el("button", { class: "insp-close", "aria-label": "Fermer", title: "Fermer (Échap)", html: CLOSE_SVG, onclick: deselect });
}

/** En-tête d'un panneau : titre, sous-titre et bouton de fermeture. */
function title(text, sub, lead) {
  return el("div", { class: "insp-head" }, lead || null,
    el("div", { class: "insp-head-text" }, el("h3", {}, text), sub ? el("span", { class: "sub" }, sub) : null),
    closeBtn());
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
  const preview = el("div", { class: "insp-preview", id: "insp-preview" });
  setTimeout(renderKeyPreviewInInspector);
  const name = def ? ((def.label || "").replace(/\{[^}]*\}/g, "").trim() || (def.widget ? (WIDGETS.find(([v]) => v === def.widget) || [0, "Touche vivante"])[1] : `Touche ${k}`)) : `Touche ${k}`;
  const out = [title(name, `Touche ${k} · ${pageName}`, preview)];
  if (!def) {
    out.push(el("div", { class: "insp-empty" },
      el("p", {}, "Cette touche est libre."),
      el("p", { class: "hint" }, "Glisse une action depuis la liste de gauche, ou crée une touche ici."),
      el("button", { class: "btn primary", onclick: () => {
        setKeyDef(k, { key: k, label: "Nouvelle touche", color: "#3949AB", icon: "builtin:star", action: { type: "none" } });
        keyChanged(k, { immediate: true }); renderInspector();
      } }, "Créer une touche"),
      S.clipboard ? el("button", { class: "btn", onclick: () => pasteKey(k) }, "Coller la touche copiée") : null));
    return out;
  }
  if (!def.toggle) S.editState = 0;
  const tab = S.keyTab || "action";
  out.push(el("div", { class: "seg seg-full insp-tabs" },
    el("button", { class: "seg-btn" + (tab === "action" ? " on" : ""), onclick: () => { S.keyTab = "action"; renderInspector(); } }, "Action"),
    el("button", { class: "seg-btn" + (tab === "look" ? " on" : ""), onclick: () => { S.keyTab = "look"; renderInspector(); } }, "Apparence")));
  const body = [];
  if (def.toggle) {
    body.push(el("div", { class: "tabs" },
      el("button", { class: S.editState === 0 ? "on" : "", onclick: () => { S.editState = 0; renderInspector(); } }, "État 1"),
      el("button", { class: S.editState === 1 ? "on" : "", onclick: () => { S.editState = 1; renderInspector(); } }, "État 2")));
  }
  const st = S.editState === 1 && def.toggle ? def.toggle : def;
  const upd = (fn, opts) => { fn(st); keyChanged(k, opts); };

  if (tab === "action") {
    if (def.toggle) body.push(el("p", { class: "hint", style: "margin:-4px 0 12px" }, S.editState === 0
      ? "Action lancée quand la touche est dans l'état 1 ; elle passe ensuite à l'état 2."
      : "Action lancée quand la touche est dans l'état 2 ; elle revient ensuite à l'état 1."));
    body.push(...actionEditor(st, k, () => keyChanged(k)));
    body.push(...keyOptions(def, st, k));
    body.push(el("div", { class: "sep" }));
    body.push(switchRow("Touche à 2 états", !!def.toggle, (v) => {
      if (v) def.toggle = { label: (def.label || "") + " (2)", icon: def.icon, color: "#FF7A1A", text_color: def.text_color, action: clone(def.action || { type: "none" }) };
      else { delete def.toggle; S.editState = 0; }
      keyChanged(k, { immediate: true });
      renderInspector();
    }, "Elle change d'image et d'action à chaque appui (micro coupé / actif…)."));
    if (S.editState === 0 && (isAdv() || def.long || def.double)) body.push(...gestureEditors(def, k));
  } else {
  const live = S.editState === 0 && def.widget;
  if (S.editState === 0) {
    const wsel = el("select", { class: "input" }, ...WIDGETS.map(([v, l]) => el("option", { value: v, selected: v === (def.widget || "") }, l)));
    wsel.onchange = () => {
      delete def.widget_style;
      if (wsel.value) { def.widget = wsel.value; if (!def.color) def.color = "#1F2229"; } else delete def.widget;
      keyChanged(k, { immediate: true });
      renderInspector();
    };
    body.push(field("Touche vivante", wsel, live ? el("p", { class: "hint" }, "L'image se met à jour toute seule. L'action ci-dessous reste active quand on appuie.") : null));
    if (live) {
      const styles = WIDGET_STYLES[def.widget] || [];
      const cur = def.widget_style || (styles[0] || [""])[0];
      const row = el("div", { class: "chips" }, ...styles.map(([v, l]) => el("span", {
        class: "chip" + (v === cur ? " on style-on" : " add"),
        onclick: () => { if (v === styles[0][0]) delete def.widget_style; else def.widget_style = v; keyChanged(k, { immediate: true }); renderInspector(); },
      }, l)));
      body.push(field("Style", row));
      if (def.widget === "timer" && (def.widget_style || "stopwatch") === "countdown") {
        body.push(field("Durée", el("input", { class: "input", value: def.widget_target || "5:00", spellcheck: "false", placeholder: "ex. 5:00, 90, 1:30:00, 10 min",
          onchange: (e) => { def.widget_target = e.target.value.trim() || "5:00"; keyChanged(k, { immediate: true }); } }),
          el("p", { class: "hint" }, "À la fin, la touche clignote en rouge pendant 10 secondes.")));
      }
      if (def.widget === "disk") {
        body.push(field("Disque", el("input", { class: "input", value: def.widget_target || "C", maxlength: "2", spellcheck: "false", style: "width:80px",
          onchange: (e) => { def.widget_target = (e.target.value.trim()[0] || "C").toUpperCase(); keyChanged(k, { immediate: true }); } })));
      }
      if (def.widget === "media") {
        body.push(el("p", { class: "hint" }, "Ce que Windows affiche dans ses contrôles multimédia : Spotify, YouTube dans le navigateur, VLC…"));
      }
      if (def.widget === "app_volume") {
        body.push(field("Application(s)", el("input", { class: "input", value: def.widget_target || "", spellcheck: "false", placeholder: "ex. discord.exe",
          onchange: (e) => {
            const v = exeList(e.target.value);
            if (v) def.widget_target = v; else delete def.widget_target;
            // L'appui coupe le son de la même appli.
            if (def.action && def.action.type === "app_mute") def.action.app = v;
            keyChanged(k, { immediate: true }); renderInspector();
          } }), el("p", { class: "hint" }, "Le nom du programme, comme dans « Volume d'applications » des faders.")));
      }
      if (def.widget === "weather" && !(S.cfg.general.weather_city || "").trim()) {
        body.push(el("p", { class: "hint warn-text" }, "Choisis d'abord ta ville dans les réglages (section Météo)."));
      }
      if (["gpu", "gpu_temp", "vram"].includes(def.widget)) {
        body.push(el("p", { class: "hint" }, "Cartes NVIDIA (lu par le pilote, rien à installer)."));
      }
    }
  }
  // Style de la touche (thème)
  if (S.editState === 0) {
    const pageTheme = currentPage().theme || S.cfg.general.theme || "graphite";
    body.push(field("Style de la touche", themeChips(def.theme, `Comme la page (${themeName(pageTheme)})`, (v) => {
      if (v) def.theme = v; else delete def.theme;
      keyChanged(k, { immediate: true }); renderInspector();
    }), ownVisual(st)
      ? el("p", { class: "hint" }, "Cette touche affiche ta propre image ou une animation : elle reste telle quelle. Retire l'image pour utiliser un style.")
      : el("div", { class: "row", style: "flex-wrap:wrap;margin-top:8px" },
        el("button", { class: "btn small", onclick: () => {
          const t = effTheme(def, S.page);
          const p = currentPage();
          p.theme = t;
          for (const d of p.keys) delete d.theme;
          rethemed(); renderInspector(); toast(`Style « ${themeName(t)} » appliqué à toute la page`);
        } }, "Appliquer à toute la page"),
        el("button", { class: "btn small", onclick: async () => {
          const t = effTheme(def, S.page);
          if (!(await confirm_(`Mettre le style « ${themeName(t)} » sur toutes les touches du profil ? Les styles propres aux pages et aux touches seront retirés.`))) return;
          S.cfg.general.theme = t;
          for (const p of S.cfg.pages) { delete p.theme; for (const d of p.keys) delete d.theme; }
          rethemed(); renderInspector(); toast(`Style « ${themeName(t)} » appliqué à tout le profil`);
        } }, "À tout le profil"))));
  }
  body.push(field(live ? "Titre (facultatif)" : "Texte affiché", el("input", { id: "label-input", class: "input", value: st.label || "",
    placeholder: live ? "(titre par défaut)" : "(aucun)", oninput: (e) => upd((d) => { d.label = e.target.value; delete d._autoName; }) })));
  if (!live) body.push(iconField(k, st, upd));
  // Avec un style (thème), la couleur sert d'accent et la couleur du texte ne sert pas.
  const themed = effTheme(def, S.page) !== "classic" && !ownVisual(st);
  body.push(colorField(themed ? "Couleur d'accent" : "Couleur de fond", st.color, (v) => upd((d) => { if (v) d.color = v; else delete d.color; })));
  if (!themed) body.push(colorField("Couleur du texte et de l'icône", st.text_color || "#FFFFFF", (v) => upd((d) => { if (v) d.text_color = v; else delete d.text_color; })));

  }
  out.push(el("div", { class: "insp-body" }, ...body));
  out.push(el("div", { class: "insp-foot" },
    el("button", { class: "btn grow", title: "Ctrl+C", onclick: () => copyKey(k) }, "Copier"),
    el("button", { class: "btn grow", title: "Ctrl+V", disabled: !S.clipboard, onclick: () => pasteKey(k) }, "Coller"),
    el("button", { class: "btn danger grow", onclick: async () => {
      if (!(await confirm_(`Vider la touche ${k} ?`))) return;
      setKeyDef(k, null); delete S.previews[`${S.page}:${k}:0`]; delete S.previews[`${S.page}:${k}:1`]; updateKeyNode(k); changed(); renderInspector();
    } }, "Vider")));
  return out;
}

/** Options d'une touche : fermer l'appli lancée, confirmation, répétition. */
function keyOptions(def, st, k) {
  const out = [];
  const a = st.action || {};
  // Touche « Lancer » d'un programme : point vert et appui long pour le fermer.
  const exe = a.type === "launch" && /\.exe"?\s*$/i.test(a.target || "") && !/:\/\//.test(a.target || "")
    ? (a.target || "").trim().replace(/^"|"$/g, "").split(/[\\/]/).pop().toLowerCase() : null;
  if (exe && S.editState === 0) {
    const cur = def.long && def.long.type === "close_app" ? (def.long.force ? "force" : "close") : def.long ? "custom" : "none";
    if (cur !== "custom") {
      out.push(field("Appui long", el("select", { class: "input", onchange: (e) => {
        const v = e.target.value;
        if (v === "none") delete def.long; else def.long = { type: "close_app", target: exe, ...(v === "force" ? { force: true } : {}) };
        keyChanged(k, { immediate: true }); renderInspector();
      } }, ...[["none", "Rien"], ["close", `Fermer ${exe}`], ["force", `Forcer la fermeture de ${exe}`]]
        .map(([v, l]) => el("option", { value: v, selected: v === cur }, l))),
        el("p", { class: "hint" }, "Un point vert s'affiche sur la touche quand l'application est ouverte.")));
    } else {
      out.push(el("p", { class: "hint" }, "Un point vert s'affiche sur la touche quand l'application est ouverte."));
    }
  }
  if (S.editState === 0) {
    out.push(switchRow("Demander une confirmation", !!def.confirm, (v) => {
      if (v) def.confirm = true; else delete def.confirm; keyChanged(k);
    }, "Il faut appuyer deux fois (la touche s'entoure de jaune) : pour « Arrêter le stream », « Éteindre le PC »…"));
    const canRepeat = ["hotkey", "text", "media", "var"].includes(a.type) && !def.toggle && !def.long && !def.double;
    if (canRepeat || def.repeat) out.push(switchRow("Répéter tant que la touche est enfoncée", !!def.repeat, (v) => {
      if (v) def.repeat = true; else delete def.repeat; keyChanged(k);
    }, "Pratique pour le volume, le zoom ou faire défiler."));
  }
  return out;
}

/** Appui long et double appui d'une touche (actions supplémentaires). */
function gestureEditors(def, k) {
  const out = [el("div", { class: "sep" }), el("h4", { class: "insp-sub" }, "Appui long et double appui")];
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
  const isAnim = icon.startsWith("anim:");
  const current = el("div", { class: "ic" });
  if (isBuiltin) current.innerHTML = svgIcon(icon.slice(8));
  else if (isAnim) current.innerHTML = svgIcon((ANIMS.find(([v]) => v === icon.slice(5)) || [0, 0, "sparkles"])[2]);
  else if (icon) current.append(el("span", { style: "font-size:10px;color:var(--muted);padding:2px;text-align:center" }, "image"));
  const name = !icon ? "Aucune" : isBuiltin ? icon.slice(8) : isAnim ? "Animation : " + animName(icon.slice(5)) : icon.split(/[\\/]/).pop();

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
    const anims = el("div", { class: "chips", style: "margin-top:8px" }, ...ANIMS.map(([id, nm]) => el("span", {
      class: "chip" + (icon === "anim:" + id ? " on style-on" : " add"),
      onclick: () => { upd((d) => { d.icon = "anim:" + id; }, { immediate: true }); renderInspector(); },
    }, nm)));
    out.append(el("div", { class: "icon-picker" }, search, grid, el("div", { class: "step-branch" }, "Animations"), anims, iconPacksBlock(icon, upd)));
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

/** Mode simple : on cache ce qui sert surtout aux utilisateurs avancés. */
const isAdv = () => S.uiMode !== "simple";
const SIMPLE_HIDDEN_CATS = ["Macros intelligentes"];
const SIMPLE_HIDDEN_ITEMS = ["Maintenir des touches", "Macro (plusieurs actions)", "Requête web", "Webhook Discord", "Bascule personnalisée"];
const ADV_TYPES = ["hold", "macro", "var", "if", "http", "open_config"];

/** Valeurs par défaut quand on choisit un type d'action. */
const ACTION_DEFAULTS = {
  none: { type: "none" }, hotkey: { type: "hotkey", keys: "" }, hold: { type: "hold", keys: "" }, text: { type: "text", text: "" },
  sound: { type: "sound", file: "", volume: 100, mode: "toggle" }, stop_sounds: { type: "stop_sounds" },
  "launch:program": { type: "launch", target: "" }, "launch:url": { type: "launch", target: "https://" },
  "launch:path": { type: "launch", target: "", _kind: "path" }, media: { type: "media", command: "play_pause" }, app_mute: { type: "app_mute", app: "" }, audio_output: { type: "audio_output", devices: "" }, timer: { type: "timer", op: "toggle" },
  page: { type: "page", to: "next" }, lock: { type: "lock" }, open_config: { type: "open_config" },
  macro: { type: "macro", steps: [{ type: "hotkey", keys: "" }] }, wait: { type: "wait", ms: 500 },
  folder: { type: "folder", to: "" }, back: { type: "back" },
  profile: { type: "profile", to: "next" }, game: { type: "game" },
  var: { type: "var", name: "compteur", op: "add", value: "1" },
  if: { type: "if", left: "", cmp: "==", right: "", then: [], else: [] },
  ai: { type: "ai", preset: "fix", output: "replace" },
  close_app: { type: "close_app", target: "" },
  http: { type: "http", method: "GET", url: "https://" },
};

const AI_PRESETS = [["fix", "Corriger l'orthographe"], ["translate_en", "Traduire en anglais"], ["translate_fr", "Traduire en français"],
  ["summarize", "Résumer"], ["formal", "Style professionnel"], ["custom", "Consigne perso…"]];
const AI_OUTPUTS = [["replace", "Remplacer la sélection"], ["clipboard", "Copier le résultat (à coller où tu veux)"], ["var", "Ranger dans une variable"]];
const AI_PROVIDERS = [
  ["", "Pas d'IA", "", ""],
  ["ollama", "Ollama (sur ton PC, gratuit)", "http://localhost:11434", "llama3.2"],
  ["openai", "OpenAI ou service compatible (clé API)", "https://api.openai.com/v1", "gpt-4o-mini"],
  ["anthropic", "Claude d'Anthropic (clé API)", "https://api.anthropic.com", "claude-haiku-5-5"],
];

/** Résumé d'une étape de macro (affiché dans son en-tête). */
function stepSummary(a) {
  switch (a.type) {
    case "hotkey": return a.keys ? `Raccourci ${a.keys}` : "Raccourci clavier";
    case "hold": return a.keys ? `Appuyer sur ${a.keys}` : "Touches";
    case "sound": return a.file ? `Son ${a.file.split(/[\\/]/).pop()}` : "Jouer un son";
    case "stop_sounds": return "Arrêter les sons";
    case "text": return a.text ? `Taper « ${a.text.length > 24 ? a.text.slice(0, 24) + "…" : a.text} »` : "Taper un texte";
    case "launch": return a.target ? `Ouvrir ${a.target.split(/[\\/]/).pop()}` : "Ouvrir…";
    case "media": return (MEDIA[a.command] || [a.command])[0];
    case "close_app": return `Fermer ${(a.target || "une appli").replace(/\.exe$/i, "")}`;
    case "app_mute": return `Couper ${(a.app || "une appli").split(",")[0].replace(/\.exe$/, "")}`;
    case "audio_output": {
      const d = (a.devices || "").split("|").filter(Boolean);
      return d.length === 1 ? `Sortie : ${d[0]}` : d.length ? `Sortie suivante (${d.length})` : "Sortie audio suivante";
    }
    case "timer": return a.op === "reset" ? "Minuteur à zéro" : "Minuteur : départ / pause";
    case "page": return a.to === "next" ? "Page suivante" : a.to === "previous" ? "Page précédente" : `Page ${a.to}`;
    case "lock": return "Verrouiller le PC";
    case "open_config": return "Ouvrir la configuration";
    case "wait": return `Pause ${(a.ms / 1000).toLocaleString("fr-FR")} s`;
    case "folder": { const p = S.cfg.pages[Number(a.to) - 1]; return `Ouvrir le dossier ${p ? "« " + p.name + " »" : a.to}`; }
    case "back": return "Retour (fermer le dossier)";
    case "profile": return a.to === "next" ? "Profil suivant" : a.to === "previous" ? "Profil précédent" : `Profil « ${a.to} »`;
    case "game": return "Mini-jeu";
    case "var": {
      const n = a.name || "?";
      if (a.op === "toggle") return `Basculer ${n}`;
      if (a.op === "add") { const v = String(a.value || "0").trim(); return v.startsWith("-") ? `${n} − ${v.slice(1)}` : `${n} + ${v}`; }
      return `${n} = ${a.value || "(vide)"}`;
    }
    case "ai": return "IA : " + ((AI_PRESETS.find(([v]) => v === a.preset) || AI_PRESETS[0])[1]).replace("…", "");
    case "http": { let h = a.url || ""; try { h = new URL(a.url).host; } catch (_) {} return `${a.method || "GET"} ${h || "…"}`; }
    case "if": return `Si ${a.left || "…"} ${(CMPS.find(([v]) => v === a.cmp) || CMPS[0])[2]} ${a.right || "(vide)"}`;
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
  if (!isAdv()) types = types.filter(([v]) => !ADV_TYPES.includes(v) || v === actionTypeId(a));
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
  const out = opts.extra ? [field(opts.extra, typeSel)] : nested ? [field("Type", typeSel)] : [field("Quand j'appuie sur la touche", typeSel)];
  const t = actionTypeId(a);
  const autoIcon = (p) => {
    if (nested) return;
    // Icône et nom automatiques (l'icône seulement si la touche n'a pas déjà sa propre image).
    const ownImage = st.icon && !st.icon.startsWith("builtin:") && !st.icon.startsWith("icons/app-");
    applyAppIcon(k, st, p, { quiet: true, keepIcon: ownImage }).then((ok) => { if (!ok) renderInspector(); });
  };

  if (t === "hotkey" || t === "hold") {
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
    out.push(field(t === "hold" ? "Touches à maintenir" : "Combinaison", cap, el("div", { style: "height:8px" }), extra,
      t === "hold"
        ? el("p", { class: "hint" }, "Ces touches restent enfoncées tant que tu tiens la touche du MasterHUB : klaxon, accélérateur, appuyer-pour-parler… (dans une macro ou un appui long : simple appui).")
        : nested ? null : el("p", { class: "hint" }, "Ex. ctrl+shift+s · win+d · alt+f4. Les combinaisons avec la touche Windows s'ajoutent avec la liste.")));
  } else if (t === "sound") {
    const name = el("input", { class: "input grow", value: a.file || "", placeholder: "Choisis un fichier son…", readonly: true });
    out.push(field("Son", el("div", { class: "row" }, name, el("button", { class: "btn", onclick: async () => {
      const p = await invoke("pick_file", { kind: "sound" });
      if (!p) return;
      try {
        const rel = await invoke("import_sound", { path: p });
        upd((x) => { x.file = rel; });
        if (!nested && labelIsGeneric(st)) { st.label = rel.split("/").pop().replace(/\.[^.]+$/, ""); keyChanged(k); }
        renderInspector();
      } catch (e) { toast(String(e), true); }
    } }, "Choisir…")), el("p", { class: "hint" }, "mp3, wav, ogg, flac ou m4a. Astuce : glisse un fichier son directement sur une touche.")));
    out.push(rangeField("Volume de ce son", a.volume ?? 100, 0, 100, " %", (v) => upd((x) => { x.volume = v; })));
    out.push(field("Quand je rappuie pendant la lecture", el("select", { class: "input", onchange: (e) => upd((x) => { x.mode = e.target.value; }) },
      ...[["toggle", "Le son s'arrête"], ["restart", "Le son recommence"], ["overlap", "Il se rejoue par-dessus"]].map(([v, l]) => el("option", { value: v, selected: v === (a.mode || "toggle") }, l)))));
    out.push(el("button", { class: "btn small", disabled: !a.file, onclick: () => invoke("test_sound", { file: a.file, volume: a.volume ?? 100 }) }, "▶ Écouter sur le PC"));
  } else if (t === "stop_sounds") {
    out.push(el("p", { class: "hint" }, "Coupe tous les sons de la soundboard en cours de lecture."));
  } else if (t === "text") {
    out.push(field("Texte à taper", el("textarea", { class: "input", oninput: (e) => upd((x) => { x.text = e.target.value; }) }, a.text || ""),
      el("p", { class: "hint" }, "Astuce : {nom} tape la valeur d'une variable, {app} l'appli au premier plan, {hour}:{minute} l'heure.")));
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
  } else if (t === "audio_output") {
    const chosen = (a.devices || "").split("|").filter(Boolean);
    const box = el("div", { class: "chips" }, el("span", { class: "hint" }, "Recherche des sorties…"));
    out.push(field("Sorties", box, el("p", { class: "hint" },
      "Une seule cochée : la touche passe sur celle-ci. Plusieurs : chaque appui passe à la suivante. Aucune : passe d'une sortie à l'autre parmi toutes.")));
    invoke("list_audio_outputs").then((list) => {
      box.innerHTML = "";
      const names = [...new Set([...(list || []), ...chosen])];
      if (!names.length) box.append(el("span", { class: "hint" }, "Aucune sortie trouvée."));
      for (const n of names) box.append(el("span", {
        class: "chip" + (chosen.includes(n) ? " on style-on" : " add"),
        onclick: () => {
          const cur = (a.devices || "").split("|").filter(Boolean);
          const next = cur.includes(n) ? cur.filter((x) => x !== n) : [...cur, n];
          upd((x) => { x.devices = next.join("|"); }); renderInspector();
        },
      }, n));
    }).catch(() => { box.innerHTML = ""; box.append(el("span", { class: "hint" }, "Liste indisponible.")); });
  } else if (t === "timer") {
    out.push(field("Effet", el("select", { class: "input", onchange: (e) => upd((x) => { x.op = e.target.value; }) },
      ...[["toggle", "Démarrer / mettre en pause"], ["reset", "Remettre à zéro"]].map(([v, l]) => el("option", { value: v, selected: v === (a.op || "toggle") }, l))),
      el("p", { class: "hint" }, "Agit sur le minuteur de cette touche (touche vivante « Minuteur »). Astuce : mets « Remettre à zéro » en appui long.")));
  } else if (t === "close_app") {
    out.push(field("Application", el("input", { class: "input", value: a.target || "", spellcheck: "false", placeholder: "ex. discord.exe",
      onchange: (e) => upd((x) => { x.target = exeList(e.target.value).split(",")[0] || ""; }) })));
    out.push(switchRow("Forcer la fermeture", !!a.force, (v) => upd((x) => { if (v) x.force = true; else delete x.force; }),
      "Ferme même si l'application ne répond plus. Attention : un travail non enregistré est perdu."));
  } else if (t === "app_mute") {
    out.push(field("Applications", el("input", { class: "input", value: a.app || "", spellcheck: "false", placeholder: "ex. discord.exe, spotify.exe",
      onchange: (e) => upd((x) => { x.app = exeList(e.target.value); }) }),
      el("p", { class: "hint" }, "Un appui coupe le son, le suivant le rétablit. Plusieurs applications séparées par des virgules.")));
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
  } else if (t === "var") {
    const listId = "vars-" + Math.random().toString(36).slice(2);
    out.push(field("Nom de la variable", el("input", { class: "input", value: a.name || "", list: listId, spellcheck: "false", placeholder: "ex. compteur",
      oninput: (e) => upd((x) => { x.name = e.target.value.trim(); }) }),
      el("datalist", { id: listId }, ...knownVars().map((n) => el("option", { value: n })))));
    out.push(field("Opération", el("select", { class: "input", onchange: (e) => { upd((x) => { x.op = e.target.value; if (x.op === "toggle") x.value = ""; }); renderInspector(); } },
      ...[["add", "Ajouter (nombre négatif pour retirer)"], ["set", "Donner la valeur"], ["toggle", "Basculer 0 ↔ 1"]].map(([v, l]) => el("option", { value: v, selected: v === (a.op || "set") }, l)))));
    if (a.op !== "toggle") out.push(field(a.op === "add" ? "Nombre à ajouter" : "Valeur", el("input", { class: "input", value: a.value ?? "", spellcheck: "false",
      placeholder: a.op === "add" ? "ex. 1 ou -1" : "ex. 0, oui, {autre}", oninput: (e) => upd((x) => { x.value = e.target.value; }) })));
    const cur = S.status && S.status.vars && a.name ? S.status.vars[a.name.toLowerCase()] : undefined;
    out.push(el("p", { class: "hint" }, cur !== undefined ? `Valeur actuelle : ${cur}. ` : "",
      "Pour l'afficher sur une touche, écris ", el("b", {}, `{${a.name || "nom"}}`), " dans son texte. Les variables sont gardées même après un redémarrage."));
  } else if (t === "ai") {
    out.push(field("Que faire du texte sélectionné ?", el("select", { class: "input", onchange: (e) => { upd((x) => { x.preset = e.target.value; }); renderInspector(); } },
      ...AI_PRESETS.map(([v, l]) => el("option", { value: v, selected: v === (a.preset || "fix") }, l)))));
    if (a.preset === "custom") out.push(field("Consigne", el("textarea", { class: "input", rows: 3, placeholder: "ex. Mets ce texte en liste à puces. / Réponds poliment à ce message.",
      oninput: (e) => upd((x) => { x.prompt = e.target.value; }) }, a.prompt || "")));
    out.push(field("Résultat", el("select", { class: "input", onchange: (e) => { upd((x) => { x.output = e.target.value; }); renderInspector(); } },
      ...AI_OUTPUTS.map(([v, l]) => el("option", { value: v, selected: v === (a.output || "replace") }, l)))));
    if (a.output === "var") out.push(varTargetField(a, upd, "ex. resume"));
    out.push(el("p", { class: "hint" }, "Sélectionne un texte dans n'importe quelle appli, puis appuie sur la touche : elle s'entoure d'orange pendant que l'IA travaille, de vert quand c'est fait, de rouge en cas d'erreur. ",
      el("a", { href: "#", style: "color:var(--accent-2)", onclick: (e) => { e.preventDefault(); select({ type: "settings", focus: "ai" }); } }, "Régler l'IA")));
  } else if (t === "http") {
    out.push(field("Méthode et adresse", el("div", { class: "row", style: "gap:6px" },
      el("select", { class: "input", style: "width:96px;flex:none", onchange: (e) => { upd((x) => { x.method = e.target.value; }); renderInspector(); } },
        ...["GET", "POST", "PUT", "PATCH", "DELETE"].map((m) => el("option", { value: m, selected: m === (a.method || "GET") }, m))),
      el("input", { class: "input", value: a.url || "", spellcheck: "false", placeholder: "https://…", oninput: (e) => upd((x) => { x.url = e.target.value.trim(); }) }))));
    if ((a.method || "GET") !== "GET") out.push(field("Corps (JSON ou texte)", el("textarea", { class: "input mono", rows: 4, spellcheck: "false", placeholder: '{"content": "Bonjour"}',
      oninput: (e) => upd((x) => { x.body = e.target.value; }) }, a.body || "")));
    const adv = el("details", { class: "adv" }, el("summary", {}, "En-têtes (clé API, jeton…)"),
      el("textarea", { class: "input mono", rows: 3, spellcheck: "false", placeholder: "Authorization: Bearer xxx\nX-Api-Key: xxx",
        oninput: (e) => upd((x) => { x.headers = e.target.value; }) }, a.headers || ""));
    if (a.headers) adv.open = true;
    out.push(adv);
    out.push(varTargetField(a, upd, "ex. temperature", "Ranger la réponse dans la variable (facultatif)"));
    if (a.save_to) out.push(field("Champ de la réponse JSON (facultatif)", el("input", { class: "input mono", value: a.pick || "", spellcheck: "false", placeholder: "ex. data.temperature ou items.0.name",
      oninput: (e) => upd((x) => { x.pick = e.target.value.trim(); }) })));
    const result = el("p", { class: "hint" });
    out.push(el("div", { class: "row", style: "gap:6px;align-items:center" }, el("button", { class: "btn small", onclick: async () => {
      result.textContent = "Envoi…";
      try { const r = await invoke("test_http", { method: a.method || "GET", url: a.url || "", body: a.body || "", headers: a.headers || "", pick: a.save_to ? (a.pick || "") : "" });
        result.textContent = "Réponse : " + (r || "(vide)"); }
      catch (e) { result.textContent = "Erreur : " + e; }
    } }, "Essayer"), result));
    out.push(el("p", { class: "hint" }, "L'adresse, le corps et les en-têtes acceptent les {variables}. Affiche la réponse sur une touche en écrivant {nom} dans son texte. Attention : « Essayer » envoie vraiment la requête."));
  } else if (t === "if") {
    if (!Array.isArray(a.then)) a.then = [];
    if (!Array.isArray(a["else"])) a["else"] = [];
    const left = el("input", { class: "input", value: a.left || "", spellcheck: "false", placeholder: "ex. {compteur} ou {app}", oninput: (e) => upd((x) => { x.left = e.target.value; }) });
    const chips = el("div", { class: "chips", style: "margin-top:6px" },
      ...[...knownVars().map((n) => ["{" + n + "}", "variable"]), ...VAR_SPECIALS].map(([v, l]) =>
        el("span", { class: "chip add", title: l, onclick: () => { left.value = v; upd((x) => { x.left = v; }); } }, v)));
    out.push(field("Si", left, chips));
    out.push(field("", el("select", { class: "input", onchange: (e) => upd((x) => { x.cmp = e.target.value; }) },
      ...CMPS.map(([v, l]) => el("option", { value: v, selected: v === (a.cmp || "==") }, l)))));
    out.push(field("Valeur", el("input", { class: "input", value: a.right || "", spellcheck: "false", placeholder: "ex. 10, obs64.exe, {autre}",
      oninput: (e) => upd((x) => { x.right = e.target.value; }) }),
      el("p", { class: "hint" }, "Compare des nombres si les deux côtés en sont, sinon du texte (sans tenir compte des majuscules). Une variable jamais utilisée vaut 0.")));
    out.push(...stepsEditor(a.then, k, onChange, { title: "Alors", hint: "Exécuté si la condition est vraie.", compact: true }));
    out.push(...stepsEditor(a["else"], k, onChange, { title: "Sinon", hint: "Exécuté si elle est fausse (peut rester vide).", compact: true }));
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
    if (a.type === "if") { a.then = (a.then || []).map(fix).filter(Boolean); a["else"] = (a["else"] || []).map(fix).filter(Boolean); return a; }
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
    if (d.long) d.long = fix(d.long) || { type: "none" };
    if (d.double) d.double = fix(d.double) || { type: "none" };
  }
  for (const w of wheelActionSets()) { w.up = fix(w.up) || { type: "none" }; w.down = fix(w.down) || { type: "none" }; }
}

/** Liste des étapes d'une macro, chacune avec son propre éditeur. */
function macroEditor(a, k, onChange) {
  if (!Array.isArray(a.steps)) a.steps = [];
  return stepsEditor(a.steps, k, onChange, {
    title: "Étapes", hint: "Exécutées dans l'ordre, de haut en bas. Ajoute des pauses pour laisser le temps à une application de s'ouvrir.",
  });
}

/** Éditeur d'une liste d'étapes (macro, branches « alors » / « sinon » d'une condition). */
function stepsEditor(steps, k, onChange, opts) {
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
    opts.compact ? el("div", { class: "step-branch" }, `${opts.title} (${steps.length})`) : el("h2", { style: "margin-top:4px" }, `${opts.title} (${steps.length})`),
    el("p", { class: "hint", style: "margin:-4px 0 10px" }, opts.hint),
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
