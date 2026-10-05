---
name: opentunebox-dev
description: Procédures du projet OpenTuneBox (analyse Blackbox Betaflight, clone de PIDtoolbox Pro) — valider le décodeur et les analyses sur le corpus de vrais logs, recouper avec PIDtoolbox, ajouter un outil à l'interface, faire des captures hors écran, calibrer l'assistant, fabriquer les exécutables. À charger avant de modifier le décodeur, les analyses (FFT, Wiener, filtres BF) ou l'interface Qt.
---

# OpenTuneBox — procédures

Les règles générales (organisation, décisions, commandes, pièges) sont dans `AGENTS.md` à la racine :
le lire d'abord. Cette compétence détaille les procédures récurrentes.

## Où trouver quoi

- Formule ou réglage du firmware → `docs/spec-betaflight.md` (sources : dépôts `betaflight` et
  `betaflight-configurator` clonés à côté de ce projet, dans `../`).
- Méthode d'analyse → `docs/spec-methodes.md` ; écran PIDtoolbox à reproduire → `docs/spec-ecrans.md`.
- Valeurs de référence déjà établies → `docs/validation.md` ; seuils de l'assistant → `docs/assistant.md`.

## Valider une modification du décodeur

```sh
.venv/bin/python scripts/validate_corpus.py "$OTB_LOGS" --limit 50     # échantillon rapide
.venv/bin/python scripts/validate_corpus.py "$OTB_LOGS" --start 300    # reprise après un blocage
```
Statuts : `ok`, `gap` (trames tronquées avant un trou SD, tolérées), `slow` (trames S corrompues acceptées
par la référence), `rows` / `values` (à examiner), `refhang` (référence bloquée, délai 180 s).
Référence : `blackbox_decode` de PIDtoolbox (`OTB_REF_DECODER`, défaut `~/.local/share/PIDtoolboxProV094/`).
Objectif : ne pas faire baisser la proportion de sessions `ok` (95 %).

## Recouper une analyse avec PIDtoolbox

1. Prendre un log du corpus dont le dossier contient des captures `PTB_FIGS_*` (ex. `1. PTB/Allykill`,
   `1. PTB/BlackSummer`, `Cedric/JeNo 5`).
2. Lire la valeur sur la capture (ouvrir l'image), calculer la même avec `analysis/` sur la même période.
3. Ajouter la mesure à `docs/validation.md` si elle est nouvelle.

## Ajouter un outil à l'interface

1. Le calcul dans `analysis/`, avec un test synthétique dans `tests/`.
2. Une classe `ToolWindow` dans `gui/tools/<nom>.py` : `build_controls`, `snapshot_options`, `compute`, `render`.
3. L'enregistrer dans `gui/tools/__init__.py:REGISTRY` et ajouter une ligne à `main_window.py:TOOLS`
   (libellé court, icône Lucide copiée telle quelle dans `gui/assets/icons/`, info-bulle).
4. Test hors écran (modèle : `tests/test_gui_step.py`), capture et vérification visuelle.
5. Section dans `docs/guide-utilisateur.md`, ligne dans `CHANGELOG.md`.

## Captures hors écran

Script dans le répertoire temporaire de la session, lancé avec `QT_QPA_PLATFORM=offscreen` :
créer `MainWindow`, `load_paths([...])`, pomper les événements (`app.processEvents()` en boucle) jusqu'à
`win._loading == 0`, `open_tool(clé)`, attendre `not w._busy and w._last_result is not None`,
`w.grab().save(...)`, puis **`hard_exit(0, wait_all)`**. Toujours regarder l'image produite.

## Calibrer l'assistant

`scripts/calibrate_advisor.py 600 stats.json`, puis recalculer p10/p50/p90 par profil et mettre à jour
`advisor/profiles.py` et le tableau de `docs/assistant.md`.

## Fabriquer les exécutables

- Linux : `.venv/bin/pyinstaller packaging/opentunebox.spec --noconfirm` (≈ 30 s, `dist/OpenTuneBox/` ≈ 460 Mo).
  Pas d'option `strip` : celui d'Ubuntu 22.04 abîme `libscipy_openblas` de numpy (« not page-aligned »).
  Vérifier après tout changement de la recette : exécutable d'essai (même recette, script qui lance
  `opentunebox report` avec un `NUMBA_CACHE_DIR` vide puis ouvre chaque outil hors écran).
- Windows : sources par `git archive --format=zip --prefix=OpenTuneBox/ -o dist/OpenTuneBox-source.zip HEAD`,
  puis `packaging\windows\construire_windows.bat` sur le PC Windows (Python 3.13 installé par winget).
- Tous systèmes : workflow `construire.yml` (manuel, ou étiquette `v*` → page Releases ; préparer la version
  avec `scripts/release.py prepare X.Y.Z`, voir `CONTRIBUTING.md`) : une archive par système
  (installateur Inno Setup Windows, `.dmg` macOS, AppImage Linux construite sur Ubuntu 22.04),
  chacun essayé au démarrage. AppImage en local : télécharger appimagetool 1.9.1 dans le répertoire temporaire,
  puis `APPIMAGETOOL=… sh packaging/linux/build_appimage.sh dist/OpenTuneBox SORTIE.AppImage`.
  Icônes `.icns` / `.png` : `python packaging/make_icons.py` si le SVG change.
