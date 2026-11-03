# AGENTS.md — consignes pour les agents IA

OpenTuneBox analyse les logs Blackbox Betaflight (`.bbl`/`.bfl`) pour régler les drones : décodeur natif,
analyses FFT / Wiener / filtres Betaflight, interface Qt, assistant à règles. Objectif : couvrir tous les
outils de PIDtoolbox Pro V0.94, en multiplateforme et hors ligne.

## Le propriétaire

Cédric pilote et règle des drones FPV ; il débute en programmation. **Lui répondre en français, expliquer
chaque étape simplement**, et justifier les choix techniques par leur effet concret. Il lance lui-même
les commandes qui demandent `sudo` : préparer la commande ou un script, ne pas la contourner.

## Décisions fixées (ne pas remettre en cause sans lui demander)

Résumé ; contexte, options écartées et preuves dans `docs/decisions.md`.

- Multiplateforme : Python + **PySide6** + **pyqtgraph** ; décodeur Python accéléré par **numba** (aucun binaire externe). D-001, D-002
- Assistant **100 % hors ligne**, à règles : aucune API, aucune donnée envoyée. D-003
- Projet **privé, tous droits réservés** (pas de licence libre). D-001
- **Aucune copie de code PIDtoolbox** (binaire chiffré) ni d'autres projets : tout est réimplémenté à partir
  des formules du firmware Betaflight et de méthodes publiées (voir `docs/spec-*.md`).
  Seule exception, validée par Cédric : les icônes [Lucide](https://lucide.dev) (licence ISC) dans
  `gui/assets/icons/`, à garder avec leur fichier de licence. D-004, D-005
- Migration vers Rust : `main` reste en Python, le travail passe par `feat/rust-migration` et ses sous-branches
  (`docs/migration-rust.md`) ; chaque module est prouvé équivalent avant de supprimer l'ancien. D-031
- Objectif de la migration : **plus aucun Python** dans le dépôt (application, tests, scripts), interface egui + egui_plot
  (sous réserve d'un prototype d'essai), noyau Rust autonome, Python figé comme étalon (tag `python-etalon`). D-035
- Interface, messages et documentation en français ; code et identifiants en anglais ; commentaires en français. D-001

## Suivi du projet

Trois registres numérotés, un maître par sujet (D-018) ; `tests/test_docs.py` vérifie que chaque numéro
est unique et que chaque numéro cité existe.

- `docs/decisions.md` (`D-xxx`) : choix en vigueur, options écartées, preuves. Un choix qui écarte une
  option, tranché par Cédric ou qui fixe un contrat entre deux parties du code y entre ; le remplacer =
  nouvelle décision, avec son accord.
- `docs/lecons.md` (`L-xxx`) : histoire complète des pièges. **En début de session**, lire son index rapide
  et la section du domaine touché ; **en fin de session**, y ajouter tout piège qui a coûté du temps. Ici,
  « Pièges connus » ne garde que la règle courte et son numéro.
- `docs/travaux-reportes.md` (`W-xxx`) : ce qui est vu et laissé de côté, avec la condition de reprise.
  Y noter tout défaut repéré hors du sujet de la branche, plutôt que de l'oublier ou de le corriger en douce.
- Renvoi dans le code : `# ⚠️ <ce qui casse si on l'ignore> (L-xxx)` ; la demande de fusion liste les
  numéros créés ou modifiés. Pas de numéro dans le sujet des commits.

## Organisation

```
src/opentunebox/
  decoder/    décodage binaire BBL (numba, cache=True) → DecodedSession (valeurs brutes) . Moteur par défaut : extension Rust (`OTB_BACKEND=python` : numba, le temps de la migration). L-032
  log/        FlightLog (unités physiques, courbes en float32 : D-029), settings.py (en-tête normalisé par version), debug_tables.py, modes.py, pid_terms.py (`PidTerms`)
  analysis/   calculs purs numpy (plus de scipy : `_numeric.py`, D-032 ; filtres Betaflight en Rust, `rust/otb-dsp`) : spectral, step (Wiener), step_series (séries de vols), loop (marges de
              stabilité), delay, bf_filters, sliders, motors, stats, period, datasets (jeux de données analysables)
  advisor/    profils de taille (profiles.py), règles (rules.py), diagnostic du bruit de Spectre (noise.py),
              diagnostic du réglage de la Réponse indicielle (step_tuning.py),
              guide intégré : vérification des logs (log_check.py), fiche drone (drone_card.py), parcours
              à cocher (tuning_path.py)
  gui/        app.py, main_window.py (visualiseur), guide.py (guide intégré : fiches drone, cases ; accueil dans assets/guide/),
              state.py, worker.py, theme.py, icons.py, geometry.py, tools/ (une page par outil), widgets/, assets/
  cli.py      commandes `opentunebox info` et `opentunebox report`
rust/         espace de travail Cargo (D-031) : otb-decoder, otb-log, otb-analysis, otb-advisor, otb-cli, otb-gui (interface egui), otb-parity (comparateur
              avec les références de Python), otb-dsp, otb-format, otb-py, otb-dev (outils de développement, jamais livrés : vérification de la
              documentation). Détail de chaque crate et de ses scripts de cas : docs/architecture.md, « Espace de travail Rust ».
tests/        pytest ; conftest.py (Qt hors écran, réglages isolés, fixtures `app`, `synthetic_log`, `synthetic_state`),
              synthetic.py (`make_flightlog`, `make_session` : vol et session fabriqués, sans corpus),
              bbl_factory.py (fichiers Blackbox binaires fabriqués : en-tête, trames I/P/S/E),
              data/jeno5_5s.bbl (5 s d'un vrai log de Cédric, décodé en CI par test_real_log.py) ;
              les tests marqués `corpus` utilisent de vrais logs (ignorés s'ils sont absents) ;
              test_docs.py vérifie la documentation
scripts/      validate_corpus.py (décodeur vs référence), calibrate_advisor.py (seuils de l'assistant),
              advisor_rates.py (taux de déclenchement des constats), corpus_labels.py (tableau d'étiquettes
              du corpus, passages parallèles map_files), golden.py (références figées pour la migration Rust, D-031), release.py (préparer une version, notes de la page Releases)
packaging/    opentunebox.spec (PyInstaller), launcher.py, icônes (make_icons.py), installateurs :
              windows/ (Inno Setup), macos/ (.dmg), linux/ (AppImage)
docs/         guide utilisateur, architecture, validation, assistant, spécifications (index : docs/README.md)
```

Règles :
- `analysis/` et `advisor/` n'importent **jamais** `gui/` : tout calcul doit rester utilisable en ligne de commande et testable.
- `gui/` affiche seulement ce que `analysis/` calcule. Un outil s'affiche dans une page de la fenêtre principale
  (`gui/main_window.py:MainWindow.open_tool`, boutons de la barre = onglets, jamais de fenêtre séparée) ; il hérite de `gui/tools/base.py:ToolWindow`
  (`build_controls`, `snapshot_options`, `compute` en arrière-plan, `render`) et s'enregistre dans `gui/tools/__init__.py:REGISTRY`.
- Les widgets Qt ne se lisent pas dans `compute` (autre thread) : copier leurs valeurs dans `snapshot_options`
  (vérifié par `tests/test_gui_tools.py` sur chaque outil du registre) ; les tests appellent `compute_sync`.
- Un constat de l'assistant (`advisor/rules.py:Finding`) se reconnaît par sa clé `key` (littéral anglais),
  jamais par son titre ou ses actions : les textes se reformulent librement ; un réglage déjà conseillé se
  déclare dans `settings` (D-028).

## Commandes

```sh
python3 -m venv .venv && .venv/bin/pip install -e '.[dev]'
.venv/bin/pytest -q                                  # tous les tests (≈ 4 min avec le corpus, 1 min sans)
.venv/bin/pytest -q -m "not corpus"                  # tests rapides, sans vrais logs
.venv/bin/ruff check src tests scripts packaging/make_icons.py   # lint (doit être vide, D-030)
.venv/bin/pytest -q --cov=opentunebox --cov-report=term-missing   # couverture (hors code numba : L-025)
.venv/bin/pytest -q tests/test_docs.py               # cohérence de la documentation (< 1 s)
(cd rust && cargo test -p otb-dev --test docs)   # la même, en Rust (remplacera la précédente à la phase 7, D-035)
(cd rust && cargo run -p otb-dev -- icons)   # refait packaging/linux/opentunebox.png et packaging/opentunebox.ico depuis le SVG
(cd rust && cargo run -p otb-dev -- release prepare X.Y.Z)   # préparer une version ; `release notes vX.Y.Z` : notes de la page Releases ; jusqu'à la 7-5, ne pas préparer une version avec `scripts/release.py` (il ne met pas à jour `rust/Cargo.toml`)
(cd rust && cargo run --release -p otb-dev -- corpus rates SORTIE.json)   # taux de chaque constat sur le corpus (remplace advisor_rates.py) ; `--compare AVANT.json APRÈS.json` : mêmes conseils ?
(cd rust && cargo run --release -p otb-dev -- corpus calibrate 600 SORTIE.json)   # mesures brutes du corpus pour calibrer les seuils (remplace calibrate_advisor.py) ; `corpus calibrate --resume SORTIE.json` : p10 / p50 / p90 par taille
(cd rust && cargo run --release -p otb-dev -- corpus labels)   # tableau d'étiquettes $OTB_LOGS/corpus.csv (remplace corpus_labels.py) ; ne réécrit jamais `etat`, `taille_reelle` ni `remarque` ; `--csv COPIE` pour essayer sur une copie
(cd rust && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -p otb-decoder -p otb-dsp -p otb-format -p otb-log -p otb-analysis -p otb-advisor -p otb-cli -p otb-gui -p otb-parity -p otb-dev)   # Rust
.venv/bin/pip install -e .                           # recompile l'extension Rust (maturin, profil release) après un changement de rust/
.venv/bin/opentunebox-gui [LOG…]                     # interface
.venv/bin/opentunebox report LOG.bbl                 # diagnostic en ligne de commande
.venv/bin/python scripts/validate_corpus.py [DOSSIER] [--limit N]   # décodeur vs blackbox_decode
.venv/bin/python scripts/corpus_labels.py                        # tableau d'étiquettes $OTB_LOGS/corpus.csv
.venv/bin/python scripts/advisor_rates.py SORTIE.json            # taux de chaque constat sur le corpus (≈ 2 min)
.venv/bin/python scripts/advisor_rates.py --compare AVANT.json APRÈS.json   # mêmes conseils avant / après ?
.venv/bin/python scripts/golden.py record|check "$OTB_LOGS/golden"   # références pour la migration Rust (D-031)
.venv/bin/pyinstaller packaging/opentunebox.spec --noconfirm        # exécutable autonome
.venv/bin/python scripts/release.py prepare X.Y.Z                 # préparer une version (CONTRIBUTING.md)
```

CI : `tests.yml` (lint, tests, couverture) et `audit.yml` (failles des dépendances, voir `CONTRIBUTING.md`) ; la
CI ne voit pas le corpus.

Variables d'environnement : `OTB_LOGS` (dossier de vrais logs, défaut `~/Documents/Drones/Logs` ; contient
aussi `corpus.csv`, les étiquettes des drones remplies par Cédric, à ne jamais écraser),
`OTB_REF_DECODER` (`blackbox_decode` de référence pour `validate_corpus.py`).

