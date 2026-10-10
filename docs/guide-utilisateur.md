# OpenTuneBox — guide d'utilisation

## 1. Installer

Télécharger le fichier de son système sur la page **Releases** du dépôt GitHub (colonne de droite de la
page d'accueil), section *Assets* de la dernière version. Un seul fichier par système ; Python et les
bibliothèques sont inclus.

**Windows 10 / 11** : `OpenTuneBox-Windows-Setup.exe`. Double-cliquer, puis suivre l'assistant ; en
option, raccourci sur le Bureau et ouverture des `.bbl` / `.bfl` par double-clic. Pas besoin des droits
administrateur (installation dans le dossier de l'utilisateur). Une nouvelle version s'installe
par-dessus l'ancienne ; désinstallation : *Paramètres → Applications → OpenTuneBox → Désinstaller*.
Le programme n'est pas signé : au premier lancement de l'installateur, SmartScreen peut afficher
« Windows a protégé votre ordinateur » → **Informations complémentaires → Exécuter quand même**.

**macOS** (Mac à puce Apple : M1 et suivants) : `OpenTuneBox-macOS.dmg`. L'ouvrir, puis glisser
**OpenTuneBox** sur **Applications**. L'application n'est pas signée par Apple : au premier lancement,
macOS refuse de l'ouvrir. Aller dans *Réglages Système → Confidentialité et sécurité*, en bas :
**Ouvrir quand même** (une seule fois). Un `.bbl` / `.bfl` s'ouvre ensuite dans OpenTuneBox par
double-clic (*Ouvrir avec* la première fois).

**Linux** (Ubuntu 22.04, Debian 12, Fedora 36… ou plus récent) : `OpenTuneBox-Linux.AppImage`, un seul
fichier. Le rendre exécutable (clic droit → *Propriétés* → *Autoriser l'exécution*, ou
`chmod +x OpenTuneBox-Linux.AppImage`), puis double-clic. S'il ne démarre pas (système sans FUSE) :
`./OpenTuneBox-Linux.AppImage --appimage-extract-and-run`.

Lancer OpenTuneBox depuis ses sources ou fabriquer l'exécutable soi-même : voir la partie
*Développement* du [README](../README.md#développement).

## 2. Ouvrir des logs

Bouton bleu **Ouvrir** de la barre d'outils (ou Ctrl+O), glisser-déposer dans la fenêtre, ou double-clic
sur un `.bbl` / `.bfl` (Windows et macOS, voir ci-dessus). Sous Linux, la fenêtre d'ouverture est celle
du bureau (KDE, GNOME…). Chaque session d'un fichier (un armement = une session) apparaît comme un log
séparé ; 10 logs au maximum, chacun avec sa couleur, que l'on retrouve dans tous les outils.

## 3. Bien enregistrer

Dans le CLI Betaflight :

```
set blackbox_sample_rate = 1/2      # 2 kHz avec une boucle PID à 4 kHz (1/4 à 8 kHz, 1/2 à 3,2 kHz → 1,6 kHz)
set debug_mode = FFT_FREQ           # centres des notchs dynamiques + gyro avant notch
save
```

- **Au moins 2 kHz** avec un gyro à 8 kHz, **au moins 1,6 kHz** avec un gyro à 3,2 kHz (BMI270) : le
  blackbox garde un échantillon sur N sans filtre anti-repliement, tout bruit au-delà de la moitié de la
  fréquence du log se replie dans le spectre (fausses raies). Les petits drones demandent plus : il faut
  au moins 6 fois la fréquence des moteurs pour voir leur 3e harmonique (whoop à 480 Hz : 4 kHz).
- **Deux vols pour régler les filtres** : un vol stationnaire (régime stable, le meilleur pour repérer les
  harmoniques moteur, les résonances et le bruit restant) et un vol avec des mouvements francs (le seul où le
  retard des filtres se mesure bien).
- En **4.4**, préférez `debug_mode = GYRO_SCALED` (seul moyen d'avoir le gyro non filtré).
- **DShot bidirectionnel** activé : sans lui, ni filtre RPM ni fréquences moteur mesurées.

### Vols de test (20 à 40 s chacun)

| But | Comment voler |
|---|---|
| Bruit, filtres, résonances | Stationnaire puis montées lentes de gaz de 0 à 100 %, sans manœuvre brusque |
| Réponse PID (réponse indicielle) | Mouvements francs et rapides sur chaque axe, flips et rolls, retours au neutre nets ; inclure le yaw |
| Propwash | Descentes dans sa propre turbulence (split-S, gaz coupés puis remis) |
| Comparaison de réglages | Le même vol, la même batterie (charge similaire), un seul réglage changé entre deux logs |

## 4. Le visualiseur (fenêtre principale)

- **Barre d'outils** en haut : ouvrir et fermer les logs, puis un bouton par outil (Spectre, Fréq. × gaz,
  Indicielle…) ; l'icône tout à droite exporte la fenêtre en image. Survolez un bouton pour sa description.
- **Traces** à gauche, rangées par groupe (Gyro, Consigne et PID, Autres, Moteurs). Les traces que le log
  ne contient pas sont masquées.
- **Période d'analyse** : la zone claire, affichée sur les graphiques roll, pitch, yaw et moteurs (le reste
  du vol est grisé). Tous les outils n'analysent que cette zone. Elle est choisie automatiquement (du
  décollage à l'atterrissage, en s'arrêtant avant un crash) ; déplacez ses bords à la souris sur n'importe
  quel graphique pour l'ajuster (les autres suivent ; au survol, une info-bulle indique le bord et son
  temps), **Période auto** pour revenir au calcul automatique.
  Chaque log ouvert garde sa propre période.
