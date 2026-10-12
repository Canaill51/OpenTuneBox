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

Sans eRPM : 1S → whoop ; sinon par le nom du drone : « whoop », « Meteor », ou le nombre entier 65 ou 75
(« Air65 », pas « 1750 ») → whoop ; une taille en pouces, à un ou deux chiffres (« 5" », « 3.5in », « 7 pouces »,
« 10in ») : < 3,8 → 2–3,5", < 5,8 → 5", < 7,8 → 7", au-delà 8"+. À défaut : profil 5"
(`tests/test_advisor_rules.py`). Le profil peut être forcé dans la fenêtre de l'assistant ou avec `opentunebox report --profil`.

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

- **par session** (seuils de bruit et de latence des 2–3,5", 5" et 7" ; retards des filtres de toutes les
  tailles, sur les seuls vols avec mouvements) ;
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
| Retard filtres gyro, vols avec mouvements (ms) | 1,13 / 1,25 / 1,46 | 1,01 / 1,31 / 1,71 | 0,97 / 1,34 / 1,85 | 1,28 / 1,85 / 2,50 | 0,79 / 2,26 / 3,50 |
| Retard filtres D, vols avec mouvements (ms) | 1,74 / 2,10 / 2,46 | 1,67 / 2,31 / 2,56 | 1,89 / 2,27 / 2,67 | 2,35 / 2,55 / 2,72 | 1,77 / 2,51 / 3,04 |

Unités : densité spectrale de puissance en dB de (°/s)²/Hz pour le gyro, d'unités PID²/Hz pour le D. Pic
de sensibilité et marge de phase : boucle ouverte mesurée (voir « Boucle ouverte mesurée ») ; un pic négatif
vient d'une bande mesurable trop courte (minorant). Retards : 573 vols avec mouvements sur 681 (consigne
au-delà de 50 °/s au moins 5 % du temps, voir « Vol avec mouvements » dans « Origine des seuils ») ; 101,
95, 246, 117 et 14 sessions de la gauche vers la droite (243 et 115 pour le D en 5" et 7").

Seuils retenus dans `advisor/profiles.py` :

- **2–3,5", 5" et 7"** (recalculés le 2026-10-05) : bruit = p90 par session arrondi au 0,1 dB ; latence =
  p10 arrondi vers le bas et p90 vers le haut ; retards = p90 arrondi au dixième supérieur.
  2–3,5" : gyro −29,4 dB, D −14,8 dB, 20–80 Hz −13,9 dB, harmonique −32,5 dB, latence 10–21 ms, retards
  1,8 / 2,6 ms (provisoire : 20 drones, à la limite). 5" : −28,3 / −13,1 / −15,7 / −28,4 dB, 10–22 ms,
  1,9 / 2,7 ms. 7" : −31,4 / −17,5 / −21,7 / −31,8 dB, 15–24 ms, 2,6 / 2,8 ms.
- **Whoop** (provisoire, 11 drones) : inchangé depuis la calibration précédente (10 drones, une voix par
  drone) — gyro −28,0 dB, D −14,7 dB, 20–80 Hz −13,9 dB, harmonique −32,2 dB, latence 13–20 ms. Un seul
  drone ajouté (un whoop 1S) déplaçait le seuil gyro de 4 dB. Retards recalculés (voir ci-dessous) :
  1,5 / 2,5 ms.
- **8"+** (provisoire, 6 drones, 20 sessions) : seuils estimés à la main, inchangés (gyro −31 dB, D −17 dB,
  20–80 Hz −22 dB, harmonique −32 dB, latence 18–30 ms) ; retards recalculés : 3,5 / 3,1 ms.
- **Retards des filtres, toutes tailles** (recalculés le 2026-10-05, décision de Cédric) : p90 par session
  des seuls vols avec mouvements, arrondi au dixième supérieur — whoop 1,5 / 2,5 ms, 2–3,5" 1,8 / 2,6,
  5" 1,9 / 2,7, 7" 2,6 / 2,8, 8"+ 3,5 / 3,1 (gyro / D). Avec tous les vols, stationnaires compris, les
  repères étaient montés (5" : 2,1 / 2,8 ms) et ne signalaient plus que 2 à 4 % des vols avec mouvements
  en 2–5" au lieu d'environ 10 %. Effet sur le corpus : « Filtrage gyro peut-être trop prudent » 4 → 6 %
  des sessions (8 % des vols avec mouvements), « Filtrage du D peut-être trop fort » 2 → 4 %. Whoop : surtout les sessions d'un même AIR65 (une voix par drone : 2,1 /
  3,3 ms) ; 8"+ : 5 drones, provisoire.

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

- **Échelle des spectres** : la densité spectrale calculée est 6,5 dB plus haute que celle de PIDtoolbox
  (`spectral.PTB_PSD_OFFSET_DB`, `validation.md`) ; l'interface affiche l'échelle de PIDtoolbox, mais les
  niveaux du tableau de calibration ci-dessus sont sur la densité calculée (6,5 dB au-dessus de l'affichage). La mesure est le sommet du spectre lissé 80–500 Hz
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
- **Raie « visible »** (`rules.HARMONIC_VISIBLE_DB`, calibration du 2026-10-12, 704 sessions) : même mesure
  prise là où il n'y a aucune raie, à 0,45 et 0,6 × la fréquence moteur (gyro filtré, maximum du roll et
  du pitch). Médiane −0,2 dB, 99e percentile 3,4 à 4,0 dB ; elle dépasse 3 dB dans 1,4 à 2,8 % des
  sessions, 4 dB dans 0,3 à 0,9 %. Sur du bruit blanc simulé, elle reste sous 1 dB : l'écart vient de la
  forme des vrais spectres. Harmoniques réelles au-dessus de 4 dB après filtres : 9,1 % (1re), 2,9 % (2e),
  4,5 % (3e). Seuil passé de 3 à 4 dB (accord de Cédric) : à 3 dB, la 2e harmonique « visible » (3,7 %)
  l'était à peine plus souvent qu'une fréquence sans raie. Effet sur les taux : « Raie moteur encore
  visible après le filtre RPM » 9 → 6 %, « Raie à X Hz encore visible après filtrage » 57 → 48 vols
  stationnaires sur 115, « Q trop haut » 4 → 3 %, « Poids trop bas » 3 → 2 %, « Q peut monter » 36 → 38 %.

### Notch dynamique (2026-10-05)

Notes de réglage Betaflight 4.3, § « Multi dynamic notch » [S19] : englober chaque résonance avec 20 Hz de
marge de chaque côté, garder le minimum « as high as practical (eg 200hz) » et le moins de notchs possible
(« often only one ») pour limiter le retard ; sans filtre RPM, 3 à 4 notchs et Q 120–150 ; une résonance
fixe forte se traite mieux par un notch statique. Code (`flight/dyn_notch_filter.c`) : SDFT de 72 points
(cases d'environ 18,5 Hz), les N plus grands pics de la plage sont suivis s'ils dépassent 2 fois la
puissance moyenne (bruit estimé au-dessus de 60 Hz) ; un notch sans pic garde sa dernière fréquence et
filtre toujours ; désactivé sous 2 kHz de boucle PID.

- **Mesure** (`motors.dyn_notch_targets`, `rules.dyn_notch_view`) : spectre du gyro non filtré, bandes des
  harmoniques traitées par le filtre RPM retirées fenêtre par fenêtre (et leur image repliée si le log est
  lent), cases de 18,5 Hz, plage réglée élargie de deux cases de chaque côté, plus la raie fixe signalée.
- **Corpus** (681 sessions) : toutes à 2 kHz de boucle PID ou plus, une seule sans filtre RPM ; 1 à 3
  notchs (2 le plus souvent), Q 300–500, plage 100–600 Hz le plus souvent (minimum de 60 à 240 Hz).
- **Taux** : « moins de notchs suffiraient » 27 % des sessions (16 % des drones) — d'où le niveau info ;
  raie fixe hors de la plage 1 % ; relèvement de `dyn_notch_min_hz` dans le conseil sur le retard 2 %.
  La règle « sans filtre RPM » n'a pas été retenue (une session).

### Stratégie de filtrage et vol stationnaire (2026-10-05)

Demande de Cédric : les constats de filtrage doivent déboucher sur une stratégie (filtre RPM, notch
dynamique, passe-bas du gyro et du D), et le vol stationnaire, qui ne permet pas de juger le retard, est le
meilleur log pour identifier le bruit.

- **Stratégie** (`rules.filter_strategy`, constat en tête) : étapes dans l'ordre enregistrement → à corriger
  d'abord (repliement, D sous-filtré, bruit trop fort) → mécanique → filtre RPM → notch dynamique → passe-bas
  du gyro → passe-bas du D, avec pour chaque étape l'action qui change un réglage ; un changement à la fois,
  vol d'essai, température des moteurs [S30]. Affichée **à partir de deux étapes** (décision de Cédric du
  2026-10-05) : avec une seule étape, le constat seul dit déjà quoi faire. Dès une étape, elle apparaissait
  dans 79 % des sessions (92 % des vols stationnaires), souvent pour la seule fréquence d'enregistrement ;
  à partir de deux, dans 37 % (34 % avec mouvements, 50 % en stationnaire), repères de retard recalculés
  compris ; 25 % sans compter l'enregistrement, option écartée (`scripts/advisor_rates.py`).
