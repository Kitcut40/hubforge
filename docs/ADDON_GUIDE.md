# Créer un addon HubForge

Un addon ajoute à HubForge des **actions prêtes à l'emploi** (avec leurs icônes), des **pages toutes faites** et la **détection d'une application**. Ce guide décrit le **niveau 1** : un addon déclaratif, sans aucun code, donc sans aucun risque pour l'utilisateur.

> Plus aucun addon n'est livré avec HubForge : ils seront proposés dans un catalogue à télécharger.

---

## 1. Structure

```
monlogiciel/              ← le nom du dossier = l'id de l'addon
├── manifest.toml         ← obligatoire
└── icons/                ← facultatif : icônes SVG (conseillé) ou PNG/JPG/GIF/WebP
    ├── action1.svg
    └── …
```

Pour distribuer l'addon : **compresse le dossier en ZIP** et renomme le fichier en **`monlogiciel.forge`**. Le `manifest.toml` peut être à la racine de l'archive ou dans un seul dossier.

## 2. Installer

- Glisser le fichier `.forge` sur la fenêtre de HubForge, ou
- bouton **Addons** (pièce de puzzle, en haut à droite) → **Installer un addon…**, ou
- copier le `.forge` **ou le dossier non compressé** dans `%APPDATA%\HubForge\addons\`.

L'addon apparaît **sans redémarrer** (HubForge surveille le dossier). En cas d'erreur dans le manifeste, la page Addons affiche le message exact (champ, action, touche…).

Pour développer, travaille directement dans `%APPDATA%\HubForge\addons\<id>\` : chaque enregistrement du `manifest.toml` recharge l'addon.

## 3. Le manifeste

### 3.1 `[addon]` — obligatoire

```toml
[addon]
id          = "monlogiciel"       # minuscules, chiffres, - et _ ; = nom du dossier
name        = "Mon logiciel"      # obligatoire
version     = "1.0.0"
author      = "Ton nom"
license     = "MIT"               # au choix de l'auteur (voir ADDON-EXCEPTION.md)
description = "Ce que fait l'addon, en une phrase."
icon        = "icons/logo.svg"    # ou "builtin:nom", ou "auto" (voir § 4), facultatif
format      = 1                   # version du format (1 pour l'instant)
```

### 3.2 `[detect]` — facultatif

```toml
[detect]
windows = ["MonLogiciel.exe"]     # nom du programme (voir Gestionnaire des tâches → Détails)
```

Les pages de l'addon ajoutées au profil s'afficheront **toutes seules** quand ce programme passe au premier plan (pages automatiques).

### 3.3 `[[category]]` — dossiers (facultatif)

Dans la bibliothèque de HubForge, chaque addon est **un seul dossier** (à son nom). Les `[[category]]` sont les **sous-dossiers** de ce dossier ; avec `parent`, un sous-dossier peut en contenir d'autres (4 niveaux au maximum). Les actions sans `category` sont directement dans le dossier de l'addon.

```toml
[[category]]
id   = "dessin"
name = "Dessin"

[[category]]
id     = "formes"
name   = "Formes"
parent = "dessin"      # sous-dossier de « Dessin »
```

Résultat dans la bibliothèque :

```
▸ Mon logiciel
   ▸ Dessin
      ▸ Formes
         Ligne, Rectangle…
      Créer une esquisse…
```

### 3.4 `[[action]]` — les actions

Chaque action a des **informations d'affichage**, puis la **description de l'action elle-même**, exactement comme dans `config.toml`.

```toml
[[action]]
id          = "extrude"            # obligatoire, unique dans l'addon
name        = "Extrusion"          # obligatoire : nom dans la bibliothèque
label       = "Extrusion"          # texte sur la touche (par défaut : name)
category    = "dessin"             # id d'une [[category]]
icon        = "icons/extrude.svg"  # ou "builtin:nom"
color       = "#EF6C00"            # couleur de fond de la touche
text_color  = "#FFFFFF"            # couleur du texte et de l'icône SVG
description = "Raccourci : E"      # affiché au survol
type        = "hotkey"             # ↓ l'action
keys        = "e"
```

Types d'action disponibles :

| `type` | Champs | Exemple |
|---|---|---|
| `hotkey` | `keys` | `keys = "ctrl+shift+s"` · touches : `ctrl alt shift win`, lettres, chiffres, `f1`–`f24`, `enter esc tab space delete backspace home end pageup pagedown up down left right printscreen` |
| `text` | `text` | `text = "Bonjour !"` (accents acceptés) |
| `launch` | `target`, `args` (facultatif) | `target = "https://exemple.fr"` ou `"notepad.exe"` ou un dossier |
| `media` | `command` | `play_pause next previous stop mute volume_up volume_down mic_mute` |
| `page` | `to` | `next`, `previous` ou un numéro |
| `back` | — | ferme un dossier |
| `lock` | — | verrouille le PC |
| `macro` | `steps` | liste d'actions, avec des pauses `{ type = "wait", ms = 300 }` |
| `none` | — | ne fait rien |

Exemple de macro (ouvrir la recherche de commandes, taper un nom, valider) :

```toml
[[action]]
id    = "chamfer"
name  = "Chanfrein"
icon  = "icons/chamfer.svg"
type  = "macro"
steps = [
  { type = "hotkey", keys = "s" },
  { type = "wait", ms = 350 },
  { type = "text", text = "Chanfrein" },
  { type = "wait", ms = 600 },
  { type = "hotkey", keys = "enter" },
]
```

Interdits dans un addon (pour la sécurité de l'utilisateur) : `profile`, `game`, `open_config`.

### 3.5 `[[page]]` — pages prêtes à l'emploi (facultatif)

L'utilisateur les ajoute à son profil en un clic (« Ajouter ses pages au profil »).

```toml
[[page]]
id   = "main"
name = "Mon logiciel"
# apps = ["MonLogiciel.exe"]      # par défaut : celles de [detect]
keys = [
  { key = 1,  action = "extrude" },
  { key = 2,  action = "chamfer", label = "Chanf." },   # texte remplacé
  { key = 15, open = "dessin", label = "Dessin…" },     # ouvre la page "dessin"
]

