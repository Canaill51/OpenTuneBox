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
rust/         espace de travail Cargo (D-031) : otb-advisor (assistant en Rust, phase 5a : `profiles.rs` (5a-1) ; `rules.rs` (5a-2, 5a-3), `noise.rs` (5a-4 : `scripts/noise_cases.py`) et `step_tuning.rs` (5a-5 : `scripts/step_tuning_cases.py`, références par dossier de drone `golden.py record-tuning`, couche `otb-parity/src/tuning_layer.rs`, L-062) ; `drone_card.rs`, `log_check.rs` et `tuning_path.rs` (5b : `scripts/drone_card_cases.py`, `scripts/log_check_cases.py`, `scripts/tuning_path_cases.py`, couche corpus `golden.py record-guide` / `otb-parity check-guide`, W-029) ; `pyclasses.rs` est généré par `scripts/profiles_cases.py` ; `scripts/golden_advisor.py` écrit les références, `golden.py record-advisor` / `otb-parity check-advisor`, L-060), otb-decoder (décodeur, Rust pur ; `header.rs` : en-têtes et sessions, 4a-2 ; `file.rs` : `decode_file` de bout en bout, 4a-3 ; `scripts/header_cases.py` et `scripts/decoder_fixtures.py` écrivent leurs cas), otb-dsp (noyaux des filtres Betaflight et de `step_response` ; dépendances realfft et rayon, D-033), otb-format (formats de nombres identiques à Python : `{:.2f}`, `%g`, `repr`, `round`, D-035 ; cas générés par `scripts/format_cases.py`), otb-parity (comparateur : relit les références de `golden.py record-modules` et rejoue chaque module porté ; `otb-parity check $OTB_LOGS/golden`, `check-header` pour les en-têtes, `check-settings` pour les réglages, `check-flightlog` pour les journaux de vol, `check-decoder` pour le décodage de bout en bout, D-035), otb-spike-egui (prototype d'interface jetable, hors espace de travail, E1), otb-log (rejet des trames impossibles et conversions float32 de l'ouverture d'un log, D-034 ; `debug_tables` et `modes` portés en 4a-1 : `scripts/log_tables_cases.py` écrit leurs cas ; `settings.rs` : `parse_settings`, 4a-4 : `scripts/settings_cases.py` et `golden.py record-settings` ; `flightlog.rs` : `FlightLog`, `from_session`, `load` et les méthodes, 4a-5 : `golden.py record-flightlog`, `scripts/decoder_fixtures.py`), otb-analysis (boucles par fenêtre de `analysis/motors.py`, génériques f32 / f64 : L-041, L-042 ; dépend d'otb-dsp et de rayon ; `spectral.rs` et `bf_filters.rs` portent `analysis/spectral.py` et `bf_filters.py`, `numeric.rs` d'otb-dsp porte `_numeric.py`, 4b-1 à 4b-4 ; `sliders.rs`, 4b-5 : `scripts/sliders_cases.py` et `golden.py record-sliders` / `otb-parity check-sliders` ; `npstat.rs`, `datasets.rs`, `period.rs`, `stats.rs`, `delay.rs` et `motors.rs` portent `analysis/datasets.py`, `period.py`, `stats.py`, `delay.py` et `motors.py` (colle des noyaux de la 3b), 4c-1 à 4c-3 : `scripts/flight_stats_cases.py`, `scripts/delay_cases.py`, `scripts/motors_cases.py`, `golden.py record-analysis` / `otb-parity check-analysis` ; `open_loop.rs` porte `analysis/loop.py` par étapes, 4c-B(a) : `margins`, `loop_from_t`, `dterm_model`, `scripts/loop_cases.py` ; (b) : `loop_spectra`, `estimate`, `scripts/loop_spectra_cases.py` ; (c) : `loop_from_terms`, jackknife, `scripts/loop_terms_cases.py` ; `step.rs` et `step_series.rs` portent `analysis/step.py` et `step_series.py`, 4c-B(d) et (e), plus le lissage du gyro et `segment_metrics` de `_step_common.py` en 6-3a : `scripts/step_cases.py`, `scripts/step_series_cases.py`), otb-cli (binaire `opentunebox info` / `report`, phase 5c : `lib.rs` produit le texte, `main.rs` l'écrit ; `scripts/golden.py record-cli` / `otb-parity check-cli`, W-030, W-031), otb-gui (interface egui, phase 6 : `theme.rs` seul fichier à contenir des couleurs (les tables de palettes de `colormaps.rs`, générées par `scripts/colormap_tables.py`, ne contiennent que des octets), `tools/` (un fichier par outil : `spectral.rs`, `freq_throttle.rs`, `freq_time.rs` en 6-2 ; `step.rs` (Réponse indicielle), `pid_slider.rs` (Curseurs PID) et `step_common.rs` (calcul d'un fichier, graphique des curseurs) en 6-3 ; `advisor.rs` (Assistant), `setup.rs` (Configuration, lignes alignées par nom : D-038) et `filter_sim.rs` + `filter_sim_calc.rs` (Simulateur de filtres : l'interface et les calculs purs, sans egui ; `scripts/filter_sim_cases.py` écrit `rust/otb-gui/tests/data/filter_sim.json`) en 6-4 ; `common.rs` briques partagées, `report_ui.rs` constats et diagnostic ; `tools::all_pages` les enregistre ; `cargo run --release -p otb-gui --example check_tools -- $OTB_LOGS/golden` les compare à Python, L-067, L-068), `state.rs` (logs chargés ; `Cache` des résultats par log, clé = réglages + période : D-037), `worker.rs` (file d'attente d'un calcul par outil), `tool.rs` (trait `Tool` : `compute` sans `self`), `plots.rs` (contournements d'egui_plot), `icons.rs`, `app.rs` (écrans Guide / Courbes / outil), `viewer.rs` (traces, lissage) et `viewer_page.rs` (page du visualiseur) ; `guide.rs` (Guide sans egui : fichier des fiches `drones.json`, mêmes octets et même chemin que Python, fichier illisible mis de côté, D-039 ; liens `otb:`, avancement, outils cités) et `guide_page.rs` (page Guide : accueil `accueil.md` de Python, phases, fiches, logs du drone vérifiés en arrière-plan ; elle rend des `GuideAction` que `App` exécute ; `App::new` prend un guide sans fichier, jamais le vrai `drones.json` dans les tests) ; `examples/capture.rs` fait des captures PNG sans fenêtre (rendu wgpu) ; `examples/banc_carte.rs` mesure `Heatmap` contre `PlotImage` ; banc de test : `with_context_setup(app::setup)`, L-066), otb-py (pont PyO3 → `opentunebox._native`)
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
(cd rust && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -p otb-decoder -p otb-dsp -p otb-format -p otb-log -p otb-analysis -p otb-advisor -p otb-cli -p otb-gui -p otb-parity)   # Rust
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

Règle courte de chaque piège ; histoire, cause et preuve dans `docs/lecons.md` sous le numéro indiqué.

- `blackbox_decode` (référence de PIDtoolbox) compte mal `loopIteration` avec les en-têtes `P interval:N`
  (Betaflight récents) et jette les trames P qui suivent la 1re trame I d'un bloc : comparer en alignant sur
  `time`. Il accepte aussi des trames corrompues (tronquées avant un trou de carte SD, trames S parasites)
  et se bloque sur certains fichiers ; notre décodeur suit le firmware (itération = boucle PID). L-001
- `log/flightlog.py:_drop_implausible` écarte les trames impossibles (gyro ou consigne > 4000 °/s, terme PID > ±65535) ;
  une session corrompue à plus de 50 % est ignorée. L-002
- `stats["corrupt_frames"]` compte des tentatives de resynchronisation, pas du temps perdu : juger un log
  abîmé par ses trous (`FlightLog.recording_gaps`). Toute analyse à fenêtres prend `breaks=log.breaks(sel)` :
  sans lui, chaque trou ajoute un faux bruit (jusqu'à +18 dB). D-020, L-027
- Numéros de `debug_mode` différents selon la version (FFT_FREQ = 17 en 4.x, 16 depuis 2025.12) → `log/debug_tables.py`. L-003
- 4.4/4.5 : `rollPID` D = D max et `d_min` = D de base ; 2025.12+ : l'inverse avec `d_max` → normalisé dans `log/settings.py`. L-004
- Retards de filtres : méthode de **phase** (10–100 Hz, pondérée par la cohérence), pas l'intercorrélation
  (dominée par le bruit haute fréquence). PIDtoolbox affiche un retard D 0,3 à 0,9 ms trop court (le gyro
  concorde) : ses objectifs de retard D ne s'appliquent pas tels quels. D-007, L-006
- Retard des filtres : jugé seulement avec des mouvements (`delay.MOVING_SETPOINT_DPS`, consigne
  > 30 °/s au moins 5 % du temps) ; en vol stationnaire, les filtres dynamiques ne travaillent qu'à un régime.
  Pas 50 °/s : les wobbles en mode Angle limité à 30° plafonnent vers 47 °/s. D-010, D-026, L-007
- Fréquence du log : deux minimums à ne pas mélanger. **Filtres** : au moins 2 kHz pour un gyro à 8 kHz,
  1,6 kHz à 3,2 kHz, plus pour les petits drones (`rules.log_rate_needed`) ; le blackbox sous-échantillonne
  sans filtre, tout ce qui dépasse fs/2 se replie. **Séries PID** : 1 kHz (`rules.pid_log_rate_needed`). Le
  guide range les logs par rôle (`log_check.FILTERS` / `PID`) : chaque case ne lit que ceux de sa phase.
  Vol stationnaire = identifier le bruit ; vol avec mouvements = juger le retard. D-017, L-008
- Feedforward : forcé à 0 par le firmware en mode **Angle** seulement (pas en Horizon) → masque
  `flightlog.ANGLE_MODE_MASK`, distinct de `ANGLE_HORIZON_MASK` (instrument de la boucle ouverte). Juger la
  réponse selon `FlightConditions.ff_gain(axe)` (0 en Angle), jamais le F de l'en-tête. D-014, D-025, L-005
- Réponse indicielle par séries (`advisor/step_tuning.py`) : seuils tirés de 10 séries Master du corpus,
  choix des pilotes relus dans les vols suivants ; la cassure varie d'environ 0,01 entre deux vols
  identiques (incertitude jackknife plus faible : elle ne couvre que le bruit d'un vol). L-012
- Échelle des DSP : `analysis/` calcule la densité physique (= `scipy.signal.welch`), 6,5 dB au-dessus de PIDtoolbox
  (`spectral.PTB_PSD_OFFSET_DB`) ; l'interface affiche l'échelle de PTB (`spectral.to_ptb_db`, dans `compute` des
  outils), l'assistant convertit lui-même (`rules.band_peak_ptb_db`) : ne jamais appliquer la conversion deux fois. D-016, L-009
- Réponse indicielle : fenêtres de 2 s, pas de 62,5 ms, consigne min 20 °/s, régularisation 1e-4 ; pic
  cherché dans max(150 ms, 3 × latence). Bande passante calculée à partir de 2 Hz, lissage `uniform_filter1d(mode="nearest")`.
- Notch dynamique : prévoir ses pics avec la **réponse** des notchs RPM du firmware (poids, Q), jamais en
  retirant les bandes des harmoniques : une harmonique à poids réduit laisse un reste qu'il suit. Le debug
  FFT_FREQ (`bf_filters.fft_freq_centres`) donne ce qu'il a vraiment suivi : y vérifier tout modèle. D-022, L-028
- Simulateur de filtres : méthode **relative** `bf_filters.whatif_spectrum` (spectre loggé × |H_modifié|²/|H_actuel|²
  à la cadence de la boucle PID, fenêtre par fenêtre), exacte quand rien ne change. La simulation temporelle
  `simulate_gyro` est approximative au-dessus de ~0,4 × fréquence du log. D-008, L-011, L-024
- Seuils de l'assistant = 90e percentile du corpus par taille de drone (`docs/assistant.md`) : les recalculer
  avec `scripts/calibrate_advisor.py` (`--resume` pour les percentiles ; scripts du corpus en parallèle sur
  tous les cœurs, compétence `calculs-corpus`), ne pas les inventer. Compter **une voix
  par drone** quand quelques drones dominent une taille (whoop, seuils mécaniques) ; moins de ~20 drones →
  seuil commun ou profil provisoire. Chaque seuil a une origine notée (corpus, théorie, firmware…). D-011, L-013
- Boucle ouverte (`analysis/loop.py`) : L = G·(Ce + Cd) **mesurée** à partir des termes P, I, D, F loggés ;
  G par estimateur instrumental (consigne, ou `rcCommand` en mode angle/horizon, plus l'excitation du debug
  CHIRP de Betaflight, ajoutée à la consigne avant le mode angle), jamais S(u, gyro)/S(u, u)
  (biaisé en boucle fermée). L = T/(1 − T) n'est qu'un repli « approché » (faux pour Betaflight : D sur le
  gyro, FF sur la consigne). La bande mesurable s'arrête vers 20–40 Hz (énergie du manche) : pic de
  sensibilité souvent minorant. L'onglet Autotune du configurateur n'est pas une source. D-012, L-010
- Interface : style Qt **Fusion** imposé partout (`gui/theme.py`). Le style natif Windows 11 ignore la palette
  et grise les textes. Sous Fusion (Qt 6.11), les cases et boutons radio sont presque invisibles : ils sont
  redessinés par la feuille de style globale. Le délégué natif des `QComboBox` ignore cette feuille
  (texte blanc sur blanc) : `theme._ComboDelegateFix` le remplace par `QStyledItemDelegate`. D-009, L-014, L-015
- Couleurs : jamais de couleur en dur (`#777`, `gray`…) ni de taille en pixels hors de `gui/theme.py` ; textes
  par `theme.accent_style`, `theme.muted_style` et `theme.current().ui`, courbes et repères par `theme.PLOT_COLORS`
  (`tests/test_gui_contracts.py` refuse tout code couleur ailleurs). Vérifier les deux thèmes. L-016
- Linux : fenêtres d'ouverture / d'enregistrement par le portail XDG (`QT_QPA_PLATFORMTHEME=xdgdesktopportal`
  dans `gui/app.py`), indépendant du bureau. Logo de KDE sur la fenêtre du portail : normal. Sans portail,
  `theme._match_icon_theme` choisit les icônes claires / sombres. Textes de Qt en français : `qtbase_fr.qm`,
  seule traduction gardée par la recette PyInstaller. D-013, L-017
- Une feuille de style Qt ne charge une image que depuis un fichier : icône recolorée écrite dans le dossier
  temporaire (`theme._colored_svg`). L-018
- scipy, numba et llvmlite ne servent plus qu'aux tests (extra `dev`) : `tests/test_no_heavy_deps.py` interdit
  de les importer dans l'application. Réexport nécessaire à un test (`core.CTX_*`) : `# noqa: F401`, car
  `ruff check --fix` supprime les imports « inutiles ». D-032, L-034, L-035, L-037. Tableau 2D passé au Rust à plat : `is_c_contiguous()` (L-036). Noyau parallèle (rayon, D-033) :
  jamais de `fork` après un appel ; `RAYON_NUM_THREADS=1` dans chaque processus du corpus (L-038)
- Pas de compilateur C sur le poste de développement : pas de dépendance à compiler. D-006
- Linux : PyInstaller a besoin de `binutils` (`objdump`). Windows/macOS : construire sur le système cible. L-020
- Exécutable Windows sans console (`console=False`) : `sys.stdout` et `sys.stderr` valent `None` ;
  ne jamais les utiliser sans vérifier (voir `gui/app.py:hard_exit`). L-021
- Vérifier avant un commit : lancer `pytest` sans tube (`| tail` masque son échec) ; le crochet de Claude Code
  (`.claude/hooks/check-docs.sh`) ne voit que `git commit` et `git -c … commit`. L-022
- Les couches du corpus (`advisor_rates.py`, `golden_advisor.py`) écartent les sessions de moins de 8 s : elles ne voient pas un plantage sur une session courte ; seule la couche de la ligne de commande les rejoue. Voir le test échouer avant tout correctif. L-065
- Panneau egui redimensionnable : son contenu est toujours dans un `ScrollArea` qui remplit (`auto_shrink([false, false])`), même dans la branche « rien à montrer » ; sinon il garde la taille de son premier cadre. Un test part de l'état vide, et on regarde une capture. L-073
- Identité d'un log : `LoadedLog::uid` (ou `LogInput::uid`), jamais l'adresse de son `Arc` (elle peut resservir à un autre log). L-072
- Test d'une clé de cache ou d'une règle d'affichage : casser la protection visée pour voir le test échouer ; `AppState::set_period` vide déjà le cache, donc ne prouve pas la clé (changer `period` à la main) ; une règle d'affichage sans effet testable devient une fonction pure. L-071
- Tests du corpus : ouvrir un log par `tests/corpus_paths.py:corpus_log` (ignoré en le nommant s'il manque) ;
  après un changement du corpus, `pytest -rs` liste les logs à remplacer. L-023
- Couverture : la CI n'a pas le corpus et `coverage` ne voit pas le code numba (`NUMBA_DISABLE_JIT=1`) ;
  tout outil ou analyse a d'abord un test sur les logs synthétiques (`tests/synthetic.py`), et un fichier
  marqué `corpus` en bloc ne contient aucun test pur. L-025

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
