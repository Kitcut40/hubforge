"use strict";
/* HubForge — Panneaux des faders, des molettes et des pages. */

// --- Fader ---
function faderInspector(i) {
  const page = currentPage();
  const own = pageFader(i);
  const slot = own || S.cfg.faders[i];
  const pageName = page.name || `Page ${S.page + 1}`;
  const out = [title(`Fader ${i + 1}`, ["le plus à gauche", "", "au milieu", "", "le plus à droite"][i] || "")];
  const live = S.status.faders ? S.status.faders[i] + " %" : "non connecté";
  out.push(el("p", { class: "hint", style: "margin:-6px 0 10px" }, "Position actuelle : ", el("b", { id: "fader-live" }, live)));
  if (S.status.pickup && S.status.pickup[i]) out.push(el("p", { class: "hint warn-text", style: "margin:-4px 0 10px" },
    "En attente de reprise : passe le fader par le volume actuel pour qu'il reprenne la main (sa LED pulse en attendant)."));

  // Réglage commun ou propre à la page
  out.push(el("div", { class: "tabs scope-tabs" },
    el("button", { class: own ? "" : "on", onclick: () => { if (!own) return; page.faders = (page.faders || []).filter((f) => f.index !== i + 1); if (!page.faders.length) delete page.faders; changed(); renderDevice(); } }, "Toutes les pages"),
    el("button", { class: own ? "on" : "", onclick: () => { if (own) return; page.faders = page.faders || []; page.faders.push(Object.assign(clone(S.cfg.faders[i]), { index: i + 1 })); changed(); renderDevice(); } }, `Page « ${pageName} »`)));
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
/** Tous les jeux d'actions de molettes (réglage commun et pages). */
function wheelActionSets() {
  const r = S.cfg.rollers, out = [];
  for (const side of ["left", "right"]) if (r[side + "_actions"]) out.push(r[side + "_actions"]);
  for (const p of S.cfg.pages) for (const side of ["left", "right"]) if (p[`roller_${side}_actions`]) out.push(p[`roller_${side}_actions`]);
  return out;
}

function rollersInspector() {
  const r = S.cfg.rollers;
  const page = currentPage();
  const pageName = page.name || `Page ${S.page + 1}`;
  const out = [title("Molettes", pageName)];
  for (const [side, label] of [["left", "Molette de gauche"], ["right", "Molette de droite"]]) {
    const key = "roller_" + side, akey = `roller_${side}_actions`;
    const own = page[key] != null;
    const mode = own ? page[key] : r[side];
    const setMode = (v) => { if (own) page[key] = v; else r[side] = v; changed(); renderDevice(); };
    out.push(el("h2", {}, label));
    out.push(el("div", { class: "tabs scope-tabs" },
      el("button", { class: own ? "" : "on", onclick: () => { delete page[key]; delete page[akey]; changed(); renderDevice(); } }, "Commune"),
      el("button", { class: own ? "on" : "", onclick: () => {
        if (own) return;
        page[key] = r[side];
        if (r[side + "_actions"]) page[akey] = clone(r[side + "_actions"]);
        changed(); renderDevice();
      } }, `Page « ${pageName} »`)));
    const kind = rollerKind(mode);
    out.push(field("Action", el("select", { class: "input", onchange: (e) => { setMode(e.target.value === "app" ? "app:" : e.target.value); renderInspector(); } },
      ...ROLLER_MODES.map(([v, l]) => el("option", { value: v, selected: v === kind }, l)))));
    if (kind === "app") {
      out.push(field("Applications", el("input", { class: "input", value: mode.slice(4), spellcheck: "false", placeholder: "ex. discord.exe, spotify.exe",
        onchange: (e) => {
          const list = e.target.value.split(",").map((x) => x.trim().toLowerCase()).filter(Boolean).map((x) => /\.exe$/.test(x) ? x : x + ".exe");
          setMode("app:" + list.join(","));
        } }), el("p", { class: "hint" }, "Plusieurs applications séparées par des virgules.")));
    } else if (kind === "actions") {
      const owner = own ? page : r;
      const prop = own ? akey : side + "_actions";
      if (!owner[prop]) owner[prop] = { up: { type: "none" }, down: { type: "none" } };
      const w = owner[prop];
      for (const [dir, l] of [["up", "Un cran vers le haut"], ["down", "Un cran vers le bas"]]) {
        const holder = { get action() { return w[dir]; }, set action(v) { w[dir] = v; } };
        out.push(el("div", { class: "step wheel-action" }, el("div", { class: "step-body" }, ...actionEditor(holder, -1, () => changed(), { extra: l }))));
      }
      out.push(el("p", { class: "hint" }, "Chaque cran lance l'action une fois : parfait pour faire défiler une timeline, changer d'outil, régler un paramètre ou un compteur. « Maintenir » devient un simple appui."));
    }
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
  out.push(field("Style des touches", themeChips(p.theme, `Comme le profil (${themeName(S.cfg.general.theme || "graphite")})`, (v) => {
    if (v) p.theme = v; else delete p.theme;
    rethemed(); renderInspector();
  }), p.keys.some((d) => d.theme) ? el("button", { class: "btn small", style: "margin-top:8px", onclick: () => {
    for (const d of p.keys) delete d.theme;
    rethemed(); renderInspector(); toast("Toutes les touches de la page suivent maintenant son style");
  } }, "Appliquer à toutes les touches de la page") : null));

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
