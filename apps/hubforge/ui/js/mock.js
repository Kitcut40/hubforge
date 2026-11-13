"use strict";
/* HubForge — Simulation : ouverture de l'interface dans un navigateur, pour le développement. */

// ---------------------------------------------------------------------------
// Simulation (ouverture dans un navigateur, pour le développement)
// ---------------------------------------------------------------------------
window.__mockUpdate = () => ({ current: "0.8.0", checking: false, auto: true, last_check: Date.now() / 1000 - 600, error: null,
  available: location.search.includes("update") ? { version: "0.8.1", current: "0.8.0", name: "HubForge 0.8.1", published: "2026-11-08T10:00:00Z",
    page_url: "https://github.com/Kitcut40/hubforge/releases/tag/v0.8.1", asset_name: "HubForge-Setup-0.8.1.exe", asset_size: 6340000,
    notes: "### Nouveautés\n- **Appui long** sur les touches\n- Intégration `OBS` avec retour d'état\n\n### Corrections\n- Le module 15 touches revient seul après un rebranchement" } : null });

async function mockInvoke(cmd, args) {
  if (!window.__mock) {
    const cfg = await fetch("mock-config.json").then((r) => r.json()).catch(() => ({ pages: [{ name: "Accueil", keys: [] }] }));
    const icons = await fetch("mock-icons.json").then((r) => r.json()).catch(() => []);
    window.__mock = { cfg, icons };
  }
  const rotated = location.search.includes("layout=2");
  const mods = rotated
    ? [{ kind: "keys15", port: "134", placement: { col: 1, row: 0, rot: 270 } }, { kind: "faders5", port: "122", placement: { col: 0, row: 4, rot: 0 } },
       { kind: "rollers2", port: "141", placement: { col: 0, row: 0, rot: 270 } }, { kind: "base" }]
    : [{ kind: "keys15", port: "24", placement: { col: 0, row: 0, rot: 0 } }, { kind: "faders5", port: "41", placement: { col: 0, row: 3, rot: 0 } },
       { kind: "rollers2", port: "121", placement: { col: 0, row: 5, rot: 0 } }, { kind: "base" }];
  switch (cmd) {
    case "get_layout": return window.__mockLayout || (window.__mockLayout = { base_rot: location.search.includes("rot=90") ? 90 : 0, placements: {} });
    case "set_base_rot": window.__mockLayout.base_rot = args.rot; return null;
    case "set_placement": return null;
    case "preview_animation": return null;
    case "list_sound_devices": return ["Haut-parleurs (Realtek(R) Audio)", "CABLE Input (VB-Audio Virtual Cable)"];
    case "import_sound": case "import_sound_data": return "sounds/" + String(args.name || args.path || "son.mp3").split(/[\\/]/).pop();
    case "test_sound": return null;
    case "list_audio_outputs": return ["Haut-parleurs (Realtek(R) Audio)", "Casque (Arctis 7 Game)", "CABLE Input (VB-Audio Virtual Cable)"];
    case "open_vars_folder": return "C:/Users/…/HubForge/variables";
    case "clear_vars": onStatus(Object.assign({}, S.status, { vars: {} })); return null;
    case "start_doom": if (location.search.includes("doom")) { setTimeout(() => onStatus(Object.assign({}, S.status, { doom: true })), 300); return "freedoom1.wad"; } throw "no_wad";
    case "download_freedoom": return null;
    case "stop_doom": onStatus(Object.assign({}, S.status, { doom: false })); return null;
    case "list_profiles": return ["Benjamin", "Laura", "Streaming"];
    case "list_backups": return [0, 1, 2].map((i) => ({ file: `${1794000000 - i * 3600}.toml`, time: Date.now() / 1000 - i * 5400 - 300, size: 4000, pages: 3, keys: 30 - i * 2 }));
    case "get_diagnostic": return { version: "0.8.1", os: "windows x86_64", profile: "Benjamin", config_dir: "C:\\Users\\benja\\AppData\\Roaming\\HubForge",
      status: { modules: [{ kind: "keys15", serial: "MHKD15AA431251602923", port: "24", placement: { col: 0, row: 0, rot: 0 } }, { kind: "faders5", serial: "MHFD05AA421251700387", port: "41", placement: { col: 0, row: 3, rot: 0 } }] },
      addons: ["fusion360 1.1.0", "discord 1.0.0"], log: "[20:12:01] HubForge v0.8.1 démarre\n[20:12:02] ✓ 15 touches connecté\n[20:12:02] ✓ 5 faders connecté" };
    case "check_updates_now": case "get_update_state": return window.__mockUpdate();
    case "set_update_auto": return Object.assign(window.__mockUpdate(), { auto: args.on });
    case "skip_update": return Object.assign(window.__mockUpdate(), { available: null });
    case "list_addons": return await fetch("mock-addons.json").then((r) => r.json()).catch(() => ({ addons: [], signature: "", folder: "" }));
    case "addons_signature": return "x";
    case "addon_pages": return [{ name: "Fusion 360", apps: ["Fusion360.exe"], keys: [{ key: 1, label: "Esquisse", color: "#1565C0", action: { type: "hotkey", keys: "s" } }] }];
    case "get_initial":
      return { update: window.__mockUpdate(), whats_new: location.search.includes("new") ? "### Nouveautés\n- **Installateur Windows** : menu Démarrer, désinstallation propre.\n- **Mises à jour depuis GitHub** en un clic.\n- Double-clic sur un fichier `.forge`." : null, profile: "Benjamin", profiles: ["Benjamin", "Laura", "Streaming"], game_best: 0, config: window.__mock.cfg, config_path: "C:\\Users\\benja\\AppData\\Roaming\\HubForge\\config.toml", autostart: false, version: "0.8.1", ui_mode: location.search.includes("simple") || location.search.includes("wizard") ? "simple" : "advanced", show_wizard: location.search.includes("wizard"), lang: location.search.includes("en") ? "en" : "fr",
        status: { modules: mods, faders: [72, 35, 60, 18, 90], page: 0, pressed: Array(15).fill(false), toggled: Array(15).fill(false),
          leds: location.search.includes("leds") ? { base: "#FFC800", faders: ["#00E676", "#00E676", "#FFC800", "#FFC800", "#000000"] } : { base: null, faders: [] },
          sim: location.search.includes("leds") ? { name: "Assetto Corsa", live: true, rpm: 7120, max_rpm: 8000, flag: 2 } : null,
          base_rot: location.search.includes("rot=90") ? 90 : 0, vars: { compteur: "7", interrupteur: "1" }, pickup: [false, true, false, false, false] } };
    case "render_keys": return args.keys.map((d) => d && { src: mockPreview(d), animated: !!(d.icon && /\.gif$/i.test(d.icon)) });
    case "list_icons": return window.__mock.icons;
    case "list_icon_packs": return window.__mockPacks || [];
    case "create_starter_profile": window.__starter = args; return args.name;
    case "icon_pack_thumbs": return (window.__mockPacks || []).find((p) => p.folder === args.folder).icons.map((i, n) => [i.path, i.name, mockPreview({ label: i.name, color: ["#E5484D", "#3E63DD", "#30A46C", "#F76B15"][n % 4] })]);
    case "list_audio_apps": return ["chrome.exe", "discord.exe", "spotify.exe"];
    case "list_running_apps": return ["chrome.exe", "explorer.exe", "fusion360.exe", "obs64.exe", "prusa-slicer.exe"];
    case "app_info": return { icon: null, name: (args.target.split(/[\\/]/).pop() || "").replace(/\.exe$/i, "") };
    default: return null;
  }
}

