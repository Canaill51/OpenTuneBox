# Écrans de PIDtoolbox Pro V0.94 (d'après les 638 captures exportées par Cédric)

Chemins relatifs à `~/Documents/Drones/Logs/`. Les exports sont des captures de fenêtre entière
`PTB_FIGS_AAAA-MM-JJ-N.png`. Écrans visibles : visualiseur de log (Log Viewer), analyse spectrale
(Spectral Analyzer, 2 modes pour le volet droit), Freq x Throttle, Freq x Time, réponse indicielle
(Step Response, avec la vue des curseurs BF). Les fenêtres PID slider tool, Filter Sim et Setup Info
n'ont jamais été exportées (seuls leurs boutons sont visibles) : leur interface a été conçue pour
OpenTuneBox. Les modules de PTB (binaires chiffrés) citent aussi : PTplotFlightStats, PTdispSetupInfo,
PTplotFilterSim, PTpidSliderTool, PTcalcDelays, PTgetBandwidth, PTB_AIAssistant_*.

Conventions : fond gris, axes blancs avec grille ; couleurs des fichiers comparés rouge, orange, jaune,
vert clair, cyan, bleu, bleu foncé… (10 fichiers au maximum) ; couleurs des moteurs M1/5 rouge, M2/6 orange,
M3/7 bleu, M4/8 cyan. Chaque outil a : Run, Reset, Save Fig, Save Settings, Markup, Quick Guide,
« select files (max 10) ». `PTBdefaults.txt` garde les réglages d'interface (Ymax du visualiseur 500,
épaisseur de trait 3, lissage spec2D 3, préréglages / palette / lissage de Freq x Throttle, lissage de
Freq x Time, Ymax de la réponse indicielle 1,5, thème, police…).

## 1. Visualiseur de log (Log Viewer)
- Barre de défilement du temps en haut ; firmware + carte en haut à droite. 3 axes empilés R/P/Y (deg/s,
  échelle ± 500 par défaut), 4e axe « Throttle | Motor (%) » 0–100. Modes de vol sous l'axe du temps
  (« |ARM|ANGLE »).
- Grisé hors de la période d'analyse ; curseur du lecteur vertical en tirets.
- Le bouton radio R/P/Y agrandit l'axe choisi sur toute la hauteur (le panneau des gaz reste).
- Panneau de sélection : échelle Y ; cases Gyro(pf), Gyro, P-term, I-term, D-term(pf), D-term, F-term,
  Set point, PID sum, PID error, Motor 1/5..4/8, RPS 1/5..4/8, Throttle, Debug ; texte du mode debug.
  Couleurs : gyro noir, consigne rouge, P vert, I bleu, D(pf) olive, D jaune, F brun-rouge, somme PID violet,
  erreur PID cyan. Par défaut : Gyro, Set point, Motors, Throttle.
- Panneau de commande : Open / Restore / Reset, liste des fichiers, radios R/P/Y, Period, Markup, couleur
  de trait, police, lissage, épaisseur de trait, thème, boutons Spectral Analyzer, Step Resp Tool,
  PID slider tool, Filter Sim, Setup Info, Save Fig, Save Settings, site web, Quick Guide.
- Lecteur : << < - > >>, « time: x sec », deux manches (T/Y à gauche, P/R à droite), % moteurs et gyro au curseur.
- Réf. : `1. PTB/Allykill/PTB_FIGS_2026-09-28/PTB_FIGS_2026-09-28-1.png`, `Cedric/BF Chirp/PTB_FIGS_2026-08-09/PTB_FIGS_2026-08-09-12.png`.

## 2. Analyse spectrale (Spectral Analyzer)
- Grille 3×2, lignes R/P/Y. À gauche « Full Spectrum » 0–Nyquist, DSP en dB −50..20 ; légende « <donnée> | <fichier> ».
- Volet droit : « Sub 100Hz » (0–100 Hz, texte rouge « Gyro: 1.25ms | Dterm: 1ms » = retards des filtres)
  ou « Motor Noise » (x = 1re/2e/3e harmonique, moyenne ± barres d'erreur, filtré en trait plein, préfiltre
  en pointillés, texte « Filter Weights | Q », p. ex. « 100,40,75% | Q800 »).
- Filtré en trait plein épais, préfiltre en pointillés de même couleur. Superpositions en courbes en S
  (type fonction de répartition) montant jusqu'à 0 dB : harmoniques de chaque moteur (1re pleine, 2e en tirets,
  3e en pointillés), répartition des notchs dynamiques, plage des notchs RPM.
