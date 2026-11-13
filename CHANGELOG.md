# Historique des versions

Toutes les évolutions notables de HubForge. Les numéros suivent le principe
MAJEUR.MINEUR.CORRECTIF ; avant la 1.0, chaque version mineure peut changer
beaucoup de choses.

## [0.20.0] — 2026-11-13

### Nouveautés
- **Nouvelle interface**, plus aérée et plus lisible :
  - **Barre du haut épurée** : le profil au centre, l'état du MasterHUB d'un coup d'œil (« MasterHUB connecté », clic = diagnostic), les réglages.
  - **Le MasterHUB au centre**, mis en valeur. Les onglets de pages sont juste au-dessus.
  - **Actions à gauche** : une recherche, des pastilles par famille (Clavier, Applis, Son, Vivantes, IA et web, Système, Addons), et une ligne d'explication sous les actions courantes.
  - **Le panneau de droite n'apparaît que quand il sert** : clic sur une touche, un fader, une molette ou une page. Il se ferme avec la croix ou Échap.
  - **Touche** : deux onglets, « Action » et « Apparence », avec l'aperçu en en-tête et Copier / Coller / Vider toujours en bas.
  - **Réglages en pleine page**, rangés par sections : Général, Apparence, Son, Météo et IA, Veille, Avancé, Mises à jour, À propos. Chaque réglage est expliqué en une ligne.

### Interne
- Le code de l'interface est découpé en fichiers par thème (bibliothèque, éditeurs, réglages, profils…) au lieu d'un seul fichier de 3 500 lignes.

## [0.19.0] — 2026-11-13

### Nouveautés
- **Assistant de premier lancement** : au tout premier démarrage, HubForge montre les modules branchés, puis demande en quatre étapes :
  - le mode d'affichage (simple ou avancé) ;
  - le style des touches, avec un aperçu en direct ;
  - un profil de départ prêt à l'emploi : **Bureau**, **Streaming et Discord**, **Jeu**, **Partir de zéro**, ou garder son profil actuel.
  - Le profil de départ remplit les 15 touches et règle les faders. Il est créé à côté du profil actuel, sans le modifier.
  - L'assistant se relance quand on veut : Réglages › Relancer l'assistant de démarrage.
- **Modes simple et avancé** (Réglages › Affichage) :
  - le mode **simple** garde l'essentiel et cache les macros, variables, conditions, requêtes web, touches maintenues, l'appui long et double appui, et les réglages fins (faders, variables, délais des touches) ;
  - le mode **avancé** montre tout, comme avant ;
  - une touche qui utilise déjà une fonction avancée reste modifiable dans les deux modes.
  - Les nouveaux utilisateurs démarrent en simple ; ceux qui avaient déjà HubForge restent en avancé.

## [0.18.0] — 2026-11-13

### Nouveautés
- **IA sur le texte sélectionné** (nouvelle catégorie « Connecteurs ») : sélectionne un texte dans n'importe quelle appli, appuie sur la touche, et il est corrigé, traduit en anglais ou en français, résumé, passé en style professionnel, ou transformé selon ta propre consigne.
  - Le résultat remplace la sélection, est copié dans le presse-papiers, ou va dans une variable à afficher sur une touche. Ton ancien presse-papiers est remis ensuite.
  - Fonctionne avec **Ollama** (gratuit, sur ton PC), **OpenAI** et les services compatibles (Mistral, Groq, OpenRouter, LM Studio…), ou **Claude d'Anthropic**. Réglages › Intelligence artificielle, avec un bouton « Essayer ».
  - La clé API reste sur ton PC : elle n'est jamais incluse dans un profil exporté.
- **Requête web** : GET, POST, PUT, PATCH ou DELETE vers n'importe quelle adresse (domotique, webhook Discord, serveur local…).
  - Corps et en-têtes libres ; `{variables}` acceptées partout.
  - La réponse, ou un seul champ d'une réponse JSON, peut aller dans une variable affichée sur une touche.
  - Bouton « Essayer » dans l'éditeur.
- **Liseré d'état** : pendant une requête ou un travail de l'IA, la touche s'entoure d'orange, puis de vert si tout s'est bien passé, de rouge en cas d'erreur.
- Nouvelles icônes : correction, traduction, document, porte-documents, baguette magique, webhook, robot, avion en papier.

## [0.17.0] — 2026-11-13

### Nouveautés
- **Import des profils Stream Deck** (`.streamDeckProfile`) : menu des profils › Importer, ou glisser le fichier sur la fenêtre.
  - Les actions de base sont converties : raccourci clavier, texte, ouvrir une application ou un fichier, site web, multimédia, soundboard (sons copiés dans HubForge, avec leur volume et leur mode), multi-actions (avec les pauses), dossiers, retour, page suivante ou précédente.
  - Les images et les titres des touches sont repris.
  - Les touches d'un plugin Stream Deck sans équivalent arrivent avec leur image, sans action, sur fond gris ; le compte rendu liste ces plugins.
  - Profils pour Stream Deck XL ou + : les touches en trop vont sur les places libres, puis sur des pages supplémentaires.
  - Les profils récents et l'ancien format sont reconnus.
