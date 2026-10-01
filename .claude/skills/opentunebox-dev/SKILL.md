---
name: opentunebox-dev
description: Conventions, commandes et pièges du projet OpenTuneBox (analyse Blackbox Betaflight, clone de PIDtoolbox Pro). À charger avant de modifier le décodeur, les analyses (FFT, Wiener, filtres BF) ou l'interface Qt du projet, ou pour valider sur le corpus de logs.
---

# OpenTuneBox — développement

Projet : `~/Documents/Github/OpenTuneBox` (privé, sans licence). Utilisateur : Cédric, francophone,
débutant en programmation → expliquer en français, étape par étape.

## Références à lire selon la tâche
- `docs/spec-betaflight.md` : formules exactes du firmware (filtres, chaînes gyro/D, RPM, notch dynamique,
  tables debug par version, clés d'en-tête, curseurs simplifiés, unités). Source : clone `~/Documents/Github/betaflight`.
- `docs/spec-methodes.md` : méthodes d'analyse (Wiener, spectres, heatmaps, retards) + repères de réglage.
- `docs/spec-ecrans.md` : écrans PIDtoolbox à reproduire (captures dans `~/Documents/Drones/Logs/**/PTB_FIGS_*`).
- Plan approuvé : `~/.claude/plans/ticklish-crunching-rainbow.md`.

## Règles d'architecture
- `src/opentunebox/decoder/` : décodage binaire (numba, `cache=True`). `reader.decode_file()` → `DecodedSession` (valeurs brutes).
- `src/opentunebox/log/` : `FlightLog` (unités physiques), `settings.py` (en-tête normalisé par version), `debug_tables.py`.
- `src/opentunebox/analysis/` : calculs purs numpy/scipy, **jamais d'import de `gui/`**.
- `src/opentunebox/gui/` : PySide6 + pyqtgraph, n'affiche que ce que `analysis/` calcule.
- Interface et messages en français ; code et noms de variables en anglais ; commentaires en français.
- Ne jamais copier de code PIDtoolbox (binaire chiffré) ni d'autres projets : réimplémenter à partir des specs.

## Commandes
```sh
cd ~/Documents/Github/OpenTuneBox
.venv/bin/pytest -q                                  # tests unitaires
.venv/bin/pytest -q -m corpus                        # tests sur vrais logs (lents)
.venv/bin/python scripts/validate_corpus.py [DOSSIER] # décodeur vs blackbox_decode
```
Pas de compilateur C sur la machine ; pip du venv installé via get-pip. Python 3.14.

## Pièges connus
- Le `blackbox_decode` de PTB (`~/.local/share/PIDtoolboxProV094/`) compte mal `loopIteration` quand
  l'en-tête écrit `P interval:N` (BF récents) et jette les trames P qui suivent la 1re trame I d'un bloc.
  → comparer en alignant sur `time`, ignorer `loopIteration`. Notre décodeur suit le firmware (itération = boucle PID).
- Numéros `debug_mode` différents selon la version (FFT_FREQ = 17 en 4.x, 16 depuis 2025.12).
- 4.4/4.5 : `rollPID` D = D max et `d_min` = D de base ; 2025.12+ : l'inverse avec `d_max`.
- Corpus : `~/Documents/Drones/Logs/` (780 sessions, BF 4.4 → 2026.6). Fichiers `.part` = transferts incomplets.
- Validation corpus (2026-10-01) : 95 % des sessions identiques à la référence ; les écarts restants
  sont des trames corrompues que la référence accepte (trames tronquées avant un trou SD, trames S
  parasites, valeurs ±2³¹). `FlightLog` écarte les trames impossibles (`_drop_implausible`) et ignore
  les sessions corrompues à plus de 50 %.
- Fin de programme Qt : utiliser `opentunebox.gui.app.hard_exit(code)` dans les scripts (PySide6 6.11 +
  pyqtgraph segfaultent pendant le nettoyage automatique de sortie). Scripts de capture hors écran :
  `QT_QPA_PLATFORM=offscreen`, `widget.grab().save(...)`, puis regarder l'image.
- Simulateur de filtres : méthode relative `bf_filters.whatif_spectrum` (spectre loggé × |H_mod|²/|H_cur|²
  à la cadence PID, fenêtre par fenêtre) — exacte quand rien ne change. `simulate_gyro` (temporel, à la
  cadence du log) reste approximatif au-dessus de ~0,4 × fs du log.
- Retards de filtres : méthode de phase (10–100 Hz), pas l'intercorrélation (dominée par le bruit HF).
- Seuils de l'assistant = p90 du corpus par taille (docs/assistant.md, scripts/calibrate_advisor.py).
- Construire l'exécutable : `pyinstaller packaging/opentunebox.spec` (Linux : nécessite `binutils`). Windows :
  `packaging/construire_windows.bat` sur un PC Windows (Python 3.13 via winget) ; sources à copier :
  `git archive --format=zip --prefix=OpenTuneBox/ -o dist/OpenTuneBox-source.zip HEAD`. Alternative :
  `.github/workflows/construire.yml` (Windows/macOS/Linux) si le dépôt est poussé sur GitHub.
- Commits : auteur Cedric <desmetgerry@gmail.com>, terminer par `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
