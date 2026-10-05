# Méthodes d'analyse

Réimplémentées à partir de descriptions publiques (code et wiki publics de PIDtoolbox v0.55, PID-Analyzer,
Blackbox Explorer, BlackBox_CSV_Render, pid-tuna). **Aucun code n'est copié.** Les parties internes de
PTB Pro (simulateur de filtres, curseurs PID, estimation RPM, assistant) ne sont pas publiques : elles
sont conçues ici à partir des formules de Betaflight (`spec-betaflight.md`). Ce document décrit ce que
fait OpenTuneBox ; les conventions de PIDtoolbox (PTB) sont indiquées quand elles servent de référence.

## Conventions de chargement

- Fréquence du log `Fs = 1e6 / médiane(diff(time_us))` ; toutes les fenêtres sont données en ms × Fs.
- Gaz % = `setpoint[3] / 10`. Moteurs % = `(moteur − 48) / 1999 · 100` (DShot).
- Erreur PID = `gyroADC − setpoint` (signe de PTB).
- **D préfiltre** (reconstruit, `log/flightlog.py`) : `−d(gyroADC)/dt × k`, le gain `k` étant ajusté par
  moindres carrés sur les composantes lentes (< ~30 Hz, moyenne glissante sur Fs/60) du terme D loggé,
  sur chaque axe dont le D n'est pas nul. (PTB utilise le rapport médian, roll et pitch seulement.)
- **Gyro préfiltre** : `gyroUnfilt` (4.5+), sinon debug GYRO_SCALED [0..2] (4.4), sinon indisponible.

## Réponse indicielle (déconvolution de Wiener, `analysis/step.py`)

Entrées : consigne SP = setpoint[axe], GY = gyroADC[axe] sur la période choisie.
1. Lissage du gyro au choix (aucun / 20 / 40 / 60 échantillons).
2. Fenêtres de 2 s (1, 2 ou 3 s au choix), pas fixe de 62,5 ms.
3. Fenêtre gardée si max|SP| ≥ 20 °/s.
4. Fenêtre de Hann sur les deux signaux, 100 zéros ajoutés de chaque côté.
5. `G = FFT(gy)/N`, `H = FFT(sp)/N`, `imp = réel(IFFT(G·conj(H) / (|H|² + 1e-4)))`, réponse = somme cumulée de `imp`.
6. Correction Y (case « Y corr. », désactivée par défaut) : m = moyenne(réponse, 200–500 ms) ; réponse × (2 − m), comme PTB.
7. Contrôle : fenêtre gardée si 0,5 < min et max < 3 sur le régime établi (200–500 ms).
8. Réponse sur 500 ms ; moyenne ± écart-type et nombre de fenêtres n. Séparation possible à max|SP| 500 °/s.
9. Mesures sur la moyenne : pic = maximum sur t ≤ max(150 ms, 3 × latence) et son instant ;
   latence = passage à 50 % interpolé ; temps de montée 10–90 % ; établissement à ±5 %.

Mesures de boucle fermée (conception propre, même déconvolution) : T(f) = réponse en fréquence consigne →
gyro (moyenne des interspectres / autospectres des fenêtres), de 2 à 150 Hz, points de cohérence ≥ 0,5,
lissage complexe sur 5 points (`uniform_filter1d(mode="nearest")`). Bande passante = −3 dB de |T|,
cherchée à partir de 2 Hz ; pic de suivi = max |1 − T| (consigne → erreur).

### Boucle ouverte (`analysis/loop.py`)

Boucle coupée à l'entrée du drone, comme dans les manuels. Le régulateur de Betaflight
(`flight/pid.c`) : P + I agissent sur l'erreur e = consigne − gyro (Ce), le D sur le gyro seul (D = −Cd·gyro,
après les filtres du D), le feedforward F sur la consigne ; u = P + I + D + F commande le drone G (moteurs,
cadre, filtres gyro). Le gyro et la consigne loggés sont exactement ceux qu'utilise le PID
(`gyro.gyroADCf`, `previousPidSetpoint`). Vraie boucle ouverte : **L = G·(Ce + Cd)**, sensibilité
S = 1/(1 + L).

