# doomgeneric dans HubForge

Ce dossier contient le moteur de Doom de [doomgeneric](https://github.com/ozkl/doomgeneric)
(commit `e257a09527062bfb986b6e5e0780839b6cbd5b71`), lui-même basé sur Chocolate Doom
et sur le code source de Doom publié par id Software. Il est distribué sous la
**GNU GPL, version 2 ou (à votre choix) toute version ultérieure** (voir `LICENSE`),
ce qui le rend compatible avec la GPL-3.0-or-later de HubForge.

Seuls les fichiers nécessaires ont été copiés (pas de son, pas de plateforme SDL/X11).

## Modifications faites pour HubForge
- `hubforge_platform.c` (nouveau) : relie le moteur à HubForge (image, temps, touches, état du joueur).
- `i_system.c` : `I_Quit` termine le processus (Doom tourne dans un processus à part).

Aucun fichier de données de Doom (WAD) n'est inclus. HubForge utilise
[Freedoom](https://freedoom.github.io/) (licence BSD) téléchargé à la demande,
ou un fichier WAD que vous possédez déjà.