[[page]]
id          = "dessin"
name        = "Mon logiciel — Dessin"
folder      = true                # page dossier (absente de page suivante/précédente)
close_after = false               # true = revient à la page d'avant après une action
keys = [
  { key = 1, back = true },       # touche Retour
  { key = 2, action = "line" },
]
```

Les touches sont numérotées de **1** (en haut à gauche) à **15** (en bas à droite), ligne par ligne.

## 4. Icônes

- **SVG conseillé**, dans le style [Lucide](https://lucide.dev) : `viewBox="0 0 24 24"`, `fill="none"`, `stroke="currentColor"`, `stroke-width="2"`. `currentColor` prend la couleur du texte de la touche.
- PNG / JPG / GIF / WebP acceptés (affichés tels quels, le GIF est animé).
- `builtin:nom` utilise une des icônes intégrées à HubForge (bouton « Icônes… » de l'interface pour voir les noms : `save`, `play`, `mic`, `folder`, `settings`…).
- **Pas de logos de marques** dans un addon partagé publiquement sans autorisation de leur propriétaire.
- **`icon = "auto"`** (icône de l'addon seulement) : HubForge extrait l'icône du **logiciel installé sur le PC** de l'utilisateur, d'après `[detect]` (programme ouvert, sinon recherche dans Program Files et AppData). Le logo n'est jamais redistribué dans l'addon. Sans le logiciel installé, une icône générique s'affiche.
- **Icônes d'un logiciel open source** : autorisées si tu respectes leur licence. Exemple : l'addon PrusaSlicer reprend les icônes d'outils de PrusaSlicer (AGPL-3.0), donc l'addon est lui-même sous AGPL-3.0, avec la source indiquée (`icons/prusaslicer/SOURCE.md`) et le texte de la licence.

## 5. Erreurs fréquentes

| Message | Solution |
|---|---|
| `la section [addon] est obligatoire` | ajoute `[addon]` avec `id` et `name` |
| `uniquement minuscules, chiffres, - et _` | `id = "fusion360"`, pas `"Fusion 360"` |
| `le dossier doit s'appeler comme l'id` | renomme le dossier comme l'`id` |
| `le champ « type » est obligatoire` | chaque `[[action]]` a besoin de `type = "…"` |
| `catégorie « x » inconnue` | ajoute le `[[category]]` correspondant |
| `dossier parent « x » inconnu` / `boucle` | `parent` doit être l'`id` d'un autre `[[category]]`, sans boucle |
| `icône introuvable` | vérifie le chemin (`icons/…`) et la casse |
| `Expected newline…` | en TOML, une ligne = un champ : pas de `type = "hotkey", keys = "e"` sur la même ligne (sauf entre `{ }`) |

## 6. Faire écrire un addon par une IA

Copie ce texte dans Claude (ou une autre IA), en complétant la fin :

```
Crée un addon HubForge (niveau 1, déclaratif) pour le logiciel <NOM DU LOGICIEL>.
Respecte exactement le format décrit dans ce guide : <colle ici le contenu de ADDON_GUIDE.md>.

Je veux :
- l'id "<id>", le programme à détecter "<Programme.exe>" ;
- ces actions (nom → raccourci clavier) : <liste> ;
- une page principale de 15 touches avec les actions les plus utiles, et si besoin
  une page dossier pour le reste (touche 1 = Retour) ;
- une icône SVG simple style Lucide pour chaque action (24×24, stroke currentColor) ;
- des couleurs par famille d'actions.

Donne-moi le manifest.toml complet et chaque fichier SVG, puis explique comment
en faire un fichier .forge.
```

## 7. Licence

Les addons qui n'utilisent que ce format (manifeste + icônes) **ne sont pas soumis à la GPL** de HubForge : chaque auteur choisit sa licence. Voir [`ADDON-EXCEPTION.md`](../ADDON-EXCEPTION.md).

## 8. Et après ?

Les niveaux 2 (scripts Lua dans un bac à sable, touches qui changent selon l'état du logiciel) et 3 (programme externe) sont prévus dans [`SPECS.md`](SPECS.md) §4.5. Le `format = 1` restera compris par toutes les versions futures.
