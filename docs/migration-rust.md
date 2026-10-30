# Migration vers Rust

Plan et suivi de la réécriture d'OpenTuneBox en Rust, **sans régression** (décision D-031, travail reporté W-015).
Ce document ne vit que sur `feat/rust-migration` : `main` reste en Python jusqu'à la validation finale.

## Branches

```
main                          Python, publiable à tout moment
 └─ feat/rust-migration       branche d'intégration (longue durée)
      ├─ chore/rust-safety-net      phase 0 : références, outillage, décisions
      ├─ feat/rust-decoder          phase 1
      ├─ feat/rust-dsp              phase 2a (2b : nouvelle branche)
      └─ …                          une branche par module ou par étape
```

- Une branche par étape, créée depuis `feat/rust-migration` ; la PR va vers `feat/rust-migration` (jamais
  `main`) et son titre est le nom de la branche.
- Les correctifs de `main` sont rapatriés dans `feat/rust-migration` par `git merge main`, régulièrement et
  toujours avant une mesure de parité.
- Fin du projet : une seule PR `feat/rust-migration` → `main`, après validation complète (tests, parité sur
  tout le corpus, installateurs Linux, Windows et macOS) et accord de Cédric.

## Principe

**Phases 0 à 3** : chaque module Python est réécrit en Rust, branché par PyO3 (module `opentunebox._native`) puis prouvé
équivalent ; `OTB_BACKEND=python|rust` choisit l'implémentation le temps de la transition.

**Depuis la décision D-035 (phases 4 à 7)** : plus aucun Python à la fin, tout le dépôt en Rust. Le noyau est réécrit en
bibliothèques Rust autonomes, **sans pont PyO3 module par module** ; le Python de la branche reste intact et sert
d'**étalon figé** (tag `python-etalon`), par des références enregistrées sur le corpus (`golden.py record-modules`) que le
comparateur Rust `otb-parity` relit. L'interface devient egui + egui_plot, sous réserve d'un prototype d'essai. Espace de
travail Cargo :

```
rust/  otb-decoder  otb-dsp  otb-log  otb-analysis  otb-advisor  otb-cli  otb-gui  otb-parity (dev)
       otb-py (pont PyO3, supprimé à la phase 7)
```

## Preuves d'équivalence

| Couche | Exigence | Outil |
|---|---|---|
| Décodeur | identique au bit près sur tout le corpus | `scripts/golden.py` (empreintes), `scripts/validate_corpus.py` |
| Calculs et outils | écart mesuré, ≈ 1e-5 relatif (FFT différente de scipy) | `scripts/golden.py check --rtol X`, comparaison directe à scipy |
| Assistant | mêmes constats (clés, textes, chiffres) | `scripts/advisor_rates.py --compare` |
| Interface | mêmes thèmes, mêmes graphiques, Windows compris | captures hors écran, tests `test_gui_*` |

### Références figées (`scripts/golden.py`)

Enregistrées **avant** d'écrire du Rust, avec le code Python de `main`, dans `$OTB_LOGS/golden/` (hors du dépôt) :

```sh
.venv/bin/python scripts/golden.py record "$OTB_LOGS/golden"            # ≈ 15 min, plusieurs centaines de Mo
.venv/bin/python scripts/golden.py check  "$OTB_LOGS/golden"            # code 1 si une différence subsiste
.venv/bin/python scripts/golden.py check  "$OTB_LOGS/golden" --rtol 1e-5   # tolérance pour une réécriture de FFT
```

Trois couches : empreintes SHA-256 de chaque tableau décodé (tout le corpus), sortie de `compute` de chaque
outil du registre sur 20 logs répartis dans le corpus (tous les nombres), constats de l'assistant sur tout le
corpus. `record` décode deux fois et refait seul tout fichier discordant (inversions de bits du poste,
compétence `calculs-corpus`) ; `check` refait seul tout fichier qui diffère avant de le signaler.
Le filet a été éprouvé : une valeur altérée à la main est détectée (décodeur et outils).

