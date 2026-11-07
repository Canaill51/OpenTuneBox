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

**Linux** (Ubuntu 22.04, Debian 12, Fedora 36… ou plus récent) : `OpenTuneBox-Linux.AppImage`, un seul
fichier. Le rendre exécutable (clic droit → *Propriétés* → *Autoriser l'exécution*, ou
`chmod +x OpenTuneBox-Linux.AppImage`), puis double-clic. S'il ne démarre pas (système sans FUSE) :
`./OpenTuneBox-Linux.AppImage --appimage-extract-and-run`. Au premier lancement, OpenTuneBox s'ajoute au
menu des applications et y met son icône ; si vous déplacez le fichier ensuite, relancez-le depuis son
nouvel emplacement et corrigez ou supprimez l'ancien raccourci
(`~/.local/share/applications/OpenTuneBox.desktop`).

Lancer OpenTuneBox depuis ses sources ou fabriquer l'exécutable soi-même : voir la partie
*Développement* du [README](../README.md#développement).

## 2. Ouvrir des logs

Au premier lancement, OpenTuneBox s'ouvre sur la page **Guide** : le parcours de réglage à cocher, par
drone (voir [6. Méthode de réglage](#6-méthode-de-réglage)). Le bouton **Guide** de la barre
d'outils y ramène à tout moment.

Bouton bleu **Ouvrir** de la barre d'outils (ou Ctrl+O), glisser-déposer dans la fenêtre, ou double-clic
sur un `.bbl` / `.bfl` (Windows, voir ci-dessus). Sous Linux, la fenêtre d'ouverture est celle
du bureau (KDE, GNOME…). Chaque session d'un fichier (un armement = une session) apparaît comme un log
séparé ; 10 logs au maximum, chacun avec sa couleur, que l'on retrouve dans tous les outils.

## 3. Bien enregistrer

Le guide intégré (page **Guide**, phase « Préparer le drone ») donne les réglages d'enregistrement adaptés à
la fiche de votre drone et les vérifie dans chaque log. En bref :

- onglet **Blackbox** du configurateur, fréquence d'enregistrement selon le vol : pour **régler les filtres**,
  **au moins 2 kHz** avec un gyro à 8 kHz, **1,6 kHz** avec un gyro à 3,2 kHz (BMI270), plus pour les petits
  drones ; pour les **séries PID**, **1 kHz** suffit (1,6 kHz avec une boucle PID à 3,2 kHz) ; mode débug **FFT_FREQ**
  (4.5 et plus) ou **GYRO_SCALED** (4.4) ;
- **DShot bidirectionnel** activé (onglet **Moteurs**) ;
- **un vol de réglage en deux parties** : 20 à 30 s de stationnaire avec des montées de gaz (bruit), puis 20
  à 30 s de wobbles en **mode Angle**, ou en **Acro avec le feedforward à 0**, avec le profil de rates
  « LOG » (ACTUAL 200 / 200 / 0, limite d'angle 30° en mode Angle), roll et pitch mêlés (retard des filtres, réponse des PID). OpenTuneBox découpe les deux parties tout
  seul.

## 4. Le visualiseur (fenêtre principale)

- **Barre d'outils** en haut : ouvrir et fermer les logs, puis les boutons **Guide**, **Visualiseur**, Spectre,
  Indicielle…, chacun avec sa couleur ; l'icône tout à droite exporte la fenêtre en image. Survolez un bouton
  pour sa description.
- **Les boutons servent d'onglets** : un clic remplace l'affichage par l'outil choisi, dans la même fenêtre
  (aucune nouvelle fenêtre) ; le bouton de l'affichage en cours est mis en valeur. **Visualiseur** ramène aux
  courbes du vol. Un outil garde ses réglages et ses résultats quand on passe à un autre. Fréq. × gaz et
  Fréq. × temps s'ouvrent depuis l'outil Spectre ; Configuration, depuis le bouton en bas du panneau de
  droite du visualiseur.
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

Chaque outil s'affiche par son bouton dans la barre d'outils (nom du bouton entre parenthèses).

### Analyse spectrale (Spectre)
Spectres (FFT) du gyro et des termes PID : on y voit où se trouve le bruit et ce que les filtres
enlèvent (préfiltre en pointillés, filtré en trait plein). Volet « < 100 Hz » pour les oscillations et
le propwash, avec les **retards des filtres** (mesurables seulement si le vol contient des mouvements :
sur un simple vol stationnaire, les filtres dynamiques ne travaillent qu'à un régime et le retard affiché
est marqué « peu fiable ») ; volet « bruit moteur » pour l'efficacité du filtre RPM
sur chaque harmonique. Les courbes en S montrent où se situent les harmoniques moteur pendant le vol :
**Harmoniques moteur** donne une case par harmonique (1re, 2e, 3e), à cocher séparément, et le menu
**Notchs** une case par notch réellement enregistré (N1 à N7, seulement avec le mode débug FFT_FREQ),
avec « Tous » et « Aucun » ; le bouton du menu dit ce qui est tracé sans l'ouvrir ;
**Plage du filtre RPM** ajoute par-dessus les bandes couvertes par le filtre RPM.

Les niveaux en dB sont sur la **même échelle que PIDtoolbox** (gyro filtré sous −30 dB, terme D sous
−10 dB : les repères habituels se lisent directement), dans le Spectre comme dans Fréq. × gaz, Fréq. × temps
et le simulateur de filtres. Résolution par défaut : 0,5 Hz, comme PIDtoolbox.

Case **Diagnostic du bruit** (cadre « Diagnostic », en bas du panneau de réglages) : ouvre, à côté des courbes, un rapport sur le bruit et la stratégie de
filtrage, pour chaque log choisi et sur la période d'analyse. Il est calculé seulement quand la case est
cochée (quelques secondes) et compte quatre parties :
1. **Conditions du log** : fréquence d'enregistrement suffisante pour juger les filtres, vol stationnaire (bon pour identifier le
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

Les constats et les seuils sont ceux de l'assistant : les deux outils donnent les mêmes conseils.

### Fréquence × gaz (Fréq. × gaz)
Bouton de l'outil Spectre. Carte du bruit selon le gaz. Une **diagonale** qui monte avec le gaz = bruit moteur (normal, doit être
retiré par le filtre RPM). Une **ligne horizontale** = résonance de châssis ou pièce qui vibre. Une
tache basse fréquence à mi-gaz = oscillation de réglage ou propwash.

### Fréquence × temps (Fréq. × temps)
Bouton de l'outil Spectre. Le même bruit dans le temps : utile pour relier un bruit à un moment du vol (le curseur est lié au
lecteur ; un clic sur un spectrogramme le déplace). Les harmoniques moteur et les notchs dynamiques se choisissent un par un,
comme dans le Spectre.

### Réponse indicielle (Indicielle)
La réaction du drone à un ordre de manche « en marche d'escalier », moyennée sur tout le vol.
**L'ordre des courbes et leurs couleurs sont les vôtres** : elles suivent l'ordre dans lequel vous cochez
les fichiers (la 1re cochée prend la 1re couleur), et vous pouvez tout décocher pour vider la page. C'est
propre à cette page : ailleurs, chaque log garde la couleur reçue à son ouverture.
Objectifs : montée rapide, **pic proche de 1** (sans feedforward, une réponse sans dépassement visible donne
environ 1,03 ; le feedforward fait monter le pic), pas de rebond. Colonnes de mesures au choix :
latence, temps de montée, bande passante, pic de sensibilité (< 7 dB), marge de phase (> 35°), marge de
gain, fréquence de croisement…
Les marges de stabilité sont **mesurées** à partir des termes P, I, D et F enregistrés dans le log (point
plein, barre d'erreur = incertitude de la mesure). Si un de ces termes manque, elles sont seulement
**approchées** (point creux) : à prendre avec prudence. Un triangle signifie « au moins cette valeur » :
le pic de sensibilité se trouve au-delà des fréquences que vos mouvements de manche permettent de mesurer.
Plus vos wobbles sont rapides et secs (roll et pitch en même temps, au hasard), plus la mesure va loin
en fréquence ; l'Assistant le rappelle quand le pic tombe au bord.
Sous le diagnostic : **curseurs PID Betaflight** de chaque log (réglage simplifié ; point plein = curseurs de
l'en-tête, point creux = estimés depuis les PID), pour voir d'un coup d'œil ce qui change entre les vols.
Dans chaque graphique, en bas à droite : gains P, I, D, Dm, F de chaque fichier, nombre de fenêtres (n), pic,
latence et cassure. « Temps affiché » : 250 ms par défaut (la montée en grand), 500 ms comme PIDtoolbox, ou
150 ms pour bien voir la cassure. Le yaw est masqué au départ (case Y) : la méthode ne le juge pas et ses
mouvements suffisent rarement. La colonne « Cassure (ripple) » suit la cassure d'un vol à l'autre.

**Diagnostic du réglage**, toujours affiché à côté des courbes, d'après la méthode des séries de vols. Chargez une **série de vols** du même drone, mêmes filtres, où un seul curseur change par pas
de 0,2 ; l'outil reconnaît la série tout seul et lit, selon le curseur :
- **Damping** (équilibre P/D) : jugé sur le **roll**, la première valeur sans dépassement visible (pic
  ≤ 1,05), ou celle au-delà de laquelle le pic ne baisse plus. Si le pitch dépasse encore, il a plus
  d'inertie : l'outil conseille alors de monter **Pitch Damping** (son D à part) plutôt que le Damping ;
- **Pitch Damping** : même lecture, sur le pic du pitch ;
- **Master** (niveau des gains) : on monte tant que la latence baisse sans **cassure** (« ripple » : la
  montée s'arrête un instant avant la consigne, puis reprend lentement) ni **vagues** sur le palier (lues
  seulement à partir de 200 fenêtres : en dessous, le bruit seul en fabrique). Chaque
  vol est classé propre, premiers signes (proche de l'optimum) ou trop haut ; au premier vol trop haut, on
  garde la valeur d'avant ;
- **Pitch Tracking** (ou, sans série, le vol conseillé) : écart de latence entre pitch et roll, visé
  sous 1 ms, acceptable jusqu'à 2 ms selon le drone ;
- **Feedforward** (curseur « Réponse des sticks », en finitions) : la série est reconnue mais pas jugée, car
  le feedforward fausse la réponse indicielle ; ses retards se lisent dans l'**Assistant**.

Conditions vérifiées pour chaque vol (hors série Feedforward) : feedforward à 0 en Acro, ou vol en **mode Angle** (le feedforward y
est coupé ; pas en Horizon), curseur I (Drift-Wobble) à 0,2–0,3, assez de mouvements.
Dans les colonnes de mesures, un cercle entoure les vols conseillés ; sur les courbes, un point creux marque
l'arrêt de la montée quand il y a une cassure.

### Curseurs PID
Positions des curseurs (lues dans le log, ou estimées si le réglage simplifié est désactivé) et
simulateur « et si » : bougez un curseur, voyez les PID et filtres obtenus, copiez les commandes CLI.

### Simulateur de filtres (Filtres)
Reconstitue exactement la chaîne de filtres de Betaflight à partir du log. Modifiez un filtre :
le programme montre la réponse (gain, phase), le retard ajouté et le bruit qui passerait, en rejouant
le vrai gyro du vol. Copiez ensuite les commandes CLI. Les deux spectres (gyro et terme D) sont sur
l'échelle de PIDtoolbox, **−50 à 20 dB**, la même que la page Spectre : les deux lignes et les deux pages
se lisent l'une contre l'autre.

### Infos de configuration (Configuration)
Bouton **Configuration** en bas du panneau de droite du visualiseur. Tous les réglages des logs côte à côte ; les différences sont surlignées (« Seulement les différences »).

### Assistant de réglage (Assistant)
Diagnostic complet d'un log : qualité de l'enregistrement, réponse, bruit, filtres, moteurs,
mécanique. Chaque constat donne la mesure et le seuil (calibré sur ~700 vols réels, en comparant votre
drone à ceux de la même taille) et propose une action. Le bruit et le retard des filtres sont aussi comparés
aux repères usuels de PIDtoolbox (gyro filtré sous −30 dB, terme D au plus −10 dB sur
l'échelle de ses spectres, objectif de retard selon la taille). Quand plusieurs filtres sont à revoir, la **stratégie de filtrage**, en tête, ordonne les réglages à faire :
enregistrement, bruit à corriger, mécanique, filtre RPM, notch dynamique, passe-bas du gyro puis du D, un
changement à la fois. Les conseils fondés sur le retard des filtres demandent un vol avec des
mouvements francs (flips, rolls, wobbles) : sur un vol stationnaire, l'assistant affiche le retard sans
conseiller. Il donne aussi le **retard consigne → gyro** en roll et en pitch, avec la valeur du feedforward :
c'est la mesure de la série Feedforward des finitions (0, 0,5, 1… en **Acro uniquement**, le firmware coupant
le feedforward en mode Angle), à comparer d'un vol à l'autre. Pour les tailles encore peu représentées (whoop,
2–3,5", 8" et plus), l'assistant prévient que ses seuils sont provisoires. Les conseils sont des points de départ : confirmez au ressenti et à la
température des moteurs.

## 6. Méthode de réglage

La méthode est le **parcours du guide intégré** (bouton **Guide**) : des cases à cocher, par drone.

- **Fiche drone** (en haut de la page) : nom (le même que dans Betaflight : les logs sont reconnus grâce à
  lui), version de Betaflight, taille, boucle PID, ESC ; préremplie d'après le log ouvert. Les valeurs du
  guide s'y adaptent et chaque drone garde son avancement ; **Recommencer** décoche toutes ses cases, **Supprimer** retire la fiche (les logs ne sont pas
  touchés).
- **Frise et liste des phases** : où vous en êtes (cases faites / total).
- **Cases** : un clic coche ; le bouton **ⓘ Aide** affiche l'aide ; **Ouvrir …** ouvre l'outil utile, avec la période
  d'analyse réglée sur la bonne partie du vol. Dès qu'un log du drone est ouvert, certaines cases se
  vérifient toutes seules (« ✔ Vérifié », ou « ▲ À revoir » avec la raison).
- **Logs du drone** (sous les cases), en deux parties : **logs des filtres** et **logs des séries PID**.
  Ouvrez-les avec le bouton de la case « Récupérer le log et l'ouvrir » (phase Filtres) ou « Ouvrir les logs
  de la série » (phase PID, et sa pareille pour la série Feedforward des Finitions) : chaque log garde ce rôle, et les cases d'une phase ne lisent que les logs de
  cette phase. Un log ouvert autrement est classé tout seul (partie stationnaire → filtres, sinon série PID) ;
  le lien « → classer dans les … » corrige le classement. Pour chaque log : à quoi il sert (identifier le
  bruit, juger le retard des filtres, vol d'une série PID), ce qu'il faut corriger avant le prochain vol (où
  dans le configurateur), et un bouton qui copie les commandes CLI équivalentes. Sous les logs des séries PID :
  chaque série reconnue (un seul curseur changé, mêmes filtres), les vols aux réglages identiques et ceux
  hors série.

Les quatre phases :

1. **Préparer le drone** : Mode Expert, sauvegarde `diff all`, retour aux valeurs par défaut, mécanique,
   enregistrement, DShot bidirectionnel, profil de rates « LOG », interrupteurs (mode Angle, profil de rates).
2. **Filtres** : le vol de réglage, puis un changement à la fois : filtre RPM, notch dynamique, passe-bas du
   gyro puis du D. Fin quand les repères sont atteints (gyro filtré sous −30 dB, terme D au plus −10 dB,
   retard sous l'objectif de la taille).
3. **PID par séries** : mode Angle (ou Acro avec le feedforward à 0), curseur I à 0,25, Dynamic Damping à 0 ; série
   **Damping** (0,6 → 1,6, ou 1,0 → 1,6 pour un 6" et plus), **Master** (1,0 → 2,0), puis Pitch Damping et
   Pitch Tracking selon le diagnostic ; au moins 200 fenêtres par vol. Le « Diagnostic du réglage » de la
   Réponse indicielle lit chaque série.
4. **Finitions** : série Feedforward en Acro (0, 0,5, 1 ; retard consigne → gyro de chaque vol, donné par
   l'Assistant et sous les logs du guide), terme I dans une plage selon la taille, Dynamic Damping à votre goût, retour à vos
   rates.

Les changements passent d'abord par les **curseurs et les onglets** du configurateur Betaflight, le CLI
seulement en repli. Revenez en arrière dès qu'un changement dégrade le ressenti, le bruit ou la température
des moteurs.
