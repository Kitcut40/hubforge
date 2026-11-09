# Historique des versions

Toutes les évolutions notables de HubForge. Les numéros suivent le principe
MAJEUR.MINEUR.CORRECTIF ; avant la 1.0, chaque version mineure peut changer
beaucoup de choses.

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
