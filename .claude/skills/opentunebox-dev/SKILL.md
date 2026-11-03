---
name: opentunebox-dev
description: Procédures du projet OpenTuneBox (analyse Blackbox Betaflight, clone de PIDtoolbox Pro) — valider le décodeur et les analyses sur le corpus de vrais logs, recouper avec PIDtoolbox, ajouter un outil à l'interface, faire des captures hors écran, calibrer l'assistant, fabriquer les exécutables. À charger avant de modifier le décodeur, les analyses (FFT, Wiener, filtres BF) ou l'interface Qt.
---

# OpenTuneBox — procédures

Les règles générales (organisation, décisions, commandes, pièges) sont dans `AGENTS.md` à la racine :
le lire d'abord. Cette compétence détaille les procédures récurrentes. Après **toute** modification,
mettre la documentation à jour avec la compétence `docs-sync` (un crochet bloque les commits si
`tests/test_docs.py` échoue).

## Où trouver quoi

- Formule ou réglage du firmware → `docs/spec-betaflight.md` (sources : dépôts `betaflight` et
  `betaflight-configurator` clonés à côté de ce projet, dans `../`).
- Méthode d'analyse → `docs/spec-methodes.md` ; écran PIDtoolbox à reproduire → `docs/spec-ecrans.md`.
- Valeurs de référence déjà établies → `docs/validation.md` ; seuils de l'assistant → `docs/assistant.md`.

## Contrôler la compilation Windows avant de pousser

La CI Windows met 10 à 25 min et est le seul endroit où l'on voit un test qui ne compile pas sous Windows (L-078, L-074).
La cible `x86_64-pc-windows-gnu` est installée sur le poste : une vérification de compilation (sans exécution) prend
quelques secondes.

```sh
cd rust && cargo check -p CRATE --lib --profile test --target x86_64-pc-windows-gnu
```

- `otb-gui` (même en dépendance de test) exige `windres` pour l'icône, absent ici : pour `otb-dev`, retirer
  **temporairement** la section `[dev-dependencies]` de `otb-dev/Cargo.toml` (sauvegarde dans le répertoire temporaire,
  puis `git checkout rust/Cargo.lock`) et vérifier les tests d'intégration un par un (`--test corpus_labels` …) ; le test
  `docs` dépend d'`otb-gui` et reste à la CI.
