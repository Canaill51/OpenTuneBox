# Plan de travail

**Le seul plan du projet.** Ce qu'il reste à faire est en haut ; l'historique des lots déjà faits est un
tableau en bas, une ligne par lot. Il n'y a **pas d'autre document de plan** : l'état d'un travail reporté
se lit dans [travaux-reportes.md](travaux-reportes.md), jamais ici (L-092).

**État au 2026-11-10, vérifié** : la **0.9.0 est publiée** (page « dernière version », AppImage et
installateur Windows), les huit outils de PIDtoolbox Pro V0.94 sont tous couverts
([spec-ecrans.md](spec-ecrans.md), `rust/otb-gui/src/tools/mod.rs:all_pages`), `main` est propre, la CI est
verte, et `cargo run -p otb-dev -- verify --strict` avec le corpus, les références figées et la comparaison
au bit près : **0 saut, tout passe**.

Le programme est donc complet et publié, et depuis le 2026-11-10 **il ne reste que du confort** (lot 15 ;
le lot 11 est fait le même jour). Les deux autres familles ont disparu le même jour : vérifier ce que seule une vraie machine vérifie
(lot 13) n'avait plus d'objet, **personne n'ayant jamais utilisé la 0.8.0** ; et **plus aucun seuil de
l'assistant n'est sans origine ni condition** (lot 14), sans qu'une seule valeur ait eu à changer. Le 2026-11-12, le
lot 21 a relu les licences livrées et les sources, en vue de la **0.10.0** (la 1.0 est gardée pour plus tard).

**Chantier ouvert le 2026-11-18 — bibliothèque, refonte et USB.** Deux études de Cédric, hors dépôt,
ouvrent un chantier bien plus proche de l'utilisateur : un **canevas de neuf maquettes**
du 2026-11-16 (tableau de bord par drone, navigation à deux niveaux, espace « Bruit et filtres » fusionné, écran
d'extraction d'un **fragment** de vol, assistant en cartes) et une **spécification** du 2026-11-14 (récupération
automatique des logs quand on branche un drone en USB, **stockage** des logs et des fragments, petite base
**SQLite**, suivi des mesures dans le temps contre une fiche cible). Repris en lots **28 à 32**, dont le **28 est fait**
(2026-11-18, crate `otb-store`) : la bibliothèque de données d'abord, parce que les fragments, la refonte et l'USB
s'appuient tous dessus. Celui-ci
**change la vie d'un pilote** — moins de gestes entre l'atterrissage et l'analyse, et des logs qui ne se perdent
plus. Deux arbitrages sont déjà rendus (2026-11-18) : le stockage se fait en **fichiers JSON** et non en base
(lot 28), et **D-015 est amendée** pour laisser une entrée de navigation regrouper plusieurs vues (lot 30). Il ne
reste qu'une seule dépendance nouvelle à introduire, pour l'USB (lot 32).

**Repris d'un fork le 2026-11-22.** Les lots 28, 29a et 30a à 30c bis ont été écrits du 2026-11-18 au 2026-11-19 dans
le fork `GerryNatif/OpenTuneBox` (sa demande de fusion n° 1), parti de la 0.10.0 ; ils sont repris ici par-dessus le lot 22,
avec leurs registres renumérotés — deux décisions devenues **D-060** et **D-061**, un travail reporté devenu **W-069** —
parce que leurs numéros du fork étaient déjà pris ici. Le même fork porte un second chantier, **scripts firmware** (ses
lots 22 à 27 et sa décision sur la forme d'un script), écrit le
2026-11-18 **avant** le lot 22 de ce dépôt. Il est **repris dans les registres le 2026-11-22** (sa demande de fusion
n° 2), relu contre le rejeu de la chaîne de filtres (D-058) : décision **D-062**, travaux reportés **W-071** et **W-072**,
contrat d'un script en [spec-betaflight.md](spec-betaflight.md) §8. Cédric a confirmé ses quatre arbitrages du 2026-11-18
et en a ajouté trois : le chantier est **conditionnel** (lots 23 à 25, en attente de vrais logs INAV ou ArduCopter :
pour Betaflight seul, le lot 22 a déjà vérifié le simulateur sur de vrais vols) ; le lot 24 se prouvera par le banc
**et** par le contrôle par étage du lot 22 ; et le seul écart qui touche déjà l'écran, l'**ordre du terme D**, est
corrigé à part (lot 33). Ses lots 26 (INAV) et 27 (ArduCopter) restent sortis du plan.

## Comment s'en servir

- **Une session = un lot = une PR.** Faire `/clear`, puis demander « Lot N de `docs/plan.md` ». La branche
  porte le nom indiqué dans le lot ; la PR a pour titre une phrase en français qui dit l'effet du changement et
  une seule étiquette `RN: …` (`CONTRIBUTING.md`, D-056).
- Chaque lot suit les phases **explorer → planifier → implémenter → vérifier → livrer → consigner**.
  Explorer = lire les fichiers listés dans « À lire ». Planifier = présenter le découpage et attendre
  l'accord de Cédric. Consigner = cocher les cases du lot, mettre à jour les registres ([decisions.md](decisions.md), [lecons.md](lecons.md),
  [travaux-reportes.md](travaux-reportes.md)) selon la compétence `docs-sync`.
- **Un nouveau test se prouve par contre-preuve** : réintroduire le défaut (ou le casser à la main), voir le
  test rougir, le noter dans la PR (L-071).
- **Vérification de tout lot** : `cargo run -p otb-dev -- verify` **sans tube** (un `| tail` masque son
  échec, L-022), vert avant de livrer.
- **Critère d'arrêt** : si une hypothèse d'un lot tombe, s'arrêter, consigner ce qui a été vu, et revenir
  vers Cédric ; ne pas improviser un autre découpage.
- Modèle conseillé par lot : **Sonnet** pour le mécanique, la documentation, les tests ; **Opus** pour le
  décodeur, la mémoire et le débogage difficile.
- La numérotation des lots est une **série unique**, jamais réutilisée : les lots 0 à 14 et 16 à 21
  sont au tableau en bas (faits, non retenus ou supprimés), le 15, le 22 et les 30 à 32 sont ouverts (les 28, 29 et 33
  sont faits) ; les **23 à 25** sont le chantier des scripts firmware, **conditionnel** (D-062), et les **26 et 27** en sont
  sortis le 2026-11-18 ; le prochain numéro libre est **34**.

## Ordre des étapes

| # | Lot | Sujet | Dépend de |
|---|---|---|---|
| 1 | **22** | Calibration des filtres étage par étage : un seul mode debug (`FFT_FREQ`) et un simulateur vérifié contre le firmware (W-057, W-059, W-060, L-100) | 22a, 22a bis (24 alertes → 2, acceptées le 2026-11-22) et 22c (suivi rejoué, pas encore branché) faits ; 22b fait (étages séparés ; arrondi de l'entrée et LPF2 corrigés dans le contrôle, 0 alerte) ; 22d et 22e faits (retard gyro sans décalage d'horloge ; page Filtres rejouée, D-058, W-057, W-059 et W-064 traités) ; 22f fait (le rejeu prédit mieux que le rapport ; l'assistant passe au rejeu, D-059) |
| 2 | **15** | Dette de l'interface (W-052, pages spectrales) — **conditionnel** | le volet « cache » est soldé (L-094, D-053) ; le reste attend un déclencheur mesuré |
| 3 | **30** | Refonte de la navigation et de la mise en page (les neuf maquettes) | lot 29 (fait) ; 30a à 30e faits (rail, espace « Bruit et filtres », tableau de bord, fiche drone, choix du signal) ; rail aux sept entrées du prototype depuis le 29b ; 30e fait (choix « Gyro | Terme D » et axes partagés) ; 30f fait (assistant en cartes, D-070) ; reste : avancement du parcours (30g, avec W-074) |
| 4 | **31** | Mesures suivies dans le temps et fiche cible par drone | lot 29 (fait) |
| 5 | **32** | Récupération des logs par USB (MSP / MSC) — le plus risqué | 32a (protocole, sans interface) **fait le 2026-11-22** (crate `otb-usb`, essai sur une vraie carte par `otb-dev usb`) ; 32b (écran, stockage de masse, bibliothèque) : lot 30 |
| — | **23** | Chaîne Betaflight décrite en données, à résultat identique — **conditionnel** | D-062 : de vrais logs INAV ou ArduCopter, et la demande de Cédric |
| — | **24** | Briques à l'arithmétique du firmware, vecteurs de référence **et bascule** — **conditionnel** | lot 23 |
| — | **25** | Plages de versions et veille des changements du firmware — **conditionnel** | lot 24 |

Il ne reste que du **confort**, mesuré comme tel : le budget de 60 images par seconde est tenu avec une marge
de 3 sur la plus longue session du corpus. Les lots qui touchaient à un utilisateur réel ont été réglés le
2026-11-10 : le **13** s'est avéré sans objet (personne n'a eu la 0.8.0), le **14** est fait sans qu'aucune
valeur de seuil ait eu à changer, et le **11** a livré l'ouverture de plusieurs logs à la fois (× 2,26). Il ne
reste que le **15**, dont le volet « cache des outils » est **soldé le 2026-11-10** : mesuré (L-094), allégé, puis borné
(D-053) — **W-035 est clos**. Ses autres volets (tracé du viewer, pages spectrales) attendent toujours leur déclencheur
mesuré : ils peuvent attendre que les retours des pilotes disent ce qui gêne vraiment.

Le **2026-11-22**, Cédric a ouvert le **lot 22** : ce n'est pas du confort, mais la **précision du simulateur de
filtres**, jusqu'ici jamais vérifiée filtre par filtre contre le vrai firmware.

---

## Lot 22 : calibration des filtres étage par étage (Opus)

**But** (Cédric, 2026-11-22) : **un seul mode debug pour les filtres** (`FFT_FREQ`) et un simulateur **cohérent avec le
firmware**, qui montre le gyro et le terme D **avant et après filtrage**, mesurés et simulés, pour les réglages actuels
comme pour des réglages modifiés. Les modes `GYRO_SAMPLE` et `D_LPF` ne servent qu'**une fois**, pour la calibration.

