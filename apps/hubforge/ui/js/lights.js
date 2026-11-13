"use strict";
/* HubForge — LED de la base, orientation, positions, animations. */

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
