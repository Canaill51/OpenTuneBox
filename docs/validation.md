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
| Retard des filtres gyro | Allykill `btfl_002` (log retiré du corpus le 2026-10-12) | 1,25 ms | 1,2 ms (modèle de filtres : 1,32 ms ; `pid_process_denom` 1, donc rien à retirer depuis le lot 22d, L-100) |
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

**Écran d'extraction** (lot 29b, 2026-11-23, D-065) : le visualiseur devenu hors rail, avec sa barre de navigation et
son panneau de 300 points, un seul essai, même poste, `--glissement`, 1920 × 1080 :

| Log | Échantillons | Viewer, vue entière | Pendant un glissement | Par échantillon | Tessellation |
|---|---|---|---|---|---|
| `Cedric/JeNo 5/FINAL.bbl` | 321 729 | **2,2 ms** | 3,0 ms | 6,9 ns | 0,55 ms |

Pas plus lent qu'avant (3,1 et 3,7 ms) : les graphiques sont moins larges (le panneau de droite prend 300 points), et la
silhouette du gaz de la barre de navigation est calculée une fois par log et par largeur.

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

### Retard des filtres gyro sans le décalage d'horloge (2026-11-22, lot 22d, L-100)

`delay.rs:compute_delays` retire ⌊`pid_process_denom` / 2⌋ / fréquence gyro du retard gyro (0,125 ms à 8 kHz avec un denom de
2 ; rien avec un denom de 1). Test : `rust/otb-analysis/tests/delay_clock.rs` (log fabriqué sans filtre : retard nul pour des
denoms de 1, 2 et 4 ; sans la correction, 0,125 ms au denom 2). Effet mesuré sur le corpus (`otb-dev corpus rates`, `main`
contre la branche, 675 sessions) : le texte d'un constat change dans 496 sessions (le chiffre du retard), et la nature d'un
constat dans 16 seulement : 16 « retard des filtres gyro » correct (`gyro_delay`, ok) apparaissent ; 14 « filtrage gyro
prudent » (`gyro_filter_cautious`) disparaissent, remplacés par un conseil de Q RPM plus haut (`rpm_q_raise`) ; 3
`filter_strategy` et 2 « retard gyro au-dessus du repère avec du bruit » (`gyro_delay`, info) disparaissent. Centiles des retards gyro des vols avec mouvements
(`otb-dev corpus calibrate`, p10 / p50 / p90, ms) :

| Profil | Avant | Après | Repère de l'assistant |
|---|---|---|---|
| whoop | 0,97 / 1,28 / 1,48 | 0,85 / 1,16 / 1,36 | 1,5 → 1,4 |
| micro | 1,01 / 1,31 / 1,83 | 0,88 / 1,19 / 1,71 | 1,8 |
| 5" | 0,98 / 1,35 / 1,97 | 0,93 / 1,27 / 1,87 | 1,9 |
| 7" | 1,46 / 2,01 / 2,49 | 1,35 / 1,89 / 2,41 | 2,6 |
| 8"+ | 1,89 / 2,79 / 3,44 | 1,76 / 2,68 / 3,44 | 3,5 |

Repère du whoop passé de 1,5 à 1,4 ms (choix de Cédric, règle « p90 arrondi au dixième supérieur » ; vols de whoop avec
mouvements au-dessus du repère 7 → 10 sur 98) ; les autres ne bougent pas à l'arrondi. Les objectifs affichés par taille (`profiles::filter_delay_target_ms`) viennent de valeurs lues dans
PIDtoolbox, qui garde le décalage.

### Modèle de filtres étage par étage (2026-11-22, lots 22a, 22a bis et 22b)

Outil : `rust/otb-analysis/src/filter_stages.rs:check_stages` (méthode dans son en-tête) ; corpus :
`(cd rust && cargo run --release -p otb-dev -- corpus stages SORTIE.json)` (22 s pour tout le corpus le 2026-11-22 après le lot 22b, 14 s avant ; 508 sessions
lues ; `--resume SORTIE.json` refait le tableau). Écart = gain du firmware − gain du modèle, moyenne en dB pondérée par la
cohérence ; une bande n'est retenue que si sa cohérence moyenne atteint 0,5. Décalage d'horloge de L-100 retiré. Depuis 22a bis :
filtres du modèle calculés pour la cadence nominale et bandes en hertz du firmware (L-101), TPA appliqué au terme D, instants
où le firmware force le D à zéro écartés. Depuis 22b : arrondi de l'entrée retiré (L-103), LPF2 à la cadence gyro (L-104) ;
résultats ci-dessous (« Lot 22b »), les tableaux qui suivent sont ceux d'avant. Logs à 1,6 kHz ou plus (D-057).

**Logs chirp de l'Air65** (`Cedric/Air65/Test BF Chirp`, 3 logs, Betaflight 2026.12, gyro 8 kHz, PID 4 kHz, log à chaque boucle PID,
`FFT_FREQ`, sans haute résolution) : écarts signés, du plus petit au plus grand des 3 logs (axes roll et pitch pour le D).

| Étage | 5–50 Hz | 50–100 Hz | 100–250 Hz | 250–500 Hz (cohérence) |
|---|---|---|---|---|
| LPF2 + RPM + LPF1 (bloc) | −0,02 à +0,00 dB | −0,37 à −0,25 dB | −0,10 à −0,01 dB | +0,40 à +0,59 dB (0,51 à 0,62) |
| notch dynamique | −0,03 à −0,00 dB | −0,49 à −0,22 dB | −0,57 à −0,29 dB | −1,35 à −1,17 dB (0,65 à 0,71) |
| filtres D (gain ajusté sur 10–50 Hz) | +0,00 à +0,01 dB | −0,86 à −0,37 dB | −0,70 à −0,11 dB | −1,94 à −1,26 dB (0,74 à 0,84) |

Retard résiduel : −0,02 à +0,02 ms pour tous les étages ; résidu 0,7 à 0,9 % (gyro), 3,6 à 4,1 % (D). Les corrections de 22a bis
ne changent ces logs que de quelques centièmes de dB (cadence mesurée 4 032 Hz, nominale 4 000 Hz ; gaz sous le seuil du TPA la
plupart du temps). Même ordre de grandeur que l'essai jetable du plan (0,2 / 0,4 / 0,6 dB sous 250 Hz) ; la méthode diffère
(moyenne par bande ici). Les écarts sont tous du même signe d'un log à l'autre : le firmware atténue un peu plus que le modèle
entre 50 et 250 Hz (W-062).

