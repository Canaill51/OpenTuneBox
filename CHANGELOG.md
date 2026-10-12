# Journal des versions

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) ; numérotation
[SemVer](https://semver.org/lang/fr/).

## [Non publié]

### Ajouté
- Tests sans corpus des analyses de vol (`tests/test_analysis_flight.py` : fréquence moteur mesurée ou
  estimée par KV ou multiplicateur, médiane en stationnaire, bruit aux harmoniques, statistiques de vol,
  erreur PID robuste, détection de crash et période automatique), du diagnostic du bruit
  (`tests/test_noise_report.py` : rangement des constats, verdict de chaque bloc selon l'eRPM, le gyro
  non filtré, les filtres coupés et la cadence de la boucle), des cas limites du diagnostic du réglage
  (séries incomplètes, « déjà sans dépassement », cassure forte ou incertaine, axe non lu), des textes du
  parcours du guide qui doivent suivre les constantes du code, des scripts du corpus (passage parallèle avec
  reprise des fichiers en erreur, tableaux récapitulatifs) et de la recette PyInstaller (chemins cités,
  filtres Qt et pyqtgraph, lanceur). `tests/test_docs.py` vérifie que `docs/assistant.md` porte chaque
  seuil de `advisor/profiles.py` (bruit, latence, retards, tableau des seuils mécaniques).
- Tests des encodages groupés du décodeur par aller-retour avec des encodeurs de la fabrique
  (`TAG2_3SVARIABLE`, `TAG2_3S32`, `TAG8_4S16` v1 et v2 avec quartets décalés, `TAG8_8SVB`, `NEG_14BIT`),
  trame mêlant tous les encodages, groupe plus long que la définition, encodage inconnu ; lecture de
  l'en-tête (fin de ligne Windows, ligne sans deux-points, octets parasites, trames P héritant de I,
  replis numériques). Simulation des filtres : chaque passe-bas égal à `lfilter`, filtre au-delà de la
  cadence du log transparent, état initial stationnaire, notch à fréquence variable qui suit une raie
  mobile, centres `FFT_FREQ` selon la version, `simulate_gyro` sur le vol synthétique.
- Tests du décodeur sur des fichiers Blackbox fabriqués (`tests/bbl_factory.py`) : `P interval` (N et a/b),
  trames corrompues et resynchronisation, trame trop longue, saut d'itération refusé, `LOGGING_RESUME`,
  événements, trames S, sessions multiples, prédicteurs (dont la troncature vers zéro de `AVERAGE_2`).
- Tests des contrats de l'interface sans corpus : `analysis/`, `advisor/`, `log/`, `decoder/` et la CLI
  n'importent jamais Qt (contrôle du code et à l'exécution), icônes citées présentes avec la licence Lucide,
  deux thèmes sur la fenêtre principale et sur chaque outil, demande rejouée après un calcul, erreur de
  calcul affichée sans plantage.
- Tests de l'assistant sans corpus : chaque seuil de profil encadré (constat présent juste sous le seuil,
  absent juste au-dessus, seuil cité dans le texte), deux branches du « Terme D bruité », repères de retard,
  bornes de la taille par la fréquence moteur, instantané des profils et des constantes (toute modification
  volontaire passe par la compétence `assistant-seuils`), paliers de pic avec et sans feedforward, marges de
  stabilité et de phase, règles de passe-bas, poids RPM et garde de version 4.5, repères de fin des filtres,
  vérification des logs du guide (cadences, version, champs, rôles, usages, mode de vol, curseur I).
- Tests d'invariants des analyses sans corpus : DSP égale à `scipy.signal.welch` (fenêtre paire ou impaire),
  puissance d'un sinus et Parseval, échelle de PIDtoolbox appliquée une seule fois (contrôle statique des
  appelants), valeurs par défaut de la réponse indicielle figées, compte exact des fenêtres, horizon du pic,
  réponse identité et retard pur, bande passante dès 2 Hz, marges (gain × k, retard pur), retards par la
  phase et par intercorrélation, `compute_delays`.
- Tests de la couche log sans corpus : trames impossibles et session corrompue à plus de 50 %, unités de
  `from_session` (DShot / PWM, eRPM ou debug, debug `GYRO_SCALED` comme gyro brut), masques Angle / Horizon et
  instrument de la boucle ouverte, excitation chirp, conventions D / D max aux limites, cadences et
  `P interval`, familles de version, tables de debug et de modes, replis de l'en-tête.
