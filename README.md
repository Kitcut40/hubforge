# HubForge

**Logiciel libre et hors ligne pour le Cooler Master MasterHUB.**

Le MasterHUB est un excellent contrôleur modulaire (touches à écran, faders, molettes…), mais son logiciel officiel n'est presque plus suivi. HubForge le remplace par un logiciel rapide, sans compte, sans connexion Internet, pour Windows et Linux — et extensible par de simples fichiers d'**addons** (`.forge`).

> 🚧 **Projet en cours de développement.** Rien n'est encore utilisable au quotidien.

## État d'avancement

| Phase | Contenu | État |
|---|---|---|
| 0 | Analyse et validation du protocole USB | ✅ terminée |
| 1 | Service d'arrière-plan : touches, profils, raccourcis, macros | 🔧 v0.1 en test |
| 2 | Interface de configuration | ⏳ |
| 3 | Faders, molettes, audio par application | ⏳ |
| 4 | Système d'addons `.forge` | ⏳ |
| 5 | Linux, installeurs, publication v1.0 | ⏳ |

## Modules pris en charge

| Module | Détection | Contrôles | Écrans / LED |
|---|---|---|---|
| 15 touches à écran | ✅ | ✅ | ✅ images JPEG, luminosité |
| 5 faders | ✅ | ✅ | ✅ LED |
| 2 molettes | ✅ | ✅ | — |
| LED de la base | ✅ | — | ✅ |
| Encodeur à écran, 3 knobs | identifiés | non testés | non testés |

## Documentation

- [`docs/SPECS.md`](docs/SPECS.md) — fonctionnalités, architecture et feuille de route
- [`docs/PROTOCOLE.md`](docs/PROTOCOLE.md) — protocole USB du MasterHUB (HID), vérifié sur le matériel

## Essayer HubForge (v0.1)

```sh
cargo run --release -p hubforge
```

Au premier lancement, HubForge crée `config.toml` (dans `%APPDATA%\HubForge` sous Windows, `~/.config/HubForge` sous Linux). Modifie-le et enregistre-le : les touches se mettent à jour toutes seules.

Déjà disponible : images, texte et **GIF animés** sur les touches, pages, raccourcis clavier, saisie de texte, lancement d'applis / fichiers / URL, contrôle multimédia, verrouillage, **volume général, micro et par application sur les faders**, **couleur par fader**, molettes (volume, défilement, zoom).

## Outil de test

`tools/hubforge-probe` : petit programme en ligne de commande qui détecte les modules et vérifie le protocole (aucune commande de firmware ni de réinitialisation).

```sh
cargo run --release -p hubforge-probe
```

## Licence

- HubForge est distribué sous licence **GPL-3.0-or-later** (voir [`LICENSE`](LICENSE)) : toute version modifiée et distribuée doit rester open source.
- Les **addons** qui utilisent uniquement l'interface publique d'addons peuvent avoir la licence de leur choix (voir [`ADDON-EXCEPTION.md`](ADDON-EXCEPTION.md)).

HubForge n'est ni affilié à Cooler Master, ni approuvé par Cooler Master. « Cooler Master » et « MasterHUB » sont des marques de leurs propriétaires respectifs. Le protocole a été documenté à des fins d'interopérabilité ; aucun code ni ressource du logiciel officiel n'est inclus dans ce dépôt.
