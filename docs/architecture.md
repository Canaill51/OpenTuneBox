# Architecture

## Origine et objectif

PIDtoolbox Pro V0.94 est un logiciel MATLAB fermé et payant, disponible sous Linux seulement via le
MATLAB Runtime. OpenTuneBox couvre les mêmes outils avec un code entièrement réécrit, multiplateforme et
hors ligne, pour régler des drones de toutes tailles (whoops 65–75 mm jusqu'aux 8"+).

Le corpus de mise au point (`OTB_LOGS`, par défaut `~/Documents/Drones/Logs`) rassemble les vrais logs Betaflight de
Cédric (4.4.2 → 2026.6.2, DShot bidirectionnel, majoritairement 2 kHz, debug `FFT_FREQ`) et les captures PIDtoolbox
(`PTB_FIGS_*`) qui servent de référence visuelle. Il grossit à chaque vol : ses chiffres à jour (nombre de fichiers,
de sessions et de captures) sont dans [validation.md](validation.md), qui en est le maître — les recopier ici
revenait à les laisser vieillir sans que personne ne s'en aperçoive.

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
| `filter_stages.rs` | Contrôle du modèle de filtres étage par étage contre le firmware (canaux debug `GYRO_SAMPLE` / `FFT_FREQ`, terme D), par bande de fréquences ; lot 22 du plan, D-057 |
| `filter_replay.rs` | Simulateur de la page Filtres : chaîne du firmware rejouée sur le gyro non filtré (règles du contrôle par étage, suivi du notch dynamique), spectres avant / après (« actuel » rejoué, « modifié » = log × rejeu modifié / rejeu actuel) ; `replay_from` reprend le rejeu actuel quand seuls les filtres du D changent ; lot 22e, D-058 ; sert aussi à l'assistant, D-059 |
| `dyn_notch.rs` | Suivi des pics du notch dynamique rejoué comme le firmware (SDFT, pics, lissage, variantes par version), centres par axe, contrôle contre les centres enregistrés en `FFT_FREQ` ; lot 22c du plan, W-057 |
| `bf_filters.rs` | Filtres Betaflight exacts (PT1/2/3, biquad, SVF, RPM, notch dynamique, LPF dynamiques), chaînes gyro et D, spectre « et si » par les réponses \|H\|² (assistant, logs sans gyro non filtré), passe-bas relevé ou coupé ; noyaux de calcul dans `rust/otb-dsp` (D-032) |
| `numeric.rs` (d'otb-dsp), `npstat.rs` | Primitives numériques (fenêtre de Hann, moyenne glissante, flou gaussien, intercorrélation : D-032) et statistiques de numpy sur des `float32` dans `npstat.rs` (somme par paires, moyenne, écart-type, médiane, centile, histogramme) |
| `sliders.rs` | Curseurs simplifiés ⇄ PID et filtres, commandes CLI |
| `motors.rs` | Fréquences moteur depuis l'eRPM, harmoniques, centres des notchs dynamiques, bruit moteur, hauteur des harmoniques et poids RPM juste suffisant, pics à suivre par le notch dynamique, raies hors moteurs (vol stationnaire) ; boucles par fenêtre génériques f32 / f64 (D-034) |
| `stats.rs`, `period.rs` | Statistiques de vol, erreur PID robuste, période automatique, détection de crash |
| `datasets.rs` | Jeux de données analysables (gyro, gyro préfiltre, terme D…, comme les listes de PIDtoolbox) |

## Interface

Interface egui + egui_plot (`rust/otb-gui/src/`), D-035.

- `state.rs` : logs ouverts (10 maximum, une couleur chacun), log courant, période d'analyse et fragment de la bibliothèque
  correspondant (`LoadedLog::fragment`, D-065) ; `Cache` des résultats par log, clé = réglages + période (D-037). Un
  changement relance les outils ouverts. `period_of_range` passe des bornes d'un fragment (échantillons) à la période (secondes).
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

  `rust/otb-gui/src/tools/common.rs` : briques partagées (dont `ViewMemo`, la vue retenue d'un graphique : « Tout voir » l'oublie,
  et `space_tabs` : les onglets d'un espace, avec `ESPACE_BRUIT` et `VUES_BRUIT` — le nom de l'espace « Bruit et filtres » et ses
  quatre vues, D-060).
- `dashboard.rs` : le **tableau de bord** (lot 30c), premier écran du rail. Une carte par drone connu de la bibliothèque
  (`otb-store`), **au dessin du prototype** (D-061) : badge teinté de la couleur du drone, nom et sous-titre, pastille d'état,
  nombre de fragments et date du dernier vol côte à côte, étape du parcours et barre d'avancement ; la carte choisie prend un
  trait bleu de 2 points et une ombre portée. Les mesures relevées dans la maquette sont des constantes nommées en tête du
  fichier. Le panneau de droite montre les **fragments** du drone choisi, du plus récent en haut, chacun derrière un rond
  « + » (ou une coche quand son vol est déjà ouvert) et suivi d'un crayon qui ouvre l'écran d'extraction sur lui
  (`DashAction::Edit`). Un drone est reconnu par le **nom de machine** de
  l'en-tête (`Craft name`) tant que la récupération par USB n'apporte pas l'identifiant de la carte (lot 32).
- `app.rs` : fenêtre principale, écrans Tableau de bord / Guide / outil, et l'écran d'extraction (`Screen::Viewer`), hors
  rail (D-065) : il a sa propre barre du haut, retient l'écran d'où l'on vient (`App::return_to`) et y revient. Ouvrir un
  fichier range ses sessions comme fragments (`remember`) : bornes **choisies** gardées, sinon limites automatiques ; des
  bornes choisies deviennent la période d'analyse du log ouvert (`accepte`), et `save_fragment` les enregistre. L'application tient la bibliothèque
  (`App::library`) : **en mémoire** partout sauf au lancement réel, pour qu'aucun banc de test n'écrive celle de l'utilisateur ;
  chaque fichier ouvert y est rangé (`remember`), et elle est écrite quand eframe enregistre. La navigation est un **rail à gauche** (`rail`, largeur
  `RAIL_WIDTH`, D-060) : une entrée par outil ou par **espace**, les vues d'un espace (`HIDDEN`) n'en ayant pas à elles —
  c'est l'entrée nommée par leur `TAB` qui reste mise en valeur. La barre du haut garde ce qui agit sur les logs et sur la
  fenêtre (ouvrir, fermer, thème, export), et la page exportée commence au bord droit du rail. Un outil caché ne
  recalcule qu'à son affichage. `main.rs` : démarrage (pas de console sous Windows, icône de fenêtre, ouvre les logs passés en
  argument) ; le titre de la fenêtre vient d'`app.rs:window_title` (nom + numéro de version du paquet), tandis que
  l'`app_id` garde le nom seul, qui est celui du fichier `.desktop` (L-076).
- `viewer.rs` (traces, lissage) et `viewer_page.rs` : l'**écran d'extraction d'un fragment** (lot 29b, D-065), au dessin du
  prototype — barre du haut (retour, « Modifier le fragment … », Annuler, Enregistrer), graphiques avec les bornes, leurs
  onglets et leurs poignées, barre de navigation sur le log entier (fenêtre visible déplaçable, fragment en bleu), panneau de
  droite à sections repliables (Fragment, Traces, Affichage, Lecteur, Au curseur). Les bornes vivent dans un brouillon
  (`Draft`) toujours posé sur un échantillon (`snap_bounds`) ; l'écran rend des `ExtAction` que `App` exécute. Chaque courbe affichée est gardée en cache avec son index multi-résolution
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
- Exemples d'`otb-gui` : `examples/capture.rs` (captures sans fenêtre, rendu wgpu) ; `examples/banc_carte.rs` mesure
  `Heatmap` contre `PlotImage` ; `examples/banc_cadre.rs` mesure le temps de cadre de l'écran Guide et du viewer sur un
  vrai log (W-049, `docs/validation.md`) ; `examples/banc_cache.rs` mesure ce que **pèse une entrée du cache des
  outils** sur tout le corpus, avec un allocateur qui compte les octets vivants (W-035 point 9, `docs/validation.md`) —
  il dépend d'`otb-dev` (liste des logs du corpus) **en dépendance de développement seulement** ; `examples/check_tools.rs` **rejoue les sept outils sur les références figées
  et compare chaque clé** (`cargo run --release -p otb-gui --example check_tools -- $OTB_LOGS/golden`) — c'est la seule
  preuve que les outils de l'interface rendent ce que rendait la version Python sur de vrais vols, et ce n'est pas un
  test : la CI le compile sans le lancer (W-054).
- Bancs d'`otb-dsp` (mesures, jamais livrés) : `examples/banc_4b0.rs` et `examples/banc_4c0.rs` (primitives numériques,
  L-051) et `examples/banc_minmax.rs` (index multi-résolution du viewer, D-048).
  Banc de test : `with_context_setup(app::setup)` (L-066).