**⚠️ Conclusion fausse, corrigée en 22b (L-103).** Le texte de 22a disait : « Hypothèse écartée : un biais de la mesure. Partout,
l'écart ressemble à 10·log10(cohérence), la signature d'un bruit sur l'entrée (arrondi au °/s) ; essai : l'entrée décalée d'un
échantillon comme variable instrumentale ; résultat identique au centième. » L'essai ne pouvait que redonner le même chiffre :
dans une fenêtre, l'entrée décalée a la même transformée à une phase près, bruit compris. Le biais était bien là.

**Corpus** (aucun log `GYRO_SAMPLE` : les étages séparés attendent les vols 22b). |écart| médian et maximal, bandes concluantes,
après 22a bis (entre parenthèses dans la colonne Sessions : le maximum sous 250 Hz mesuré en 22a, avant les corrections) :

| Étage | Log | Sessions | 5–50 Hz | 50–100 Hz | 100–250 Hz |
|---|---|---|---|---|---|
| bloc gyro | chaque boucle PID | 15 (8,34) | 0,02 (0,08) | 0,20 (0,52) | 0,21 (0,76) |
| notch dynamique | chaque boucle PID | 15 (3,98) | 0,01 (0,09) | 0,19 (0,49) | 0,35 (0,99) |
| filtres D | chaque boucle PID | 14, 28 axes (7,33) | 0,00 (0,02) | 0,37 (0,86) | 0,38 (1,35) |
| bloc gyro | une boucle sur 2 ou moins | 433 (5,35) | 0,19 (2,57) | 0,90 (5,20) | 0,59 (3,88) |
| notch dynamique | une boucle sur 2 ou moins | 433 (3,24) | 0,11 (2,13) | 0,76 (3,21) | 0,73 (2,15) |

**Critère d'arrêt du lot 22 : 24 alertes en 22a, 2 après 22a bis.** Une alerte = |écart| > 1 dB sous 250 Hz, log à chaque boucle
PID, bande concluante. Les 7 sessions en alerte de 22a, examinées une à une :

- `1. PTB/cupidstunt/petrel-85mm-issues.bbl` #2 et #4 (16 alertes, tous les étages ; D jusqu'à −7,3 dB, résidu D 0,36 à 0,45) et
  `1. PTB/redhonu/redhonu_TMOTORVELOXF7SE.BBL` #1 (bloc +8,3 dB, notch dynamique −4,0 dB) : **contrôleurs surchargés**. Boucle
  mesurée à 1 976 Hz pour 4 000 annoncés (`petrel`, F411), 6 329 Hz pour 8 000 (`redhonu`) ; le firmware calcule ses filtres pour
  la cadence annoncée (L-101). Avec le modèle à la cadence nominale et le TPA : `petrel` résidu D 0,03 à 0,04, |écart| sous 0,3 dB
  partout ; `redhonu` bloc −0,3 dB, notch dynamique −0,7 dB.
- `Cedric/BF Chirp/Air65-4.bbl` #2 (4 alertes, D, résidu 0,85 à 0,90) : **récupération de vrille en lacet** à 11,2 s (choc, gyro
  lacet au-delà du seuil). Le firmware y annule P, I et D du roulis et du tangage pendant 92 boucles (23 ms, `flight/pid.c`) ;
  le modèle, lui, produisait un D énorme. Ces 23 ms suffisaient à fausser toute la session. Une fois ces instants écartés :
  résidu 0,07 à 0,14, plus d'alerte. Repéré en découpant la session en tranches de 2 s (une seule tranche décrochait).
- **TPA** (piste 1 du plan) : `petrel` et `Air65-4` ont un TPA fort (`tpa_rate` 65, `tpa_breakpoint` 1350, `tpa_mode` 1 — valeurs
  par défaut, que tous les logs Air65 ont aussi). Seul, il ramène le résidu D de `petrel` de 0,36–0,45 à 0,27–0,30 et laisse
  `Air65-4` #2 à 0,90 : seul, il n'expliquait pas les alertes. Mais la cadence nominale seule laisse le résidu D de `petrel` à
  0,23–0,37 : il faut les deux (0,04). Le TPA entre donc dans le modèle, comme dans le firmware (résidu D des vols chirp 3,9 → 3,6 %).
- **Seuil de cohérence** (piste 2) : à 0,6 ou 0,7 au lieu de 0,5, les alertes de 22a passaient de 24 à 19 ou 17 seulement ; après
  22a bis, il en reste 2 aux trois seuils. `MIN_COHERENCE` reste à 0,5.
- **Gaz d'avant le throttle boost** pour les LPF dynamiques (piste 3, W-062) : aucun effet au millième près ; non appliqué.
- `1. PTB/StraylightFPV/KM/Filtering/011326 KM HOVER WOBBLE 5.bbl` #1 (bloc +2,3 dB) et `… KM HOVER 4K 3.bbl` #1 (notch dynamique
  −1,03 dB) : bloc à +0,8 dB et notch à −1,0 dB, sous le seuil, une fois les bandes comptées à la cadence nominale (boucle à
  4 049 Hz pour 4 000).

**Restent 2 alertes**, toutes deux sur le D entre 100 et 250 Hz, cohérence 0,85 : `KM HOVER WOBBLE 5` #1 axe pitch −1,31 dB et
`Cedric/Air65/Tune/F1.bbl` #1 axe roll −1,35 dB. Même signe et même bande que l'écart léger de tous les autres logs (W-062) :
cause à chercher, suite décidée avec Cédric ([plan.md](plan.md), lot 22). **Toutes deux disparaissent en 22b** (arrondi de l'entrée,
ci-dessous) : −0,27 et −0,43 dB au plus sur le D de ces deux sessions.

#### Lot 22b : étages séparés (`GYRO_SAMPLE`), arrondi de l'entrée, cadence des filtres, log à 2 kHz

