# OpenTuneBox

Boîte à outils d'analyse des logs **Blackbox Betaflight** pour régler les drones de toutes tailles,
du whoop au 10 pouces. Elle reprend les outils de PIDtoolbox Pro V0.94 avec un décodeur et des
calculs entièrement réécrits, et fonctionne **hors ligne** sous Linux, Windows et macOS.

![Visualiseur de log](docs/images/visualiseur.png)

## Outils

| Outil | Ce qu'il montre |
|---|---|
| Visualiseur de log | Gyro, consigne, termes P/I/D/F, moteurs, gaz, debug ; période d'analyse (crash exclu automatiquement), lecteur, manches, modes de vol |
| Analyse spectrale | Spectres avant/après filtres, retards des filtres, bruit moteur par harmonique |
| Fréquence × gaz | Cartes de bruit en fonction des gaz (4 colonnes, préréglages type PIDtoolbox) |
| Fréquence × temps | Spectrogrammes avec harmoniques moteur et notchs dynamiques |
| Réponse indicielle | Déconvolution de Wiener : pic, latence, temps de montée, bande passante, marge de phase… |
| Curseurs PID | Position des curseurs simplifiés retrouvée depuis les PID, simulation « et si » + commandes CLI |
| Simulateur de filtres | Chaînes gyro et D exactes de la version Betaflight, Bode, retards, spectre filtré recalculé sur le vrai vol |
| Infos de configuration | Réglages des logs côte à côte, différences surlignées |
| Statistiques de vol | Manches, termes PID, moteurs, erreur PID |
| Assistant de réglage | Diagnostic à règles, seuils calibrés par taille de drone, chaque conseil justifié par sa mesure |

Jusqu'à 10 logs comparés à la fois. Formats `.bbl`, `.bfl` (Betaflight 4.4 → 2026.x).

<p>
  <img src="docs/images/reponse-indicielle.png" width="49%" alt="Réponse indicielle">
  <img src="docs/images/assistant.png" width="49%" alt="Assistant de réglage">
</p>

## Installation

### Télécharger (recommandé)

Dernière version : page [**Releases**](../../releases/latest), section *Assets*. Rien d'autre à installer
(Python et les bibliothèques sont inclus).

| Système | Fichier | Installation |
|---|---|---|
| Windows 10 / 11 | `OpenTuneBox-Windows-Setup.exe` | double-clic, puis suivre l'assistant (menu Démarrer, Bureau, ouverture des `.bbl`) |
| macOS (puce Apple) | `OpenTuneBox-macOS.dmg` | ouvrir, puis glisser OpenTuneBox sur Applications |
| Linux (Ubuntu 22.04 ou plus récent) | `OpenTuneBox-Linux.AppImage` | rendre le fichier exécutable, puis double-clic |

Le programme n'est pas signé : Windows et macOS affichent un avertissement au premier lancement (marche à
suivre dans le [guide](docs/guide-utilisateur.md#installer-une-version-construite-par-github)).

### Fabriquer soi-même la version autonome

| Système | Fabrication |
|---|---|
| Linux | `pyinstaller packaging/opentunebox.spec --noconfirm` (nécessite `binutils`) → `dist/OpenTuneBox/` |
| Windows | Double-clic sur `packaging\windows\construire_windows.bat` → `dist\OpenTuneBox\OpenTuneBox.exe` |
| macOS | `pyinstaller packaging/opentunebox.spec --noconfirm` → `dist/OpenTuneBox.app` |

Chaque version se fabrique sur son propre système. Le workflow GitHub
[`construire.yml`](.github/workflows/construire.yml) fabrique d'un coup les installateurs des trois systèmes
(onglet *Actions*) et les publie sur la page Releases pour chaque version (voir [Contribuer](CONTRIBUTING.md)).
Le dossier `dist/OpenTuneBox/` se partage **en entier**.

### Depuis les sources (Python 3.11 à 3.14)

```sh
python3 -m venv .venv
.venv/bin/pip install -e '.[dev]'
.venv/bin/opentunebox-gui                 # interface graphique
sh packaging/linux/install_linux.sh       # Linux : ajoute OpenTuneBox au menu des applications
```

Sous Windows, remplacer `.venv/bin/` par `.venv\Scripts\`.

## Ligne de commande

```sh
opentunebox info LOG.bbl                  # sessions, firmware, réglages principaux
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

```sh
.venv/bin/pytest -q                  # tests (ceux qui demandent de vrais logs sont ignorés sans corpus)
OTB_LOGS=/chemin/logs .venv/bin/pytest -q -m corpus   # tests sur un dossier de vrais logs
```

Les consignes pour les agents IA (Claude Code, Codex, Copilot…) sont dans [AGENTS.md](AGENTS.md).

## Licence

Projet privé, tous droits réservés (voir [LICENSE](LICENSE)). OpenTuneBox ne contient aucun code de
PIDtoolbox ; il s'appuie sur les formules publiques du firmware Betaflight et des méthodes d'analyse publiées.
