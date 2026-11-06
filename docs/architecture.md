# Architecture

## Origine et objectif

PIDtoolbox Pro V0.94 est un logiciel MATLAB fermé et payant, disponible sous Linux seulement via le
MATLAB Runtime. OpenTuneBox couvre les mêmes outils avec un code entièrement réécrit, multiplateforme et
hors ligne, pour régler des drones de toutes tailles (whoops 65–75 mm jusqu'aux 8"+).

Le corpus de mise au point (`OTB_LOGS`, par défaut `~/Documents/Drones/Logs`) contient environ 780 sessions
(582 fichiers, Betaflight 4.4.2 → 2026.6.2, DShot bidirectionnel, majoritairement 2 kHz, debug `FFT_FREQ`)
et 638 captures PIDtoolbox (`PTB_FIGS_*`) servant de référence visuelle.

## Décisions

Les choix structurants (interface egui, noyau Rust, assistant hors ligne, aucune copie de
PIDtoolbox, langue, licence, méthodes de mesure…) sont dans le [journal des décisions](decisions.md), avec
leur contexte, les options écartées et ce qui les prouve. L'application était d'abord écrite en Python (Qt, numba) ;
elle a été portée en Rust module par module, chaque module prouvé équivalent à l'ancien ([migration vers Rust](migration-rust.md)).
Le dernier état Python est le tag `python-final` (références : `python-etalon`) ; voir « Refaire une référence Python » dans
[validation](validation.md).

## Flux de données

```
fichier .bbl/.bfl
  └─ rust/otb-decoder/src/header.rs   en-têtes H, définitions de trames, débuts de sessions
  └─ rust/otb-decoder/src/frame.rs    prédicteurs et lecture d'une trame ou d'un événement I/P/S/G/H/E
  └─ rust/otb-decoder/src/reader.rs   session décodée (entiers bruts, trames S propagées) ; file.rs : `decode_file` de bout en bout
       └─ rust/otb-log                FlightLog : temps, gyro, consigne, P/I/D/F, moteurs, gaz, debug, tension…
                                      en unités physiques ; trames impossibles écartées, conversions float32 (D-034) ;
                                      settings.rs normalise l'en-tête
            └─ rust/otb-analysis      calculs purs (aucune interface)
                 ├─ rust/otb-advisor/src/rules.rs        constats (clé stable `Finding.key`, D-028) + actions, seuils par profil
                 │                        (profiles.rs) ; `analyze` par sections (enregistrement, réponse,
                 │                        bruit, retards, filtres, moteurs, curseurs) sur un état partagé
                 ├─ noise.rs              diagnostic du bruit de la fenêtre Spectre (constats de rules.rs rangés
                 │                        par partie : conditions, gyro, terme D, stratégie ; niveaux avant/après)
                 ├─ step_tuning.rs        diagnostic du réglage de la Réponse indicielle (séries de vols :
                 │                        équilibre P/D, Master, roll / pitch ; step_series.rs d'otb-analysis)
                 ├─ log_check.rs          vérification des logs ouverts pour le guide intégré : parties du vol
                 │                        (`flight_parts` de period.rs), protocole des mouvements, usages,
                 │                        corrections (configurateur d'abord, CLI en repli), repères des filtres
                 ├─ drone_card.rs         fiche drone (version, taille, boucle PID, ESC) et valeurs qui en découlent
                 ├─ tuning_path.rs        parcours du guide : 4 phases de cases, état automatique d'après les logs
                 ├─ rust/otb-cli          opentunebox info / report
                 └─ rust/otb-gui/src/tools/*   une page par outil (calcul en arrière-plan, affichage egui_plot)
```

## Modules d'analyse

Dans `rust/otb-analysis/src/` (noyaux de calcul dans `rust/otb-dsp/src/`).

| Module | Rôle |
|---|---|
| `spectral.rs` | PSD de Welch, fréquence × gaz (fenêtres 300 ms, 100 tranches), fréquence × temps |
| `step.rs` | Réponse indicielle par déconvolution de Wiener (noyau dans `rust/otb-dsp/src/step.rs`, D-033) ; métriques (pic, latence, montée, cassure, vagues), bande passante, et boucle ouverte par `open_loop.rs` |
| `step_series.rs` | Séries de vols à un seul curseur changé (Damping, Master, Pitch Tracking, Pitch Damping) et conditions du protocole (feedforward, mode Angle, curseur I) |
| `open_loop.rs` | Boucle ouverte L = G·(Ce + Cd) mesurée à partir des termes P, I, D, F (estimateur instrumental, jackknife), repli approché T/(1 − T) ; marges de phase et de gain, croisement, pic de sensibilité |
| `delay.rs` | Retards des filtres (phase pondérée par la cohérence) et consigne → gyro |
| `bf_filters.rs` | Filtres Betaflight exacts (PT1/2/3, biquad, SVF, RPM, notch dynamique, LPF dynamiques), chaînes gyro et D, spectre « et si », passe-bas relevé ou coupé ; noyaux de calcul dans `rust/otb-dsp` (D-032) |
| `numeric.rs` (d'otb-dsp), `npstat.rs` | Primitives numériques (fenêtre de Hann, moyenne glissante, flou gaussien, intercorrélation : D-032) et statistiques de numpy sur des `float32` dans `npstat.rs` (somme par paires, moyenne, écart-type, médiane, centile, histogramme) |
| `sliders.rs` | Curseurs simplifiés ⇄ PID et filtres, commandes CLI |
| `motors.rs` | Fréquences moteur depuis l'eRPM, harmoniques, centres des notchs dynamiques, bruit moteur, hauteur des harmoniques et poids RPM juste suffisant, pics à suivre par le notch dynamique, raies hors moteurs (vol stationnaire) ; boucles par fenêtre génériques f32 / f64 (D-034) |
| `stats.rs`, `period.rs` | Statistiques de vol, erreur PID robuste, période automatique, détection de crash |
| `datasets.rs` | Jeux de données analysables (gyro, gyro préfiltre, terme D…, comme les listes de PIDtoolbox) |

## Interface

Interface egui + egui_plot (`rust/otb-gui/src/`), D-035.

- `state.rs` : logs ouverts (10 maximum, une couleur chacun), log courant, période d'analyse ; `Cache` des résultats par log,
  clé = réglages + période (D-037). Un changement relance les outils ouverts.
- `worker.rs` : file d'attente d'un calcul par outil, en arrière-plan ; le résultat revient au thread de l'interface.
- `tool.rs` : trait `Page` (une page de la fenêtre principale) et trait `Tool` ; `Tool::compute` est une fonction associée
  sans `self` : elle ne peut pas lire les réglages de l'interface, elle ne reçoit que `Options` (copie des réglages au moment de la
  demande) et `LogInput`, deux types `Send` (le compilateur fait respecter la règle). `ToolPage` numérote chaque demande (`latest`) et ne garde que le résultat
  qui porte le dernier numéro : un calcul terminé après un changement de sélection ou après le vidage de l'état est jeté (C2).
- Outils (`tools/`, une page chacun dans la fenêtre principale, enregistrés par `tools::all_pages` dans `rust/otb-gui/src/tools/mod.rs`) :

  | Bouton | Module | Fenêtre |
  |---|---|---|
  | Spectre | `rust/otb-gui/src/tools/spectral.rs` | analyse spectrale |
  | Fréq. × gaz (dans Spectre) | `rust/otb-gui/src/tools/freq_throttle.rs` | fréquence × gaz, jusqu'à 4 colonnes |
  | Fréq. × temps (dans Spectre) | `rust/otb-gui/src/tools/freq_time.rs` | spectrogrammes, curseur lié au lecteur |
  | Indicielle | `rust/otb-gui/src/tools/step.rs` | réponse indicielle (+ `rust/otb-gui/src/tools/step_common.rs`, partagé avec les curseurs PID : calcul d'un fichier, graphique des curseurs) |
  | Curseurs PID | `rust/otb-gui/src/tools/pid_slider.rs` | curseurs du réglage simplifié et simulation |
  | Filtres | `rust/otb-gui/src/tools/filter_sim.rs` | simulateur de filtres (interface) ; `rust/otb-gui/src/tools/filter_sim_calc.rs` : calculs purs, sans egui |
  | Configuration (panneau de droite) | `rust/otb-gui/src/tools/setup.rs` | réglages des logs côte à côte (lignes alignées par nom : D-038) |
  | Assistant | `rust/otb-gui/src/tools/advisor.rs` | rapport de l'assistant de réglage ; `rust/otb-gui/src/tools/report_ui.rs` : constats et diagnostic |

  `rust/otb-gui/src/tools/common.rs` : briques partagées (dont `ViewMemo`, la vue retenue d'un graphique : « Tout voir » l'oublie).
- `app.rs` : fenêtre principale, écrans Guide / Courbes / outil, boutons de la barre qui servent d'onglets ; un outil caché ne
  recalcule qu'à son affichage. `main.rs` : démarrage (pas de console sous Windows, icône de fenêtre, ouvre les logs passés en
  argument).
- `viewer.rs` (traces, lissage) et `viewer_page.rs` (page du visualiseur : traces groupées, traces absentes du log masquées,
  sections repliables Affichage et Lecteur). Chaque courbe affichée est gardée en cache avec son index multi-résolution
  (`Series`, clé : trace, axe, lissage) ; le tracé n'appelle plus que `otb_dsp::minmax::window_points` (D-048).
- `guide.rs` : le Guide sans egui (fichier des fiches `drones.json` dans le dossier de données de l'utilisateur, fichier illisible
  mis de côté : D-039 ; liens `otb:`, avancement, outils cités). `guide_page.rs` : la page **Guide** (accueil
  `rust/otb-gui/assets/guide/accueil.md`, frise et liste des phases, fiches drone, cases décrites par `tuning_path.rs`, logs du drone
  vérifiés en arrière-plan par `log_check.rs`) ; elle rend des `GuideAction` que `App` exécute. Les liens `otb:phase/<clé>` et
  `otb:tool/<clé>` de l'accueil sont vérifiés par le test de documentation.
- `theme.rs` : thèmes clair et sombre, **seul fichier** à contenir des couleurs (les tables de palettes de `colormaps.rs` ne contiennent
  que des octets) ; `rust/otb-gui/tests/contracts.rs` refuse tout code couleur ailleurs (L-016).
- `icons.rs` : icônes Lucide (`rust/otb-gui/assets/icons/`, licence ISC) colorées selon le thème.
- `plots.rs` : contournements d'egui_plot. `export.rs` : export PNG (nom `OTB_FIGS_AAAA-MM-JJ-N.png`, recadrage borné de la capture
  d'écran d'egui ; la boîte d'enregistrement est `App::save_dialog`, remplaçable dans les tests). `build.rs` : icône de l'exécutable Windows.
- Captures sans fenêtre : `examples/capture.rs` (rendu wgpu) ; `examples/banc_carte.rs` mesure `Heatmap` contre `PlotImage` ;
  `examples/banc_cadre.rs` mesure le temps de cadre de l'écran Guide et du viewer sur un vrai log (W-049, `docs/validation.md`).
  Banc de test : `with_context_setup(app::setup)` (L-066).

## Distribution

- Installateurs, un par système, fabriqués et essayés par `.github/workflows/construire.yml`, envoyés dans un
  brouillon de la page Releases à chaque étiquette `v…`, publiée quand les deux y sont (sans *Artifacts* :
  `otb-dev release`, procédure dans `CONTRIBUTING.md`) ; macOS abandonné (D-042). Le workflow compile
  `cargo build --release -p otb-gui` (Windows : `crt-static`), vérifie les DLL demandées, démarre le programme 20 s avec un petit log,
  installe / lance / désinstalle l'installateur, et démarre l'AppImage sous un écran virtuel (xvfb) avec le pilote Vulkan logiciel lavapipe :
  Windows `packaging/windows/opentunebox.iss` (Inno Setup ; un seul fichier, l'exécutable Rust installé sous le nom `OpenTuneBox.exe` ; 7,5 Mo mesurés par la CI le 2026-11-03 ; le dossier `_internal` de l'ancienne version Python est supprimé à la mise à jour, W-044),
  Linux `packaging/linux/build_appimage.sh` (AppImage de l'exécutable Rust `opentunebox-gui`, appimagetool 1.9.1 et runtime type2 figé `20251108`, tous deux vérifiés par leur SHA-256 dans `construire.yml`, construite sur Ubuntu 22.04 ; 10 Mo mesurés le 2026-11-03).
- Icônes : `otb-dev icons` fabrique le `.png` (Linux) et le `.ico` (Windows) depuis `rust/otb-gui/assets/opentunebox.svg`.
- Depuis les sources : `packaging/linux/install_linux.sh` (compile `otb-gui`, l'installe dans `~/.local/bin` et ajoute le raccourci du menu Linux) ;
  `packaging/windows/construire_windows.bat` fabrique l'exécutable Rust (cargo, bibliothèque C embarquée) et, si Inno Setup est là, l'installateur sur un PC Windows.

## Espace de travail Rust

Espace de travail Cargo (D-031), `rust/`. Une entrée par crate. Les fichiers de cas et de références
(`tests/data/*.json`, `$OTB_LOGS/golden`) ont été écrits par l'ancienne version Python (tag `python-final`) : pour en refaire un, voir
« Refaire une référence Python » dans [validation](validation.md).

- **otb-decoder** : décodeur, Rust pur. `header.rs` (en-têtes et sessions), `file.rs` (`decode_file` de bout en bout ; `decode_bytes_compact` range les trames principales en `CompactTable`, `i32` sauf `loopIteration` et `time`, pour `otb_log::flightlog::load` : L-081), `error.rs` (`DecodeError` : en-tête hors bornes, mémoire insuffisante ; D-044), `frame.rs`
  (prédicteurs et lecture d'une trame ou d'un événement), `reader.rs` (session décodée).
- **otb-log** : rejet des trames impossibles et conversions float32 de l'ouverture d'un log (D-034) ; `debug_tables.rs` et `modes.rs`
  (tables de debug et de modes), `settings.rs` (`parse_settings`), `flightlog.rs` (`FlightLog`, `from_session`, `load`).
- **otb-dsp** : noyaux des filtres Betaflight, de `step_response`, primitives numériques (`numeric.rs`) et décimation min/max des
  courbes (`minmax.rs` : `MinMaxPyramid`, index multi-résolution par courbe, `window_points` en O(points rendus) ; D-048) ;
  dépendances realfft et rayon (D-033).
- **otb-format** : formats de nombres (`{:.2f}`, `%g`, `repr`, `round`, D-035), pour que les textes restent identiques à ceux de l'ancienne version.
- **otb-analysis** : voir « Modules d'analyse » ; dépend d'otb-dsp et de rayon (L-041, L-042).
- **otb-advisor** : l'assistant. `profiles.rs` (profils de taille et seuils), `rules.rs` (constats), `noise.rs`, `step_tuning.rs`,
  `drone_card.rs`, `log_check.rs`, `tuning_path.rs` (voir « Flux de données ») ; `pyclasses.rs` : tables de caractères (`\d`, `\w`) qui
  reproduisent les classes d'expressions régulières de Python, nécessaires pour que les noms de drone se comparent comme avant.
- **otb-cli** : binaire `opentunebox info` / `report` ; `lib.rs` produit le texte, `main.rs` l'écrit au fil de l'eau.
- **otb-gui** : l'interface (voir « Interface ») ; `colormaps.rs` : palettes des cartes de fréquence (tables d'octets, licences en tête du fichier).
- **otb-parity** : comparateur avec les références de l'ancienne version (`otb-parity check $OTB_LOGS/golden`, `check-header`,
  `check-settings`, `check-flightlog`, `check-decoder`…, D-035) ; modules : `registry.rs` (fonctions rejouables par nom), `value.rs`
  (valeurs des fichiers de référence), `compare.rs`, `layers.rs` (couches par fichier du corpus : en-têtes), `decoded.rs` et
  `flightlog_digest.rs` (empreintes des sessions décodées et des `FlightLog`), `settings_json.rs` (`Settings` en JSON), `analysis_layer.rs`,
  `advisor_layer.rs`, `cli_layer.rs`, `guide_layer.rs` et `tuning_layer.rs` (une couche par famille de références, L-062).
- **otb-dev** : outils de développement, jamais livrés.
  - `docs.rs` : les contrôles de la documentation (liens, chemins et symboles cités, outils, guide, modules, index de `docs/`, fichiers
    publiés, commandes de la CLI, langue, registres D / L / W, seuils de `docs/assistant.md`). `rust/otb-dev/tests/docs.rs` les lance sur
    la vraie documentation (`OTB_DOCS_ROOT` désigne une autre racine) et `rust/otb-dev/tests/checks.rs` prouve que chacun échoue sur un
    cas fabriqué. Les registres d'histoire (`HISTORICAL_DOCS` : CHANGELOG, decisions, lecons, migration-rust, travaux-reportes) peuvent
    citer des chemins Python disparus ; les autres documents ne citent que des chemins qui existent.
  - `verify.rs` : `otb-dev verify [--quick] [--strict]` enchaîne `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --no-fail-fast` (tout l'espace de travail : mêmes 74 exécutables qu'avec la liste des `-p`) puis `cargo deny check` (avertissement s'il manque ; sauté par `--quick`) ; durée de chaque étape, code de sortie 0 seulement si tout passe. Un test qui se saute faute de corpus écrit `[OTB-SAUT]` sur la sortie standard (`golden.rs`, `smooth.rs`) : `verify` les compte et `--strict` en fait un échec.
  - `bench_open.rs` : `otb-dev bench-open LOG [ESSAIS]` ouvre un log (`otb_log::flightlog::load`) et affiche durées et pic de mémoire (`VmHWM` de `/proc/self/status`, Linux seulement) ; un processus par log, car le pic ne redescend pas.
  - `release.rs` : `otb-dev release prepare X.Y.Z | notes vX.Y.Z` (numéro de version dans `rust/Cargo.toml`, CHANGELOG).
  - `icons.rs` : `otb-dev icons` (PNG Linux et `.ico` Windows depuis `opentunebox.svg`, par `resvg` et `ico`, Rust pur ; pas de `.icns` : D-042).
  - `licenses.rs` : `otb-dev licenses` écrit `THIRD-PARTY-LICENSES.txt` à la racine : les fichiers de licence de chaque crate que `otb-gui` embarque (dépendances normales pour Linux et Windows, via `cargo metadata`, sans construction, test ni macros procédurales), les polices (Hack, Ubuntu Light, logos d'egui) et les icônes Lucide ; les textes identiques sont écrits une fois, les composants sans fichier de licence sont listés avec leur licence et leurs auteurs déclarés. L'installateur Windows et l'AppImage livrent ce fichier (W-043). À refaire à chaque changement de dépendance (voir « Publier une version » dans `CONTRIBUTING.md`).
  - `src/corpus/` : outils du corpus de vrais logs (`otb-dev corpus <outil>`). `mod.rs` est le socle (liste des logs sans doublons ni
    `__MACOSX`, passage parallèle rayon avec reprise des fichiers en échec, sessions d'au moins 8 s, options) ; `rates.rs`
    (`corpus rates SORTIE.json`, `--resume`, `--compare AVANT APRÈS` : code 1 si un conseil diffère) ; `calibrate.rs`
    (`corpus calibrate NB_FICHIERS SORTIE.json` : mesures brutes de chaque session ; `--resume SORTIE.json` : p10 / p50 / p90 par taille) ;
    `json.rs` (lit et écrit les `NaN` / `Infinity` du JSON) ; `labels.rs` (`corpus labels [--limit N] [--csv CHEMIN]` : tableau
    `corpus.csv`, une ligne par drone ; les colonnes `etat`, `taille_reelle` et `remarque` de Cédric ne sont jamais réécrites ; fichier
    écrit avec BOM, `;`, `\r\n`) ; `validate.rs` (`corpus validate [--limit N] [--start N] [--decoder CHEMIN] [-j FILS]` : lance
    `blackbox_decode` avec un délai de 180 s, compare ses CSV à notre décodeur sur l'horodatage `time`, statuts `ok` / `gap` / `slow` /
    `rows` / `values` / `missing` / `noref` / `refhang` ; 4 fichiers à la fois par défaut, la mémoire montant à ≈ 1 Go).

## Hors périmètre (pour l'instant)

Logs KISS (`.TXT`) et INAV : W-001. Le reste de ce qui est vu et laissé de côté est dans les
[travaux reportés](travaux-reportes.md).