**Vols** (`Cedric/Air65/Test BF Chirp/GYRO_SAMPLE_*.bbl`, 12 logs) : mêmes réglages que les logs chirp (Betaflight 2026.12, gyro
8 kHz, PID 4 kHz, log à chaque boucle PID, PT1 partout, aucun notch statique), `debug_mode = GYRO_SAMPLE`, axe debug roll ou pitch,
avec et sans haute résolution, chirp de 25, 45 et 65 Hz. Le second vol prévu (SVF, PT2, notch statique, notch D) n'a pas été fait.

**Premier passage, outil de 22a bis** : 0 alerte ; chaque étage sous 0,5 dB jusqu'à 250 Hz. Mais l'étage **horloge**
(`gyroUnfilt` → `GYRO_SAMPLE[0]`, la même variable `gyro.gyroADC` lue à deux instants, gain 1 par construction) mesure −0,25 à
−0,42 dB entre 50 et 100 Hz sans haute résolution, −0,04 à −0,22 dB avec : un défaut de la **mesure**. Essai jetable (hors dépôt,
signaux exportés puis numpy) : en retirant de S(x, x) l'arrondi connu de l'entrée (pas²/12), l'étage horloge tombe à 0,00–0,11 dB
sans haute résolution. Sur le D, les vols **en haute résolution** (gyro au 1/10 °/s) suivent le modèle à 0,04 dB près dans toutes les
bandes **sans aucune correction**, contre −1,1 à −2,3 dB entre 250 et 500 Hz pour les mêmes vols sans haute résolution : même
drone, mêmes filtres, seule la résolution change. L'écart « de même signe partout » (W-062) venait de la mesure (L-103).

**LPF2** : une fois l'arrondi retiré, il reste +0,41 à +0,45 dB entre 250 et 500 Hz sur les 12 vols (cohérence 0,98) : le firmware
calcule le LPF2 pour la cadence gyro, le modèle pour la cadence du log. Théorie (PT1 à 500 Hz, 8 kHz contre 4 kHz) : +0,44 dB
(250–500 Hz), +0,17 dB (100–250 Hz) ; mesuré +0,41 à +0,45 et +0,08 à +0,18 (L-104). Rejoué à la cadence gyro
(`bf_filters::gyro_stage_upsampled`), échantillons gyro absents du log reconstruits par interpolation. Interpolation **linéaire**
d'abord : LPF2 à +0,12 à +0,17 dB entre 250 et 500 Hz, car elle agit comme un passe-bas sur l'entrée reconstruite ; cubique
(Catmull-Rom) : −0,03 à +0,02 dB ; **sinus cardinal fenêtré** (Hann, ±16 échantillons, retenu) : −0,04 à +0,01 dB.

**Vols `FFT_FREQ` à 2 kHz** (`FFT_FREQ_2k-CHIRP-25/45/65Hz.bbl`, `blackbox_sample_rate = 1/2`, mêmes réglages que les logs chirp,
et `FFT_FREQ_4k-CHIRP-25Hz.bbl`, même jour, à chaque boucle PID, qui redonne les chiffres des logs chirp du 2026-11-17) : le notch
dynamique est aussi juste qu'à 4 kHz (±0,11 dB), mais le bloc gyro dévie de +0,25 à +0,32 dB entre 100 et 250 Hz (+0,01 à +0,04 à
4 kHz). Même cause que le LPF2 : le firmware applique RPM, notchs statiques et LPF1 à chaque boucle PID (4 kHz), le modèle les
calculait pour la cadence du log (2 kHz) ; théorie pour le LPF1 de l'Air65 (PT1 de 250 à 500 Hz) : +0,31 à +0,41 dB entre 100 et
250 Hz. Rejoués à la cadence PID (même fonction) : interpolation linéaire, +0,09 à +0,17 dB entre 100 et 250 Hz mais +0,94 à
+1,16 dB entre 250 et 500 Hz ; sinus cardinal, −0,23 à −0,15 et +0,11 à +0,19 dB. Reste à 2 kHz un petit écart négatif sous 250 Hz
(−0,35 à −0,15 dB) qu'aucune interpolation ne change, alors que la théorie leur prête à toutes moins de 0,03 dB d'erreur à
50–100 Hz : pas le modèle ; cause supposée, le repliement dans le log du bruit au-dessus de 1 kHz, qui agit sur la mesure comme un
bruit d'entrée.

**Après les corrections**, écarts signés (12 vols `GYRO_SAMPLE` ; bloc et notch dynamique : 4 vols `FFT_FREQ` à chaque boucle PID,
3 à 2 kHz ; D : 32 axes, log à chaque boucle PID seulement) :

| Étage | 5–50 Hz | 50–100 Hz | 100–250 Hz | 250–500 Hz |
|---|---|---|---|---|
| horloge (aucun filtre) | −0,02 à +0,03 dB | −0,22 à +0,00 dB | −0,09 à −0,01 dB | −0,07 à −0,00 dB |
| LPF2 | −0,04 à +0,00 dB | −0,22 à −0,02 dB | −0,09 à +0,00 dB | −0,04 à +0,01 dB |
| notchs RPM | −0,03 à +0,01 dB | −0,06 à +0,05 dB | −0,01 à +0,03 dB | +0,25 à +0,35 dB |
| notchs statiques + LPF1 | −0,01 à +0,03 dB | −0,04 à +0,10 dB | −0,01 à +0,07 dB | +0,03 à +0,22 dB |
| bloc LPF2 + RPM + LPF1 | −0,01 à +0,01 dB | −0,08 à +0,00 dB | −0,02 à −0,00 dB | +0,14 à +0,32 dB |
| bloc, log à 2 kHz | −0,03 à −0,00 dB | −0,35 à −0,17 dB | −0,23 à −0,15 dB | +0,11 à +0,19 dB |
| notch dynamique | −0,00 à +0,01 dB | −0,07 à +0,01 dB | −0,11 à +0,03 dB | −0,11 à −0,01 dB |
| notch dynamique, log à 2 kHz | −0,01 à +0,00 dB | +0,02 à +0,09 dB | −0,02 à +0,06 dB | +0,05 à +0,12 dB |
| filtres D | −0,00 à +0,00 dB | −0,27 à +0,01 dB | −0,28 à +0,02 dB | −0,43 à +0,01 dB |

