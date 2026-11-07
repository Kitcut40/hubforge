# SPECS — Logiciel custom pour Cooler Master MasterHUB

> **Statut : v1.0 — VALIDÉE pour le socle du logiciel.**
> Priorité absolue : un logiciel **propre et fonctionnel de A à Z** (sections 2 à 9).
> Les addons métiers (Fusion 360, OBS, PrusaSlicer, SolidCraft, MSFS) seront précisés **après** le socle. Les points ❓ restants les concernent.

Nom du logiciel : **HubForge** ✅ — extension des addons : **`.forge`**

### Historique des décisions
| Version | Décision |
|---|---|
| v0.1 | Rust, Windows + Linux, 100 % hors ligne, `.exe` installable |
| v0.2 | Interface : **Tauri 2** (priorité au rendu visuel) |
| v0.2 | **Projet open source** (GitHub) |
| v0.2 | Streamlabs et Adobe **retirés** ; suite **SolidCraft / SolveCraft, PrusaSlicer** ajoutés |
| v0.2 | **Système d'addons** à glisser dans un dossier = cœur du projet |
| v0.3 | Scripts des addons niveau 2 : **Lua 5.4** (via `mlua`, bac à sable) |
| v0.3 | Licence : **GPL-3.0** — toute copie ou version modifiée distribuée doit rester open source |
| v0.4 | Nom : **HubForge** ; addons en **`.forge`** |
| v0.4 | Addons tiers : **licence au choix de leur auteur** (exception d'addon ajoutée à la GPL) |
| v0.5 | **MSFS 2024 repoussé** (P3, après la v1) — Benjamin n'y joue plus pour l'instant |
| v0.5 | Pas d'autre slicer que PrusaSlicer |
| v0.5 | Addon Fusion 360 détaillé (section 5.7) |
| v1.0 | **Priorité au socle** : logiciel complet et fiable d'abord ; addons métiers détaillés ensuite |
| v1.0 | Mise à jour du firmware : **laissée au logiciel officiel** (risque de rendre le MasterHUB inutilisable) |

---

## 1. Contexte et objectif

Le MasterHUB a un bon matériel (modules magnétiques, touches à écran IPS), mais le logiciel officiel est quasiment abandonné par Cooler Master : bugs, peu d'intégrations, raccourcis à saisir à la main sans liste déroulante, logiciel obligatoire en permanence.

**Objectif :** un logiciel libre, 100 % maison, qui :
- fait **tout ce que fait le logiciel officiel**, en mieux ;
- ajoute ce que les utilisateurs réclament (intégrations, ergonomie, fiabilité) ;
- couvre les usages de Benjamin : **Fusion 360, Windows/Linux, OBS, suite maker (SolidCraft, PrusaSlicer)** — MSFS 2024 plus tard ;
- peut être **étendu par n'importe qui avec de simples fichiers d'addon**, y compris générés avec une IA (Claude Code, etc.).

## 2. Contraintes non négociables

| # | Contrainte |
|---|---|
| C1 | Vrai logiciel installé : installeur `.exe`/`.msi` Windows, paquet Linux |
| C2 | **Fonctionne 100 % hors ligne**, aucun compte, aucune télémétrie |
| C3 | **Interface la plus belle possible** (au moins le niveau d'Elgato / Loupedeck) |
| C4 | Windows 10/11 64 bits **et** Linux |
| C5 | Léger : service d'arrière-plan < 50 Mo de RAM, CPU ≈ 0 % au repos |
| C6 | Réactif : délai appui → action imperceptible (cible < 10 ms) |
| C7 | Ne casse rien : retour au logiciel officiel possible à tout moment |
| C8 | **Open source**, code et format d'addon documentés publiquement |

## 3. Matériel à supporter

| Module | Possédé | Contrôles |
|---|---|---|
| Base (FlexBase) | ✅ | Connexion USB-C, détection des modules |
| 15 touches à écran IPS | ✅ | 15 boutons + 15 images |
| 5 faders | ✅ | 5 curseurs analogiques + LED |
| 2 molettes (rollers) | ✅ | 2 défilements infinis |
| Encodeur + écran IPS | 🛒 envisagé | Rotation + appui + écran |
| 3 knobs | 🛒 envisagé | 3 boutons rotatifs + éclairage |

Le logiciel est **modulaire dès le départ** : ajouter un module = ajouter un "pilote" de module, sans refaire le reste.

---

## 4. Système d'addons ⭐ (cœur du projet)

### 4.1 Principe
Un **addon** est un fichier unique qu'on glisse dans le dossier `addons/` (ou directement sur la fenêtre du logiciel). Il ajoute :
- une **catégorie** dans la bibliothèque d'actions (ex. « Fusion 360 ») ;
- des **actions** prêtes à l'emploi (Extrusion, Esquisse, Congé, Orbite…) avec leurs **icônes** ;
- des **sous-catégories** (Esquisse, Modélisation, Vues, Inspection…) ;
- la **détection de l'application** (bascule automatique de profil quand elle passe au premier plan) ;
- optionnellement un **profil prêt à l'emploi** pour tous les modules (touches + faders + molettes).

Exemple : je dépose `fusion360.forge` → la catégorie Fusion 360 apparaît **instantanément**, sans redémarrer.

### 4.2 Emplacement des addons
| Dossier | Usage |
|---|---|
| `<installation>/addons/` | Addons officiels livrés avec le logiciel |
| Dossier utilisateur (`%APPDATA%\HubForge\addons\` / `~/.config/hubforge/addons/`) | Addons ajoutés par l'utilisateur — **pas besoin des droits administrateur** |

Bouton « Ouvrir le dossier des addons » dans l'interface + **glisser-déposer du fichier sur la fenêtre** pour l'installer.

### 4.3 Format du fichier
- Extension : **`.forge`**
- C'est une **archive ZIP renommée** (comme `.vsix` ou `.streamDeckPlugin`) qui contient :

```
fusion360.forge
├── manifest.toml        ← description de l'addon (obligatoire)
├── icons/               ← icônes des actions (SVG ou PNG)
├── profiles/            ← profils prêts à l'emploi (optionnel)
└── scripts/             ← logique avancée (optionnel, niveau 2)
```

- Pour développer : un **dossier non compressé** fonctionne aussi (rechargé à chaque modification).

### 4.4 Exemple de `manifest.toml` (niveau 1, sans code)

```toml
[addon]
id          = "fusion360"
name        = "Fusion 360"
version     = "1.0.0"
author      = "Benjamin"
license     = "GPL-3.0"
description = "Commandes Fusion 360 pour MasterHUB"
icon        = "auto"          # icône extraite du logiciel installé sur le PC
format      = 1               # version du format d'addon

[detect]                      # bascule auto de profil
windows = ["Fusion360.exe"]
linux   = []

[[category]]
id   = "sketch"
name = "Esquisse"

[[category]]
id   = "model"
name = "Modélisation"

[[action]]
id       = "extrude"
name     = "Extrusion"
category = "model"
icon     = "icons/extrude.svg"
type     = "hotkey"
keys     = "E"

[[action]]
id       = "sketch_line"
name     = "Ligne"
category = "sketch"
icon     = "icons/line.svg"
type     = "hotkey"
keys     = "L"

[[action]]                    # action pour molette / fader
id       = "zoom"
name     = "Zoom"
category = "view"
type     = "rotary"           # molette : un sens = action A, l'autre = action B
up       = { type = "scroll", amount = 1 }
down     = { type = "scroll", amount = -1 }
```

### 4.5 Trois niveaux d'addon
| Niveau | Contenu | Capacités | Sécurité |
|---|---|---|---|
| **1 — Déclaratif** | Uniquement `manifest.toml` + icônes | Raccourcis, macros, lancer une appli, URL, texte, profils | ✅ Aucun risque, aucun code |
| **2 — Scripté** | + scripts **Lua 5.4** dans un bac à sable | Logique, états dynamiques (touche qui change d'image), API locales (WebSocket, HTTP local) | ⚠️ Permissions déclarées et affichées à l'installation |
| **3 — Natif** | + programme externe (n'importe quel langage) qui dialogue avec le logiciel | Intégrations lourdes (SimConnect pour MSFS, add-in Fusion…) | ⚠️⚠️ Avertissement clair, à réserver aux addons de confiance |

### 4.6 Permissions (niveaux 2 et 3)
L'addon déclare ce dont il a besoin ; l'utilisateur valide à l'installation :
`reseau_local`, `internet`, `lancer_programmes`, `lire_fichiers`, `ecrire_fichiers`, `audio`, `etat_systeme`.
Un addon de niveau 1 n'a besoin d'aucune permission.

### 4.7 Pensé pour l'IA
- **Format simple et lisible** (TOML) que Claude Code ou une autre IA peut écrire sans erreur
- **Schéma officiel** (JSON Schema) publié → l'IA et les éditeurs de code valident le fichier automatiquement
- **Guide `ADDON_GUIDE.md`** + **prompt modèle** prêt à copier : « Crée un addon HubForge pour <logiciel> avec les raccourcis suivants… »
- **Validateur intégré** : si un addon est mal écrit, l'interface explique précisément l'erreur (ligne, champ) au lieu de planter
- **Mode développeur** : rechargement à chaud, journal des événements de l'addon

### 4.8 Icônes et marques
- Icône d'application `auto` : **extraite du logiciel installé sur le PC** de l'utilisateur
- Les logos officiels (Autodesk, Microsoft…) **ne sont pas inclus dans le dépôt open source** (marques déposées) ; les icônes d'actions sont dessinées pour le projet ou sous licence libre

### 4.9 Les intégrations officielles sont elles-mêmes des addons
Fusion 360, OBS, SolidCraft, PrusaSlicer (et MSFS plus tard) sont livrés **sous forme d'addons**. Ça garantit que le système d'addons est assez puissant, et ça sert d'exemples.

### 4.10 Partage
- **P2** Export d'un profil + ses addons en un seul fichier partageable
- **P3** Catalogue communautaire ❓ (simple dossier du dépôt GitHub, consultable sans compte ; le logiciel reste utilisable hors ligne)

### 4.11 Licences
| Élément | Licence |
|---|---|
| Logiciel HubForge (service, interface, pilotes) | **GPL-3.0** — toute version modifiée distribuée doit rester open source |
| Addons officiels livrés avec HubForge | **GPL-3.0** |
| Addons créés par d'autres | **Au choix de leur auteur** (libre, propriétaire, gratuit, payant…) |

- Pour éviter toute ambiguïté juridique (un addon chargé par un programme GPL pourrait être considéré comme une « œuvre dérivée »), le dépôt contient un fichier **`ADDON-EXCEPTION.md`** : une exception explicite à la GPL qui précise que **les addons qui utilisent uniquement l'API publique d'addons** (manifeste, API Lua, protocole des addons natifs) ne sont pas soumis à la GPL.
- Même principe que l'exception utilisée par le noyau Linux pour les programmes qui l'utilisent.
- Le champ `license` est ajouté au manifeste (ex. `license = "MIT"`), affiché dans le gestionnaire d'addons.

---

## 5. Fonctionnalités

Priorité : **P1** = version 1 indispensable · **P2** = version 1 si possible · **P3** = plus tard

### 5.1 Matériel et appareil
- **P1** Détection automatique de la base et des modules (branchés à chaud)
- **P1** Affichage de la disposition réelle des modules dans l'interface
- **P1** Réglage de luminosité des écrans et LED
- **P2** Mise en veille des écrans après X minutes / extinction quand le PC est verrouillé
- **P2** Éclairage RGB par module (couleur fixe, effets, couleur liée à l'état d'une action)
- **P2** Affichage de la version du firmware
- ~~Mise à jour du firmware~~ → hors périmètre, faite avec le logiciel officiel
- **P3** Profils stockés dans le MasterHUB lui-même (si le firmware le permet ❓)

### 5.2 Profils, pages et dossiers
- **P1** Profils multiples (un profil = configuration de tous les modules)
- **P1** Pages par profil, **sans la limite de 5 pages** du logiciel officiel
- **P1** **Changement automatique de profil selon l'application au premier plan**
- **P1** Dossiers sur les touches (une touche ouvre une sous-grille)
- **P2** Import / export de profils
- **P2** Sauvegarde automatique de la config + restauration
- **P2** Profil par défaut au démarrage

### 5.3 Touches à écran (15 touches)
- **P1** Image, icône ou texte par touche
- **P1** **Import automatique de l'icône d'une application**
- **P1** GIF animés
- **P2** Éditeur d'icône intégré : fond, couleur, texte, emoji, bibliothèque d'icônes
- **P2** Touches dynamiques : l'image change selon un état (micro coupé, autopilote ON / OFF…)
- **P2** Touches "widget" : heure, CPU, GPU, températures, RAM
- **P3** Une image découpée sur plusieurs touches

### 5.4 Actions de base (intégrées, hors addons)
- **P1** Raccourci clavier **avec sélecteur en liste déroulante** + mode capture
- **P1** Macros : séquence de touches, délais, enregistrement
- **P1** Lancer une application, un fichier, un dossier, une URL
- **P1** Taper un texte
- **P1** Contrôle multimédia (lecture / pause, suivant, précédent)
- **P1** Volume système, coupure micro
- **P1** Navigation : page suivante / précédente, ouvrir un dossier, changer de profil
- **P2** Multi-actions (plusieurs actions sur un appui)
- **P2** Actions bascule (2 états)
- **P2** **Appui court / appui long / double appui** = actions différentes
- **P2** Lancer une commande / un script (PowerShell, bash, Python)
- **P2** Changer de sortie audio (casque ↔ enceintes)
- **P3** Requête HTTP (domotique, homelab futur)

### 5.5 Faders et molettes
- **P1** Fader → volume général, micro, **volume par application**
- **P1** Molette → défilement, zoom, ou actions « + / − » personnalisées
- **P2** Courbes de réponse (linéaire, logarithmique), inversion, zone morte
- **P2** Molette avec accélération (tourner vite = plus grands pas)
- **P2** LED des faders liées au niveau ou à l'état (muet = rouge)

### 5.6 Addons livrés avec la version 1
| Addon | Priorité | Niveau | Contenu |
|---|---|---|---|
| **Système** (Windows / Linux) | P1 | intégré | Audio par appli, médias, fenêtres |
| **Fusion 360** | P1 | 1 puis 3 | Voir section 5.7 |
| **OBS Studio** | P1 | 2 | obs-websocket : scènes, sources, mixage, enregistrement / live, état en direct |
| **SolidCraft / SolveCraft** | P2 | 1 | Raccourcis de modélisation ❓ (selon les raccourcis disponibles dans le logiciel) |
| **PrusaSlicer** | P2 | 1 | Raccourcis : vues, découpe, rotation, export G-code… |
| Discord | P2 | 1 | Mute / sourdine via raccourcis globaux |
| Lecteurs média / Spotify | P2 | intégré | Titre + pochette sur une touche |
| Surveillance matériel | P2 | intégré | Widgets CPU / GPU / RAM |
| MSFS 2024 | **P3 (après v1)** | 3 | SimConnect : AP, radios, train, volets, lumières + lecture d'état. Windows uniquement |

### 5.7 Addon Fusion 360 (premières notes — à préciser après le socle)

**Commandes demandées et raccourcis par défaut de Fusion :**

| Commande | Catégorie | Raccourci natif Fusion |
|---|---|---|
| Créer une esquisse | Esquisse | ❌ aucun |
| Ligne | Esquisse | `L` |
| Rectangle 2 points | Esquisse | `R` |
| Cercle centre-diamètre | Esquisse | `C` |
| Cercle 2 points | Esquisse | ❌ aucun |
| Cercle 3 points | Esquisse | ❌ aucun |
| Arc 3 points | Esquisse | ❌ aucun |
| Ajuster (trim) | Esquisse | `T` |
| Décalage (offset) | Esquisse | `O` |
| Cote | Esquisse | `D` |
| Extrusion | Modélisation | `E` |
| Congé (fillet) | Modélisation | `F` |
| Chanfrein | Modélisation | ❌ aucun |
| Symétrie (mirror) | Modélisation | ❌ aucun |
| Trou | Modélisation | `H` |
| Appuyer/tirer | Modélisation | `Q` |
| Déplacer | Modélisation | `M` |

**Problème :** 6 commandes demandées n'ont pas de raccourci par défaut.

**Solution en 2 temps :**
1. **v1 (niveau 1, sans code)** — pour les commandes sans raccourci, macro via la **boîte à outils de recherche de Fusion** : touche `S` → saisie du nom de la commande → `Entrée`. Fonctionne sans rien configurer dans Fusion.
   *Limite :* dépend de la langue de Fusion (noms FR/EN) et peut être un peu moins instantané ; l'addon contiendra les noms dans les deux langues.
2. **v2 (niveau 3)** — **add-in Fusion** (petit script Python installé dans Fusion) qui reçoit les ordres de HubForge et lance chaque commande par son **identifiant interne**. Instantané, indépendant de la langue et des raccourcis, et permet de savoir si on est en mode esquisse ou modélisation (changement de page automatique).

**Disposition proposée — 15 touches (3 rangées × 5)** ❓ à valider

| | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| **Rangée 1 — Esquisse** | Créer esquisse | Ligne | Rectangle 2 pts | Cercle centre | Cercle 2 pts |
| **Rangée 2 — Esquisse** | Cercle 3 pts | Arc 3 pts | Ajuster | Décalage | Cote |
| **Rangée 3 — Modélisation** | Extrusion | Congé | Chanfrein | Symétrie | Valider (`Entrée`) |

- Appui long sur une touche = variante (ex. appui long sur *Annuler / Valider* → `Échap`)
- Page 2 (P2) : Trou, Appuyer/tirer, Déplacer, vues (dessus, face, droite, maison), visibilité

**Molettes :**
| Molette | Rotation | Appui (si le module le permet) |
|---|---|---|
| Molette 1 | **Zoom** avant / arrière | Ajuster la vue (« fit ») |
| Molette 2 | **Annuler / Rétablir** (`Ctrl+Z` / `Ctrl+Y`) | — |

**Faders :** ❓ Fusion n'a pas de réglage continu utile via raccourcis → proposition : les faders gardent le **mixage audio** (système, musique, Discord…) même dans le profil Fusion. Avec l'add-in (v2), un fader pourrait piloter la **valeur de l'extrusion ou du congé en direct**.

### 5.8 Interface
- **P1** Design soigné : thème sombre, animations fluides, typographie et icônes cohérentes
- **P1** Vue du MasterHUB reproduisant la disposition physique des modules
- **P1** Bibliothèque d'actions à gauche, classée par catégories (= addons), avec recherche
- **P1** **Glisser-déposer** des actions sur les touches / faders / molettes
- **P1** Aperçu en direct : appuyer sur un bouton physique le sélectionne dans l'interface
- **P1** Gestionnaire d'addons : liste, activer / désactiver, permissions, erreurs
- **P1** Icône dans la zone de notification, démarrage avec le système
- **P2** Interface en français + anglais (traductions en fichiers, contributions faciles)
- **P2** Thème clair en option
- **P3** Annuler / rétablir

---

## 6. Architecture (proposition)

```
┌────────────────────────┐        ┌───────────────────────────┐
│  Interface Tauri 2     │◄──────►│  Service d'arrière-plan   │
│  ouverte à la demande  │  local │  Rust (toujours actif)    │
└────────────────────────┘        └─────────────┬─────────────┘
                                                │
       ┌──────────────┬──────────────┬──────────┴───┬──────────────┬──────────────┐
       ▼              ▼              ▼              ▼              ▼              ▼
  Pilote USB     Moteur         Gestionnaire   Audio /        Détection      Hôte de
  MasterHUB      d'actions      d'addons       médias         appli au       scripts
  (modules)      & profils      (chargement,   système        1er plan       (Lua, bac
                                validation)                                   à sable)
```

- **Langage : Rust** (rapide, léger, Windows + Linux)
- **Interface : Tauri 2** — rendu web moderne (meilleur potentiel visuel), binaire léger, installeurs Windows et Linux inclus
- **Service séparé de l'interface** : fermer la fenêtre ne coupe pas le MasterHUB
- **Config en fichiers lisibles** (TOML) dans le dossier utilisateur
- **Installation** : `.exe` / `.msi` (Windows), `.deb` + AppImage (Linux), règles udev Linux fournies

## 7. Communication avec le matériel

Voir **`PROTOCOLE.md`** (v0.1, issue de l'analyse statique du logiciel officiel, 07/11/2026).

- **Transport : HID USB standard** — chaque module est un périphérique HID distinct (VID `0x2516`), aucun pilote spécial
- **Bibliothèque : crate Rust `hidapi`** (Windows + Linux) — `cooltek-sdk` n'est plus nécessaire
- Protocole documenté à ~80 % : luminosité, mode logiciel, images JPEG 72×72 sur les touches, couleurs, LED des faders / knobs / base, lecture des touches, faders et molettes
- Reste à confirmer en direct : tailles de rapports, position exacte des octets en entrée, plages des LED
- **Outil de test** en ligne de commande (`hubforge-probe`) : premier code du projet, sert à valider le protocole sur le vrai matériel

## 8. Exigences non fonctionnelles
- Démarrage du service < 2 s
- Reconnexion automatique (câble débranché, sortie de veille)
- Aucune perte de config en cas de plantage (écriture atomique + sauvegardes)
- Un addon défectueux ne fait **jamais** planter le logiciel (il est désactivé avec un message clair)
- Journaux consultables et exportables
- Code sur GitHub, avec README, guide de contribution et guide d'addons

## 9. Phases de développement (proposition)

| Phase | Contenu | Résultat visible |
|---|---|---|
| 0 | Protocole USB : analyse statique ✅ puis validation avec `hubforge-probe` | Lire les appuis, afficher une image test |
| 1 | Service + touches + profils + raccourcis / macros | Remplace l'officiel pour l'usage de base |
| 2 | Interface complète (glisser-déposer, éditeur d'icônes, réglages) | Logiciel agréable à configurer |
| 3 | Faders / molettes + audio par application | Mixage audio complet |
| 4 | **Système d'addons** (niveaux 1, 2, 3) + gestionnaire d'addons + addon d'exemple simple | Déposer un `.forge` = nouvelle catégorie |
| 5 | Linux finalisé, widgets, installeurs, guide IA, publication open source | **Socle v1.0 terminé** |
| ensuite | Addons métiers à préciser ensemble : Fusion 360, OBS, PrusaSlicer, SolidCraft, puis MSFS 2024 | — |

## 10. Questions ouvertes ❓
1. ~~Nom du logiciel~~ → **HubForge**, addons en `.forge` ✅
2. ~~Langage des scripts~~ → **Lua 5.4** ✅
3. ~~Licence~~ → **GPL-3.0** ✅ — les addons tiers choisissent librement leur licence (voir 4.11) ✅
4. ~~Mise à jour firmware~~ → laissée au logiciel officiel ✅
5. Catalogue communautaire d'addons : utile pour la v1 ou plus tard ?
6. ~~MSFS~~ → repoussé après la v1 ✅
7. Addon Fusion 360 : disposition à valider (après le socle)
8. ~~Autres slicers~~ → aucun, PrusaSlicer seulement ✅
9. Faders en mode Fusion : garder l'audio ou autre usage ? (après le socle)