## Phases

| # | Contenu | Risque | Preuve |
|---|---|---|---|
| 0 | Filet : références, décisions, CI de la branche | faible | `golden.py` éprouvé |
| 1 | Décodeur (`decoder/`) ; règle aussi W-014 et W-016 | faible | empreintes identiques, `validate_corpus.py` |
| 2a | scipy → numpy (`_numeric.py`), noyaux des filtres Betaflight en Rust (numba et scipy quittent l'application) | moyen | parité mesurée : outils 2,4e-10, filtres identiques |
| 2b | Pilote : noyau de `step_response` en Rust (la FFT seule ne rapporte que 5 %, D-033) | moyen | parité à tolérance mesurée ; gain ≥ 1,5× |
| 3a | Ouverture des logs en Rust (`_drop_implausible`, conversions float32) : une seule passe sur la matrice (D-034) | faible | sorties identiques au bit près ; chargement ≥ 1,5× |
| 3b | `motors.py` et `sections_response` en Rust (boucles par fenêtre entières) | moyen | parité mesurée ; assistant ≥ 1,25× à un fil |
| 3c | ~~`whatif_spectrum` (filter_sim)~~ ABANDONNÉE le 2026-10-30 : gain plafonné à 1,04× à un fil (D-034) | — | — |
| E1 | Prototype d'interface egui sur de vraies données (D-035) | moyen | tableau « point → supporté / contourné / bloquant » |
| E2 | Filet de parité : tag `python-etalon`, `record-modules`, `otb-parity`, formats de nombres, `PidTerms` sorti de `loop.py` | faible | références enregistrées, formats identiques sur ≥ 10⁶ valeurs |
| 4a | Décodeur (en-tête, lecture), `log/` (`debug_tables`, `modes`, `settings`, `FlightLog`) | moyen | décodage au bit près, réglages identiques, tableaux float32 identiques |
| 4b | `_numeric`, `spectral`, `bf_filters` (chaînes), `sliders` | moyen | tolérance de chaque module fixée avant le code |
| 4c | `delay`, `loop`, `step`, `step_series`, `stats`, `period`, `datasets`, `motors` | élevé | idem ; outils à ≤ 2,4e-10 déjà vus |
| 5a | Assistant : `profiles`, `rules`, `noise`, `step_tuning` | élevé | 0 session aux constats différents sur 630 |
| 5b | `drone_card`, `log_check`, `tuning_path` | moyen | idem + JSON des fiches identique |
| 5c | Ligne de commande en binaire Rust (`otb-cli`) | faible | rapports identiques caractère par caractère |
| 6 | Interface egui : socle, visualiseur, 8 outils, guide, 41 tests d'interface réécrits | très élevé | captures des 2 thèmes, contrats couleurs, Linux, Windows, macOS |
| 7 | Basculement : suppression de tout le Python, CI, installateurs, outil d'icônes en Rust | élevé | `git ls-files '*.py'` vide, 3 systèmes, accord de Cédric |

Règles de chaque étape (jusqu'à la phase 7 : tests Python de l'étalon et tests Rust verts) : `ruff` et `clippy` propres, un test de parité qui
échoue si le Rust diffère, aucun test affaibli, code Python supprimé seulement après recherche de ses usages
et parité prouvée, documentation et `CHANGELOG.md` à jour, pièges notés dans `lecons.md`.

Après la phase 2, **mesurer le poids réel des installateurs** (objectif n° 1) avant de poursuivre.

## Verdict du prototype d'interface (E1)

Prototype jetable `rust/otb-spike-egui` (egui 0.37, egui_plot 0.38, egui_commonmark 0.26, rfd 0.17, egui_kittest 0.37), hors de
l'espace de travail Cargo donc hors de la CI. Données : un vrai log de Cédric (Valkyrie `M1.4.bbl`, 407 334 points, 1 984 Hz),
exporté depuis Python. Mesures sur ce poste (Linux, Wayland, rendu logiciel pour les captures).

| Point | Verdict | Détail |
|---|---|---|
| Axe logarithmique | **contourné** | egui_plot n'en a pas : tracé en log10(f), graduations 1-2-5 et libellés refaits (≈ 20 lignes). Vérifié sur capture |
| Barres d'erreur | **contourné** | pas d'élément dédié : un segment vertical par point. Vérifié sur capture |
| Carte fréquence × temps + barre de couleurs | **contourné** | `PlotImage` sur une texture colorée par nos soins ; barre dessinée à la main (sans graduations pour l'instant). L'élément `Heatmap` d'egui_plot (tuiles à libellés) n'a pas été essayé : **non vérifié** pour 10⁴ cases et plus |
| Lignes et régions déplaçables | **contourné** | `VLine`, `HLine`, `Span` existent mais ne se glissent pas seuls : sélection et déplacement faits à la main (≈ 25 lignes). Prouvé par un test d'interface sans écran (L-043) |
| Second axe Y | **contourné** | `custom_y_axes` : axe de droite en échelle liée à celui de gauche ; ses graduations suivent celles de la gauche (valeurs non rondes : 103, 88, 74 %). Vérifié sur capture |
| Série de 1,2 M points | **supporté** | tracé direct : ≈ 20 ms par cadre (construction et tessellation, processeur seul, ≈ 50 images/s) ; avec décimation min/max recalculée à chaque cadre : ≈ 7 ms. Médianes de 7 cadres, version `release`. Le coût du GPU n'est pas mesuré |
| Texte riche | **supporté** (Markdown) | `egui_commonmark` : titres, gras, code, listes, citation, tableau, lien. **HTML non vérifié** : les textes du guide actuel en HTML seront à convertir en Markdown |
| Thèmes clair et sombre | **supporté** | les deux rendus vérifiés sur capture ; les contrôles de couleur en dur de `test_gui_contracts.py` sont à réécrire |
| Fenêtres de fichiers | **supporté, non essayé à la main** | `rfd` compile, le portail XDG est présent sur ce poste ; le clic n'a pas été fait. Windows et macOS non vérifiés |
| Test d'interface sans écran | **supporté** | `egui_kittest` : rendu en PNG dans les deux thèmes, souris simulée (survol, glisser, lâcher) |
| Fenêtre réelle | **supporté** | l'exécutable s'ouvre et tourne 6 s sans erreur sous Wayland (rien d'autre n'y a été essayé à la main) |

Taille de l'exécutable du prototype : 22,6 Mo après `strip` (30,8 Mo avant), profil `release` sans LTO.
Une fois l'interface complète écrite, elle sera plus grosse : à remesurer en phase 6 (objectif n° 1).

**Conclusion** : aucun point bloquant ; cinq contournements courts, à écrire une fois dans un module de graphiques commun
(`otb-gui`). Deux points restent à lever avant la phase 6 : `Heatmap` d'egui_plot contre `PlotImage` à grand nombre de cases,
et HTML.

## Avancement

| Phase | État |
|---|---|
| 0 | Fait le 2026-10-29 (branche `chore/rust-safety-net`) : références enregistrées (499 fichiers, 680 sessions, 630 sessions de l'assistant, 20 logs d'outils, 705 Mo) ; `check` relancé sur tout le corpus : identique |
| 1 | Fait le 2026-10-29 (branche `feat/rust-decoder`) : décodeur Rust par défaut, identique au bit près sur tout le corpus, 6 fois plus rapide ; relu par un agent indépendant (L-032). Reste : CI Windows à voir tourner après envoi sur GitHub |
| 2a | Fait le 2026-10-29 (branche `feat/rust-dsp`, D-032) : scipy remplacé par `analysis/_numeric.py`, noyaux des filtres Betaflight en Rust (`rust/otb-dsp`), scipy et numba hors de l'application (extra `dev`). Parité : décodeur 0 différence (499 fichiers), assistant 0 différence (630 sessions), outils : écart relatif maximal 2,4e-10 sur 20 logs. Relu par un agent indépendant (L-036, L-037) |
| Poids | **Mesuré le 2026-10-29** (Linux, `feat/rust-dsp`, environnement neuf `pip install . pyinstaller`, PySide6 6.12, numpy 2.5.4) : dossier PyInstaller **206 Mo, contre 463 Mo** avant la migration (−55 %) ; PySide6 102 Mo, numpy 14 Mo + `numpy.libs` 27 Mo (OpenBLAS de numpy), `opentunebox` 0,7 Mo ; ni scipy, ni numba, ni llvmlite ; l'exécutable reste ouvert 25 s. Reste à mesurer : installateurs compressés (AppImage, Windows, macOS), à lire sur `construire.yml` quand le quota d'artefacts GitHub le permet |
| 2b | Pilote `step_response` en Rust (branche `feat/rust-fft`, D-033) : noyau fait, 1,70× à un fil et 4,9× avec tous les cœurs ; parité sur le corpus : assistant 0 différence (630 sessions), outils 2,4e-10, relu par un agent indépendant (L-038). `_seg_spectra`, `loop_spectra`, `xcorr` : décidés après ce pilote (phase 3) |
| 3a | Fait le 2026-10-29 (branche `feat/rust-log-open`, D-034) : ouverture des logs en Rust (`rust/otb-log`). Chargement 838 → ≈ 530 ms (1,6×) sur 33,6 Mo, sorties identiques au bit près, mémoire conservée −10 %. Parité : assistant 0 différence (630 sessions), outils 2,4e-10. Relu par un agent indépendant (L-039, L-040) |
| 3b | Fait le 2026-10-29 (branche `feat/rust-motors`, D-034) : boucles par fenêtre de `motors.py` en Rust (`rust/otb-analysis`, génériques f32 / f64). Assistant 666 → 515 ms par session à un fil (1,29×), sorties identiques au bit près. Parité : assistant 0 différence (630 sessions), outils 0 à `rtol=1e-9`, décodeur 0 (499 fichiers). Huit mutations vues en échec (L-041, L-042) |
| 3c | Abandonnée le 2026-10-30 (D-034) : la construction des chaînes de filtres pèse 4,0 % de l'assistant à un fil (mesure sans profileur), gain maximal 1,04× ; mémorisation essayée en prototype : 1,06× |
| E0 | Décision D-035 écrite le 2026-10-30 (egui + egui_plot sous réserve du prototype, périmètre complet, noyau Rust autonome) ; W-019 repris |
| E1 | Fait le 2026-10-30 (branche `feat/egui-spike`) : prototype `rust/otb-spike-egui` sur un vrai log de 407 334 points ; verdict ci-dessous : aucun point bloquant. Reste à voir : essai à la main de la fenêtre de fichiers, Windows et macOS |
| E2 | Fait le 2026-10-30 (branche `feat/egui-spike`) ; tag local `python-etalon` posé sur le dernier commit de code Python (pas envoyé sur GitHub) ; `pytest` complet : 1 623 réussis, 8 ignorés. Détail : formats de nombres (`rust/otb-format`, 12 M de cas ≈ 1 M de valeurs identiques à Python, L-044). `PidTerms` sorti de `loop.py` vers `log/pid_terms.py` (1 585 tests verts). `golden.py record-modules` / `check-modules` (`scripts/golden_modules.py`) : 20 logs, 3 123 fichiers npz, 840 Mo dans `$OTB_LOGS/golden/modules/`, deuxième enregistrement identique (0 différence), les 94 fonctions publiques de `analysis/` et `log/` couvertes. `rust/otb-parity` : lit les 3 123 fichiers, rejoue les fonctions portées (`pt1_gain`, `notch_q` de `otb-dsp` : 60 appels identiques au bit près) ; une erreur volontaire de 1e-15 relatif est détectée. Phase suivante : 4a |
| 4a | En cours (branche `feat/rust-log`, plan : en-tête, lecture, `Settings`, `FlightLog`). 4a-1 faite le 2026-10-30 : `debug_tables` et `modes` en Rust (`rust/otb-log`), 6 tests sur les tables entières et 9 000 cas de Python (3 mutations vues en échec), 140 appels du corpus rejoués par `otb-parity` sans écart. 4a-2 faite le 2026-10-30 : en-têtes (`rust/otb-decoder/src/header.rs`) identiques à Python sur 477 cas limites écrits ou tirés au hasard (`scripts/header_cases.py`) et sur les 680 sessions du corpus (`golden.py record-header`, `otb-parity check-header`) ; 7 mutations vues en échec, dont un écart réel trouvé (`0x-1`). Écarts de robustesse assumés : `inf` et entier hors de `i64` rendent la valeur par défaut (Python plante ou a des entiers sans limite). 4a-3 faite le 2026-10-30 : `decode_file` en Rust (`rust/otb-decoder/src/file.rs`) : empreintes identiques à `decoder.json` (Python d'origine) sur les 680 sessions / 499 fichiers du corpus (`otb-parity check-decoder`, 12 s sur 6 fils) et sur 34 fichiers fabriqués ou abîmés dont les attendus viennent du décodeur numba (`scripts/decoder_fixtures.py`) ; 10 mutations vues en échec, dont 3 qui survivaient d'abord (seuil de 2 champs I, indices `GPS_home`). 4a-4 faite le 2026-10-30 : `Settings` / `parse_settings` en Rust (`rust/otb-log/src/settings.rs`) identiques à Python sur 1 376 cas limites (versions, D/D max, filtres, RPM, moteurs, curseurs, chaque caractère latin-1 dans le motif du firmware ; `scripts/settings_cases.py`) et sur les 680 sessions du corpus (`golden.py record-settings`, `otb-parity check-settings`) ; 12 mutations vues en échec (1 équivalente : `>` / `>=` du D max). 4a-5 faite le 2026-10-30 : `FlightLog` en Rust (`rust/otb-log/src/flightlog.rs`) : `from_session`, `load`, propriétés et méthodes. Preuves : **bits identiques** sur les 668 sessions exploitables du corpus pour chaque tableau (`t`, `gyro`, …, `events`), `fs`, statistiques, colonnes PID, `pid_sum`, `pid_error`, `armed`, `cells`, `segments` (`golden.py record-flightlog`, `otb-parity check-flightlog`, 15 s) ; 62 fichiers fabriqués ou abîmés (`scripts/decoder_fixtures.py`, 31 scénarios `fl_…`) ; 532 appels de méthodes du corpus rejoués (`otb-parity check`). **Tolérance fixée avant le code** : `d_term_prefilter` (dérivée, `np.convolve` et `np.dot` par BLAS) à 1e-7 de son plus grand échantillon (≈ 2 ulp `f32`) ; mesuré : **0** sur tout le corpus (identique au bit près), donc la tolérance n'est qu'une marge pour une autre machine. 24 mutations de `from_session` vues en échec (dont 4 qui survivaient d'abord, une qui ne changeait pas le fichier, deux équivalentes : signe du gradient, plancher de la largeur de lissage) ; `summary` et le texte des constats attendent la ligne de commande (phase 5c). **Relecture indépendante** (agent `relecteur`, 2026-10-30) : 4 écarts trouvés et corrigés avec test (convolution `n < w`, `strip`, débordement de `cell_count`, `index_of(NaN)`), 1 reporté (W-020 : ordre des clés de `raw`) |
| 4b | En cours (branche `feat/rust-numeric`). **4b-0 faite le 2026-10-30 : tolérances fixées avant le code.** Mesure sur un vrai log (Allykill `btfl_002.bbl`, 2 000 Hz), longueurs 600 à 4 096 dont 997 : `realfft` contre `np.fft.rfft` / `irfft` : jamais identique au bit près, écart ≤ 8,9e-16 du plus grand élément ; `cos` (Hann), `ln`, `exp` de Rust : identiques au bit près ; `log10`, `atan2`, `hypot` : 1 à 7 % de valeurs qui diffèrent du dernier bit (≤ 2,4e-16). **Tolérances retenues** : arithmétique pure, formules, Hann, `ln`, `exp` et chaînes de filtres → **bit pour bit** (`rtol = 0`) ; tout ce qui passe par une FFT, `log10`, `atan2` ou `hypot` → `rtol = 1e-12` du plus grand élément du tableau (marge ×1 000 sur le mesuré) ; `sliders_from_pids` (`lstsq` remplacé) : à mesurer en 4b-5, puis `slider_view` identique. Banc : `rust/otb-dsp/examples/banc_4b0.rs`. **Correction de la règle en 4b-1 (L-048)** : `hann` (`cos`) et `gaussian_filter` (`exp`) passent à `rtol = 1e-14`, pas « bit pour bit » : `np.exp` AVX-512 diffère de `math.exp` pour 4,5 % des arguments et la bibliothèque mathématique de Windows peut différer de celle de Linux ; seule l'arithmétique pure reste exigée identique. **4b-1 faite le 2026-10-30** : `_numeric` en Rust (`rust/otb-dsp/src/numeric.rs`) : 1 660 cas écrits (`scripts/numeric_cases.py`, dont longueurs 0 et 1, NaN, infinis, zéros négatifs, float32) et 155 appels du corpus rejoués par `otb-parity` sans écart ; mesuré sur le corpus : `hann`, `uniform_filter1d`, `gaussian_filter` **identiques au bit près**, `xcorr_full` ≤ 1e-15 du plus grand élément ; 10 mutations vues en échec (dont l'`nfft` trop petit trouvé en écrivant le code) |
**4b-2 faite le 2026-10-30** : `spectral` en Rust (`rust/otb-analysis/src/spectral.rs`, FFT par lignes `otb_dsp::numeric::rfft_rows`) : 384 cas écrits (`scripts/spectral_cases.py` : signal vide ou plus court que la fenêtre, raccords sur la frontière d'une fenêtre, moyennes de gaz tombant sur une frontière de tranche, NaN et infinis dans l'axe des gaz, demi-entiers de `round()`, trois durées de recouvrement) et 340 appels du corpus rejoués sans écart. **Écarts mesurés** (rapportés au plus grand élément) : amplitude ≤ 5,0e-16 ; spectre en dB ≤ 1,5e-10 sur les cas écrits (la dynamique du spectre amplifie l'écart de la FFT sur les petits éléments : l'estimation avant mesure, 1e-5, était trop large), ≤ 1e-13 (`spectrum`) et entre 1e-11 et 1e-10 (`freq_time`) sur le corpus ; `smooth_map` ≤ 4,3e-16 ; `to_db` ≤ 7,4e-18 ; identiques au bit près : `clear_of_breaks`, `ptb_subsample`, `to_ptb_db`, `smooth_freq`, fréquences, centres de tranches, nombres de fenêtres. **Tolérances retenues** : amplitude et `smooth_map` 1e-14, dB 1e-9 (`registry::tolerance`). 20 mutations vues en échec (dont l'arrondi au pair de `round()`, `astype(int)` de NaN = `INT_MIN`, somme par paires de `np.mean`), 1 équivalente (tranche vide : `0/0 = NaN` de toute façon). **Poids des jeux de cas** (réduits le 2026-10-30, sans perdre de situation testée) : un signal est tiré une fois par forme ou par longueur puis rangé dans une table `inputs` partagée par les cas, au lieu d'être retiré pour chaque réglage ; `numeric.json` 3,1 → 1,9 Mo et `spectral.json` 6,8 → 3,3 Mo (git les stocke compressés : 1,3 → 0,8 Mo et 2,6 → 1,1 Mo). Compresser les fichiers soi-même n'aurait rien économisé dans le dépôt. Les 30 mutations de `_numeric` et `spectral` sont rejouées sur ces jeux réduits : toutes vues en échec |
| 4c à 7 | à faire (D-035) |