- Tests sans corpus : logs synthétiques (`tests/synthetic.py`) et fixtures communes (`tests/conftest.py`,
  réglages de l'application isolés pendant les tests) ; chaque page d'outil et la fenêtre principale sont
  exercées en intégration continue ; les tests des fonctions pures de l'assistant ne dépendent plus des
  vrais logs ; passe du décodeur en Python pur (numba désactivé) dans le workflow de tests.
- Documentation de développement : journal des décisions (`docs/decisions.md`), leçons tirées des pièges
  (`docs/lecons.md`) et travaux reportés (`docs/travaux-reportes.md`), numérotés `D-`, `L-`, `W-` ; repris de
  `AGENTS.md` et `docs/architecture.md` avec contexte, options écartées et preuves. Numéros vérifiés à chaque
  `pytest`.

### Modifié
- Toutes les couleurs fixes de l'interface (courbes et repères des graphiques, badges de l'assistant, curseur et
  textes posés sur les spectrogrammes, surlignage de la page Configuration) sont regroupées dans `gui/theme.py` ;
  les écarts verts / rouges du tableau des curseurs PID et des retards du simulateur, le titre d'alerte du
  simulateur et le contour des points de série suivent désormais les couleurs du thème (lisibles en clair comme
  en sombre). Un test refuse toute couleur en dur hors de `theme.py` (W-008 clos).

### Corrigé
- Changer de thème (clair / sombre) repeint aussi les outils déjà ouverts : leur fond de graphique gardait
  les couleurs de l'ancien thème jusqu'à un nouveau calcul.
- Spectres : avec une fenêtre d'un nombre impair d'échantillons (fréquence d'enregistrement non entière),
  la dernière case n'était pas doublée comme chez scipy (3 dB sur une seule case au bord, sans effet visible).
- Simulateur de filtres : sur les fréquences exactement au creux d'un notch (RPM ou dynamique), le spectre
  simulé s'effondrait d'environ 90 dB, même sans aucun changement de réglage (plancher numérique appliqué
  au seul dénominateur du rapport |H_modifié|² / |H_actuel|²).
- Documentation de l'assistant : tableau « Origine des seuils » réaligné sur `profiles.py` (écart roll /
  pitch, déséquilibre, terme I, résonance, règle du nom) ; `hover_hz` sert à la fiche drone du guide.
- Assistant de réglage : la taille de drone choisie était lue dans le widget depuis le thread de calcul ;
  tous les outils passent maintenant leurs réglages par `snapshot_options` (sans repli lisant les
  widgets), ce qu'un test vérifie sur chaque page du registre.
- `opentunebox report` : un fichier illisible ou sans session est signalé en une ligne, comme avec `info`,
  au lieu d'interrompre la commande avec une trace d'erreur.
- Décodeur : un en-tête tronqué dont la trame GPS a moins de prédicteurs que de noms faisait planter la
  lecture (`IndexError`) au lieu d'ignorer la session.