**Principe** : comparer le modèle au firmware **un étage à la fois**. Le modèle filtre le signal enregistré à l'entrée
d'un étage, et on compare son résultat au signal enregistré à la sortie : les erreurs ne s'additionnent pas d'un
étage à l'autre, et un écart désigne un filtre précis. Points de mesure du firmware (`sensors/gyro_filter_impl.c`,
`flight/pid.c`) :

| Étage | Entrée → sortie enregistrées | Mode debug |
|---|---|---|
| LPF2 (cadence gyro) | `GYRO_SAMPLE[0]` → `[1]` | `GYRO_SAMPLE` |
| notchs RPM | `GYRO_SAMPLE[1]` → `[2]` | `GYRO_SAMPLE` |
| notchs statiques + LPF1 | `GYRO_SAMPLE[2]` → `[3]` | `GYRO_SAMPLE` |
| LPF2 + RPM + notchs statiques + LPF1 (en bloc) | `gyroUnfilt` → `debug[0]` | `FFT_FREQ` |
| notch dynamique | `debug[0]` → gyro, centres dans `debug[1…]` | `FFT_FREQ` |
| filtres D | −dg/dt du gyro enregistré → `axisD` (gain Kd × TPA normalisé) | aucun, si log à chaque boucle PID et `d_max` = D |

`GYRO_SAMPLE` et le debug `FFT_FREQ` ne couvrent qu'un axe (`gyro_filter_debug_axis`, en-tête `gyro_debug_axis`) ; `D_LPF`
ne couvre ni le yaw ni rien que le gyro enregistré ne donne déjà, d'où « aucun » pour les filtres D.

**Déjà mesuré** (2026-11-22, essai jetable hors dépôt, 3 logs `$OTB_LOGS/Cedric/Air65/Test BF Chirp/`, Betaflight
2026.12, gyro 8 kHz, PID 4 kHz, log à chaque boucle PID, `FFT_FREQ`, balayage chirp du firmware) : jusqu'à 250 Hz, le
modèle actuel suit le firmware à **0,2 dB** près pour le bloc LPF2 + RPM + LPF1, **0,4 dB** pour le notch dynamique et
**0,6 dB** pour les filtres D ; résidu temporel ≈ 1 % (gyro) et 2 à 4 % (D). Au-dessus de 250 Hz, la cohérence tombe
sous 0,3 (valeurs enregistrées arrondies au °/s) : pas de conclusion possible. Un écart reste à trancher : notch
dynamique entre 250 et 500 Hz, −1,8 dB réels contre −0,3 dB modélisés, cohérence 0,65 à 0,7 (tranché en 22b : biais de la
mesure, L-103). Et le décalage d'horloge de L-100 (0,11 ms).

**À lire** : [travaux-reportes.md](travaux-reportes.md) W-057, W-059, W-060 (et W-058, W-061, W-062, mêmes fonctions),
[lecons.md](lecons.md) L-100, L-011, L-024, [decisions.md](decisions.md) D-008, D-022, D-041,
`rust/otb-analysis/src/bf_filters.rs:simulate_gyro`, `rust/otb-analysis/src/bf_filters.rs:simulate_dterm`,
`rust/otb-analysis/src/delay.rs:compute_delays` ; firmware : `sensors/gyro_filter_impl.c`, `flight/dyn_notch_filter.c`,
`common/sdft.c`, `fc/core.c` (lire les sources clonées, ne rien en copier ; jamais de code du simulateur externe de
Cédric, sous GPL : D-004).

- [x] **22a — Comparaison par étage** (branche `feat/filter-stage-check`, 2026-11-22) : fait —
      `rust/otb-analysis/src/filter_stages.rs`, `otb-dev corpus stages`, logs à 1,6 kHz ou plus (D-057). Résultats dans
      [validation.md](validation.md) : sur les logs chirp, écarts de 0,2 à 0,8 dB sous 250 Hz, retard résiduel nul une fois
      L-100 retiré ; l'idée d'un biais de la mesure est testée et écartée. **Le critère d'arrêt est atteint** (ci-dessous) :
      7 sessions sur 18 à pleine cadence dépassent 1 dB. Le texte d'origine de 22a suit.
      Comparaison par étage : fonctions pures dans `otb-analysis` qui, pour
      un log, rendent par étage et par bande de fréquences le gain et la phase mesurés et modélisés, l'écart en dB, le
      retard en ms, la cohérence et le résidu temporel ; décalage de L-100 retiré ; étages `GYRO_SAMPLE` nommés dans
      `otb-log` quand le log en a. Test sur un log fabriqué dont les canaux debug sortent de filtres connus (écart nul),
      contre-preuve en faussant un filtre (L-071). Puis passage sur **tout le corpus `FFT_FREQ`** (compétence
      `calculs-corpus`) : écarts par version du firmware et par type de filtre, résultats dans [validation.md](validation.md).
- [x] **22a bis — Examen des alertes de 22a** (branche `fix/filter-stage-alerts`, 2026-11-22) : fait — **24 alertes → 2**
      ([validation.md](validation.md)). Causes mesurées : deux **contrôleurs surchargés** dont la boucle tourne moins vite que
      l'en-tête alors que le firmware calcule ses filtres pour la cadence annoncée (`petrel`, `redhonu` ; L-101 : piste absente
      du plan, trouvée en lisant l'horloge du log) ; une **récupération de vrille en lacet** qui annule le D 23 ms (`Air65-4` #2) ;
      le **TPA**, nécessaire mais pas suffisant seul. Seuil de cohérence (2) sans objet ; gaz d'avant le boost (3) sans effet
      sur les LPF dynamiques ; `petrel` (4) expliqué, gardé. Ces corrections ne sont que dans `filter_stages` : le simulateur
      affiché attend la décision de Cédric (W-064). Restent 2 alertes légères sur le D (−1,3 dB, 100–250 Hz), même famille que
      l'écart de W-062. Le texte d'origine de 22a bis suit.
      Examen des alertes (décidé par Cédric le 2026-11-22, avant 22c à 22f) : les 7 sessions en alerte ([validation.md](validation.md)) se rangent en trois familles ; mesurer chaque
      hypothèse sur le corpus, sans la supposer, puis relancer `otb-dev corpus stages` et revenir vers Cédric avec ce qui reste.
      1. **TPA** : `Cedric/BF Chirp/Air65-4.bbl` #2 et `1. PTB/cupidstunt/petrel-85mm-issues.bbl` #2 et #4 (résidu D de 0,36
         à 0,90, contre 4 % sur les vols chirp) ont un TPA fort sur le D (`tpa_rate` 65, `tpa_breakpoint` 1350, `tpa_mode` 1),
         alors que l'étage D ajuste un gain constant. Recalculer le gain instant par instant d'après les gaz et l'en-tête
         (lire `flight/pid.c`, `getTpaFactor`) : si le résidu retombe vers 4 %, la correction entre dans `filter_stages`.
      2. **Fiabilité** : les alertes de `KM HOVER 4K 3`, `KM HOVER WOBBLE 5`, `redhonu` et en partie `F1` tombent à une
         cohérence de 0,45 à 0,61 (stationnaire : au-dessus de 100 Hz, surtout du bruit moteur pile sur les notchs). Relancer
         avec `MIN_COHERENCE` à 0,7 et compter les alertes qui restent ; choisir le seuil avec Cédric.
      3. **W-062** : écart léger et de même signe partout (notch dynamique et D, −0,3 à −0,8 dB entre 50 et 250 Hz, firmware
         qui filtre plus). Essayer les gaz d'avant le throttle boost pour le LPF dynamique du D et mesurer l'effet ; pour le
         notch dynamique, cause à chercher.
      4. `petrel-85mm-issues` dévie aussi sur le gyro (+7 à +17 dB au-dessus de 100 Hz) : log d'un drone en difficulté, à
         écarter seulement si 1 à 3 ne l'expliquent pas, et en le disant.
- [x] **22b — Vols de calibration** (branche `fix/filter-stage-measurement`, 2026-11-22) : fait — les 12 vols
      `Cedric/Air65/Test BF Chirp/GYRO_SAMPLE_*.bbl` passés dans 22a ([validation.md](validation.md)). LPF2, RPM et LPF1 séparés :
      0 alerte. Deux défauts trouvés et corrigés dans le seul contrôle par étage : l'**arrondi de l'entrée** faussait la mesure
      (L-103 : l'écart « de même signe partout » de W-062, l'écart du notch dynamique entre 250 et 500 Hz et les 2 alertes
      acceptées en venaient ; corpus : 2 alertes → 0) ; le **LPF2** était calculé pour la cadence du log et non celle du gyro (L-104,
      +0,45 dB entre 250 et 500 Hz ; s'ajoute à W-064 pour le simulateur affiché). Après correction, tous les étages sous 0,5 dB
      jusqu'à 500 Hz. Vols `FFT_FREQ` à 2 kHz ajoutés le même jour (Cédric loguera ses drones moyens à 2 kHz) : même défaut pour
      RPM et LPF1, appliqués à chaque boucle PID par le firmware ; tous les filtres sont maintenant rejoués à leur cadence (L-104).
      Restent les écarts de W-067 (drones qui vibrent fort, logs à cadence réduite), à mesurer sur le premier drone moyen. Le second vol (SVF, PT2, notch statique, notch D) n'a pas été fait : à voler si 22e en a besoin.
      Consigne d'origine : `debug_mode = GYRO_SAMPLE`,
      `gyro_filter_debug_axis = ROLL`, `blackbox_sample_rate = 1/1`, `blackbox_high_resolution = ON` (il n'affine que le gyro
      et `gyroUnfilt`, **pas** les canaux `GYRO_SAMPLE`, arrondis au °/s : un chirp plus ample aide davantage), mode angle,
      10 s de stationnaire puis le chirp sur le roll. Un second vol avec d'autres types de filtres (BIQUAD, donc SVF en
      2026.12 ; PT2 ; un notch statique gyro ; un notch D), que le premier vol ne teste pas. Les passer dans 22a pour
      séparer LPF2, RPM et LPF1.