- **Vol stationnaire** (`motors.spectrum_lines`) : raies moteur repérées par l'eRPM et retirées jusqu'à la
  8e harmonique (± 5 % de l'harmonique pour les trois premières, ± 5 % de la fréquence moteur ensuite,
  images repliées comprises : sur un whoop logué à 4 kHz, la 8e harmonique retombe vers 200 Hz) ; toute
  autre raie dépassant le bruit voisin de 6 dB est listée, avec sa hauteur avant et après filtres.
  « Raie à X Hz encore visible après filtrage » (plus de 3 dB après filtres, 4 dB depuis le 2026-10-12 : 48
  vols stationnaires sur 115) : 53 vols stationnaires sur 107, raies surtout entre 85 et 510 Hz (médiane 233 Hz), 11 sur 75 sous 100 Hz (mise en garde : un notch y
  retarde le PID, la raie peut venir du réglage).
- **Seuil de 6 dB** (`rules.HOVER_LINE_MIN_DB`, vérifié le 2026-10-12, 115 vols stationnaires) : du bruit sans
  raie, blanc ou coloré, simulé avec la durée (17 à 56 s) et la fréquence de log de chaque vol, ne donne jamais
  de pic de 3 dB : le spectre moyen est trop lisse. Le seuil ne sert donc pas à écarter de fausses raies, il
  fixe la hauteur qui mérite un conseil. Estimation de l'effet : 4 dB → 3,6 raies listées par vol, 71 vols
  avec conseil, 41 raies conseillées sous 100 Hz ; 6 dB → 2,0, 56, 17 ; 8 dB → 1,1, 42, 10. 6 dB gardé
  (choix de Cédric) : plus bas, on ajoute surtout des raies sous 100 Hz, où filtrer retarde le PID. Couper le
  vol en deux ne départage pas les seuils : même à 10 dB, 18 % des raies d'une moitié manquent dans
  l'autre, car le spectre change pendant le vol (régime, batterie).
- **Filtre RPM** : harmonique visible hors du filtre RPM → `rpm_filter_harmonics` (4 sessions, souvent un
  whoop réglé sur 1 harmonique) ; harmonique visible malgré un poids de 100 → `motor_poles`, Q −30 % (60).
- **Log trop lent** : sous le minimum, aucun allègement (poids RPM, LPF1 du gyro, nombre de notchs, raies
  résiduelles, relèvement du D) : le spectre est replié.

### Passe-bas du gyro et du D (2026-10-05)

Sources : aides du configurateur Betaflight [S29] (LPF1 du gyro « can usually be disabled completely […]
when RPM filtering is used » ; LPF2 du gyro à garder quand la boucle PID est plus lente que le gyro ; pour
le D « at least two PT1 lowpass filters, or a single second order filter, are required » ; LPF1 du D
statique « not recommended ») ; notes de réglage 4.2 [S12] et 4.3 [S19] (LPF2 du gyro « absolutely
essential » à 2 kHz, recommandé à 4 kHz, réglable ou coupé à 8 kHz ; biquad remplacé par le PT2) ; guide
du filtrage RPM [S30] (drone propre : relever ou couper les passe-bas du gyro, un réglage à la fois, en
surveillant la température des moteurs). Code : les curseurs multiplient les coupures par défaut (gyro
LPF1 250–500 Hz, LPF2 500 Hz ; D LPF1 75–150 Hz, LPF2 150 Hz) et ne rallument jamais un filtre coupé
(`config/simplified_tuning.c`) ; coupure plafonnée à 1000 Hz.

- **Corpus** (681 sessions) : LPF1 du gyro déjà coupé dans 86 % des sessions, LPF2 du gyro presque
  toujours en PT1 ; boucle PID / gyro 3,2 / 3,2 kHz (64), 4 / 8 kHz (508), 8 / 8 kHz (109) ; LPF2 du gyro
  coupé à 4 kHz : 6 sessions ; D d'ordre 1 : 14 sessions (2 %) ; LPF1 du D statique : 83 (12 %) ; aucun
  biquad sur le gyro.
- **Simulateur** (`rules.predict_filter_change`, spectre « et si » de `bf_filters`, variation ajoutée aux
  mesures des règles) : couper le LPF1 du gyro (92 sessions) fait gagner 0,47 ms (médiane) pour +3,4 dB de
  gyro et +3,1 dB de D ; le bruit reste sous les repères (gyro et D sous le p90, sommet du D sous −10 dB
  PIDtoolbox) dans 55 cas (8 % du corpus), 30 (4 %) en ne gardant que les vols avec mouvements. Relever les filtres du gyro de 25 % ne fait gagner que 0,06 ms
  (règle écartée). Relever ceux du D de 25 % : 0,48 ms pour +2,6 dB, sous les repères dans 60 % des
  sessions, d'où la prévision ajoutée au seul conseil « filtrage du D peut-être trop fort » (34 sessions
  candidates).

### Pic de réponse indicielle (2026-10-05)

Les anciens paliers (conseil > 1,10, attention > 1,20, objectif 1,02–1,08) étaient des estimations sans
source. Ils signalaient 46 % des sessions du corpus (23 % en attention). Mesures sur 681 sessions, axes roll
et pitch, une voix par drone (médiane de ses axes) :

| Pic de réponse | Axes | Drones | p50 | p75 | p90 | p95 |
|---|---|---|---|---|---|---|
| Sans feedforward (F = 0) | 492 | 40 | 1,06 | 1,10 | 1,15 | 1,20 (axes : 1,25) |
| Avec feedforward (F > 0) | 761 | 76 | 1,12 | 1,22 | 1,42 | 1,60 (axes : 1,55) |

- **Le feedforward gonfle le pic** (mesuré ici ; aussi dit par Brian White dans une vidéo de questions-réponses,
  non citée faute de citation vérifiée) : p90 par session 1,18 sans, 1,38 avec. Un même
  palier pour les deux signalerait surtout les drones avec feedforward.
- **Paliers communs à toutes les tailles** : moins de 20 drones par taille dans chaque groupe (sauf le 5"
  avec feedforward), percentiles par taille instables. Par session, sans feedforward : 5" p90 1,16, 7"
  1,11, 2–3,5" 1,28.