- **Packs d'icônes Stream Deck** (`.streamDeckIconPack`) : importés de la même façon, ils apparaissent dans « Icônes… » de chaque touche, avec une recherche par nom. Un pack se retire en un clic.

## [0.16.0] — 2026-11-13

### Nouveautés
- **Changer de sortie audio d'un appui** (casque ↔ enceintes) :
  - choisis une sortie, ou plusieurs pour passer de l'une à l'autre ;
  - la touche affiche la sortie active, avec une icône casque ou haut-parleur.
- **Musique en cours** : titre, artiste et pochette de ce que joue Windows (Spotify, YouTube dans le navigateur, VLC…), en style « pochette » ou « titre ». Un appui met en lecture ou en pause.
- **Minuteurs** :
  - chronomètre, et compte à rebours avec un anneau qui se vide ;
  - un appui démarre ou met en pause, un appui long remet à zéro ;
  - à la fin du compte à rebours, la touche clignote en rouge.
- **Variables pour OBS** : chaque variable peut être écrite dans un fichier texte, pour afficher un compteur sur ton stream.
- **Fenêtres** (nouvelle catégorie) : fenêtre à gauche, à droite, agrandir, vers l'autre écran, bureaux virtuels, vue des tâches.
- **Veille du MasterHUB** : écrans et LED s'éteignent après un temps sans activité, ou quand le PC est verrouillé. Le premier appui rallume sans lancer d'action.
- **Nouvelles touches vivantes** : débits réseau (texte ou graphique) et espace disque.

## [0.15.0] — 2026-11-10

### Nouveautés
- **Styles de touches** : cinq styles dessinés pour le MasterHUB.
  - **Graphite** : sobre, fond anthracite, icône pleine dans sa couleur. C'est le style par défaut.
  - **Néon** : halo et contour lumineux.
  - **Plan** : papier de plan bleu, traits fins.
  - **Console** : bouton rétroéclairé façon cockpit.
  - **Mono** : traits blancs sur noir.
  - **Classique** : l'ancien rendu, toujours disponible.
- **Un style pour tout le profil, une page ou une seule touche**, avec les boutons « Appliquer à toute la page » et « À tout le profil ». La couleur de la touche devient la couleur d'accent du style. Tes images et GIF restent tels quels.
- **Touches vivantes redessinées** dans chaque style : CPU, RAM, GPU, volume, micro, horloge, date, météo.
- **Nouvelles icônes pleines** (Phosphor), avec les logos de Discord, Spotify, Steam, Twitch, YouTube et Chrome pour les touches de volume d'application.
- **16 touches animées intégrées** (nouvelle catégorie « Animations ») : En direct, REC, On Air, égaliseur, onde sonore, chargement, battement de cœur, radar, notification, arc-en-ciel, scintillements, flamme, fusée, pause café, pluie, pluie de code.

### Corrections
- Une application ouverte mais silencieuse affiche « Aucun son » au lieu de « Fermée ». Noms d'applis plus lisibles (Edge, Chrome, OBS…).
- Les pastilles « 2 états », « Live », « Macro » et « GIF » ne cachent plus le haut des touches dans le logiciel.

## [0.14.0] — 2026-11-09

### Nouveautés
- **Touches vivantes météo** :
  - trois styles : maintenant (ville, icône, température, max/min), compacte, et prochains jours ;
  - fond qui suit le temps (ciel bleu, nuit, gris, pluie…) ;
  - ta ville se règle dans Réglages → Météo ; les prévisions viennent d'Open-Meteo, gratuites et sans compte ;
  - HubForge ne se connecte à internet que si une touche météo existe.
- **Carte graphique** : utilisation, température et mémoire vidéo (VRAM), en barre, jauge ou graphique. Pour les cartes NVIDIA, lu directement par le pilote, rien à installer.
- **Volume d'une application sur une touche** : affiche en direct le volume de Discord, Spotify… (« Fermée » si l'appli ne tourne pas, « Muet » si coupée). Un appui coupe ou rétablit son son.
- Nouvelle action « Couper le son d'une application ».
- Bibliothèque : 6 nouvelles touches vivantes.

## [0.13.0] — 2026-11-09

