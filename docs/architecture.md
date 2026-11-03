# Architecture

## Origine et objectif

PIDtoolbox Pro V0.94 est un logiciel MATLAB fermé et payant, disponible sous Linux seulement via le
MATLAB Runtime. OpenTuneBox couvre les mêmes outils avec un code entièrement réécrit, multiplateforme et
hors ligne, pour régler des drones de toutes tailles (whoops 65–75 mm jusqu'aux 8"+).

Le corpus de mise au point (`OTB_LOGS`, par défaut `~/Documents/Drones/Logs`) contient environ 780 sessions
(582 fichiers, Betaflight 4.4.2 → 2026.6.2, DShot bidirectionnel, majoritairement 2 kHz, debug `FFT_FREQ`)
et 638 captures PIDtoolbox (`PTB_FIGS_*`) servant de référence visuelle.

## Décisions

Les choix structurants (Qt et pyqtgraph, décodeur numba, assistant hors ligne, aucune copie de
PIDtoolbox, langue, licence, méthodes de mesure…) sont dans le [journal des décisions](decisions.md), avec
leur contexte, les options écartées et ce qui les prouve.

## Flux de données

```
fichier .bbl/.bfl
  └─ decoder/header.py      en-têtes H, définitions de trames, débuts de sessions
  └─ decoder/core.py        décodage numba des trames I/P/S/G/H/E (encodages + prédicteurs)
     rust/otb-decoder       même décodeur en Rust (extension `opentunebox._native`, pont `rust/otb-py`) ;
                            `OTB_BACKEND=rust|python` choisit l'un ou l'autre pendant la migration (D-031)
                            `decoder/core.py` (numba) n'est chargé que par `OTB_BACKEND=python` (tests de parité, D-032)
  └─ decoder/reader.py      DecodedSession (entiers bruts, trames S propagées)
       └─ log/flightlog.py  (rejet des trames impossibles et conversions float32 en Rust : `rust/otb-log`, D-034)
                            FlightLog : temps, gyro, consigne, P/I/D/F, moteurs, gaz, debug, tension…
                            en unités physiques ; trames impossibles écartées ; settings.py normalise l'en-tête
            └─ analysis/*   calculs purs (aucun Qt)
                 ├─ advisor/rules.py      constats (clé stable `Finding.key`, D-028) + actions, seuils par profil
                 │                        (advisor/profiles.py) ; `analyze` par sections (enregistrement, réponse,
                 │                        bruit, retards, filtres, moteurs, curseurs) sur un état partagé
                 ├─ advisor/noise.py      diagnostic du bruit de la fenêtre Spectre (constats de rules.py rangés
                 │                        par partie : conditions, gyro, terme D, stratégie ; niveaux avant/après)
                 ├─ advisor/step_tuning.py  diagnostic du réglage de la Réponse indicielle (séries de vols :
                 │                        équilibre P/D, Master, roll / pitch ; analysis/step_series.py)
                 ├─ advisor/log_check.py  vérification des logs ouverts pour le guide intégré : parties du vol
                 │                        (analysis/period.py:flight_parts), protocole des mouvements, usages,
                 │                        corrections (configurateur d'abord, CLI en repli), repères des filtres
                 ├─ advisor/drone_card.py fiche drone (version, taille, boucle PID, ESC) et valeurs qui en découlent
                 ├─ advisor/tuning_path.py parcours du guide : 4 phases de cases, état automatique d'après les logs
                 ├─ cli.py                opentunebox info / report
                 └─ gui/tools/*           une page par outil (calcul en arrière-plan, affichage pyqtgraph)
```

## Modules d'analyse

| Module | Rôle |
|---|---|
| `spectral.py` | PSD de Welch, fréquence × gaz (fenêtres 300 ms, 100 tranches), fréquence × temps |
| `step.py` | Réponse indicielle par déconvolution de Wiener (noyau en Rust, `rust/otb-dsp/src/step.rs`, D-033) ; métriques (pic, latence, montée, cassure, vagues), bande passante, et boucle ouverte par `loop.py` |
| `step_series.py` | Séries de vols à un seul curseur changé (Damping, Master, Pitch Tracking, Pitch Damping) et conditions du protocole (feedforward, mode Angle, curseur I) |
| `loop.py` | Boucle ouverte L = G·(Ce + Cd) mesurée à partir des termes P, I, D, F (estimateur instrumental, jackknife), repli approché T/(1 − T) ; marges de phase et de gain, croisement, pic de sensibilité |
| `delay.py` | Retards des filtres (phase pondérée par la cohérence) et consigne → gyro |
| `bf_filters.py` | Filtres Betaflight exacts (PT1/2/3, biquad, SVF, RPM, notch dynamique, LPF dynamiques), chaînes gyro et D, spectre « et si », passe-bas relevé ou coupé ; noyaux de calcul en Rust (`rust/otb-dsp`, D-032) |
| `_numeric.py` | Primitives numpy qui remplacent scipy : fenêtre de Hann, moyenne glissante, flou gaussien, intercorrélation (D-032) |
| `sliders.py` | Curseurs simplifiés ⇄ PID et filtres, commandes CLI |
| `motors.py` | Fréquences moteur depuis l'eRPM, harmoniques, centres des notchs dynamiques, bruit moteur, hauteur des harmoniques et poids RPM juste suffisant, pics à suivre par le notch dynamique, raies hors moteurs (vol stationnaire) ; boucles par fenêtre en Rust (`rust/otb-analysis`, D-034), FFT et découpage restent en numpy |
| `stats.py`, `period.py` | Statistiques de vol, erreur PID robuste, période automatique, détection de crash |
| `datasets.py` | Jeux de données analysables (gyro, gyro préfiltre, terme D…, comme les listes de PIDtoolbox) |

## Interface

- `gui/state.py:AppState` : logs ouverts (10 maximum, une couleur chacun), log courant, période d'analyse ;
  ses signaux relancent les outils ouverts.
- `gui/worker.py:run_async` : calcul dans un `QThreadPool`, résultat renvoyé au thread de l'interface.
- `gui/tools/base.py:ToolWindow` : sélection des fichiers, axes R/P/Y, boutons communs, file de recalcul.
- Outils (`gui/tools/`, une page chacun dans la fenêtre principale, enregistrés dans `gui/tools/__init__.py:REGISTRY`) :

  | Bouton | Module | Fenêtre |
  |---|---|---|
  | Spectre | `spectral_window.py` | analyse spectrale (+ `_spectral_common.py`, partagé avec les deux suivantes) |
  | Fréq. × gaz (dans Spectre) | `freq_throttle.py` | fréquence × gaz, jusqu'à 4 colonnes |
  | Fréq. × temps (dans Spectre) | `freq_time.py` | spectrogrammes, curseur lié au lecteur |
  | Indicielle | `step_window.py` | réponse indicielle (+ `_step_common.py`, partagé avec les curseurs PID) |
  | Curseurs PID | `pid_slider.py` | curseurs du réglage simplifié et simulation |
  | Filtres | `filter_sim.py` | simulateur de filtres |
  | Configuration (panneau de droite) | `setup_info.py` | réglages des logs côte à côte |
  | Assistant | `advisor_panel.py` | rapport de l'assistant de réglage |
- `gui/app.py` : démarrage. `hard_exit` sort sans le nettoyage automatique de PySide6 (contournement d'un
  plantage) ; `FileOpenFilter` charge les logs ouverts depuis le Finder de macOS (`QFileOpenEvent`) ;
  sous Linux, fenêtres d'ouverture du bureau par le portail XDG.
- `gui/main_window.py` : fenêtre principale. Barre d'outils (`TOOLS` : clé, libellé, icône, couleur, info-bulle)
  dont les boutons servent d'onglets : `QStackedWidget` (`pages`), une page pour le visualiseur, une pour le guide et une par outil
  ouvert (`open_tool`, `show_viewer`), bouton coché = page affichée (`OTHER_TOOLS` : bouton d'origine de
  Fréq. × gaz, Fréq. × temps, Configuration) ; un outil caché ne recalcule qu'à son affichage
  (`ToolWindow._stale`). Visualiseur : traces
  groupées (`TRACE_GROUPS`, traces absentes du log masquées), sections repliables Affichage et Lecteur.
- `gui/guide.py` : page **Guide** (`GuidePage`), affichée au démarrage sans log et par le bouton Guide
  (`MainWindow.show_guide`). Fiche drone (`CardDialog`, fiches en JSON dans le dossier de données de
  l'utilisateur), frise et liste des phases, cases (`StepWidget`) décrites par `advisor/tuning_path.py`,
  logs du drone (reconnus par `craft_name`) rangés en logs des filtres et logs des séries PID (rôle donné par
  le bouton d'ouverture d'une case, retenu par fichier, sinon deviné par `LogCheck.role`), vérifiés par
  `advisor/log_check.py` : rapide d'abord, complet en
  arrière-plan (vrai nombre de fenêtres, repères des filtres), résultats gardés dans `LoadedLog.extras` par
  période. Accueil : `gui/assets/guide/accueil.md` ; liens `otb:phase/<clé>` et `otb:tool/<clé>` vérifiés
  par `tests/test_docs.py`.
- `gui/theme.py` : thèmes clair et sombre. Style Qt **Fusion** sur tous les systèmes, palette complète,
  feuille de style globale (boutons, champs, listes, cases), couleurs d'accent par thème (`accent`, `accent_style` ; aussi celles des icônes d'outils),
  textes d'aide (`muted_style`), police de l'application × 1,15. Seul fichier de `gui/` à écrire une couleur :
  courbes et repères des graphiques (`PLOT_COLORS`), badges de l'assistant (`SEVERITY_COLORS`), texte sur fond
  coloré (`ON_ACCENT`), fond de légende par thème (`Theme.legend_fill`) ; `tests/test_gui_contracts.py` refuse
  tout code couleur ailleurs (L-016). Textes de Qt en français
  (`install_qt_translations`) ; thème d'icônes du bureau accordé au thème clair / sombre (`_match_icon_theme`).
- `gui/icons.py` : icônes Lucide (`gui/assets/icons/`, licence ISC) colorées selon le thème.
- `gui/geometry.py` : taille de la fenêtre principale (95 % de l'écran au premier lancement), puis taille et
  position mémorisées (`geometry/main` dans les réglages).
- `gui/widgets/` : `section.py` (section repliable mémorisée), `axis.py` (axes à grille principale seule),
  `sticks.py` (manches du lecteur).

## Distribution

- Exécutable autonome : PyInstaller (`packaging/opentunebox.spec`, point d'entrée `packaging/launcher.py`),
  allégé des parties de Qt et de pyqtgraph inutilisées.
- Installateurs, un par système, fabriqués et essayés par `.github/workflows/construire.yml`, envoyés dans un
  brouillon de la page Releases à chaque étiquette `v…`, publiée quand les trois y sont (sans *Artifacts* :
  quota de 500 Mo d'un dépôt privé ; `scripts/release.py`, procédure dans `CONTRIBUTING.md`) :
  Windows `packaging/windows/opentunebox.iss` (Inno Setup ; un seul fichier, l'exécutable Rust installé sous le nom `OpenTuneBox.exe` ; le dossier `_internal` de la version Python est supprimé à la mise à jour, W-044), macOS `packaging/macos/build_dmg.sh` (.dmg),
  Linux `packaging/linux/build_appimage.sh` (AppImage de l'exécutable Rust `opentunebox-gui`, appimagetool 1.9.1, construite sur Ubuntu 22.04 ; 10,6 Mo mesurés le 2026-11-03, contre 167 Mo en Python 0.8.0).
- Icônes : `packaging/make_icons.py` fabrique `.icns` (macOS) et `.png` (Linux) depuis le SVG ; `.ico` pour Windows.
- Depuis les sources : `packaging/linux/install_linux.sh` (compile `otb-gui`, l'installe dans `~/.local/bin` et ajoute le raccourci du menu Linux) ;
  `packaging/windows/construire_windows.bat` fabrique l'exécutable Rust (cargo, bibliothèque C embarquée) et, si Inno Setup est là, l'installateur sur un PC Windows.

## Espace de travail Rust

Espace de travail Cargo (D-031), `rust/`. Une entrée par crate ; les scripts `scripts/*_cases.py` écrivent les cas de test, `scripts/golden*.py` les références du corpus.

- **otb-advisor** (assistant en Rust, phase 5a : `profiles.rs` (5a-1) ; `rules.rs` (5a-2, 5a-3), `noise.rs` (5a-4 : `scripts/noise_cases.py`) et `step_tuning.rs` (5a-5 : `scripts/step_tuning_cases.py`, références par dossier de drone `golden.py record-tuning`, couche `otb-parity/src/tuning_layer.rs`, L-062) ; `drone_card.rs`, `log_check.rs` et `tuning_path.rs` (5b : `scripts/drone_card_cases.py`, `scripts/log_check_cases.py`, `scripts/tuning_path_cases.py`, couche corpus `golden.py record-guide` / `otb-parity check-guide`, W-029) ; `pyclasses.rs` est généré par `scripts/profiles_cases.py` ; `scripts/golden_advisor.py` écrit les références, `golden.py record-advisor` / `otb-parity check-advisor`, L-060)
- **otb-decoder** (décodeur, Rust pur ; `header.rs` : en-têtes et sessions, 4a-2 ; `file.rs` : `decode_file` de bout en bout, 4a-3 ; `frame.rs` : prédicteurs et lecture d'une trame ou d'un événement, miroir de `decoder/core.py` ; `scripts/header_cases.py` et `scripts/decoder_fixtures.py` écrivent leurs cas)
- **otb-dsp** (noyaux des filtres Betaflight et de `step_response` ; dépendances realfft et rayon, D-033)
- **otb-format** (formats de nombres identiques à Python : `{:.2f}`, `%g`, `repr`, `round`, D-035 ; cas générés par `scripts/format_cases.py`)
- **otb-parity** (comparateur : relit les références de `golden.py record-modules` et rejoue chaque module porté ; `otb-parity check $OTB_LOGS/golden`, `check-header` pour les en-têtes, `check-settings` pour les réglages, `check-flightlog` pour les journaux de vol, `check-decoder` pour le décodage de bout en bout, D-035 ; modules : `registry.rs` (fonctions Python rejouables par nom), `value.rs` (valeurs des fichiers de référence), `layers.rs` (couches par fichier du corpus : en-têtes), `decoded.rs` et `flightlog_digest.rs` (empreintes des sessions décodées et des `FlightLog`), `settings_json.rs` (`Settings` en JSON), `analysis_layer.rs`, `advisor_layer.rs`, `cli_layer.rs` et `guide_layer.rs` (une couche par famille de références))
- **otb-log** (rejet des trames impossibles et conversions float32 de l'ouverture d'un log, D-034 ; `debug_tables` et `modes` portés en 4a-1 : `scripts/log_tables_cases.py` écrit leurs cas ; `settings.rs` : `parse_settings`, 4a-4 : `scripts/settings_cases.py` et `golden.py record-settings` ; `flightlog.rs` : `FlightLog`, `from_session`, `load` et les méthodes, 4a-5 : `golden.py record-flightlog`, `scripts/decoder_fixtures.py`)
- **otb-analysis** (boucles par fenêtre de `analysis/motors.py`, génériques f32 / f64 : L-041, L-042 ; dépend d'otb-dsp et de rayon ; `spectral.rs` et `bf_filters.rs` portent `analysis/spectral.py` et `bf_filters.py`, `numeric.rs` d'otb-dsp porte `_numeric.py`, 4b-1 à 4b-4 ; `sliders.rs`, 4b-5 : `scripts/sliders_cases.py` et `golden.py record-sliders` / `otb-parity check-sliders` ; `npstat.rs`, `datasets.rs`, `period.rs`, `stats.rs`, `delay.rs` et `motors.rs` portent `analysis/datasets.py`, `period.py`, `stats.py`, `delay.py` et `motors.py` (colle des noyaux de la 3b), 4c-1 à 4c-3 : `scripts/flight_stats_cases.py`, `scripts/delay_cases.py`, `scripts/motors_cases.py`, `golden.py record-analysis` / `otb-parity check-analysis` ; `open_loop.rs` porte `analysis/loop.py` par étapes, 4c-B(a) : `margins`, `loop_from_t`, `dterm_model`, `scripts/loop_cases.py` ; (b) : `loop_spectra`, `estimate`, `scripts/loop_spectra_cases.py` ; (c) : `loop_from_terms`, jackknife, `scripts/loop_terms_cases.py` ; `step.rs` et `step_series.rs` portent `analysis/step.py` et `step_series.py`, 4c-B(d) et (e), plus le lissage du gyro et `segment_metrics` de `_step_common.py` en 6-3a : `scripts/step_cases.py`, `scripts/step_series_cases.py`)
- **otb-cli** (binaire `opentunebox info` / `report`, phase 5c : `lib.rs` produit le texte, `main.rs` l'écrit ; `scripts/golden.py record-cli` / `otb-parity check-cli`, W-030, W-031)
- **otb-dev** (outils de développement, jamais livrés, phase 7-1 : `src/docs.rs` porte les contrôles de `tests/test_docs.py` (liens, chemins et symboles cités, outils, guide, modules, index de `docs/`, fichiers publiés, commandes de la CLI, langue, registres D / L / W, seuils de `docs/assistant.md`) ; `rust/otb-dev/tests/docs.rs` les lance sur la vraie documentation (`OTB_DOCS_ROOT` désigne une autre racine) et `rust/otb-dev/tests/checks.rs` prouve que chacun échoue sur un cas fabriqué ; binaire `otb-dev`, phase 7-2 : `release.rs` (`otb-dev release prepare X.Y.Z | notes vX.Y.Z` : numéro de version dans `rust/Cargo.toml`, `pyproject.toml` et `__init__.py` tant qu'ils existent, CHANGELOG ; remplace `scripts/release.py`) et `icons.rs` (`otb-dev icons` : PNG Linux et `.ico` Windows refaits depuis `opentunebox.svg` par `resvg` et `ico`, Rust pur ; remplace `packaging/make_icons.py`, sans `.icns` : D-042) ; phase 7-3, `src/corpus/` : outils du corpus de vrais logs (`otb-dev corpus <outil>`) ; `mod.rs` est le socle (liste des logs sans doublons ni `__MACOSX`, passage parallèle rayon avec reprise des fichiers en échec, sessions d'au moins 8 s, options) et `rates.rs` porte `scripts/advisor_rates.py` (`corpus rates SORTIE.json`, `--resume`, `--compare AVANT APRÈS` : code 1 si un conseil diffère) ; `calibrate.rs` porte `scripts/calibrate_advisor.py` (`corpus calibrate NB_FICHIERS SORTIE.json` : mesures brutes de chaque session ; `--resume SORTIE.json` : p10 / p50 / p90 par taille ; Rust prend les premiers fichiers de la liste là où Python en tire au hasard) et `json.rs` lit et écrit les `NaN` / `Infinity` du JSON comme Python ; `labels.rs` porte `scripts/corpus_labels.py` (`corpus labels [--limit N] [--csv CHEMIN]` : tableau `corpus.csv`, une ligne par drone ; les colonnes `etat`, `taille_reelle` et `remarque` de Cédric ne sont jamais réécrites ; fichier écrit comme Python : BOM, `;`, `\r\n`) ; `validate.rs` porte `scripts/validate_corpus.py` (`corpus validate [--limit N] [--start N] [--decoder CHEMIN] [-j FILS]` : lance `blackbox_decode` avec un délai de 180 s, compare ses CSV à notre décodeur sur l'horodatage `time`, statuts `ok` / `gap` / `slow` / `rows` / `values` / `missing` / `noref` / `refhang` ; 4 fichiers à la fois par défaut, la mémoire montant à ≈ 1 Go))
- **otb-gui** (interface egui, phase 6 : `theme.rs` seul fichier à contenir des couleurs (les tables de palettes de `colormaps.rs`, générées par `scripts/colormap_tables.py`, ne contiennent que des octets), `tools/` (un fichier par outil : `spectral.rs`, `freq_throttle.rs`, `freq_time.rs` en 6-2 ; `step.rs` (Réponse indicielle), `pid_slider.rs` (Curseurs PID) et `step_common.rs` (calcul d'un fichier, graphique des curseurs) en 6-3 ; `advisor.rs` (Assistant), `setup.rs` (Configuration, lignes alignées par nom : D-038) et `filter_sim.rs` + `filter_sim_calc.rs` (Simulateur de filtres : l'interface et les calculs purs, sans egui ; `scripts/filter_sim_cases.py` écrit `rust/otb-gui/tests/data/filter_sim.json`) en 6-4 ; `common.rs` briques partagées (dont `ViewMemo`, la vue retenue d'un graphique : « Tout voir » l'oublie), `report_ui.rs` constats et diagnostic ; `tools::all_pages` les enregistre ; `cargo run --release -p otb-gui --example check_tools -- $OTB_LOGS/golden` les compare à Python, L-067, L-068), `state.rs` (logs chargés ; `Cache` des résultats par log, clé = réglages + période : D-037), `worker.rs` (file d'attente d'un calcul par outil), `tool.rs` (trait `Tool` : `compute` sans `self`), `plots.rs` (contournements d'egui_plot), `icons.rs`, `export.rs` (export PNG, 6-6a : nom `OTB_FIGS_AAAA-MM-JJ-N.png`, recadrage borné de la capture d'écran d'egui, écriture ; la boîte d'enregistrement est `App::save_dialog`, remplaçable dans les tests, et les tests de capture fournissent leur propre moteur de rendu car la CI n'a pas de carte graphique), `build.rs` (icône de l'exécutable Windows) et `main.rs` (pas de console sous Windows, icône de fenêtre), `app.rs` (écrans Guide / Courbes / outil, icône d'export de la fenêtre), `viewer.rs` (traces, lissage) et `viewer_page.rs` (page du visualiseur) ; `guide.rs` (Guide sans egui : fichier des fiches `drones.json`, mêmes octets et même chemin que Python, fichier illisible mis de côté, D-039 ; liens `otb:`, avancement, outils cités) et `guide_page.rs` (page Guide : accueil `accueil.md` de Python, phases, fiches, logs du drone vérifiés en arrière-plan ; elle rend des `GuideAction` que `App` exécute ; `App::new` prend un guide sans fichier, jamais le vrai `drones.json` dans les tests) ; `examples/capture.rs` fait des captures PNG sans fenêtre (rendu wgpu) ; `examples/banc_carte.rs` mesure `Heatmap` contre `PlotImage` ; banc de test : `with_context_setup(app::setup)`, L-066)
- **otb-py** (pont PyO3 → `opentunebox._native`)

## Hors périmètre (pour l'instant)

Logs KISS (`.TXT`) et INAV : W-001. Le reste de ce qui est vu et laissé de côté est dans les
[travaux reportés](travaux-reportes.md).
