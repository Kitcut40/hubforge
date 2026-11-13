"use strict";
/* HubForge — Réglages, modes simple / avancé, assistant, mises à jour, boîtes de dialogue. */

// --- Réglages : une page à part, rangée par sections ---
const SETTING_SECTIONS = [
  ["general", "Général", "sliders-horizontal", "Ce qui s'applique à tout ton MasterHUB."],
  ["look", "Apparence", "palette", "Le style des touches, les LED et les animations."],
  ["sound", "Son", "volume-2", "Soundboard et faders."],
  ["connect", "Météo et IA", "sparkles", "Les services en ligne utilisés par certaines touches."],
  ["sleep", "Veille", "moon", "Éteindre les écrans et les LED quand tu n'es pas là."],
  ["advanced", "Avancé", "zap", "Délais des touches, pages automatiques, variables.", true],
  ["updates", "Mises à jour", "download", "Nouvelles versions de HubForge."],
  ["about", "À propos", "info", "Version, licence, dépannage."],
];

function renderSettingsPage() {
  const page = $("#settings-page");
  const secs = SETTING_SECTIONS.filter((x) => !x[4] || isAdv());
  if (!secs.some(([id]) => id === S.setSec)) S.setSec = "general";
  const cur = secs.find(([id]) => id === S.setSec);
  const nav = el("nav", { class: "set-nav", "aria-label": "Sections des réglages" },
    el("div", { class: "set-nav-head" },
      el("button", { class: "set-back", "aria-label": "Retour au MasterHUB", title: "Retour (Échap)", onclick: deselect,
        html: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 6l-6 6 6 6"/></svg>' }),
      el("h1", {}, "Réglages")),
    ...secs.map(([id, label, icon]) => el("button", { class: "set-nav-item" + (id === S.setSec ? " on" : ""), onclick: () => { S.setSec = id; renderSettingsPage(); } },
      el("span", { class: "set-nav-ic", html: svgIcon(icon) || svgIcon("settings") }), label)));
  const body = el("div", { class: "set-body" }, el("h2", {}, cur[1]), el("p", { class: "set-lead" }, cur[3]), ...settingsSection(cur[0]));
  page.innerHTML = "";
  page.append(nav, el("section", { class: "set-content" }, body));
}

function settingsSection(id) {
  const g = S.cfg.general;
  const out = [];
  if (id === "general") {
    out.push(setRow("Affichage", isAdv()
      ? "Mode avancé : tout est visible (macros, variables, conditions, requêtes web, appui long…)."
      : "Mode simple : l'essentiel seulement. Le mode avancé montre aussi les macros, variables et requêtes web.", modeSwitch()));
    out.push(setRow("Langue", "Language — the interface and the text drawn on the keys.", langSwitch()));
    out.push(setRow("Luminosité des écrans", "Les 15 écrans des touches.", rangeCtl(g.brightness, 0, 100, " %", (v) => { g.brightness = v; changed(); })));
    out.push(setRow("Luminosité des LED", "Base et faders.", rangeCtl(g.led_brightness, 0, 100, " %", (v) => { g.led_brightness = v; changed(); })));
    out.push(setRow("Démarrer avec Windows", "HubForge se lance discrètement dans la zone de notification. Fermer la fenêtre ne coupe pas le MasterHUB.",
      toggleCtl(S.autostart, async (v) => { try { S.autostart = await invoke("set_autostart", { on: v }); } catch (e) { toast(String(e), true); } renderInspector(); })));
    out.push(setRow("Assombrir la touche pendant l'appui", "Un retour visuel à chaque appui.", toggleCtl(g.press_feedback, (v) => { g.press_feedback = v; changed(); })));
    out.push(setRow("Mode nuit", "Écrans et LED moins lumineux le soir, tout seuls.", toggleCtl(g.night_dim, (v) => { g.night_dim = v; changed(); renderInspector(); })));
    if (g.night_dim) {
      const hourSel = (val, onPick) => el("select", { class: "input", style: "width:96px", onchange: (e) => onPick(Number(e.target.value)) },
        ...Array.from({ length: 24 }, (_, h) => el("option", { value: h, selected: h === val }, `${h} h`)));
      out.push(setRow("Horaires du mode nuit", "De l'heure de début à l'heure de fin (le lendemain si elle est plus tôt).",
        el("div", { class: "row", style: "gap:8px;align-items:center" },
          hourSel(g.night_from ?? 22, (v) => { g.night_from = v; changed(); }), el("span", { class: "hint", style: "margin:0" }, "à"),
          hourSel(g.night_to ?? 7, (v) => { g.night_to = v; changed(); }))));
      out.push(setRow("Luminosité la nuit", "En pourcentage de la luminosité normale.", rangeCtl(g.night_level ?? 30, 5, 100, " %", (v) => { g.night_level = v; changed(); })));
    }
    out.push(setRow("Mode dynamique", "Un profil peut s'activer tout seul quand son application passe au premier plan (menu des profils › Applications de ce profil).",
      toggleCtl(S.autoProfiles, async (v) => { S.autoProfiles = v; try { await invoke("set_auto_profiles", { on: v }); } catch (e) { toast(String(e), true); } })));
    out.push(setCard("sparkles", "Assistant de démarrage", "Choisir un style et un profil prêt à l'emploi en quatre étapes.",
      el("button", { class: "btn", onclick: () => openWizard() }, "Lancer")));
  } else if (id === "look") {
    out.push(setBlock("Style des touches du profil", "Chaque page et chaque touche peut aussi avoir son propre style. Tes images et GIF restent tels quels.",
      themeChips(g.theme || "graphite", null, (v) => { g.theme = v; rethemed(); renderInspector(); }),
      S.cfg.pages.some((p) => p.theme || p.keys.some((d) => d.theme)) ? el("button", { class: "btn small", style: "margin-top:10px", onclick: async () => {
        if (!(await confirm_("Retirer les styles propres aux pages et aux touches, pour que tout le profil suive ce style ?"))) return;
        for (const p of S.cfg.pages) { delete p.theme; for (const d of p.keys) delete d.theme; }
        rethemed(); renderInspector();
      } }, "Appliquer à toutes les pages et touches") : null));
    out.push(setRow("Base", "Couleur des LED, orientation sur le bureau, positions des modules.", el("button", { class: "btn", onclick: () => select({ type: "base" }) }, "Régler")));
    out.push(setRow("Animations", "Effets lumineux de la base et des faders.", el("button", { class: "btn", onclick: () => select({ type: "lights" }) }, "Ouvrir")));
  } else if (id === "sound") {
    const devSel = (value, onPick, emptyLabel) => {
      const sel = el("select", { class: "input", onchange: (e) => onPick(e.target.value) }, el("option", { value: "" }, emptyLabel));
      if (value) sel.append(el("option", { value, selected: true }, value));
      invoke("list_sound_devices").then((list) => {
        for (const d of list || []) if (d !== value) sel.append(el("option", { value: d }, d));
      });
      return sel;
    };
    out.push(setRow("Volume des sons", "Le soundboard : les touches « Jouer un son ».", rangeCtl(g.sound_volume, 0, 100, " %", (v) => { g.sound_volume = v; changed(); })));
    out.push(setBlock("Sortie des sons", null, devSel(g.sound_device, (v) => { g.sound_device = v; changed(); }, "Sortie par défaut de Windows")));
    out.push(setBlock("Jouer aussi sur", "Pour que tes amis entendent les sons dans Discord ou sur ton stream : choisis un câble audio virtuel (par exemple VB-CABLE) et mets-le comme micro dans Discord ou OBS.",
      devSel(g.sound_device2, (v) => { g.sound_device2 = v; changed(); }, "Aucune autre sortie")));
    if (isAdv()) out.push(setRow("Reprise douce des faders", "Quand un fader contrôle autre chose en changeant de page ou de profil, il n'agit qu'une fois passé par le volume actuel. Sa LED pulse en attendant.",
      toggleCtl(g.fader_pickup, (v) => { g.fader_pickup = v; changed(); })));
  } else if (id === "connect") {
    out.push(setBlock("Ville des touches météo", "Prévisions Open-Meteo, gratuites et sans compte. HubForge ne se connecte à internet que si une touche météo existe.",
      el("input", { class: "input", value: g.weather_city || "", placeholder: "ex. Toulouse", spellcheck: "false", onchange: (e) => { g.weather_city = e.target.value.trim(); changed(); } })));
    out.push(el("h3", { class: "set-h3", id: "settings-ai" }, "Intelligence artificielle"));
    out.push(el("div", { class: "set-block" }, aiSettingsBlock()));
  } else if (id === "sleep") {
    out.push(setRow("Mettre le MasterHUB en veille après", "Sans toucher ni le PC ni le MasterHUB. Un appui le rallume, sans lancer l'action.",
      el("select", { class: "input", style: "width:180px", onchange: (e) => { g.sleep_after = Number(e.target.value); changed(); } },
        ...[[0, "Jamais"], [1, "1 minute"], [2, "2 minutes"], [5, "5 minutes"], [10, "10 minutes"], [15, "15 minutes"], [30, "30 minutes"], [60, "1 heure"]]
          .map(([v, l]) => el("option", { value: v, selected: v === (g.sleep_after || 0) }, l)))));
    out.push(setRow("En veille quand le PC est verrouillé", "Écrans et LED s'éteignent dès que tu verrouilles la session.", toggleCtl(g.sleep_locked, (v) => { g.sleep_locked = v; changed(); })));
  } else if (id === "advanced") {
    out.push(setRow("Durée d'un appui long", "Temps à garder la touche enfoncée.", rangeCtl(g.long_press_ms, 200, 1500, " ms", (v) => { g.long_press_ms = v; changed(); })));
    out.push(setRow("Délai d'un double appui", "Plus il est court, plus l'appui simple des touches à double appui est rapide.", rangeCtl(g.double_press_ms, 150, 600, " ms", (v) => { g.double_press_ms = v; changed(); })));
    out.push(setRow("Pages automatiques", "Une page liée à une application s'affiche quand elle passe au premier plan (bouton ⚙ de l'onglet de la page).", toggleCtl(g.auto_pages, (v) => { g.auto_pages = v; changed(); })));
    out.push(setRow("Fluidité max. des GIF", "Partagée entre toutes les touches animées.", rangeCtl(g.gif_max_fps, 5, 40, " i/s", (v) => { g.gif_max_fps = v; changed(); })));
    out.push(el("h3", { class: "set-h3" }, "Variables"));
    const vars = Object.entries((S.status && S.status.vars) || {});
    out.push(vars.length
      ? el("div", { class: "vars-list" }, ...vars.map(([n, v]) => el("div", { class: "row var-row" }, el("code", {}, "{" + n + "}"), el("span", { class: "spacer" }), el("b", {}, v))))
      : el("p", { class: "hint" }, "Aucune variable pour l'instant. Crée-en avec l'action « Variable » (Macros intelligentes)."));
    out.push(setRow("Écrire les variables dans des fichiers texte", "Un fichier par variable (nom.txt), à afficher dans OBS avec une source « Texte » → « Lire depuis un fichier ».",
      toggleCtl(g.vars_files, (v) => { g.vars_files = v; changed(); renderInspector(); })));
    const vbtns = [];
    if (g.vars_files) vbtns.push(el("button", { class: "btn", onclick: async () => { try { await invoke("open_vars_folder"); } catch (err) { toast(String(err), true); } } }, "Ouvrir le dossier"));
    if (vars.length) vbtns.push(el("button", { class: "btn", onclick: async () => {
      if (!(await confirm_("Remettre toutes les variables à zéro ?"))) return;
      await invoke("clear_vars"); toast("Variables remises à zéro");
    } }, "Remettre à zéro"));
    if (vbtns.length) out.push(el("div", { class: "row", style: "margin-top:12px" }, ...vbtns));
  } else if (id === "updates") {
    out.push(...updateSettings());
  } else {
    out.push(...aboutSection());
    out.push(el("h3", { class: "set-h3" }, "Dépannage"));
    out.push(setRow("Modules du MasterHUB", "Voir ce qui est détecté et reconnecter les modules.", el("button", { class: "btn", onclick: showDiagnostic }, "Diagnostic")));
    out.push(setRow("Dossier de configuration", S.configPath || "", el("button", { class: "btn", onclick: () => invoke("open_config_folder") }, "Ouvrir")));
  }
  return out;
}

/** Ligne de réglage : titre et explication à gauche, contrôle à droite. */
function setRow(label, desc, control) {
  return el("div", { class: "set-row" },
    el("div", { class: "set-row-text" }, el("span", { class: "set-row-title" }, label), desc ? el("span", { class: "set-row-desc" }, desc) : null),
    el("div", { class: "set-row-ctl" }, control));
}
/** Bloc de réglage : titre, puis contrôle en pleine largeur. */
function setBlock(label, desc, ...content) {
  return el("div", { class: "set-block" }, el("span", { class: "set-row-title" }, label), desc ? el("span", { class: "set-row-desc" }, desc) : null,
    el("div", { class: "set-block-ctl" }, ...content));
}
function setCard(icon, label, desc, action) {
  return el("div", { class: "set-card" }, el("span", { class: "set-card-ic", html: svgIcon(icon) }),
    el("div", { class: "set-row-text" }, el("span", { class: "set-row-title" }, label), el("span", { class: "set-row-desc" }, desc)), action);
}
function toggleCtl(value, onChange) {
  const input = el("input", { type: "checkbox", checked: !!value });
  input.onchange = () => onChange(input.checked);
  return el("label", { class: "switch bare" }, input, el("span", { class: "knob" }));
}
function rangeCtl(value, min, max, unit, onChange) {
  const val = el("span", { class: "val" }, value + unit);
  const r = el("input", { type: "range", min, max, value });
  r.oninput = () => { val.textContent = r.value + unit; onChange(Number(r.value)); };
  return el("div", { class: "range-row", style: "width:260px" }, r, val);
}

// --- Langue ---
function langSwitch() {
  const b = (lang, label) => el("button", { class: "seg-btn" + (S.lang === lang ? " on" : ""), translate: "no", onclick: () => setLang(lang) }, label);
  return el("div", { class: "seg", translate: "no" }, b("fr", "Français"), b("en", "English"));
}
async function setLang(lang) {
  if (lang === S.lang) return;
  try { await invoke("set_lang", { lang }); } catch (e) { toast(String(e), true); return; }
  // L'interface se recharge dans la nouvelle langue et rouvre le même écran.
  try { sessionStorage.setItem("hf-reopen", JSON.stringify({ sel: S.sel, setSec: S.setSec, wizard: !!document.querySelector("#wizard:not(.hidden)") })); } catch (_) {}
  location.reload();
}

// --- Mode simple / avancé ---
function modeSwitch() {
  const b = (mode, label) => el("button", { class: "seg-btn" + (S.uiMode === mode ? " on" : ""), onclick: () => setUiMode(mode) }, label);
  return el("div", { class: "seg" }, b("simple", "Simple"), b("advanced", "Avancé"));
}
async function setUiMode(mode, quiet) {
  S.uiMode = mode;
  try { await invoke("set_ui_mode", { mode }); } catch (e) { toast(String(e), true); }
  buildLibrary();
  renderInspector();
  if (!quiet) toast(mode === "simple" ? "Mode simple : l'essentiel seulement." : "Mode avancé : toutes les fonctions sont visibles.");
}

// --- Assistant de premier lancement ---
const STARTERS = [
  { id: "bureau", name: "Bureau", desc: "Copier, coller, captures, fenêtres, horloge et processeur.", icon: "monitor",
    keys: ["Copier", "Coller", "Couper", "Annuler", "Capture d'écran",
      "Explorateur", "Calculatrice", "Afficher le bureau", "Fenêtre à gauche", "Fenêtre à droite",
      "Horloge", "Processeur (CPU)", "Mémoire (RAM)", "Lecture / Pause", "Verrouiller le PC"],
    faders: ["master", "mic", "none", "none", "none"] },
  { id: "stream", name: "Streaming et Discord", desc: "Micro, son, musique, chronomètre, compteur et correction par IA.", icon: "mic",
    keys: ["Micro ON / OFF", "Son ON / OFF", "Changer de sortie audio", "Musique en cours (pochette)", "Piste suivante",
      "Chronomètre (appui long = 0)", "Compteur (+1, appui long = 0)", "IA : corriger la sélection", "Capture d'écran", "Horloge",
      "Volume général", "État du micro", "Lecture / Pause", "Piste précédente", "Arrêter tous les sons"],
    faders: ["master", "mic", "app:discord.exe", "app:spotify.exe", "none"] },
  { id: "jeu", name: "Jeu", desc: "Températures, carte graphique, Game Bar, micro et musique.", icon: "gamepad-2",
    keys: ["Horloge", "Processeur (CPU)", "Température du GPU", "Carte graphique (GPU)", "Mémoire (RAM)",
      "Micro ON / OFF", "Son ON / OFF", "Changer de sortie audio", "Capture d'écran",
      { label: "Game Bar", icon: "gamepad-2", color: "#107C10", action: { type: "hotkey", keys: "win+g" } },
      "Piste précédente", "Lecture / Pause", "Piste suivante", "Réseau (débits)",
      { label: "Clip 30 s", icon: "circle-dot", color: "#C62828", action: { type: "hotkey", keys: "win+alt+g" } }],
    faders: ["master", "mic", "app:discord.exe", "none", "none"] },
  { id: "vide", name: "Partir de zéro", desc: "Une page vide : tu glisses toi-même les actions sur les touches.", icon: "square", keys: [], faders: ["master", "none", "none", "none", "none"] },
  { id: "keep", name: "Garder mon profil actuel", desc: "Ne rien créer ; seul le style choisi est appliqué.", icon: "check", keys: null },
];

function starterPages(st) {
  const keys = [];
  st.keys.forEach((k, i) => {
    const item = typeof k === "string" ? libItemByName(k) : Object.assign({ name: k.label }, k);
    if (item) keys.push(clean(defFromItem(item, i + 1)));
  });
  return [{ name: tr("Accueil"), keys }];
}

function openWizard() {
  const W = { step: 0, mode: S.uiMode || "simple", theme: (S.cfg.general && S.cfg.general.theme) || "graphite",
    starter: S.cfg.pages.some((p) => p.keys.length) ? "keep" : "bureau" };
  let node = $("#wizard");
  if (!node) { node = el("div", { class: "wizard", id: "wizard" }); document.body.append(node); }
  node.classList.remove("hidden");
  const close = async () => { node.classList.add("hidden"); try { await invoke("finish_wizard"); } catch (_) {} };
  const steps = ["Bienvenue", "Affichage", "Style", "Profil", "C'est prêt"];

  const preview = async (box) => {
    const sample = ["Copier", "Micro ON / OFF", "Horloge", "Processeur (CPU)", "Lecture / Pause"].map((n, i) => {
      const d = clean(defFromItem(libItemByName(n), i + 1)); d.theme = W.theme; return d;
    });
    try {
      const res = await invoke("render_keys", { keys: sample });
      box.innerHTML = "";
      res.forEach((r) => box.append(el("div", { class: "wz-key" }, r ? el("img", { src: r.src }) : null)));
    } catch (_) { /* aperçu facultatif */ }
  };

  const draw = () => {
    node.innerHTML = "";
    const body = el("div", { class: "wz-body" });
    const s = W.step;
    if (s === 0) {
      const mods = (S.status && S.status.modules) || [];
      const names = { keys15: "module 15 touches", faders5: "module 5 faders", rollers2: "module 2 molettes" };
      const found = mods.map((m) => names[m.kind]).filter(Boolean);
      body.append(el("h2", {}, "Bienvenue dans HubForge"),
        el("p", {}, "HubForge pilote ton Cooler Master MasterHUB : des touches qui lancent des actions, affichent l'heure, la musique ou la température, des faders pour le volume de chaque application, et bien plus."),
        el("div", { class: "wz-detect" + (found.length ? " ok" : "") }, found.length
          ? `Branché : base${found.length ? ", " + found.join(", ") : ""}.`
          : "Aucun module détecté pour l'instant : tu peux quand même tout préparer, ils seront reconnus dès qu'ils seront branchés."),
        el("p", { class: "hint" }, "Quatre petites questions, et c'est parti. Tout se change ensuite dans les réglages."),
        el("div", { class: "row", style: "gap:10px;align-items:center;margin-top:14px" }, el("span", { class: "hint", style: "margin:0" }, "Langue / Language"), langSwitch()));
    } else if (s === 1) {
      const card = (mode, t, d) => el("div", { class: "wz-card" + (W.mode === mode ? " on" : ""), onclick: () => { W.mode = mode; draw(); } }, el("b", {}, t), el("span", {}, d));
      body.append(el("h2", {}, "Comment veux-tu voir HubForge ?"),
        el("div", { class: "wz-cards two" },
          card("simple", "Simple", "L'essentiel : raccourcis, applis, son, musique, touches vivantes, IA. Idéal pour commencer."),
          card("advanced", "Avancé", "Tout : macros, variables, conditions, requêtes web, appui long et double appui.")));
    } else if (s === 2) {
      const pv = el("div", { class: "wz-preview" });
      body.append(el("h2", {}, "Choisis le style des touches"),
        themeChips(W.theme, null, (v) => { W.theme = v; draw(); }), pv,
        el("p", { class: "hint" }, "Chaque page et chaque touche pourra aussi avoir son propre style. Tes images et GIF restent tels quels."));
      preview(pv);
    } else if (s === 3) {
      body.append(el("h2", {}, "Par quoi commencer ?"),
        el("div", { class: "wz-cards" }, ...STARTERS.map((st) => el("div", { class: "wz-card row-card" + (W.starter === st.id ? " on" : ""), onclick: () => { W.starter = st.id; draw(); } },
          el("span", { class: "wz-ic", html: svgIcon(st.icon) }), el("div", {}, el("b", {}, st.name), el("span", {}, st.desc))))),
        el("p", { class: "hint" }, W.starter === "keep" ? "Ton profil reste tel quel ; il prend seulement le style choisi."
          : W.starter === "vide" ? "Un nouveau profil vide est créé : ton profil actuel n'est pas modifié."
          : "Un nouveau profil est créé : ton profil actuel n'est pas modifié. Les faders contrôlent le volume général, le micro et des applications courantes ; tout se change ensuite."));
    } else {
      body.append(el("h2", {}, "C'est prêt !"),
        el("ul", { class: "wz-tips" },
          el("li", {}, "Glisse une action de la liste de gauche sur une touche pour la changer."),
          el("li", {}, "Clique sur une touche pour régler son texte, son icône et son action."),
          el("li", {}, "Clique sur un fader ou une molette pour choisir ce qu'il contrôle."),
          el("li", {}, "Les réglages (roue en haut à droite) permettent de passer en mode avancé ou de relancer cet assistant.")));
    }
    const back = s > 0 ? el("button", { class: "btn ghost", onclick: () => { W.step--; draw(); } }, "Retour") : el("button", { class: "btn ghost", onclick: close }, "Passer");
    const next = el("button", { class: "btn primary", onclick: async () => {
      if (s < 3) { W.step++; draw(); return; }
      if (s === 3) {
        next.disabled = true;
        try {
          if (W.mode !== S.uiMode) await setUiMode(W.mode, true);
          const st = STARTERS.find((x) => x.id === W.starter);
          if (st.keys) {
            const name = await invoke("create_starter_profile", { name: tr(st.name), pages: starterPages(st), theme: W.theme, faders: st.faders });
            toast(`Profil « ${name} » créé.`);
          } else if ((S.cfg.general.theme || "graphite") !== W.theme) {
            S.cfg.general.theme = W.theme; rethemed();
          }
        } catch (e) { toast(String(e), true); next.disabled = false; return; }
        W.step++; draw(); return;
      }
      close();
    } }, s === 0 ? "Commencer" : s === 3 ? "Créer" : s === 4 ? "C'est parti" : "Suivant");
    node.append(el("div", { class: "wz-card-main" },
      el("div", { class: "wz-steps" }, ...steps.map((t, i) => el("span", { class: i === s ? "on" : i < s ? "done" : "" }, t))),
      body,
      el("div", { class: "wz-actions" }, back, el("span", { class: "spacer" }), next)));
  };
  draw();
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
  const out = [];
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
    box,
    el("div", { class: "links" },
      link("Site du projet", repo),
      link("Nouveautés", repo + "/blob/main/CHANGELOG.md"),
      link("Guide des addons", repo + "/blob/main/docs/ADDON_GUIDE.md"),
      link("Signaler un problème", repo + "/issues")),
    el("p", { class: "hint" }, "HubForge n'est pas affilié à Cooler Master. MasterHUB est une marque de Cooler Master. Aucun firmware n'est modifié."),
  ];
}

function switchRow(label, value, onChange, hint) {
  const input = el("input", { type: "checkbox", checked: !!value });
  input.onchange = () => onChange(input.checked);
  return el("label", { class: "switch" }, el("span", { class: "switch-text" }, el("span", {}, label), hint ? el("span", { class: "switch-hint" }, hint) : null),
    input, el("span", { class: "knob" }));
}

function rangeField(label, value, min, max, unit, onChange, hint) {
  const val = el("span", { class: "val" }, value + unit);
  const r = el("input", { type: "range", min, max, value });
  r.oninput = () => { val.textContent = r.value + unit; onChange(Number(r.value)); };
  return field(label, el("div", { class: "range-row" }, r, val), hint ? el("p", { class: "hint" }, hint) : null);
}