Restes connus, tous sous 0,5 dB : les notchs RPM entre 250 et 500 Hz (+0,25 à +0,35 dB, cohérence 0,65 seulement : la bande des
moteurs) ; l'étage horloge en haute résolution, −0,1 à −0,2 dB entre 50 et 100 Hz, sans explication (piste supposée : repliement du
bruit au-dessus de 2 kHz, invisible dans un log à 4 kHz).

**Corpus** (`otb-dev corpus stages`, 22 s avec l'interpolation, 14 s avant ; avant = `main` au 2026-11-22, après = cette branche,
mêmes sessions ; |écart| médian (maximal), bandes concluantes) :

| Étage, log | 50–100 Hz avant → après | 100–250 Hz avant → après | 250–500 Hz avant → après |
|---|---|---|---|
| bloc gyro, chaque boucle PID (15) | 0,20 (0,52) → 0,05 (0,10) | 0,21 (0,76) → 0,06 (0,71) | 0,59 (2,78) → 0,35 (2,84) |
| notch dynamique, chaque boucle PID (15) | 0,19 (0,49) → 0,01 (0,09) | 0,35 (0,99) → 0,02 (0,15) | 1,05 (1,97) → 0,03 (0,22) |
| filtres D, chaque boucle PID (52 axes) | 0,27 (0,86) → 0,02 (0,27) | 0,22 (1,35) → 0,03 (0,43) | 1,36 (3,99) → 0,06 (0,69) |
| bloc gyro, une boucle sur 2 ou moins | 0,90 (5,20) → 0,20 (4,46) | 0,59 (3,88) → 0,28 (4,09) | 0,63 (1,90) → 0,22 (3,01) |
| notch dynamique, une boucle sur 2 ou moins | 0,76 (3,21) → 0,06 (0,94) | 0,73 (2,15) → 0,07 (2,41) | 1,12 (2,47) → 0,13 (4,51) |

Moyenne signée, avant → après, à chaque boucle PID : notch dynamique −0,21 → −0,00 (50–100 Hz), −0,41 → +0,01 (100–250 Hz),
−0,97 → −0,03 dB (250–500 Hz) ; D −0,29 → −0,07, −0,35 → −0,07, −1,24 → −0,11 dB ; une boucle sur 2 ou moins, notch dynamique
−0,93 → +0,06, −0,72 → +0,18, −1,10 → +0,38 dB. **Alertes : 2 → 0.**

**Garde-fou** (`MAX_INPUT_NOISE_SHARE`) : une case où l'arrondi dépasse cette part de l'énergie d'entrée sort des moyennes. À 0,5,
47 bandes de logs à cadence réduite empiraient de plus de 0,5 dB, jusqu'à +5 dB : vols stationnaires très calmes (91 à 97 % des
échantillons consécutifs identiques dans `debug[0]` : l'arrondi n'y est plus un bruit blanc), où la correction d'une case peut
atteindre +6 dB. À 0,25 : 32 (jusqu'à +4,2 dB) ; à **0,1** (retenu ; correction bornée à +0,9 dB par case) : 14, et à chaque boucle PID rien
n'empire, 2 bandes concluantes sur 511 perdues (mesuré avant l'interpolation à la cadence des filtres).

**Logs à cadence réduite, cas extrêmes (W-067)** : avec l'arrondi retiré et les filtres à leur cadence, 24 bandes empirent de plus
de 0,5 dB par rapport à `main` (14 avant l'interpolation), toutes vers le positif (le modèle filtre plus que le firmware, jusqu'à
+4,5 dB) ; 21 sur 24 entre 100 et 500 Hz, surtout dans le notch dynamique, cohérence 0,50 à 0,65, drones qui vibrent fort
(`… hover wobble 1900k…`, `Sasha/Foxwhoop`, `Sasha/Amax`) ; PID 4 kHz et log 2 kHz pour 13, PID 3,2 kHz et log 1,6 kHz pour 8,
PID 8 kHz et log 2 kHz pour 3. Cause supposée, non mesurée : le bruit au-dessus de la moitié de la cadence du log se replie dans le
log ; le firmware n'applique pas son notch à ce bruit (il est ailleurs en fréquence), le modèle si (il le voit à la fréquence
repliée). Aucun modèle ne peut le corriger : le contenu replié n'est pas dans le log. L'écart typique, lui, s'améliore (tableau
ci-dessus).

### Suivi du notch dynamique (2026-11-22, lot 22c)

