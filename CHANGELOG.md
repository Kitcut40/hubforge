# Historique des versions

Toutes les évolutions notables de HubForge. Les numéros suivent le principe
MAJEUR.MINEUR.CORRECTIF ; avant la 1.0, chaque version mineure peut changer
beaucoup de choses.

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
