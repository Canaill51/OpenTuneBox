# Assistant de réglage — fonctionnement et calibration

L'assistant (`src/opentunebox/advisor/`) transforme les mesures d'un log en constats argumentés.
Chaque constat cite la mesure et le seuil qui le déclenchent ; les actions passent d'abord par les
curseurs du réglage simplifié de Betaflight, par petits pas (0,1), une chose à la fois.

## Taille du drone

Détectée par la fréquence de rotation des moteurs en vol (médiane de l'eRPM mesuré, drone armé, gaz > 5 %,
moteurs > 10 Hz) et par le nombre de cellules de la batterie (`FlightLog.cells`, formule du firmware) :

- batterie 1S → whoop ;
- ≥ 345 Hz whoop · 290–345 Hz 2–3,5" · 160–290 Hz 4–5" · 115–160 Hz 6–7" · < 115 Hz 8"+ ;
- entre 200 et 290 Hz, une batterie de 2 à 4S → 2–3,5" (un 5" en 4S tourne plus lentement).

Sans eRPM : 1S → whoop ; sinon par le nom du drone : « whoop », « 65 », « 75 », « Meteor » → whoop ; une
taille en pouces (« 5" », « 7 pouces ») : < 3,8 → 2–3,5", < 5,8 → 5", < 7,8 → 7", au-delà 8"+. À défaut :
profil 5". Le profil peut être forcé dans la fenêtre de l'assistant ou avec `opentunebox report --profil`.

Vérification (2026-10-05) sur 41 drones de taille connue : les 20 tailles notées par Cédric dans
`corpus.csv` et 21 drones dont le nom donne la taille (Bee35, Whisper25, Pavo20, SevenInch, Mario8, M5…).
Justes : 38 drones sur 41 (27 avec la fréquence seule) ; par session, 80 % (68 %). Restent faux un 5" en
4S à 222 Hz (classé 2–3,5"), un drone rangé dans un dossier « 3.5 » qui tourne à 203 Hz, et un 3" sans
tension enregistrée. Pas de règle pour le 6S : deux 3,5" en 6S tournent vers 255 Hz, mais des 5" en vol
freestyle montent jusqu'à 275 Hz (JeNo 5 « FINAL » à 255 Hz) ; ils restent donc rangés en 5". La limite
5"/7" est passée de 175 à 160 Hz : plusieurs 5" tournent entre 160 et 170 Hz. La moitié des tailles de
vérification vient de noms interprétés à la main et les limites ont été choisies sur cette liste : le
score est un peu optimiste.

## Calibration (2026-10-05)

`scripts/calibrate_advisor.py 600 stats.json` sur le corpus `~/Documents/Drones/Logs`, puis
`scripts/calibrate_advisor.py --resume stats.json` : 681 sessions exploitables de 94 drones
(299 × 5" / 55 drones, 133 × 7" / 19, 121 whoops / 11, 108 × 2–3,5" / 20, 20 × 8"+ / 6 ; un drone peut
compter dans deux tailles si ses sessions diffèrent), période automatique, roll + pitch. Un drone est
identifié par son nom (`craft_name`), à défaut par le dossier du log ; un nom porté par plusieurs machines
(`SHARED_NAMES` de `scripts/corpus_labels.py`, ex. « Jax » chez Guigui) est complété par le dossier. Les
logs sont lus par `corpus_labels.py:log_files` : dossiers `__MACOSX` ignorés, et un fichier au contenu
identique à un autre n'est compté qu'une fois.

**Tout le corpus compte** : il n'y a pas de corpus de référence. Plus de la moitié des logs (« 1. PTB »,
« 3. BBLR FB ») sont des demandes d'aide, donc surtout des drones à problème, et Cédric ne peut pas dire
quels drones sont bien réglés (décision du 2026-10-05). Les percentiles sont donc **probablement trop
indulgents** : « plus bruité que 90 % du corpus » est une limite haute.

Les seuils « élevé » sont des **90e percentiles** : un drone est signalé quand il fait partie des 10 % les
plus bruités / lents / déséquilibrés de sa catégorie. Deux façons de compter :

- **par session** (seuils de bruit, de latence et de retard des 2–3,5", 5" et 7") ;
- **une voix par drone** (médiane de ses sessions), pour qu'un drone très souvent loggé ne fixe pas seul le
  seuil : seuils du whoop (la plupart des sessions whoop viennent d'un même AIR65) et seuils mécaniques.

| Mesure (p10 / p50 / p90 par session) | whoop | 2–3,5" | 5" | 7" | 8"+ |
|---|---|---|---|---|---|
| Gyro filtré 80–500 Hz (dB) | −36,5 / −31,7 / −28,4 | −38,1 / −33,3 / −29,4 | −37,6 / −33,3 / −28,3 | −39,5 / −37,5 / −31,4 | −38,9 / −36,2 / −28,1 |
| Gyro non filtré 80–500 Hz (dB) | −25,1 / −22,2 / −13,2 | −31,2 / −20,6 / −13,1 | −27,7 / −20,6 / −13,1 | −29,5 / −25,4 / −18,1 | −24,9 / −21,3 / −16,9 |
| Terme D 80–500 Hz (dB) | −25,1 / −18,1 / −11,8 | −27,0 / −21,3 / −14,8 | −29,8 / −20,6 / −13,1 | −33,2 / −24,9 / −17,5 | −30,9 / −22,7 / −12,9 |
| Gyro filtré 20–80 Hz (dB) | −26,0 / −21,5 / −14,9 | −29,9 / −24,6 / −13,9 | −30,6 / −25,0 / −15,7 | −32,4 / −28,6 / −21,7 | −34,2 / −26,2 / −16,8 |
| 1re harmonique moteur restante (dB) | −42,1 / −36,0 / −29,6 | −40,4 / −36,5 / −32,5 | −40,2 / −34,9 / −28,4 | −41,3 / −38,8 / −31,8 | −40,2 / −33,4 / −25,0 |
| Pic de réponse indicielle | 1,03 / 1,08 / 1,27 | 1,00 / 1,09 / 1,39 | 1,00 / 1,06 / 1,29 | 1,01 / 1,07 / 1,21 | 1,04 / 1,11 / 1,38 |
| Latence 50 % (ms) | 10,5 / 14,5 / 18,2 | 10,8 / 15,5 / 20,1 | 10,6 / 15,7 / 21,4 | 15,5 / 19,9 / 24,0 | 19,1 / 23,4 / 29,3 |
| Pic de sensibilité, boucle mesurée (dB) | −1,1 / 5,6 / 8,4 | −8,7 / 3,6 / 6,3 | −13,0 / 3,0 / 6,7 | −1,6 / 3,5 / 6,0 | −0,5 / 3,5 / 7,5 |
| Marge de phase, boucle mesurée (°) | 30 / 40 / 51 | 40 / 52 / 71 | 37 / 52 / 66 | 41 / 54 / 63 | 39 / 48 / 65 |
| Fréquence de croisement (Hz) | 12 / 20 / 27 | 8 / 14 / 23 | 10 / 15 / 25 | 7 / 12 / 20 | 6 / 10 / 13 |
| Retard filtres gyro (ms) | 1,1 / 1,2 / 1,5 | 1,0 / 1,3 / 1,9 | 1,0 / 1,4 / 2,0 | 1,3 / 1,9 / 2,6 | 1,0 / 2,9 / 3,6 |
| Retard filtres D (ms) | 1,7 / 2,3 / 2,5 | 1,7 / 2,3 / 2,6 | 1,9 / 2,3 / 2,8 | 2,3 / 2,5 / 2,7 | 1,9 / 2,5 / 2,9 |

Unités : densité spectrale de puissance en dB de (°/s)²/Hz pour le gyro, d'unités PID²/Hz pour le D. Pic
de sensibilité et marge de phase : boucle ouverte mesurée (voir « Boucle ouverte mesurée ») ; un pic négatif
vient d'une bande mesurable trop courte (minorant).

Seuils retenus dans `advisor/profiles.py` :

- **2–3,5", 5" et 7"** (recalculés le 2026-10-05) : bruit = p90 par session arrondi au 0,1 dB ; latence =
  p10 arrondi vers le bas et p90 vers le haut ; retards = p90 arrondi au dixième supérieur.
  2–3,5" : gyro −29,4 dB, D −14,8 dB, 20–80 Hz −13,9 dB, harmonique −32,5 dB, latence 10–21 ms, retards
  2,0 / 2,6 ms (provisoire : 20 drones, à la limite). 5" : −28,3 / −13,1 / −15,7 / −28,4 dB, 10–22 ms,
  2,1 / 2,8 ms. 7" : −31,4 / −17,5 / −21,7 / −31,8 dB, 15–24 ms, 2,6 / 2,8 ms.
- **Whoop** (provisoire, 11 drones) : inchangé depuis la calibration précédente (10 drones, une voix par
  drone) — gyro −28,0 dB, D −14,7 dB, 20–80 Hz −13,9 dB, harmonique −32,2 dB, latence 13–20 ms, retards
  2,1 / 3,5 ms. Un seul drone ajouté (un whoop 1S) déplaçait le seuil gyro de 4 dB.
- **8"+** (provisoire, 6 drones, 20 sessions) : seuils estimés à la main, inchangés (gyro −31 dB, D −17 dB,
  20–80 Hz −22 dB, harmonique −32 dB, latence 18–30 ms, retards 3,2 / 3,5 ms).

Pour un profil provisoire, l'assistant ajoute le constat « Seuils provisoires pour cette taille » (info).

**Seuils mécaniques** (p90, une voix par drone) : propres au 5" (55 drones) et au 7" (19 drones, gardé à
part bien qu'à la limite de 20), communs à tout le corpus (94 drones) pour les autres tailles. La saturation
est toujours commune : par taille, son p90 vaut 0 % en 7" (la moindre saturation serait signalée).

| Seuil mécanique | Commun (whoop, 2–3,5", 8"+) | 5" | 7" |
|---|---|---|---|
| Déséquilibre des moteurs (%) | 13,5 | 14,0 | 11,2 |
| Terme I moyen (valeur absolue) | 33 | 32 | 31 |
| Saturation (% du temps ≥ 98 %) | 1,5 | 1,5 | 1,5 |
| Écart de pic roll / pitch | 0,12 | 0,11 | 0,08 |
| Résonance (dB au-dessus du bruit voisin) | 7,9 | 6,8 | 8,2 |

Les seuils communs, recalculés (déséquilibre 13,3 %, terme I 33, écart 0,12, résonance 7,9 dB, saturation
1,4 %), restent ceux de la calibration précédente.

### Tableau d'étiquettes (`corpus.csv`)

`scripts/corpus_labels.py` tient un tableau, `corpus.csv`, rangé dans le dossier des logs (`OTB_LOGS`, hors
du dépôt), une ligne par drone (même identité que la calibration), séparateur « ; » :

| Colonne | Contenu |
|---|---|
| `drone`, `dossiers`, `sessions`, `origine` | identité, dossiers des logs, sessions de 8 s ou plus, dossier de 1er niveau |
| `taille_detectee`, `freq_moteur_hz` | profil détecté par l'assistant, fréquence moteur médiane en vol |
| `motor_kv` | réglage `motor_kv` ; vide s'il vaut 1960, la valeur par défaut du firmware (`pg/motor.c`) |
| `cellules` | formule du firmware (`sensors/battery.c`) : tension au branchement (`vbatref`) / `vbatcellvoltage` max + 1 |
| `etat` | `aide`, `origine` (réglages d'usine), `regle` (réglage terminé) ou `inconnu` ; deviné, non utilisé |
| `taille_reelle` | taille en pouces (« 3,5 ») ou « whoop » : sert à vérifier la détection de taille |
| `remarque` | texte libre |

Les colonnes automatiques sont mises à jour à chaque passage du script. `etat`, `taille_reelle` et
`remarque` sont pré-remplies à la création de la ligne, puis **jamais écrasées** : `etat` = `aide` pour
« 1. PTB » et « 3. BBLR FB », sinon `regle` si un nom de fichier ou de dossier contient « final »,
« tuned » ou « last », sinon `origine` s'il contient « stock », sinon `inconnu` ; `taille_reelle` seulement
si la taille est écrite dans un nom (« 3.5 inch », « 5" », « Air65 »). Un drone absent d'un passage complet
garde sa ligne avec 0 session (à effacer à la main). Le script signale les noms trouvés dans des dossiers
sans lien (ni l'un dans l'autre) : si ce sont des machines différentes, ajouter le nom à `SHARED_NAMES`,
sinon à `SAME_DRONE_NAMES` (plus d'alerte).

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
  - stabilité (boucle ouverte, voir « Boucle ouverte mesurée ») : pic de sensibilité > 7 dB (conseil ;
    « au moins » quand le pic est au bord de la bande mesurable ; précision ajoutée si la boucle n'est
    qu'approchée) ; sinon marge de phase < 35° (conseil). Conseil : Damping +0,1 ou Master −0,1 ; seulement
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
| Taille par l'eRPM et les cellules | 345 / 290 / 160 / 115 Hz ; 1S → whoop ; 2–4S entre 200 et 290 Hz → 2–3,5" | Corpus + estimation | Vérifié sur 41 drones de taille connue, 38 justes (voir « Taille du drone ») ; la limite 8"+ (115 Hz) n'est vérifiée que sur 6 drones. Le tableau de fréquences par taille de `spec-methodes.md` est une estimation qui ne correspond pas aux logs |
| Taille par le nom | « whoop », « 65 », « 75 », « Meteor » ; 3,8 / 5,8 / 7,8 pouces | Estimation | |
| Profil par défaut | 5" | Corpus | Taille la plus fréquente (366 sessions sur 707) |
| Fréquence du log trop basse | < 900 Hz | Firmware + estimation | Défaut de Betaflight : 1 trame sur 4 (`blackbox_sample_rate` = 1/4), soit 2 kHz avec une boucle à 8 kHz et 1 kHz à 4 kHz ; 900 Hz laisse une marge sous 1 kHz |
| Harmoniques hors de la plage visible | 3 × f moteur > Fs / 2 | Théorie | Fréquence de Nyquist |
| Gyro non filtré absent, pas d'eRPM | présence des champs | Firmware | |
| Trames corrompues | > 20 | Estimation | Constat informatif ; p90 du corpus ≈ 1 trame, un seuil calibré signalerait la moindre trame |
| Mouvements de manche suffisants | ≥ 20 fenêtres | Estimation | |
| Fenêtre de réponse retenue | consigne ≥ 20 °/s | Communauté | Code de PID-Analyzer (2018) et de PIDtoolbox v0.55 (2022) [S5, S2] |
| Dépassement | pic > 1,10 (conseil), > 1,20 (attention) ; objectif 1,02–1,08 | Estimation | Aucune source publique pour ces paliers (recherche du 2026-10-05) : les sources ne donnent que le sens (« not too much overshoot », plus de D contre le dépassement [S1, S4]) ; UAV Tech vise une réponse sans dépassement. Dans le corpus, le p90 vaut 1,2 à 1,3 : plus de 10 % des sessions sont en « attention ». À calibrer sur le corpus |
| Dépassement dû au feedforward | F > 0 et latence < p10 de la taille | Corpus + estimation | Repère de latence mesuré, raisonnement choisi |
| Réponse molle | pic < 0,98 et latence > p90 de la taille | Estimation (0,98) + corpus (latence) | |
| Roll et pitch différents | écart de pic > 0,07 à 0,12 selon la taille | Corpus | Seuils mécaniques |
| Stabilité | pic de sensibilité > 7 dB (conseil) | Théorie + corpus | Skogestad et Postlethwaite : Ms « inférieur à environ 2 (6 dB) » ; 7 dB (Ms ≈ 2,2) est un peu au-delà. Avec 6 dB / 40°, 33 % des sessions du corpus étaient signalées (72 % des whoops) ; avec 7 dB / 35°, 20 % (décision de Cédric, 2026-10-05). Conseil seulement : la mesure s'arrête souvent sous le pic (minorant) |
| Marge de phase | < 35° (conseil) | Théorie + corpus | Åström et Murray : valeurs raisonnables 30–60° ; 35° est dans cette plage, choisi avec le corpus (voir ci-dessus). Même réserve pour la valeur approchée |
| D dominant au croisement | \|Ĉd/Ĉe\| > 1 | Estimation | Choisit le conseil (Master −0,1 plutôt que plus de D). Appuyé sur la simulation (`tests/test_loop.py`) et la série BlackSummer (`validation.md`) : au-delà, plus de D monte le croisement sans gagner de marge, et le pic de sensibilité augmente |
| Bandes de bruit | 80–500 Hz ; 20–80 Hz | Communauté + estimation | Propwash et oscillations « ~20Hz to ~100Hz » [S1], moteurs ~200–500 Hz [S7] ou au-delà de 250 Hz [S6] ; bornes exactes choisies |
| Bruit gyro, terme D, 20–80 Hz, harmonique moteur | p90 de la taille | Corpus | Relatif au corpus : « plus bruité que 90 % », pas « mauvais » dans l'absolu. Repères de Brian White (gyro filtré < −30 dB, D < −20 dB idéal, −10 dB au plus [S20, S21]) sur l'échelle de PIDtoolbox, non comparable à notre densité spectrale moyenne 80–500 Hz |
| Retards des filtres gyro et D | p90 de la taille, arrondi vers le haut | Corpus | Brian White vise ≤ 1,5 ms pour le gyro filtré d'un 5", ≈ 5 ms pour un 10" [S20] ; p90 du corpus 5" : 2,0 ms (méthode de mesure différente de PIDtoolbox) |
| Latence (repères bas et haut) | p10 arrondi vers le bas, p90 vers le haut | Corpus | |
| Seuils des profils whoop et 2–3,5" | voir « Calibration » | Corpus (provisoire) | 10 et 8 drones seulement |
| Seuils du profil 8"+ | voir « Calibration » | Estimation | 6 drones, 20 sessions |
| Résonance | raie > 7,3 à 7,9 dB selon la taille ; 60–900 Hz, ≥ 8 tranches de gaz, à plus de ±8 % des harmoniques moteur | Corpus (seuil) + estimation (méthode) | |
| Déséquilibre des moteurs | > 11 à 14,1 % selon la taille | Corpus | L'interprétation (côté ou diagonale au-delà de 12 %) reste estimée |
| Terme I décentré | moyenne > 29 à 33 (unités PID) selon la taille | Corpus | |
| Saturation des moteurs | ≥ 98 % pendant > 1,5 % du temps | Corpus | La notion de « moteur à 98 % » est estimée : le firmware ne journalise pas de saturation (il renormalise le mixage quand l'écart entre moteurs dépasse 100 %) |
| Pas des curseurs conseillés | 0,1 (0,1 à 0,2 pour les filtres) | Estimation | Le configurateur avance par 0,05 ; en mode de base il borne PI, D et FF à 0,70–1,40, le filtre gyro à 0,50–1,50 et le filtre D à 0,80–1,20 |
| `rpm_filter_q` conseillé | 500 → 350 | Firmware (500) + estimation (350) | Défaut 500, plage 250–3000. Les sources conseillent plutôt de **monter** Q (jusqu'à 750 [S9] ou 1000 [S6, S14] ; 750–1000 sur un 5" propre selon Brian White [S20]) et de redescendre seulement si le bruit moteur réapparaît, ce qui est le cas où la règle le propose (bruit moteur restant après filtrage) ; aucune ne donne de valeur sous 500, d'où 350 estimé. Une règle « monter Q sur un drone propre » reste à écrire |
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
- seuils de stabilité recalibrés le 2026-10-05 avec la mesure : pic > 7 dB ou marge < 35°, en conseil
  (20 % des sessions du corpus ; avec 6 dB / 40°, 33 %). Les whoops restent souvent signalés (46 %) : à
  revoir quand leur mesure sera mieux comprise.

### Onglet Autotune du configurateur Betaflight

Il utilise l'ancienne méthode approchée (L = T/(1 − T), Ms ≤ 2) avec des marges de phase cibles de 50°, 60° et
72,5°. Il **n'est pas cité comme source** (vérifié le 2026-10-05) : ces valeurs n'existent que sur la branche
de développement (PR [#5406](https://github.com/betaflight/betaflight-configurator/pull/5406), absente de
2026.6.2) ; 50 et 60° sont des choix dans la plage classique, 72,5° est propre à l'auteur, réglé sur deux
vols d'un seul 5" ; la PR a été approuvée sans essais en vol, et un ticket de perte de contrôle
([#5258](https://github.com/betaflight/betaflight-configurator/issues/5258)) est ouvert. Ms ≤ 2 est cité
d'après Skogestad et Postlethwaite.
