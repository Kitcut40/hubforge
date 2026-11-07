# HubForge — Feuille de route vers la v1.0

> Rédigée le 7 novembre 2026, à partir de la v0.7.2.
> Objectif : une **v1.0 publiable**, qu'un inconnu peut télécharger, installer et utiliser sans aide, et qui fait mieux que le logiciel officiel sur ce qui compte.

---

## 1. Où on en est (v0.7.2)

### Ce qui marche, testé sur le matériel de Benjamin

| Domaine | État |
|---|---|
| Protocole USB (touches, faders, molettes, LED) | ✅ décodé et documenté (`PROTOCOLE.md`) |
| Détection des modules, branchement à chaud, position et rotation réelles sur la base | ✅ |
| Touches : texte, icônes (110 intégrées), images, **GIF animés**, icône et nom des applis récupérés | ✅ |
| Touches à 2 états, macros avec pauses, dossiers, pages, **pages automatiques par appli** | ✅ |
| **Profils** complets (touches + faders + molettes + LED), changement depuis le hub | ✅ |
| **Touches vivantes** : horloge (3 styles), date, CPU/RAM (barre, jauge, graphique), volume, micro | ✅ |
| Faders : volume général, micro, **volume par application**, couleur par fader | ✅ |
| Molettes : volume, défilement vertical/horizontal, zoom | ✅ |
| LED de la base : couleur, 9 effets, luminosité | ✅ |
| Interface : glisser-déposer partout, aperçu en direct, réorganisation des pages, suppression par glisser | ✅ |
| **Addons `.forge` niveau 1** (déclaratifs) : installation par glisser, rechargement à chaud, validation précise | ✅ |
| Addons : Fusion 360 (officiel), Discord, PrusaSlicer (vraies icônes) | ✅ |
| Démarrage avec Windows, zone de notification, instance unique, journal | ✅ |
| Easter egg tape-taupe | ✅ 🎮 |

### Ce qui n'est pas encore « produit »

- Livré en **2 fichiers à copier** (`hubforge.exe` + `WebView2Loader.dll`) : pas d'installateur, pas de désinstallation, pas de mise à jour, et Windows affiche un avertissement SmartScreen (programme non signé).
- **Windows uniquement**, **français uniquement**.
- Pas d'**annuler/rétablir** dans l'interface, pas de **sauvegarde automatique** des configurations, pas d'**export/import** de profil.
- Aucune intégration « vivante » avec une application : les touches ne savent pas si OBS enregistre ou si Discord est en muet.
- Modules **Encodeur** et **3 boutons rotatifs** non pris en charge (pas de matériel pour les tester).
- Le module 15 touches ne revient pas toujours seul quand on le rebranche à chaud (Windows ne le voit plus : piste matérielle/base).
- Pas de tests automatiques sur GitHub (CI), pas de guide utilisateur, pas de captures pour le README.

---

## 2. Ce que veulent les utilisateurs

Synthèse des tests de presse du MasterHUB, des forums et de Reddit (Stream Deck, OpenDeck, Deckboard…).

### 2.1 Les reproches faits au logiciel officiel du MasterHUB

| Reproche | Où | Réponse de HubForge |
|---|---|---|
| Logiciel resté longtemps « en bêta », bugs, mises à jour qui échouent | Tom's Hardware, APH Networks, GamingTrend, Profesional Review | Stabilité, journal, sauvegardes, CI → **à finir** (§ 3) |
| Peur que Cooler Master abandonne le produit | Tom's Hardware | **Open source** : le logiciel survit à la marque ✅ |
| Moins de fonctions prêtes à l'emploi que le Stream Deck | GamingTrend | Fonctions de base riches ✅ + addons ✅ ; **intégrations à ajouter** (OBS, musique…) |
| Peu d'applis prises en charge (OBS, Twitch, Streamlabs, Adobe…) | Fiche produit, Tom's Hardware | Addons faciles à écrire (même par une IA) ✅ |
| Ne fonctionne que si le logiciel tourne | APH Networks | Tourne discrètement en arrière-plan, démarre avec Windows ✅ |
| Module qui dépasse de la base : l'affichage « s'écrase » | GamingTrend | Positions hors grille **à gérer** |
| Méfiance envers le cloud, les comptes et les abonnements | Commentaires TechPowerUp | **100 % hors ligne, sans compte** ✅ |

