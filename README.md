# OpenTuneBox

Boîte à outils d'analyse des logs **Blackbox Betaflight** pour régler les drones de toutes tailles,
du whoop au 10 pouces. Elle reprend les outils de PIDtoolbox Pro V0.94 avec un décodeur et des
calculs entièrement réécrits, et fonctionne **hors ligne** sous Linux, Windows et macOS.

![Visualiseur de log](docs/images/visualiseur.png)

## Outils

Au premier lancement, la page **Guide** propose un parcours de réglage à cocher, par drone : préparer le
drone, régler les filtres, puis les PID par séries (méthode de Brian White), puis les finitions. Les valeurs
s'adaptent à la fiche du drone ; les logs ouverts cochent ce qu'ils prouvent et disent quoi corriger avant
le prochain vol, curseur par curseur. Les logs des filtres et ceux des séries PID sont demandés et jugés à
part (fréquence d'enregistrement, protocole), et chaque série d'un drone est reconnue.

![Guide intégré](docs/images/guide.png)

La fenêtre principale est le **visualiseur de log** : gyro, consigne, termes P/I/D/F, moteurs, gaz, debug ;
période d'analyse (crash exclu automatiquement), mesure de temps et de fréquence entre deux points, lecteur,
manches, modes de vol. Chaque outil s'affiche par son bouton dans la barre d'outils, dans la même fenêtre (Fréq. × gaz et Fréq. × temps depuis
l'outil Spectre, Configuration depuis le panneau de droite du visualiseur).

| Bouton | Outil | Ce qu'il montre |
|---|---|---|
| Guide | Guide intégré | Parcours de réglage à cocher par drone (préparer, filtres, séries PID, finitions), adapté à la fiche du drone ; logs des filtres et des séries PID vérifiés à part (protocole, usages, séries, corrections dans le configurateur) |
| Spectre | Analyse spectrale | Spectres avant/après filtres, retards des filtres, bruit moteur par harmonique ; diagnostic du bruit gyro et D avec stratégie de filtrage (case à cocher) |
| Fréq. × gaz | Fréquence × gaz | Cartes de bruit en fonction des gaz (4 colonnes, préréglages type PIDtoolbox) |
| Fréq. × temps | Fréquence × temps | Spectrogrammes avec harmoniques moteur et notchs dynamiques |
| Indicielle | Réponse indicielle | Déconvolution de Wiener : pic, latence, temps de montée, bande passante, marge de phase… |
| Curseurs PID | Curseurs PID | Position des curseurs simplifiés retrouvée depuis les PID, simulation « et si » + commandes CLI |
| Filtres | Simulateur de filtres | Chaînes gyro et D exactes de la version Betaflight, Bode, retards, spectre filtré recalculé sur le vrai vol |
| Configuration | Infos de configuration | Réglages des logs côte à côte, différences surlignées |
| Assistant | Assistant de réglage | Diagnostic à règles, seuils calibrés par taille de drone, chaque conseil justifié par sa mesure ; retard consigne → gyro (série feedforward) |

Jusqu'à 10 logs comparés à la fois. Formats `.bbl`, `.bfl` (Betaflight 4.4 → 2026.x).

<p>
  <img src="docs/images/reponse-indicielle.png" width="49%" alt="Réponse indicielle">
  <img src="docs/images/assistant.png" width="49%" alt="Assistant de réglage">
</p>

## Installation

Dernière version : page [**Releases**](../../releases/latest), section *Assets*. Un seul fichier par
système, rien d'autre à installer (Python et les bibliothèques sont inclus).

| Système | Fichier | Installation |
|---|---|---|
| Windows 10 / 11 | `OpenTuneBox-Windows-Setup.exe` | double-clic, puis suivre l'assistant (menu Démarrer, Bureau, ouverture des `.bbl`) |
| macOS (puce Apple) | `OpenTuneBox-macOS.dmg` | ouvrir, puis glisser OpenTuneBox sur Applications |
| Linux (Ubuntu 22.04, Debian 12, Fedora 36… ou plus récent) | `OpenTuneBox-Linux.AppImage` | rendre le fichier exécutable, puis double-clic |

Le programme n'est pas signé : Windows et macOS affichent un avertissement au premier lancement (marche à
suivre dans le [guide](docs/guide-utilisateur.md#1-installer)).

## Ligne de commande (depuis les sources)

```sh
opentunebox info LOG.bbl                     # sessions, firmware, réglages principaux
opentunebox report LOG.bbl [--profil five]   # diagnostic (profils : whoop micro five seven large)
```

## Documentation

- [Guide d'utilisation](docs/guide-utilisateur.md) : bien enregistrer, vols de test, lecture de chaque outil, méthode de réglage
- [Architecture](docs/architecture.md) : organisation du code et décisions
- [Validation](docs/validation.md) : comparaison avec le décodeur et les mesures de PIDtoolbox
- [Assistant](docs/assistant.md) : règles et calibration des seuils
- Spécifications : [Betaflight](docs/spec-betaflight.md), [méthodes d'analyse](docs/spec-methodes.md), [écrans de référence](docs/spec-ecrans.md)
- [Journal des versions](CHANGELOG.md) · [Contribuer](CONTRIBUTING.md)

## Développement

Depuis les sources (Python 3.11 ou plus récent ; [Rust](https://rustup.rs) est requis sur la branche
`feat/rust-migration` : l'extension `opentunebox._native` est compilée par `pip install`) :

```sh
python3 -m venv .venv
.venv/bin/pip install -e '.[dev]'
.venv/bin/opentunebox-gui                 # interface graphique
sh packaging/linux/install_linux.sh       # Linux : compile l'interface Rust (cargo) et l'ajoute au menu des applications
.venv/bin/pytest -q                       # tests (ceux qui demandent de vrais logs sont ignorés sans corpus)
(cd rust && cargo test -p otb-decoder)    # tests du code Rust
OTB_LOGS=/chemin/logs .venv/bin/pytest -q -m corpus   # tests sur un dossier de vrais logs
```

Sous Windows, remplacer `.venv/bin/` par `.venv\Scripts\`.

Exécutable autonome fabriqué soi-même, sur le système visé : `pyinstaller packaging/opentunebox.spec --noconfirm`
(Linux : nécessite `binutils`) → `dist/OpenTuneBox/` (`dist/OpenTuneBox.app` sous macOS) ; sous Windows,
double-clic sur `packaging\windows\construire_windows.bat`. Les installateurs des trois systèmes sont
fabriqués par le workflow GitHub [`construire.yml`](.github/workflows/construire.yml) et publiés sur la page
Releases à chaque version ([Contribuer](CONTRIBUTING.md)).

Les consignes pour les agents IA (Claude Code, Codex, Copilot…) sont dans [AGENTS.md](AGENTS.md).

## Licence

Projet privé, tous droits réservés (voir [LICENSE](LICENSE)). OpenTuneBox ne contient aucun code de
PIDtoolbox ; il s'appuie sur les formules publiques du firmware Betaflight et des méthodes d'analyse publiées.