## Distribution

- Installateurs, un par système, fabriqués et essayés par `.github/workflows/construire.yml`, envoyés dans un
  brouillon de la page Releases à chaque étiquette `v…`, publiée quand les deux y sont (sans *Artifacts* :
  `otb-dev release`, procédure dans `CONTRIBUTING.md`) ; macOS abandonné (D-042). Notes de la page : liste que GitHub génère des PR
  fusionnées depuis la dernière version publiée, rangées par leur étiquette `RN: …` (`.github/release.yml`, contrôlée sur chaque PR par
  `.github/workflows/etiquette-pr.yml`), traduite par `otb-dev release notes` (D-056). Le workflow compile
  `cargo build --release -p otb-gui` (Windows : `crt-static`), vérifie les DLL demandées, démarre le programme 20 s avec un petit log,
  installe / lance / désinstalle l'installateur, et démarre l'AppImage sous un écran virtuel (xvfb) avec le pilote Vulkan logiciel lavapipe :
  Windows `packaging/windows/opentunebox.iss` (Inno Setup ; un seul fichier, l'exécutable Rust installé sous le nom `OpenTuneBox.exe` ; 7,5 Mo mesurés par la CI le 2026-11-03),
  Linux `packaging/linux/build_appimage.sh` (AppImage de l'exécutable Rust `opentunebox-gui`, appimagetool 1.9.1 et runtime type2 figé `20251108`, tous deux vérifiés par leur SHA-256 dans `construire.yml`, construite sur Ubuntu 22.04 ; 10 Mo mesurés le 2026-11-03 ; son `AppRun` écrit le raccourci du menu et l'icône dans `~/.local/share` au premier lancement, jamais par-dessus un fichier existant, W-045).
- Icônes : `otb-dev icons` fabrique le `.png` (Linux) et le `.ico` (Windows) depuis `rust/otb-gui/assets/opentunebox.svg`.
- Depuis les sources : `packaging/linux/install_linux.sh` (compile `otb-gui`, l'installe dans `~/.local/bin` et ajoute le raccourci du menu Linux) ;
  `packaging/windows/construire_windows.bat` fabrique l'exécutable Rust (cargo, bibliothèque C embarquée) et, si Inno Setup est là, l'installateur sur un PC Windows.

## Espace de travail Rust

Espace de travail Cargo (D-031), `rust/`. Une entrée par crate. Les fichiers de cas et de références
(`rust/*/tests/data/*.json`, deux d'entre eux compressés en `.json.gz` : D-052, et `$OTB_LOGS/golden`) ont été écrits par l'ancienne version Python (tag `python-final`) : pour en refaire un, voir
« Refaire une référence Python » dans [validation](validation.md).

- **otb-decoder** : décodeur, Rust pur. `header.rs` (en-têtes et sessions), `file.rs` (`decode_file` de bout en bout ; `decode_bytes_compact` range les trames principales en `CompactTable`, `i32` sauf `loopIteration` et `time`, pour `otb_log::flightlog::load` : L-081), `error.rs` (`DecodeError` : en-tête hors bornes, mémoire insuffisante ; D-044), `frame.rs`
  (prédicteurs et lecture d'une trame ou d'un événement), `reader.rs` (session décodée).
- **otb-log** : rejet des trames impossibles et conversions float32 de l'ouverture d'un log (D-034) ; `debug_tables.rs` et `modes.rs`
  (tables de debug et de modes), `settings.rs` (`parse_settings`), `flightlog.rs` (`FlightLog`, `from_session`, `load`),
  `parallel.rs` (`load_many` : plusieurs fichiers ouverts à la fois, au plus `MAX_PARALLEL_FILES` = 3 décodés en même temps,
  ordre des fichiers conservé ; D-051). Dépendances : otb-decoder, otb-dsp et rayon.
- **otb-dsp** : noyaux des filtres Betaflight, de `step_response`, primitives numériques (`numeric.rs`) et décimation min/max des
  courbes (`minmax.rs` : `MinMaxPyramid`, index multi-résolution par courbe, `window_points` en O(points rendus) ; D-048) ;
  dépendances realfft et rayon (D-033).
- **otb-format** : formats de nombres (`{:.2f}`, `%g`, `repr`, `round`, D-035), pour que les textes restent identiques à ceux de l'ancienne version.
- **otb-store** : la bibliothèque de données (lot 28), en **fichiers JSON** et non en base — tranché par Cédric le 2026-11-18,
  le volume étant petit et une base embarquée apportant un compilateur C et une frontière FFI (D-054). Six tables dans
  `<données>/OpenTuneBox/OpenTuneBox/bibliotheque/` : drones (clé = identifiant de la carte), logs connus (copiés ou laissés
  en place), fragments de vol, configurations (`diff all` datés, gardés seulement quand ils changent), mesures (une par
  fragment, axe et grandeur, avec l'empreinte des réglages et la **version de calcul** qui les périme) et liste d'analyse.
  `model.rs` : les tables et leur écriture JSON, conversion à la main et lecture tolérante ; un fragment porte `chosen`
  (bornes choisies dans l'écran d'extraction, sinon elles suivent les limites automatiques, D-065). `link.rs` : la période d'un
  fragment dans un log ouvert (`range_of`, en **numéros d'échantillon**), avec un message clair quand le fichier a changé
  depuis — sans dépendre du décodeur, l'appelant passant `FlightLog::n()`. La liste d'analyse est tenue par la
  bibliothèque : ordre, **indice** de couleur attribué à l'entrée et jamais changé ensuite (les couleurs restent dans
  `theme.rs`, L-016), actif ou non, et un fragment de travail qui doit appartenir à la liste. **Un seul module d'accès** : le
  reste du programme ne voit jamais un fichier, ce qui limitera à ce fichier le passage à une vraie base le jour où le
  déclencheur du plan sera atteint. Un fichier illisible ou d'une version plus récente est **mis de côté**, jamais écrasé
  (même règle que les fiches drone, D-039). Mesures de volume dans [validation](validation.md). Dépendance : serde_json.
- **otb-usb** : récupération des logs par USB, **partie protocole** (lot 32a, D-063 ; l'écran viendra au lot 32b). Ce que
  fait le firmware : [spec-usb](spec-usb.md). `frame.rs` : trames MSP v1 (« jumbo » dès 255 octets) et v2 (CRC-8 DVB-S2),
  écrites et lues octet par octet. `link.rs` : le trait `Link` (envoyer, recevoir dans un délai) et `SerialLink`, le vrai
  port par `serial2`, **réservé** par un verrou exclusif sous Linux (Windows ne laisse qu'un propriétaire à un port COM) ;
  erreurs d'ouverture expliquées au pilote (`classify_open_error` : groupe `dialout`, port tenu par le configurateur, pilote
  Windows) ; `board_candidates` écarte les ports `ttyS*` sans matériel. `client.rs` : `Msp` — identité (`identify` s'arrête
  si ce n'est pas `BTFL`, D-062), état armé, résumé et lecture de la mémoire par blocs (`copy_flash`, avec progression et
  arrêt) ; la liste fermée `Command` ne contient **aucune écriture de réglage**, et `request` reste interne à la crate.
  `erase.rs` : les contrôles avant effacement et **le seul chemin qui efface** (`erase_flash`) : `FlashCopy` →
  `verify_and_save` (fichier neuf écrit, forcé sur le disque et relu à l'identique ; taille = place annoncée ; chaque début
  de log du fichier relu donne au moins un fragment) → `VerifiedCopy::confirm` (accord du pilote) → `ErasePermit`, consommé
  par `erase_flash`, qui relit juste avant d'envoyer : même carte (identifiant, ou même connexion s'il est nul), place
  occupée inchangée, drone désarmé **en dernier** ; puis attente de « prête et vide ». Un contrôle échoué garde le fichier
  et refuse l'effacement. `cli.rs` : `diff_all` (entre dans le CLI, lit jusqu'à l'invite **suivie d'un silence**, sort par
  `exit`, qui redémarre la carte : la connexion est consommée) et `config_changed`. `sim.rs` : `SimBoard`, carte simulée
  pour les tests (aucun matériel en CI), avec pannes à la demande (`Fault`), vol enregistré pendant la copie, effacement
  sans effet. Tests : `trames.rs`, `recuperation.rs` (parcours complet et chaque refus), `contrats.rs` (commandes figées,
  un seul envoi de l'effacement), `port_serie.rs` (Linux : le vrai `SerialLink` par un pseudo-terminal relié à la carte
  simulée, et le second accès refusé). Dépendances : otb-decoder, otb-log, serial2 (BSD-2-Clause OR Apache-2.0).
- **otb-analysis** : voir « Modules d'analyse » ; dépend d'otb-dsp et de rayon (L-041, L-042).
- **otb-advisor** : l'assistant. `profiles.rs` (profils de taille et seuils), `rules.rs` (constats), `noise.rs`, `step_tuning.rs`,
  `drone_card.rs`, `log_check.rs`, `tuning_path.rs` (voir « Flux de données ») ; `pyclasses.rs` : tables de caractères (`\d`, `\w`) qui
  reproduisent les classes d'expressions régulières de Python, nécessaires pour que les noms de drone se comparent comme avant.
- **otb-cli** : binaire `opentunebox info` / `report` ; `lib.rs` produit le texte, `main.rs` l'écrit au fil de l'eau.
- **otb-gui** : l'interface (voir « Interface ») ; `colormaps.rs` : palettes des cartes de fréquence (tables d'octets, licences en tête du fichier).
- **otb-parity** : comparateur avec les références de l'ancienne version (D-035). Onze modes : `check` (fonctions par nom)
  et `list`, puis une couche par étage — `check-header`, `check-decoder`, `check-settings`, `check-flightlog`,
  `check-sliders`, `check-analysis`, `check-advisor`, `check-guide`, `check-tuning`, `check-cli`. Tous demandent le
  dossier des références et `$OTB_LOGS` ; seuls `check_decoder` et `check_header` ont des tests (W-054) ; modules : `registry.rs` (fonctions rejouables par nom), `value.rs`
  (valeurs des fichiers de référence), `compare.rs`, `layers.rs` (couches par fichier du corpus : en-têtes), `decoded.rs` et
  `flightlog_digest.rs` (empreintes des sessions décodées et des `FlightLog`), `settings_json.rs` (`Settings` en JSON), `analysis_layer.rs`,
  `advisor_layer.rs`, `cli_layer.rs`, `guide_layer.rs` et `tuning_layer.rs` (une couche par famille de références, L-062).
- **otb-dev** : outils de développement, jamais livrés.
  - `docs.rs` : les contrôles de la documentation (liens, chemins et symboles cités, outils, guide, modules, index de `docs/`, fichiers
    publiés, commandes de la CLI, langue, registres D / L / W, seuils de `docs/assistant.md`, et **concordance des seuils
    recopiés avec leur constante Rust** : une valeur changée dans le code et oubliée dans un document est signalée). `rust/otb-dev/tests/docs.rs` les lance sur
    la vraie documentation (`OTB_DOCS_ROOT` désigne une autre racine) et `rust/otb-dev/tests/checks.rs` prouve que chacun échoue sur un
    cas fabriqué. Les registres d'histoire (`HISTORICAL_DOCS` : decisions, lecons, migration-rust, travaux-reportes) peuvent
    citer des chemins Python disparus ; les autres documents ne citent que des chemins qui existent.
  - `verify.rs` : `otb-dev verify [--quick] [--strict]` enchaîne `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --no-fail-fast` (tout l'espace de travail, et non plus une liste de `-p` à tenir à jour), le contrôle des mentions de licences (`licenses::check`, dans le processus : aucune commande lancée) puis `cargo deny check` (avertissement s'il manque ; sauté par `--quick`) ; durée de chaque étape, code de sortie 0 seulement si tout passe. Un test qui se saute faute de corpus écrit `[OTB-SAUT]` sur la sortie standard : `verify` les compte et `--strict` en fait un échec. Deux contrôles d'`otb-dev` cherchent eux-mêmes les sauts (et les fichiers réservés à un système, annoncés par `sauts_par_systeme.rs`), pour qu'un nouveau test conditionnel ne passe pas inaperçu.
  - `bench_open.rs` : `otb-dev bench-open LOG [ESSAIS]` ouvre un log (`otb_log::flightlog::load`) et affiche durées et pic de mémoire (`VmHWM` de `/proc/self/status`, Linux seulement) ; un processus par log, car le pic ne redescend pas.
  - `usb.rs` : `otb-dev usb ports | info PORT | copy PORT DOSSIER [--erase] | diff PORT FICHIER` : essai à la main de `otb-usb` sur une
    vraie carte avant l'écran (lot 32b), et mesures de [spec-usb](spec-usb.md) (débit de la copie, durée de l'effacement). L'effacement
    passe par les mêmes contrôles que l'application et demande de taper « effacer ». L'identifiant de la carte n'est montré que par une
    empreinte courte (`chip_id_summary`) : c'est un numéro de série du matériel, et la sortie se recopie dans le dépôt public (alerte
    CodeQL de la PR #169).
  - `release.rs` : `otb-dev release prepare X.Y.Z` (numéro de version dans `rust/Cargo.toml`) et `otb-dev release notes vX.Y.Z FICHIER` (contrôle
    du numéro, puis `translate_generated_notes`, fonction pure : traduit la liste générée par GitHub, ajoute le bloc Téléchargements,
    refuse une liste vide ou restée en anglais ; D-056).
  - `icons.rs` : `otb-dev icons` (PNG Linux et `.ico` Windows depuis `opentunebox.svg`, par `resvg` et `ico`, Rust pur ; pas de `.icns` : D-042).
  - `licenses.rs` : `otb-dev licenses` écrit `THIRD-PARTY-LICENSES.txt` à la racine : les fichiers de licence de chaque crate que `otb-gui` embarque (dépendances normales pour Linux et Windows, via `cargo metadata`, sans construction, test ni macros procédurales), les polices (Hack, Ubuntu Light, logos d'egui) et les icônes Lucide ; les textes identiques sont écrits une fois. Un composant dont le paquet n'a aucun fichier de licence prend les textes copiés de son dépôt d'origine (`rust/otb-dev/upstream-licenses/`, table `rust/otb-dev/src/licenses.rs:UPSTREAM`, provenance et commits dans `PROVENANCE.txt` ; lignes de copyright relevées dans les sources quand le texte MIT n'en porte pas) ; sinon il est listé avec sa licence et ses auteurs déclarés, ce que `rust/otb-dev/tests/licenses.rs` refuse pour le fichier du dépôt. L'installateur Windows et l'AppImage livrent ce fichier (W-043). À refaire à chaque changement de dépendance (voir « Publier une version » dans `CONTRIBUTING.md`).
    `otb-dev licenses --check` refait le même texte en mémoire et le compare au fichier du dépôt sans jamais l'écrire : `licenses::compare` est pure (elle rend l'écart et la ligne en cause), `verify` en fait une étape et `tests.yml` la lance sous Ubuntu et sous Windows, de sorte qu'une dépendance ajoutée ou changée ne passe pas sans mettre les mentions à jour. Le fichier est identique sous les deux systèmes : les composants des deux cibles sont demandés à `cargo metadata --filter-platform` depuis n'importe quel poste.
  - `src/corpus/` : outils du corpus de vrais logs (`otb-dev corpus <outil>`). `mod.rs` est le socle (liste des logs sans doublons ni
    `__MACOSX`, passage parallèle rayon avec reprise des fichiers en échec, sessions d'au moins 8 s, options) ; `rates.rs`
    (`corpus rates SORTIE.json`, `--resume`, `--compare AVANT APRÈS` : code 1 si un conseil diffère) ; `calibrate.rs`
    (`corpus calibrate NB_FICHIERS SORTIE.json` : mesures brutes de chaque session ; `--resume SORTIE.json` : p10 / p50 / p90 par taille) ;
    `json.rs` (lit et écrit les `NaN` / `Infinity` du JSON) ; `labels.rs` (`corpus labels [--limit N] [--csv CHEMIN]` : tableau
    `corpus.csv`, une ligne par drone ; les colonnes `etat`, `taille_reelle` et `remarque` de Cédric ne sont jamais réécrites ; fichier
    écrit avec BOM, `;`, `\r\n`) ; `validate.rs` (`corpus validate [--limit N] [--start N] [--decoder CHEMIN] [-j FILS]` : lance
    `blackbox_decode` avec un délai de 180 s, compare ses CSV à notre décodeur sur l'horodatage `time`, statuts `ok` / `gap` / `slow` /
    `rows` / `values` / `missing` / `noref` / `refhang` ; 4 fichiers à la fois par défaut, la mémoire montant à ≈ 1 Go) ; `stages.rs`
    (`corpus stages SORTIE.json [--dir SOUS-DOSSIER] [--limit N] [-j FILS]` : contrôle du modèle de filtres étage par étage,
    une ligne par session, étage et axe ; `--resume SORTIE.json` : tableau par étage, version et type de filtre, alertes du
    critère d'arrêt du lot 22) ; `dyntrack.rs` (`corpus dyntrack SORTIE.json [--dir SOUS-DOSSIER] [--limit N] [-j FILS]` :
    suivi du notch dynamique rejoué contre les centres enregistrés en `FFT_FREQ`, une ligne par session et par notch ;
    `--resume SORTIE.json` : tableau par version et cadence du log) ; `replay.rs` (`corpus replay SORTIE.json [--dir SOUS-DOSSIER]
    [--limit N] [-j FILS]` : rejeu de la page Filtres avec les réglages du log contre le spectre enregistré, gyro et D, par
    bande ; `--resume SORTIE.json` : tableau selon que le log enregistre chaque boucle PID ou non, D-058) ; `predict.rs`
    (`corpus predict AVANT APRÈS` : filtres du vol APRÈS appliqués au vol AVANT, rejeu et rapport des |H|² contre le vol
    APRÈS, et changement du gyro non filtré, par bande ; lot 22f, W-060).

## Hors périmètre (pour l'instant)

Logs KISS (`.TXT`) et INAV : W-001. Le reste de ce qui est vu et laissé de côté est dans les
[travaux reportés](travaux-reportes.md).