### Nouveautés
- **Molettes libres** : une molette peut lancer n'importe quelle action à chaque cran, une pour chaque sens (timeline, changement d'outil, page suivante, compteur…), en réglage commun ou propre à une page. Deux nouveaux modes aussi : **volume du micro** et **volume d'une application** (Discord, Spotify…).
- **Macros intelligentes** :
  - action **Variable** : compteur (+1, −1…), interrupteur 0/1 ou valeur libre, gardés même après un redémarrage ;
  - écris `{nom}` dans le texte d'une touche pour y afficher la valeur, mise à jour en direct ;
  - action **Condition** (si… alors… sinon) sur une variable, l'application au premier plan, la page, le profil, l'heure ou le jour ;
  - `{variable}`, `{app}`, `{hour}`… marchent aussi dans « Taper un texte » ;
  - bibliothèque : nouvelle catégorie « Macros intelligentes » (compteur, interrupteur, condition) ;
  - réglages : liste des variables et bouton pour les remettre à zéro.
- **Reprise douce des faders** : en changeant de page ou de profil, un fader qui contrôle autre chose n'agit qu'une fois passé par le volume actuel. Fini les sauts de volume. Sa LED pulse doucement en attendant ; le réglage se coupe dans les réglages.

### Corrections
- Les actions interdites dans un addon sont aussi refusées quand elles sont cachées dans une macro.

## [0.12.0] — 2026-11-09

### Nouveautés
- **Action « Maintenir des touches »** : les touches du PC restent enfoncées tant que tu tiens celle du MasterHUB (klaxon, accélérateur, appuyer-pour-parler, retour en arrière dans BeamNG…). Elles sont relâchées toutes seules si la page change ou si le module est débranché.
- **Soundboard** :
  - action « Jouer un son » (mp3, wav, ogg, flac, m4a) avec son volume et ce que fait un nouvel appui pendant la lecture (arrêter, recommencer ou rejouer par-dessus) ;
  - action « Arrêter tous les sons » ;
  - glisse un fichier son sur une touche pour la créer directement ;
  - réglages : volume général des sons, sortie audio, et **seconde sortie** (câble audio virtuel) pour que les sons partent aussi dans Discord ou sur le stream ;
  - les sons sont copiés dans le dossier de HubForge et inclus dans les profils exportés.

### Addons
- BeamNG.drive 1.1.0 : klaxon et frein à main maintenus, « Réparer » maintenu remonte le temps.

## [0.11.1] — 2026-11-09

### Corrections
- Raccourcis clavier : la touche **Inser** est reconnue (`insert`), et les touches de ponctuation (`,` `.` `/`…) sont envoyées comme de vraies touches du clavier, pour que les jeux les reconnaissent.
- L'outil d'exploration des zones de lumière de la base est retiré : la base n'a qu'une seule zone pilotable.

### Addons
- Nouveau dossier `addons/` du dépôt : addons à télécharger (non livrés avec le logiciel). Premier addon : **BeamNG.drive**.

## [0.11.0] — 2026-11-09

### Nouveautés
- **Appui long et double appui** : chaque touche peut avoir une action en plus pour l'appui long et une pour le double appui. Durées réglables dans les réglages. Le double appui ne retarde que les touches qui l'utilisent.
- **Faders et molettes par page** : chaque fader et chaque molette a un réglage commun à toutes les pages, et chaque page peut avoir le sien. Un point orange sur le dessin indique les réglages propres à la page.
- **Chaque fader a sa propre LED** : couleur fixe, couleur selon la position (dégradé propre au fader), animation, **lumière d'état** (rouge si le son est coupé, éteinte si l'appli n'est pas lancée) ou **VU-mètre** (s'allume avec le niveau sonore de ce que le fader contrôle).

### Changements
- Les réactions à Assetto Corsa sont retirées du logiciel : elles reviendront sous forme d'addon « jeux de course ».
- Plus aucun addon n'est livré avec HubForge (Fusion 360, Discord et PrusaSlicer retirés) : ils seront proposés dans un catalogue à télécharger.
- L'effet intégré « stroboscope » de la base, qui clignotait mal, est retiré : l'animation « Stroboscope » le remplace.
- Les anciens réglages de LED des faders (communs aux 5) sont repris automatiquement fader par fader.

## [0.10.0] — 2026-11-08