Tests d'interface et captures sans écran : `QT_QPA_PLATFORM=offscreen`, puis `widget.grab().save(...)` et
regarder l'image produite. **Terminer tout script Qt par `opentunebox.gui.app.hard_exit(code)`** :
PySide6 6.11 + pyqtgraph plantent (segfault) pendant le nettoyage automatique de fin de Python (L-019).

## Pièges connus

Règle courte de chaque piège ; histoire, cause et preuve dans `docs/lecons.md` sous le numéro indiqué. Ci-dessous, ceux qui valent pour
presque toute tâche ; **ceux d'un domaine sont dans [docs/pieges-par-domaine.md](docs/pieges-par-domaine.md) : le lire avant de travailler dessus.**

- Couleurs : jamais de couleur en dur (`#777`, `gray`…) ni de taille en pixels hors de `gui/theme.py` ; textes
  par `theme.accent_style`, `theme.muted_style` et `theme.current().ui`, courbes et repères par `theme.PLOT_COLORS`
  (`tests/test_gui_contracts.py` refuse tout code couleur ailleurs). Vérifier les deux thèmes. L-016
- scipy, numba et llvmlite ne servent plus qu'aux tests (extra `dev`) : `tests/test_no_heavy_deps.py` interdit
  de les importer dans l'application. Réexport nécessaire à un test (`core.CTX_*`) : `# noqa: F401`, car
  `ruff check --fix` supprime les imports « inutiles ». D-032, L-034, L-035, L-037. Tableau 2D passé au Rust à plat : `is_c_contiguous()` (L-036). Noyau parallèle (rayon, D-033) :
  jamais de `fork` après un appel ; `RAYON_NUM_THREADS=1` dans chaque processus du corpus (L-038)