Outil : `rust/otb-analysis/src/dyn_notch.rs:check_tracking` ; corpus : `(cd rust && cargo run --release -p otb-dev -- corpus dyntrack
SORTIE.json)` (21 s, 547 sessions `FFT_FREQ` dont le notch est actif, 1 099 notchs ; `--resume SORTIE.json` refait le tableau). Sur
l'axe debug, centres rejoués comparés aux centres enregistrés par le firmware, période de l'assistant. Trois sources : le suivi
rejoué sur le gyro **enregistré** avant le notch (`debug[0]`, contrôle du suivi seul), sur le gyro **modélisé** avant le notch
(gyro non filtré passé dans LPF2, RPM, notchs statiques et LPF1 : ce que le simulateur peut faire sans `FFT_FREQ`), et les
**centres initiaux** fixes (ce que fait le simulateur aujourd'hui sans `FFT_FREQ`, W-057). Écart absolu médian par notch, puis
médiane sur les notchs :

| Log | Notchs (sessions) | Depuis le debug | Depuis le modèle | Centres initiaux |
|---|---|---|---|---|
| chaque boucle PID, 2025.12 | 5 (4) | 0,4 Hz (99 % à moins d'une case) | 5,7 Hz (82 %) | 105,5 Hz (0 %) |
| chaque boucle PID, 2026.6 et 2026.12 | 16 (10) | 5,9 Hz (76 %) | 4,6 Hz (79 %) | 74,3 Hz (9 %) |
| une boucle sur 2 ou moins, 4.5 | 660 (322) | 14,7 Hz (55 %) | 18,7 Hz (50 %) | 79,4 Hz (8 %) |
| une boucle sur 2 ou moins, 2025.12 | 249 (131) | 8,5 Hz (65 %) | 12,2 Hz (59 %) | 89,2 Hz (4 %) |
| une boucle sur 2 ou moins, 2026.6 | 168 (79) | 7,8 Hz (70 %) | 10,2 Hz (63 %) | 54,0 Hz (9 %) |
| **tout le corpus** | 1 099 (547) | **11,8 Hz** (60 %) | **14,4 Hz** (54 %) | **78,0 Hz** (7 %) |

Le suivi rejoué est plus proche des centres enregistrés que les centres initiaux pour 1 061 notchs sur 1 099 depuis le debug,
1 026 depuis le modèle. Une case vaut 18,5 Hz dans le cas courant (PID 4 kHz, max 600 Hz).

- **Le suivi est fidèle** : à chaque boucle PID, `redhonu` (8 kHz) et `Oerni79` (3,2 kHz) donnent 0,3 à 0,4 Hz depuis le debug,
  99 à 100 % des échantillons à moins d'une case. Le reste de l'écart ne vient pas de l'algorithme.
- **Arrondi du gyro enregistré** : les sessions à 4 kHz de petits drones peu vibrants (Air65, vols stationnaires `KM HOVER`)
  restent à 2 à 13 Hz depuis le debug. Le debug est arrondi au °/s : essai jetable, l'entrée modélisée arrondie au °/s fait
  passer l'Air65 de 4–10 Hz à 6–15 Hz, le niveau obtenu depuis le debug. Sur un log enregistré une boucle sur deux, s'y ajoutent
  les boucles manquantes, interpolées.
- **Modèle de chaîne** : `Oerni79` (3,2 kHz, BMI270) est exact depuis le debug, mais à 7 à 14 Hz depuis le modèle ; le gyro
  modélisé avant le notch s'y écarte de celui du firmware (sujet du contrôle par étage, pas du suivi).
- **Contre-épreuve de l'arrondi de la cadence** (L-102) : sur les 107 notchs (42 sessions) où Nyquist / `dyn_notch_max_hz` est
  entier, forcer la cadence exacte fait passer l'écart médian de 5,6 à 14,9 Hz et la part à moins d'une case de 0,78 à 0,47 ;
  104 notchs sur 107 sont moins bien suivis. Le firmware 2026.6 se trompe donc bien d'un `sampleCount`, et le modèle avec lui.
- **Non vérifié** : le plancher au-dessus de 60 Hz (2026.12 compilé à partir du 2026-09-05) ne change rien sur les logs de
  l'Air65, dont la plage commence déjà au-dessus ; la variante 4.3 n'a aucun log `FFT_FREQ` dans le corpus (W-065).

### Simulateur de filtres rejoué (2026-11-22, lot 22e)

`otb-dev corpus replay` (D-058) : spectre du rejeu des réglages du log (« simulé actuel » de la page Filtres) contre le spectre
enregistré, période de l'assistant, moyenne du roulis et du tangage, écart moyen en dB dans chaque bande (positif : le rejeu
laisse passer plus que le firmware). 675 sessions avec gyro non filtré, 68 s pour le corpus entier.

| Logs | Sessions | Gyro 20–100 / 100–250 / 250–500 Hz : \|écart\| médian (90e centile) | Terme D, mêmes bandes |
|---|---|---|---|
| à chaque boucle PID, ≥ 1,6 kHz | 31 | 0,09 (0,16) / 0,10 (0,23) / 0,21 (0,32) dB | 0,11 (0,29) / 0,10 (0,54) / 0,25 (0,40) dB |
| une boucle sur 2 ou moins, ≥ 1,6 kHz | 493 (D : 490) | 0,47 (1,51) / 0,45 (1,13) / 0,39 (0,96) dB | 0,56 (2,27) / 0,66 (2,06) / 0,73 (2,29) dB |
| une boucle sur 2 ou moins, < 1,6 kHz | 151 | 3,03 (5,81) / 1,89 (6,24) / 1,61 (3,58) dB | 2,72 (7,29) / 1,95 (7,70) / 1,81 (4,40) dB |

Sur les logs ralentis, le rejeu dépasse le log (moyenne signée du gyro +0,4 à +0,8 dB, +1,8 à +3,3 dB sous 1,6 kHz), surtout
entre 20 et 100 Hz où les filtres n'agissent presque pas : le gyro non filtré y est sous-échantillonné sans anti-repliement,
et le bruit à haute fréquence replié passe dans le rejeu alors que le firmware l'avait filtré avant (supposé, même cause que
W-067). Le rapport « modifié / actuel » l'annule en grande partie, pas la courbe « actuel ». Sans arrondir le rejeu comme le
Blackbox, l'écart entre 250 et 500 Hz monte à −1,6 dB sur l'Air65 (L-103). Durée d'un rejeu : 1,25 s en médiane par session
avec tous les cœurs occupés, 16,6 s au plus (vol le plus long) ; 0,3 à 0,5 s par calcul sur un vol de l'Air65 seul.

