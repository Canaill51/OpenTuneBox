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
Cela demande sur le poste Linux la cible `x86_64-pc-windows-gnu` et les paquets `binutils-mingw-w64-x86-64` (`windres`, pour
l'icône d'`otb-gui`), `gcc-mingw-w64-x86-64` (éditeur de liens) et `wine64` (pour exécuter les tests).

⚠️ **Vérifier leur présence, ne pas la supposer** : elle a déjà été perdue (2026-11-08, cible et paquets absents alors que cette
page les disait installés depuis le 2026-11-05). Sans eux, `cargo check --target` échoue sur « can't find crate for `core` »
(cible) ou « ajout de l'icône à l'exécutable Windows: NotFound » (`windres`), ce qui n'a rien à voir avec le code en cours.

```sh
command -v x86_64-w64-mingw32-windres x86_64-w64-mingw32-gcc wine   # les trois doivent répondre
rustup target list --installed | grep windows                        # sinon : rustup target add x86_64-pc-windows-gnu
sudo apt install binutils-mingw-w64-x86-64 gcc-mingw-w64-x86-64 wine64   # à lancer par Cédric
```

```sh
cd rust && cargo check --workspace --all-targets --target x86_64-pc-windows-gnu   # compilation seule : ≈ 12 s pour otb-gui à chaud
```

Exécuter les tests sous `wine` (dossier `wine` 64 bits propre au projet, jamais `~/.wine`, qui est en 32 bits) :

```sh
export WINEPREFIX=$HOME/.local/share/wine-otb WINEDEBUG=-all CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine
[ -f "$WINEPREFIX/system.reg" ] || WINEARCH=win64 wineboot -i   # une seule fois
cd rust && cargo test --target x86_64-pc-windows-gnu --no-fail-fast   # ≈ 2 min ; sans tube (L-022)
```

Mesuré le 2026-11-08 (`main` après la 0.9.0) : **85 exécutables de test sur 90 passent**. Les 5 échecs **viennent de
l'environnement**, la CI Windows les fait passer :
- `otb-dev --test docs` : « `git ls-files` a échoué… program not found » (pas de `git` pour Windows dans le dossier `wine`) ;
- `otb-dev --test licenses` : « `cargo metadata` : program not found » (même cause ; ce test est né avec le lot 7a, après le
  premier relevé du 2026-11-05, qui annonçait donc 4 échecs sur 89 exécutables) ;
- `otb-advisor --test rules_rpm` (dernier chiffre), `otb-analysis --test open_loop` (1,4e-5) et `--test open_loop_spectra`
  (5e-8) : comparaisons numériques fines. La CI compile pour `x86_64-pc-windows-msvc` (fonctions mathématiques de Microsoft) ;
  ici, `windows-gnu` avec celles de `wine`. Cause **supposée** (la fonction en cause n'a pas été cherchée).

Un échec dont le message cite un programme absent (`git`, `cargo`…) est donc un signe de `wine`, pas du code : le vérifier avant
de chercher ailleurs.

Donc : `wine` fait foi pour la compilation, les chemins, les fichiers, les processus et l'interface hors écran ; **pas** pour un écart
numérique au dernier chiffre, que seule la CI Windows (ou le PC Windows) tranche. Un nouvel échec hors de cette liste est à examiner.

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
  Lancer l'AppImage avec un log en argument, puis **laisser Cédric la regarder** : ⚠️ ne jamais photographier son écran
  (`spectacle -a` capture la fenêtre **active**, c'est-à-dire la sienne, pas celle qu'on vient de lancer — arrivé le 2026-11-06,
  L-090). Pour voir l'interface, utiliser les captures hors écran (`--example capture`). Pour l'arrêter, `kill` par identifiant :
  `pkill -f` avec un motif présent dans la commande tue le shell lui-même.
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
