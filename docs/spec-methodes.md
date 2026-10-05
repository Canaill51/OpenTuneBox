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

Mesures de boucle (conception propre, même déconvolution) : T(f) = réponse en fréquence en boucle fermée
consigne → gyro (moyenne des interspectres / autospectres des fenêtres), de 2 à 150 Hz, points de
cohérence ≥ 0,5, lissage complexe sur 5 points (`uniform_filter1d(mode="nearest")`) ; sensibilité
S = 1 − T, pic de |S| en dB et sa fréquence (repères 4 dB jaune / 6 dB rouge, comme PTB) ; boucle
ouverte L = T/(1 − T) : croisement où |L| = 1, marge de phase = 180° + ∠L ; bande passante = −3 dB de |T|,
cherchée à partir de 2 Hz.
**Limite** : L = T/(1 − T) n'est la vraie boucle ouverte que si tout le régulateur agit sur l'erreur ; dans
Betaflight le D agit sur le gyro et le feedforward sur la consigne, donc S et la marge de phase sont
approchées (détail et piste de correction : `assistant.md`, « Limite connue »).

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