**Méthode « mesurée »** (termes P, I, D, F loggés, `FlightLog.pid_terms`) :
1. fenêtres de Hann de 2 s recouvertes de moitié, gardées si la consigne dépasse 20 °/s (et reste sous
   500 °/s avec la séparation « Consigne max ») ; interspectres sommés par bloc de fenêtres consécutives ;
2. Ĉe = S(e, P+I)/S(e, e) et Ĉd = −S(gyro, D)/S(gyro, gyro) : régressions directes, ces termes étant des
   fonctions déterministes de l'erreur et du gyro ;
3. Ĝ = S(z, gyro)/S(z, u) : estimateur **instrumental**, l'instrument z étant un signal extérieur à la
   boucle — la consigne, ou la commande radio (`rcCommand`) quand le mode angle ou horizon est actif plus
   de 20 % de la période (la consigne dépend alors de l'attitude), plus l'excitation du chirp de Betaflight
   quand le debug CHIRP est enregistré (`FlightLog.chirp_excitation` : le firmware l'ajoute à la consigne
   avant le mode angle, `flight/pid.c`, la commande radio ne la contient donc pas). L'estimateur direct S(u, gyro)/S(u, u)
   est biaisé en boucle fermée : les perturbations (vent, prop wash) font bouger u et le gyro ensemble ;
4. spectres lissés sur 20 % de la fréquence (au moins 5 points, moyenne glissante centrée) ;
5. bande valide : points **contigus** depuis 2 Hz, jusqu'à min(200 Hz ; 0,4 × fréquence du log), tant que
   l'erreur aléatoire de Ĝ reste ≤ 0,2 (≈ ±1,6 dB, ±11°) et que les cohérences erreur → P+I et gyro → D
   restent ≥ 0,8. Erreur aléatoire relative d'une fonction de transfert estimée avec la cohérence γ² sur
   n moyennes : √((1 − γ²)/(2·n·γ²)) (formule classique de l'analyse spectrale, Bendat et Piersol,
   *Random Data*, chap. 9 ; non vérifiée à la page), cumulée sur instrument → gyro et instrument → u, avec
   n = fenêtres × largeur du lissage / 2. Une cohérence gyro → D faible signale aussi un **biais** de Ĉd
   (bruit du gyro replié par le sous-échantillonnage du log, absent du D) ;
6. marges de L (ci-dessous) sur la bande valide ; rapport |Ĉd/Ĉe| au croisement (le D domine au-delà
   de 1) ; incertitude de chaque marge par **jackknife** sur 8 blocs (écart-type, barres d'erreur de
   l'interface).

La bande s'arrête là où le manche n'apporte plus assez d'énergie (souvent 20 à 40 Hz) : le croisement
y est en général, le pic de sensibilité souvent au-delà (valeur alors minorante). Contrôles (`LoopEstimate`) :
variante A, S = 1 − T·(Ce + Cd)/(Ce + Cf) ; variante C, Ce et Cd estimés eux aussi par l'instrument ;
phase de Ĉd comparée au modèle de l'en-tête `loop.dterm_model` (dérivée discrète × filtres du D,
`bf_filters.dterm_chain`). Résultats : `validation.md`.

**Méthode « approchée »** (repli) : L = T/(1 − T), sur les points de T décrits plus haut. Juste seulement
si tout le régulateur agit sur l'erreur. Pour Betaflight, T = G·(Ce + Cf)/(1 + L), donc
T/(1 − T) = G·(Ce + Cf)/(1 + G·(Cd − Cf)), qui n'est pas L = G·(Ce + Cd). Sur simulation sans feedforward, elle place le croisement jusqu'à 4 fois trop bas et
surestime la marge jusqu'à 31°, d'autant plus que le D domine ; elle fait croire que plus de D augmente
toujours la marge. Utilisée
quand un terme de gain non nul manque dans le log, ou quand la mesure n'a pas assez de points valides
(raison dans `LoopMetrics.notes`).

Marges (`analysis/loop.py:margins`) : phase déroulée (`np.unwrap`, premier point ramené dans
(−360°, 0]) ; croisement = premier passage de |L| au-dessous de 1, interpolé en (log f, log |L|) ; marge de
phase = 180° + ∠L, négative si la phase y est sous −180° ; marge de gain = −20·log10 |L| au premier passage
de la phase par −180° (− k·360°) après le croisement ; pic de sensibilité Ms = max |S| en dB (repères
4 dB jaune / 6 dB rouge, comme PTB), signalé « au bord de la bande » s'il tombe sur le premier ou le
dernier point (ce n'est alors qu'un minorant). Les croisements suivants sont ignorés : sur les vrais logs
ils viennent du bruit (`validation.md`).

## Spectres (`analysis/spectral.py`)

- **Spectre 2D** : Welch, fenêtre de Hann, recouvrement 50 % ; longueur de fenêtre fixée par la résolution
  (0,5 Hz par défaut, soit 2 s ; choix 0,5 / 1 / 2 / 4 Hz). DSP unilatérale en `10·log10`. Lissage de
  niveau 0 à 5 (largeur 1 + 2 × niveau). Échelle de PTB : −50..20 dB.
- **Fréquence × gaz** : fenêtres de 300 ms, pas de 300 / {5, 3, 1} ms (log ≤ 20 s, 20–60 s, > 60 s), gaz moyen
  par fenêtre, 100 tranches de gaz avec ±1 % de recouvrement, Hann, amplitude ou DSP moyennée par tranche,
  résolution 3,33 Hz. Affichage : lissage gaussien σ = (1,25 × niveau ; 0,5 × niveau) qui ignore les
  tranches vides (niveaux 0, 1, 2, 3, 4, 6) ; palette « hot » ; échelle amplitude 0–0,5, DSP −40..10 dB.
  Moyenne et pic annotés sur 100–1000 Hz (ou 20–60 Hz en mode « < 100 Hz »). Axe X en % de gaz.
- Préréglages (PTB) : 1 Gyro pf | Gyro | D pf | D ; 2 Gyro pf | Gyro | P | D ; 3 Gyro | D | Consigne | Erreur PID ;
  4 A|A|B|B Gyro pf | Gyro ; 5 A|A|B|B D pf | D ; 6–9 A|B|C|D Gyro pf / Gyro / D / Erreur PID.
- **Fréquence × temps** : fenêtres centrées de 300 ms, pas de 300 / {1, 2, 5, 10} ms, DSP, lissage en fréquence
  aucun / 5 / 10 / 20 points (largeurs réelles 1 / 5 / 11 / 21), palette « hot », Z −30..10 dB.
  Superpositions : harmoniques moteur (eRPM) et centres des notchs dynamiques (debug FFT_FREQ).
- **Bruit moteur** : moyenne ± écart-type de la DSP aux fréquences des harmoniques moteur (1re / 2e / 3e),
  en fenêtres de 300 ms, filtré contre préfiltre ; texte « poids | Q » du filtre RPM tiré de l'en-tête.
- **Hauteur des harmoniques** (`motors.harmonic_prominence`) : par fenêtre de 300 ms, DSP moyenne entre
  h × (moteur le plus lent) et h × (moteur le plus rapide) ± une case, moins la DSP moyenne de 15 à 30 % de
  part et d'autre de h × (fréquence moyenne) ; médiane des fenêtres. ≈ 0 dB sans raie.
- **Raies hors moteurs** (`motors.spectrum_lines`, vol stationnaire) : DSP moyenne des fenêtres de 300 ms,
  bandes des harmoniques 1 à 8 retirées (± 5 % de l'harmonique pour les trois premières, de la fréquence
  moteur ensuite, images repliées comprises), hauteur d'un maximum local au-dessus du bruit voisin (10 à
  20 % de part et d'autre), avant et après filtres ; une raie par groupe de ± 10 %.
- **Pics du notch dynamique** (`motors.dyn_notch_targets`) : gyro non filtré, bandes des harmoniques
  traitées par le filtre RPM retirées (± 5 %, image repliée comprise), DSP moyenne regroupée en cases de
  18,5 Hz ; pic suivi s'il dépasse 2 fois la puissance moyenne hors pics (au-dessus de 60 Hz), comme le
  firmware.
- **Estimation RPM** sans télémétrie : fréquence moteur ≈ multiplicateur (2,1) × % moteur (« RPM est. » de PTB),
  à calibrer par drone.

## Retards (`analysis/delay.py`)

- **Filtres gyro** (gyroUnfilt → gyroADC) et **D** (D préfiltre → terme D) : pente de la phase de
  H = Sxy/Sxx (Welch, fenêtres d'environ 0,5 s), entre 10 et 100 Hz, points de cohérence ≥ 0,6, moyenne
  pondérée par la cohérence, roll et pitch moyennés. L'intercorrélation est écartée : elle est dominée par
  le bruit haute fréquence.
- **Représentativité** : retard des filtres retenu seulement si la consigne dépasse 50 °/s au moins 5 % du
  temps (95e centile de |consigne|, roll ou pitch, `delay.stick_p95_dps`). En vol stationnaire, les filtres
  dynamiques (LPF dynamiques, notchs RPM et dynamiques) ne travaillent qu'au régime du stationnaire et il
  y a peu de signal entre 10 et 100 Hz (`validation.md`).
- **Consigne → gyro**, par axe : maximum de l'intercorrélation dans ±100 ms, interpolation parabolique sous
  l'échantillon ; axe ignoré si l'écart-type de la consigne est < 5 °/s.

## Autres outils

- Erreur PID : histogramme par axe, écart-type robuste (1,4826 × MAD) ; |erreur| moyenne selon la déflexion
  du manche ; limitée à |consigne| ≤ 100 °/s par défaut.
- Statistiques de vol : répartition des positions des manches et des gaz (% du vol), moyenne ± écart-type de
  |gyro|, |P|, |I|, |D|, |F| et des moteurs.
- Infos de configuration : tableau des réglages de l'en-tête de chaque log.

## Recommandations d'enregistrement
Log à 2 kHz (1 kHz suffit pour la réponse indicielle) ; debug FFT_FREQ (ou GYRO_SCALED en 4.4) ; vols de
20–30 s avec des coups de manche francs sur chaque axe pour la réponse indicielle ; stationnaire et
montées de gaz pour le bruit.

## Repères de réglage (indicatifs)

Chaque repère porte sa source entre crochets (liste ci-dessous) ou la mention **Estimation** : aucune
source publique trouvée (recherche du 2026-10-05). Les citations sont reprises mot pour mot, en version
originale ; pour les vidéos de la chaîne PIDtoolbox [S20]–[S25], d'après la transcription (sous-titres
automatiques, analysées dans le dépôt PTB-Videos-Analyse), avec le lien au moment cité.

- **Pic de réponse** : seules des règles de sens sont publiées. Plus de P → montée plus rapide, plus de
  dépassement ; plus de D → moins de dépassement, montée plus lente [S1, S4] ; rebond après les flips → plus
  de D [S3, S4]. Objectif : « fast rise time and peak time with not too much overshoot » [S1] ; UAV Tech vise
  une réponse sans dépassement (« critically damped ») [S4]. Méthode de Brian White : série de vols en
  faisant varier le curseur Damping, puis garder la première valeur sans pic de dépassement, jugée à l'œil
  [S24, S25]. Les paliers chiffrés qui circulent (1,02–1,08 optimal, > 1,20 plus de D…) n'ont aucune
  source, vidéos comprises : l'assistant utilise les percentiles du corpus, séparés selon le feedforward,
  et le pic de la première valeur sans dépassement de la série BlackSummer (1,03), voir `assistant.md`.
- **Fenêtres de la réponse indicielle** : consigne ≥ 20 °/s [S5, S2] ; séparation à 500 °/s [S5, S1].
  Réponse indicielle en mode angle pratiquement identique à l'acro [S24].
- **Bandes (5")** : < 20 Hz mouvements commandés, ~20–100 Hz propwash / oscillations [S1] ; 100–250 Hz
  résonance de châssis ou pièces desserrées, > 250 Hz moteurs et hélices [S6] (Brian White situait les
  moteurs vers 200–500 Hz [S7]). Propwash le plus souvent vers 30–50 Hz [S23].
- **Niveaux de bruit** (échelle en dB des spectres de PIDtoolbox, 6,5 dB sous notre densité spectrale,
  `validation.md`) : gyro filtré sous −30 dB, « a good rule of thumb »
  [S21, S22, repris par S6] ; terme D idéalement sous −20 dB, au plus −10 dB [S20] (S6 citait −10 dB).
- **Spectrogramme fréquence × gaz** : bruit moteur = ligne qui suit le gaz (diagonale) [S8, S6] ; résonance
  = fréquence fixe quel que soit le gaz [S6, S9] (ligne horizontale ou verticale selon l'axe des
  fréquences de l'outil).
- **Retard des filtres** : « aim for < 5ms total latency if possible » [S1, retard gyro estimé par
  PIDtoolbox par intercorrélation, roll seul : pas tout à fait notre méthode de phase] ; Brian White vise
  désormais 1,5 ms au plus pour le gyro filtré d'un 5", jusqu'à ≈ 5 ms pour un 10" (« rough
  guestimate ») [S20] ; son graphique « Rough guide to ideal filter delay » donne des droites de 1 ms à 2"
  jusqu'à 4 ms (gyro) et 3 ms (D) à 14", soit 1,75 / 1,5 ms pour un 5" [S26] (valeurs lues par PIDtoolbox,
  dont le retard D est environ 0,5 ms sous le nôtre, `validation.md`) ; 4–5 ms « raisonnable » pour un 5"
  [S10] ; 7–8 ms = filtrage très lourd [S11].
- **Fréquence fondamentale des moteurs** : régime en charge ≈ 70 % du régime à vide KV × cellules × 4 V
  [S12], soit ≈ KV × tension nominale × 0,75. Tableau par taille (whoop 65–75 mm 550–800 Hz en
  stationnaire / 1,1–1,4 kHz plein gaz ; 2,5–3,5" 350–450 / 800–950 ; 5" 200–270 / 500–650 ; 7" 130–170 /
  350–420 ; 10" 85–110 / 250–300) : **Estimation** déduite de la formule, aucune source ; trop haut pour les
  whoops. Mesuré sur le corpus (médiane par drone en vol) : AIR65 ≈ 480 Hz, Meteor 75 ≈ 355 Hz,
  3" ≈ 325 Hz, 5" ≈ 210 Hz, 7" ≈ 140 Hz, 9" ≈ 110 Hz (`assistant.md`, « Taille du drone »). Les whoops
  demandent un log rapide. Pôles : 14 par défaut, « Smaller motors have fewer magnets, often 12 » [S9] ;
  12 pour les 08XX à 16XX, 14 pour les 22XX à 24XX [S13].
- **Notch dynamique** : avec filtre RPM, Q 300–450 [S19] et 1–2 notchs [S14] (conseil de 2022 : 1 notch,
  Q 500 [S9]) ; sans filtre RPM, 3–4 notchs, Q 120–150 [S19] (4–5 notchs selon les pages de 2025–2026
  [S14]). Sur les grands drones (7" et plus), `dyn_notch_min_hz` plus bas que le défaut [S15] ; les presets
  officiels 7–10" restent entre 80 et 125 Hz [S16] (20 Hz n'est que la limite du firmware). Sur un 5", pas
  de filtrage plus bas que ≈ 100 Hz [S22].
  `rpm_filter_q` : 500 par défaut [S17] ; le monter tant que le bruit moteur ne réapparaît pas, sans
  dépasser 750 [S9] ou 1000 [S6, S14] selon les sources ; Brian White conseille 750 à 1000 sur un 5"
  propre pour réduire le retard [S20] ; documentation de Betaflight 4.5 : avec des tripales, monter Q par
  pas de 500 jusqu'à 1000 au plus, puis baisser les poids de la 2e et de la 3e harmonique (« the second
  almost invisible ») pour n'avoir que juste assez de filtrage [S27] ; presets officiels [S28].
- **Feedforward** (Betaflight 4.3 et plus) : liaison 250 Hz → `feedforward_smooth_factor` 40–50 et
  `feedforward_averaging` 2_POINT (35–40 ailleurs sur la même page ; preset officiel 45) ; 500 Hz → 65–70
  [S19, S18] ; `feedforward_jitter_factor` 12–15 « for freestyle/cinematic » [S19].

### Sources des repères

- [S1] Brian White, *PIDtoolbox user guide*, wiki GitHub, modifié le 2022-02-26 (texte déjà présent en
  2019). Dépôt supprimé : lu sur [l'archive](https://web.archive.org/web/20220422084353/https://github.com/bw1129/PIDtoolbox/wiki/PIDtoolbox-user-guide).
  « higher P gains will produce more overshoot, faster rise time » ; « ~20Hz to ~100Hz […] propwash » ;
  « aim for < 5ms total latency ».
- [S2] Brian White, code de PIDtoolbox v0.55 (2022-03-30, archive Wayback) : `minInput = 20;` sur des
  fenêtres de 2 s (`PTstepcalc.m`).
- [S3] Wiki Betaflight, [PID Tuning Guide](https://betaflight.com/docs/wiki/guides/current/PID-Tuning-Guide)
  (sans auteur ni date ; historique 2023–2026) : « bounce back, increase D ».
- [S4] Mark Spatz (UAV Tech), [*UAV Tech PID Tuning Principles*](https://theuavtech.com/wp-content/uploads/2020/10/UAV-Tech-PID-Tuning-Principles.pdf),
  PDF du 2020-10-26 : réponse visée « Critically damped ».
- [S5] Florian Melsheimer (Plasmatree), [PID-Analyzer](https://github.com/Plasmatree/PID-Analyzer), 2018 :
  « above and below 500 deg/s » ; code :
  `self.low_high_mask(self.max_in, 20)` (fenêtres sous 20 °/s ignorées).
- [S6] Oscar Liang, [*How to Tune FPV Drone Filters & PID with Blackbox*](https://oscarliang.com/pid-filter-tuning-blackbox/),
  2024-06-27 (modifié 2026-03-03) : « 100Hz – 250Hz: Frame resonance » ; « Above 250Hz: Noise from
  motors » ; « below -30dB above 50Hz » et « below -10dB » pour le D, attribués à l'auteur de PIDtoolbox
  (gyro filtré : réponse de l'auteur dans les commentaires) ; `rpm_filter_q` : « Do not exceed 1000 ».
- [S7] Brian White, README de PIDtoolbox, 2019 (copie [ianrmurphy/PIDtoolbox](https://github.com/ianrmurphy/PIDtoolbox)) :
  « particularly in the ~200-500Hz range associated with motor vibration ».
- [S8] Plasmatree, wiki [Noise analysis](https://github.com/Plasmatree/PID-Analyzer/wiki/Noise-analysis),
  2018-06-06 : « The diagonal 'line' is the motor noise ».
- [S9] Betaflight, guide [DShot RPM Filtering](https://betaflight.com/docs/wiki/guides/current/DSHOT-RPM-Filtering)
  (ctzsnooze, époque 4.1–4.3) : « fewer magnets, often 12 » ; notch dynamique « Count to 1 and Q to 500 » ;
  `rpm_filter_q` « Above 750 is not recommended ».
- [S10] UAV Tech, [Filter Calc](https://theuavtech.com/filter-calc/), 2020 : « A reasonable amount of filter
  delay is 4ms to 5ms for a 5″ quad ».
- [S11] Wiki Betaflight, [Gyro and Dterm Filtering Recommendations 3.1](https://betaflight.com/docs/wiki/guides/archive/Gyro-And-Dterm-Filtering-Recommendations-3-1)
  (archive, vers 2017) : « 7-8ms delay from strong filtering ».
- [S12] ctzsnooze, [4.2 Tuning Notes](https://betaflight.com/docs/wiki/tuning/4-2-Tuning-Notes) (vers 2020) :
  LPF2 du gyro anti-repliement, « With a 2k PID loop, don't disable gyro lowpass 2 » ;
  « 70% of the peak theoretical no-load rpm ».
- [S13] Oscar Liang, [*How to Setup RPM Filters in Betaflight*](https://oscarliang.com/rpm-filter/),
  2023-03-31 : « 14 poles in 22XX, 23XX, 24XX motors 12 poles in 08XX, 11XX, 12XX, 13XX, 14XX, 16XX motors ».
- [S14] Betaflight, [référence CLI](https://betaflight.com/docs/wiki/guides/current/Cli) (Bob Vandevliet,
  2026-03-16) : « 1–2 notches » avec filtre RPM, « 4–5 » sans ;
  même conseil sur la page de l'onglet PID (Yaros, 2025-02-15).
- [S15] ctzsnooze, 4.2 Tuning Notes [S12] : pour les drones lents et la classe X, « a lower `dyn_notch_min_hz`
  than defaults ».
- [S16] Mark Spatz, presets officiels UAV Tech 6–7", 8–9" et 10" ([firmware-presets](https://github.com/betaflight/firmware-presets), 4.5, 2024-07-25) :
  `dyn_notch_min_hz` 80 à 125.
- [S17] Firmware Betaflight, `pg/rpm_filter.c` : `.rpm_filter_q = 500`.
- [S18] ctzsnooze, presets officiels ELRS 250 Hz et 500 Hz (`presets/4.3/rc_link/`, firmware-presets,
  2021-2022) : `feedforward_smooth_factor` 45 (250 Hz) et 65 (500 Hz), `feedforward_averaging = 2_POINT`.
- [S19] ctzsnooze, [4.3 Tuning Notes](https://betaflight.com/docs/wiki/tuning/4-3-Tuning-Notes),
  2021-07-16 à 2022-02-12 : LPF2 du gyro « absolutely essential » à 2 kHz, « strongly recommend » à
  4 kHz ; biquad remplacé par le PT2 (+60 % de retard pour un PT2, ×2 pour un PT3 face au PT1) ; notch dynamique, « often only one » (souvent un seul notch), minimum « as high
  as practical (eg 200hz) », plage qui englobe la résonance avec « 20hz additional range each side » ;
  « Q values around 300-450 » ; sans filtre RPM, « three or four notches and Q
  values around 120-150 » ; « For 250hz RC links […] 40-50 », « For 500hx RC links […] 65-70 »
  (`feedforward_smooth_factor`) ; jitter « 12-15 […] for freestyle/cinematic ».
- [S20] Brian White, chaîne YouTube PIDtoolbox, [*PTB Q&A #16 : Filter tuning for performance*](https://www.youtube.com/watch?v=TiwaQEkOdyo),
  2026-04-29 : D-term « ideally below about minus 20 dB. Maybe minus 10 would be the highest »
  ([5:28](https://www.youtube.com/watch?v=TiwaQEkOdyo&t=328s)) ; « you could dial this back to say Q of
  750 » ([34:12](https://www.youtube.com/watch?v=TiwaQEkOdyo&t=2052s)) ; « For a 5-in drone, gyro filter
  delay of 1.5 milliseconds or less » ([35:52](https://www.youtube.com/watch?v=TiwaQEkOdyo&t=2152s)) ;
  « 5-in 1 and 1/2 milliseconds up to 10-in 5 » ([40:57](https://www.youtube.com/watch?v=TiwaQEkOdyo&t=2457s)).
- [S21] Brian White, direct d'analyse de logs (suite), [vidéo du 2022-01-24](https://www.youtube.com/watch?v=14wlm7yhrOY) :
  « as long as the filter gyro is below minus 30 it's going to be pretty clean » ; « a good rule of thumb »
  ([51:47](https://www.youtube.com/watch?v=14wlm7yhrOY&t=3107s)).
- [S22] Brian White, direct d'analyse de logs, [vidéo du 2022-01-17](https://www.youtube.com/watch?v=5bBFlnCTsNM) :
  bruit « below minus 30 » sur un simple balayage de gaz ([7:34](https://www.youtube.com/watch?v=5bBFlnCTsNM&t=454s)) ;
  « in a five inch […] don't want to go lower than say 100 » ([1:12:27](https://www.youtube.com/watch?v=5bBFlnCTsNM&t=4347s)).
- [S23] Brian White, [*PTB Q&A #1*](https://www.youtube.com/watch?v=LO7ZuliPsos), 2021-04-02 : « prop wash
  oscillation is happening around 30 to 50 hertz » ([15:26](https://www.youtube.com/watch?v=LO7ZuliPsos&t=926s)).
- [S24] Brian White, [réglage en mode angle](https://www.youtube.com/watch?v=sqT4MACi3d8), 2022-02-14 :
  acro et angle « for all intents and purposes, these curves are identical »
  ([2:56](https://www.youtube.com/watch?v=sqT4MACi3d8&t=176s)) ; Damping 0,6 à 1,4, « somewhere between 1.2
  to 1.4 is optimal » ([3:40](https://www.youtube.com/watch?v=sqT4MACi3d8&t=220s)).
- [S25] Brian White, [réglage Betaflight avec PIDtoolbox](https://www.youtube.com/watch?v=ehvQm8Rqrzk),
  2025-09-04 : Damping 0,6 à 1,2, « the 1.2 is even really overdampened […] 0.8 or 1.0 is fine »
  ([13:30](https://www.youtube.com/watch?v=ehvQm8Rqrzk&t=810s)).
- [S26] Brian White, graphique « Rough guide to ideal filter delay » (axe X « Drone size (inch) » de 2 à
  14, axe Y « Target Filter Delay (msec) » ; droites « gyro » de 1 à 4 ms et « dterm » de 1 à 3 ms),
  publié sur le serveur Discord de PIDtoolbox, date non relevée ; capture transmise par Cédric le
  2026-10-05. Brian White l'évoque dans [S20] ([40:24](https://www.youtube.com/watch?v=TiwaQEkOdyo&t=2424s) :
  « I made this little chart […] purely out of experience »).
- [S27] Documentation Betaflight, [*4.5 Release Notes*, § 10 « Dimmable RPM Harmonics »](https://betaflight.com/docs/wiki/release/Betaflight-4-5-Release-Notes)
  (source `betaflight.com`, commit 711c78d du 2026-06-26, [PR 12838](https://github.com/betaflight/betaflight/pull/12838)) :
  « A weight of 100 applies the filter at full strength, while 0 means 'completely off'. The Q factor of
  the RPM filter still sets the 'width' » ; tripales : « Typically the first is strongest, the second
  almost invisible, and the third is a bit less than the first » ; exemple `100, 0, 80`, bipales
  `100, 80, 0` ; « It's best to have only just enough RPM filtering that the motor harmonic lines are only
  just not visible in a D or motor trace spectrum » ; « stepwise increasing the Q from the default of 500
  up to a max of 1000, and checking the noise at each step ».
- [S28] Presets officiels de Betaflight (dépôt `firmware-presets`, commit ac721ff du 2026-09-23),
  `rpm_filter_weights` en 4.5 et plus : AOS RC « RPM Filter Weights for triblade props » 100, 0, 80
  (`presets/4.5/tune/aos_rc/aos_ul5_tune.txt`) ; Karatebrot, auteur de la fonction, « 3 blade prop »
  100, 20, 100 et « 2 blade prop » 100, 100, 50 (`presets/4.5/tune/karate/karate_brushless_whoop.txt`) ;
  SupaflyFPV 90, 30, 90 et 90, 50, 90 avec Q 700–800.
- [S29] Configurateur Betaflight, aides de l'onglet PID / filtres (`locales/en/messages.json`, commit
  a38c4a79 du 2026-10-05) : LPF1 du gyro « can usually be disabled completely in firmware 4.3 when RPM
  filtering is used » ; LPF2 du gyro « should be retained to prevent aliasing » quand la boucle PID est
  plus lente que le gyro ; D : « At least two PT1 lowpass filters, or a single second order filter, are
  required » ; LPF1 du D statique « not recommended for D filtering ».
- [S30] Documentation Betaflight, [*DSHOT RPM Filtering*](https://betaflight.com/docs/wiki/guides/current/DSHOT-RPM-Filtering) :
  si les moteurs restent froids, « move gyro lowpass filtering to higher values », « disable gyro lowpass
  1 entirely », un filtre à la fois, en vérifiant la température des moteurs.
