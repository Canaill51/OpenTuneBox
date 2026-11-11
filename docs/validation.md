# Validation

Mesures sur le corpus de Cédric, **de la plus ancienne (2026-10-01) à la plus récente** : chaque section est datée,
c'est la date qui compte, pas celle de cette ligne. À refaire après toute modification du décodeur ou des analyses
(`otb-dev corpus validate`, `otb-parity`, tests de chaque crate).

**Ce document est le maître des chiffres du corpus** : les autres documents y renvoient au lieu de les recopier.
Taille du corpus au **2026-11-08** : **525 fichiers** de logs (`.bbl` / `.bfl`, hors `golden/`) et **595 captures**
PIDtoolbox (`PTB_FIGS_*`), pour **709 sessions** lues.

## Décodeur

`otb-dev corpus validate` (anciennement `validate_corpus.py`, en Python) compare chaque session à `blackbox_decode` (fourni avec PIDtoolbox), alignée sur `time` :

- 568 fichiers décodés ; **95 % des sessions identiques** valeur par valeur.
- Les écarts restants sont des trames corrompues que la référence accepte : trames tronquées avant un trou
  d'écriture sur la carte SD (`gap`), trames S parasites (`slow`), valeurs impossibles (gyro ou consigne
  > 4000 °/s, terme PID > ±65535). OpenTuneBox les rejette.
