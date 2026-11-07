# PROTOCOLE.md — Protocole USB du Cooler Master MasterHUB

> **Statut : v0.3 — analyse statique du logiciel officiel (v1.0.0.113) + deux tests réels** (`hubforge-probe` v0.1 et v0.2, 07/11/2026, PC de Benjamin, Windows).
> Légende : ✅ confirmé sur le matériel · 🔍 déduit du logiciel officiel, à confirmer.
> Ce document décrit uniquement le **fonctionnement** du protocole (interopérabilité). Aucun code, image ou ressource de Cooler Master n'est repris dans HubForge.

---

## 1. Vue d'ensemble

| Élément | Constat |
|---|---|
| Transport | ✅ **HID USB standard**, aucun pilote spécial (Windows : `hid.dll` ; Linux : hidraw) |
| Organisation | ✅ **Chaque module = un périphérique HID distinct**, avec son propre n° de série. La base est un hub USB + un module « LED de la base » |
| Interface | ✅ Une seule interface HID par module (interface 0, usage page `0x000C`, usage `0x0001`) |
| Lecture des contrôles | ✅ Le module **envoie lui-même** un rapport d'entrée à chaque événement : **pas besoin d'interroger en boucle** |
| Bibliothèque Rust | ✅ crate `hidapi` (backend C classique, le même que celui du logiciel officiel) |

---

## 2. Identification des modules

**Vendor ID Cooler Master : `0x2516`**

| Module | Code | PID (firmware de Benjamin) | PID alternatif | Nom USB | Statut |
|---|---|---|---|---|---|
| **15 touches à écran** | M4315 | ✅ `0x01FE` | `0x9997` | « 15xKey w/ IPS, 4x3 » | ✅ |
| **5 faders** | M005S | ✅ `0x0200` | `0x9991` | « 5xFader, 4x2 » | ✅ |
| **2 molettes** | M002 | ✅ `0x0204` | `0x9995` | « 2xRoller, 4x1 » | ✅ |
| **LED de la base** | M0x64 | ✅ `0x01F6` | `0x9999` | « Base, 4x6 » | ✅ |
| Encodeur à écran | M131 | `0x0206` | `0x9994` | — | 🔍 |
| 3 knobs | M003 | `0x0202` | `0x9996` | — | 🔍 |
| Pavé 4 touches | M3704 | — | `0x9989` | — | 🔍 |
| Écran | M4300 | — | `0x9998` | — | 🔍 |
| Module à écran | M4600 | — | `0x9993` | — | 🔍 |
| Barre à écran | M5000 | — | `0x9992` | — | 🔍 |

- Les PID courts (`0x01xx` / `0x02xx`) correspondent aux firmwares récents ; HubForge doit reconnaître **les deux séries**.
- Le suffixe du nom USB (« 4x3 », « 4x2 », « 4x1 », « 4x6 ») semble indiquer la **taille du module en unités de la base** 🔍 → utile pour dessiner la disposition.
- Le n° de série encode le type : `MHKD15…` (15 touches), `MHFD05…` (faders), `MHRR02…` (molettes), `MHBS01…` (base).

---

## 3. Rapports HID (lus dans les descripteurs ✅)

| Module | Entrée | Sortie | Feature |
|---|---|---|---|
| 15 touches | ID `0x01`, 64 o | ID `0x02`, **320 o** | ID `0x03` et `0x37`…`0x3E`, 32 o chacun |
| Faders | ID `0x01`, 64 o | ID `0x02`, 64 o | ID `0x03`, `0x2D`…`0x34`, `0x36`, 32 o |
| Molettes | ID `0x01`, 64 o | ID `0x02`, 64 o | ID `0x03`, `0x2D`…`0x31`, `0x36`, 32 o |
| Base | ID `0x01`, 64 o | ID `0x02`, 64 o | ID `0x03`, `0x2D`…`0x32`, `0x34`…`0x36`, 32 o |

(tailles Report ID inclus)

**Format des rapports d'entrée ✅ :**

| Octet | Contenu |
|---|---|
| 0 | `0x01` (Report ID) |
| 1 | Type d'événement : `0x34` touches · `0x2A` faders / molettes · `0x2B` base |
| 2–3 | Longueur des données utiles (16 bits, little-endian) |
| 4… | Données utiles |

---

## 4. Module 15 touches

### 4.1 Lecture des touches ✅
Données utiles (17 octets) : `02 0F T0 T1 … T14`
- `T` = `01` touche enfoncée, `00` relâchée.
- **Numérotation : de gauche à droite, puis de haut en bas** — touche 0 en haut à gauche, touche 14 en bas à droite.
- Un rapport à l'appui **et** un au relâchement.
- ✅ Les touches ne remontent au PC **qu'en mode logiciel** (voir 4.3). Lors du 1er test, le mode était déjà actif car laissé par le logiciel officiel.

