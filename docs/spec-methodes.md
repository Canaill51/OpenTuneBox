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
   de 20 % de la période (la consigne dépend alors de l'attitude). L'estimateur direct S(u, gyro)/S(u, u)
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
- **Estimation RPM** sans télémétrie : fréquence moteur ≈ multiplicateur (2,1) × % moteur (« RPM est. » de PTB),
  à calibrer par drone.

## Retards (`analysis/delay.py`)

- **Filtres gyro** (gyroUnfilt → gyroADC) et **D** (D préfiltre → terme D) : pente de la phase de
  H = Sxy/Sxx (Welch, fenêtres d'environ 0,5 s), entre 10 et 100 Hz, points de cohérence ≥ 0,6, moyenne
  pondérée par la cohérence, roll et pitch moyennés. L'intercorrélation est écartée : elle est dominée par
  le bruit haute fréquence.
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

## Repères de réglage (indicatifs, confiance moindre)

- Pic de réponse : < 1,00 suramorti ; 1,02–1,08 optimal ; 1,08–1,12 acceptable ; 1,12–1,20 ajouter ~10–30 %
  de D ; > 1,20 ajouter davantage de D ; > 1,5 suspecter la mécanique. Rebond après les flips → plus de D.
  P plus élevé = plus rapide, plus de dépassement.
- Bandes (5") : < 20 Hz mouvements commandés ; 20–100 Hz propwash / oscillation ; 100–250 Hz résonance de
  châssis / pièces desserrées ; > 250 Hz moteurs / hélices. Plancher de bruit du gyro < −30 dB au-dessus de
  50 Hz ; terme D < −10 dB (DSP de BBE). Retard total des filtres idéalement < 5 ms. Résonance = ligne
  horizontale sur fréquence × gaz ; moteur = diagonale.
- Fréquence fondamentale des moteurs selon la taille (physique au premier ordre, RPM ≈ KV · V · 0,75) :
  whoop 65–75 mm 550–800 Hz en stationnaire / 1,1–1,4 kHz plein gaz ; 2,5–3,5" 350–450 / 800–950 ;
  5" 200–270 / 500–650 ; 7" 130–170 / 350–420 ; 10" 85–110 / 250–300. Mesuré sur le corpus (médiane par drone en vol) :
  AIR65 ≈ 480 Hz, Meteor 75 ≈ 355 Hz, 3" ≈ 325 Hz, 5" ≈ 210 Hz, 7" ≈ 140 Hz, 9" ≈ 110 Hz : les valeurs
  « stationnaire » ci-dessus sont trop hautes pour les whoops (`assistant.md`, « Taille du drone »). Les whoops demandent un log rapide ;
  sur les 7" / 10", la bande moteur recouvre le propwash (notch dynamique min jusqu'à 20 Hz).
- Notch dynamique avec filtre RPM : Q 300–450, 1–2 notchs ; sans RPM : 3–4 notchs, Q 120–150.
- Feedforward : liaison 250 Hz → smooth 40–50, 2 points ; 500 Hz → 65–70 ; jitter 12–15 pour le cinématique.
