# Assistant de réglage — fonctionnement et calibration

L'assistant (`src/opentunebox/advisor/`) transforme les mesures d'un log en constats argumentés.
Chaque constat cite la mesure et le seuil qui le déclenchent ; les actions passent d'abord par les
curseurs du réglage simplifié de Betaflight, par petits pas (0,1), une chose à la fois.

## Taille du drone

Détectée par la fréquence de rotation des moteurs en vol (médiane de l'eRPM mesuré, drone armé, gaz > 5 %,
moteurs > 10 Hz) : ≥ 345 Hz whoop · 290–345 Hz 2–3,5" · 175–290 Hz 4–5" · 115–175 Hz 6–7" · < 115 Hz 8"+.
Sans eRPM, par le nom du drone : « whoop », « 65 », « 75 », « Meteor » → whoop ; sinon une taille en pouces
(« 5" », « 7 pouces ») : < 3,8 → 2–3,5", < 5,8 → 5", < 7,8 → 7", au-delà 8"+. À défaut : profil 5".
Le profil peut être forcé dans la fenêtre de l'assistant ou avec `opentunebox report --profil`.

Limites vérifiées sur le corpus (fréquence médiane par drone) : AIR65 ≈ 480 Hz, Meteor 75 ≈ 355 Hz,
P85 ≈ 385 Hz → whoop ; Pavo20 ≈ 340 Hz, 3" ≈ 325 Hz → 2–3,5". Entre 250 et 400 Hz, la fréquence sépare mal
les tailles : un « FoxWhoop 25 » tourne à 280 Hz et des 3,5" (Bee35, JeNo 3.5) vers 255–265 Hz, rangés en 5".

## Calibration (2026-10-05)

`scripts/calibrate_advisor.py 600 stats.json` sur le corpus `~/Documents/Drones/Logs`, puis
`scripts/calibrate_advisor.py --resume stats.json` : 707 sessions exploitables de 90 drones
(366 × 5" / 54 drones, 179 × 7" / 31, 120 whoops / 10, 22 × 2–3,5" / 8, 20 × 8"+ / 6), période automatique,
roll + pitch. Un drone est identifié par son nom (`craft_name`), à défaut par le dossier du log ; un nom
porté par plusieurs machines (`SHARED_NAMES` de `scripts/corpus_labels.py`, ex. « Jax » chez Guigui) est
complété par le dossier. Les logs sont lus par `corpus_labels.py:log_files` : dossiers `__MACOSX` ignorés,
et un fichier au contenu identique à un autre n'est compté qu'une fois (26 doublons sur 565 fichiers le
2026-10-05, dont 23 logs d'un même drone publiés deux fois dans « 1. PTB »). Les chiffres de cette section
datent d'avant : doublons comptés deux fois, deux « Jax » et deux « Blueberry » comptés comme un drone.

Les seuils « élevé » sont des **90e percentiles** : un drone est signalé quand il fait partie des 10 % les
plus bruités / lents / déséquilibrés de sa catégorie. Deux façons de compter :

- **par session** (seuils de bruit, de latence et de retard des 5", 7" et 2–3,5") ;
- **une voix par drone** (médiane de ses sessions), pour qu'un drone très souvent loggé ne fixe pas seul le
  seuil : seuils du whoop (63 des sessions whoop viennent d'un même AIR65) et seuils mécaniques.

| Mesure (p10 / p50 / p90 par session) | whoop | 2–3,5" | 5" | 7" |
|---|---|---|---|---|
| Gyro filtré 80–500 Hz (dB) | −36,6 / −31,7 / −28,5 | −35,0 / −32,9 / −27,3 | −37,8 / −33,3 / −28,5 | −39,4 / −36,9 / −30,3 |
| Gyro non filtré 80–500 Hz (dB) | −25,1 / −22,3 / −13,5 | −22,7 / −21,0 / −10,2 | −28,4 / −19,9 / −13,3 | −29,4 / −24,1 / −17,5 |
| Terme D 80–500 Hz (dB) | −25,2 / −18,2 / −12,0 | −27,0 / −20,3 / −11,5 | −27,7 / −20,4 / −13,6 | −33,4 / −24,6 / −16,5 |
| Gyro filtré 20–80 Hz (dB) | −26,0 / −21,5 / −14,9 | −27,2 / −23,5 / −13,6 | −30,0 / −24,9 / −15,3 | −32,5 / −28,6 / −21,2 |
| 1re harmonique moteur restante (dB) | −42,2 / −36,0 / −29,8 | −39,2 / −36,9 / −32,0 | −40,5 / −35,3 / −28,8 | −41,2 / −38,1 / −31,9 |
| Pic de réponse indicielle | 1,03 / 1,08 / 1,27 | 1,03 / 1,10 / 1,89 | 1,00 / 1,07 / 1,34 | 1,01 / 1,07 / 1,23 |
| Latence 50 % (ms) | 10,5 / 14,5 / 18,2 | 12,1 / 17,9 / 24,5 | 10,5 / 15,2 / 20,9 | 14,5 / 19,0 / 23,5 |
| Pic de sensibilité (dB), méthode approchée | 2,4 / 3,9 / 7,5 | 2,2 / 3,1 / 5,5 | 0,0 / 3,1 / 5,4 | 1,7 / 3,0 / 5,0 |
| Marge de phase (°), méthode approchée | 37 / 63 / 74 | 45 / 66 / 71 | 49 / 68 / 77 | 51 / 65 / 74 |
| Retard filtres gyro (ms) | 1,1 / 1,2 / 1,5 | 1,0 / 1,1 / 1,6 | 1,0 / 1,3 / 2,0 | 1,2 / 1,8 / 2,5 |
| Retard filtres D (ms) | 1,7 / 2,3 / 2,5 | 1,7 / 2,3 / 2,5 | 1,9 / 2,3 / 2,7 | 2,2 / 2,5 / 2,8 |

Unités : densité spectrale de puissance en dB de (°/s)²/Hz pour le gyro, d'unités PID²/Hz pour le D.

Seuils retenus dans `advisor/profiles.py` :

- **5" et 7"** : seuils de bruit = p90 par session ; latence = p10 arrondi vers le bas et p90 vers le haut ;
  retards = p90 arrondi vers le haut (+0,1 à +0,2 ms). Valeurs de la calibration du 2026-10-01
  (708 sessions), que celle-ci reproduit à 0,3 dB près.
- **Whoop** (provisoire, 10 drones) : une voix par drone — gyro −28,0 dB, D −14,7 dB, 20–80 Hz −13,9 dB,
  harmonique −32,2 dB, latence 13–20 ms, retards 2,1 / 3,5 ms (p90 arrondi au dixième supérieur + 0,1 ms).
- **2–3,5"** (provisoire, 8 drones) : par session — gyro −27,3 dB, D −11,5 dB, 20–80 Hz −13,6 dB,
  harmonique −32,0 dB, latence 12–25 ms, retards 1,8 / 2,7 ms. Avec une voix par drone, un seul drone très
  bruité fixait le seuil (D −3,6 dB).
- **8"+** (provisoire, 6 drones, 20 sessions) : seuils estimés à la main (gyro −31 dB, D −17 dB,
  20–80 Hz −22 dB, harmonique −32 dB, latence 18–30 ms, retards 3,2 / 3,5 ms).

Pour un profil provisoire, l'assistant ajoute le constat « Seuils provisoires pour cette taille » (info).

**Seuils mécaniques** (p90, une voix par drone) : par taille quand elle compte au moins 20 drones (5" et 7"),
sinon communs à tout le corpus (90 drones). La saturation est toujours commune : par taille, son p90 vaut
0 % en 7" (la moindre saturation serait signalée).

| Seuil mécanique | Commun (whoop, 2–3,5", 8"+) | 5" | 7" |
|---|---|---|---|
| Déséquilibre des moteurs (%) | 13,5 | 14,1 | 11,0 |
| Terme I moyen (valeur absolue) | 33 | 32 | 29 |
| Saturation (% du temps ≥ 98 %) | 1,5 | 1,5 | 1,5 |
| Écart de pic roll / pitch | 0,12 | 0,12 | 0,07 |
| Résonance (dB au-dessus du bruit voisin) | 7,9 | 7,4 | 7,3 |

### Corpus de référence (`corpus.csv`)

Plus de la moitié des logs du corpus (« 1. PTB », « 3. BBLR FB ») sont des **demandes d'aide** : les
percentiles ci-dessus décrivent donc surtout des drones à problème, et sont probablement trop indulgents.
Pour les recalculer sur des drones connus, `scripts/corpus_labels.py` tient un tableau d'étiquettes,
`corpus.csv`, rangé dans le dossier des logs (`OTB_LOGS`, hors du dépôt), une ligne par drone (même
identité que la calibration), séparateur « ; » :

| Colonne | Contenu |
|---|---|
| `drone`, `dossiers`, `sessions`, `origine` | identité, dossiers des logs, sessions de 8 s ou plus, dossier de 1er niveau |
| `taille_detectee`, `freq_moteur_hz` | profil détecté par l'assistant, fréquence moteur médiane en vol |
| `motor_kv` | réglage `motor_kv` ; vide s'il vaut 1960, la valeur par défaut du firmware (`pg/motor.c`) |
| `cellules` | formule du firmware (`sensors/battery.c`) : tension au branchement (`vbatref`) / `vbatcellvoltage` max + 1 |
| `etat` | `aide`, `origine` (réglages d'usine), `regle` (réglage terminé) ou `inconnu` |
| `taille_reelle` | taille en pouces (« 3,5 ») ou « whoop » |
| `remarque` | texte libre |

Les colonnes automatiques sont mises à jour à chaque passage du script. `etat`, `taille_reelle` et
`remarque` sont pré-remplies à la création de la ligne, puis **jamais écrasées** : `etat` = `aide` pour
« 1. PTB » et « 3. BBLR FB », sinon `regle` si un nom de fichier ou de dossier contient « final »,
« tuned » ou « last », sinon `origine` s'il contient « stock », sinon `inconnu` ; `taille_reelle` seulement
si la taille est écrite dans un nom (« 3.5 inch », « 5" », « Air65 »). Le pré-remplissage est à vérifier à la
main. Un drone absent d'un passage complet garde sa ligne avec 0 session (à effacer à la main). Le script
signale les noms trouvés dans des dossiers sans lien (ni l'un dans l'autre) : si ce sont des machines
différentes, ajouter le nom à `SHARED_NAMES`, sinon à `SAME_DRONE_NAMES` (plus d'alerte).

**Corpus de référence** = drones dont l'état est `regle` ou `origine`. `calibrate_advisor.py --resume`
affiche chaque groupe (taille, puis TOUS) suivi de sa partie « référence » quand `corpus.csv` existe. Les
seuils actuels restent ceux du corpus entier tant que le tableau n'est pas rempli.

## Règles

Niveaux : **attention** (à corriger), **conseil** (amélioration), **info**. Les règles de réponse et de bruit
portent sur le roll et le pitch (le yaw n'est pas jugé).

- **Enregistrement** : fréquence du log < 900 Hz (attention) ; 3e harmonique moteur au-delà de la moitié de
  la fréquence du log ; gyro non filtré absent ; pas de télémétrie RPM ; plus de 20 trames corrompues (info).
- **Vol** : crash détecté, la période d'analyse s'arrête avant (info).
- **Réponse** (au moins 20 fenêtres par axe, sinon « Pas assez de mouvements de manche ») :
  - dépassement : pic > 1,20 (attention), > 1,10 (conseil) ; attribué au feedforward si F > 0 et si la
    latence est plus courte que le repère bas de la taille ;
  - réponse molle : pic < 0,98 et latence au-delà du repère haut de la taille ;
  - roll et pitch différents : écart de pic > seuil de la taille (curseur « D pitch ») ;
  - stabilité (boucle ouverte, voir « Boucle ouverte mesurée ») : pic de sensibilité > 6 dB (conseil ;
    « au moins » quand le pic est au bord de la bande mesurable ; précision ajoutée si la boucle n'est
    qu'approchée) ; sinon marge de phase < 40° (conseil). Conseil : Damping +0,1 ou Master −0,1 ; seulement
    Master −0,1 quand le D domine déjà au croisement (|Ĉd/Ĉe| > 1) ;
  - oscillations basses fréquences : gyro filtré 20–80 Hz > p90 de la taille.
- **Bruit** (80 Hz à min(500 Hz ; 0,95 × Nyquist), valeur maximale du roll et du pitch) : gyro filtré et
  terme D > p90 de la taille ; 1re harmonique moteur restante > p90 (seulement avec eRPM, gyro non filtré
  et filtre RPM actif).
- **Filtrage trop prudent** : retard gyro > repère de la taille alors que gyro et D sont peu bruités ;
  retard D > repère alors que le D est peu bruité.
- **Résonance** : raie au-dessus de la moyenne glissante voisine de plus que le seuil de la taille
  (60 Hz à min(900 Hz ; 0,9 × Nyquist)), présente dans la médiane de toutes les tranches de gaz (au moins
  8 tranches), à plus de ±8 % des harmoniques 1 à 3 du moteur.
- **Mécanique** : déséquilibre des moteurs > seuil (lu avant/arrière, gauche/droite ou diagonale, côté ou
  diagonale retenus au-delà de 12 %) ; terme I moyen décentré (roll ou pitch, valeur absolue > seuil) ;
  saturation moteur (≥ 98 %) plus longtemps que le seuil.
- **Réglage** : seuils provisoires pour cette taille (info) ; réglage simplifié désactivé, les conseils
  « curseur » sont indicatifs (info).

## Origine des seuils

Tous les seuils n'ont pas la même solidité. Chacun est classé selon son origine :

- **Corpus** : mesuré sur les vrais logs (percentile par taille), recalculable avec
  `scripts/calibrate_advisor.py` ;
- **Communauté** : repère de pilotes ou d'outils publics, avec sa source (auteur, date, adresse, citation)
  dans `docs/spec-methodes.md`, « Sources des repères » (recherche du 2026-10-05) ; les renvois [S1]… du
  tableau désignent cette liste ;
- **Théorie** : critère classique d'automatique ou de traitement du signal ;
- **Firmware** : valeur par défaut ou fonctionnement de Betaflight (`docs/spec-betaflight.md`) ;
- **Estimation** : choisi en écrivant les règles, ni calibré ni sourcé. À vérifier en priorité.

| Règle | Seuil | Origine | Remarque |
|---|---|---|---|
| Taille par l'eRPM | 345 / 290 / 175 / 115 Hz | Corpus (345) + estimation | 345 Hz placé entre les whoops et les 2–3,5" du corpus (voir « Taille du drone ») ; les autres limites ne sont pas vérifiées. La table de `spec-methodes.md` (whoop 550–800 Hz) ne correspond pas aux logs |
| Taille par le nom | « whoop », « 65 », « 75 », « Meteor » ; 3,8 / 5,8 / 7,8 pouces | Estimation | |
| Profil par défaut | 5" | Corpus | Taille la plus fréquente (366 sessions sur 707) |
| Fréquence du log trop basse | < 900 Hz | Firmware + estimation | Défaut de Betaflight : 1 trame sur 4 (`blackbox_sample_rate` = 1/4), soit 2 kHz avec une boucle à 8 kHz et 1 kHz à 4 kHz ; 900 Hz laisse une marge sous 1 kHz |
| Harmoniques hors de la plage visible | 3 × f moteur > Fs / 2 | Théorie | Fréquence de Nyquist |
| Gyro non filtré absent, pas d'eRPM | présence des champs | Firmware | |
| Trames corrompues | > 20 | Estimation | Constat informatif ; p90 du corpus ≈ 1 trame, un seuil calibré signalerait la moindre trame |
| Mouvements de manche suffisants | ≥ 20 fenêtres | Estimation | |
| Fenêtre de réponse retenue | consigne ≥ 20 °/s | Communauté | Code de PID-Analyzer (2018) et de PIDtoolbox v0.55 (2022) [S5, S2] |
| Dépassement | pic > 1,10 (conseil), > 1,20 (attention) ; objectif 1,02–1,08 | Estimation | Aucune source publique pour ces paliers (recherche du 2026-10-05) : les sources ne donnent que le sens (« not too much overshoot », plus de D contre le dépassement [S1, S4]) ; UAV Tech vise une réponse sans dépassement. Dans le corpus, le p90 vaut 1,2 à 1,3 : plus de 10 % des sessions sont en « attention ». À calibrer sur le corpus de référence |
| Dépassement dû au feedforward | F > 0 et latence < p10 de la taille | Corpus + estimation | Repère de latence mesuré, raisonnement choisi |
| Réponse molle | pic < 0,98 et latence > p90 de la taille | Estimation (0,98) + corpus (latence) | |
| Roll et pitch différents | écart de pic > 0,07 à 0,12 selon la taille | Corpus | Seuils mécaniques |
| Stabilité | pic de sensibilité > 6 dB | Théorie | Skogestad et Postlethwaite : Ms « inférieur à environ 2 (6 dB) ». S'applique directement à la boucle **mesurée** (celle des manuels) ; la valeur approchée (repli) n'est qu'une analogie. Mesuré, il signale 29 % des sessions du corpus (14 % avec l'ancienne formule) : constat ramené de « attention » à « conseil » jusqu'à la recalibration sur le corpus de référence (décision de Cédric, 2026-10-05) |
| Marge de phase | < 40° | Théorie + estimation | Åström et Murray : valeurs raisonnables 30–60° ; 40° est choisi dans cette plage. Même réserve pour la valeur approchée |
| D dominant au croisement | \|Ĉd/Ĉe\| > 1 | Estimation | Choisit le conseil (Master −0,1 plutôt que plus de D). Appuyé sur la simulation (`tests/test_loop.py`) et la série BlackSummer (`validation.md`) : au-delà, plus de D monte le croisement sans gagner de marge, et le pic de sensibilité augmente |
| Bandes de bruit | 80–500 Hz ; 20–80 Hz | Communauté + estimation | Propwash et oscillations « ~20Hz to ~100Hz » [S1], moteurs ~200–500 Hz [S7] ou au-delà de 250 Hz [S6] ; bornes exactes choisies |
| Bruit gyro, terme D, 20–80 Hz, harmonique moteur | p90 de la taille | Corpus | Relatif au corpus : « plus bruité que 90 % », pas « mauvais » dans l'absolu |
| Retards des filtres gyro et D | p90 de la taille, arrondi vers le haut | Corpus | |
| Latence (repères bas et haut) | p10 arrondi vers le bas, p90 vers le haut | Corpus | |
| Seuils des profils whoop et 2–3,5" | voir « Calibration » | Corpus (provisoire) | 10 et 8 drones seulement |
| Seuils du profil 8"+ | voir « Calibration » | Estimation | 6 drones, 20 sessions |
| Résonance | raie > 7,3 à 7,9 dB selon la taille ; 60–900 Hz, ≥ 8 tranches de gaz, à plus de ±8 % des harmoniques moteur | Corpus (seuil) + estimation (méthode) | |
| Déséquilibre des moteurs | > 11 à 14,1 % selon la taille | Corpus | L'interprétation (côté ou diagonale au-delà de 12 %) reste estimée |
| Terme I décentré | moyenne > 29 à 33 (unités PID) selon la taille | Corpus | |
| Saturation des moteurs | ≥ 98 % pendant > 1,5 % du temps | Corpus | La notion de « moteur à 98 % » est estimée : le firmware ne journalise pas de saturation (il renormalise le mixage quand l'écart entre moteurs dépasse 100 %) |
| Pas des curseurs conseillés | 0,1 (0,1 à 0,2 pour les filtres) | Estimation | Le configurateur avance par 0,05 ; en mode de base il borne PI, D et FF à 0,70–1,40, le filtre gyro à 0,50–1,50 et le filtre D à 0,80–1,20 |
| `rpm_filter_q` conseillé | 500 → 350 | Firmware (500) + estimation (350) | Défaut 500, plage 250–3000. Les sources conseillent plutôt de **monter** Q (jusqu'à 750 [S9] ou 1000 [S6, S14]) et de redescendre seulement si le bruit moteur réapparaît ; aucune ne donne de valeur sous 500 |
| `motor_poles` | 14 (12 pour certains micro-moteurs) | Firmware (14) + communauté (12) | Défaut 14 dans Betaflight ; « Smaller motors have fewer magnets, often 12 » [S9], 12 pour les 08XX à 16XX [S13] |
| Notch statique conseillé | coupure = 0,7 × fréquence de la raie | Estimation | |

Les champs `hover_hz` et `resonance_hz` des profils (`advisor/profiles.py`) sont renseignés mais
**aucune règle ne les utilise**.

### Sources vérifiées

- S. Skogestad, I. Postlethwaite, *Multivariable Feedback Control*, 2e éd., Wiley, 2005, § 2.4.3,
  p. 36 : Ms inférieur à environ 2 (6 dB) ; équation 2.47 : marge de phase ≥ 2·arcsin(1/(2 Ms)), soit 29°
  pour Ms = 2 ([PDF de l'auteur](https://skoge.folk.ntnu.no/book/ps/bookall.pdf)).
- K. J. Åström, R. M. Murray, *Feedback Systems*, 1re éd., Princeton, 2008, § 9.3, p. 281 : marge de phase
  30–60°, marge de gain 2–5, marge de stabilité 0,5–0,8 (Ms 1,25–2).
- Firmware Betaflight (commit e796d7f87) : `rpm_filter_q` = 500 (`pg/rpm_filter.c`), `motor_poles` = 14
  (`pg/motor.c`), `blackbox_sample_rate` = 1/4 (`blackbox/blackbox.c`). Configurateur : pas et bornes des
  curseurs (`PidSubTab.vue`, `FilterSubTab.vue`, `useTuningSliders.js`).

### Boucle ouverte mesurée

Jusqu'à la version 0.3.0, le pic de sensibilité et la marge de phase étaient calculés sur L = T/(1 − T),
T étant la fonction de transfert consigne → gyro. Ce n'est la vraie boucle ouverte que si tout le
régulateur agit sur l'erreur ; dans Betaflight le D agit sur le gyro et le feedforward sur la consigne.
Depuis, `analysis/loop.py` **mesure** L = G·(Ce + Cd) à partir des termes P, I, D, F loggés
(`spec-methodes.md`, « Boucle ouverte ») : c'est la boucle des manuels, à laquelle les limites de
Skogestad et Postlethwaite et d'Åström et Murray s'appliquent directement. Ce que montrent la simulation
et les vrais logs (`validation.md`) :

- l'ancienne formule place le croisement environ 2 fois trop bas et surestime la marge de phase d'une
  dizaine de degrés en moyenne ; elle fait croire que plus de D augmente toujours la marge. Quand le D
  domine déjà au croisement, plus de D monte le croisement là où les filtres retardent : la marge ne
  progresse plus et le pic de sensibilité augmente ;
- la mesure ne va pas plus loin que l'énergie des mouvements de manche (souvent 20 à 40 Hz) : le
  croisement est en général mesuré, le pic de sensibilité souvent au-delà (affiché « au moins ») ;
- repli sur l'ancienne formule (« approchée ») quand un terme de gain non nul n'est pas enregistré ou que
  la mesure manque de points : le constat le précise ;
- avec les seuils actuels, le pic > 6 dB signalerait 29 % des sessions du corpus (14 % avec l'ancienne
  formule), la marge < 40° 3 % (1 %) : les constats de stabilité restent des conseils jusqu'à la
  recalibration.

Les percentiles du pic de sensibilité et de la marge de phase de la section « Calibration » datent de
l'ancienne méthode : à recalculer avec la mesure lors de la prochaine recalibration.

### Onglet Autotune du configurateur Betaflight

Il utilise l'ancienne méthode approchée (L = T/(1 − T), Ms ≤ 2) avec des marges de phase cibles de 50°, 60° et
72,5°. Il **n'est pas cité comme source** (vérifié le 2026-10-05) : ces valeurs n'existent que sur la branche
de développement (PR [#5406](https://github.com/betaflight/betaflight-configurator/pull/5406), absente de
2026.6.2) ; 50 et 60° sont des choix dans la plage classique, 72,5° est propre à l'auteur, réglé sur deux
vols d'un seul 5" ; la PR a été approuvée sans essais en vol, et un ticket de perte de contrôle
([#5258](https://github.com/betaflight/betaflight-configurator/issues/5258)) est ouvert. Ms ≤ 2 est cité
d'après Skogestad et Postlethwaite.