- Assistant, taille d'après le nom du drone (sans télémétrie moteur ni tension) : « 10in » donnait 0 pouce
  (profil 2–3,5") et un nombre contenant 65 ou 75 (« 1750 ») passait pour un whoop.
- Logs en haute résolution (`blackbox_high_resolution = ON`) : le gaz était dix fois trop faible (le gaz du
  mixeur `setpoint[3]` n'est pas mis à l'échelle par le firmware, contrairement aux quatre voies
  `rcCommand`, qui le sont) ; vérifié dans `blackbox.c` 4.4 → 2026.12, spécification corrigée.
- Tests sur les vrais logs : logs supprimés du corpus remplacés par des logs aux mêmes propriétés ; un log
  absent fait ignorer le test en le nommant au lieu de le faire échouer.

## [0.6.0] — 2026-10-12

### Ajouté
- **Guide** intégré (bouton de la barre d'outils, affiché au démarrage sans log) : parcours de réglage à
  cocher, par drone, en quatre phases (préparer le drone, filtres, PID par séries, finitions). **Fiche
  drone** (version de Betaflight, taille, boucle PID, ESC ; préremplie d'après le log ouvert ; boutons
  Nouveau drone, Modifier, Recommencer, Supprimer) : fréquences d'enregistrement, listes des séries et plage
  du terme I s'y adaptent, et chaque drone garde son avancement. Changements donnés en positions de curseurs
  du configurateur, le CLI en repli ; boutons qui ouvrent l'outil utile sur la bonne partie du vol.
- Guide : les logs du drone (reconnus par le nom dans Betaflight) **cochent ce qu'ils prouvent** ou signalent
  ce qui est à revoir, avec la raison. Logs des **filtres** et logs des **séries PID** demandés et jugés à
  part : bouton d'ouverture dans chaque phase, panneau des logs en deux parties, lien pour changer le rôle
  d'un log. Le vol de réglage des filtres (stationnaire puis wobbles) est découpé automatiquement ; protocole
  de Brian White vérifié (mode Angle, ou Acro avec le feedforward à 0 ; rates ACTUAL 200 / 200 / 0 ; limite
  d'angle 30° en mode Angle ; roll et pitch mêlés ; Dynamic Damping à 0 ; curseur I ; au moins 200
  fenêtres) ; chaque série d'un drone reconnue ; repères de fin des filtres mesurés ; copie des commandes
  CLI équivalentes.
- **Série Feedforward** (curseur « Réponse des sticks », pas de 0,5 : 0, 0,5, 1…, en Acro uniquement) :
  l'Assistant donne le **retard consigne → gyro** en roll et en pitch de chaque vol, et le guide le résume
  sous les logs des séries. La Réponse indicielle reconnaît la série sans la juger (le feedforward fausse la
  réponse).

### Modifié
- Fréquence d'enregistrement : minimum propre aux **PID** (1 kHz), distinct de celui des **filtres** (2 kHz
  ou 1,6 kHz, plus pour les petits drones). Un vol de série PID à 1 kHz n'est plus signalé « trop lent » par
  l'Assistant (simple conseil pour les filtres) ni bloqué par la vérification des logs du guide, qui indique
  les deux fréquences.
- Réponse indicielle, Diagnostic du réglage : un vol de série avec moins de 200 fenêtres en roll ou en pitch
  est signalé « mesure moins sûre » (consigne de Brian White) ; sous 20, il n'est toujours pas lu.
- Spectre, Fréq. × gaz, Fréq. × temps et Filtres : les niveaux en dB sont affichés sur l'échelle de
  PIDtoolbox (6,5 dB plus bas qu'avant), pour lire directement les repères habituels (gyro sous −30 dB,
  terme D sous −10 dB) et comparer aux captures de PIDtoolbox.
- Spectre : résolution par défaut 0,5 Hz (fenêtres de 2 s), comme PIDtoolbox.

## [0.5.0] — 2026-10-10

### Ajouté
- Réponse indicielle : **Diagnostic du réglage**, toujours affiché à côté des courbes, méthode de Brian White. Reconnaît
  les séries de vols chargées (un seul curseur changé, mêmes filtres) et conseille le Damping (équilibre P/D),
  le Master (cassure « ripple », vagues et plateau de la latence) et l'équilibre roll / pitch (écart de latence
  de 1 à 2 ms au plus) ; vérifie le feedforward (0 ou mode Angle), le curseur I (0,2–0,3) et le pas (0,2).
  Vols conseillés entourés dans les colonnes de mesures, cassure marquée sur les courbes, colonne
  « Cassure (ripple) ».
- Spectre : case **Diagnostic du bruit** (cadre « Diagnostic », en bas du panneau), rapport à côté des courbes sur le bruit du gyro et du terme D
  avant et après filtrage (niveaux par axe), un bloc par filtre (filtre RPM, notch dynamique, passe-bas
  du gyro et du D) avec ses réglages, ses mesures et un verdict même sans conseil (« rien à changer »
  ou « impossible à juger », avec la raison), et stratégie de filtrage conseillée, avec les constats et
  seuils de l'assistant.
- Visualiseur : bouton **Mesurer**, écart de temps (ms) et fréquence correspondante (Hz) entre deux points
  cliqués sur les graphiques roll, pitch ou yaw ; repères déplaçables à la souris.

### Modifié
- Interface : plus aucune nouvelle fenêtre. Les boutons de la barre d'outils servent d'**onglets** : un clic
  affiche l'outil dans la fenêtre principale, nouveau bouton **Visualiseur** pour revenir aux courbes du vol,
  bouton de l'affichage en cours mis en valeur. Icônes des outils en couleur (une couleur par outil, lisible dans les
  deux thèmes).
- Visualiseur : barre d'outils allégée. Fréq. × gaz et Fréq. × temps s'ouvrent depuis l'outil Spectre ;
  le bouton Configuration passe en bas du panneau de droite.
- Réponse indicielle : courbes plus larges (gains, pic, latence et cassure de chaque fichier écrits dans le
  graphique, plus de colonne à part), durée affichée au choix (250 ms par défaut, 500 ou 150 ms), yaw
  masqué au départ, curseurs PID Betaflight toujours affichés sous le diagnostic (plus de case « Vue curseurs
  Betaflight »).
- Curseurs PID Betaflight (Réponse indicielle et Curseurs PID) : noms du configurateur Betaflight (Damping /
  D Gains, Tracking / P & I Gains, Stick Response, Dynamic Damping, Drift - Wobble, Pitch Damping, Pitch
  Tracking, Master Multiplier) au lieu de traductions maison.