- La référence se bloque sur 3 fichiers (`refhang`) qu'OpenTuneBox lit.
- Démarrage à froid du décodeur numba ≈ 0,6 s (cache numba).
- **Décodeur Rust** (D-031, 2026-10-29) : empreintes identiques à celles du décodeur numba sur 499 fichiers et
  680 sessions (`golden.py check` de l'ancienne version Python, aussi pour les outils et les constats de l'assistant : 0 différence
  sur 630 sessions) ; 6 000 flux aléatoires et 40 fichiers abîmés (`test_native_decoder` de l'ancienne version). Temps
  d'ouverture : 3,6 Mo 0,32 s → 0,05 s ; 22,3 Mo 1,94 s → 0,29 s.
- **Phase 2a** (D-032, 2026-10-29) : sans scipy ni numba, mêmes résultats que les références figées par `main` :
  décodeur 0 différence sur 499 fichiers, assistant 0 session aux constats différents sur 630, outils (20 logs)
  même structure, écart relatif maximal **2,4e-10** (boucle ouverte, qui amplifie les écarts de 1e-14 de
  `_numeric.py` face à scipy) ; noyaux des filtres Rust = numba au bit près, sauf la réponse en fréquence (1,2e-15).
- **Phase 2b, noyau de `step_response` en Rust** (D-033, 2026-10-29) : mêmes résultats que les références de
  `main` : décodeur 0 différence, assistant 0 session aux constats différents sur 630 (durée cumulée de l'assistant
  1879 s → 1410 s), outils (20 logs) même structure et écart relatif maximal 2,4e-10 ; noyau face à la boucle numpy :
  1,7e-14 sur les réponses, 4e-12 case par case sur les sommes. Temps (session de 72 s, 3 axes) : 315 ms → 185 ms à un fil (1,70×), 64 ms avec tous les cœurs (4,9×).
- **Phase 3a, ouverture des logs en Rust** (D-034, 2026-10-29) : `FlightLog` **identique au bit près** à celui de l'ancien
  code numpy (224 + 21 cas de `test_native_log` de l'ancienne version, dont 40 fichiers abîmés et le corpus) ; mêmes références de
  `main` : décodeur 0 différence, assistant 0 session aux constats différents sur 630 (durée cumulée de l'assistant
  1879 s → 1357 s), outils (20 logs) même structure et écart relatif maximal 2,4e-10. Temps (un chargement par processus,
  33,6 Mo, 620 000 trames) : 838 → 502–548 ms (1,6×) ; `from_session` 413 → 147 ms ; mémoire conservée +154 → +138 Mo.
- Un vrai extrait est versionné pour la CI : `tests/data/jeno5_5s.bbl` (5,1 s du début de la 1re session de
  `Cedric/JeNo 5/FINAL.bbl`, Betaflight 2026.6.0-rc3, 8 262 trames, coupé sur une trame I). Statut `ok` face à
  `blackbox_decode` le 2026-10-12 ; un test de l'ancienne version Python figeait l'en-tête, la première et la dernière trame,
  les statistiques et les unités physiques.

### Ligne de base avant les chantiers du décodeur (2026-11-04, `main` au commit 11f1a71, poste Linux)

Sert de point de comparaison aux lots 4 et 5 de `docs/plan.md` : après leurs changements, **rien de plus** ne doit
différer. Variables à définir : `OTB_GOLDEN=$HOME/Documents/Drones/Logs/golden` et
`OTB_REF_DECODER=$HOME/.local/share/PIDtoolboxProV094/blackbox_decode` (fourni avec PIDtoolbox).

- Corpus : 529 fichiers `.bbl`/`.bfl` sous `~/Documents/Drones/Logs` (528 lus par `corpus validate`, `golden/` exclu).
- `cargo run --release -p otb-parity -- check "$OTB_GOLDEN"` : 3123 fichiers, 1728 appels rejoués, **0 échec** (37 s).
- `cargo run --release -p otb-dev -- corpus validate` : **697 sessions conformes sur 712** (6 min 42 s, code de sortie 1).
  Statuts : ok 627, gap 67, slow 8, values 5, refhang 3, rows 2. Les 15 non conformes (`slow`, `values`, `rows`) sont des
  trames corrompues que la référence accepte et qu'OpenTuneBox rejette (voir plus haut) ; elles se trouvent dans 14 fichiers :
  - `slow` : `1. PTB/BlackSummer/` (`Hover_48kHz.bfl`, `Wobble_0.8.bfl`, `Wobble_0.8_DSHOT300.bfl`), `1. PTB/Tes/mm 1.0.BFL`,
    `1. PTB/st7ven3241/` (`108_D1.5_PD1.4.BFL`, `116_D1.5_PD1.4_MM1.4..BFL`, `LOG00042_wobble.BFL`, `LOG00081_D16_P14_MM12.BFL`) ;
  - `values` (terme PID à ±2147483648) : `1. PTB/Sohail/btfl_all.bbl` (session 7), `07-16 VX3.5 HOVER+WOBBLE TUNED.bbl`
    de `StraylightFPV/VX35/` et de `falseidol416/` (session 2), `Cedric/Meteor 75 Pro O4 HD/48k Base/t2.bbl` (session 3) ;
  - `rows` : `1. PTB/Zox - Mario8/HOVER HQPROPS.BFL`, `1. PTB/st7ven3241/LOG00054_MM1.4_PiD_1.3.BFL`.
- Ouverture d'un log (`opentunebox info`, version release, 3 essais, `/usr/bin/time` : temps écoulé et pic de mémoire) :

  | Log | Taille | Trames | Temps | Pic de mémoire |
  |---|---|---|---|---|
  | `1. PTB/st7ven3241/LOG00008.BFL` (le plus gourmand des 529) | 32 Mo | 620 079 | 0,59 à 0,62 s | **≈ 491 Mo** |
  | `1. PTB/ram/BTFL_BLACKBOX_LOG_MARIO_8_…bbl` | 21 Mo | | 0,46 à 0,49 s | ≈ 266 Mo |
  | `tests/data/jeno5_5s.bbl` | 5 s | 8 262 | < 0,01 s | ≈ 11 Mo |

  Banc : `otb-dev bench-open LOG [ESSAIS]` (pic = `VmHWM`, compteur du système, un processus par log). Refait le 2026-11-04 sur
  `main` : `LOG00008.BFL` 0,63 s, **481 Mo** (3 essais identiques) ; `jeno5_5s.bbl` 0,008 s, 12 Mo.
  **Essai du lot 5a (conversion session par session) : sans effet.** `LOG00008.BFL` n'a qu'une session et aucun log du corpus de plus
  de 1 Mo n'en a deux : le pic vient de la table d'entiers 64 bits d'une seule session, que 5a ne réduit pas. Mesuré : 513 Mo
  (+ 32 Mo, le fichier restant lu pendant toute la conversion) et 0,70 s ; code retiré.
  **Lot 5b retenu (L-081)** : le pic était dans `from_session` (table `i64` de 213 Mo gardée pendant les copies finales), pas dans le
  décodeur (295 Mo). `Fields::release_main` la rend après l'extraction du temps et des drapeaux : `LOG00008.BFL` **342 Mo**
  (÷ 1,41 ; objectif ÷ 1,5 non atteint), 0,62 s. Identité : empreinte de tous les `FlightLog` des 528 fichiers, avant et après
  (programme temporaire, non gardé) : identiques ; `otb-parity check` 0 échec sur 1728 appels ; `corpus validate` 697/712, mêmes statuts.
  **Refonte (rangement compact, L-081)** : le décodeur range directement les trames principales en `CompactTable` (`i32` ramenés aux
  32 bits pour toutes les colonnes sauf `loopIteration` et `time`, en `i64`) quand `load` le demande. `LOG00008.BFL` : **274 Mo**
  (÷ 1,75 ; objectif ÷ 2 manqué, ÷ 1,5 atteint), **0,54 s** (0,63 s avant : plus rapide). Identité : empreinte de tous les `FlightLog`
  des 528 fichiers identique à celle de `main` ; `otb-parity check` 0 échec ; `corpus validate` 697/712, mêmes statuts. Contre-preuve : la
  position des trames lentes calculée sur une table vide (bogue rencontré en route) fait échouer `otb-decoder/tests/compact.rs`.
  Le test `otb-parity --test golden` échouait avec `OTB_GOLDEN` défini (écart de 1e-15 relatif dans `spectral.spectrum`), **déjà sur `main`** :
  traité à part le 2026-11-05 (D-047, L-083 — il était rouge par construction, il applique désormais la tolérance de chaque fonction).

  Objectif du lot 5a (÷ 1,5) : passer sous ≈ 327 Mo sur `LOG00008.BFL`. Le « plus gros log » de W-014 (965 000 trames, 39 Mo)
  n'est plus dans le corpus : ses 364 Mo ne se comparent pas à ces chiffres. Ordinateur : 16 cœurs, 15 Go de mémoire.

### Ouverture de plusieurs logs (2026-11-10, lot 11, D-051)

Banc : `otb-dev bench-open LOG… [--jobs N] [--trials N] [--drop]` (version release, 3 essais, même ordinateur : 16 cœurs,
15 Go). `--jobs 1` = les fichiers les uns après les autres (l'ouverture d'avant le lot 11) ; `--jobs 3` = le plafond livré.
Les six logs mesurés, dans cet ordre : `1. PTB/st7ven3241/LOG00008.BFL` (32 Mo), `1. PTB/ram/BTFL_BLACKBOX_LOG_MARIO_8_…bbl`
(21 Mo), `Cedric/JeNo 5/FINAL.bbl`, `Cedric/BF Chirp/Air65-4.bbl`, `Jerome/Valkyrie/M1.4.bbl`, `1. PTB/TomZboril/openracer 5136.bbl`
(16 Mo chacun) — 1 416 000 trames en tout.

| Fichiers décodés en même temps | Durée totale | Gain | Pic, logs relâchés (`--drop`) | Pic, logs gardés (comme l'interface) |
|---|---|---|---|---|
| 1 (avant le lot 11) | 1,835 s | — | 324 Mo | 640 Mo |
| 2 | 1,082 s | × 1,70 | 574 Mo | 737 Mo |
| **3 (livré, D-051)** | **0,811 s** | **× 2,26** | **663 Mo** | **804 Mo** |
| 4 | 0,633 s | × 2,90 | 740 Mo | 931 Mo |

Objectif du lot (× 2) atteint. **Un seul fichier n'est pas dégradé** : `LOG00008.BFL` seul, 0,484 s avec `--jobs 1`, 0,498 s
avec `--jobs 3` (le plafond est ramené au nombre de fichiers : aucun fil n'est lancé), 324 Mo dans les deux cas.

⚠️ **Le chiffre de mémoire annoncé par D-051 était trop bas** : il comptait 180 Mo par fichier décodé, mais le plus gros log
du corpus en coûte 324 Mo à lui seul, d'où 663 Mo à trois fichiers et non « environ 540 Mo ». Le plafond en nombre de
fichiers est conservé (Cédric, 2026-11-10) et D-051 porte désormais la mesure. Dans l'interface, le surcoût réel du
parallélisme n'est que de **+ 164 Mo** (640 → 804 Mo) : les logs déjà ouverts pèsent de toute façon.

Identité : `rust/otb-parity/tests/parallel_identique.rs` compare les empreintes complètes des `FlightLog` (`flightlog_digest`)
entre `--jobs 1` et `--jobs 3`, sans corpus. Contre-preuve : un seul échantillon modifié dans le chemin parallèle fait
échouer le test. Le plafond lui-même est compté dans `rust/otb-log/tests/parallel.rs` (contre-preuve : sans plafond, douze
appels se chevauchent et le test rougit).

### Robustesse face à un en-tête hostile (2026-11-04, D-044)

`rust/otb-decoder/tests/robustness.rs` fabrique en mémoire des variantes de `tests/data/jeno5_5s.bbl` (aucun fichier de plus dans le dépôt) ;
chaque cas doit rendre une erreur ou zéro session en moins d'une seconde.

| Cas | Avant (mesuré) | Après |
|---|---|---|
| `I interval` 9·10¹⁸ ; 65 536 | décodé tel quel | refusé (`HeaderOutOfBounds`) |
| `P interval` 1/9·10¹⁸ ; 9·10¹⁸/2 | décodé tel quel | refusé |
| 513 champs ; 20 000 champs | accepté ; 20 000 : 3,7 s | refusé, instantané |
| 512 champs ; intervalles 65 535 (les bornes) | accepté | accepté |
| réglages à zéro, fichier vide, tronqué 50 %, en-tête coupé, corps à 0xFF, dix mutations à 1 % | propre | propre (non-régression) |

Contre-preuves : plafond d'intervalle retiré → 4 cas rouges ; plafond de champs retiré → 2 cas rouges (20 000 champs : 4,5 s) ;
`try_reserve` retiré de `Table::push` → son test panique ; seuil de cadence retiré → `otb-log/tests/implausible.rs` rouge.
Aucun vrai log ne change : `otb-parity check` 0 échec sur 3123 fichiers, 70 fichiers abîmés inchangés, `corpus validate` : 697 sessions conformes sur 712, mêmes statuts que la ligne de base (ok 627, gap 67, slow 8, values 5, refhang 3, rows 2).
Limite : sous Linux, une réservation énorme peut réussir (surengagement de la mémoire) ; le plafond de 512 champs borne l'essentiel, pas tout (non mesuré).

**Essai de mutations (2026-11-11, D-054).** `rust/otb-decoder/tests/support/mutations.rs` abîme la même source de six façons,
chaque cas refaisable d'après son numéro : quelques octets (1 à 8), l'en-tête (1 à 4 octets parmi ses 5 599), coupure, trou
(jusqu'à 4 Kio), morceau doublé (jusqu'à 4 Kio), 1 % des octets. Deux tests s'en servent : `robustness.rs`
(`mutations_variees_restent_propres`, décodeur seul) et `rust/otb-cli/tests/robustesse.rs` (`report` : décodage, conversion,
toutes les analyses et l'assistant, comme à l'ouverture d'un fichier dans l'interface). En CI : 24 et 6 cas. Essai prolongé :

```sh
cd rust
OTB_MUTATIONS=100000 cargo test --release -p otb-decoder --test robustness mutations_variees   # ≈ 5 min
OTB_MUTATIONS=20000 cargo test --release -p otb-cli --test robustesse                         # ≈ 5 min
```

| Essai | Cas | Durée | Résultat |
|---|---|---|---|
| Décodeur seul | 100 000 | 288 s | aucune panique, aucun blocage |
| Tout le programme (`report`) | 3 000 puis 20 000 | 43 s ; 294 s | aucune panique, aucun blocage |

Contre-épreuve : une panique ajoutée au décodeur sur un octet non ASCII de l'en-tête fait rougir les deux tests, qui nomment
le cas (« mutation 1 (en-tête) : panique »). Limite : une seule source (`jeno5_5s.bbl`, Betaflight 2026.6.0-rc3, une session) ; les
fichiers d'un autre firmware ou à plusieurs sessions ne sont pas mutés.

## Recoupement avec PIDtoolbox Pro V0.94 (mêmes logs)

| Mesure | Log | PIDtoolbox | OpenTuneBox |
|---|---|---|---|
| Retard des filtres gyro | Allykill `btfl_002` (log retiré du corpus le 2026-10-12) | 1,25 ms | 1,2 ms (modèle de filtres : 1,32 ms) |
| Pic de réponse indicielle roll | BlackSummer *Damping* 0.8 / 1.0 / 1.2 / 1.4 | 1,23 / 1,11 / 1,00 / 0,98 | 1,24 / 1,13 / 1,03 / 1,04 |
| Retard consigne → gyro | même série | 12,7 / 14,7 / 16,8 / 18,3 ms | 12,5 / 14,5 / 16,6 / 18,2 ms |
| Pic / montée | JeNo 5 `LAST` | 1,02 / 15,6 ms | 1,05 / 15,9 ms |

**Échelle des spectres** (DSP en dB, mêmes fréquences, 2026-10-05, complété le 2026-10-11) : notre densité
calculée (identique à `scipy.signal.welch`, intégrale = variance du signal) est plus haute que celle de
PIDtoolbox de 6,5 dB ± 1, quelle que soit la fréquence du log (`spectral.PTB_PSD_OFFSET_DB`) ; l'interface
affiche donc l'échelle de PIDtoolbox. Mêmes formes et mêmes pics : seule la normalisation diffère.

| Log (fréquence) | Signal | PIDtoolbox | OpenTuneBox |
|---|---|---|---|
| Apri `btfl_008` (2 kHz) | gyro roll 60–90 / 250 / 900–1000 Hz | ≈ −45 / −45 / −38 dB | −37,5 / −39,5 / −30,6 dB |
| Fred28 `LOGSetting1` (1,6 kHz) | gyro roll 60 / 120 / 400 / 720 Hz | ≈ −30 / −38 / −38 / −46 dB | −24,7 / −32,7 / −32,3 / −40,3 dB |
| Fred28 `Stock` (1,6 kHz) | même chose | ≈ −37 / −42 / −46 / −48 dB | −29,0 / −34,2 / −39,5 / −43,1 dB |
| Kris `BTFL_…_181415` (1 kHz) | gyro roll, gyro brut, gyro lacet 100–450 Hz | | 4 à 8 dB plus haut (7 dB en moyenne) |
| ram `…_161516_…_hover2` (2 kHz) | gyro et gyro préfiltre, 3 axes, courbes relevées pixel par pixel : 130–190 / 430–985 / 20–75 Hz | | +6,4 / +6,5 / +8,5 dB (médianes) |

Sur `hover2`, la rugosité des courbes de PIDtoolbox correspond à des fenêtres d'environ 2 s (résolution
0,5 Hz, notre défaut) ; des fenêtres courtes (≤ 0,25 s) font déborder le pic sous 10 Hz vers 20–40 Hz, ce
que PIDtoolbox ne montre pas. Moyenner en dB ou prendre la médiane des fenêtres n'explique pas mieux
l'écart. Les 2 dB de plus sous 100 Hz ne se retrouvent pas sur Fred28 et Apri : propres à ce vol, pas corrigés.

**Retards des filtres** affichés par PIDtoolbox (« Gyro Filter | Dterm Filter »), comparés à notre méthode
de phase et au modèle des filtres de l'en-tête (`bf_filters.gyro_chain` / `dterm_chain`, retard de phase à
10 Hz) :

| Log | PIDtoolbox gyro / D | Mesure gyro / D | Modèle gyro / D |
|---|---|---|---|
| Fred28 `LOGSetting1` | 1,125 / 2,25 ms | 1,23 / 2,82 ms | 1,02 / 2,59 ms |
| Fred28 `LOGSetting2` | 1,0 / 2,125 ms | 1,00 / 2,50 ms | 1,04 / 2,60 ms |
| Fred28 `Stock` | 2,125 / 2,125 ms | 1,76 / 2,57 ms | 1,99 / 2,63 ms |
| Kris `BTFL_…_181415` (1 kHz) | 3,0 / 3,0 ms (roll) | 3,05 / 3,74 ms | 2,89 / 3,89 ms |

Le gyro concorde à ±0,2 ms près (sauf `Stock`, 0,4 ms). Pour le D, PIDtoolbox affiche 0,3 à 0,9 ms de
moins que la mesure et le modèle, d'autant plus que le log est lent : probablement son D reconstruit
(dérivée décalée d'un demi-échantillon du log). Les objectifs de retard D lus dans PIDtoolbox sont donc
trop bas pour nos mesures (`assistant.md`, « Repères de PIDtoolbox »).

**Poids du filtre RPM** (2026-10-05, corpus) : sur les axes où le pilote a baissé un poids (760 pour la
2e harmonique, 396 pour la 3e), la raie restante mesurée dans le gyro filtré suit la prévision du firmware
(hauteur avant filtres + 20·log10(1 − poids)) à 1 dB près en médiane, et reste invisible (≤ 3 dB) dans
97 à 99 % des cas.

**Retard des filtres et mouvements** (2026-10-05, 681 sessions) : écart entre le retard mesuré et le modèle
des filtres de l'en-tête (notchs RPM au régime médian, 30 Hz), autour de sa médiane. Le 95e centile de
|consigne| (roll ou pitch) sépare les vols : stationnaires purs 1 à 24 °/s (40 sessions nommées « hover »),
stationnaire + wobble 92 à 409 °/s, vols ordinaires 174 °/s en médiane ; 108 sessions sont sous 50 °/s.

| Méthode | Écarts > 0,5 ms, ≥ 50 °/s (573) | Écarts > 0,5 ms, < 50 °/s (107) |
|---|---|---|
| Intercorrélation gyro (PIDtoolbox) | 16 % | 50 % (p10 / p90 −1,22 / +1,28 ms) |
| Phase gyro (OpenTuneBox) | 9 % | 9 % |
| Phase D (OpenTuneBox) | 2 % | 5 % |

La méthode de phase résiste mieux au manque de mouvement, mais le retard d'un vol stationnaire ne vaut
qu'à son régime : l'assistant ne conseille rien sur le retard sous le seuil des mouvements (50 °/s à cette
date, 30 °/s depuis D-026, ci-dessous). Sans ces vols, le p90 du retard
gyro passe de 2,04 à 1,85 ms en 5" et de 1,94 à 1,71 ms en 2–3,5" : repères de retard recalculés
sur ces seuls vols (`docs/assistant.md`, « Calibration »).

**Seuil des mouvements, 50 → 30 °/s** (2026-10-13, 705 sessions, D-026) : l'intervalle vide entre vols
stationnaires et vols avec wobble ne l'est plus. 67 sessions sont entre 24 et 92 °/s, dont 22 entre 40 et
50 °/s : 15 en mode Angle, 422 fenêtres de réponse indicielle en médiane (16 au-delà de 200). Ce sont surtout des
wobbles du protocole du guide, en mode Angle limité à 30° (`Cedric/JeNo 5/MM` et `PD`, `Cedric/2026.6 RC2`, 46 à
49 °/s). Les 83 sessions sous 25 °/s sont des vols stationnaires (une seule a 200 fenêtres) ; 2 sessions
sont entre 25 et 30 °/s.

Retard par tranches de 8 s d'un même vol (335 sessions ayant au moins 3 tranches à plus de 150 °/s), écart à
la médiane de ces tranches :

| Consigne de la tranche (95e centile) | Tranches | Retard gyro : écart médian | > 0,2 ms | Retard D : écart médian |
|---|---|---|---|---|
| 0–30 °/s | 258 | 0,09–0,10 ms | 22–27 % | 0,06 ms |
| 30–50 °/s | 57 | 0,09–0,13 ms | 24–29 % | 0,06–0,07 ms |
| 50–100 °/s | 127 | 0,08–0,10 ms | 16–25 % | 0,04–0,08 ms |
| 100–150 °/s | 122 | 0,05 ms | 7 % | 0,04 ms |
| ≥ 150 °/s (référence) | 1 667 | 0,03 ms | 3 % | 0,02 ms |

La dispersion diminue avec les mouvements, sans rupture à 50 °/s ; le biais reste sous 0,07 ms et la vitesse
des moteurs ne change pas d'une tranche à l'autre (rapport 0,97 à 1,02). Vol stationnaire (< 25 °/s) contre vol
avec mouvements (≥ 100 °/s) du même drone, mêmes filtres (18 drones) : +0,015 ms en médiane (p10 / p90 −0,12 /
+0,22 ms) pour le gyro, +0,03 ms pour le D.
Découpe « stationnaire / mouvements » du guide (`flight_parts` de `rust/otb-analysis/src/period.rs`, blocs de 1 s lissés sur 3) : dans les
83 vols stationnaires, 1,6 % des blocs dépassent 30 °/s (0,3 % à 50 °/s) ; dans les vols au-delà de 100 °/s,
79,7 % (74,6 %). Un vol freestyle n'a pas de vrai stationnaire : la plus longue partie calme peut changer
d'endroit avec le seuil (`Cedric/JeNo 5/FINAL`, `rust/otb-advisor/tests/tuning_path.rs`).

Écart connu : la latence 50 % peut différer d'environ 1 ms (interpolation au passage exact à 50 % contre
lecture échantillon par échantillon).

## Autres contrôles

- Curseurs simplifiés : les formules redonnent les PID de l'en-tête pour 740 sessions sur 741.
- Simulateur de filtres : sans modification des réglages, le spectre « et si » égale le spectre loggé à 0,009 dB près.
- Filtres Betaflight : réponses comparées aux formules analytiques (`test_bf_filters` de l'ancienne version Python).
- Marges de stabilité (`rust/otb-analysis/src/open_loop.rs`) : marge de phase, croisement et marge de gain égaux aux
  formules exactes d'une boucle intégrateur + retard (`test_loop` de l'ancienne version, ±0,3° et ±0,05 dB), y compris
  une marge négative. Sur 60 logs du corpus tirés au hasard (133 axes, 2026-10-05) : marge inchangée à
  1° près pour 83 des 122 axes mesurables ; une marge passe de +299° (phase repliée, ancien calcul) à
  −109° ; les 8 écarts de plus de 5° sont des axes très bruités (yaw, vol stationnaire, phase qui saute de
  50 à 100° d'un point au suivant). Prendre la plus petite marge sur **tous** les croisements |L| = 1 a été
  essayé puis écarté : |L| repasse au-dessus de 1 vers 100 Hz sur les logs bruités et donnait des marges
  de −1100°. Le pic de sensibilité tombe au bord de la bande analysée pour 57 axes sur 133.
- Boucle ouverte mesurée (`rust/otb-analysis/src/open_loop.rs`, 2026-10-05) :
  - **simulation** d'une boucle Betaflight (`bf_loop_sim` de l'ancienne version Python : P, I sur l'erreur, D sur le gyro
    filtré, F lissé, retard, moteur, filtres, perturbation et bruit ; vraie boucle connue), 5 tirages par
    cas : marge de phase à ±2,6° près (±3,2° en yaw), croisement à ±5 %, pic de sensibilité à +0,1 à
    +1,7 dB, quand le pic est dans la bande ; log à 2 kHz, 1 kHz ou 500 Hz, vol de 20 s, D variable (entre
    les boucles du D de base et du D max). Objectif initial (±3°, ±3 %, ±0,5 dB) tenu pour la marge, pas
    tout à fait pour le croisement et le pic (lissage sur 20 % de la fréquence). Ancienne formule sans
    feedforward : croisement à 7–10 Hz au lieu de 11–32 Hz, marge surestimée jusqu'à 31°, d'autant plus
    que le D domine (−2° à +31° sur 9 réglages) ;
  - **série BlackSummer** (Damping 0.6 à 1.6, même drone, seul le D change ; le D réel est lu dans
    l'en-tête : les noms de fichiers sont décalés, « 0.6 » a D = 30 comme « 1.2 ») : |Ĉd|/D constant à
    ±1,3 % (31,3 à 32,1 × 10⁻³), Ĝ identique d'un log à l'autre à 0,5–1,4 dB et 3–7° près sur la bande
    commune (5–20 Hz), sauf « 0.6 » (1,9 dB, 12°). Roll, D = 17 → 42 : marge mesurée 39 → 48 → 47°,
    pic de sensibilité 4,9 → 6,4 → 5,6 dB quand le D domine (|Ĉd/Ĉe| > 1,4) ; l'ancienne formule
    annonçait 49 → 77°. Phase de Ĉd égale au modèle des filtres de l'en-tête à ±4° jusqu'à 20 Hz ;
  - **mode angle** : dans les vols « angle » du corpus, la consigne et la commande radio comme instrument
    donnent la même boucle (0,0 à 0,3 dB, 0 à 2° jusqu'à 30 Hz) : la boucle d'attitude, lente, ne fausse
    pas la mesure au croisement ;
  - **whoops** (2026-10-06, AIR65 de Cédric, 63 sessions whoop sur 121) : simulation d'une boucle de
    whoop (croisement 22 Hz, marge réelle 39°, pic 6,8 dB, log à 1 kHz) mesurée à 37° et 7,4 dB, un peu
    pessimiste quand les moteurs sont rapides (−2 à −4° ; ±1° pour des moteurs plus lents). Vols chirp
    (excitation large bande, mesure la plus fiable) : EdgeTX en mode angle, D LPF 75 / 150 Hz, bande
    jusqu'à 60–90 Hz : roll 28–31° et 7,9–8,6 dB (±0,5°), pitch 39–42° ; chirp de Betaflight 2026.6
    (0,2–15 Hz, mode angle, D LPF 105 / 210 Hz) : roll 23,6 Hz, 39,5 ± 1,9°, pitch 44–45°, yaw 58–60°.
    L'écart entre les deux séries correspond aux filtres du D (environ 6° de phase en moins au croisement
    avec 75 / 150 Hz, le D dominant 1,4 fois). La marge faible des whoops est donc réelle ; ils sont
    plus souvent signalés aussi parce que leur bande mesurable va plus loin (46 Hz contre 22–24 Hz en
    médiane, pic au bord pour 16 % des axes contre 31–41 %) : chez les autres tailles, le pic n'est
    souvent qu'un minorant. Avant le 2026-10-06, le chirp de Betaflight n'était pas mesurable en mode angle
    (« cohérence insuffisante ») : l'excitation, absente de la commande radio, est maintenant ajoutée à
    l'instrument ;
  - **corpus** (`otb-dev corpus calibrate`, 539 logs, 681 sessions, 1253 axes roll / pitch) : boucle
    mesurée pour 88 % des axes (objectif ≥ 80 %), croisement trouvé dans la bande pour 70 % d'entre eux,
    pic de sensibilité au bord (minorant) pour 34 %. Médianes, mesurée contre ancienne formule :
    croisement 14,7 contre 8,8 Hz, marge de phase 50 contre 65°, pic de sensibilité 3,7 contre 3,2 dB ;
    le D domine au croisement (|Ĉd/Ĉe| > 1) pour 62 % des axes. Avec les seuils actuels, sessions signalées :
    pic > 6 dB 29 % (14 % avant), marge < 40° 3 % (1 % avant) ;
  - `test_matches_ptb_damping_sweep` inchangé (partie Wiener identique).
- Interface : les 8 outils s'ouvrent et calculent sans erreur sur de vrais logs (tests de l'ancienne interface Qt, puis
  `rust/otb-gui/tests/spectral.rs` et `rust/otb-gui/tests/step_tools.rs`).
- Documentation : liens, chemins, outils, modules, fichiers publiés et langue vérifiés à chaque `cargo test`
  (`cargo test -p otb-dev --test docs`).

### Temps de cadre de l'interface (2026-11-05, W-049, D-048)

Banc : `cargo run --release -p otb-gui --example banc_cadre -- LOG [--largeur 1920] [--cadres 60] [--gpu] [--glissement] [--zoom 8]`
(la vraie `App` par `egui_kittest`, hors écran, sans vsync ni compositeur ; fenêtre de la largeur demandée × 1080, session la plus
longue du log, 3 cadres de chauffe puis 60 cadres chronométrés un par un). Poste Linux, i7-11800H (16 fils), version release,
3 essais : médianes des cadres, plage des 3 essais. Les colonnes « viewer » sont déjà diminuées du temps de l'écran Guide.

**Avant l'index multi-résolution** (mesures du lot 8, et du 2026-11-05 avant D-048) :

| Log | Échantillons | Guide (aucune courbe) | Viewer, vue entière | Pendant un glissement | Par échantillon | Tessellation |
|---|---|---|---|---|---|---|
| `tests/data/jeno5_5s.bbl` | 8 262 | 0,44 ms | 0,89 ms | | 54 ns (part fixe dominante) | |
| `Cedric/JeNo 5/FINAL.bbl` | 321 729 | 0,41 à 0,43 ms | **6,9 à 8,7 ms** | 8,5 ms | 21 à 27 ns | 0,64 à 0,73 ms |
| idem, `--largeur 640` | 321 729 | 0,36 à 0,37 ms | 6,2 ms | | 18 ns | |
| `1. PTB/st7ven3241/LOG00008.BFL` | 620 079 | 0,42 à 0,45 ms | **10,5 à 12,1 ms** | 10,3 à 10,8 ms | 17 à 20 ns | 0,69 à 0,73 ms |

**Après** (D-048, index multi-résolution min/max par courbe) :

| Log | Échantillons | Viewer, vue entière | Pendant un glissement | Zoomé sur 1/8 | Par échantillon | Gain |
|---|---|---|---|---|---|---|
| `Cedric/JeNo 5/FINAL.bbl` | 321 729 | **3,1 à 3,2 ms** | 3,7 à 3,8 ms | 1,4 ms | 9,8 ns | ×2,2 |
| idem, `--largeur 640` | 321 729 | **0,95 ms** | | | 3,0 ns | ×6,5 |
| `1. PTB/st7ven3241/LOG00008.BFL` | 620 079 | **4,6 à 4,8 ms** | 4,8 à 4,9 ms | 2,1 ms | 7,5 ns | ×2,3 |

- **Le glissement coûtait autant que la vue entière** (bornes changeantes à chaque cadre) : c'est ce que mesure `--glissement`,
  qui vérifie que les bornes bougent vraiment avant de publier un chiffre. Il gagne autant que la vue entière, ce qu'un cache de
  points décimés n'aurait pas donné (option (a) écartée, D-048).
- **Ce qui reste n'est plus le parcours mais le tracé** : à 640 px (trois fois moins de points rendus) le cadre passait de 7,4 à
  6,2 ms avant (−16 %), il passe maintenant de 3,1 à 0,95 ms (−69 %). Autrement dit le coût suit désormais le nombre de points
  rendus, comme voulu.
- **Dépendance résiduelle au nombre d'échantillons**, mesurée sur le **même** log (`--zoom 8`, à nombre de points rendus égal) :
  4,7 ms en vue entière contre 2,1 ms sur un huitième. Elle vient des bords de paquets, qui touchent la courbe sur toute son
  étendue (défauts de cache, L-086). Comparer deux logs différents ne mesurerait pas cela : ils n'ont pas le même nombre de
  courbes affichées.
- **Décimation seule** (`cargo run --release -p otb-dsp --example banc_minmax`, 620 079 échantillons, 3 840 points rendus) :
  parcours 0,992 ms, index 0,169 ms (**×5,9**) ; zoomé sur 1/8 : 0,143 contre 0,060 ms ; sur 1/64 : 0,023 contre 0,025 ms —
  sous ≈ 10 000 échantillons visibles l'index ne sert plus à rien, et ne nuit pas. Construction de l'index : 1,6 ms.
- **Mémoire** : l'index pèse 43 % des courbes en cache, soit 11,6 Mo (`FINAL.bbl`) et 22,3 Mo (`LOG00008.BFL`) ; 6,4 % et 7,8 %
  du pic du processus, sous le plafond de 10 % fixé par Cédric. Le pic lui-même, mesuré avec et sans l'index, **ne change pas**
  (180 et 286 Mo) : il est atteint pendant le décodage. Le banc affiche ces trois chiffres à chaque passage.
- **Tracé inchangé** : les captures hors écran du viewer (`--example capture`, `courbes-clair.png` et `courbes-sombre.png`) sont
  identiques **au bit près** avant et après, et les tests comparent la sortie de la nouvelle décimation, point pour point, à une
  copie figée de l'ancienne (`rust/otb-dsp/tests/minmax.rs`).
- **Budget de 60 images par seconde** : le cadre le plus coûteux du corpus passe de ≈ 13,7 ms (tessellation comprise) à ≈ 5,6 ms.
  Le seuil de saccade, qui était attendu vers 800 000 échantillons, s'éloigne d'autant (extrapolation, non mesurée : aucune
  session du corpus n'est assez longue).
- **Carte graphique (`--gpu`), non reproduite** : `egui_kittest` choisit par défaut le rendu logiciel (llvmpipe : ≈ 44 ms par
  cadre, c'est le processeur qui dessine) ; le banc prend une vraie carte (ici RTX 3050 Ti, Vulkan) et l'affiche. Même ainsi,
  chaque rendu crée une texture et relit l'image : ≈ 10 ms fixes, **écran Guide compris**. L'écart viewer − Guide varie de 1 à
  4 ms d'un essai à l'autre sur `FINAL.bbl` : c'est un majorant bruité. Les 0,34 ms de W-049 ne se retrouvent pas, et la façon
  dont ils avaient été obtenus est perdue avec le banc. Le cadre se juge donc sur le temps du processeur.

### Mémoire du cache des outils (2026-11-10, lot 15, W-035 point 9, L-094)

Banc : `cargo run --release -p otb-gui --example banc_cache -- SORTIE.json [--limit N] [--part i/K]`. Il mesure ce que
**retient** chaque valeur rangée dans le cache des outils (`otb-gui/src/state.rs:Cache`, une table par log, vidée au
changement de période, **sans aucune éviction** entre réglages) : octets vivants relevés avant et après le calcul par un
allocateur qui compte, donc aucun poids écrit à la main. Un **témoin** (un `Vec<f64>` de taille connue) fait paniquer le
banc si le compteur ne retrouve pas ses octets.

Un seul fil par processus (le compteur est global) ; le corpus a été passé en 12 processus (`--part i/12`). Poste Linux,
version release. **Corpus entier : 535 logs, 642 sessions analysables, 0 erreur**, chiffres identiques d'un passage à
l'autre. Chaque type est mesuré deux fois, avec deux jeux de réglages : la deuxième mesure est le coût d'une entrée
**de plus**, et l'écart entre les deux dénoncerait une allocation faite une fois pour toutes (écart médian nul, sauf pour
la réponse indicielle où le réglage change le nombre de fenêtres).

| Ce qui est rangé | p50 | p90 | max | Entrées atteignables pour **un** log |
|---|---|---|---|---|
| Assistant (`Report`), **avant** | 7,53 Mo | 16,16 Mo | **72,63 Mo** | 6 (auto + 5 profils) |
| Assistant (`Report`), **après** | **0,29 Mo** | 0,38 Mo | 0,83 Mo | 6 |
| Réponse indicielle (`FileData`) | 7,39 Mo | 15,97 Mo | **71,31 Mo** | **72** (3 fenêtres × 4 lissages × 3 consignes × 2 « Y corr. ») |
| Simulateur de filtres (`FilterSimResult`) | 0,09 Mo | 0,09 Mo | 0,20 Mo | non borné (une par jeu de réglages) |
| Guide (`CheckResult`) | 0,01 Mo | 0,01 Mo | 0,01 Mo | 2 (rapide, complète) |

- **Le poids ne venait pas d'où on le croyait.** Le simulateur de filtres est le seul dont le nombre d'entrées n'est pas
  borné (sa clé contient tous les réglages modifiés) : c'est lui qui était suspecté, et il pèse 90 **ko**. Ce sont les
  deux outils à nombre d'entrées **borné** qui pèsent, parce que chaque entrée est énorme.
- **69 % du rapport de l'assistant (médiane) était `StepResult::responses`**, le tableau de toutes les fenêtres
  individuelles de la réponse indicielle — que **personne ne lit** dans un rapport : les règles se servent de `n`,
  `metrics` et `loop_`, la page Assistant trace `mean`, `std` et `t_ms`. Elles sont libérées depuis (L-094) :
  **l'entrée tombe d'un facteur 24 (médiane, par session), jusqu'à 165**, soit 7,26 Mo libérés par entrée (p50), 72,19 Mo
  au maximum. La longueur des fenêtres explique 74 % du gain ; le reste est la **capacité** du tableau, agrandi paquet par
  paquet (`otb-dsp/src/step.rs:step_windows`), donc toujours au-delà de sa longueur utile.
- **L'outil « Réponse indicielle » n'est pas touché** : il refait son propre calcul et garde les fenêtres, qui servent à
  sa case « Brutes (toutes les fenêtres) ». Vérifié : son entrée pèse exactement le même nombre d'octets avant et après
  (642 sessions sur 642).
- **Pire cas par log ouvert**, toutes les entrées atteignables, sans éviction : 576 Mo (p50), 1 248 Mo (p90), **5,5 Go**
  sur `Jerome/Valkyrie 2/AntiGravity.10.bbl` (174 s à 2 kHz) **avant** ; 536 Mo, 1 154 Mo et **5,1 Go** **après** (le
  simulateur compté pour 20 coups de curseur). Le gaspillage est supprimé, mais **le cache reste non borné** et l'outil
  Réponse indicielle peut à lui seul retenir 72 entrées de 71 Mo : la question du bornage reste ouverte (W-035 point 9).
- **Après le bornage** (D-053, plafond par outil, en nombre d'entrées) : le pire cas d'un log ouvert tombe de 5,1 Go à
  **≈ 223 Mo** (3 × 71,3 + 20 × 0,20 + 6 × 0,83 + 2 × 0,01). Les 28 captures hors écran (toutes les pages, les deux
  thèmes) sont **identiques au bit près** avant et après : le plafond ne change rien à ce qui s'affiche, seulement au
  moment où un résultat est recalculé.
- Poids par million d'échantillons analysés (p50) : assistant 114 Mo avant / 4,2 Mo après, réponse indicielle 112 Mo,
  simulateur 1,2 Mo, guide 0,07 Mo. Le poids suit donc la **longueur de session**, pas le nombre de logs ouverts.

### Contrôles avant la version 0.9.0 (2026-11-06, branche `chore/release-0.9.0`, poste Linux)

Les références figées (`$OTB_GOLDEN`) avaient disparu du poste : elles ont été **refaites** avec l'étalon Python
(procédure ci-dessous, `golden.py record-modules DOSSIER --sample 20`, numpy 2.5.4), puis rejouées.

- `cargo run --release -p otb-parity -- check "$OTB_GOLDEN"` : 3 128 fichiers, 90 fonctions (50 portées),
  1 732 appels rejoués, **0 écart** au bit près (`rtol=0`). Le Rust rend donc exactement ce que rend le Python
  d'aujourd'hui, et non seulement ce qu'il rendait au moment du portage.
- `cargo run -p otb-dev -- verify --strict` (avec `OTB_GOLDEN` et `OTB_BITEXACT=1`) : **tout passe, 0 saut**, 131 s.
- `cargo run --release -p otb-dev -- corpus validate` : **694 sessions conformes sur 709**, statuts
  ok 624, gap 67, slow 8, values 5, refhang 3, rows 2. Les 15 non conformes sont exactement celles de la ligne de
  base du 2026-11-04 ; l'écart sur le total vient du corpus lui-même, qui a perdu 3 fichiers (525 lus au lieu de 528).

## Refaire une référence Python

La version Python (application, tests, scripts) a été supprimée du dépôt à la phase 7-5 ; elle reste dans l'historique git. Les fichiers qui
servent de référence aux tests Rust (`rust/*/tests/data/*.json`, `$OTB_GOLDEN`, souvent `$OTB_LOGS/golden`) ont été écrits par elle. Pour en
refaire un (par exemple après un changement voulu d'un seuil, voir la compétence `assistant-seuils`) :

1. `git worktree list` **d'abord** : le dossier peut déjà être en place d'un réenregistrement précédent (sur le poste de Cédric il l'est, sur
   `python-final`, avec son `.venv` déjà installé : ≈ 1,5 Go, l'étape 2 est alors inutile). S'il n'y est pas :
   `git worktree add ../otb-etalon python-final`, le dernier état qui contient encore Python. L'étiquette `python-etalon` est plus ancienne
   (elle n'a pas les scripts `*_cases.py`). ⚠️ `git worktree add` **échoue si le dossier existe déjà** : ne pas le supprimer à la main, c'est
   `git worktree remove` qui le fait proprement.
2. Dans ce dossier : `python3 -m venv .venv && .venv/bin/pip install -e '.[dev]'` (Rust est requis : l'extension est compilée à l'installation).
3. Les scripts sont dans `scripts/` (`*_cases.py` pour les cas de `rust/*/tests/data/`, `golden.py record…` pour `$OTB_GOLDEN`) ; leur en-tête dit
   quel fichier ils écrivent. Copier le résultat dans `rust/` et lancer `otb-parity` ou les tests de la crate.
   ⚠️ Deux fichiers de cas sont stockés **compressés** (D-052) : `log_check.json.gz`, que son script écrit directement ainsi, et
   `step_tuning.json.gz`, dont le script écrit en clair — le recompresser par `gzip -9 -n` (`-n` : sans horodatage, donc mêmes octets à
   contenu identique) et ne garder que le `.gz`. Les tests lisent indifféremment l'un ou l'autre (`common::cas`), mais laisser les deux
   ferait lire le plus ancien.
4. Retirer le dossier : `git worktree remove ../otb-etalon` — ou le **garder** si un autre réenregistrement est prévu (W-028 en attend un) :
   son `.venv` représente une installation complète de numpy, scipy et numba avec l'extension Rust compilée. Dans ce cas, `git worktree list` le
   montre et ce n'est pas un oubli.

Pour `$OTB_GOLDEN` seul, inutile de tout réenregistrer : `golden.py record-modules DOSSIER --sample 20` écrit le
sous-dossier `modules/` (≈ 3 100 fichiers, 850 Mo), et c'est le **seul** que lisent `otb-parity check` et le test
`rust/otb-parity/tests/golden.rs`. Les autres `record-…` servent à `otb-parity check-cli`, `check-advisor` et
`check-guide`. Sans `$OTB_GOLDEN`, ce test saute (`[OTB-SAUT]`) et `verify --strict` reste rouge (L-023).

Le Python n'est plus maintenu : un cas qui ne peut plus s'écrire ainsi (règle ou seuil modifié dans le Rust) se met à jour à la main, dans le
test concerné, avec la raison.
