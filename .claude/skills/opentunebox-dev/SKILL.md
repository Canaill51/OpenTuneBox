---
name: opentunebox-dev
description: Procédures du projet OpenTuneBox (analyse Blackbox Betaflight, clone de PIDtoolbox Pro) — valider le décodeur et les analyses sur le corpus de vrais logs, recouper avec PIDtoolbox, ajouter un outil à l'interface, faire des captures hors écran, calibrer l'assistant, fabriquer les exécutables. À charger avant de modifier le décodeur, les analyses (FFT, Wiener, filtres BF) ou l'interface egui.
---

# OpenTuneBox — procédures

Les règles générales (organisation, décisions, commandes, pièges) sont dans `AGENTS.md` à la racine :
le lire d'abord. Cette compétence détaille les procédures récurrentes. Après **toute** modification,
mettre la documentation à jour avec la compétence `docs-sync` (un crochet bloque les commits si
`cargo test -p otb-dev --test docs` échoue).

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
2. Lire la valeur sur la capture (ouvrir l'image), calculer la même avec `rust/otb-analysis` (ou l'outil de l'interface) sur la même période.
3. Ajouter la mesure à `docs/validation.md` si elle est nouvelle.

## Ajouter un outil à l'interface

1. Le calcul dans `rust/otb-analysis/src/`, avec un test sur un log fabriqué (`rust/otb-gui/tests/support/mod.rs:synthetic_log`).
2. Un fichier `rust/otb-gui/src/tools/<nom>.rs` : une structure qui implémente le trait `Tool` de `rust/otb-gui/src/tool.rs`
   (`options`, `selected`, `compute` sans `self`, `controls`, `render`).
3. L'enregistrer dans `rust/otb-gui/src/tools/mod.rs:all_pages` (ordre = ordre de la barre) et, s'il a son bouton, ajouter son
   titre à `tab_icon` dans `rust/otb-gui/src/app.rs` (icône Lucide copiée telle quelle dans `rust/otb-gui/assets/icons/` avec sa
   référence dans `icons.rs`, couleur d'accent du thème) ; le bouton affiche l'outil dans une page de la fenêtre principale.
4. Tests : `rust/otb-gui/tests/contracts.rs` couvre les couleurs, les icônes citées et les deux thèmes ; ajouter les assertions propres
   à l'outil (modèles : `rust/otb-gui/tests/step_tools.rs`, `rust/otb-gui/tests/spectral.rs` ; `Tool::compute` se teste directement), puis une capture et une
   vérification visuelle.
5. Section dans `docs/guide-utilisateur.md`, ligne dans le tableau du `README.md`, ligne dans `CHANGELOG.md`.

## Captures hors écran

`cargo run --release -p otb-gui --example capture -- LOG.bbl DOSSIER_DE_SORTIE [AUTRE_LOG…]` (rendu wgpu sans fenêtre, jamais dans la
CI) écrit `courbes-clair.png`, `courbes-sombre.png`, puis une capture par outil et par thème. Toujours regarder l'image produite.
Pour une capture du Guide, partir d'un test de `rust/otb-gui/tests/guide.rs` : `App::new` prend un guide sans fichier, jamais le vrai
`drones.json` de Cédric.
Captures du README : `Cedric/JeNo 5/FINAL.bbl` (visualiseur, 1500 × 950, session FINAL.02) ; `LAST.bbl` +
`FINAL.bbl` (réponse indicielle avec les trois sessions, Assistant sur FINAL.02, 1600 × 1000).

## Calibrer l'assistant

Suivre la compétence `assistant-seuils` (calibration sur le corpus, choix du percentile, origine de
chaque seuil).

## Fabriquer les exécutables

Le programme livré est l'interface Rust seule (`cargo build --release -p otb-gui`, 26 Mo).

- Linux, en local : télécharger appimagetool 1.9.1 dans le répertoire temporaire (empreinte SHA-256 dans `construire.yml`), puis
  `(cd rust && cargo build --release -p otb-gui)` et
  `APPIMAGETOOL=… sh packaging/linux/build_appimage.sh rust/target/release/opentunebox-gui SORTIE.AppImage`.
  Lancer l'AppImage avec un log en argument et capturer la fenêtre (`spectacle -b -n -a -o FICHIER.png`) ; pour l'arrêter,
  `kill` par identifiant : `pkill -f` avec un motif présent dans la commande tue le shell lui-même.
- Windows : `packaging\windows\construire_windows.bat` sur le PC Windows (Rust installé par winget si absent ; les outils de
  compilation C++ de Visual Studio sont nécessaires, le script donne la commande si le lien échoue). Le script n'a pas été essayé sur
  un vrai Windows au 2026-11-03 (W-044) : à essayer à la première occasion.
- Tous systèmes : workflow `construire.yml` (manuel, ou étiquette `v*` → page Releases ; préparer la version avec
  `(cd rust && cargo run -p otb-dev -- release prepare X.Y.Z)`, voir `CONTRIBUTING.md`) : un fichier par système (installateur Inno
  Setup Windows, AppImage Linux construite sur Ubuntu 22.04), chacun essayé au démarrage avec un petit log en argument. Avant de le
  modifier : `actionlint .github/workflows/*.yml` (installé sur le poste ; mettre `shell: pwsh` aux étapes Windows, sinon il les
  prend pour du bash). Le lancer à la main sur la branche (`gh workflow run construire.yml --ref BRANCHE`, accord de Cédric) avant
  de fusionner.
- Icônes : `(cd rust && cargo run -p otb-dev -- icons)` si le SVG change.