- **Mesurer** : mesure l'écart de temps entre deux points, en millisecondes, et la fréquence correspondante
  en hertz (1 / écart), par exemple la période d'une oscillation. Enfoncez le bouton, puis cliquez deux
  points sur un graphique roll, pitch ou yaw : deux repères violets apparaissent sur les trois axes, et le
  résultat s'affiche en haut des graphiques et à côté du bouton. Glissez un repère pour ajuster ; un
  nouveau clic recommence la mesure ; désenfoncez le bouton pour l'effacer.
- **Lecteur** : curseur temporel, manches et valeurs à l'instant choisi ; ▶ pour rejouer le vol.
- **Affichage** (section repliable à droite) : lissage et épaisseur des traces, thème clair ou sombre.
  Les sections Affichage et Lecteur se replient d'un clic sur leur titre.
- Zoom : molette ; déplacement : clic-glisser ; **Vue entière** pour tout revoir.
- **Tous / Roll / Pitch / Yaw** : un seul axe sur toute la hauteur.

## 5. Les outils

Chaque outil s'ouvre par son bouton dans la barre d'outils (nom du bouton entre parenthèses).

### Analyse spectrale (Spectre)
Spectres (FFT) du gyro et des termes PID : on y voit où se trouve le bruit et ce que les filtres
enlèvent (préfiltre en pointillés, filtré en trait plein). Volet « < 100 Hz » pour les oscillations et
le propwash, avec les **retards des filtres** (mesurables seulement si le vol contient des mouvements :
sur un simple vol stationnaire, les filtres dynamiques ne travaillent qu'à un régime et le retard affiché
est marqué « peu fiable ») ; volet « bruit moteur » pour l'efficacité du filtre RPM
sur chaque harmonique. Les courbes en S montrent où se situent les harmoniques moteur pendant le vol.

Case **Diagnostic du bruit** (cadre « Diagnostic », en bas du panneau de réglages) : ouvre, à côté des courbes, un rapport sur le bruit et la stratégie de
filtrage, pour chaque log choisi et sur la période d'analyse. Il est calculé seulement quand la case est
cochée (quelques secondes) et compte quatre parties :
1. **Conditions du log** : fréquence d'enregistrement suffisante, vol stationnaire (bon pour identifier le
   bruit) ou avec mouvements (nécessaire pour juger le retard des filtres).