- Pas de linker mingw ici : on ne peut pas **exécuter** les tests (`wine` est présent, mais sans linker il n'y a pas
  d'exécutable). Le contrôle attrape les erreurs de compilation, pas les différences de comportement.
- Ne pas défaire un correctif avec `git checkout FICHIER` pendant une contre-épreuve : sauvegarder le fichier avant.

## Valider une modification du décodeur

```sh
(cd rust && cargo run --release -p otb-dev -- corpus validate --limit 50)   # échantillon rapide
(cd rust && cargo run --release -p otb-dev -- corpus validate --start 300)  # reprise après un blocage
```
Statuts : `ok`, `gap` (trames tronquées avant un trou SD, tolérées), `slow` (trames S corrompues acceptées
par la référence), `rows` / `values` (à examiner), `refhang` (référence bloquée, délai 180 s), `noref`
(la référence n'a rien écrit ; une référence introuvable arrête l'outil avec un message).
Référence : `blackbox_decode` de PIDtoolbox (`OTB_REF_DECODER`, défaut `~/.local/share/PIDtoolboxProV094/`).
Objectif : ne pas faire baisser la proportion de sessions `ok` (95 %).

## Recouper une analyse avec PIDtoolbox

1. Prendre un log du corpus dont le dossier contient des captures `PTB_FIGS_*` (ex. `Cedric/BF Chirp`,
   `1. PTB/BlackSummer`, `Cedric/JeNo 5`).
2. Lire la valeur sur la capture (ouvrir l'image), calculer la même avec `analysis/` sur la même période.
3. Ajouter la mesure à `docs/validation.md` si elle est nouvelle.

## Ajouter un outil à l'interface

1. Le calcul dans `analysis/`, avec un test synthétique dans `tests/`.
2. Une classe `ToolWindow` dans `gui/tools/<nom>.py` : `build_controls`, `snapshot_options`, `compute`, `render`.
3. L'enregistrer dans `gui/tools/__init__.py:REGISTRY` et ajouter une ligne à `main_window.py:TOOLS`
   (libellé court, icône Lucide copiée telle quelle dans `gui/assets/icons/`, couleur de `theme.ACCENTS`, info-bulle) ;
   le bouton affiche l'outil dans une page de la fenêtre principale.
4. Test hors écran : l'outil est déjà couvert par `tests/test_gui_tools.py` (chaque clé du registre, logs
   synthétiques du `conftest`) ; ajouter ses assertions propres sur le corpus (modèle : `tests/test_gui_step.py`,
   `compute_sync` puis `render`), capture et vérification visuelle.
5. Section dans `docs/guide-utilisateur.md`, ligne dans `CHANGELOG.md`.

## Captures hors écran

Script dans le répertoire temporaire de la session, lancé avec `QT_QPA_PLATFORM=offscreen` :
**`QStandardPaths.setTestModeEnabled(True)` avant `QApplication`** (sinon le script écrit dans les vrais
réglages de Cédric : dossier des logs, thème, géométrie de la fenêtre à la fermeture, L-029), créer
`MainWindow`, `win._change_theme("clair")` pour les captures du README (la fenêtre recharge le thème
enregistré), `load_paths([...])`, pomper les événements (`app.processEvents()` puis `time.sleep(0.02)` en
boucle, avec un délai en secondes) jusqu'à `win._loading == 0`, `open_tool(clé)`, attendre
`not w._busy and w._last_result is not None` ; pour plusieurs logs, `w.files.selectAll()` puis `w.run()`
et attendre de nouveau ; `w.grab().save(...)` ; ne pas fermer une fenêtre dont un calcul tourne encore
(`wait_all()` d'abord), puis **`hard_exit(0, wait_all)`**. Toujours regarder l'image produite.
Captures du README : `Cedric/JeNo 5/FINAL.bbl` (visualiseur, 1500 × 950, session FINAL.02) ; `LAST.bbl` +
`FINAL.bbl` (réponse indicielle avec les trois sessions, Assistant sur FINAL.02, 1600 × 1000).

## Calibrer l'assistant

Suivre la compétence `assistant-seuils` (calibration sur le corpus, choix du percentile, origine de
chaque seuil).

## Fabriquer les exécutables

- Linux : `.venv/bin/pyinstaller packaging/opentunebox.spec --noconfirm` (≈ 30 s, `dist/OpenTuneBox/` ≈ 460 Mo).
  Pas d'option `strip` : celui d'Ubuntu 22.04 abîme `libscipy_openblas` de numpy (« not page-aligned »).
  Vérifier après tout changement de la recette : exécutable d'essai (même recette, script qui lance
  `opentunebox report` avec un `NUMBA_CACHE_DIR` vide puis ouvre chaque outil hors écran).
- Windows : sources par `git archive --format=zip --prefix=OpenTuneBox/ -o dist/OpenTuneBox-source.zip HEAD`,
  puis `packaging\windows\construire_windows.bat` sur le PC Windows (Python 3.13 installé par winget).
- Tous systèmes : workflow `construire.yml` (manuel, ou étiquette `v*` → page Releases ; préparer la version
  avec `scripts/release.py prepare X.Y.Z`, voir `CONTRIBUTING.md`) : un fichier par système
  (installateur Inno Setup Windows, `.dmg` macOS, AppImage Linux construite sur Ubuntu 22.04),
  chacun essayé au démarrage. AppImage en local : télécharger appimagetool 1.9.1 dans le répertoire temporaire,
  puis `(cd rust && cargo build --release -p otb-gui)` et
  `APPIMAGETOOL=… sh packaging/linux/build_appimage.sh rust/target/release/opentunebox-gui SORTIE.AppImage`.
  Lancer l'AppImage avec un log en argument et capturer la fenêtre (`spectacle -b -n -a -o FICHIER.png`) ; pour l'arrêter,
  `kill` par identifiant : `pkill -f` avec un motif présent dans la commande tue le shell lui-même.
  Icônes `.icns` / `.png` : `python packaging/make_icons.py` si le SVG change.