function mockPreview(d) {
  const bg = d.color || "#000";
  const demo = { clock: "15:49", date: "Samedi 7", cpu: "CPU 23 %", ram: "RAM 41 %", volume: "45 %", mic: "Micro" };
  const label = (d.widget ? demo[d.widget] : d.label || "").replace(/&/g, "&amp;").replace(/</g, "&lt;");
  let icon = "";
  if (d.icon && d.icon.startsWith("builtin:")) {
    const svg = svgIcon(d.icon.slice(8)).replace(/currentColor/g, d.text_color || "#fff").replace(/width="24"/, 'width="34"').replace(/height="24"/, 'height="34"');
    icon = `<g transform="translate(19,${label ? 6 : 19})">${svg.replace(/<\?xml[^>]*>/, "")}</g>`;
  }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="72" height="72"><rect width="72" height="72" fill="${bg}"/>${icon}` +
    `<foreignObject x="3" y="${icon ? 44 : 3}" width="66" height="${icon ? 26 : 66}"><div xmlns="http://www.w3.org/1999/xhtml" style="height:100%;display:flex;align-items:center;justify-content:center;text-align:center;font:600 ${icon ? 10 : 13}px Inter,sans-serif;color:#fff">${label}</div></foreignObject></svg>`;
  return "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
}