2. **Gyro** : bruit avant et après filtrage, axe par axe (sommet du spectre et sa fréquence, baisse
   moyenne obtenue par les filtres), puis un bloc par filtre :
   - **Filtre RPM** : réglages, hauteur de chaque harmonique moteur avant et après filtres (effacée ou
     encore visible) ;
   - **Notch dynamique** : réglages, raie la plus marquée quel que soit le gaz, pics à suivre après le
     filtre RPM avec leur largeur et leur hauteur (d'où l'assistant calcule le Q conseillé) ;
   - **Passe-bas du gyro** : réglages et retard.

   Chaque bloc donne les conseils de l'assistant pour ce filtre ou, s'il n'y en a pas, un verdict :
   « rien à changer » avec la raison, ou « impossible à juger » avec ce qu'il faut enregistrer.
3. **Terme D** : même tableau (le D avant filtres est reconstruit à partir du gyro), puis le bloc
   **Passe-bas du D**.
4. **Stratégie de filtrage** : les réglages à faire, dans l'ordre, un à la fois.

Les constats et les seuils sont ceux de l'assistant : les deux fenêtres donnent les mêmes conseils.

### Fréquence × gaz (Fréq. × gaz)
Carte du bruit selon le gaz. Une **diagonale** qui monte avec le gaz = bruit moteur (normal, doit être
retiré par le filtre RPM). Une **ligne horizontale** = résonance de châssis ou pièce qui vibre. Une
tache basse fréquence à mi-gaz = oscillation de réglage ou propwash.

### Fréquence × temps (Fréq. × temps)
Le même bruit dans le temps : utile pour relier un bruit à un moment du vol (le curseur est lié au
lecteur ; un clic sur un spectrogramme le déplace).

