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

### Repères de PIDtoolbox (2026-10-05)

Brian White donne ses repères sur l'échelle de PIDtoolbox : gyro filtré sous −30 dB, terme D au plus
−10 dB (idéal sous −20 dB) [S20–S22], et un graphique d'objectifs de retard des filtres selon la taille
[S26]. Pour les utiliser :

- **Échelle des spectres** : nos densités spectrales sont 6,5 dB ± 1 plus hautes que celles de PIDtoolbox
  (`spectral.PTB_PSD_OFFSET_DB`, `validation.md`). La mesure est le sommet du spectre lissé 80–500 Hz
  (`rules.band_peak_ptb_db`), ce que l'œil lit sur la courbe de PIDtoolbox.
- **Taux de déclenchement** (681 sessions, roll ou pitch) : D au-dessus de −10 dB : 18 % des sessions
  (15 % des drones ; whoop 22 %, 2–3,5" 13 %, 5" 21 %, 7" 8 %, 8"+ 40 %) ; avec le p90 du corpus, 44
  sessions de plus sont signalées (79 l'étaient déjà par les deux). D au-dessus de −20 dB : 79 % (trop
  pour un constat). Gyro : sommet au-dessus de −30 dB 35 % (raies moteur et châssis), niveau moyen 3 % :
  repère seulement affiché (choix de Cédric).
- **Retards** : gyro filtré, notre méthode de phase, PIDtoolbox et le modèle des filtres de l'en-tête
  concordent à ±0,2 ms ; même médiane sur le corpus par phase et par intercorrélation (écart −0,04 ms).
  Terme D : PIDtoolbox affiche 0,3 à 0,9 ms de moins que notre mesure et que le modèle des filtres
  (`validation.md`), son objectif D n'est donc qu'affiché. Objectifs du graphique comme seuils de
  conseil (bruit faible) : gyro 2–3,5" 11 → 62 %, 5" 9 → 24 %, 7" 10 → 33 %, 8"+ 58 → 67 % des sessions ;
  D 92 à 100 %. Choix de Cédric : déclenchement au p90 du corpus, objectif affiché dans le texte.
- **Écartées** (mesures dans `calibrate_advisor.py` au 2026-10-05, puis retirées) :
  TPA (erreur PID 20–100 Hz plus haute au-dessus de 50 % de gaz dans 90 % des 5" : sprints et manche, pas
  une oscillation ; il faudrait chercher une raie étroite) ; raie du lien radio (dans la consigne de 17 %
  des axes, vers 50 et 100 Hz, reprise par le gyro sur 3 axes seulement, et seulement 6 raies fixes sur
  588 vers ces fréquences).

### Filtre RPM : poids et Q (2026-10-05)

Méthode de la documentation de Betaflight 4.5 [S27] : juste assez de filtrage RPM pour que les raies
moteur « ne se voient plus » ; avec des tripales, monter d'abord Q (de 500 jusqu'à 1000 au plus) puis
baisser les poids de la 2e et de la 3e harmonique. Le firmware (`rpmNotchUpdate`, `common/filter.c`)
retranche poids × passe-bande : au centre du notch il reste (1 − poids) de la raie, 20·log10(1 − poids) dB.