- **Paliers retenus** (décision de Cédric) : conseil = p90 par drone, attention = p95 par axe, arrondis :
  sans feedforward 1,15 / 1,25, avec feedforward 1,40 / 1,55 (`rules.STEP_PEAK_HIGH`,
  `rules.STEP_PEAK_HIGH_FF`). Sessions signalées : 13 % (`advisor_rates.py`, au moins un constat de
  dépassement ; environ 7 % en attention) ; 5" 12 %, 7" 7 %, 2–3,5" 23 %,
  whoop 18 %, 8"+ 21 %. Le corpus est fait surtout de demandes d'aide : ces paliers sont probablement
  indulgents (Damping 1.0 de BlackSummer, pic 1,14, n'est plus signalé).
- **Repère sans dépassement** : série BlackSummer (`1. PTB/BlackSummer`, sans feedforward, seul le D change),
  méthode de Brian White (garder la première valeur de Damping sans dépassement [S24, S25]) : pic roll 1,25 /
  1,14 / 1,03 / 1,03 / 1,02 (D croissant ; 1,23 / 1,11 / 1,00 / 0,98 dans PIDtoolbox, `validation.md`) ;
  la série « Master multi » faite ensuite avec ce Damping donne 1,02 à 1,03. « Sans dépassement visible »
  ≈ **1,03** sur notre échelle (`rules.STEP_PEAK_FLAT`), un seul drone : repère indicatif, affiché dans les
  textes, sans seuil.
- **Réponse molle** (2026-10-12, 705 sessions) : pic sous le p10 des axes (1,008 sans feedforward, 1,012 avec ;
  par drone 1,015 et 1,028) **et** latence au-delà du p90 de la taille. Palier 1,01 (`rules.STEP_PEAK_SOFT`,
  choix de Cédric) au lieu de 0,98, une estimation qui tombait vers le 3e percentile : 9 sessions signalées
  (1,3 %) au lieu de 3 ; 1,03 (`STEP_PEAK_FLAT`) en aurait signalé 15. C'est la latence qui limite le constat.
- **Nombre de fenêtres** (2026-10-12, 646 axes d'au moins 400 fenêtres) : tranches de n fenêtres consécutives
  comparées au vol entier. Écart du pic médiane / p90 : 0,04 / 0,20 à 20 fenêtres, 0,03 / 0,14 à 50,
  0,02 / 0,11 à 100, 0,015 / 0,07 à 200 ; latence p90 3,6 / 2,7 / 2,0 / 1,4 ms. L'écart compte aussi les
  changements de pilotage au cours du vol : c'est un majorant. À 20 fenêtres, le pic peut se tromper de plus
  que l'écart entre les paliers conseil et attention (0,10) : le vol reste lu dès 20 fenêtres
  (`rules.STEP_MIN_WINDOWS`), mais sous 200 (`rules.STEP_SURE_WINDOWS`, consigne de Brian White [S35]) les
  constats du pic et de la latence disent « mesure moins sûre » (choix de Cédric). Dans le corpus, 46 sessions
  ont moins de 20 fenêtres sur les deux axes, 61 (9 %) entre 20 et 199, 598 au moins 200.

### Diagnostic du réglage par séries (2026-10-10)

« Diagnostic du réglage » de la Réponse indicielle, toujours affiché (`advisor/step_tuning.py`), méthode de Brian White
(`spec-methodes.md`, [S32]). Séries reconnues dans tout le corpus (`analysis/step_series.py:find_series` :
mêmes filtres, un seul curseur changé) : 28 séries Master, 43 Damping, 6 Pitch Tracking / Pitch Damping Le choix final
des pilotes est relu dans les vols faits ensuite (« FINAL », « FF », « tune »…), faute d'étiquette.

| Série Master | Choix du pilote | Diagnostic |
|---|---|---|
| Air65 (Cédric) 1,0 / 1,2 | 1,0 | 1,0 (cassure 0,080 à 1,2) ; puis roll / pitch +2,1 ms → Pitch Tracking +0,2, ce que Cédric a fait (MM2) |
| JeNo 5 (Cédric) 1,0 → 1,4 | 1,3 | 1,4 proche de l'optimum (début de cassure 0,064) |
| Gerrykiki (Cédric) 1,2 → 1,6 | 1,6 | 1,6 proche de l'optimum |
| Meteor 75 03112025 (Cédric) 1,2 / 1,4 | 1,6 | 1,4 proche de l'optimum, voler à 1,6 |
| Meteor 75 48k (Cédric) 1,0 → 1,4 | 1,4 | 1,4 proche de l'optimum |
| Valkyrie 2 (Jérôme) 1,0 → 1,8 | 1,6 | 1,8 proche de l'optimum |
| Davy 1,2 → 1,6 | 1,4 | 1,2 (cassure 0,066 à 1,4) |
| Guigui 1,2 → 1,6 | 1,4 | 1,6 proche de l'optimum |
| Sasha (Amax) 1,0 → 1,8 | 1,6 | 1,6 (cassure 0,067 à 1,8) |
| dcodemonster (PTB) 0,6 → 1,6 | 1,15 | 0,8, affinage 0,9 (cassure 0,065 à 1,0, 0,11 à 1,2) |

Le choix du pilote ou la valeur d'à côté (un cran de 0,2) dans 9 cas sur 10. La cassure sépare mal les cas
limites : gardés jusqu'à 0,067, rejetés dès 0,064 ; d'où trois états (propre, premiers signes, trop haut)
plutôt qu'un seul seuil.

**Vagues** (2026-10-12, `step_tuning.WAVES_RATIO_HIGH` = 2,5 fois le bruit de mesure). Aucun pilote du corpus
n'est allé jusqu'à l'oscillation : le seuil vient d'une mesure à vide et d'essais par injection.
- Mesure à vide, vols sans feedforward (516 axes, 48 drones) : rapport médian 1,20, p90 1,69, p99 2,35,
  maximum 3,12 ; 2,5 signale 0,6 % des axes, 2,0 en signalerait 4,3 % (un drone sur quatre au moins une fois).
  Avec feedforward, le dépassement gonfle les vagues (p99 5,4) : le diagnostic demande des vols sans.
- Les 34 séries Master du corpus : vagues au plus 2,4 (VX35 à 1,6, Seeker 3 à 1,4) ; 2,5 ou 3,0 ne changent
  aucun conseil, 2,0 en changerait un (Seeker 3 : 1,2 au lieu de 1,4, sans autre signe).
- Injection d'une vague d'amplitude connue (mode à 30, 45 ou 60 Hz, consigne réelle, bruit réel du vol) sur
  8 vols : au-delà de 250 fenêtres, une vague d'environ 3 % de la consigne au début du palier atteint 2,5
  (3 à 5 % à 60 Hz) ; entre 75 et 150 fenêtres, même 10 % donne 1,6 à 2,6 ; à 25 fenêtres, rien.
- Sous 200 fenêtres (`STEP_SURE_WINDOWS`), le bruit seul donne déjà environ 2 (médiane 2,1 sous 40
  fenêtres) : les vagues d'un axe ne sont pas lues (« vagues non lues »), choix de Cédric. Dans les séries
  Master du corpus, seuls 2 vols de Davy sont concernés (178 et 191 fenêtres, vagues 1,3) : aucun conseil
  ne change.
- Limite : l'incertitude ne couvre que le bruit d'un vol ; deux vols du VX35 à Master 1,4 donnent 2,0 et 3,1
  pour une ondulation de 0,35 % (comme la cassure, L-012).

Séries Damping (choix connu pour 9 drones), jugées sur le **roll** (pic arrondi à 0,01, la valeur lue à
l'écran) : Damping règle le D des deux axes, et un pitch qui dépasse encore se corrige par Pitch Damping
(remarque de Cédric sur l'Air65, 2026-10-10 : roll sans dépassement à 1,2, pitch 1,08 ; il a gardé
Damping 1,2 et monté Pitch Damping à 1,2). La première valeur au pic roll ≤ 1,05, ou au-delà de laquelle il
baisse de moins de 0,02, donne le choix du pilote pour 6 drones (Air65, Gerrykiki, Meteor 1,2 et 1,35,
Guigui, BlackSummer), à 0,1 près pour 2 (JeNo 5 1,0 pour 1,1, Davy 1,0 pour 1,05) ; écarts : Valkyrie 2
(1,4 pour 1,2), dcodemonster (0,8 et Pitch Damping +0,2, pour 1,0). Jugée sur le plus fort des deux axes, la
règle conseillait 1,4 sur l'Air65.

Écart de latence pitch − roll : Air65 MM2 (Pitch Tracking 1,4) 0,6 ms, BlackSummer 1,9 à 2,0 ms, Sohail 1,6 ms ;
le plus grand des séries, Seeker 3 (st7ven3241), 3,8 ms.

### Trous dans l'enregistrement (2026-10-12)

Une pause d'écriture de la carte SD ou de la mémoire flash laisse un trou dans le log (pas de temps de plus
de 4 périodes, `FlightLog.recording_gaps`, mêmes trous que `FlightLog.segments`). 7,5 % des sessions du
corpus en ont au moins un (53 sur 710), presque toutes des `.BFL`.

- L'ancien constat comptait les trames corrompues (`stats["corrupt_frames"]` > 20) : ce compteur compte les
  tentatives de resynchronisation du décodeur, pas le temps perdu. Il manquait des vols troués (Tes : 3,7 %
  perdus pour 9 tentatives ; InertiaFPV : 2,7 % pour 0) et en exagérait d'autres (st7ven3241 : 4 842 pour
  1,2 %).
- Jusqu'à D-020, les analyses recollaient le vol de part et d'autre d'un trou (W-010) : chaque raccord est un
  saut du signal, donc un faux bruit. Sur les vrais vols troués, bruit du gyro mesuré normalement moins bruit mesuré en
  écartant les fenêtres qui chevauchent un trou : BlackSummer Wobble (7,9 % perdus) +18 dB entre 80 et
  500 Hz, +26 dB entre 20 et 80 Hz ; st7ven3241 D16 (1,2 %) +8 / +13 dB ; Tes mm 1.4 (3,5 %) +6 / +10 dB ;
  Zox WOBBLE (0,9 %) +4 / +12 dB ; vols stationnaires troués : moins de 1 dB / 1 à 2 dB. L'erreur dépend des
  mouvements plus que du temps perdu.
- Réponse indicielle (8 vols propres, morceaux de 50 ms retirés, p90 des écarts) : 1 % perdu, pic ±0,04 et
  latence ±2,7 ms ; 4 %, ±0,12 et ±13 ms.
- Le constat « Trous dans l'enregistrement » s'affiche dès un trou dans la période analysée, avec le nombre
  de trous et le temps perdu : 7 % des sessions du corpus (6 % des vols avec mouvements, 10 % des vols
  stationnaires). D'abord en conseil (« bruit surestimé »), il est redevenu informatif avec D-020 : les
  fenêtres qui chevauchent un trou sont écartées. Effet sur le corpus : aucune session sans trou ne change ;
  31 des 47 sessions trouées perdent de faux constats (13 « Oscillations basses fréquences », 7 conseils du
  notch dynamique, 5 « Gyro filtré bruité », 3 dépassements) ; 4 vols très hachés (un trou par seconde
  environ) n'ont plus assez de fenêtres de 2 s pour la réponse indicielle.

## Règles

Le diagnostic du bruit de la fenêtre Spectre (`advisor/noise.py`) reprend ces mêmes constats, rangés par
début de titre en quatre parties (conditions du log, gyro, terme D, stratégie) ; il n'a aucun seuil à lui.
Quand l'assistant n'écrit pas de stratégie (moins de deux étapes), il indique le seul réglage de filtre à
faire, ou qu'aucun n'est conseillé. Les constats du gyro sont répartis en blocs (filtre RPM, notch
dynamique, passe-bas ; étapes de `rules.FILTER_STAGES`) ; un bloc sans constat à appliquer reçoit un
verdict qui reprend les repères existants (harmonique effacée à 4 dB au plus, gain de poids 0,5, marge
6 dB) et dit quand l'assistant proposerait un changement (Q du filtre RPM, `dyn_notch_min_hz` : vol avec
mouvements, retard au-dessus du repère, bruit faible). Seules valeurs ajoutées, toutes du firmware :
`rpm_filter_harmonics = 3` proposé si le filtre RPM est coupé malgré l'eRPM (défaut de Betaflight,
`pg/rpm_filter.c`), `blackbox_disable_rpm = OFF` si `dshot_bidir` est actif sans eRPM enregistré. Un nouveau constat de bruit ou de filtre doit avoir sa place dans
`noise.GYRO_TITLES` ou `noise.DTERM_TITLES` (vérifié par `tests/test_advisor.py`).