Effet de changements de filtres, gyro modifié − log entre 100 et 250 Hz (Air65 `CHIRP-45Hz`, puis JeNo 5 `FINAL` #1), rejeu
contre l'ancien rapport des réponses |H|² : Q du notch dynamique divisé par 2, −1,49 / −2,21 dB contre −2,38 / −2,79 ; un notch
dynamique de plus, **+0,69 / +1,91 dB contre −0,02 / −0,97** ; plus de notch dynamique, +2,86 / +5,86 contre +6,26 / +7,66 ; RPM
coupé, +2,99 / +10,93 contre +0,15 / +13,50 ; LPF1 de l'Air65 × 0,7, −0,46 contre −0,83, et × 1,5, +1,30 contre +0,58. Le suivi
rejoué place le notch unique de l'Air65 sur le pic principal (médiane 183 Hz) ; avec deux notchs, triés par fréquence, plus
aucun n'y reste (169 et 258 Hz). Seul un vol avec un notch de plus peut trancher (22f, W-068).

### Prédiction vérifiée en vol (2026-11-22, lot 22f)

`otb-dev corpus predict AVANT APRÈS` (W-060) : les filtres du vol APRÈS appliqués au vol AVANT, prédiction comparée au vol
APRÈS, période de l'assistant, spectre moyen, changement en dB par bande ; les deux méthodes : « rejeu » (page Filtres, D-058)
et « rapport » des |H|² (assistant, D-008). Il donne aussi le changement du gyro **non filtré** d'un vol à l'autre, que le
simulateur garde tel quel (boucle fermée, batterie, gaz).

Vols : Air65 (`Cedric/Air65/Test BF Chirp`, Betaflight 2026.12, gyro 8 kHz, PID et log à 4 kHz). APRÈS :
`2DNF-FFT_FREQ_4k-CHIRP-65Hz` (`dyn_notch_count` 1 → 2, seul filtre changé ; haute résolution ; batterie à 3,65 V à
l'armement, gaz moyen 45 %). AVANT : les 16 vols du même jour à 4 kHz avec un seul notch (gaz moyen 36 à 45 %) ; les 3 vols
à 2 kHz sont écartés (cadence différente). Paire la plus proche : `GYRO_SAMPLE_Roll-HR-CHIRP-65Hz`, qui ne diffère que par le
nombre de notchs et le mode debug. Le rejeu reproduit chacun de ces vols à 0,02–0,07 dB entre 100 et 250 Hz (`corpus replay`).

Gyro filtré, roulis et tangage, 100–250 Hz (bande du notch dynamique), |prédit − mesuré| médian (maximum), 32 valeurs :

| Sens | Rejeu | Rapport | Rejeu, corrigé du non filtré | Rapport, corrigé | Non filtré, médiane |
|---|---|---|---|---|---|
| 1 → 2 notchs (AVANT → APRÈS) | **0,32** (1,77) dB | 1,95 (5,42) dB | 0,58 (2,21) dB | 1,62 (2,21) dB | +0,82 dB |
| 2 → 1 notch (APRÈS → AVANT) | **1,99** (4,93) dB | 4,01 (6,85) dB | 1,11 (1,72) dB | 3,08 (3,64) dB | −0,82 dB |

« Corrigé » : comparé au changement mesuré moins celui du gyro non filtré (effet des seuls filtres). Paire la plus proche,
gyro roulis / tangage : prédit −0,34 / −1,49 dB par le rejeu, −3,12 / −3,12 par le rapport ; mesuré −0,33 / −1,22 dB, non
filtré +0,87 / +0,52 dB. Terme D, même paire : −0,34 / −1,50 prédit, −0,54 / −1,44 mesuré.

- **Le rejeu prédit le mieux**, dans les deux sens. Le notch de plus **baisse** le bruit de ce vol (−0,3 à −1,6 dB) : le
  rejeu le voit, alors qu'il prévoyait +0,69 dB depuis le vol `CHIRP-45Hz` (section précédente) ; mesuré depuis ce vol-là :
  +1,87 dB en roulis, mais le gyro non filtré y a monté de 3,3 dB (vols d'un autre moment) : corrigé, −1,4 dB.
- Le rapport dépend du mode debug du vol AVANT (W-057) : −3,12 dB sans `FFT_FREQ` (notchs à leurs centres initiaux), −0,02 dB
  avec (centres du debug, que le second notch ne déplace pas).
- Retirer le second notch : le rejeu prévoit +2,6 dB, les vols montrent +1,2 à +1,7 dB une fois corrigés du non filtré. Il
  surestime donc la perte d'un notch d'environ 1 dB (cause non cherchée).
- Boucle fermée : non mesurable ici. Le gyro non filtré a monté de 0,8 dB en médiane avec deux notchs, mais deux vols aux
  mêmes réglages varient déjà de 0,4 à 1,0 dB, et le vol APRÈS a été fait avec la batterie la plus basse et les gaz les plus
  hauts. Une seule paire de réglages, un seul vol APRÈS : ces chiffres valent pour ce changement, pas pour un LPF ou le RPM.

### Assistant passé au rejeu (2026-11-22, D-059)

`otb-dev corpus rates` avant et après sur 676 sessions (`--compare`) : **un seul conseil change**, « le LPF1 du gyro peut
être coupé » disparaît sur `1. PTB/Kris/Wobble.bbl` (gyro prévu à −33,4 dB par l'ancienne méthode, à 2 dB du seuil de
−31,4 ; le rejeu le met au-dessus ; log enregistré une boucle sur 2, W-067). 162 sessions gardent les mêmes constats avec
d'autres chiffres prévus. Écart rejeu − ancienne méthode, valeur affichée :

| Constat | Valeur | Sessions | Médiane | 10e / 90e centile | Extrêmes |
|---|---|---|---|---|---|
| Couper le LPF1 du gyro | gyro filtré | 51 | +1,0 dB | 0,0 / +3,7 dB | −0,4 / +4,8 dB |
| Couper le LPF1 du gyro | terme D | 51 | +0,8 dB | +0,1 / +3,6 dB | −0,2 / +4,9 dB |
| Relever les filtres du D | terme D | 124 | 0,0 dB | 0,0 / 0,0 dB | 0,0 / 0,0 dB |

L'ancienne méthode sous-estimait donc le bruit ajouté en coupant un filtre du gyro ; pour les filtres du D seuls (pas de
notch suivi), les deux méthodes concordent au dixième de dB. Durée : 50 → 71 s pour le corpus entier, tous les cœurs
occupés ; une session seule, 0,2 à 0,8 s de plus (0,26 → 0,45 s, 0,53 → 1,31 s, 0,80 → 1,38 s sur les trois plus lentes).
`otb-parity check` (références fonction par fonction) : 0 écart.

### Ordre du terme D (2026-11-22, lot 33)

Le rejeu (page Filtres, assistant) et le contrôle par étage passent par `rust/otb-analysis/src/bf_filters.rs:simulate_dterm_at`,
qui suit désormais le firmware (`flight/pid.c`) : chaîne D sur le gyro, **puis** `−dg/dt` ; il dérivait avant de filtrer
(W-072). `simulate_dterm` garde l'ancien ordre pour les références Python figées : `otb-parity check`, 0 écart (20 appels).
Mesures avant / après sur le corpus entier (676 sessions), même binaire hormis l'ordre :

- **Spectres exacts du D rejoué** (programme jetable, gyro du log, cadence du modèle) : 0,001 dB au plus entre les deux ordres
  sur les vols aux gaz calmes, **−0,03 à −0,06 dB** entre 20 et 100 Hz et −0,008 dB entre 100 et 250 Hz sur
  `Yohann/Gerry Mini/stocklp1.bbl` (gaz de 10e à 90e centile étalés sur 52 %, LPF1 D dynamique). L'ordre ne compte que quand la
  coupure du LPF1 D suit des gaz qui bougent, et peu.
- **Courbe « actuel » de la page Filtres** (`otb-dev corpus replay`, D arrondi à l'unité comme le log), |après − avant| :

| Logs | LPF1 D | Gaz p90 − p10 | Sessions | 20–100 / 100–250 / 250–500 Hz : médiane (max) |
|---|---|---|---|---|
| à chaque boucle PID | dynamique | < 20 % | 29 | 0,000 (0,002) / 0,000 (0,003) / 0,001 (0,010) dB |
| à chaque boucle PID | dynamique | ≥ 40 % | 2 | 0,010 (0,019) / 0,014 (0,017) / 0,009 (0,023) dB |
| une boucle sur 2 ou moins | dynamique | < 20 % | 493 | 0,001 (0,384) / 0,001 (0,341) / 0,001 (0,256) dB |
| une boucle sur 2 ou moins | dynamique | 20–40 % | 26 | 0,001 (0,013) / 0,003 (0,046) / 0,002 (0,013) dB |
| une boucle sur 2 ou moins | dynamique | ≥ 40 % | 39 | 0,007 (0,049) / 0,008 (0,052) / 0,012 (0,053) dB |
| une boucle sur 2 ou moins | fixe | toutes | 83 | 0,000 (0,019) / 0,000 (0,019) / 0,000 (0,008) dB |

  Les maximums de 0,25 à 0,38 dB viennent de deux vols aux gaz presque immobiles (`1. PTB/NAiTiNAiN/03Hover.bbl`, un vol de
  `1. PTB/ram`) dont le D enregistré est sous l'unité : l'arrondi, pas l'ordre (spectres exacts : 0,001 dB, L-108). Écart de la
  courbe « actuel » au log, médiane : inchangé au centième dans chaque groupe (0,111 → 0,112 dB, 0,100 → 0,100, 0,256 → 0,257
  sur les 29 vols à chaque boucle PID).
- **Contrôle par étage, filtres D** (`otb-dev corpus stages`, 56 axes à chaque boucle PID) : |écart| médian inchangé entre 5 et
  100 Hz (0,001 et 0,025 dB), 0,027 → 0,031 dB entre 100 et 250 Hz (maximum 0,428 → 0,433), 0,059 → 0,058 dB entre 250 et 500 ;
  résidu médian 3,71 % inchangé, maximum 26,43 % inchangé. Ni amélioration ni dégradation mesurable : les logs à chaque boucle
  PID du corpus ont peu de gaz qui bougent (2 sur 31).
- **Assistant** (`otb-dev corpus rates --compare`) : **aucun constat ne bascule** ; 3 sessions sur 676 voient un chiffre du
  conseil « Le LPF1 du gyro peut être coupé » bouger (terme D prévu −0,1 dB, sommet du D −1 dB PIDtoolbox à l'arrondi).
- Durées inchangées : rejeu 66 s, contrôle par étage 21 s, assistant 61 s pour le corpus entier.

### Chaîne décrite en données (2026-11-23, lot 23)

L'ordre et la cadence des filtres ne sont plus écrits en dur : la réponse en fréquence (`gyro_chain`, `dterm_chain`), les
étages de la simulation, le rejeu, le suivi du notch dynamique, le contrôle par étage et la dérivée du D les lisent dans
`rust/otb-analysis/chains/betaflight.json` (`chain.rs`, D-062). Le changement ne devait **rien** changer ; mesuré avant / après
sur le corpus entier (676 sessions), binaires de `main` mis de côté avant de toucher au code :

- `otb-parity check` : 1 732 appels rejoués, **0 échec à rtol = 0** (au bit près) ;
- `otb-dev corpus replay`, `corpus stages`, `corpus dyntrack` : fichiers de résultats **identiques**, hormis la durée de chaque
  session pour le rejeu ; `corpus rates --compare` : **0 session** aux constats différents ;
- captures de l'exemple `capture` sur `Cedric/JeNo 5/FINAL.bbl` (42 images, page Filtres comprise, deux thèmes) : **identiques
  octet pour octet** (deux passages de l'ancien binaire donnent déjà les mêmes octets) ;
- contre-preuve : LPF1 placé avant les notchs statiques dans le fichier → 6 tests rouges, dont `gyro_chain_identique` et
  `simulate_gyro_dans_la_tolerance` (références Python) ; LPF2 passé à la cadence PID → 14 tests rouges, dont
  `lpf2_calcule_a_la_cadence_gyro` (L-104) et le rejeu de la page Filtres ;
- durées sans changement notable : rejeu médian par session 1,19 → 1,15 s ; durée cumulée de l'assistant 1 018 → 945 s, tous
  les cœurs occupés (avant : rejeu 67 s, contrôle par étage 21 s, suivi du notch 19 s, assistant 62 s pour le corpus entier).

Critère d'arrêt du lot (plus de 0,5 dB sous 250 Hz, ou un constat qui bascule) : non atteint. L'effet attendu « faible » est
confirmé : quelques centièmes de dB au plus ; pas de capture, la courbe D ne bouge pas à l'œil.

### Briques à l'arithmétique du firmware (2026-11-24, lots 24a et 24b)

Briques float32 de `rust/otb-dsp/src/firmware/betaflight/` (pas encore branchées) contre le firmware **compilé sur PC** par
`otb-dev firmware-bench` (D-062) : `common/filter.c` et `maths.c` de chaque version téléchargés, compilés avec le programme du
projet `rust/otb-dev/bench/betaflight_filters.c` (GCC 15.2, `-O2 -ffp-contract=off`), dans le dossier temporaire du système.

| Version jouée | Arithmétique | Cas | Identiques au bit près |
|---|---|---|---|
| 4.3.2 | `V4_3` | 23 | 23 |
| 4.4.3 | `V4_3` | 23 | 23 |
| 4.5.5 | `V4_5` | 23 | 23 |
| 2025.12.5 | `V2025_12` | 23 | 23 |
| 2026.6.2 | `V2026_6` | 24 | 24 |
| 2026.12 (commit `1b94a3871`, pas encore d'étiquette) | `V2026_6` | 24 | 24 |

- Cas joués : sinus et cosinus sur 2 048 angles (de −36 à 36 rad, multiples de π/2 et leurs voisins au bit près), gains
  PT1/PT2/PT3 sur 168 couples (fréquence, pas de temps), Q des notchs sur 60 couples, et sur un signal de 3 072 échantillons
  (balayage de 20 à 3 800 Hz, bruit, échelons) : PT1/2/3 fixes et à coupure variable, biquad ou SVF passe-bas fixe et
  dynamique, notchs statiques, notch à centre variable, notch RPM à centre et poids variables (trois axes en SVF), et trois
  chaînes à deux cadences (LPF2 à la cadence gyro, décimation par 2, 3 ou 4, notch et LPF1 à la cadence PID). Vecteurs :
  1,6 Mo dans `rust/otb-dsp/tests/data/firmware/betaflight/`, rejoués par `rust/otb-dsp/tests/firmware_vectors.rs` (CI).
- **Le banc distingue bien les versions** : 4.3.2 et 4.4.3 donnent les mêmes bits, comme 2026.6.2 et 2026.12 ; entre 4.4.3 et
  4.5.5, le gain des PTn change (264 gains sur 504 ; jusqu'à 2 354 sorties sur 3 072 d'un PT2) ; entre 4.5.5 et 2025.12.5,
  seul le cosinus approché change (2 261 valeurs sur 4 096), sans effet sur les sorties des biquads de ces cas.
- Reproductible : deux passages complets du banc écrivent des fichiers identiques (empreintes SHA-256).
- **Contre-preuves** (une erreur introduite, le test rougit) : PT1 écrit `(1 − k)·état + k·entrée` → rouge (2 636 valeurs
  sur 3 072) ; DF1 du biquad avec les additions regroupées autrement → rouge ; 2025.12 avec le sinus de la 4.5 → rouge ;
  notch SVF `a2q = f·(1/d·q)` au lieu de `(f·1/d)·q` → rouge ; moyenne de la décimation `× 1/n` au lieu de `÷ n` → **vert**
  d'abord (décimations par 2 et 4 seulement, L-118), rouge après l'ajout du cas par 3 (919 valeurs sur 1 024). Sans effet, et
  c'est attendu : `powf` des 4.3–4.4 calculé par la bibliothèque du système (même valeur ici que l'évaluation de GCC) et
  `BIQUAD_Q` remplacé par `FRAC_1_SQRT_2` (mêmes bits que `1.0f / sqrtf(2.0f)`).
- Ce que le banc ne vérifie pas (W-081) : l'ordonnancement des recalculs dans la boucle (notchs RPM un à un, LPF dynamiques),
  le suivi des pics du notch dynamique, et la fusion multiplication-addition que le compilateur de la carte de vol emploie.

### Bibliothèque de données en fichiers JSON (2026-11-18, lot 28)

Mesuré par `rust/otb-store/tests/bibliotheque.rs` (`a_volume_reel_les_fichiers_restent_petits_et_louverture_rapide`),
poste Linux, profil de test. Volume simulé : **535 logs** (la taille du corpus de Cédric), 4 fragments par log, et
12 grandeurs × 3 axes par fragment, soit **2 140 fragments et 77 040 mesures**.

| Grandeur | Mesure |
|---|---|
| Tous les fichiers de la bibliothèque | **9 Mo**, dont **8 Mo** pour les seules mesures |
| Écriture complète | 928 ms |
| Ouverture complète | 959 ms |

**Ce que ces chiffres disent.** Le déclencheur écrit d'avance dans [plan.md](plan.md) — « un fichier qui dépasse
quelques Mo, un lancement visiblement plus lent » — est **atteint au volume plein**, et seulement par la table des
mesures. Les cinq autres tables réunies pèsent 1 Mo. Deux conséquences, dans cet ordre : la table des mesures ne doit
pas se charger au lancement mais à la première question posée (lot 31), et le passage à une vraie base est un
**quand**, pas un **si** — ce pour quoi tout passe par un seul module d'accès.

**Au passage, un piège mesuré** : avec les mesures rangées en liste, chaque enregistrement parcourait toute la table
et le test mettait **47 s** ; rangées par clé (fragment, axe, grandeur), **2,1 s**. Le commentaire de `measures` dans
`rust/otb-store/src/lib.rs` porte le chiffre.

### Mesures suivies dans le temps (2026-11-23, lot 31a)

**Table des mesures lue à la demande** (D-064), même test et même volume que ci-dessus, poste Linux, 10 Mo de mesures
(chaque mesure porte maintenant les bornes de son fragment) :

| Grandeur | Profil de test | `--release` |
|---|---|---|
| Ouverture de la bibliothèque, sans les mesures | 34 ms | 5 ms |
| Lecture de la table des mesures (sur un autre fil) | 1 195 ms | 308 ms |
| Écriture complète | 884 ms | 204 ms |

L'ouverture ne paie plus la table des mesures ; sa lecture se fait hors du fil de l'écran, et une table inchangée n'est
plus réécrite à chaque enregistrement automatique. Le passage à SQLite (D-064, « Ouvert ») n'est pas déclenché.

**Calcul sur de vrais logs** (`otb_advisor::tracked::measure`, `--release`, limites automatiques du vol, chaque session
mesurée deux fois) : 11 sessions de `Cedric/JeNo 5` (FINAL, LAST), `Air65`, `Apex 5` et `BF Chirp`, de 1 à 194 s de vol.
**Les deux calculs sont identiques au bit près** sur les 11 (le critère d'arrêt du lot 31). Durée : 0,06 à 3,5 s par
fragment (3,5 s pour 99 s de vol à 8 kHz), en arrière-plan. Grandeurs non mesurées (NaN) : 1 à 5 sur 22 pour un vrai vol,
le plus souvent la marge de gain ou un axe sans assez de mouvements ; 17 sur 22 pour une session d'une seconde.

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