- Spectre et Réponse indicielle : le panneau de réglages garde sa largeur quand le rapport s'ouvre (il
  laissait un grand vide à droite).
- Assistant : `rpm_filter_q` conseillé par pas de 100 dans les deux sens (au lieu de +250 et −30 %) ;
  nouveau conseil « Filtre RPM : Q peut monter » quand les harmoniques traitées sont effacées et que le
  bruit est sous les repères (jusqu'à 1000) ; nouveau conseil « Notch dynamique : Q peut monter » (+50
  jusqu'à 450 avec filtre RPM quand aucune raie ne passe).
- Assistant : `dyn_notch_q` calculé d'après la largeur et la hauteur des raies à suivre (baissé jusqu'à
  100 si une raie l'exige, monté jusqu'à 450 sinon) ; `dyn_notch_count` augmenté (un notch par pic) quand
  une raie passe ou que le gyro est bruité.
- Assistant : nouveau conseil « Terme D propre : ses filtres peuvent être relevés » sur tout vol, quand le
  D est sous −20 dB PIDtoolbox : le simulateur choisit le plus grand pas du curseur « filtre D » (jusqu'à
  1,2) qui garde le D sous les repères.
- Visualiseur : la période d'analyse se règle sur les graphiques roll, pitch et yaw, plus seulement sur
  celui des moteurs ; les quatre zones restent synchronisées pendant le glissement.
- Visualiseur : info-bulle sur les bords de la période d'analyse (début ou fin, avec le temps).

### Corrigé
- Réponse indicielle : les noms des curseurs Betaflight (sur deux lignes) ne se chevauchent plus sous le
  diagnostic ; le graphique reçoit la hauteur nécessaire, le rapport défile.

### Supprimé
- Fenêtre Statistiques de vol (peu utile ; les mesures restent utilisées par l'assistant).

## [0.4.0] — 2026-10-06

### Ajouté
- Assistant : constat « Seuils provisoires pour cette taille » pour les whoops, les 2–3,5" et les 8"+
  (trop peu de drones dans le corpus de calibration).
- `scripts/calibrate_advisor.py` : déséquilibre, terme I, saturation, résonance, fréquence moteur et nom
  du drone mesurés ; `--resume` affiche les percentiles par taille, par session et avec une voix par drone.
- `scripts/corpus_labels.py` : tableau d'étiquettes du corpus (`corpus.csv` dans le dossier des logs, une
  ligne par drone : état, taille réelle, fréquence moteur, KV, cellules), pré-rempli puis complété à la
  main ; un nom porté par plusieurs drones (« Jax ») est complété par le dossier ; les logs en double
  (même contenu) et les dossiers `__MACOSX` sont ignorés, aussi par `calibrate_advisor.py` ; `calibrate_advisor.py --resume` affiche à côté de chaque groupe son corpus de référence (drones
  réglés ou d'origine, hors demandes d'aide).

- Réponse indicielle : **boucle ouverte mesurée** à partir des termes P, I, D et F du log (D sur le gyro,
  feedforward sur la consigne, comme dans Betaflight) au lieu de l'approximation T/(1 − T), qui plaçait
  le croisement environ 2 fois trop bas et surestimait la marge de phase. Marge de gain ajoutée ; barres
  d'erreur (jackknife) sur les marges ; point creux quand la valeur n'est qu'approchée (termes absents),
  triangle quand le pic de sensibilité n'est qu'un minimum (au-delà des fréquences mesurables).

- Assistant : repères de Brian White (PIDtoolbox) sur l'échelle de ses spectres (nos DSP sont 6,5 dB
  plus hautes) : terme D signalé aussi quand le sommet de son spectre dépasse −10 dB ; niveau du gyro
  filtré affiché face au repère −30 dB ; objectifs de retard des filtres selon la taille (graphique de
  Brian White) rappelés dans les textes ; conseil « filtrage gyro trop prudent » complété par
  `rpm_filter_q` + 250, par pas jusqu'à 1000 (documentation de Betaflight 4.5).
- Assistant : poids du filtre RPM (`rpm_filter_weights`, 4.5 et plus) — poids juste suffisant calculé pour
  chaque harmonique à partir de sa hauteur au-dessus du bruit voisin et de la formule du firmware ;
  conseil quand la 2e ou la 3e harmonique est filtrée bien plus que nécessaire, ou quand une raie reste
  visible alors que son poids a été baissé.