### 4.2 Image sur une touche ✅ — rapport de sortie `0x02`, commande `0x36`
Paquets de **320 octets** :

| Octet | Contenu |
|---|---|
| 0 | `0x02` |
| 1 | `0x36` |
| 2–3 | N° du paquet (16 bits LE, à partir de 0) |
| 4–5 | Taille des données dans ce paquet (16 bits LE, max **312**) |
| 6 | N° de touche (0–14, même numérotation que 4.1) |
| 7 | `1` si dernier paquet, sinon `0` |
| 8… | Données |

- ✅ **JPEG 72 × 72 px**, affiché à l'endroit, sans rotation.
- ✅ ~1,5 à 2 Ko par image → 5 à 7 paquets ; les 15 touches se mettent à jour en **environ 0,5 s**.

### 4.3 Commandes courtes — **feature reports** 32 octets
⚠️ Ce sont des *feature reports*, pas des rapports de sortie (l'envoi en sortie est refusé par Windows ✅).

| Commande | Octets | Statut |
|---|---|---|
| Luminosité des écrans | `03 34 VV` (0–100) | ✅ |
| Couleur unie sur une touche | `03 37 KK RR GG BB` | ✅ |
| Mode logiciel | `03 3A 00/01` | ✅ |

**Mode logiciel ✅ :**
- `00` = **mode autonome** : le module affiche l'animation du logo MasterHUB et **n'envoie plus les appuis** au PC.
- `01` = **mode PC** : les écrans affichent les images envoyées et les appuis sont transmis.
- → HubForge envoie `01` au démarrage et `00` en quittant (le module retrouve son animation quand le logiciel est fermé).

### 4.4 Image plein écran — commande `0x35` 🔍
Même découpage que 4.2. Résolution inconnue, non testé.

---

## 5. Faders (M005S) ✅

### 5.1 Lecture en continu ✅
Données utiles (7 octets) : `02 05 F1 F2 F3 F4 F5`
- Valeur **0 (bas) à 100 (haut)** par fader, fader 1 = le plus à gauche.
- Un rapport à chaque variation (~ toutes les 50 ms en mouvement).

### 5.2 Lecture à la demande ✅ — feature report `0x33`
Réponse : `33 05 F1 F2 F3 F4 F5` (même échelle 0–100). Pratique pour connaître la position au démarrage.

---

## 6. Molettes (M002) ✅
Données utiles (4 octets) : `01 02 D G`
- `D` = molette droite, `G` = molette gauche 🔍 (ordre à vérifier visuellement).
- `FF` = pas de mouvement · `00` = un cran dans un sens · `01` = un cran dans l'autre.
- ✅ **Un rapport par cran** : il faut compter chaque rapport, même identique au précédent.

---

## 7. LED (famille M0 : faders, base, knobs) ✅

Rapport de **sortie** `0x02`, 64 octets :

| Commande | Octets | Statut |
|---|---|---|
| **Couleur** | `02 2D CC II RR GG BB SS` | ✅ rouge / vert / bleu OK sur faders et base |
| **Luminosité** | `02 2E CC II VV` | ✅ 20 = plus sombre, 100 = plus fort |
| Mode d'effet | `02 2C CC II MM SS` (MM 0–9) | 🔍 |
| Vitesse d'effet | `02 30 …` | 🔍 |

- `SS` = 1 pour **sauvegarder** dans le module, 0 pour un changement temporaire.
- Cible `CC` / index `II` : base `00 00` ✅ · faders `01 FF` (tous) ou `01 0–4` ✅ · knobs `02 FF` ou `02 0–2` 🔍.

---

## 8. Base
- Envoie en continu un rapport `01 2B 01` (≈ 1 par seconde) 🔍 signification inconnue (signal de vie ?).

---

## 9. Séquence de connexion pour HubForge
1. Énumérer les périphériques HID `0x2516` (les deux séries de PID).
2. Ouvrir chaque module par son chemin / n° de série.
3. Faders : lire le feature report `0x33` pour la position initiale.
4. 15 touches : **activer le mode logiciel** (`03 3A 01`), régler la luminosité, puis envoyer les 15 images JPEG 72×72.
5. À la fermeture : `03 3A 00` (retour à l'animation du logo).
6. Lire les rapports d'entrée en continu (bloquant, pas de boucle d'interrogation).
7. Débranchement : l'erreur de lecture signale le module retiré ; redétecter à l'arrivée d'un nouveau périphérique.