- Commandes : liste de fichiers ×10 ; données Gyro, Gyro prefilt, Dterm, Dterm prefilt, Pterm, PID error,
  Set point, Fterm, PIDsum, Motors, debug ; Run, Reset ; boutons Freq x Throttle, Freq x Time ; listes
  retard des filtres et lissage ; R/P/Y ; PSD ; Y min/max ; cases moteurs ; liste des harmoniques
  (1st, 2nd, 3rd / RPM notch) ; liste Dyn notch ; épaisseur de trait ; mode du volet droit ;
  estimation RPM + multiplicateur 2,1.
- Réf. : `1. PTB/Allykill/PTB_FIGS_2026-09-28/PTB_FIGS_2026-09-28-3.png` (sub100), `-6.png` (bruit moteur).

## 3. Freq x Throttle
- Jusqu'à 4 colonnes, chacune avec ses listes donnée + fichier et une case « <100Hz » ; lignes R/P/Y.
  Carte de chaleur x « % Throttle » 0–100, y fréquence 0–Nyquist, palette hot, barre de couleur horizontale
  0..échelle (0,5), texte blanc « mean=… », « peak=… ».
- Paramètres : Run, Reset, fichier, Save, Presets, Markup, palette, lissage, radios « 2D » / « PSD »,
  limites de fréquence 20 / 60 Hz, Quick Guide.
- Réf. : `1. PTB/schteven/PTB_FIGS_2026-07-27/PTB_FIGS_2026-07-27-3.png`.

## 4. Freq x Time
- 3 spectrogrammes R/P/Y, x temps (s), y fréquence, palette hot, barre de couleur « Power Spectral density (dB) »
  Zmin −30 / Zmax 10.
- Superpositions : harmoniques moteur à la couleur de chaque moteur (1re pleine, 2e en tirets, 3e en pointillés),
  centres des notchs dynamiques (rouge).
- Commandes : fichier, donnée, Run/Reset, lissage en fréquence, lissage en temps, palette, épaisseur de trait,
  R/P/Y, <100Hz, Z min/max, cases moteurs, liste des harmoniques, notchs dynamiques, estimation RPM +
  multiplicateur, radios Player/Traces, < - >.
- Réf. : `Cedric/JeNo 5/PTB_FIGS_2026-07-27/PTB_FIGS_2026-07-27-8.png`.

## 5. Réponse indicielle (Step Response)
- À gauche, 3 lignes « Step Response Functions » : y 0..Ymax (1,5), ligne en tirets à 1, x 0–500 ms ;
  courbe moyenne par fichier ; « Raw » montre tous les segments.
- Texte par ligne : en-tête « P, I, D, Dm, F », par fichier « 1) 45,15,17,17,0 (n=391) » ou « insufficient data ».
- Deux colonnes de mesures (listes) en fonction du numéro de test, avec barres d'erreur : SR Peak, SR Latency,
  SP-Gyro Delay, Sensitivity Peak (dB ; lignes à 4 et 6 dB), Sensitivity Peak Freq, Phase Margin,
  Gain Crossover Freq, Rise Time, Bandwidth.
- Vue « BF sliders » : catégories Damping, Tracking, Stick Resp., Dynamic D., Drift-Wobble, Pitch D., Pitch T.,
  Master ; x position du curseur 0–2 ; un point par fichier ; « MODE: RPY ».
- Commandes : fichiers ×10, Run/Reset, Save, Period, Markup, lissage, listes des mesures, R/P/Y, Expand, Raw,
  BF sliders, Y corr., Y max, liste « 2sec deconv. window ».
- Valeurs observées : latence 13–22 ms ; retard consigne→gyro 12–21 ms ; pic 1,0–1,22 ; marge de phase ~66° ;
  croisement ~11 Hz ; montée 15–22 ms ; bande passante 14–20 Hz.
- Réf. : `1. PTB/BlackSummer/PTB_FIGS_2026-08-16/PTB_FIGS_2026-08-16-1.png`, `-4.png` (curseurs),
  `Cedric/JeNo 5/PTB_FIGS_2026-08-09/PTB_FIGS_2026-08-09-2.png`..`-4.png`.

## Référence pour les infos de configuration
Les captures d'en-têtes Blackbox Explorer des utilisateurs montrent les groupes que regardent les régleurs :
PID, rates, filtres (fréquences des LPF, Q / min / max du notch dynamique, Q / min du RPM), RC smoothing,
moteurs, mode debug, curseurs.
