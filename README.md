# OpenTuneBox

Boîte à outils d'analyse des logs Blackbox Betaflight pour régler les drones de toutes tailles :
visualiseur de log, analyses spectrales (FFT, fréquence × gaz, fréquence × temps), réponse
indicielle, retards de filtres, simulateur de filtres, curseurs PID, statistiques et assistant
de réglage. Fonctionne sous Linux, Windows et macOS.

## Installation (développement)

```sh
cd ~/Documents/Github/OpenTuneBox
python3 -m venv .venv
.venv/bin/pip install -e '.[dev]'
```

## Utilisation

```sh
.venv/bin/opentunebox-gui            # interface graphique
.venv/bin/opentunebox info LOG.bbl   # résumé d'un log en ligne de commande
```

## Documentation

- `docs/spec-betaflight.md` : formules et formats Betaflight (filtres, debug, en-têtes, unités)
- `docs/spec-methodes.md` : méthodes d'analyse (FFT, Wiener, retards) et repères de réglage
- `docs/spec-ecrans.md` : écrans de référence (PIDtoolbox Pro V0.94)
