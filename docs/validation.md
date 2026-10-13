# Validation

Mesures du 2026-10-01 sur le corpus de Cédric. À refaire après toute modification du décodeur ou des
analyses (`scripts/validate_corpus.py`, tests `-m corpus`).

## Décodeur

`scripts/validate_corpus.py` compare chaque session à `blackbox_decode` (fourni avec PIDtoolbox), alignée sur `time` :

- 568 fichiers décodés ; **95 % des sessions identiques** valeur par valeur.
- Les écarts restants sont des trames corrompues que la référence accepte : trames tronquées avant un trou
  d'écriture sur la carte SD (`gap`), trames S parasites (`slow`), valeurs impossibles (gyro ou consigne
  > 4000 °/s, terme PID > ±65535). OpenTuneBox les rejette.
- La référence se bloque sur 3 fichiers (`refhang`) qu'OpenTuneBox lit.
- Démarrage à froid du décodeur ≈ 0,6 s (cache numba).
- Un vrai extrait est versionné pour la CI : `tests/data/jeno5_5s.bbl` (5,1 s du début de la 1re session de
  `Cedric/JeNo 5/FINAL.bbl`, Betaflight 2026.6.0-rc3, 8 262 trames, coupé sur une trame I). Statut `ok` face à
  `blackbox_decode` le 2026-10-12 ; `tests/test_real_log.py` fige l'en-tête, la première et la dernière trame,
  les statistiques et les unités physiques.

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
Découpe « stationnaire / mouvements » du guide (`period.flight_parts`, blocs de 1 s lissés sur 3) : dans les
83 vols stationnaires, 1,6 % des blocs dépassent 30 °/s (0,3 % à 50 °/s) ; dans les vols au-delà de 100 °/s,
79,7 % (74,6 %). Un vol freestyle n'a pas de vrai stationnaire : la plus longue partie calme peut changer
d'endroit avec le seuil (`Cedric/JeNo 5/FINAL`, `tests/test_tuning_path.py`).

Écart connu : la latence 50 % peut différer d'environ 1 ms (interpolation au passage exact à 50 % contre
lecture échantillon par échantillon).

## Autres contrôles

- Curseurs simplifiés : les formules redonnent les PID de l'en-tête pour 740 sessions sur 741.
- Simulateur de filtres : sans modification des réglages, le spectre « et si » égale le spectre loggé à 0,009 dB près.
- Filtres Betaflight : réponses comparées aux formules analytiques (`tests/test_bf_filters.py`).
- Marges de stabilité (`analysis/loop.py`) : marge de phase, croisement et marge de gain égaux aux
  formules exactes d'une boucle intégrateur + retard (`tests/test_loop.py`, ±0,3° et ±0,05 dB), y compris
  une marge négative. Sur 60 logs du corpus tirés au hasard (133 axes, 2026-10-05) : marge inchangée à
  1° près pour 83 des 122 axes mesurables ; une marge passe de +299° (phase repliée, ancien calcul) à
  −109° ; les 8 écarts de plus de 5° sont des axes très bruités (yaw, vol stationnaire, phase qui saute de
  50 à 100° d'un point au suivant). Prendre la plus petite marge sur **tous** les croisements |L| = 1 a été
  essayé puis écarté : |L| repasse au-dessus de 1 vers 100 Hz sur les logs bruités et donnait des marges
  de −1100°. Le pic de sensibilité tombe au bord de la bande analysée pour 57 axes sur 133.
- Boucle ouverte mesurée (`analysis/loop.py`, 2026-10-05) :
  - **simulation** d'une boucle Betaflight (`tests/bf_loop_sim.py` : P, I sur l'erreur, D sur le gyro
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
  - **corpus** (`scripts/calibrate_advisor.py`, 539 logs, 681 sessions, 1253 axes roll / pitch) : boucle
    mesurée pour 88 % des axes (objectif ≥ 80 %), croisement trouvé dans la bande pour 70 % d'entre eux,
    pic de sensibilité au bord (minorant) pour 34 %. Médianes, mesurée contre ancienne formule :
    croisement 14,7 contre 8,8 Hz, marge de phase 50 contre 65°, pic de sensibilité 3,7 contre 3,2 dB ;
    le D domine au croisement (|Ĉd/Ĉe| > 1) pour 62 % des axes. Avec les seuils actuels, sessions signalées :
    pic > 6 dB 29 % (14 % avant), marge < 40° 3 % (1 % avant) ;
  - `test_matches_ptb_damping_sweep` inchangé (partie Wiener identique).
- Interface : les 8 outils s'ouvrent et calculent sans erreur sur de vrais logs (`tests/test_gui_spectral.py`,
  `tests/test_gui_step.py`, `tests/test_advisor.py`).
- Documentation : liens, chemins, outils, modules, fichiers publiés et langue vérifiés à chaque `pytest`
  (`tests/test_docs.py`).