- Assistant : notch dynamique (notes de réglage Betaflight 4.3 et code du firmware) — pics qu'il a à suivre
  après le filtre RPM ; « moins de notchs suffiraient » (info) ; plage élargie quand une raie fixe en sort
  (20 Hz de marge, jusqu'à la limite de 20 Hz du firmware) ; `dyn_notch_min_hz` relevé dans le conseil
  « filtrage gyro trop prudent ».
- Assistant : raies fixes (résonances) cherchées dès 20 Hz au lieu de 60 Hz.
- Assistant : **stratégie de filtrage** en tête du diagnostic, qui ordonne les réglages (enregistrement,
  bruit à corriger, mécanique, filtre RPM, notch dynamique, passe-bas du gyro et du D).
- Assistant : vol stationnaire exploité pour identifier le bruit (« Vol stationnaire : bruit identifié » :
  harmoniques avant / après filtres, autres raies, bruit restant) et conseiller un notch pour chaque raie
  encore visible ; harmonique moteur hors du filtre RPM (`rpm_filter_harmonics`) ou visible malgré un poids
  de 100 (`motor_poles`, `rpm_filter_q`).
- Assistant : fréquence d'enregistrement minimale selon le gyro (2 kHz à 8 kHz, 1,6 kHz à 3,2 kHz, plus
  pour voir la 3e harmonique des petits drones) au lieu de 900 Hz ; sous ce minimum, aucun allègement de
  filtre n'est proposé.
- Retards des filtres jugés seulement sur un vol avec des mouvements (consigne au-delà de 50 °/s au moins
  5 % du temps) : sur un vol stationnaire, l'assistant affiche « Retard des filtres peu représentatif »
  sans conseil, et l'outil Spectre marque le retard « peu fiable ».
- Assistant : passe-bas du gyro et du D (configurateur et notes de réglage de Betaflight) — LPF2 du gyro
  coupé avec une boucle PID plus lente (repliement) ; D filtré à l'ordre 1 seulement ; LPF1 du D statique ;
  biquad sur le gyro ; « Le LPF1 du gyro peut être coupé » quand le simulateur de filtres prévoit un bruit
  encore sous les repères ; prévision du simulateur dans le conseil sur le filtrage du D, qui n'est plus
  proposé si le D dépasserait les repères.
- `scripts/calibrate_advisor.py` : sommets des spectres gyro et D sur l'échelle de PIDtoolbox, retard gyro
  par intercorrélation, réglages du filtre RPM, hauteur des harmoniques avant et après filtres.
- Scripts du corpus en parallèle sur tous les cœurs (`-j N` pour choisir, `-j 1` = un fichier à la fois) :
  `calibrate_advisor.py` en 2 min 40 au lieu d'une dizaine de minutes, `corpus_labels.py` en 40 s ; un
  fichier en erreur dans un processus est refait seul à la fin. Nouveau `scripts/advisor_rates.py` : part
  des sessions du corpus signalées par chaque constat de l'assistant (vols avec mouvements, stationnaires).