### Easter egg
- **Doom tourne sur le MasterHUB.** Tape `IDDQD` dans la fenêtre de HubForge : le jeu s'affiche sur les 15 touches du MasterHUB (chaque touche montre un morceau de l'image) ; la fenêtre ne montre que les commandes. Les touches servent de manette, la molette gauche vise, la molette droite fait avancer, les LED des faders montrent ta vie et la base prend la couleur de l'action (rouge quand tu es touché). Appui long sur la touche en haut à gauche pour quitter.
- Le moteur est [doomgeneric](https://github.com/ozkl/doomgeneric) (GPL-2.0 ou ultérieure). Les niveaux ne sont pas inclus : HubForge propose de télécharger [Freedoom](https://freedoom.github.io/) (libre et gratuit), ou d'utiliser ton DOOM.WAD / DOOM2.WAD.

## [0.9.0] — 2026-11-08

### Nouveautés
- **Animations des LED** : 9 modèles prêts à l'emploi (respiration, arc-en-ciel, vague, comète, battement de cœur, gyrophare, feu de camp, remplissage, stroboscope) avec leurs couleurs et leur vitesse, ou des animations faites à la main étape par étape (couleur de la base et de chaque fader, durée, fondu). Bouton « Essayer » pour les voir en direct sur le MasterHUB.
- **Faders qui changent de couleur selon leur position** : dégradé réglable (par exemple vert en bas, jaune au milieu, rouge en haut).
- **Animations sur les faders** : les LED des faders peuvent maintenant être animées, et plus seulement fixes.
- **Jeux de course : Assetto Corsa** : les faders deviennent des témoins de régime qui clignotent au rupteur, la base clignote en rouge au rupteur et prend la couleur des drapeaux (bleu, jaune, blanc, damier, pénalité). HubForge lit seulement les données publiées par le jeu.
- **Ranger une touche dans un dossier** : glisse-la sur la touche du dossier.
- **Orientation de la base** : verticale, horizontale vers la droite ou la gauche, ou à l'envers. Les images des touches restent lisibles.
- **Position des modules** : si un branchement n'est pas encore reconnu, tu peux indiquer où est le module dans les réglages de la base.

## [0.8.1] — 2026-11-07

### Nouveautés
- **Annuler / Rétablir** : Ctrl+Z et Ctrl+Y (ou les flèches en haut à droite) annulent n'importe quelle modification : touche, page, fader, LED…
- **Sauvegardes automatiques** : avant chaque modification, HubForge garde une copie du profil (les 30 dernières). Menu du profil → « Sauvegardes… » pour revenir à une version précédente en un clic.
- **Partage de profils** : menu du profil → « Exporter dans un fichier » crée un `.hfprofile` qui contient les touches, les pages et les images. Pour l'importer : double-clic sur le fichier, glisser-déposer sur la fenêtre, ou « Importer un profil… ». Les addons manquants sont signalés.
- **Copier / coller une touche** : Ctrl+C sur une touche, Ctrl+V sur une autre (ou les boutons de l'inspecteur).
- **Dupliquer une page** depuis ses réglages.
- **Diagnostic** (Réglages) : modules détectés, ports, addons et journal ; boutons « Reconnecter les modules », « Réinitialiser la base » (utile si un module rebranché à chaud n'est plus vu) et « Copier le rapport » pour demander de l'aide.

## [0.8.0] — 2026-11-07

### Nouveautés
- **Installateur Windows** : HubForge s'installe comme un vrai logiciel (menu Démarrer, raccourci sur le Bureau en option, démarrage avec Windows en option, désinstallation propre). Pas besoin des droits administrateur.
- **Mises à jour depuis GitHub** : HubForge vérifie discrètement s'il existe une nouvelle version (une fois par jour, désactivable). Les nouveautés s'affichent, et un clic télécharge, vérifie puis installe la mise à jour ; HubForge redémarre tout seul.
- **Double-clic sur un fichier `.forge`** : installe l'addon directement.
- **Nouveau logo** et nouvelles icônes (programme, zone de notification, installateur).
- Section **À propos** dans les réglages (version, licence, liens), et fenêtre des nouveautés après chaque mise à jour.

### Améliorations
- L'interface reprend le nouveau logo et une barre du haut plus lisible.

## [0.7.2] — 2026-11-07
- Addon PrusaSlicer avec les vraies icônes de PrusaSlicer (licence AGPL-3.0).
- Icône d'addon « auto » : extraite du logiciel installé sur le PC.

## [0.7.1] — 2026-11-07
- Bibliothèque en dossiers : un dossier par addon, avec sous-dossiers.
- Addons d'exemple Discord et PrusaSlicer.

## [0.7.0] — 2026-11-07
- Système d'addons `.forge` (niveau 1, sans code) et addon officiel Fusion 360.

## [0.6.0] — 2026-11-07
- Profils (configurations complètes), styles des touches vivantes, panneau de la LED de la base, réorganisation des pages par glisser, suppression d'une touche en la glissant hors du hub, mini-jeu caché.

## [0.5.0] — 2026-11-07
- Touches vivantes : horloge, date, processeur, mémoire, volume, micro.

## [0.4.0 – 0.4.2] — 2026-11-07
- Pages automatiques selon l'application au premier plan, nom des applications récupéré, macros, dossiers.

## [0.3.0] — 2026-11-07
- Disposition réelle des modules, icônes intégrées, icône des applications, touches à 2 états, LED réparées.

## [0.2.0] — 2026-11-07
- Interface graphique complète.

## [0.1.0] — 2026-11-07
- Premier service : touches, pages, raccourcis, faders, molettes.