- Pas de compilateur C sur le poste de développement : pas de dépendance à compiler. D-006
- Vérifier avant un commit : lancer `pytest` sans tube (`| tail` masque son échec) ; le crochet de Claude Code
  (`.claude/hooks/check-docs.sh`) ne voit que `git commit` et `git -c … commit`. L-022
- Test d'une clé de cache ou d'une règle d'affichage : casser la protection visée pour voir le test échouer ; `AppState::set_period` vide déjà le cache, donc ne prouve pas la clé (changer `period` à la main) ; une règle d'affichage sans effet testable devient une fonction pure. L-071
- Tests du corpus : ouvrir un log par `tests/corpus_paths.py:corpus_log` (ignoré en le nommant s'il manque) ;
  après un changement du corpus, `pytest -rs` liste les logs à remplacer. L-023
- Couverture : la CI n'a pas le corpus et `coverage` ne voit pas le code numba (`NUMBA_DISABLE_JIT=1`) ;
  tout outil ou analyse a d'abord un test sur les logs synthétiques (`tests/synthetic.py`), et un fichier
  marqué `corpus` en bloc ne contient aucun test pur. L-025

Sections de `docs/pieges-par-domaine.md` : décodeur et journaux de vol ; analyses ; filtres Betaflight et notch dynamique ; assistant de réglage ;
interface Qt ; interface egui et Rust ; construction des exécutables.

## Méthode de travail

1. Lire la doc liée à la tâche (`docs/spec-betaflight.md` pour toute formule firmware, `docs/spec-methodes.md`
   pour les analyses, `docs/architecture.md`) et l'index de `docs/lecons.md` ; vérifier qu'aucune décision
   (`docs/decisions.md`) ni aucun travail reporté (`docs/travaux-reportes.md`) ne porte déjà sur le sujet.
2. Valider sur **de vrais logs** en plus des tests synthétiques ; recouper avec les captures PIDtoolbox
   (`PTB_FIGS_*` du corpus) quand elles existent (résultats de référence dans `docs/validation.md`).
3. **Documentation à jour à chaque modification**, dans le même commit que le code : suivre la compétence
   `.claude/skills/docs-sync` (quels documents relire selon le changement) ; `tests/test_docs.py` vérifie
   automatiquement liens, chemins, outils, modules, fichiers publiés et langue.
   `pytest` et `ruff check` propres avant chaque commit ; mettre à jour `CHANGELOG.md` (section *Non publié*).
4. Commits en français, à l'impératif ou descriptifs, un sujet par commit. Auteur `Cedric <desmetgerry@gmail.com>`.
   Ne jamais pousser sur GitHub ni publier quoi que ce soit sans l'accord de Cédric.
5. Une branche par sujet, nommée `type/description-courte` (convention GitHub : `feat`, `fix`, `docs`,
   `refactor`, `test`, `chore` ; minuscules, tirets, anglais). La PR vers `main` a pour titre **exactement
   le nom de la branche** (détails dans `CONTRIBUTING.md`).
