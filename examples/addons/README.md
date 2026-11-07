# Addons d'exemple

Addons de niveau 1 (déclaratifs) pour tester et servir de modèles.
Format : [docs/ADDON_GUIDE.md](../../docs/ADDON_GUIDE.md).

| Addon | Contenu |
|---|---|
| [`discord/`](discord/) | Vocal (micro, sourdine, appels), navigation serveurs/salons, emojis, GIF, messages rapides — 24 actions, 2 pages |
| [`prusaslicer/`](prusaslicer/) | Objets (agencer, déplacer, pivoter, échelle, poser, couper, copies), découpe, G-code, carte SD, vues et onglets de réglages — 38 actions, 2 pages. **Vraies icônes de PrusaSlicer** (AGPL-3.0, donc addon sous AGPL-3.0) ; logo extrait du logiciel installé (`icon = "auto"`) |

## Fabriquer le fichier `.forge`

Compresser le dossier de l'addon en ZIP puis renommer en `.forge`.
Sous Windows : clic droit sur le dossier `discord` → *Compresser vers → Fichier ZIP*, puis renommer `discord.zip` en `discord.forge`.

Les tests de HubForge vérifient automatiquement que chaque addon de ce dossier est valide.
