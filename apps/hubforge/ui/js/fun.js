"use strict";
/* HubForge — Mini-jeu et Doom. */

// --- 🎮 ---
// --- Easter egg : Doom sur le MasterHUB (taper IDDQD dans la fenêtre) ---
const DOOM_KEYS = {
  ArrowUp: 0xad, z: 0xad, w: 0xad, ArrowDown: 0xaf, s: 0xaf, ArrowLeft: 0xac, ArrowRight: 0xae,
  q: 0xa0, a: 0xa0, d: 0xa1, " ": 0xa2, Control: 0xa3, f: 0xa3, Shift: 0x80 + 0x36, Escape: 27, Enter: 13, Tab: 9,
  e: 0x5d, r: 0x5b, y: 0x79, n: 0x6e, 1: 0x31, 2: 0x32, 3: 0x33, 4: 0x34, 5: 0x35, 6: 0x36, 7: 0x37,
};

function setupDoom() {
  const code = "iddqd";
  let pos = 0;
  document.addEventListener("keydown", (e) => {
    if (S.status.doom) return;
    const tag = (e.target.tagName || "").toLowerCase();
    if (tag === "input" || tag === "textarea" || S.capturing) return;
    const k = e.key.toLowerCase();
    pos = k === code[pos] ? pos + 1 : k === code[0] ? 1 : 0;
    if (pos === code.length) { pos = 0; launchDoom(); }
  });
  const send = (e, pressed) => {
    if (!S.status.doom || e.repeat) { if (S.status.doom) e.preventDefault(); return; }
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    const c = DOOM_KEYS[k];
    if (c === undefined) return;
    e.preventDefault();
    e.stopPropagation();
    invoke("doom_key", { pressed, code: c });
  };
  window.addEventListener("keydown", (e) => send(e, true), true);
  window.addEventListener("keyup", (e) => send(e, false), true);
  $("#doom-quit").onclick = () => invoke("stop_doom");
}

async function launchDoom(wad) {
  if (!S.status.modules.some((m) => m.kind === "keys15")) { toast("👹 Branche le module 15 touches pour lancer Doom…", true); return; }
  try {
    const name = await invoke("start_doom", { wad: wad || null });
    toast(`👹 Chargement de Doom (${name})…`);
  } catch (e) {
    if (String(e) !== "no_wad") { toast(String(e), true); return; }
    openDialog({
      title: "Doom a besoin d'un fichier de jeu",
      sub: "Le moteur est intégré à HubForge, mais pas les niveaux",
      html: `<p><b>Freedoom</b> est un jeu libre et gratuit compatible avec Doom (licence BSD) : HubForge peut le télécharger pour toi depuis GitHub (24 Mo, une seule fois).</p>
        <p>Tu peux aussi utiliser ton propre <b>DOOM.WAD</b> ou <b>DOOM2.WAD</b> (version Steam ou GOG de Doom / Doom II).</p>`,
      icon: `<div class="dialog-glyph doom-glyph">👹</div>`,
      actions: [
        el("button", { class: "btn ghost small", onclick: async () => { const p = await invoke("pick_file", { kind: "wad" }); if (p) { closeDialog(); launchDoom(p); } } }, "Choisir un fichier WAD…"),
        el("span", { class: "spacer" }),
        el("button", { class: "btn", onclick: closeDialog }, "Annuler"),
        el("button", { class: "btn primary", onclick: downloadFreedoom }, "Télécharger Freedoom"),
      ],
    });
  }
}

async function downloadFreedoom() {
  $("#dialog-progress").classList.remove("hidden");
  $("#dialog-bar").style.width = "0%";
  $("#dialog-progress-text").textContent = "Connexion à GitHub…";
  $("#dialog-actions").querySelectorAll("button").forEach((b) => (b.disabled = true));
  try {
    await invoke("download_freedoom");
    closeDialog();
    toast("👹 Freedoom téléchargé. Chargement…");
  } catch (e) {
    $("#dialog-progress").classList.add("hidden");
    $("#dialog-actions").querySelectorAll("button").forEach((b) => (b.disabled = false));
    toast(String(e), true);
  }
}

/** Affiche / masque la fenêtre de Doom selon l'état du moteur. */
function updateDoomView(prev, st) {
  if (st.doom_error && st.doom_error !== prev.doom_error) toast("Doom : " + st.doom_error, true);
  // Le jeu s'affiche sur les touches ; la fenêtre ne montre que les commandes.
  $("#doom").classList.toggle("hidden", !st.doom);
}

function setupEasterEgg() {
  const code = ["ArrowUp", "ArrowUp", "ArrowDown", "ArrowDown", "ArrowLeft", "ArrowRight", "ArrowLeft", "ArrowRight", "b", "a"];
  let pos = 0;
  document.addEventListener("keydown", (e) => {
    const tag = (e.target.tagName || "").toLowerCase();
    if (tag === "input" || tag === "textarea" || S.capturing) return;
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    pos = k === code[pos] ? pos + 1 : k === code[0] ? 1 : 0;
    if (pos === code.length) { pos = 0; launchGame(); }
  });
  let clicks = 0, timer = null;
  $(".brand-logo").addEventListener("click", () => {
    clicks++;
    clearTimeout(timer);
    timer = setTimeout(() => { clicks = 0; }, 1500);
    if (clicks >= 7) { clicks = 0; launchGame(); }
  });
}

function launchGame() {
  if (!S.status.modules.some((m) => m.kind === "keys15")) { toast("🎮 Branche le module 15 touches pour jouer…", true); return; }
  invoke("start_game");
  toast("🎮 Tape-taupe ! Tape les taupes sur le MasterHUB (dorée = +3, bombe = −3).");
}
