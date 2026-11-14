# Addon « Imprimante Prusa » pour HubForge

L'écran de ton imprimante Prusa sur le MasterHUB, dans le style de son
interface : aperçu de la pièce sur 4 touches, barre d'avancement sur une
rangée, temps restant, heure de fin, températures de la buse et du plateau,
pause, arrêt, ré-impression et préchauffage des filaments du menu Prusa.

Le programme parle à l'imprimante par **PrusaLink**, sur le réseau local
(rien ne passe par Internet).

## Installation

1. Zippe ce dossier et renomme l'archive en `prusa.forge` (ou prends celle
   qui est fournie), puis glisse-la sur la fenêtre de HubForge.
2. Dans la fiche de l'addon : **Autoriser ce programme**, puis renseigne
   l'adresse IP de l'imprimante et le mot de passe PrusaLink (sur
   l'imprimante : Réglages › Réseau › PrusaLink, utilisateur `maker`).
3. **Ajouter ses pages au profil** : trois pages arrivent (« Imprimante »,
   et les dossiers « Menu » et « Préchauffage »).
4. Dans Réglages › Apparence › *Zones sur plusieurs touches*, affiche la mire
   et règle l'écart pour que l'aperçu et la barre tombent juste d'une touche
   à l'autre.

## Pages

| Page | Touches |
|---|---|
| Imprimante | aperçu de la pièce (1, 2, 6, 7), buse, plateau, Menu, temps restant, heure de fin, pause, % et barre (11 à 15) |
| Menu | Retour, pause, arrêter (double appui), relancer (double appui), PrusaLink, préchauffage, refroidir, actualiser, buse, plateau, % et barre |
| Préchauffage | PLA, PETG, ASA, PC, PVB, ABS, HIPS, PP, FLEX, PA, perso, Retour, buse, plateau, refroidir |

Le préchauffage envoie un mini fichier G-code (`hubforge-preheat.gcode`) et
le lance : PrusaLink n'a pas de commande « régler la température ».

## Licences

- Code et manifeste : MIT.
- `icons/prusa/` : icônes de l'interface du firmware Prusa
  (Prusa-Firmware-Buddy), © Prusa Research a.s., licence
  **CC BY-NC-SA 4.0** : usage non commercial, attribution, partage à
  l'identique. Voir `icons/prusa/SOURCE.md`.
- `fonts/` : DejaVu Sans Mono (police de l'écran des imprimantes Prusa),
  licence libre DejaVu / Bitstream Vera, voir `fonts/LICENSE-DejaVu.txt`.

« Prusa » et « Original Prusa » sont des marques de Prusa Research a.s. Cet
addon n'est ni affilié à Prusa Research, ni approuvé par Prusa Research.
