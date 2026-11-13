import json, os, sys
# Génère apps/hubforge/ui/js/i18n.js à partir des dictionnaires en1/en2/en3.
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from en1 import EN
from en2 import EN2
from en3 import EN3
D = {**EN, **EN2, **EN3}
D.update({"IA": "AI", "Benjamin": "Benjamin"})
D = {k.strip(): v for k, v in D.items() if k.strip()}
js = r'''"use strict";
/* HubForge — Langue de l'interface (français / anglais).
   Le code de l'interface est écrit en français. En anglais, ce fichier
   traduit à la volée les textes affichés (nœuds de texte, info-bulles,
   textes d'exemple, boîtes de dialogue) avec le dictionnaire ci-dessous :
   phrase exacte, puis modèles avec des parties variables, puis morceaux de
   phrases. Les éléments marqués translate="no" (noms de profils, de pages,
   textes saisis…) ne sont jamais traduits. */

let LANG = "fr";

const EN_DICT = ''' + json.dumps(D, ensure_ascii=False, indent=0) + r''';

/** Modèles : [expression, remplacement] ; « $T1 » = groupe 1 traduit lui aussi. */
const EN_PATTERNS = [
  [/^(.+) — (.+)$/, "$T1 — $T2"],
  [/^Modèle : (.+)$/, "Template: $T1"],
  [/^(Gauche|Droite) · (.+)$/, "$T1 · $T2"],
  [/^Étapes \((\d+)\)$/, "Steps ($1)"],
  [/^Modules détectés \((\d+)\)$/, "Detected modules ($1)"],
  [/^Touche (\d+) · (.+)$/, "Key $1 · $2"],
  [/^Touche (\d+)$/, "Key $1"],
  [/^Page (\d+)$/, "Page $1"],
  [/^Page (\d+) — (.+)$/, "Page $1 — $2"],
  [/^Page « (.+) »$/, "Page “$1”"],
  [/^Profil « (.+) »$/, "Profile “$1”"],
  [/^Profil « (.+) » créé\.$/, "Profile “$1” created."],
  [/^⇪ Exporter « (.+) » dans un fichier…$/, "⇪ Export “$1” to a file…"],
  [/^⧉ Dupliquer « (.+) »…$/, "⧉ Duplicate “$1”…"],
  [/^✎ Renommer « (.+) »…$/, "✎ Rename “$1”…"],
  [/^⚡ Applications de « (.+) » \(mode dynamique\)…$/, "⚡ “$1” applications (dynamic mode)…"],
  [/^⟲ Sauvegardes de « (.+) »…$/, "⟲ “$1” backups…"],
  [/^Sauvegardes de « (.+) »$/, "“$1” backups"],
  [/^Comme le profil \((.+)\)$/, "Same as profile ($T1)"],
  [/^Comme la page \((.+)\)$/, "Same as page ($T1)"],
  [/^Fermer (\S+\.exe)$/i, "Close $1"],
  [/^Fermer (.+)$/, "Close $1"],
  [/^Forcer la fermeture de (.+)$/, "Force close $1"],
  [/^Ouvrir le dossier « (.+) »$/, "Open the “$1” folder"],
  [/^Ouvrir (.+)$/, "Open $1"],
  [/^Couper (.+)$/, "Mute $1"],
  [/^Appuyer sur (.+)$/, "Press $1"],
  [/^IA : (.+)$/, "AI: $T1"],
  [/^Rechercher dans « (.+) »$/, "Search in “$1”"],
  [/^Touche (\d+) copiée\. Sélectionne une autre touche et fais Ctrl\+V\.$/, "Key $1 copied. Select another key and press Ctrl+V."],
  [/^Touche collée sur la (\d+) \(Ctrl\+Z pour annuler\)\.$/, "Key pasted on $1 (Ctrl+Z to undo)."],
  [/^Vider la touche (\d+) \?$/, "Clear key $1?"],
  [/^Touche rangée dans le dossier « (.+) » \(touche (\d+)\)\.$/, "Key moved to the “$1” folder (key $2)."],
  [/^Le dossier « (.+) » est plein \(15 touches\)\.$/, "The “$1” folder is full (15 keys)."],
  [/^Supprimer la page « (.+) » et ses touches \?$/, "Delete page “$1” and its keys?"],
  [/^Supprimer le profil « (.+) » \? \(une copie est gardée dans profiles-supprimes\)$/, "Delete profile “$1”? (a copy is kept in profiles-supprimes)"],
  [/^Supprimer l'animation « (.+) » \?$/, "Delete the “$1” animation?"],
  [/^Supprimer l'addon « (.+) » \? Les touches déjà placées gardent leur action\.$/, "Delete the “$1” addon? Keys already placed keep their action."],
  [/^Style « (.+) » appliqué à tout le profil$/, "“$T1” style applied to the whole profile"],
  [/^Style « (.+) » appliqué à toute la page$/, "“$T1” style applied to the whole page"],
  [/^Mettre le style « (.+) » sur toutes les touches du profil \? Les styles propres aux pages et aux touches seront retirés\.$/, "Use the “$T1” style on every key of the profile? Page and key styles will be removed."],
  [/^Pack d'icônes « (.+) » ajouté : (\d+) icônes?\. Tu le trouves dans « Icônes… » de chaque touche\.$/, "Icon pack “$1” added: $2 icons. You'll find it under “Icons…” on every key."],
  [/^Retirer le pack « (.+) » \? Les touches qui utilisent ses icônes n'auront plus d'image\.$/, "Remove the “$1” pack? Keys using its icons will have no image anymore."],
  [/^Profil « (.+) » importé et activé\.$/, "Profile “$1” imported and activated."],
  [/^Profil « (.+) » importé avec (.+) et activé\.$/, "Profile “$1” imported with $T2 and activated."],
  [/^Profil « (.+) » importé(?: avec (.+?))?\. Touches sans action \(plugins Stream Deck sans équivalent, en gris\) : (.+)\.$/, "Profile “$1” imported. Keys without an action (Stream Deck plugins with no equivalent, in grey): $3."],
  [/^Profil « (.+) » importé(?: avec (.+?))?\. Addons à installer pour certaines touches : (.+)\.$/, "Profile “$1” imported. Addons to install for some keys: $3."],
  [/^Profil exporté : (.+)$/, "Profile exported: $1"],
  [/^Profil non importé : (.+)$/, "Profile not imported: $1"],
  [/^Son « (.+) » ajouté sur la touche (\d+)\.$/, "Sound “$1” added to key $2."],
  [/^Garder la touche enfoncée ([\d,.]+) s lance une autre action\.$/, "Holding the key for $1 s runs another action."],
  [/^Deux appuis rapides lancent une autre action\. L'appui simple attend alors (\d+) ms pour être sûr que ce n'en est pas un\.$/, "Two quick presses run another action. A single press then waits $1 ms to make sure it isn't one."],
  [/^Nom de la copie de « (.+) » :$/, "Name of the copy of “$1”:"],
  [/^Sortie suivante \((\d+)\)$/, "Next output ($1)"],
  [/^Valeur actuelle : (.+)\.$/, "Current value: $1."],
  [/^HubForge est à jour · vérifié (.+)$/, "HubForge is up to date · checked $T1"],
  [/^il y a (\d+) min$/, "$1 min ago"],
  [/^il y a (\d+) h$/, "$1 h ago"],
  [/^il y a (\d+) j$/, "$1 days ago"],
  [/^jamais$/, "never"],
  [/^(\d+) images?$/, "$1 images"],
  [/^(\d+) sons?$/, "$1 sounds"],
  [/^(.+) et (.+)$/, "$T1 and $T2", true],
  [/^Mise à jour (.+)$/, "Update $1"],
  [/^Addon « (.+) » installé : ses actions sont dans la liste de gauche\.$/, "Addon “$1” installed: its actions are in the list on the left."],
  [/^Addon non installé : (.+)$/, "Addon not installed: $1"],
  [/^« (.+) » joue maintenant sur la base\.$/, "“$1” is now playing on the base."],
  [/^« (.+) » joue maintenant sur les 5 faders\.$/, "“$1” is now playing on the 5 faders."],
  [/^🏆 Nouveau record : (\d+) points !$/, "🏆 New record: $1 points!"],
  [/^👹 Chargement de Doom \((.+)\)…$/, "👹 Loading Doom ($1)…"],
  [/^Remettre le profil « (.+) » comme il était le (.+) \? \(La version actuelle est sauvegardée avant, tu pourras y revenir\.\)$/, "Restore profile “$1” as it was on $2? (The current version is saved first, so you can come back to it.)"],
  [/^Réexamen impossible : (.+)\. Débranche puis rebranche le câble USB de la base\.$/, "Rescan failed: $1. Unplug and plug the base's USB cable back in."],
  [/^(.+) était lié à la page « (.+) » : déplacé ici\.$/, "$1 was linked to page “$2”: moved here."],
  [/^Ajouter (ses pages|sa page) au profil « (.+) »$/, "Add its page(s) to profile “$2”"],
  [/^Version (.+)$/, "Version $1"],
  [/^Branché : (.+)\. Clique pour le diagnostic\.$/, "Plugged in: $T1. Click for diagnostics."],
  [/^Branché : (.+)\.$/, "Plugged in: $T1."],
  [/^(.+), (.+)$/, "$T1, $T2", true],
  [/^base$/, "base"],
  [/^Ce réglage s'applique sur toutes les pages qui n'ont pas le leur\.$/, "This setting applies to every page that doesn't have its own."],
  [/^Sur cette page, ce fader a son propre réglage\. Les autres pages gardent le réglage commun\.$/, "On this page, this fader has its own setting. Other pages keep the shared setting."],
  [/^Profil « (.+) » importé$/, "Profile “$1” imported"],
];

/** Morceaux de phrases (textes assemblés par morceaux) : les plus longs d'abord. */
const EN_FRAGMENTS = Object.keys(EN_DICT).filter((k) => k.length >= 12 && k.includes(" ")).sort((a, b) => b.length - a.length);

function tr(s) {
  if (LANG !== "en" || typeof s !== "string") return s;
  const m = /^(\s*)([\s\S]*?)(\s*)$/.exec(s);
  const core = m[2];
  if (!core || !/[A-Za-zÀ-ÿ]/.test(core)) return s;
  const out = trCore(core, 0);
  return out === null ? s : m[1] + out + m[3];
}

function trCore(core, depth) {
  if (Object.prototype.hasOwnProperty.call(EN_DICT, core)) return EN_DICT[core];
  if (depth > 3) return null;
  for (const [rx, rep, nested] of EN_PATTERNS) {
    if (nested && depth === 0) continue;
    const mm = rx.exec(core);
    if (!mm) continue;
    return rep.replace(/\$T(\d)/g, (_, i) => { const g = mm[+i] || ""; const t = trCore(g, depth + 1); return t === null ? g : t; })
      .replace(/\$(\d)/g, (_, i) => mm[+i] || "");
  }
  if (depth > 0) return null;
  // Dernier recours : remplacer les morceaux connus.
  let t = core, hit = false;
  for (const f of EN_FRAGMENTS) {
    if (t.includes(f)) { t = t.split(f).join(EN_DICT[f]); hit = true; }
  }
  return hit ? t : null;
}

const TR_ATTRS = ["placeholder", "title", "aria-label"];
function trSkip(node) {
  const e = node.nodeType === 1 ? node : node.parentElement;
  if (!e) return true;
  if (e.closest('[translate="no"], script, style, textarea, code, pre')) return true;
  return false;
}
function trNode(node) {
  if (node.nodeType === 3) {
    if (trSkip(node)) return;
    const v = node.nodeValue;
    const t = tr(v);
    if (t !== v) node.nodeValue = t;
    return;
  }
  if (node.nodeType !== 1 || trSkip(node)) return;
  for (const a of TR_ATTRS) {
    const v = node.getAttribute(a);
    if (v) { const t = tr(v); if (t !== v) node.setAttribute(a, t); }
  }
  if (node.tagName === "INPUT" && (node.type === "button" || node.type === "submit") && node.value) node.value = tr(node.value);
  for (const c of node.childNodes) trNode(c);
}

/** Choisit la langue ; en anglais, traduit la page et tout ce qui s'y ajoute. */
function setupLang(lang) {
  LANG = lang === "en" ? "en" : "fr";
  document.documentElement.lang = LANG;
  if (LANG !== "en") return;
  trNode(document.body);
  document.title = tr(document.title);
  const obs = new MutationObserver((muts) => {
    for (const m of muts) {
      if (m.type === "characterData") trNode(m.target);
      else if (m.type === "attributes") trNode(m.target);
      else m.addedNodes.forEach(trNode);
    }
  });
  obs.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: TR_ATTRS });
  const p = window.prompt.bind(window), c = window.confirm.bind(window), a = window.alert.bind(window);
  window.prompt = (msg, def) => p(tr(msg), def);
  window.confirm = (msg) => c(tr(msg));
  window.alert = (msg) => a(tr(msg));
}
'''
open(os.path.join(HERE, '..', '..', 'apps', 'hubforge', 'ui', 'js', 'i18n.js'), 'w', encoding='utf-8').write(js)
print(len(D))
