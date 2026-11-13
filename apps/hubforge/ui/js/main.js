"use strict";
/* HubForge — État en direct, raccourcis clavier, fenêtres modales, notifications, lancement. */

// ---------------------------------------------------------------------------
// État en direct (envoyé par le moteur)
// ---------------------------------------------------------------------------
function onStatus(st) {
  const prev = S.status;
  if (st.profile && S.profile && st.profile !== S.profile) {
    S.status = st;
    reloadProfile();
    return;
  }
  if (st.game !== prev.game) {
    const stage = $(".stage");
    const b = stage.querySelector(".game-banner");
    if (st.game && !b) stage.append(el("div", { class: "game-banner" }, "🎮 Partie de tape-taupe en cours sur le MasterHUB…"));
    if (!st.game && b) { b.remove(); invoke("get_initial").then((ini) => { if (ini.game_best > (S.gameBest || 0)) toast(`🏆 Nouveau record : ${ini.game_best} points !`); S.gameBest = ini.game_best; buildLibrary(); }); }
  }
  const sig = (s) => JSON.stringify((s.modules || []).map((m) => [m.kind, m.port]));
  const layoutChanged = sig(prev) !== sig(st);
  const pageChanged = st.page !== S.devicePage;
  const errChanged = st.config_error !== prev.config_error;
  S.status = st;
  if (layoutChanged) { renderModulesChips(); renderDevice(); fitDevice(); }
  if (errChanged) renderBanner();
  updateFaderLive();
  updateLedLive();
  updateDoomView(prev, st);
  if (S.layout && st.base_rot !== undefined && st.base_rot !== S.layout.base_rot) { S.layout.base_rot = st.base_rot; renderDevice(); fitDevice(); }
  if (pageChanged) {
    S.devicePage = st.page;
    // La page a changé depuis le MasterHUB : l'interface suit.
    if (S.page !== st.page && st.page < S.cfg.pages.length) { S.page = st.page; renderKeys(); refreshPreviews(S.page); if (S.sel.type === "key") renderInspector(); }
    renderPages();
  }
  if (S.devicePage === S.page && !layoutChanged) {
    for (let i = 0; i < 15; i++) {
      const wasT = !!(prev.toggled && prev.toggled[i]), isT = !!(st.toggled && st.toggled[i]);
      if (wasT !== isT) { updateKeyNode(i + 1); continue; }
      if (!!(prev.pressed && prev.pressed[i]) !== !!(st.pressed && st.pressed[i])) {
        const n = document.querySelector(`#keys .key[data-key="${i + 1}"]`);
        if (n) n.classList.toggle("pressed", !!st.pressed[i]);
      }
    }
  }
}

function onGlobalKey(e) {
  if (S.capturing) return;
  const tag = (e.target.tagName || "").toLowerCase();
  if (tag === "input" || tag === "textarea" || tag === "select") return;
  const ctrl = e.ctrlKey || e.metaKey;
  const k = e.key.toLowerCase();
  if (ctrl && k === "z" && !e.shiftKey) { e.preventDefault(); undo(); return; }
  if (ctrl && (k === "y" || (k === "z" && e.shiftKey))) { e.preventDefault(); redo(); return; }
  if (ctrl && k === "c" && S.sel.type === "key") { e.preventDefault(); copyKey(S.sel.key); return; }
  if (ctrl && k === "v" && S.sel.type === "key") { e.preventDefault(); pasteKey(S.sel.key); return; }
  if (e.key === "Escape") deselect();
  if ((e.key === "Delete" || e.key === "Backspace") && S.sel.type === "key" && keyDef(S.sel.key)) {
    const k = S.sel.key;
    confirm_(`Vider la touche ${k} ?`).then((ok) => { if (!ok) return; setKeyDef(k, null); updateKeyNode(k); changed(); renderInspector(); });
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
function toast(msg, isError, action) {
  const t = $("#toast");
  t.innerHTML = "";
  t.append(document.createTextNode(msg));
  if (action) t.append(el("span", { class: "toast-action", onclick: () => { t.classList.add("hidden"); action.fn(); } }, action.label));
  t.className = "toast" + (isError ? " error" : "");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.add("hidden"), isError || action ? 6000 : 2500);
}

init().catch((e) => { document.body.innerHTML = `<pre style="padding:20px;color:#ffb3b5">Erreur au démarrage de l'interface :\n${e}\n${e.stack || ""}</pre>`; });
