# HubForge

**Logiciel libre et hors ligne pour le Cooler Master MasterHUB.**

Le MasterHUB est un excellent contrôleur modulaire (touches à écran, faders, molettes…), mais son logiciel officiel n'est presque plus suivi. HubForge le remplace par un logiciel rapide, sans compte, sans connexion Internet, pour Windows et Linux — et extensible par de simples fichiers d'**addons** (`.forge`).

> 🚧 **Projet en cours de développement.** Rien n'est encore utilisable au quotidien.

## État d'avancement

> Feuille de route vers la v1.0 (analyse, attentes de la communauté, plan) : [docs/ROADMAP-V1.md](docs/ROADMAP-V1.md)

| Phase | Contenu | État |
|---|---|---|
| 0 | Analyse et validation du protocole USB | ✅ terminée |
| 1 | Service d'arrière-plan : touches, profils, raccourcis, macros | ✅ v0.1 validée sur le matériel |
| 2 | Interface de configuration | ✅ v0.2 → v0.3 (disposition réelle des modules, icônes, touches à 2 états) |
| 3 | Faders, molettes, audio par application | ✅ v0.2 |
| 3b | Noyau avancé : pages automatiques par application ✅ v0.4, macros ✅ v0.4.1, dossiers ✅ v0.4.2, touches vivantes ✅ v0.5.0 (styles v0.6.0), profils ✅ v0.6.0, appui long | 🔧 |
| 4 | Système d'addons `.forge` | 🔧 niveau 1 (déclaratif) ✅ v0.7.0 + addon officiel Fusion 360 — guide : [docs/ADDON_GUIDE.md](docs/ADDON_GUIDE.md) |
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

## Essayer HubForge

```sh
cargo run --release -p hubforge
```

HubForge s'installe dans la **zone de notification** (près de l'horloge) : clic sur l'icône pour ouvrir l'interface, clic droit pour « Démarrer avec Windows » ou « Quitter ». Fermer la fenêtre ne coupe pas le MasterHUB.

L'interface montre ton MasterHUB à l'écran : glisse une action sur une touche, clique sur une touche, un fader ou une molette pour la régler, dépose une image ou un GIF depuis l'Explorateur. Tout est appliqué en direct et enregistré automatiquement.

> Windows : l'exécutable a besoin de `WebView2Loader.dll` à côté de lui (fourni dans l'archive). Le moteur WebView2 est déjà présent sur Windows 10 et 11.

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