### Réponse indicielle (Indicielle)
La réaction du drone à un ordre de manche « en marche d'escalier », moyennée sur tout le vol.
Objectifs : montée rapide, **pic proche de 1** (sans feedforward, une réponse sans dépassement visible donne
environ 1,03 ; le feedforward fait monter le pic), pas de rebond. Colonnes de mesures au choix :
latence, temps de montée, bande passante, pic de sensibilité (< 7 dB), marge de phase (> 35°), marge de
gain, fréquence de croisement…
Pour des marges plus sûres, un vol avec le chirp de Betaflight (2025.12 et plus, `debug_mode = CHIRP`)
excite le drone sur toute une plage de fréquences, en acro comme en mode angle : réglez sa fréquence de
fin (`chirp_frequency_end_deci_hz`) à au moins 600, soit 60 Hz, pour couvrir la zone où se joue la marge.
Les marges de stabilité sont **mesurées** à partir des termes P, I, D et F enregistrés dans le log (point
plein, barre d'erreur = incertitude de la mesure). Si un de ces termes manque, elles sont seulement
**approchées** (point creux) : à prendre avec prudence. Un triangle signifie « au moins cette valeur » :
le pic de sensibilité se trouve au-delà des fréquences que vos mouvements de manche permettent de mesurer.
Plus vous donnez de coups de manche francs et variés, plus la mesure va loin en fréquence.
« Vue curseurs Betaflight » : position des curseurs du réglage simplifié de chaque log.
Dans chaque graphique, en bas à droite : gains P, I, D, Dm, F de chaque fichier, nombre de fenêtres (n), pic,
latence et cassure. « Temps affiché » : 250 ms par défaut (la montée en grand), 500 ms comme PIDtoolbox, ou
150 ms pour bien voir la cassure. Le yaw est masqué au départ (case Y) : la méthode ne le juge pas et ses
mouvements suffisent rarement. La colonne « Cassure (ripple) » suit la cassure d'un vol à l'autre.

**Diagnostic du réglage**, toujours affiché à côté des courbes, d'après la méthode de Brian White. Chargez une **série de vols** du même drone, mêmes filtres, où un seul curseur change par pas
de 0,2 ; l'outil reconnaît la série tout seul et lit, selon le curseur :
- **Damping** (équilibre P/D) : jugé sur le **roll**, la première valeur sans dépassement visible (pic
  ≤ 1,05), ou celle au-delà de laquelle le pic ne baisse plus. Si le pitch dépasse encore, il a plus
  d'inertie : l'outil conseille alors de monter **Pitch D.** (son D à part) plutôt que le Damping ;
- **Pitch D.** : même lecture, sur le pic du pitch ;
- **Master** (niveau des gains) : on monte tant que la latence baisse sans **cassure** (« ripple » : la
  montée s'arrête un instant avant la consigne, puis reprend lentement) ni **vagues** sur le palier. Chaque
  vol est classé propre, premiers signes (proche de l'optimum) ou trop haut ; au premier vol trop haut, on
  garde la valeur d'avant ;
- **Pitch T.** (ou, sans série, le vol conseillé) : écart de latence entre pitch et roll, visé
  sous 1 ms, acceptable jusqu'à 2 ms selon le drone.

Conditions vérifiées pour chaque vol : feedforward à 0 en Acro, ou vol en **mode Angle** (le feedforward y
est coupé ; pas en Horizon), curseur I (Drift-Wobble) à 0,2–0,3, assez de mouvements.
Dans les colonnes de mesures, un cercle entoure les vols conseillés ; sur les courbes, un point creux marque
l'arrêt de la montée quand il y a une cassure.

### Curseurs PID
Positions des curseurs (lues dans le log, ou estimées si le réglage simplifié est désactivé) et
simulateur « et si » : bougez un curseur, voyez les PID et filtres obtenus, copiez les commandes CLI.

### Simulateur de filtres (Filtres)
Reconstitue exactement la chaîne de filtres de Betaflight à partir du log. Modifiez un filtre :
le programme montre la réponse (gain, phase), le retard ajouté et le bruit qui passerait, en rejouant
le vrai gyro du vol. Copiez ensuite les commandes CLI.

### Infos de configuration (Configuration)
Tous les réglages des logs côte à côte ; les différences sont surlignées (« Seulement les différences »).

### Statistiques de vol (Statistiques)
Utilisation des manches et des gaz, moyennes des termes PID, charge des moteurs, distribution de
l'erreur PID, batterie.

### Assistant de réglage (Assistant)
Diagnostic complet d'un log : qualité de l'enregistrement, réponse, bruit, filtres, moteurs,
mécanique. Chaque constat donne la mesure et le seuil (calibré sur ~700 vols réels, en comparant votre
drone à ceux de la même taille) et propose une action. Le bruit et le retard des filtres sont aussi comparés
aux repères de Brian White, l'auteur de PIDtoolbox (gyro filtré sous −30 dB, terme D au plus −10 dB sur
l'échelle de ses spectres, objectif de retard selon la taille). Quand plusieurs filtres sont à revoir, la **stratégie de filtrage**, en tête, ordonne les réglages à faire :
enregistrement, bruit à corriger, mécanique, filtre RPM, notch dynamique, passe-bas du gyro puis du D, un
changement à la fois. Les conseils fondés sur le retard des filtres demandent un vol avec des
mouvements francs (flips, rolls, wobbles) : sur un vol stationnaire, l'assistant affiche le retard sans
conseiller. Pour les tailles encore peu représentées (whoop,
2–3,5", 8" et plus), l'assistant prévient que ses seuils sont provisoires. Les conseils sont des points de départ : confirmez au ressenti et à la
température des moteurs.

## 6. Méthode de réglage conseillée

1. **Mécanique d'abord** : hélices neuves, vis serrées, moteurs sains. Un log de stationnaire + montée
   de gaz ; vérifiez Fréquence × gaz et l'assistant (résonances, déséquilibre).
2. **Filtres** : réglez le filtre RPM (motor_poles juste), puis les curseurs de filtres. Visez le moins
   de retard possible tant que le terme D reste peu bruité et les moteurs tièdes.
3. **PID** par séries de vols (méthode de Brian White), un curseur à la fois, par pas de 0,2 ; feedforward à 0
   (ou vols en mode Angle) et curseur I à 0,2–0,3 pendant les séries. D'abord **Damping** avec Master 1,0
   (0,8 pour un drone très puissant), puis **Master**, puis l'équilibre roll / pitch (Pitch T.), puis
   affinage ; le « Diagnostic du réglage » de la Réponse indicielle lit chaque série. Ensuite, I et
   Stick response (FF) à votre goût.
4. **Comparez** : ouvrez les logs successifs ensemble (réponse indicielle, infos de configuration).
5. Revenez en arrière dès qu'un changement dégrade le ressenti, le bruit ou la température.