Le diagnostic du réglage de la Réponse indicielle (`advisor/step_tuning.py`) a ses propres seuils, tous
dans ce module (tableau ci-dessous) ; il réutilise seulement les niveaux de gravité et la mise en forme des
constats.

Niveaux : **attention** (à corriger), **conseil** (amélioration), **info**. Les règles de réponse et de bruit
portent sur le roll et le pitch (le yaw n'est pas jugé).

- **Enregistrement** : deux minimums, selon le réglage. **PID** : 1 kHz (plus petite fraction 1/2^n de la
  boucle PID qui l'atteint) ; en dessous, « Fréquence d'enregistrement trop basse » (attention). **Filtres** :
  2 kHz pour un gyro à 8 kHz, 1,6 kHz pour un gyro à 3,2 kHz (plus petite fraction 1/2^n de la boucle PID qui
  atteint 1,6 kHz) ; entre les deux, « Fréquence d'enregistrement trop basse pour les filtres » (conseil : un
  vol de série PID n'a pas besoin de plus). Sous le minimum des filtres, aucun allègement de filtre n'est
  proposé (spectre replié). 3e harmonique moteur au-delà de la
  moitié de la fréquence du log (conseil, `blackbox_sample_rate` pour 6 × la fréquence moteur) ; gyro non filtré absent ; pas de télémétrie RPM ; trous dans l'enregistrement dans la période analysée
  (info, voir « Trous dans l'enregistrement »).
- **Vol** : crash détecté, la période d'analyse s'arrête avant (info).
- **Réponse** (au moins 20 fenêtres par axe, sinon « Pas assez de mouvements de manche » ; sous 200, les
  constats du pic et de la latence ajoutent « mesure moins sûre ») :
  - retard consigne → gyro roll / pitch (intercorrélation, info), avec la valeur du feedforward et le mode de
    vol : la mesure de la série Feedforward des finitions, comparée d'un vol à l'autre ;
  - dépassement : sans feedforward, pic > 1,25 (attention), > 1,15 (conseil) ; avec feedforward (F > 0),
    > 1,55 et > 1,40 ; attribué au feedforward si F > 0 et si la
    latence est plus courte que le repère bas de la taille ;
  - réponse molle : pic < 1,01 et latence au-delà du repère haut de la taille ;
  - roll et pitch différents : écart de pic > seuil de la taille (curseur « D pitch ») ;
  - stabilité (boucle ouverte, voir « Boucle ouverte mesurée ») : pic de sensibilité > 7 dB (conseil ;
    « au moins » quand le pic est au bord de la bande mesurable ; précision ajoutée si la boucle n'est
    qu'approchée) ; sinon marge de phase < 35° (conseil). Conseil : Damping +0,1 ou Master −0,1 ; seulement
    Master −0,1 quand le D domine déjà au croisement (|Ĉd/Ĉe| > 1) ; pour un whoop, explication de la
    cause habituelle (boucle rapide, D dominant, filtres du D) ; pic au bord de la bande : action de plus,
    des wobbles plus rapides et plus secs pour que la mesure aille plus loin (W-004 ; pas de vol chirp,
    D-019) ;
  - oscillations basses fréquences : gyro filtré 20–80 Hz > p90 de la taille.
- **Bruit** (80 Hz à min(500 Hz ; 0,95 × Nyquist), valeur maximale du roll et du pitch) : gyro filtré
  > p90 de la taille ; terme D > p90 de la taille **ou** sommet de son spectre au-dessus de −10 dB sur
  l'échelle de PIDtoolbox (voir « Repères de PIDtoolbox ») ; 1re harmonique moteur restante > p90
  (seulement avec eRPM, gyro non filtré et filtre RPM actif). Sans constat, une ligne « Bruit du gyro
  filtré » (ok sous −30 dB PIDtoolbox, info au-dessus) et « Bruit du terme D » (ok) affiche le sommet sur
  l'échelle de PIDtoolbox.
- **Filtrage trop prudent** : retard gyro > repère de la taille alors que gyro et D sont peu bruités
  (action en plus : `rpm_filter_q` + 100, par pas jusqu'à 1000, quand le filtre RPM est actif) ; retard D > repère
  alors que le D est peu bruité. Retard gyro au-dessus du repère avec du bruit : info. Les textes et les
  lignes « ok » rappellent l'objectif du graphique de Brian White pour la taille (2–3,5" lu à 3", 8"+ à
  8" ; pas d'objectif pour les whoops, sous 2").
- **Filtre RPM** (eRPM, gyro non filtré et filtre RPM actif) : raie moteur encore visible (> 4 dB au-dessus
  du bruit voisin après filtrage) alors que son poids est < 100 → remonter le poids ; en 4.5 et plus, si le
  bruit moteur est bien éliminé, poids de la 2e ou de la 3e harmonique plus fort que nécessaire d'au moins
  0,5 → `rpm_filter_weights` proposés (voir « Filtre RPM : poids et Q ») ; harmonique 2 ou 3 visible après
  filtrage mais hors du filtre RPM → `rpm_filter_harmonics` augmenté ; harmonique visible malgré un poids de
  100 → `motor_poles` à vérifier, `rpm_filter_q` −100 (la 1re harmonique reste à « Bruit moteur mal
  éliminé », qui propose aussi −100) ; toutes les harmoniques traitées effacées (≤ 4 dB) avec gyro et D sous
  leurs repères, log assez rapide, Q < 1000 → « Filtre RPM : Q peut monter », `rpm_filter_q` + 100 (tout
  type de vol ; pas proposé si un autre constat parle déjà de `rpm_filter_q`).
- **Notch dynamique : Q et N** (filtre RPM actif, gyro non filtré) : pour chaque pic à suivre, largeur à
  mi-puissance et hauteur au-dessus du bruit voisin sur le gyro non filtré (harmoniques traitées retirées,
  le plus large et le plus haut du roll et du pitch) ; Q nécessaire = le plus haut Q qui ramène chaque raie
  à 4 dB du bruit voisin sur sa demi-largeur (forme du notch de Betaflight [S31]), arrondi à 50 vers le bas,
  100 au moins. Q actuel plus haut → « Q trop haut pour les raies à suivre » (`dyn_notch_q` = Q nécessaire) ;
  sinon, sans raie qui passe et avec gyro et D sous leurs repères, « Q peut monter » jusqu'au Q nécessaire,
  450 au plus (+50 si aucune raie n'est mesurable). Plus de pics que de notchs avec une raie qui passe ou un
  gyro bruité → « plus de notchs » (`dyn_notch_count` = nombre de pics, 7 au plus) ; une raie qui passe
  dans la plage → `dyn_notch_count` + 1.
- **Terme D propre** (tout vol, log assez rapide, gyro et D non signalés) : sommet du D sous −20 dB
  PIDtoolbox → le simulateur essaie le curseur « filtre D » jusqu'à 1,2 (ou +0,2 au-delà ; sans curseur,
  coupures × 1,1, deux pas au plus), du plus grand pas au plus petit, et retient le premier qui garde le D
  sous −10 dB PIDtoolbox et sous le p90 de la taille. Remplace alors le conseil « filtrage du D trop
  fort » fondé sur le retard (+0,1 seulement).
- **Retards des filtres** : seulement si le vol contient des mouvements (consigne au-delà de 50 °/s au
  moins 5 % du temps) ; sinon « Retard des filtres peu représentatif » (info), sans conseil fondé sur le
  retard mesuré (« filtrage gyro trop prudent », « filtrage du D trop fort »). « Le LPF1 du gyro peut être
  coupé » reste proposé : il repose sur le bruit, et son retard vient du modèle des filtres.
- **Vol stationnaire** (voir « Stratégie de filtrage et vol stationnaire ») : « Vol stationnaire : bruit
  identifié » (info : harmoniques avant / après filtres, autres raies, bruit restant) ; « Raie à X Hz encore
  visible après filtrage » (conseil) pour chaque raie hors moteurs qui dépasse encore le bruit voisin de
  4 dB après filtres, sauf si « Raie fixe » la traite déjà.
- **Stratégie de filtrage** (conseil, en tête) : synthèse ordonnée des constats de filtrage, quand ils
  touchent au moins deux étapes.
- **Passe-bas** (voir « Passe-bas du gyro et du D ») : LPF2 du gyro coupé avec une boucle PID plus lente
  que le gyro (attention sous 3 kHz, conseil à 4 kHz) ; D filtré à l'ordre 1 seulement (attention) ; LPF1
  du D statique et biquad sur le gyro (info) ; avec le filtre RPM et un bruit sous les repères, « Le LPF1
  du gyro peut être coupé » si le simulateur prévoit un bruit encore sous les repères ; « Filtrage du D
  peut-être trop fort » donne la prévision du simulateur pour le curseur « filtre D » +0,1 et devient une
  info (sans conseil) si le D dépasserait les repères.
- **Notch dynamique** (actif : au moins un notch, boucle PID ≥ 2 kHz ; filtre RPM actif et gyro non
  filtré) : plus de notchs que de pics à suivre → « moins de notchs suffiraient » (info) ; raie fixe hors de
  la plage à 20 Hz près → plage proposée dans le constat « Raie fixe » ; `dyn_notch_min_hz` relevé dans le
  conseil « filtrage gyro trop prudent » (voir « Notch dynamique »).
- **Résonance** : raie au-dessus de la moyenne glissante voisine de plus que le seuil de la taille
  (20 Hz à min(900 Hz ; 0,9 × Nyquist), sans les 5 premiers Hz), présente dans la médiane de toutes les tranches de gaz (au moins
  8 tranches), à plus de ±8 % des harmoniques 1 à 3 du moteur.
- **Notch statique conseillé** (« Raie fixe », « Raie à X Hz encore visible après filtrage » ;
  `rules.static_notch_cutoff`, choix de Cédric du 2026-10-12) : `gyro_notch1_cutoff` tiré de la forme de la
  raie dans le gyro non filtré (largeur à mi-puissance et hauteur, la plus large et la plus haute du roll et
  du pitch), par le même calcul que le Q du notch dynamique : Q nécessaire pour ramener la raie à 4 dB du
  bruit voisin, arrondi et borné de 1 à 4,5 (100 à 450), puis coupure c telle que Q = f·c / (f² − c²)
  (`filterGetNotchQ`, `spec-betaflight.md`) : 0,62 à 0,89 × la fréquence. Repli sur 0,7 × la fréquence
  quand la raie ne se mesure pas (pas de gyro non filtré ou d'eRPM, ou raie sous 4 dB dans le spectre moyen :
  « Raie fixe » la repère par tranche de gaz) ; 0,7 est dans la plage des anciens défauts de Betaflight
  3.2.0 (gyro 400 / 300 et 200 / 100 Hz, D 260 / 160 Hz : 0,5 à 0,75). Sur 44 conseils tirés du corpus,
  20 calculés (souvent au plafond 0,89 : raies fines), 24 en repli.
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
| Taille par le nom | « whoop », « Meteor », 65 ou 75 (nombre entier) ; taille en pouces à un ou deux chiffres, bornes 3,8 / 5,8 / 7,8 | Estimation | Mis de côté par Cédric le 2026-10-12 (W-003) |
| Profil par défaut | 5" | Corpus | Taille la plus fréquente (366 sessions sur 707) |
| Fréquence du log trop basse | gyro 8 kHz : 2 kHz ; gyro 3,2 kHz : 1,6 kHz (plus petite fraction de la boucle PID ≥ 1,6 kHz) | Pilote expérimenté (Cédric, 2026-10-05) + firmware | Le blackbox sous-échantillonne sans filtre (`blackbox_sample_rate` 1/1 à 1/16 de la boucle PID, `cli/settings.c`) : tout bruit au-delà de la moitié de la fréquence du log se replie. 178 sessions sur 681 (26 %) en dessous, surtout des gyros à 8 kHz logués à 1 kHz |
| Fréquence du log pour les PID | 1 kHz (bloquant pour une série sous 500 Hz) | Pilote expérimenté (Cédric, 2026-10-12) + théorie | Réponse indicielle et latence à la milliseconde près ; la bande analysée s'arrête à 0,4 × la fréquence du log (200 Hz à 500 Hz). Les mouvements du manche restent sous quelques dizaines de hertz : le repliement du bruit moteur pèse peu sur la réponse |
| Harmoniques des petits drones | log ≥ 6 × fréquence moteur (3e harmonique sous la moitié de la fréquence du log) | Théorie (Nyquist) | 75 sessions de plus atteignent le minimum mais pas ce seuil |
| Diagnostic du réglage : pas de série | 0,2 ; plus fin que 0,15 signalé | Communauté [S32] | Brian White : un pas de 0,1 ne se lit pas sur la réponse |
| Diagnostic du réglage : curseur I | 0,2–0,3 (signalé hors de 0,15–0,35) | Communauté [S32] | Le I déforme la réponse |
| Diagnostic du réglage : fenêtres | moins de 20 : vol non lu ; moins de 200 : « mesure moins sûre » (avertissement) | Communauté [S35] + corpus | 89 % des 240 vols de série du corpus atteignent 200 (62 séries sur 77 entièrement) ; ~12 fenêtres par seconde de wobble (médiane), soit ~17 s (mesure du 2026-10-12) |
| Guide : roll et pitch mêlés | les deux axes au-delà de 20 °/s pendant au moins 40 % du temps en mouvement | Corpus (choix de Cédric, 2026-10-12) | Deux groupes dans le corpus : wobbles mêlés 0,55–0,70, vols « un axe à la fois » 0–0,25. À 0,4 : 39 % des vols avec mouvements et 22 % des fichiers nommés « wobble » signalés (0,3 : 29 % et 13 %) |
| Guide : protocole des mouvements | mode Angle ≥ 90 % ou Acro avec le feedforward à 0 (choix de Cédric, 2026-10-12 ; même réponse, [S24]) ; rates ACTUAL 200 / 200 / 0 sur les trois axes ; limite d'angle 30° (mode Angle seulement) ; Dynamic Damping à 0 | Communauté [S24, S33, S34] + choix de Cédric | Rates 200 / 200 / 0 choisies par Cédric (linéaires, comme [S33]) ; Brian White conseille 20° pour un gros drone, non retenu |
| Guide : séries | Damping 0,6 → 1,6 (jusqu'à 5") ou 1,0 → 1,6 (6" et plus) ; Master 1,0 → 2,0 ; Pitch Damping et Pitch Tracking : 3 vols dans le sens du diagnostic | Communauté [S25, S34] | Arrêt au premier vol « trop haut » |
| Guide : finitions | I 0,8–1,0 (jusqu'à 5"), 0,6–0,8 (6" et plus) | Communauté [S34, S36] + choix de Cédric | Plages de départ, arrondies autour des valeurs de Brian White |
| Série Feedforward (finitions) | pas de 0,5 (0, 0,5, 1), Acro uniquement ; retard consigne → gyro par vol, dans l'Assistant et sous les logs du guide, sans valeur conseillée | Choix de Cédric (2026-10-12) + firmware | Remplace les plages de départ 0,8–1,2 / 0,5–1,0. Pas jugée par le Diagnostic du réglage : le feedforward fausse la réponse indicielle. Retard par intercorrélation (`analysis/delay.py:xcorr_delay_ms`). Feedforward coupé en mode Angle (`pid.c`) : un vol de la série en mode Angle est signalé |
| Guide : repères de fin des filtres | gyro filtré ≤ −30 dB, terme D ≤ −10 dB (échelle PIDtoolbox, partie stationnaire) ; retard sous l'objectif de la taille (D : + 0,5 ms, écart de mesure avec PIDtoolbox) | Communauté [S20, S21, S26] | Mêmes repères que l'assistant (`rules.GYRO_PTB_MAX_DB`, `DTERM_PTB_MAX_DB`, `profiles.filter_delay_target_ms`) |
| Diagnostic du réglage : feedforward | F > 0 hors mode Angle (≥ 90 % du vol), sauf vols d'une série Feedforward | Firmware + communauté [S32] | Le feedforward est forcé à 0 en Angle (`pid.c`), pas en Horizon |
| Diagnostic du réglage : Damping | pic roll ≤ 1,05 ; ou le cran suivant baisse le pic de moins de 0,02 (pic ≤ 1,10) ; pitch au-dessus de 1,05 et de 0,02 de plus que le roll → Pitch Damping +0,2 | Communauté [S24, S25, S32] + corpus | Repère 1,03 (`rules.STEP_PEAK_FLAT`) plus le bruit d'un vol ; 9 séries de choix connu. Série Pitch Damping : même règle sur le pitch |
| Diagnostic du réglage : Master, cassure | début 0,03 ; nette 0,065 ; forte 0,10 (pas d'affinage) | Corpus (10 séries) | Gardés ≤ 0,067, rejetés ≥ 0,064 : cas limites possibles |
| Diagnostic du réglage : Master, plateau | gain de latence < 4 % par cran ; arrêt si cassure ≥ 0,05 et qui grandit de 0,02 | Corpus (10 séries) + communauté [S32] | Un drone qui progresse gagne 6 à 10 % par cran de 0,2 |
| Diagnostic du réglage : vagues | 2,5 fois le bruit de mesure ; pas lues sous 200 fenêtres | Corpus + essais | Mesure à vide p99 2,35 sans feedforward ; vague d'environ 3 % détectée au-delà de 250 fenêtres (« Vagues », 2026-10-12) |
| Diagnostic du réglage : roll / pitch | écart de latence ≤ 1 ms équilibré, ≤ 2 ms acceptable | Communauté [S32] | Pilote expérimenté (Cédric, d'après Brian White) : 1 à 2 ms selon l'inertie et les moteurs |
| Harmoniques hors de la plage visible | 3 × f moteur > Fs / 2 | Théorie | Fréquence de Nyquist |
| Gyro non filtré absent, pas d'eRPM | présence des champs | Firmware | |
| Trous dans l'enregistrement | au moins un trou (pas de temps > 4 périodes du log) dans la période analysée | Corpus | Constat informatif (fenêtres touchées écartées, D-020) ; 7 % des sessions signalées (« Trous dans l'enregistrement », 2026-10-12). Remplace le compteur de trames corrompues (> 20), qui comptait des tentatives de resynchronisation et non du temps perdu |
| Mouvements de manche suffisants | ≥ 20 fenêtres ; « mesure moins sûre » sous 200 | Corpus + communauté [S35] | Écart du pic p90 0,20 à 20 fenêtres, 0,07 à 200 (« Pic de réponse indicielle », 2026-10-12) ; mêmes valeurs que le Diagnostic du réglage |
| Fenêtre de réponse retenue | consigne ≥ 20 °/s | Communauté | Code de PID-Analyzer (2018) et de PIDtoolbox v0.55 (2022) [S5, S2] |
| Dépassement | sans feedforward : pic > 1,15 (conseil), > 1,25 (attention) ; avec feedforward : > 1,40, > 1,55 ; repère sans dépassement 1,03 | Corpus + communauté (méthode) | Conseil = p90 par drone, attention = p95 par axe, communs à toutes les tailles, séparés selon F (« Pic de réponse indicielle ») ; aucune source publique pour des paliers chiffrés [S1, S4] ; repère 1,03 = première valeur sans dépassement de la série BlackSummer (méthode de Brian White [S24, S25]) |
| Dépassement dû au feedforward | F > 0 et latence < p10 de la taille | Corpus + estimation | Repère de latence mesuré, raisonnement choisi |
| Réponse molle | pic < 1,01 et latence > p90 de la taille | Corpus | Pic : p10 des axes (« Pic de réponse indicielle », 2026-10-12) ; 1,3 % des sessions |
| Roll et pitch différents | écart de pic > 0,08 (7") à 0,12 (commun) selon la taille | Corpus | Seuils mécaniques (`profiles.py`, voir « Calibration ») |
| Stabilité | pic de sensibilité > 7 dB (conseil) | Théorie + corpus | Skogestad et Postlethwaite : Ms « inférieur à environ 2 (6 dB) » ; 7 dB (Ms ≈ 2,2) est un peu au-delà. Avec 6 dB / 40°, 33 % des sessions du corpus étaient signalées (72 % des whoops) ; avec 7 dB / 35°, 20 % (décision de Cédric, 2026-10-05) ; whoops 32 %, gardés : marge réelle (« Boucle ouverte mesurée »). Conseil seulement : la mesure s'arrête souvent sous le pic (minorant) |
| Marge de phase | < 35° (conseil) | Théorie + corpus | Åström et Murray : valeurs raisonnables 30–60° ; 35° est dans cette plage, choisi avec le corpus (voir ci-dessus). Même réserve pour la valeur approchée |
| D dominant au croisement | \|Ĉd/Ĉe\| > 1 | Corpus | Choisit le conseil (Master −0,1 plutôt que plus de D). 55 séries Damping et Pitch Damping (« D dominant », 2026-10-12) : dès 1,0, un cran de D fait monter le pic de sensibilité ; aussi la simulation (`tests/test_loop.py`) et la série BlackSummer (`validation.md`) |
| Bandes de bruit | 80–500 Hz ; 20–80 Hz | Communauté + estimation | Propwash et oscillations « ~20Hz to ~100Hz » [S1], moteurs ~200–500 Hz [S7] ou au-delà de 250 Hz [S6] ; bornes exactes choisies |
| Bruit gyro, terme D, 20–80 Hz, harmonique moteur | p90 de la taille | Corpus | Relatif au corpus : « plus bruité que 90 % », pas « mauvais » dans l'absolu |
| Terme D, échelle de PIDtoolbox | sommet > −10 dB (attention, avec le p90) ; idéal < −20 dB (texte) | Communauté | Brian White [S20] (« maybe minus 10 would be the highest ») ; 18 % des sessions (voir « Repères de PIDtoolbox ») |
| Gyro filtré, échelle de PIDtoolbox | sommet < −30 dB (affiché) | Communauté | « a good rule of thumb » [S21, S22] ; ne déclenche rien (35 % des sessions au-dessus) |
| Passage à l'échelle de PIDtoolbox | −6,5 dB | Corpus | 5 sessions de 4 drones avec captures PIDtoolbox, ± 1 dB (`validation.md`) |
| Retards des filtres gyro et D | p90 de la taille (vols avec mouvements), arrondi vers le haut | Corpus | Déclenche le conseil. Objectif affiché : graphique de Brian White [S26], 1 ms à 2" puis 0,25 ms (gyro) et 0,17 ms (D) par pouce (5" : 1,75 / 1,5 ms) ; dans la vidéo [S20] il citait de mémoire 1,5 ms pour un 5" et ≈ 5 ms pour un 10". Objectif D en valeurs PIDtoolbox, environ 0,5 ms sous notre mesure |
| Taille pour l'objectif de retard | 2–3,5" → 3" ; 8"+ → 8" | Corpus | Médiane des drones de taille connue rangés dans le profil (étiquettes de `corpus.csv` et noms, 2026-10-12). 2–3,5" : 9 drones, 2" (Pavo20), 2,5" (Whisper25 ×2, FoxWhoop 25, Gerrykiki « Test Hélices 2.5 »), 3" (04Pro3, T3, Seeker 3), 3,5" (JeNo 3,5 ×2) → 3" ; un 5" mal classé écarté. 8"+ : 8" (demonsgalore, Mario8), 9" (Pulsar) → 8". Peu de drones : à revoir avec W-002. Le graphique [S26] commence à 2" : pas d'objectif pour les whoops |
| `rpm_filter_q` pour gagner du retard | + 100 par pas, jusqu'à 1000 | Firmware (doc) + décision | « stepwise increasing the Q from the default of 500 up to a max of 1000 » [S27] ; Brian White : 750 [S20, 34:12]. Pas de 100 : décision de Cédric (2026-10-10), à la place de 250. Proposé seulement dans le conseil « filtrage gyro trop prudent » (bruit faible) |
| Poids RPM juste suffisant | 1 − 10^(−(hauteur + 6 dB)/20), arrondi au dixième supérieur | Firmware + estimation (marge) | Profondeur du notch tirée du code (`rpmNotchUpdate`) ; marge de 6 dB choisie pour un taux de 6 % (voir « Filtre RPM : poids et Q ») ; seulement 2e et 3e harmoniques, comme [S27] ; presets officiels en tripale : 100, 0, 80 (AOS), 100, 20, 100 (Karatebrot), 90, 30, 90 (SupaflyFPV) [S28] |
| Gain minimal pour proposer un poids | 0,5 | Estimation | Choisi avec Cédric sur le taux de déclenchement |
| Raie moteur « visible » | > 4 dB au-dessus du bruit voisin | Corpus | Même mesure là où il n'y a aucune raie (0,45 et 0,6 × la fréquence moteur) : au-dessus de 4 dB dans moins de 1 % des sessions (3 dB : 1,4 à 2,8 %), voir « Filtre RPM : poids et Q » (2026-10-12). Repris tel quel pour les raies hors moteurs après filtrage, sans calibration propre |
| Latence (repères bas et haut) | p10 arrondi vers le bas, p90 vers le haut | Corpus | |
| Seuils des profils whoop et 2–3,5" | voir « Calibration » | Corpus (provisoire) | 10 et 8 drones seulement |
| Seuils du profil 8"+ | voir « Calibration » | Estimation | 6 drones, 20 sessions |
| Résonance | raie > 6,8 (5") à 8,2 dB (7") selon la taille, 7,9 dB commun ; 20–900 Hz (sans les 5 premiers Hz), ≥ 8 tranches de gaz, à plus de ±8 % des harmoniques moteur | Corpus (seuil) + estimation (méthode) | | Recherche descendue de 60 à 20 Hz le 2026-10-05 (`dyn_notch_min_hz` descend jusqu'à 20 Hz) : 39 sessions signalées au lieu de 35, dont 5 raies réelles sous 60 Hz (7" de Thalhammer à 37–50 Hz) ; les raies collées à 20 Hz venaient des mouvements et des tests wobble / chirp ; seuils calibrés avec l'ancienne plage
| Pic suivi par le notch dynamique | > 2 × la puissance moyenne de la plage (+3 dB), cases de 18,5 Hz | Firmware | `flight/dyn_notch_filter.c` (`sdftNoiseThreshold *= 2`, bruit au-dessus de 60 Hz) ; bandes des harmoniques traitées par le filtre RPM retirées (± 5 % : estimation) |
| Plage du notch dynamique autour d'une raie | ± 20 Hz | Firmware (doc) | « including 20hz additional range each side of the noise » [S19] ; bornes du firmware 20–250 Hz (min) et 200–1000 Hz (max) |
| Relèvement de `dyn_notch_min_hz` | jusqu'à 200 Hz (7" et plus : 125 Hz), sous le pic le plus bas − 20 Hz, si le gain atteint 30 Hz | Firmware (doc) + communauté + estimation (30 Hz) | « as high as practical (eg 200hz) » [S19] ; presets officiels 7–10" entre 80 et 125 Hz [S16] ; seulement dans le conseil « filtrage gyro trop prudent » (2 % des sessions) |
| Nombre de notchs | autant que de pics à suivre, 1 au moins | Firmware (doc) | « Usually no more than two or three are needed […] and often only one » ; « only using the minimum number of notches » [S19] ; constat **info** : 27 % des sessions (16 % des drones)
| Raie hors moteurs listée (vol stationnaire) | ≥ 6 dB au-dessus du bruit voisin (10 à 20 % de part et d'autre) | Théorie (simulation) + choix de Cédric | Du bruit sans raie, simulé avec les durées et fréquences des 115 vols stationnaires du corpus, ne dépasse jamais 3 dB : toute raie listée est réelle, le seuil règle le nombre de conseils (voir « Stratégie de filtrage et vol stationnaire », 2026-10-12). Visible après filtrage au-delà de 4 dB (`HARMONIC_VISIBLE_DB`) |
| Raie basse fréquence | < 100 Hz : mise en garde avant de la filtrer | Communauté | Propwash et oscillations « ~20Hz to ~100Hz » [S1] |
| Baisse de `rpm_filter_q` (raie moteur visible) | −100 par pas, borne 250 | Firmware (borne) + décision | Pas de 100 : décision de Cédric (2026-10-10), à la place de −30 % ; aussi dans « Bruit moteur mal éliminé » |
| Hausse de `rpm_filter_q` (harmoniques effacées) | + 100 par pas, jusqu'à 1000 | Firmware (doc) + décision | [S27] ; condition « harmoniques effacées, bruit sous les repères, tout type de vol » : décision de Cédric (2026-10-10) |
| `dyn_notch_q` avec filtre RPM | Q calculé d'après la largeur et la hauteur des raies, arrondi à 50, de 100 à 450 | Théorie + communauté + estimation | Forme du notch [S31] (décision de Cédric, 2026-10-10 : calcul plutôt que paliers) ; 450 au plus avec filtre RPM [S19] ; 100 au moins : estimation (sources : 120–150 sans filtre RPM [S19]) ; +50 si aucune raie n'est mesurable ; raie ramenée à 4 dB du bruit voisin (`HARMONIC_VISIBLE_DB`) |
| `dyn_notch_count` en hausse | un notch par pic à suivre, 7 au plus, si une raie passe ou si le gyro est bruité | Firmware (borne) + décision | `cli/settings.c` (0 à 7) ; décision de Cédric (2026-10-10) |
| Terme D propre : relèvement des filtres | D sous −20 dB PIDtoolbox ; jusqu'à 1,2 (ou +0,2 au-delà), D prévu sous −10 dB et sous le p90 | Communauté + configurateur + estimation | « ideally below about minus 20 dB » [S20] ; 0,8–1,2 hors mode expert (`useTuningSliders.ts`) ; +0,2 et deux pas × 1,1 sans curseur : estimations ; décision de Cédric (2026-10-10) |
| `rpm_filter_harmonics` | jusqu'à 3 | Firmware | `cli/settings.c` (0 à 3) |
| Ordre de la stratégie de filtrage | enregistrement, corrections, mécanique, RPM, notch dynamique, LPF gyro, LPF D | Firmware (doc) + estimation | Un filtre à la fois [S30] ; ordre RPM → notch → passe-bas comme le guide RPM |
| Stratégie de filtrage affichée | à partir de 2 étapes (`rules.STRATEGY_MIN_STAGES`) | Corpus + décision de Cédric | 37 % des sessions contre 79 % dès une étape (2026-10-05) |
| Vol avec mouvements (retards représentatifs) | 95e centile de \|consigne\| ≥ 50 °/s (roll ou pitch) | Corpus + estimation | Remarque de Cédric (2026-10-05) ; vols stationnaires du corpus ≤ 24 °/s, avec wobble ≥ 92 °/s ; l'intercorrélation s'y trompe une fois sur deux (`validation.md`). 108 sessions (16 %) en dessous ; `calibrate_advisor.py` en écarte les retards ; seuils de retard calibrés sur les vols restants |
| LPF2 du gyro anti-repliement | PID < gyro : PT1 500 Hz (attention sous 3 kHz), 500–1000 Hz (conseil à 4 kHz) | Firmware (doc) | [S12, S19, S29] |
| Ordre minimal du filtrage du D | 2 (deux PT1 ou un filtre d'ordre 2) | Firmware (doc) | [S29] ; PT2 et biquad comptent pour 2, PT3 pour 3 |
| LPF1 du D dynamique proposé | min = coupure statique, max = 2 × min | Firmware (défauts 75–150 Hz) + estimation | Mode statique « not recommended » [S29] |
| Coupure du LPF1 du gyro | si filtre RPM actif, vol avec mouvements, bruit actuel et prévu sous les repères | Firmware (doc) + corpus (repères) | [S29, S30] ; 4 % des sessions |
| Prévision du curseur « filtre D » | +0,1 = multiplicateur +10 (×1,1 sans curseur) | Firmware | `simplified_tuning.c` ; relèvement proposé seulement si le D prévu reste sous les repères |
| Déséquilibre des moteurs | > 11,2 (7") à 14 % (5") selon la taille, 13,5 % commun | Corpus | L'interprétation (côté ou diagonale au-delà de 12 %) reste estimée |
| Terme I décentré | moyenne > 31 (7") à 33 (commun, unités PID) selon la taille | Corpus | |
| Saturation des moteurs | ≥ 98 % pendant > 1,5 % du temps | Corpus | La notion de « moteur à 98 % » est estimée : le firmware ne journalise pas de saturation (il renormalise le mixage quand l'écart entre moteurs dépasse 100 %) |
| Pas des curseurs conseillés | 0,1 (0,1 à 0,2 pour les filtres) | Estimation | Le configurateur avance par 0,05 ; en mode de base il borne PI, D et FF à 0,70–1,40, le filtre gyro à 0,50–1,50 et le filtre D à 0,80–1,20 |
| `rpm_filter_q` conseillé | −100 par pas (500 → 400) | Firmware (500) + décision (pas) | Défaut 500, plage 250–3000. Les sources conseillent plutôt de **monter** Q (jusqu'à 750 [S9] ou 1000 [S6, S14] ; 750–1000 sur un 5" propre selon Brian White [S20]) et de redescendre seulement si le bruit moteur réapparaît, ce qui est le cas où la règle le propose (bruit moteur restant après filtrage) ; aucune ne donne de valeur sous 500, d'où des pas de 100 (décision de Cédric, 2026-10-10). Monter Q : voir la ligne « pour gagner du retard ». Avec un poids < 100, la règle « poids trop bas » propose d'abord de le remonter |
| `motor_poles` | 14 (12 pour certains micro-moteurs) | Firmware (14) + communauté (12) | Défaut 14 dans Betaflight ; « Smaller motors have fewer magnets, often 12 » [S9], 12 pour les 08XX à 16XX [S13] |
| Notch statique conseillé | coupure tirée du Q nécessaire pour la raie, 0,62 à 0,89 × sa fréquence ; 0,7 sans mesure | Théorie + firmware | Même calcul que le Q du notch dynamique [S31], `filterGetNotchQ` ; repli 0,7 dans la plage des défauts de Betaflight 3.2.0 (0,5 à 0,75), voir « Règles » (2026-10-12) |

Le champ `resonance_hz` des profils (`advisor/profiles.py`) est renseigné mais **aucune règle ne
l'utilise** ; `hover_hz` ne sert qu'à la fiche drone du guide (`advisor/drone_card.py:log_rate`,
fréquence d'enregistrement conseillée d'après le milieu de la plage).

### Sources vérifiées

- S. Skogestad, I. Postlethwaite, *Multivariable Feedback Control*, 2e éd., Wiley, 2005, § 2.4.3,
  p. 36 : Ms inférieur à environ 2 (6 dB) ; équation 2.47 : marge de phase ≥ 2·arcsin(1/(2 Ms)), soit 29°
  pour Ms = 2 ([PDF de l'auteur](https://skoge.folk.ntnu.no/book/ps/bookall.pdf)).
- K. J. Åström, R. M. Murray, *Feedback Systems*, 1re éd., Princeton, 2008, § 9.3, p. 281 : marge de phase
  30–60°, marge de gain 2–5, marge de stabilité 0,5–0,8 (Ms 1,25–2).
- Firmware Betaflight (commit e796d7f87) : `rpm_filter_q` = 500 (`pg/rpm_filter.c`), `motor_poles` = 14
  (`pg/motor.c`), `blackbox_sample_rate` = 1/4 (`blackbox/blackbox.c`). Configurateur : pas et bornes des
  curseurs (`PidSubTab.vue`, `FilterSubTab.vue`, `useTuningSliders.js`).
- Firmware Betaflight (commit cc1bb9d28) : `rpm_filter_harmonics` = 3 (`pg/rpm_filter.c`) ; réglages
  `blackbox_disable_rpm` et `blackbox_disable_gyrounfilt` (`cli/settings.c`).
- Firmware Betaflight 3.2.0 (étiquette `v3.2.0`) : défauts des notchs statiques `gyro_soft_notch_hz_1` /
  `cutoff_1` = 400 / 300, `_2` = 200 / 100 (`sensors/gyro.c`), `dterm_notch_hz` / `cutoff` = 260 / 160
  (`flight/pid.c`) ; à 0 (désactivés) depuis la 4.0.

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
  (20 % des sessions du corpus ; avec 6 dB / 40°, 33 %) ;
- **D dominant** (2026-10-12, `rules.D_DOMINANT_RATIO` = 1,0) : effet d'un cran de +0,2 sur la stabilité,
  dans les 55 séries Damping et Pitch Damping du corpus (130 crans mesurés, au moins 200 fenêtres, 25
  drones, une voix par drone, intervalle à 90 % par rééchantillonnage). Selon |Ĉd/Ĉe| au départ : à 0,6, pic
  de sensibilité −0,25 dB et marge +3,7° (le pic baisse chez 73 % des drones) ; à 0,8, pic −0,03 dB, marge
  +3,9° ; à 1,0, pic **+0,14 dB [+0,03 ; +0,26]**, marge +2,7° (le pic baisse chez 27 % des drones) ; à 1,2,
  pic +0,14 dB, marge +1,3° ; à 1,4, pic +0,24 dB, marge +0,7°. Le pic, qui déclenche 89 des 101 axes
  mesurés signalés, monte dès 1,0 : seuil gardé à 1,0 (choix de Cédric). La marge seule gagne encore 1 à
  3° jusqu'à 1,3 (5 axes signalés par la marge seule entre 1,0 et 1,3) : un seuil propre à la marge a été
  écarté, gain trop faible. Aucun axe signalé entre 0,8 et 1,0 : 0,8 donnerait les mêmes conseils ;
- whoops (diagnostic du 2026-10-06, `validation.md`) : 32 % des sessions whoop signalées, 16 % hors des
  drones de Cédric (45 sessions). La mesure est vérifiée (simulation, vols chirp de l'AIR65) : la marge
  est réellement faible (croisement vers 20 Hz, D dominant 1,4 fois, filtres du D qui retardent au
  croisement). Seuils inchangés (décision de Cédric) ; le constat d'un whoop l'explique et rappelle que
  le seuil vient de la théorie et non d'une comparaison avec les autres whoops.

### Onglet Autotune du configurateur Betaflight

Il utilise l'ancienne méthode approchée (L = T/(1 − T), Ms ≤ 2) avec des marges de phase cibles de 50°, 60° et
72,5°. Il **n'est pas cité comme source** (vérifié le 2026-10-05) : ces valeurs n'existent que sur la branche
de développement (PR [#5406](https://github.com/betaflight/betaflight-configurator/pull/5406), absente de
2026.6.2) ; 50 et 60° sont des choix dans la plage classique, 72,5° est propre à l'auteur, réglé sur deux
vols d'un seul 5" ; la PR a été approuvée sans essais en vol, et un ticket de perte de contrôle
([#5258](https://github.com/betaflight/betaflight-configurator/issues/5258)) est ouvert. Ms ≤ 2 est cité
d'après Skogestad et Postlethwaite.