### Modifié
- Assistant : sur un whoop, les constats de marge de stabilité expliquent la cause habituelle (boucle
  rapide, D dominant, filtres du D qui retardent au croisement). Diagnostic des whoops souvent signalés :
  la mesure est vérifiée (simulation, vols chirp de l'AIR65), la marge est réelle, seuils inchangés.
- Assistant : paliers du pic de réponse indicielle calibrés sur le corpus et séparés selon le feedforward
  (qui gonfle le pic) : sans feedforward, conseil au-delà de 1,15 et attention au-delà de 1,25 ; avec, 1,40
  et 1,55 (avant : 1,10 et 1,20 pour tous, 46 % des sessions signalées, maintenant 13 %). L'objectif
  « 1,02–1,08 », sans source, est remplacé par le repère de la série BlackSummer (environ 1,03 sans
  dépassement visible), dans les constats, l'outil « Indicielle » et le guide.
- `scripts/calibrate_advisor.py` : gains P, D et F de chaque axe et positions des curseurs enregistrés.
- Assistant : repères de retard des filtres recalculés sur les seuls vols avec mouvements, pour toutes les
  tailles (gyro / D : whoop 1,5 / 2,5 ms, 2–3,5" 1,8 / 2,6, 5" 1,9 / 2,7, 7" 2,6 / 2,8, 8"+ 3,5 / 3,1) :
  les vols stationnaires les avaient fait monter, et les conseils sur le retard ne touchaient plus que 2 à
  4 % des vols.
- Assistant : la stratégie de filtrage n'apparaît plus qu'à partir de deux étapes de filtrage à revoir
  (37 % des sessions du corpus au lieu de 79 %) ; une seule étape, le constat seul suffit.
- Assistant : un retard des filtres gyro au-dessus du repère n'est plus affiché « ok » quand le bruit
  empêche de filtrer moins (info) ; nouvelle ligne « ok » pour le retard des filtres du D.
- Assistant : taille du drone détectée par la fréquence des moteurs **et** le nombre de cellules
  (1S → whoop ; 2 à 4S entre 200 et 290 Hz → 2–3,5") ; limite 5"/7" abaissée de 175 à 160 Hz. Juste pour
  38 drones de taille connue sur 41, au lieu de 27 : les 3,5" et 2,5" ne sont plus pris pour des 5".
- Assistant : seuils recalibrés sur tout le corpus avec la nouvelle détection (2–3,5", 5", 7" ; whoop et
  8"+ inchangés) ; stabilité en conseil au-delà de 7 dB de pic de sensibilité ou sous 35° de marge de phase
  (au lieu de 6 dB et 40°). Plus de corpus de référence : `calibrate_advisor.py --resume` compte tout le
  corpus.
- Assistant : constats de stabilité sur la boucle mesurée, en conseil (et non plus en alerte) jusqu'à la
  recalibration du seuil ; quand le D domine déjà au croisement, il conseille Master −0,1 plutôt que plus
  de D.
- Documentation : chaque repère de réglage « communauté » a sa source (auteur, date, adresse, citation
  exacte) dans `docs/spec-methodes.md`, ou est reclassé en estimation. Les paliers chiffrés du pic de
  réponse (1,02–1,08, 1,10, 1,20) et le tableau des fréquences moteur par taille n'ont aucune source
  publique : ce sont des estimations. Repères recoupés avec les vidéos de Brian White (chaîne PIDtoolbox),
  citées avec le moment de la vidéo : terme D sous −20 dB (−10 au plus), retard gyro ≤ 1,5 ms pour un 5".
- Assistant : seuils mécaniques calibrés sur 90 drones (déséquilibre 11–14 % au lieu de 12 %, terme I
  29–33 au lieu de 40, saturation 1,5 % du temps au lieu de 5 %, écart roll / pitch 0,07–0,12 au lieu
  de 0,08, résonance 7,3–7,9 dB au lieu de 6 dB).
- Assistant : limite whoop abaissée de 420 à 345 Hz (Meteor 75 et P85 reconnus comme whoops) ; seuils
  whoop recalculés avec une voix par drone, seuils 2–3,5" recalculés sans eux.
- Documentation de l'assistant : origine de chaque seuil (corpus, communauté, théorie, firmware ou
  estimation), comparaison avec le corpus et champs de profil inutilisés (`docs/assistant.md`) ;
  sources vérifiées (Skogestad et Postlethwaite, Åström et Murray, firmware et configurateur Betaflight) ;
  limite connue du pic de sensibilité et de la marge de phase (L = T/(1 − T), D sur le gyro) ; l'onglet
  Autotune du configurateur examiné et écarté comme source.

### Corrigé
- Réponse indicielle : les vols faits avec le chirp de Betaflight (debug CHIRP) en mode angle sont
  maintenant mesurés (boucle ouverte, marges) ; l'excitation du chirp, absente de la commande radio, servait
  de signal de référence manquant (« cohérence insuffisante »).
- Marge de phase de la réponse indicielle : la phase est maintenant déroulée. Une phase sous −180° au
  croisement donnait une marge repliée vers +180° (ex. +299° au lieu de −109°), donc aucune alerte de
  l'assistant ; elle est désormais négative. Croisement interpolé entre deux points de mesure.

## [0.3.0] — 2026-10-05

### Ajouté
- Installateurs construits par GitHub (`construire.yml`) et joints à la page Releases :
  - Windows : `OpenTuneBox-Windows-Setup.exe` (Inno Setup), sans droits administrateur, menu Démarrer,
    raccourci du Bureau et ouverture des `.bbl` / `.bfl` en option, désinstallation depuis les
    Paramètres, mise à jour par-dessus l'ancienne version. Il remplace le `.zip` : une seule version
    Windows, pour ne pas faire hésiter l'utilisateur.
  - macOS : image disque `OpenTuneBox-macOS.dmg` (glisser dans Applications), icône de l'application,
    numéro de version et ouverture des `.bbl` / `.bfl` depuis le Finder.
  - Linux : `OpenTuneBox-Linux.AppImage`, un seul fichier à rendre exécutable (remplace le `.tar.gz`).
  - Essais automatiques de chaque fichier : démarrage sur les trois systèmes ; sous Windows,
    installation, lancement puis désinstallation complète.
- Icônes macOS (`.icns`) et Linux (`.png`) fabriquées depuis le SVG par `packaging/make_icons.py`.
- Documentation vérifiée automatiquement à chaque `pytest` et sur chaque demande de fusion
  (`tests/test_docs.py`) : liens et titres, chemins et fonctions cités, chaque outil dans le README et le
  guide, chaque module dans `AGENTS.md` / `docs/architecture.md`, fichiers publiés, commandes, langue.
  Compétence `docs-sync` (quels documents relire selon le changement) et crochet Claude Code qui bloque
  un commit si la documentation n'est plus cohérente.

### Modifié
- Assistant : taille du drone détectée par la fréquence des moteurs **et** le nombre de cellules
  (1S → whoop ; 2 à 4S entre 200 et 290 Hz → 2–3,5") ; limite 5"/7" abaissée de 175 à 160 Hz. Juste pour
  38 drones de taille connue sur 41, au lieu de 27 : les 3,5" et 2,5" ne sont plus pris pour des 5".
- Assistant : seuils recalibrés sur tout le corpus avec la nouvelle détection (2–3,5", 5", 7" ; whoop et
  8"+ inchangés) ; stabilité en conseil au-delà de 7 dB de pic de sensibilité ou sous 35° de marge de phase
  (au lieu de 6 dB et 40°). Plus de corpus de référence : `calibrate_advisor.py --resume` compte tout le
  corpus.
- Documentation entièrement relue et mise à jour : README (outils sous le nom de leur bouton, partie
  développeur séparée), guide (installation et ouverture des logs réorganisées), assistant (toutes les
  règles réelles, détection de la taille, profil 8"+ provisoire), méthodes d'analyse (retards, lissages,
  latence corrigés), architecture (fenêtres d'outils, distribution), validation ; spécifications
  traduites en français.
- Tests rapides (`-m "not corpus"`) de nouveau en ≈ 1 s : les tests d'interface sur vrais logs portent
  la marque `corpus`.

### Corrigé
- Linux : la fenêtre d'ouverture de fichiers était peu lisible (icônes claires presque invisibles avec le
  thème clair sur un bureau sombre, textes en anglais, colonnes tronquées). OpenTuneBox utilise désormais
  la fenêtre du bureau (KDE, GNOME, Cinnamon, XFCE…) par le portail XDG, quel que soit le bureau ; sans
  portail, la fenêtre de Qt reste lisible : icônes du bureau en variante claire ou sombre selon le thème
  choisi, et textes de Qt en français (aussi les boutons OK / Annuler sur tous les systèmes).
- Page Releases : la publication échouait parce que les archives `.zip` Windows et macOS étaient
  décompressées avant d'être jointes (`skip-decompress` de `download-artifact`).

## [0.2.0] — 2026-10-05

### Ajouté
- Structure GitHub : `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, modèles de tickets et de demande de fusion,
  tests automatiques à chaque envoi (workflow `tests.yml`), `.gitattributes`, `.editorconfig`.
- Construction Windows par double-clic (`packaging/windows/construire_windows.bat`) et workflow GitHub
  `construire.yml` (Windows, macOS, Linux).
- Icône d'application sous Linux (fenêtres, barre des tâches Wayland, menu) et sous Windows (`.ico`).
- Dossier de logs des tests réglable par la variable `OTB_LOGS`.
- Page de téléchargement **Releases** créée automatiquement pour chaque étiquette de version `v…`, avec
  les archives Windows, macOS et Linux et la section du CHANGELOG ; script `scripts/release.py` qui
  prépare une version (numéro dans `pyproject.toml` et `__init__.py`, section datée du CHANGELOG) et
  arrête la construction si l'étiquette ne correspond pas.
- Convention de nommage des branches (`type/description-courte`) et des demandes de fusion (titre = nom
  de la branche) dans `CONTRIBUTING.md`, `AGENTS.md` et le modèle de PR.

### Modifié
- Assistant : taille du drone détectée par la fréquence des moteurs **et** le nombre de cellules
  (1S → whoop ; 2 à 4S entre 200 et 290 Hz → 2–3,5") ; limite 5"/7" abaissée de 175 à 160 Hz. Juste pour
  38 drones de taille connue sur 41, au lieu de 27 : les 3,5" et 2,5" ne sont plus pris pour des 5".
- Assistant : seuils recalibrés sur tout le corpus avec la nouvelle détection (2–3,5", 5", 7" ; whoop et
  8"+ inchangés) ; stabilité en conseil au-delà de 7 dB de pic de sensibilité ou sous 35° de marge de phase
  (au lieu de 6 dB et 40°). Plus de corpus de référence : `calibrate_advisor.py --resume` compte tout le
  corpus.
- Scripts d'installation rangés par système : `packaging/linux/`, `packaging/windows/`.
- Interface plus lisible, identique sous Windows, macOS et Linux : style Qt « Fusion » avec palettes
  claire et sombre complètes (fini le gris beige et les boutons qui semblaient désactivés sous Windows),
  couleurs des boutons d'outils contrastées dans chaque thème, cases à cocher et boutons radio bien
  visibles, traces repérées par un trait de couleur, textes d'aide moins petits.
- Fenêtre principale modernisée : barre d'outils avec icônes (Lucide, licence ISC) à la place des boutons
  du panneau droit, texte 15 % plus grand, traces groupées (Gyro, PID, Moteurs) et traces absentes du log
  masquées, sections « Affichage » et « Lecteur » repliables, traces lissées d'épaisseur 2 par défaut,
  grille limitée aux graduations principales.
- Fenêtres plus grandes : au premier lancement, la fenêtre principale occupe 95 % de l'écran (90 % pour
  les outils), comme PIDtoolbox ; ensuite chaque fenêtre rouvre à la taille et à la position laissées.
- Captures du README (visualiseur, réponse indicielle, assistant) refaites avec la nouvelle interface.

### Corrigé
- Visualiseur sans log ouvert : les graphiques ne sont plus entièrement grisés.
- Le raccourci Linux ne se déclare plus capable d'ouvrir tous les fichiers binaires.
- Windows : erreur « 'NoneType' object has no attribute 'flush' » à la fermeture de l'exécutable
  (pas de console, donc pas de `sys.stdout`).
- Exécutables construits par GitHub (`construire.yml`) : une seule archive par système, qui garde les
  droits d'exécution et les liens symboliques (le programme Linux se lançait mal et l'application macOS
  pouvait être abîmée) ; macOS n'envoie plus l'application en double (481 Mo auparavant) ; version Linux
  fabriquée sur Ubuntu 22.04 pour fonctionner aussi sur les distributions plus anciennes, avec les
  bibliothèques `libxcb` embarquées et un essai de démarrage.
- Exécutable allégé de 488 à 460 Mo sous Linux : sans le clavier virtuel de Qt (et son moteur QML/Quick),
  le lecteur PDF, les traductions de Qt ni les exemples, la 3D et l'intégration Jupyter de pyqtgraph.
  La construction Linux ne plante plus en parcourant les exemples de pyqtgraph (ouverture d'une fenêtre
  sur un serveur sans écran).

### Problèmes connus
- Analyse spectrale : les courbes des notchs dynamiques chargent fortement le spectre complet.
- Fréquence × gaz : le terme D (surtout préfiltre) sature l'échelle par défaut ; « Échelle auto » compense.
- Assistant : le graphique yaw affiche un axe incohérent quand les données sont insuffisantes.

## [0.1.0] — 2026-10-01

Première version complète.

### Ajouté
- Décodeur Blackbox natif (numba) : tous les encodages, prédicteurs et trames I/P/S/G/H/E, plusieurs
  sessions par fichier, fichiers tronqués ; validé sur 568 fichiers (95 % des sessions identiques à
  `blackbox_decode`, le reste étant des trames corrompues que la référence accepte).
- Modèle `FlightLog` en unités physiques, réglages d'en-tête normalisés de Betaflight 4.4 à 2026.x,
  tables `debug_mode` par version, modes de vol.
- Analyses : spectres, fréquence × gaz, fréquence × temps, retards de filtres (méthode de phase),
  réponse indicielle par déconvolution de Wiener et métriques de boucle, simulation exacte des filtres
  Betaflight, curseurs simplifiés ⇄ PID, statistiques de vol, erreur PID, période automatique avec
  détection de crash.
- Interface PySide6 : visualiseur de log et 9 outils (analyse spectrale, fréquence × gaz, fréquence × temps,
  réponse indicielle, curseurs PID, simulateur de filtres, infos de configuration, statistiques, assistant).
- Assistant de réglage à règles, profils whoop / 2–3,5" / 5" / 7" / 8"+ détectés par la rotation moteur,
  seuils calibrés sur 708 sessions.
- Ligne de commande `opentunebox info` et `opentunebox report`.
- Lanceur Linux (menu des applications) et recette PyInstaller.
- Guide d'utilisation en français.

[Non publié]: ../../compare/v0.6.0...HEAD
[0.6.0]: ../../releases/tag/v0.6.0
[0.5.0]: ../../releases/tag/v0.5.0
[0.4.0]: ../../releases/tag/v0.4.0
[0.3.0]: ../../releases/tag/v0.3.0
[0.2.0]: ../../releases/tag/v0.2.0
[0.1.0]: ../../releases/tag/v0.1.0