### 2.2 Ce que demandent les utilisateurs de Stream Deck (et alternatives libres)

| Demande | Source | Priorité HubForge |
|---|---|---|
| **OBS** : changer de scène, enregistrer/streamer, couper une source, **avec retour d'état sur la touche** | Elgato (plugin n°1), avis Deckboard (« connexion automatique à OBS », « l'image change quand je coupe ») | 🔴 v1 |
| **Discord** : muet/sourdine fiables, push-to-talk, salons vocaux | Elgato, r/elgato (muet/sourdine « parmi les actions les plus utilisées ») | 🟠 v1 (raccourcis ✅, état en direct si possible) |
| **Musique** : lecture, titre en cours, pochette, volume | BarRaider (Spotify = plugin le plus populaire), Elgato | 🔴 v1 (sans compte, via Windows) |
| **Voicemeeter / mixage audio** avec retour | BarRaider, Elgato (Wave Link) | 🟠 v1 (idéal pour les faders) |
| **Multi-actions, bascules, profils par appli** | OpenDeck (« tout ce qu'on attend »), Reddit | ✅ déjà fait |
| **Linux** | r/linux_gaming (OpenDeck, StreamController) | 🟠 v1 |
| **Sauvegarder / partager ses configurations**, importer un profil avec ses plugins | Vortez, Stream Deck 6.9 | 🔴 v1 |
| **Appui long** (2 actions par touche) | SuperMacro, plugin Home Assistant | 🟢 v1 en option |
| Économiseur d'écran / image quand le PC est verrouillé | BarRaider (Screen Saver) | 🟢 v1 |
| Twitch, YouTube, Home Assistant, météo… | Elgato, BarRaider | ⚪ après v1, en addons |
| Catalogue de plugins | Elgato Marketplace, Cooler Master (« marketplace bientôt ») | ⚪ après v1 (catalogue GitHub) |

### 2.3 Ce que ça veut dire

Le MasterHUB a un matériel remarquable (modulaire, faders, écrans) mais un logiciel jugé en retard. **HubForge peut devenir « le logiciel que le MasterHUB méritait »** s'il apporte trois choses :

1. la **fiabilité** d'un produit fini (installateur, mises à jour, sauvegardes, zéro surprise) ;
2. les **intégrations vivantes** que tout le monde attend (OBS, musique, Discord, audio) ;
3. la **communauté** : addons faciles, partage de profils, Linux, open source.

---

## 3. Ce qu'il faut pour la v1.0

Légende : 🔴 bloquant pour la v1 · 🟠 fortement souhaité · 🟢 bonus si le temps le permet.

### 3.1 Installation et mises à jour 🔴

- [ ] **Installateur Windows** (Tauri/NSIS) : menu Démarrer, désinstallation propre, WebView2 installé si absent, `HubForge.exe` unique.
- [ ] **Association des fichiers `.forge`** : double-clic = installer l'addon.
- [ ] **Mises à jour** depuis GitHub Releases : vérification au démarrage **désactivable**, jamais automatique sans accord (le logiciel reste utilisable hors ligne).
- [ ] **Signature du programme** pour éviter l'alerte SmartScreen (piste gratuite pour l'open source : SignPath Foundation ; sinon expliquer l'alerte dans le README).
- [ ] Coexistence avec le logiciel officiel : détecter MasterHUB.exe et proposer de le fermer.

### 3.2 Fiabilité et sécurité des données 🔴

- [ ] **Sauvegardes automatiques** de chaque profil (10 dernières versions) + bouton « Restaurer une version ».
- [ ] **Annuler / Rétablir** (Ctrl+Z / Ctrl+Y) dans l'interface.
- [ ] **Exporter / importer un profil** en un fichier (`.hfprofile` : config + images + liste des addons utilisés, avec proposition d'installer ceux qui manquent).
- [ ] Copier / coller / dupliquer une touche ou une page.
- [ ] Module 15 touches au rebranchement : bouton **« Réinitialiser la base »** et message clair.
- [ ] Positions des modules **hors de la grille** (module qui dépasse) et dispositions encore inconnues : repli propre.
- [ ] Page « Diagnostic » : modules vus, ports, version, journal, bouton « Copier le rapport » pour les demandes d'aide.
- [ ] Mesure de la consommation au repos (objectif : < 1 % CPU, < 80 Mo de RAM) et correction si besoin.

### 3.3 Intégrations vivantes (addons niveau 2) 🔴

- [ ] **Niveau 2 des addons** : états dynamiques (une touche change d'image selon l'état de l'appli), permissions affichées à l'installation.
  - Choix à faire : **Lua 5.4 en bac à sable** (prévu dans les specs) ou **intégrations natives** dans HubForge exposées aux addons. Recommandation : commencer par des **intégrations natives** (OBS, médias) et une **API d'état** simple pour les addons, puis Lua après la v1.
- [ ] **OBS** (obs-websocket, inclus dans OBS) : scènes, enregistrement, stream, replay, sources, muets, **état en direct sur les touches**, faders = volumes des sources OBS. Connexion automatique.
- [ ] **Lecture en cours** (sans compte, via l'API média de Windows) : titre, artiste, **pochette sur une touche vivante**, lecture/pause/suivant. Fonctionne avec Spotify, YouTube, Deezer, VLC…
- [ ] **Discord** : état muet/sourdine en direct (selon ce que Discord permet sans compte développeur ; sinon garder les raccourcis et le documenter).
- [ ] 🟠 **Voicemeeter** : volumes et muets des bandes sur les faders, avec retour d'état.
- [ ] 🟢 Molettes et faders dans les addons (`type = "rotary"` des specs) : une molette = action « haut » / action « bas ».

### 3.4 Fonctions demandées 🟠

- [ ] **Appui long** en option par touche (l'action courte part au relâchement uniquement sur ces touches).
- [ ] **Répétition tant qu'on appuie** (volume, défilement).
- [ ] **Économiseur** : luminosité réduite après X minutes, écrans et LED éteints quand le PC est verrouillé ou en veille, image/horloge de veille.
- [ ] **Soundboard** : jouer un son (mp3/wav) sur une touche, choix de la sortie audio.
- [ ] 🟢 Minuteur / chronomètre en touche vivante.
- [ ] 🟢 Éditeur d'image de touche : taille et position du texte et de l'icône, police, emoji.
- [ ] 🟢 Bibliothèque d'icônes complète (≈ 1 500 icônes Lucide, licence ISC) avec recherche.
- [ ] 🟢 Favoris et actions récentes dans la bibliothèque.

### 3.5 Modules non testés 🟠

- [ ] **Encodeur à écran** et **3 boutons rotatifs** : décoder leur protocole (méthode du `hubforge-probe`), afficher et configurer. Nécessite le matériel ou un testeur volontaire ; sinon livrés comme « expérimentaux ».

### 3.6 Plateformes et langues 🟠

- [ ] **Linux** (Ubuntu/Fedora/Arch) : HID ✅ (hidapi), règles udev, volume via PipeWire/PulseAudio, appli au premier plan (X11 ; Wayland limité), paquets `.deb`/`.rpm`/AppImage ou Flatpak.
- [ ] **Anglais** en plus du français (interface, messages, addons officiels), langue choisie automatiquement.
- [ ] ⚪ macOS : hors v1 (matériel de test absent).

### 3.7 Expérience et finition 🔴

- [ ] **Premier lancement guidé** : détection des modules, choix d'un profil de départ (Bureau, Streaming, Création 3D), proposition des addons.
- [ ] États vides et messages d'erreur relus partout ; infobulles sur chaque réglage.
- [ ] Navigation au clavier et contrastes vérifiés (accessibilité de base).
- [ ] Icône, nom et écran « À propos » propres (licence, crédits, lien GitHub, mention « non affilié à Cooler Master »).

### 3.8 Projet open source 🔴

- [ ] **CI GitHub Actions** : compilation Windows + Linux, tests, `clippy`, publication automatique des versions.
- [ ] **README** avec captures et GIF, **guide utilisateur** (installation, premiers pas, FAQ, dépannage), **CHANGELOG**.
- [ ] `CONTRIBUTING.md`, modèles d'issues (bug, idée, nouveau module), code de conduite.
- [ ] Mention légale : HubForge n'est **pas affilié à Cooler Master** ; « MasterHUB » est une marque de Cooler Master ; aucun firmware modifié.
- [ ] Liste de vérification matérielle avant chaque version (branchements, rotation, faders, LED, veille, reconnexion).

---

## 4. Plan proposé

| Version | Thème | Contenu principal | Effort estimé* |
|---|---|---|---|
| **0.8** | Produit fiable | Installateur, `.forge` en double-clic, sauvegardes + restauration, annuler/rétablir, export/import de profil, copier/coller, page Diagnostic, « Réinitialiser la base » | 3–4 sessions |
| **0.9** | Intégrations vivantes | Niveau 2 (états), **OBS**, **lecture en cours**, Discord (état si possible), appui long, répétition, économiseur | 4–5 sessions |
| **0.10** | Ouverture | **Linux**, **anglais**, CI GitHub, mises à jour, premier lancement guidé, Voicemeeter, soundboard | 4–5 sessions |
| **0.11 RC** | Finition | Documentation complète, captures, relecture de l'interface, mesure des performances, correction des bugs remontés par les testeurs | 2–3 sessions |
| **1.0** | Sortie | Publication GitHub Release + annonce (Reddit r/CoolerMaster, r/streamdeck, forum Printables / Cooler Master) | — |

\* Une « session » = une longue séance de travail comme celles d'aujourd'hui. Estimation indicative.

### Pendant ce temps, côté Benjamin

- Tester chaque version sur le vrai matériel (et Fusion 360, Discord, PrusaSlicer).
- Si possible : trouver 2–3 **testeurs** avec d'autres dispositions de modules (encodeur, boutons rotatifs, modules tournés).
- Prendre les captures et la courte vidéo pour le README.

---

## 5. Définition de « v1.0 terminée »

La v1 sort quand **toutes** ces phrases sont vraies :

1. Un inconnu télécharge **un seul installateur**, l'installe, branche son MasterHUB et voit ses modules **en moins d'une minute**, sans lire de documentation.
2. On ne peut **pas perdre sa configuration** : sauvegardes automatiques, annuler, export.
3. **OBS** et la **musique en cours** fonctionnent avec retour d'état sur les touches.
4. Un addon s'écrit en suivant le guide (ou en le donnant à une IA) et s'installe par **double-clic**.
5. Ça fonctionne sous **Windows 10/11** et **Linux**, en **français** et en **anglais**.
6. Le logiciel tourne **une semaine sans redémarrage** et sans fuite de mémoire.
7. Le dépôt GitHub a une **CI verte**, un README avec captures, un guide et un CHANGELOG.

---

## Sources

- Tom's Hardware — *Cooler Master MasterHUB review: A modular stream deck with potential* : https://tomshardware.com/peripherals/docking-stations-hubs/cooler-master-masterhub-review
- GamingTrend — *Cooler Master MasterHUB review — The Voltron of stream decks* : https://gamingtrend.com/reviews/cooler-master-masterhub-review-the-voltron-of-stream-decks/
- APH Networks — *Cooler Master MasterHUB Review* : https://aphnetworks.com/reviews/cooler-master-masterhub/2
- Profesional Review — *Cooler Master MasterHUB Review* : https://www.profesionalreview.com/2024/09/12/cooler-master-masterhub-review/amp/
- TechPowerUp — annonce et commentaires : https://techpowerup.com/325254/cooler-master-launches-masterhub-control-module
- EB Games — fiche du MasterHUB (marketplace et SDK annoncés) : https://www.ebgames.com.au/product/pc/318719-cooler-master-masterhub-modular-controller-stream-kit
- Elgato — *Best Stream Deck Plugins for Streaming* : https://www.elgato.com/explorer/products/stream-deck/stream-deck-plugins-for-streaming/
- BarRaider — plugins Stream Deck les plus populaires : https://barraider.com/
- OBS Forum — avis sur Deckboard : https://obsproject.com/forum/resources/deckboard.704/reviews?order=vote_score
- r/elgato — mégafil Stream Deck 6.9 : https://lr.eu.psf.lt/r/elgato/comments/1jujscp/beta_megathread_stream_deck_69_download_missing
- r/linux_gaming — OpenDeck : https://reddit.birdcat.cafe/r/linux_gaming/comments/1i99add/opendeck_the_best_linux_software_for_the_elgato
- OpenDeck (GitHub) : https://github.com/nekename/OpenDeck
- Vortez — test du Stream Deck XL (sauvegarde des configurations) : https://www.vortez.net/articles_pages/elgato_stream_deck_xl_review,5.html