- **Mesure** (`motors.harmonic_prominence`) : hauteur de chaque harmonique au-dessus du bruit voisin (15 à
  30 % de part et d'autre), fenêtres de 300 ms, roll et pitch, dans le gyro non filtré et le gyro filtré.
  Poids juste suffisant (`motors.rpm_weight_needed`) = 1 − 10^(−(hauteur + marge)/20).
- **Corpus** (680 sessions, filtre RPM actif ; hauteur avant filtres p10 / p50 / p90) : 1re harmonique
  3,8 / 17,3 / 32,3 dB, 2e −1,3 / 1,7 / 9,7 dB, 3e 0,8 / 7,1 / 19,1 dB : la 2e est presque invisible, comme
  l'écrit [S27]. Poids de la 2e laissé à 100 dans 33 % des sessions, de la 3e dans 57 %.
- **Contrôle** : sur les axes où le pilote a déjà baissé un poids (760 pour la 2e harmonique, poids médian
  0,50 ; 396 pour la 3e, 0,80), la raie restante mesurée suit la prévision du firmware à 1 dB près
  (médiane −0,8 et −1,0 dB) et reste invisible dans 97 à 99 % des cas.
- **Taux** (poids proposé si le gain est d'au moins 0,5, raie restée invisible, bruit moteur bien
  éliminé) : marge 6 dB → 6 % des sessions (9 % des drones) ; sans la condition sur le bruit moteur, 8 % ;
  gain ≥ 0,3 → 26 % ; marge 3 dB, gain ≥ 0,3 → 45 %. Monter Q comme constat à part (aucune raie visible
  après filtres) : 49 à 66 % des sessions, donc seulement en action du conseil sur le retard. Raie encore
  visible avec un poids < 100 : 3 % des sessions.

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
- **Bruit** (80 Hz à min(500 Hz ; 0,95 × Nyquist), valeur maximale du roll et du pitch) : gyro filtré
  > p90 de la taille ; terme D > p90 de la taille **ou** sommet de son spectre au-dessus de −10 dB sur
  l'échelle de PIDtoolbox (voir « Repères de PIDtoolbox ») ; 1re harmonique moteur restante > p90
  (seulement avec eRPM, gyro non filtré et filtre RPM actif). Sans constat, une ligne « Bruit du gyro
  filtré » (ok sous −30 dB PIDtoolbox, info au-dessus) et « Bruit du terme D » (ok) affiche le sommet sur
  l'échelle de PIDtoolbox.
- **Filtrage trop prudent** : retard gyro > repère de la taille alors que gyro et D sont peu bruités
  (action en plus : `rpm_filter_q` + 250, par pas jusqu'à 1000, quand le filtre RPM est actif) ; retard D > repère
  alors que le D est peu bruité. Retard gyro au-dessus du repère avec du bruit : info. Les textes et les
  lignes « ok » rappellent l'objectif du graphique de Brian White pour la taille (2–3,5" lu à 3", 8"+ à
  8" ; pas d'objectif pour les whoops, sous 2").
- **Filtre RPM** (eRPM, gyro non filtré et filtre RPM actif) : raie moteur encore visible (> 3 dB au-dessus
  du bruit voisin après filtrage) alors que son poids est < 100 → remonter le poids ; en 4.5 et plus, si le
  bruit moteur est bien éliminé, poids de la 2e ou de la 3e harmonique plus fort que nécessaire d'au moins
  0,5 → `rpm_filter_weights` proposés (voir « Filtre RPM : poids et Q »).
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
| Bruit gyro, terme D, 20–80 Hz, harmonique moteur | p90 de la taille | Corpus | Relatif au corpus : « plus bruité que 90 % », pas « mauvais » dans l'absolu |
| Terme D, échelle de PIDtoolbox | sommet > −10 dB (attention, avec le p90) ; idéal < −20 dB (texte) | Communauté | Brian White [S20] (« maybe minus 10 would be the highest ») ; 18 % des sessions (voir « Repères de PIDtoolbox ») |
| Gyro filtré, échelle de PIDtoolbox | sommet < −30 dB (affiché) | Communauté | « a good rule of thumb » [S21, S22] ; ne déclenche rien (35 % des sessions au-dessus) |
| Passage à l'échelle de PIDtoolbox | −6,5 dB | Corpus | 4 sessions de 3 drones avec captures PIDtoolbox, ± 1 dB (`validation.md`) |
| Retards des filtres gyro et D | p90 de la taille, arrondi vers le haut | Corpus | Déclenche le conseil. Objectif affiché : graphique de Brian White [S26], 1 ms à 2" puis 0,25 ms (gyro) et 0,17 ms (D) par pouce (5" : 1,75 / 1,5 ms) ; dans la vidéo [S20] il citait de mémoire 1,5 ms pour un 5" et ≈ 5 ms pour un 10". Objectif D en valeurs PIDtoolbox, environ 0,5 ms sous notre mesure |
| Taille pour l'objectif de retard | 2–3,5" → 3" ; 8"+ → 8" | Estimation | Le graphique commence à 2" : pas d'objectif pour les whoops |
| `rpm_filter_q` pour gagner du retard | + 250 par pas, jusqu'à 1000 | Firmware (doc) | « stepwise increasing the Q from the default of 500 up to a max of 1000 » [S27] ; Brian White : 750 [S20, 34:12]. Pas de 250 : estimation. Proposé seulement dans le conseil « filtrage gyro trop prudent » (bruit faible) |
| Poids RPM juste suffisant | 1 − 10^(−(hauteur + 6 dB)/20), arrondi au dixième supérieur | Firmware + estimation (marge) | Profondeur du notch tirée du code (`rpmNotchUpdate`) ; marge de 6 dB choisie pour un taux de 6 % (voir « Filtre RPM : poids et Q ») ; seulement 2e et 3e harmoniques, comme [S27] ; presets officiels en tripale : 100, 0, 80 (AOS), 100, 20, 100 (Karatebrot), 90, 30, 90 (SupaflyFPV) [S28] |
| Gain minimal pour proposer un poids | 0,5 | Estimation | Choisi avec Cédric sur le taux de déclenchement |
| Raie moteur « visible » | > 3 dB au-dessus du bruit voisin | Estimation | Sans raie, la mesure vaut environ 0 dB (± 1 dB) |
| Latence (repères bas et haut) | p10 arrondi vers le bas, p90 vers le haut | Corpus | |
| Seuils des profils whoop et 2–3,5" | voir « Calibration » | Corpus (provisoire) | 10 et 8 drones seulement |
| Seuils du profil 8"+ | voir « Calibration » | Estimation | 6 drones, 20 sessions |
| Résonance | raie > 7,3 à 7,9 dB selon la taille ; 60–900 Hz, ≥ 8 tranches de gaz, à plus de ±8 % des harmoniques moteur | Corpus (seuil) + estimation (méthode) | |
| Déséquilibre des moteurs | > 11 à 14,1 % selon la taille | Corpus | L'interprétation (côté ou diagonale au-delà de 12 %) reste estimée |
| Terme I décentré | moyenne > 29 à 33 (unités PID) selon la taille | Corpus | |
| Saturation des moteurs | ≥ 98 % pendant > 1,5 % du temps | Corpus | La notion de « moteur à 98 % » est estimée : le firmware ne journalise pas de saturation (il renormalise le mixage quand l'écart entre moteurs dépasse 100 %) |
| Pas des curseurs conseillés | 0,1 (0,1 à 0,2 pour les filtres) | Estimation | Le configurateur avance par 0,05 ; en mode de base il borne PI, D et FF à 0,70–1,40, le filtre gyro à 0,50–1,50 et le filtre D à 0,80–1,20 |
| `rpm_filter_q` conseillé | 500 → 350 | Firmware (500) + estimation (350) | Défaut 500, plage 250–3000. Les sources conseillent plutôt de **monter** Q (jusqu'à 750 [S9] ou 1000 [S6, S14] ; 750–1000 sur un 5" propre selon Brian White [S20]) et de redescendre seulement si le bruit moteur réapparaît, ce qui est le cas où la règle le propose (bruit moteur restant après filtrage) ; aucune ne donne de valeur sous 500, d'où 350 estimé. Monter Q : voir la ligne « pour gagner du retard ». Avec un poids < 100, la règle « poids trop bas » propose d'abord de le remonter |
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