- [x] **22c — Suivi des pics du notch dynamique** (branche `feat/dyn-notch-tracking`, 2026-11-22) : fait —
      `rust/otb-analysis/src/dyn_notch.rs` (variantes par version, L-102), `otb-dev corpus dyntrack`. Sur 1 099 notchs du
      corpus ([validation.md](validation.md#suivi-du-notch-dynamique-2026-11-22-lot-22c)) : 0,3 à 0,4 Hz d'écart médian aux
      centres enregistrés à chaque boucle PID en 8 et 3,2 kHz ; 14,4 Hz depuis le gyro modélisé sur tout le corpus, contre
      78 Hz pour les centres initiaux. **Pas branché** sur le simulateur affiché ni sur W-059 : **décidé par Cédric le 2026-11-22**,
      le branchement se fait avec 22d et 22e (W-064), même branche ; valeurs affichées, conseils,
      références Python et D-041 changent, décision de Cédric (W-057). Variantes non vérifiées : W-065. Le texte d'origine suit.
      Réimplémenter d'après sa description le suivi du firmware
      (`flight/dyn_notch_filter.c`, `common/sdft.c`), le vérifier contre les centres enregistrés du corpus `FFT_FREQ`.
      Il donne aussi les centres des deux autres axes : aujourd'hui, `simulate_gyro` applique aux trois axes les centres
      de l'axe debug, alors que le firmware suit chaque axe à part. Reprendre W-059 avec lui.
- [x] **22d — Retard des filtres corrigé** (branche `feat/filter-sim-before-after`, 2026-11-22) : fait — `compute_delays`
      retire ⌊denom/2⌋ / fréquence gyro (−0,125 ms à 8 kHz, denom 2) ; seule comparaison `gyroUnfilt` → gyro par la phase
      (le bruit de l'assistant compare des amplitudes). Corpus : 16 sessions sur 675 changent de constat ; repère de retard
      gyro du whoop 1,5 → 1,4 ms (choix de Cédric, p90 1,48 → 1,36 ms), les autres inchangés ([validation.md](validation.md)). Texte d'origine :
      retirer le décalage d'horloge du retard gyro de `compute_delays`, et de toute autre comparaison `gyroUnfilt` → gyro ;
      mesurer l'effet sur le corpus avant de livrer (valeur affichée qui change : le dire dans la PR).
- [x] **22e — Avant / après filtrage dans la page Filtres** (même branche, 2026-11-22) : fait — choix de Cédric : rejeu +
      rapport (D-058). `rust/otb-analysis/src/filter_replay.rs` rejoue la chaîne vérifiée (cadence nominale, filtres à leur
      cadence, TPA, suivi du notch dynamique avec les réglages simulés) ; « simulé actuel » = rejeu, « simulé modifié » = log ×
      rejeu modifié / rejeu actuel. `otb-dev corpus replay` : 0,1–0,2 dB entre rejeu actuel et log sur les logs à chaque
      boucle PID ([validation.md](validation.md#simulateur-de-filtres-rejoué-2026-11-22-lot-22e)). À voler en 22f : un notch
      dynamique de plus, que le rejeu donne **plus** bruyant (+0,7 à +1,9 dB entre 100 et 250 Hz) et l'ancienne méthode
      moins (W-068). Texte d'origine : gyro et terme D, mesurés et simulés, pour les réglages actuels et modifiés.
- [x] **22f — Prédiction vérifiée** (W-060, branche `feat/filter-sim-flight-check`, 2026-11-22) : fait —
      `otb-dev corpus predict AVANT APRÈS`. Vol de Cédric avec deux notchs dynamiques au lieu d'un, comparé aux 16 vols du
      même jour : entre 100 et 250 Hz, le rejeu se trompe de 0,3 dB en médiane (2,0 dB dans l'autre sens), le rapport des
      |H|² de 2,0 dB (4,0). Boucle fermée non séparable de la variation d'un vol à l'autre (batterie, gaz)
      ([validation.md](validation.md#prédiction-vérifiée-en-vol-2026-11-22-lot-22f)). Suite décidée par Cédric : l'assistant passe
      au rejeu (D-059, W-068). Texte d'origine : vol de l'Air65 après un changement de filtres, comparé à la prédiction faite
      depuis le vol d'avant. C'est la seule mesure de la vraie limite du simulateur : la **boucle fermée** (moins filtrer
      envoie plus de bruit aux moteurs, qui en produisent davantage ; le simulateur garde le bruit d'avant).
- **Ensuite seulement** : conseiller aux pilotes `FFT_FREQ`, un log à chaque boucle PID et `blackbox_high_resolution`
      (guide intégré, compétence `guide-integre`), si 22a à 22f le justifient. **Reporté par Cédric le 2026-11-22** :
      `FFT_FREQ` est déjà conseillé ; le reste attend d'autres vols (W-073).
- **Simulateur externe de Cédric** : **mis de côté par Cédric le 2026-11-22**, outil trop différent d'OpenTuneBox.
- [ ] **Critère d'arrêt** : si 22a trouve, sur les logs à pleine cadence, un écart de plus de 1 dB sous 250 Hz pour une
      version du firmware ou un type de filtre, s'arrêter et revenir vers Cédric avant 22e.
      **Atteint le 2026-11-22** : 7 sessions sur 18 (liste dans [validation.md](validation.md)), dont deux termes D que le modèle
      ne suit pas du tout (résidu 0,4 à 0,9). Après 22a bis : 2 alertes, sur 2 sessions, toutes deux à −1,3 dB sur le D entre
      100 et 250 Hz. **Décidé par Cédric le 2026-11-22** : ces 2 alertes sont acceptées (écart léger connu, suivi par W-062 et
      repris avec les vols 22b) ; les corrections de 22a bis passent dans le simulateur affiché avec 22d et 22e (W-064) ; 22c à
      22f peuvent reprendre. **Après 22b** : 0 alerte (les 2 venaient de l'arrondi de l'entrée, L-103).

---

## Lot 15 : dette de l'interface (Opus) — **conditionnel**

**But** : reprendre les trois dettes connues de l'interface. **Ce lot ne se fait pas « pour faire bien »** :
le budget de 60 images par seconde est tenu avec une marge de 3 sur la plus longue session du corpus
(5,6 ms contre 16,7). Il attend un **déclencheur mesuré**.

**Déclencheurs** (l'un suffit) : le viewer accroche sur un log plus long que le corpus ; on veut afficher
beaucoup plus de courbes à la fois ; un pilote signale une lenteur. (La mémoire du cache des outils était le
quatrième : elle a été mesurée le 2026-11-10, et le cache est borné depuis — D-053.)

**Branche** : `perf/viewer-draw` (W-052) ou `refactor/spectral-pages` selon le déclencheur.

**À lire** : [travaux-reportes.md](travaux-reportes.md) W-052 (W-035 est clos : son point 9 a été mesuré puis borné, D-053),
`rust/otb-gui/src/viewer_page.rs:ui`, `rust/otb-gui/src/tools/freq_time.rs` (la texture en cache, le modèle
à suivre), [decisions.md](decisions.md) D-037 et D-048, [lecons.md](lecons.md) L-085, L-086.

- [ ] **W-052** — le cadre est désormais dominé par le **tracé** des ≈ 3 840 points par courbe (4,7 ms en
      vue entière contre 2,1 ms zoomé sur un huitième du même log, à nombre de points égal). Deux pistes, à
      mesurer avant de choisir : un seul segment vertical par colonne de pixels au lieu de deux points, ou la
      courbe décimée dessinée dans une texture en cache comme `freq_time`. Preuve exigée, comme au lot 10 :
      **tracé identique** (captures au bit près, les deux thèmes) et chiffres de `banc_cadre` avant / après.
- [x] **Cache des outils : mesuré** (2026-11-10, `examples/banc_cache.rs`, tout le corpus — 535 logs, 642 sessions ;
      [validation.md](validation.md), « Mémoire du cache des outils »). Une entrée pèse 7,5 Mo (p50) à **72,6 Mo** pour
      l'assistant et 7,4 à **71,3 Mo** pour la réponse indicielle ; le simulateur de filtres, seul suspect de départ
      parce que son nombre d'entrées n'est pas borné, ne pèse que 90 ko. **69 % du rapport de l'assistant** était les
      fenêtres individuelles de la réponse indicielle, que personne n'y lit : libérées, l'entrée tombe d'un facteur 24
      (médiane) à 165 (L-094, `rust/otb-advisor/tests/rapport_memoire.rs`, contre-preuve vue en échec).
- [x] **Cache des outils borné** (2026-11-10, D-053) : plafond **par outil**, en nombre d'entrées — 3 réponses
      indicielles, 20 simulations de filtres, 6 rapports, 2 vérifications du Guide — la moins récemment utilisée jetée la
      première. Le pire cas d'un log ouvert passe de **5,1 Go à ≈ 223 Mo**. Quatre tests dans
      `rust/otb-gui/tests/state.rs` (module `plafond`), contre-preuves vues en échec ; 28 captures identiques au bit
      près. **W-035 est clos.**
- [ ] **Factorisation des pages spectrales** : les pages `spectral`, `freq_throttle` et `freq_time` répètent
      la même mécanique, comme la découpe de période (×6), `breaks` (×5), la découpe bornée (×9), les listes
      déroulantes (×5) et `reset_view` (×5). À ne faire que si une page de plus est prévue, sinon c'est du
      remaniement sans effet.
- [ ] **Lints pédantiques ciblés** : activer quelques lints `clippy::pedantic` utiles, **jamais
      `suboptimal_flops`** (L-051 : il change le résultat des calculs).
- [ ] **Critère d'arrêt** : un tracé qui diffère d'un pixel entre avant et après, ou un gain inférieur
      à ×1,5.

---

## Lot 29 : fragments (Opus)

**But** : nommer et garder un morceau de vol, au lieu de redécouper à la main à chaque ouverture.

**Branche** : `feat/fragments`. **S'appuie sur `otb-store`** (lot 28, fait le 2026-11-18).

**Ce qui existe déjà** : `rust/otb-analysis/src/period.rs:auto_period` calcule la période de vol (décollage →
atterrissage) et la période fait partie de la clé du cache des outils (D-037) ; le **rôle** de chaque log (filtres,
série PID) est déjà retenu par la mémoire du Guide (D-039). Le fragment en est la généralisation : plusieurs par
session, nommés, datés, avec leurs bornes.

**Fait le 2026-11-18 — 29a, le cœur** (`rust/otb-store/src/link.rs`, `rust/otb-store/tests/fragments.rs`) :

- [x] **Index des fragments** : log d'origine, session, bornes en numéros d'échantillon, nom, rôle, date. Le rôle
      est la chaîne des constantes du Guide, pas un nouveau vocabulaire.
- [x] **Période d'un fragment dans un log ouvert** (`range_of`) et son **message clair** quand le fichier a changé
      depuis : c'est le critère d'arrêt du lot, tenu par un test et sa contre-preuve.
- [x] **Liste d'analyse** : ordre, **indice** de couleur attribué à l'entrée et jamais changé ensuite (un indice
      libéré est repris), actif ou non, fragment de travail qui doit appartenir à la liste.
- [x] **Preuve sur un vrai log** : un fragment de `tests/data/jeno5_5s.bbl` relu après redémarrage rend les mêmes
      bornes **et les mêmes échantillons** ; la contre-preuve d'un décalage d'un seul échantillon est dans le test.
      26 tests pour `otb-store`, 8 contre-preuves vues en échec au total.

**Fait le 2026-11-23 — 29b, l'écran** (branche `feat/fragment-extraction`, D-065, W-069 clos) :

- [x] **Écran d'extraction** (planche du prototype, hors rail) : c'est le visualiseur, qui quitte le rail. Il s'ouvre à
      l'ouverture d'un log, par le crayon d'un fragment au tableau de bord et par le Guide, et revient d'où l'on venait.
      Bornes tirées sur n'importe quel graphique (onglets « Début / Fin », poignées), saisie à l'échantillon près dans le
      panneau, Limites auto, Session entière, barre de navigation sur le log entier.
- [x] **Une seule notion** : les bornes du fragment sont la période d'analyse ; rien n'atteint les outils avant
      « Enregistrer le fragment » ; des bornes choisies se retrouvent à la réouverture. **Défaut corrigé** : rouvrir un
      fichier remettait ses fragments au vol entier.
- [x] Rail aux **sept entrées du prototype**, dans son ordre ; « Configuration » y a son entrée (icône `cpu`).
- [x] **Preuve** : 13 tests nouveaux ou récrits ; les 9 qui portent une règle nouvelle vus en échec, protection
      retirée (L-071) ; toute la suite de
      l'interface verte ; captures des deux thèmes regardées (un débordement du panneau attrapé ainsi) ; temps de cadre
      **2,2 ms** en vue entière et **3,0 ms** en glissement sur `FINAL.bbl` (avant : 3,1 et 3,7 ms), images de la
      documentation refaites.
- [x] **Écarts au prototype, assumés** (D-061) : marge de 1 % de « Vue entière » (une borne au tout début du log reste
      attrapable) ; section **Affichage** repliée (échelle, lissage, épaisseur : absents de la maquette, gardés du
      visualiseur) ; toutes les traces présentes dans le log, noms longs abrégés par « … » ; « Image précédente /
      suivante » avancent d'un échantillon.
**Fait le 2026-11-23 — 29c, la liste d'analyse à l'écran** (branche `feat/analysis-list`, D-066, D-049 remplacée) :

- [x] **Barre de fragments** en haut de tous les écrans, au dessin du prototype (`rust/otb-gui/src/analysis_bar.rs`) : une
      pastille par log ouvert (rond de couleur, nom, drone, « × »), fiche au survol ; clic = désactiver / réactiver, clic
      sur le rond = fragment maître, glisser = ordre des courbes, « × » = retirer.
- [x] **Les pages lisent la barre** : plus de liste « Fichiers » par page ; actifs dans l'ordre de la barre, maître pour
      les pages à un seul vol ; couleur fixe par fragment, même sur la Réponse indicielle (D-049 remplacée). Fréq. × gaz :
      une colonne garde son vol quand l'ordre change (défaut que le glisser aurait introduit, corrigé et testé).
- [x] **Le « + » d'un fragment** au tableau de bord l'ajoute en n'ouvrant que sa session, ou l'en retire ; il prend la
      couleur de sa pastille.
- [x] **Liste retrouvée au lancement** (ordre, couleurs, pastilles désactivées, maître) ; un fichier disparu est signalé.
- [x] **Preuve** : 10 tests nouveaux ou récrits ; 9 contre-preuves vues en échec, une par protection nouvelle (L-071) ;
      toute la suite de l'interface verte ; captures des deux thèmes regardées, dont une pastille désactivée
      (`barre-desactivee-*.png`).

**Fait le 2026-11-23 — 29d, le second fragment d'une session** (branche `feat/second-fragment`, D-067, D-061 amendée) :

- [x] **« Enregistrer comme nouveau fragment »** dans la barre du haut de l'écran d'extraction (écart au prototype,
      assumé) : l'origine ne bouge pas ; l'autre est rangé dans la même session (identifiant `<empreinte>-<session>-<k>`,
      nom rendu distinct), posé en pastille juste après elle sur les **mêmes échantillons**, et devient maître.
- [x] **Supprimer un fragment ajouté**, en deux temps, avec ses mesures et sa place dans la liste ; le fragment de la
      session n'a pas ce bouton.
- [x] « Ouvrir… » ne met en analyse que le fragment de chaque session ; un fragment ajouté revient par son « + », son
      crayon ou la liste relue au lancement, sans relire le fichier si sa session est déjà ouverte.
- [x] **Preuve** : 5 tests nouveaux (2 `otb-store`, 3 interface), 8 contre-preuves vues en échec (L-071) ; captures
      `fragment-ajoute-*.png` des deux thèmes regardées.
- Laissé de côté : le Guide lit encore le rôle par fichier (W-074).

---

## Lot 30 : refonte de la navigation et de la mise en page (Opus)

**But** : appliquer le **prototype complet de l'application** du canevas du 2026-11-16. Cédric l'a désigné le
2026-11-18 comme **la maquette qui fait foi** : les huit autres sont des essais, et en cas de désaccord c'est le
prototype qui gagne.

**Branche** : `feat/navigation-redesign`. **Dépend du lot 29.**

**Ce que montre le prototype** (relevé dans son code le 2026-11-18) : un **rail de navigation à gauche**, de sept
entrées — Tableau de bord, Guide, Assistant, Bruit et filtres, Réponse indicielle, Curseurs PID, Configuration —
et **deux écrans hors rail**, ouverts par un geste puis refermés : l'extraction d'un fragment et la fiche d'un
drone (Identité, Machine, Configurations Betaflight, Fiche cible). L'espace « Bruit et filtres » porte les onglets
Spectre, Fréq. × gaz, Fréq. × temps, un choix d'axe, et le panneau « Et si » qui remplace le Simulateur. Donc :
**huit outils à plat deviennent sept entrées de rail plus un tableau de bord**, le seul regroupement étant
Spectre + × gaz + × temps + Simulateur → « Bruit et filtres ».

**Décision à écrire dans ce lot**, tranchée le 2026-11-18 (Cédric a demandé de décider) : **D-015 est amendée, pas
remplacée.** Ce qui en reste tel quel : **aucune fenêtre séparée**, et **une vue cachée ne calcule pas** — cette
seconde partie s'étend aux vues internes d'un espace, où elle compte davantage qu'avant, puisqu'un espace
n'affiche qu'une vue sur quatre. Ce qui change : une entrée de navigation n'est plus forcément un outil, c'est
**soit un outil, soit un espace qui regroupe plusieurs vues d'un même sujet** ; les boutons deviennent un rail à
gauche ; et **deux écrans hors rail** sont permis. Ce qui ne change pas : le trait d'outil
(`rust/otb-gui/src/tool.rs`) et la liste d'enregistrement (`rust/otb-gui/src/tools/mod.rs`) restent le point
d'entrée unique — un espace s'y enregistre comme un outil, et chaque vue interne garde son calcul et son cache
(D-037, D-053). Le texte de D-015 cite encore des fichiers Python : l'amendement le remet en termes Rust. D-040
(écarts voulus avec l'ancienne interface Qt) se relit en même temps.

**Fait le 2026-11-19 — 30a, le rail** (`rust/otb-gui/src/app.rs:rail`, `rust/otb-gui/tests/app.rs`) :

- [x] **Amendement de D-015 écrit avant la première ligne de code** : c'est **D-060**, avec les sept entrées du
      rail et les deux écrans hors rail comme contrat. Il a trouvé que le regroupement **existait déjà** dans le
      code (les vues `HIDDEN` au `TAB` de leur espace) : la décision met le texte en accord avec le programme.
- [x] **Le rail** : les boutons de navigation quittent la barre du haut pour une colonne à gauche de 212 points,
      entrées sans cadre sauf celle qui est affichée. La barre du haut ne garde que ce qui agit sur les logs et
      sur la fenêtre, et la page exportée commence désormais au bord droit du rail.
- [x] **Preuve** : deux tests (`le_rail_porte_les_entrees_de_navigation`, `cliquer_une_entree_du_rail_change_decran`)
      avec contre-preuve vue en échec ; 27 tests de la fenêtre verts ; captures hors écran des deux thèmes
      regardées, et les quatre images de la documentation refaites (elles montraient une barre qui n'existe plus).
- [x] **Un défaut révélé, corrigé dans la foulée** : le `ui.horizontal` imbriqué de `phase_button` formait un bloc
      insécable, et la dernière phase du Guide se faisait **rogner** au lieu de passer à la ligne dès que la
      fenêtre se resserrait. Mis à plat, elle passe à la ligne (capture à l'appui).

**Reste** :

**Fait le 2026-11-19 — 30c, le tableau de bord** (`rust/otb-gui/src/dashboard.rs`) :

- [x] **Cartes de drones** : une par drone que la bibliothèque connaît, avec son firmware, ses logs connus et la
      date du plus récent ; panneau de droite avec ses logs, du plus récent en haut, et un **+** qui rouvre un log
      sans passer par la boîte de fichiers.
- [x] **La bibliothèque entre dans l'interface** : l'application la tient (`App::library`), range chaque fichier
      ouvert (empreinte du contenu : rouvrir le même fichier ne fait pas deux entrées) et l'écrit quand eframe
      enregistre. ⚠️ **En mémoire partout sauf au lancement réel**, pour qu'aucun banc n'écrive celle de
      l'utilisateur — même règle que les fiches drone, qui y vivent depuis D-068.
- [x] **Preuve** : deux tests (cartes et panneau, rangement à l'ouverture) avec **leurs deux contre-preuves vues en
      échec** ; 281 tests de l'interface verts ; capture du tableau de bord ajoutée à l'exemple `capture`, regardée
      dans les deux thèmes.
- [x] **Un défaut attrapé par la capture, pas par les tests** : le caractère « ✓ » n'existe dans aucune police
      embarquée et fait **paniquer** l'application. Le piège était déjà écrit (L-068) : remplacé par une icône.
**Repris le 2026-11-19 — 30c bis, au dessin de la maquette (D-061)** : Cédric a rappelé que « le design doit
correspondre à 100 % à la maquette ». Les trois lots précédents en avaient la structure, pas le dessin.

- [x] **Carte de drone au détail près** : badge teinté 44 × 44 à l'icône du quadricoptère, nom 16 et sous-titre 12,5,
      pastille d'état, « Fragments » et « Dernier vol » en deux colonnes, étape du parcours, barre d'avancement de
      6 points, trait bleu de 2 points et ombre portée sur la carte choisie. Mesures en constantes nommées.
- [x] **Entrée de rail** au fond bleu pâle et au texte bleu quand elle est choisie, à plat sinon ; panneau blanc,
      trait de séparation, marges de la maquette. ⚠️ `selected` est **gardé** : il met l'entrée à l'état « coché »
      pour un lecteur d'écran, ce que les couleurs seules ne font pas.
- [x] **Trois écarts assumés et écrits** (D-061) : thème sombre dérivé, un fragment par session ouverte (d'autres depuis D-067), avancement
      compté sur les cases cochées à la main.
- [x] **Deux couleurs ajoutées au thème** (`select`, `border`) : tout le reste de la palette de la maquette s'y
      trouvait déjà. Trois icônes ajoutées, dont celle du drone, dessinée d'après la maquette.
- [x] **Le « + » d'un fragment ajoute à la liste d'analyse** (et non plus seulement ouvrir son vol) : fait au lot 29c
      (D-066).
- [ ] **L'avancement du parcours** sur la carte (« Étape 2 / 4 · Filtres ») demande les vérifications du Guide,
      qui coûtent une analyse par log : à faire avec le lot 30d, pas en ouvrant le tableau de bord.
**Fait le 2026-11-19 — 30b, l'espace** :

- [x] **Espace « Bruit et filtres »** : une seule entrée de rail pour le Spectre, Fréq. × gaz, Fréq. × temps et le
      « Et si » des filtres, avec ses **onglets en haut de page** (`space_tabs`). Le trait de page gagne la notion
      d'espace (`SPACE`), et passer d'une vue à l'autre n'exige pas d'avoir un log ouvert, contrairement aux
      boutons du Guide. Les deux raccourcis du panneau du Spectre disparaissent : deux chemins pour le même geste
      faisaient deux boutons du même nom sur la page.
- [x] **Preuve** : `les_onglets_de_lespace_mènent_aux_quatre_vues` avec contre-preuve vue en échec, **280 tests de
      l'interface verts**, captures des deux thèmes regardées, images de la documentation refaites.
- [x] La crainte du plan du fork — « faire le lot 23 (chaîne de filtres décrite en données) d'abord, sinon le même code est déplacé deux fois » — **ne s'est pas
      réalisée** : le regroupement est une affaire de navigation (`tool.rs`, `app.rs`), et n'a pas touché au calcul
      des filtres. Ce lot du chantier firmware reste indépendant.

- [x] **Fiche d'un drone** — **fait le 2026-11-23, 30d** (branche `feat/drone-sheet`, D-068) : second écran hors
      rail du prototype (Identité, Machine, Configurations Betaflight, suppression), ouvert par le crayon d'une carte,
      « Nouveau drone » ou le Guide. **Une seule fiche par drone, dans la bibliothèque** (choix de Cédric) : le Guide en
      lit une copie, sa fenêtre de fiche disparaît, l'ancien `drones.json` est repris au lancement puis renommé
      `.repris`. La « Fiche cible » attend le lot 31 (choix de Cédric) ; l'identifiant et la carte, le lot 32b.
- [x] **Gyro / Terme D dans l'espace** — **fait le 2026-11-23, 30e** (branche `feat/signal-choice`, D-069) : un seul
      choix en tête du panneau de droite des quatre vues, et des axes partagés (choix de Cédric) ; les cases détaillées
      restent dessous (choix de Cédric), un autre signal choisi là n'allume aucun bouton ; le « Et si » perd ses
      onglets « Gyro » et « D », doublons du choix.
- [x] **Assistant en cartes** — **fait le 2026-11-23, 30f** (branche `feat/advisor-cards`, D-070) : une carte par constat
      triée par gravité, les « info » et les vérifications OK repliées, pastilles par gravité ; un constat qui compare
      une mesure à un seul seuil porte une **jauge** (`Gauge`, hors des références Python, `otb-parity` 0 échec). Cartes
      seules (choix de Cédric) : comparaison, votes, historique et lignes CLI → W-076 ; « Appliquer » refusé (D-063).
- [x] **Preuve** : captures avant / après sur les deux thèmes (`docs/images/assistant.png` refaite), 0,4 ms par cadre.
- [x] **Critère d'arrêt** : aucune page n'a perdu son test (les tests de l'ancienne mise en page sont récrits sur les
      cartes), budget tenu.
- [ ] **Avancement du parcours** (30g) : la carte du tableau de bord et le Guide sur la liste d'analyse (W-074).

---

## Lot 31 : mesures suivies dans le temps (Opus)

**But** : répondre à « est-ce que ce drone s'améliore ? », ce qu'aucun écran ne sait faire aujourd'hui.

**Branche** : `feat/metrics-history`. **Dépend du lot 29** ; la table des mesures existe déjà (`otb-store`).

- [ ] **Une ligne par fragment, axe et mesure** : latence, pic, temps de montée, marges, bruit du gyro et du
      terme D, retard des filtres — avec les réglages de calcul qui l'ont produite et la version de calcul du
      lot 28. Une mesure se jette quand les bornes du fragment ou cette version changent.
- [ ] ⚠️ **La table des mesures ne se charge pas au lancement, mais à la première question posée.** Mesuré au lot 28
      ([validation.md](validation.md)) : au volume plein (535 logs, 77 040 mesures) elle pèse **8 Mo** et met **959 ms**
      à se lire — à elle seule le déclencheur du plan. Les cinq autres tables réunies pèsent 1 Mo.
- [x] **Fichiers JSON ou vraie base (SQLite)** : **tranché le 2026-11-22, JSON gardé** (D-064) ; la table des mesures
      se lit à la première question, hors du fil de l'interface.
- [ ] **Fiche cible par drone** (réactivité des PID, filtrage) et un graphique par mesure sur tous ses fragments,
      la cible en bande.
- [ ] ⚠️ **Dire lequel prime** : la cible du drone n'est pas un seuil de l'assistant (D-011, calibré sur le
      corpus). Deux chiffres qui se contredisent à l'écran, c'est un utilisateur perdu.
- [ ] **Critère d'arrêt** : une mesure stockée qui ne se recalcule pas à l'identique à la demande.

---

## Lot 32 : récupération des logs par USB (Opus) — **le plus risqué**

**But** : brancher le drone, récupérer, vérifier, puis seulement effacer. Un log perdu est un vol à refaire ; un
effacement reporté ne coûte rien.

**Branche** : `feat/usb-retrieve`. **Dépend du lot 30** ; la bibliothèque et le refus des doublons existent déjà.

**À lire** : [spec-usb.md](spec-usb.md) (ce que fait le firmware, vérifié le 2026-11-22), [decisions.md](decisions.md) D-063.

**Découpage** (Cédric, 2026-11-22, D-063) : **32a — le protocole**, sans interface et avant la fin du lot 30 : trames MSP
v1 (jumbo) et v2, identité, lecture par blocs, effacement, `diff all`, port simulé pour les tests, et la suite des
contrôles avant effacement ; port série par `serial2`. **32b — l'écran**, après le lot 30. **Aucune écriture de
réglage sur la carte** : les « Et si » restent des lignes CLI.

**32a fait le 2026-11-22** (branche `feat/usb-protocol`, crate `otb-usb`, détail dans [architecture.md](architecture.md)) :
trames, identité, copie MSP par blocs avec progression et arrêt, contrôles avant effacement et seul chemin qui efface,
`diff all`, port simulé, verrou du port sous Linux, erreurs d'ouverture expliquées. **Reste pour 32b** : l'écran, la voie
du stockage de masse (redémarrage en MSC, fichiers `BTFL_*.BBL`, carte SD effacée par le système avec les mêmes
contrôles), le rangement dans la bibliothèque (drone reconnu par l'identifiant de la carte, `diff all` gardé seulement
s'il change : `rust/otb-usb/src/cli.rs:config_changed`), et les essais restants sur une vraie carte de [spec-usb.md](spec-usb.md)
(procédure d'essai à la main, par `otb-dev usb`).

- [x] **Identité de la machine** par MSP (32a ; le rangement par identifiant : 32b) : nom, identifiant de la carte, version du firmware, état armé,
      occupation de la mémoire. L'identifiant de la carte est la clé ; le nom seul ne suffit pas (deux drones
      peuvent le porter, et il se change en une commande).
- [ ] **Deux voies de copie** : mode stockage de masse quand la carte le permet (rapide, seule voie pour une carte
      SD), repli sur la lecture bloc par bloc en MSP (lent : plusieurs minutes pour 16 Mo). Barre de progression,
      et une copie interrompue reprend au début.
- [x] ⚠️ **Jamais d'effacement sans vérification** (32a : `rust/otb-usb/src/erase.rs`, l'écran de confirmation : 32b), dans cet ordre : taille copiée = place annoncée, le décodeur
      ouvre chaque log et en extrait au moins un fragment, les fichiers sont écrits sur le disque, **puis**
      l'utilisateur confirme. Un échec de lecture interdit l'effacement, quelle que soit l'option choisie.
- [ ] ⚠️ **Sécurité** : drone armé → aucune opération ; conseil de retirer les hélices ; un seul accès au port à la
      fois (le configurateur de Betaflight peut l'occuper) ; droits sur le port série sous Linux et pilote du port
      virtuel sous Windows, détectés et expliqués au lieu d'échouer.
- [ ] **Sauvegarde de la configuration** au même branchement (`diff all`), gardée seulement si elle diffère de la
      précédente : l'écran Configuration peut alors comparer deux versions des réglages, et plus seulement les
      en-têtes de deux logs.
- [x] **Les huit points à vérifier** de l'étude, dans le code et la documentation de Betaflight, **avant** d'écrire
      (elle a été rédigée de mémoire) : faits le 2026-11-22, branche « docs/usb-firmware-check », consignés dans
      [spec-usb.md](spec-usb.md). Six tranchés par le code ; le **débit MSP** et la **durée d'un effacement** se mesurent
      sur une vraie carte (procédure dans le même document) ; **appliquer les réglages « Et si » sur la carte** reste
      à Cédric. Deux constats changent le dessin : le firmware **n'empêche ni la lecture ni l'effacement quand le drone
      est armé** (l'état armé se relit juste avant d'effacer), et la mémoire interne est **en lecture seule en mode
      stockage de masse** (effacer demande de revenir en MSP). Bibliothèque série : `serial2` (BSD-2 ou Apache-2.0),
      `serialport` étant refusée par sa licence MPL-2.0.
- [x] ⚠️ **Nouvelle dépendance et nouvelle surface** (32a : `cargo deny` vert, port simulé, procédure dans [spec-usb.md](spec-usb.md)) : licence vérifiée par `cargo deny` (`rust/deny.toml`), et
      **aucun matériel en CI** — donc un port simulé pour les tests automatiques et une procédure d'essai à la main
      écrite dans la documentation, comme pour les installateurs.
- [ ] **Critère d'arrêt** : un seul chemin de code où l'effacement pourrait partir sans vérification complète →
      s'arrêter net.

---

## Lots 23 à 25 : scripts firmware — **conditionnels**

Repris du fork `GerryNatif/OpenTuneBox` (sa demande de fusion n° 2, écrite le 2026-11-18 avant le lot 22), relus le
2026-11-22 (D-062). **Déclencheur** : de vrais logs INAV ou ArduCopter, et la demande de Cédric. Pour un pilote Betaflight
seul, ces lots ne corrigent rien : le lot 22 a déjà vérifié le simulateur sur de vrais vols, et le calcul au bit près ne
réduirait pas l'écart qui reste (il vient de ce que le log n'enregistre pas). L'ordre du terme D, seul écart visible,
est sorti dans le lot 33.

### Lot 23 : la chaîne Betaflight décrite en données (Opus)

**But** : **une seule description** de la chaîne de filtres, au lieu de deux, **sans changer un seul résultat**. C'est
ce qui rend un script de firmware possible : D-062 veut une chaîne décrite une fois.

**Branche** : `refactor/chain-as-data`.

**À lire** : `rust/otb-analysis/src/bf_filters.rs:gyro_chain`, `rust/otb-analysis/src/bf_filters.rs:GyroStage`,
`rust/otb-analysis/src/filter_replay.rs`, [spec-betaflight.md](spec-betaflight.md) §3 et §8, [lecons.md](lecons.md) L-048,
L-071, L-104.

**Constats du dépôt, relus le 2026-11-22** (le fork les avait écrits avant le rejeu de D-058) :

- **La chaîne est écrite deux fois** : en **réponse en fréquence** (`gyro_chain`, `dterm_chain` : repli de la page
  Filtres et de l'assistant pour un log sans gyro non filtré, retard gagné par un conseil, boucle ouverte), et **dans le
  temps** (étages `GyroStage`, communs à `simulate_gyro`, au contrôle par étage et au rejeu). Le temps a donc déjà une
  seule description ; la seconde est la réponse en fréquence.
- La cadence propre à chaque filtre est déjà dans le rejeu (L-104) ; seul `simulate_gyro` garde celle du log (W-072,
  point 2), pour les tests et `otb-parity`.

- [ ] **Description de la chaîne en données** : étapes ordonnées (brique, cadence, paramètres, en service ou non),
      **prises** (où le log a enregistré chaque signal — c'est ce qui décide du badge « simulé » de
      `rust/otb-gui/src/tools/filter_sim.rs`) et **position de la dérivation**. La réponse en fréquence et le rejeu la
      lisent au lieu de porter l'ordre en dur.
- [ ] **Preuve d'iso-résultat** : `cargo run --release -p otb-parity -- check` 0 écart, `verify --strict` 0 saut,
      `otb-dev corpus replay` et `corpus stages` inchangés, captures de la page Filtres **identiques au bit près** sur les
      deux thèmes.
- [ ] **Contre-preuve** (L-071) : une étape déplacée ou une cadence changée dans la description fait **rougir** un
      test, vu en échec.
- [ ] **Critère d'arrêt** : un seul résultat qui change sans explication chiffrée → s'arrêter et revenir vers Cédric.

### Lot 24 : briques à l'arithmétique du firmware (Opus)

**But** : atteindre le bit à bit contre le firmware compilé sur PC, **et basculer l'application sur cette nouvelle
chaîne dans le même lot** (D-062). C'est le plus gros lot du plan : il déplace un chiffre dans presque tous les écrans,
y compris le rejeu de la page Filtres (D-058) et le bruit prévu par l'assistant (D-059).

**Branche** : `feat/firmware-exact-bricks`. **Dépend du lot 23.**

**À lire** : `rust/otb-dsp/src/lib.rs` (les noyaux actuels, D-033), [decisions.md](decisions.md) D-029 (float32),
D-047 et D-011, [travaux-reportes.md](travaux-reportes.md) W-028, [lecons.md](lecons.md) L-048 et L-051
(⚠️ jamais `suboptimal_flops` : il change le résultat des calculs), [spec-betaflight.md](spec-betaflight.md) §2 (biquads
RBJ jusqu'à la 2025.12, filtres SVF ensuite : même réponse, autre arithmétique).

⚠️ **Pourquoi l'ordre des étapes n'est pas négociable.** La chaîne actuelle est la référence de tout : les
références figées, les seuils de l'assistant, les captures au bit près et le contrôle par étage du lot 22. La bascule
d'un coup a été choisie en connaissance de cause ; elle ne tient que si chaque étape ci-dessous est **livrée et vérifiée
avant la suivante**, et qu'aucune ne saute.

- [ ] **24a — les briques, sans rien brancher.** Float32 à côté des actuelles, rangées par famille de firmware même si
      **seule celle de Betaflight est écrite** : tout en float32 sauf là où le firmware calcule lui-même en double ;
      approximations du firmware recopiées ; **ordre exact des opérations** (`état + k × (entrée − état)` n'a pas les
      mêmes bits que `(1 − k) × état + k × entrée`) ; initialisations et moment du recalcul des coefficients recopiés ;
      **deux domaines de cadence** réellement exécutés (cadence gyro, décimation, cadence PID). Rien de visible.
- [ ] **24b — le banc et les vecteurs de référence.** Le banc écrit par le projet télécharge et compile le firmware,
      ne garde que des vecteurs (D-062) ; licences d'INAV et d'ArduPilot vérifiées à leur source à ce moment-là.
      Balayage de fréquence, bruit, échelons, fréquences de notch variables. La CI les rejoue et exige l'**égalité bit à
      bit** ; elle n'a pas le corpus, donc tout se prouve sur vecteurs et sur `tests/data/jeno5_5s.bbl` (L-023).
- [ ] **24c — l'écart chiffré sur tout le corpus, avant de brancher quoi que ce soit.** Différence entre l'ancienne
      et la nouvelle chaîne sur tout le corpus, et `otb-dev corpus stages` / `corpus replay` relancés : le contrôle par
      étage **ne doit pas se dégrader** (preuve par les vrais vols, D-062). Chiffres dans [validation.md](validation.md).
      **C'est le point de non-retour** : un écart plus grand que prévu s'explique **ici**, pas après la bascule.
- [ ] **24d — la bascule.** L'interface passe à la nouvelle chaîne, les références figées sont réenregistrées
      (W-028 et W-072 point 2 en profitent), et les captures sont refaites sur les deux thèmes.
- [ ] **24e — les seuils de l'assistant revérifiés** (`otb-dev corpus calibrate`, compétence `calculs-corpus`).
      ⚠️ Si un seuil change l'**avis** de l'assistant sur un vrai log, **s'arrêter** et revenir vers Cédric avec le
      log et le chiffre : c'est exactement le risque accepté en choisissant la bascule d'un coup.
- [ ] **Écart au vol réel mesuré et affiché**, jamais promis : écart entre gyro simulé et gyro enregistré, montré
      dans l'interface avec sa cause quand le log ne contient pas l'entrée des filtres à pleine cadence.
- [ ] **Critère d'arrêt** : un vecteur qui ne tombe pas au bit près sans cause identifiée (approximation, ordre des
      opérations, initialisation), un étage du lot 22 qui se dégrade, ou un avis de l'assistant qui change sans
      explication → s'arrêter, consigner, revenir vers Cédric.

### Lot 25 : plages de versions et veille du firmware (Sonnet)

**But** : qu'un script dise de quelles versions il répond, et qu'un changement d'arithmétique en amont soit vu
**avant** qu'un pilote ne voie une simulation fausse.

**Branche** : `feat/version-ranges`. **Dépend du lot 24.**

- [ ] **Plage déclarée** par script : première version couverte, dernière version **vérifiée**, « ouverte » tant
      qu'aucun changement n'est connu. Au-delà de la dernière vérifiée, la simulation reste possible mais
      **marquée « non vérifiée »** dans l'interface.
- [ ] **Ce qui coupe une plage** ([spec-betaflight.md](spec-betaflight.md) §8) : arithmétique d'une brique, ordre de la
      chaîne, cadence, façon de calculer le terme D, noms ou plages des réglages. **Ce qui ne la coupe pas** : un nouveau
      réglage par défaut, un champ de log sans effet sur le calcul.
- [ ] **Veille** : un travail de CI compare, d'une version du firmware à la suivante, les fichiers qui portent le
      calcul, puis rejoue les vecteurs. Il **télécharge et compare, ne garde rien** (D-045), et prend `--locked`
      comme le reste de la CI.
- [ ] **Critère d'arrêt** : une veille qui crie à chaque version sans rien apprendre → la retirer plutôt que la
      laisser grincer.

## Hors plan, volontairement

Ces travaux reportés sont **vus et laissés** : aucun n'empêche un pilote d'utiliser OpenTuneBox aujourd'hui.
Leur détail et leur condition de reprise sont dans [travaux-reportes.md](travaux-reportes.md) ; ne pas les
rouvrir ici sans cette condition. Deux d'entre eux (W-043 et W-044) viennent du lot 13 supprimé : ils n'ont
jamais dépendu de la 0.8.0, et gardent donc leur condition de reprise. **W-041 a quitté ce tableau le
2026-11-10** : sa condition de reprise — un échec au hasard de la CI sur ce test — a été atteinte, et il est
clos (L-095). C'est à cela que sert une condition de reprise écrite d'avance. **W-043 l'a quitté le 2026-11-12** : il
a été traité par le lot 21.

| ID | Pourquoi il n'est pas dans un lot |
|---|---|
| W-001 | Logs KISS (`.TXT`) et INAV non décodés : hors périmètre, à rouvrir si Cédric change de radio ou de firmware |
| W-071 | ArduCopter (logs DataFlash) : périmètre **Betaflight seul** (D-062), aucun log ArduCopter réel dans le corpus |
| W-002 | Profils whoop, 2–3,5" et 8"+ appuyés sur trop peu de drones : il faut **plus de logs**, pas plus de calcul |
| W-004 | La boucle ouverte ne se mesure que jusqu'à 20–40 Hz : limite physique de l'énergie du manche |
| W-022 | Tolérance par fonction et non par champ dans `otb-parity` : reprise si un écart est soupçonné sur `freqs` / `bins` / `resolution_hz` |
| W-028 | Écarts des références figées : se règle au **prochain réenregistrement**, pas avant |
| W-029 (4) | Branche « crash » du guide : aucun crash réel dans le corpus, seuls des cas fabriqués |
| W-042 | Avis RustSec hérités d'eframe : ⏸ attend une version d'eframe qui change ses dépendances |
| W-044 (2) | `construire_windows.ps1` jamais essayé sur un vrai Windows : la CI fabrique l'installateur livré, ce script ne sert qu'à construire chez soi — à l'occasion. Son point 1 (`_internal`) est **clos** |
| W-051 | Pistes de performance écartées **par la mesure** le 2026-11-04 : ne pas les rouvrir sans mesure nouvelle |
| W-053 | 45 messages d'erreur imitant les exceptions de Python : atteignabilité non établie |
| W-054 | Huit des dix comparateurs d'`otb-parity` sans test : fabriquer une référence Python pour ces couches est le vrai coût |

**Écarté par arbitrage de Cédric** (« rien sans bénéfice démontrable ») : découper `rules.rs` (3 631 lignes),
`log_check.rs` (1 356) et `guide_page.rs` (1 053) — testés et validés au bit près, les remanier est un risque
sans effet visible ; les ~300 `pub` inutilement publiques (exposition trop large, pas du code mort) ; un cache
Cargo en CI (une mécanique à entretenir pour une CI qui dure 7 min).

---

## Historique des lots

Une ligne par lot fait — ou par lot abandonné, pour que son numéro ne serve jamais à autre chose. Le détail est dans les demandes de fusion et les pages Releases (jusqu'au lot 20 : l'ancien journal,
`git show 2cd2416:CHANGELOG.md`), les mesures dans [validation.md](validation.md),
les pièges dans [lecons.md](lecons.md) et les choix dans [decisions.md](decisions.md) — ce tableau ne fait
que dire quand et par quoi chaque lot est prouvé.

Les lots 0 à 6 viennent de l'**audit du dépôt du 2026-11-04**, qui avait relevé cinq chantiers : décodeur
robuste face à un en-tête hostile (C1), résultat périmé affiché par le socle des outils (C2), commande unique
de vérification (C3), mémoire du décodeur (C4), consignes et CI périmées (C5). **Les cinq sont traités et
prouvés.**

| Lot | Fait le | Sujet et preuve | Ce qu'il en reste |
|---|---|---|---|
| 0 | 2026-11-04 | Sonde du corpus et des références (`chore/corpus-probe`) : `otb-parity` 0 écart, `corpus validate` 697/712, départ mémoire 491 Mo. La ligne de base n'était **pas** « 0 écart » comme le plan le supposait | rien |
| 1 | 2026-11-04 | **C3** — `otb-dev verify [--quick] [--strict]` (`feat/dev-verify`, L-087), marqueur `[OTB-SAUT]`, crochet sans `jq`. 4 contre-preuves ; 74 exécutables de test avec et sans la liste des `-p` (elle était inutile) | rien |
| 2 | 2026-11-04 | **C5** — consignes et CI remises à jour (PR #103) : `AGENTS.md` de 12,6 à 8,4 Ko | rien |
| 3 | 2026-11-04 | **C2** — résultat périmé du socle des outils (PR #104, L-088). Contre-preuve : l'ancien `update` rougit les deux tests | W-036 (8) : garde-fou propre au Simulateur gardé |
| 4 | 2026-11-04 | **C1** — décodeur robuste (`fix/decoder-hostile-header`, D-044, L-080) : bornes 512 champs / intervalles 1 à 65 535, `try_reserve`. 10 cas dont **6 rouges avant** ; 20 000 champs : 3,7 s puis abandon → refus instantané | limite écrite : sous Linux une réservation énorme peut réussir (surengagement) |
| 5 | 2026-11-04 | **C4** — mémoire du décodeur (`perf/decoder-memory`, L-081) : `CompactTable` en `i32` dès le décodeur, **481 → 274 Mo (÷ 1,75)** et 0,63 → 0,54 s ; `FlightLog` identiques sur 528 fichiers | un décodage par blocs vers `f32` irait plus loin (non fait, non chiffré) ; W-014 et W-016 clos |
| 6 | 2026-11-06 | Bilan des cinq chantiers (branche « docs/plan-suite ») : L-087 et L-088 écrites, W-014 et W-016 clos. Aucun code livré | rien |
| 7 | 2026-11-08 | **Première publication Rust, 0.9.0.** 7a `otb-dev licenses --check` (`feat/licenses-check`) ; 7b CHANGELOG et captures (`chore/release-0.9.0`) ; 7c **quatre tours** d'essai sur de vraies machines et **sept défauts** corrigés (PR #118 à #126), **aucun vu par les tests** ; 7d–7e publication, page « dernière version », brouillon d'essai supprimé | W-043 (licences non relues, échéance repoussée à la 1.0) et W-044 (mise à jour depuis la 0.8.0 jamais essayée) → passés au lot 13, puis « Hors plan » quand il a été supprimé (ligne 13) |
| 8 | 2026-11-05 | Banc de temps de cadre (`chore/viewer-frame-bench`) : `examples/banc_cadre.rs`. Étalon retrouvé sur `FINAL.bbl` (7,4 à 8,3 ms) ; `LOG00008.BFL` 12,6 à 13,0 ms à 620 079 échantillons, sous les 16,7 ms | les 0,34 ms de carte graphique de W-049 ne se reproduisent pas (L-084) |
| 9 | — | **Non retenu** : W-048 ne devait pas se régler en affaiblissant un test. Repris hors lot le 2026-11-05 (`test/golden-tolerance`, D-047, L-083) | W-048 clos |
| 10 | 2026-11-05 | Tracé du viewer sans parcourir toute la série (`perf/viewer-pyramid`, D-048, L-085, L-086) : index multi-résolution min/max dans `otb-dsp`, cadre **÷ 2,2 à ÷ 6,5**, tracé **identique au bit près**, pic de mémoire inchangé | W-049 clos ; **W-052 ouvert** (le tracé domine maintenant) → lot 15 |
| 11 | 2026-11-10 | **Ouverture de plusieurs logs en parallèle** (branche `perf/open-parallel`, D-051, L-093) : `otb_log::parallel`, paquets d'au plus 3 fichiers décodés en même temps dans un pool `rayon` propre à l'appel. 6 logs : **1,835 s → 0,811 s (× 2,26)**, un seul fichier non dégradé, `FlightLog` identiques (empreintes) ; 4 contre-preuves | **W-050 clos** ; le chiffre de mémoire de D-051 était trop bas (663 Mo et non ≈ 540), plafond gardé par Cédric |
| 12 | 2026-11-08 | Ménage des registres (PR #129, `chore/registers-cleanup`) : 10 travaux reportés clos ou requalifiés, une **panique** corrigée (`log_rate`, W-029 point 3), un paramètre mort retiré (W-024), D-051 écrite. `verify --strict` 0 saut (125 s) | W-053 ouvert au passage |
| 13 | — | **Supprimé le 2026-11-10** : « nettoyage d'après publication ». Cédric a indiqué que **personne n'a jamais utilisé la 0.8.0** — l'essai de mise à jour 0.8.0 → 0.9.0 et son filet dans l'installateur devenaient sans objet. Le filet (`[InstallDelete]` de `opentunebox.iss`) est retiré dans la foulée (branche `chore/drop-python-upgrade-net`), W-044 point 1 clos | W-043 (licences, échéance 1.0) et W-044 point 2 (script Windows) ne dépendaient pas de la 0.8.0 → « Hors plan », avec leur condition |
| 14 | 2026-11-10 | **Derniers seuils sans origine** (branche docs/source-last-thresholds) : **aucune valeur changée**, parce qu'aucun des deux seuils restants ne peut être calibré — le corpus n'emprunte **jamais** la détection de taille par le nom (0 drone sur 96 sans fréquence moteur) et le profil 8"+ n'a que 6 drones (W-002). Chacun porte désormais sa **condition de levée**. W-005 clos : formule de la bande lue à la page sur une reproduction ([S37]), avec ses trois limites écrites. `verify --strict` 0 saut (126 s) | W-055 ouvert (cinq sources nomment une personne : arbitrage D-050) ; W-003 reste ouvert, mais borné à deux lignes assumées |
| 16 | 2026-11-08 | **Revue intégrale** du code, des tests, de la documentation et de l'outillage (PR #129) : 4 relevés (~41 000 lignes de code, ~20 500 de tests, 30 `.md`, 4 workflows), 53 cases, **2 défauts réels** corrigés (`compare_header`, `slice_time`), **6 constats de relevé faux** corrigés après vérification, chaîne Rust figée (`rust-toolchain.toml`), clippy sous Windows, D-052 écrite, D-050 précisée, récidive de L-079 bridée | **W-054 ouvert** : seuls 2 des 10 comparateurs d'`otb-parity` ont reçu un test |
| 17 | 2026-11-08 | **Rassemblement des plans** (branche « docs/rassembler-les-plans ») : les quatre documents de plan deviennent ce seul `docs/plan.md`, L-092 écrite | rien |
| 18 | 2026-11-11 | **Audit de sécurité** (branche `chore/security-hardening`, D-054) : réglages GitHub relus et complétés par Cédric (versions immuables, CodeQL, actions autorisées, approbation des contributions extérieures) ; `claude.yml` refuse les forks ; `--locked` en CI ; `unsafe_code = "deny"` ; crochet `guard-git.sh` ; essai de mutations (100 000 cas pour le décodeur, 20 000 de bout en bout, 0 plantage). Contre-épreuves notées dans D-054 | signature de l'installateur Windows : ouverte dans D-054 (l'épinglage SHA est exigé depuis le 2026-11-11) |
| 19 | 2026-11-11 | **Relecture des workflows en CI** (branche `chore/lint-workflows`, D-055) : job `actionlint` 1.7.12 dans `tests.yml`, vérifié par empreinte ; rien à installer sur le poste. Avant l'envoi : 19 scripts bash passés à `shellcheck`, rien à signaler | rien : `actionlint` est un contrôle exigé de `protection-main` depuis le 2026-11-11 |
| 20 | 2026-11-11 | **Notes de version à la manière de Betaflight** (PR #147, D-056) : titre de PR en phrase et une étiquette `RN: …` (8 étiquettes créées, dont `dependencies`, absente jusque-là : L-096) ; `.github/release.yml` ; contrôle `etiquette-pr.yml` (rouge sans étiquette, vert avec, vu sur #147) ; `otb-dev release notes` traduit la liste de GitHub ; `CHANGELOG.md` supprimé. 86 PR étiquetées et retitrées, Dependabot excepté (#60 à #147). Page de la 0.9.0 refaite (50 Ko → 6 Ko) depuis une liste fabriquée en local, GitHub ne lisant `release.yml` que sur l'étiquette (L-097) | `etiquette` exigé par `protection-main` depuis le 2026-11-12 ; l'appel de `generate-notes` par le jeton de la CI sera éprouvé à la première étiquette `essai-v…` ou `v…` |
| 21 | 2026-11-12 | **Licences livrées et sources sans nom** (branche « docs/licenses-and-sources-review », D-050 précisée) : textes des 22 composants sans fichier de licence copiés de leur dépôt d'origine (`rust/otb-dev/upstream-licenses/`, table `UPSTREAM`), dont la licence BSD du code d'AccessKit issu de Chromium, **absente du fichier livré avec la 0.9.0** ; polices relues (Noto Emoji, seule police OFL, non compilée) ; partie 3 du fichier vide et refusée par un test, contre-preuve vue en échec. Noms de personnes retirés de 13 sources (5 relevées, 8 trouvées en relisant), pseudonymes compris | W-043 et W-055 clos ; reste, avant la 0.10.0 : rien de ce lot |

La migration Python → Rust elle-même, terminée le 2026-11-04, garde son propre document :
[migration-rust.md](migration-rust.md) (phases, preuves d'équivalence, verdict du prototype d'interface).
| 28 | 2026-11-18 | **Bibliothèque de données** (branche `feat/data-library`) : crate `otb-store`, six tables en **fichiers JSON** dans `bibliotheque/` — drones, logs connus, fragments, `diff all` datés, mesures avec leur version de calcul, liste d'analyse. Un seul module d'accès ; un fichier illisible ou plus récent est mis de côté, jamais écrasé. 15 tests, **5 contre-preuves vues en échec** (mise de côté, version plus récente, péremption des mesures, doublon de log, réglages identiques). Volumes mesurés dans [validation.md](validation.md) | la table des mesures pèse 8 Mo et met 959 ms à se lire au volume plein : **à charger à la demande au lot 31** (noté dans ce lot) |
| 33 | 2026-11-22 | **Terme D rejoué dans l'ordre du firmware** (branche `fix/dterm-filter-order`, W-072 point 1) : `simulate_dterm_at` (page Filtres, assistant, contrôle par étage) filtre puis dérive ; `simulate_dterm` garde l'ancien ordre pour les références Python (`otb-parity` 0 écart). Mesuré sur 676 sessions : quelques centièmes de dB au plus sur les spectres exacts, contrôle par étage du D inchangé, **aucun constat de l'assistant ne bascule** ([validation.md](validation.md#ordre-du-terme-d-2026-11-22-lot-33)). Contre-preuve : l'ancien ordre s'écarte de 1,3 % d'un D « firmware » écrit pas à pas (test rouge), L-108 | W-072 point 2 et l'ordre de `simulate_dterm` : au réenregistrement des références (W-028) ou au lot 23 |
| 32a | 2026-11-22 | **Récupération par USB, protocole** (branche `feat/usb-protocol`, D-063) : crate `otb-usb` (trames MSP v1 jumbo et v2, `serial2`, copie par blocs, contrôles avant effacement, `diff all`, carte simulée) et `otb-dev usb` pour l'essai à la main. Preuve : 38 tests, dont un par refus d'effacer (drone armé, vol enregistré pendant ou après la copie, autre carte, identifiant nul sur une autre connexion, copie incomplète, log sans vol, fichier existant) ; chaque protection retirée à la main fait rougir un test (L-071) ; le vrai port essayé par un pseudo-terminal | 32b (écran, stockage de masse, bibliothèque) ; essais restants sur une vraie carte (drone armé, configurateur ouvert, Windows). **Essayé sur une G474** : 8 Mo copiés à 3,25 Mo par minute, vérifiés, effacés en 42,5 s ([spec-usb.md](spec-usb.md#résultats)) |
| 29b | 2026-11-23 | **Écran d'extraction d'un fragment** (branche `feat/fragment-extraction`, D-065, W-069 clos) : le visualiseur devient l'écran hors rail du prototype (barre « Modifier le fragment », bornes à onglets et poignées, barre de navigation, panneau Fragment / Traces / Lecteur) ; les bornes du fragment sont la période d'analyse, enregistrées et retrouvées à la réouverture ; rail aux sept entrées du prototype. Preuve : 13 tests nouveaux ou récrits, dont 9 vus en échec protection retirée, captures des deux thèmes, 2,2 ms par cadre sur `FINAL.bbl` | liste d'analyse à l'écran, second fragment d'une session ; fiche drone (lot 30) |
| 29c | 2026-11-23 | **Liste d'analyse à l'écran** (branche `feat/analysis-list`, D-066, D-049 remplacée) : barre de fragments du prototype (désactiver, maître, glisser, retirer), les pages la lisent au lieu de leurs listes « Fichiers », « + » du tableau de bord qui n'ouvre que sa session, liste retrouvée au lancement. Preuve : 10 tests, 9 contre-preuves vues en échec, captures des deux thèmes | second fragment d'une session ; fiche drone (lot 30) |
| 29d | 2026-11-23 | **Second fragment d'une session** (branche `feat/second-fragment`, D-067, D-061 amendée) : « Enregistrer comme nouveau fragment » dans l'écran d'extraction (pastille posée sur les mêmes échantillons, juste après l'origine), suppression d'un fragment ajouté en deux temps, réouverture par « + », crayon ou liste du lancement. Preuve : 5 tests, 8 contre-preuves vues en échec, captures des deux thèmes. **Lot 29 terminé** | W-074 (rôle vu par le Guide) ; fiche drone (lot 30) |
| 30d | 2026-11-23 | **Fiche d'un drone** (branche `feat/drone-sheet`, D-068, D-036, D-039 et D-061 amendées) : écran hors rail au dessin du prototype (Identité, Machine, Configurations Betaflight avec « Voir » et « Comparer », suppression en deux temps) ; la fiche vit dans la bibliothèque et le Guide en lit une copie ; ancien `drones.json` repris puis renommé ; un log rejoint le drone qui porte son nom ; bouton rouge plein lisible en thème sombre. Preuve : 17 tests nouveaux (5 de la bibliothèque, 8 de la fiche, 4 du Guide), les tests du Guide récrits sur la bibliothèque, 6 contre-preuves vues en échec, captures des deux thèmes, 0,2 ms par cadre | « Fiche cible » (lot 31) ; identifiant et carte au branchement (lot 32b) ; W-075 |
| 30e | 2026-11-23 | **Choix « Gyro \| Terme D » de l'espace** (branche `feat/signal-choice`, D-069) : un seul choix du signal et des axes partagés par le Spectre, Fréq. × gaz, Fréq. × temps et le « Et si », en tête de leur panneau ; un changement d'une vue vaut pour les autres, l'étage choisi (avant ou après filtres) est gardé ; cases détaillées gardées dessous ; × gaz s'ouvre sur « Gyro pf \| Gyro » par fragment ; doublons « Gyro » et « D » du « Et si » retirés. Preuve : 5 tests, 5 contre-preuves vues en échec, captures des deux thèmes ; temps d'un cadre mesuré pour chaque page par l'exemple `capture` | assistant en cartes (30f) ; avancement du parcours (30g, W-074) |
| 30f | 2026-11-23 | **Assistant en cartes** (branche `feat/advisor-cards`, D-070) : une carte par constat triée par gravité, « info » et vérifications OK repliées, pastilles par gravité, réponse indicielle de chaque axe en petit à droite ; jauge « mesure contre seuil » sur chaque constat à un seul seuil (`Gauge`, mêmes variables que le texte), hors des références Python (`otb-parity` 0 échec sur 1 732 appels). Preuve : 12 tests nouveaux ou récrits (7 de la page, 5 de la jauge), 9 contre-preuves vues en échec, captures des deux thèmes, 0,4 ms par cadre | W-076 (comparaison, votes, historique, lignes CLI) ; avancement du parcours (30g, W-074) |
