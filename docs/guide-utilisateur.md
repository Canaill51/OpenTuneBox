# OpenTuneBox — guide d'utilisation

## 1. Installer

Télécharger le fichier de son système sur la page **Releases** du dépôt GitHub (colonne de droite de la
page d'accueil), section *Assets* de la dernière version. Un seul fichier par système, qui contient tout : rien
d'autre à installer.

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

Pour **désinstaller** sous Linux : supprimer l'AppImage et son raccourci
(`~/.local/share/applications/OpenTuneBox.desktop`, et l'icône dans
`~/.local/share/icons/hicolor/256x256/apps/`). Vos fiches drone ne sont pas touchées : elles restent dans
`~/.local/share/OpenTuneBox/`.

Lancer OpenTuneBox depuis ses sources ou fabriquer l'exécutable soi-même : voir la partie
*Développement* du [README](../README.md#développement).

## 2. Ouvrir des logs

Au premier lancement, OpenTuneBox s'ouvre sur la page **Guide** : le parcours de réglage à cocher, par
drone (voir [6. Méthode de réglage](#6-méthode-de-réglage)). Le bouton **Guide** de la barre
d'outils y ramène à tout moment.

Bouton bleu **Ouvrir…** de la barre d'outils, glisser-déposer dans la fenêtre, ou double-clic
sur un `.bbl` / `.bfl` (Windows, voir ci-dessus). Un log enregistré par liaison série, avec l'extension
`.txt`, s'ouvre aussi : c'est son contenu qui compte, pas son nom. Sous Linux, la fenêtre d'ouverture est celle
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

## 4. La fenêtre, et les courbes du vol

- **Titre de la fenêtre** : le nom du programme suivi de sa **version** (« OpenTuneBox 0.9.0 ») — utile pour savoir
  laquelle vous utilisez avant de signaler un problème.
- **Barre du haut** : la **barre de fragments** (une pastille par vol en analyse, voir ci-dessous), puis
  **Ouvrir…**, l'icône qui exporte la fenêtre en image et la lune qui passe au **thème sombre** (ou clair).
- **La barre de fragments** décide de ce que montrent **tous les outils** : chaque pastille porte la couleur de ses
  courbes, le nom du fragment et celui du drone. Survolez-la pour sa fiche (rôle, date, heure de début, durée,
  PID roll, firmware).
  - **Un clic sur la pastille** la **désactive** (barrée, en pointillés) : ses courbes disparaissent des outils sans
    que le vol soit fermé. Un second clic la réactive.
  - **Un clic sur son rond** en fait le **fragment maître** (rond plus gros, coché) : c'est le vol des pages qui n'en
    montrent qu'un (Assistant, Fréquence × temps, « Et si » des filtres).
  - **Glissez-la** pour changer l'ordre des courbes ; sa couleur, elle, ne change jamais.
  - **Le ×** la retire de l'analyse (le vol est fermé).
  - La liste est **retrouvée au lancement suivant** : mêmes vols, même ordre, mêmes couleurs.
- **Rail de navigation à gauche** : **Tableau de bord**, **Guide**, **Assistant**, **Bruit et filtres**,
  **Réponse indicielle**, **Curseurs PID** et **Configuration**, chacun avec sa couleur, l'un sous l'autre.
  Survolez une entrée pour sa description.
- **Un clic dans le rail** remplace l'affichage par l'outil choisi, dans la même fenêtre
  (aucune nouvelle fenêtre) ; l'entrée de l'affichage en cours est mise en valeur. Un outil garde ses réglages et
  ses résultats quand on passe à un autre.
- **Tableau de bord** : une carte par drone que le logiciel a déjà vu — son nom, sa taille, où il en est du parcours
  de réglage, son nombre de **fragments** et la date du dernier vol. Cliquer une carte ouvre à droite la liste de ses
  fragments, du plus récent en haut ; le rond **+** rouvre le vol correspondant sans passer par la boîte de fichiers,
  une coche verte marque ceux qui sont déjà ouverts, et le **crayon** ouvre les courbes du fragment pour en
  régler les limites. Le logiciel retient ces drones et ces vols **d'un lancement à l'autre** : un drone est
  reconnu par le nom de machine inscrit dans ses logs, et chaque session ouverte devient un fragment.
- **Une entrée peut réunir plusieurs vues** : **Bruit et filtres** porte le **Spectre**, **Fréq. × gaz**,
  **Fréq. × temps** et le **Et si** des filtres. Ses onglets, en haut de la page, passent de l'une à l'autre sans
  repasser par le rail.

### Les courbes du vol et les limites du fragment

Ouvrir un log mène à cet écran, qui occupe toute la fenêtre (sans le rail). On y revient par le **crayon** d'un
fragment au tableau de bord, ou par le bouton **Visualiseur** du Guide. Les pages du Guide l'appellent
« Visualiseur ».

- **Barre du haut** : la flèche de retour, le drone, le fichier et la session, puis **Modifier le fragment …**.
  À droite, la lune change de thème, **Annuler** revient sans rien changer, **Enregistrer le fragment** garde les
  limites choisies et revient à l'écran d'où l'on venait.
- **Plusieurs morceaux d'un même vol** : **Enregistrer comme nouveau fragment** laisse le fragment affiché tel quel
  et en range un autre dans la même session, avec les limites, le nom et le rôle réglés (un nom déjà pris reçoit
  « (2) »). Il rejoint aussitôt la barre de fragments avec sa propre couleur, juste après celui d'origine, et devient
  le fragment maître : comparez par exemple une rafale de gaz et un vol stationnaire du même log. Il apparaît aussi
  dans la liste du drone au tableau de bord. Rouvrir le fichier par « Ouvrir… » ne remet en analyse que le fragment
  de chaque session ; les autres se rajoutent par leur rond **+**.
- **Supprimer un fragment ajouté** : en bas de la section **Fragment**, **Supprimer ce fragment…** puis
  **Supprimer définitivement** (ou **Garder**). Le fragment quitte la bibliothèque, ses mesures et la barre. Le
  fragment de la session elle-même n'a pas ce bouton : il revient à chaque ouverture du fichier.
- **Le fragment** : le morceau du vol que **tous les outils analysent** (la « période d'analyse »). Il est choisi
  automatiquement, du décollage à l'atterrissage, en s'arrêtant avant un crash. Ses limites sont les traits bleus
  « Début » et « Fin » ; le reste du vol est voilé. Tirez un trait (ou sa poignée) sur n'importe quel graphique
  pour l'ajuster. Dans le panneau de droite, section **Fragment** : son **nom**, son **rôle** (Filtres ou Série
  PID, que le Guide reprend), le **début** et la **fin** — les flèches avancent ou reculent de 0,1 s, la valeur se
  tape ou se glisse à l'échantillon près —, la **durée**, **Limites auto** pour revenir au calcul automatique et
  **Session entière** pour tout prendre.
- ⚠️ **Rien ne change avant « Enregistrer le fragment »** : tant que vous tirez les limites, les outils gardent
  l'ancienne période. Une fois enregistré, le fragment est retenu d'un lancement à l'autre : rouvrir le même
  fichier retrouve ses limites. Un log sans nom de machine n'est rangé sous aucun drone : ses limites valent alors
  pour la séance seulement (le panneau le dit). Chaque log ouvert garde son propre fragment.
- **Barre de navigation**, sous les graphiques : le log entier (le profil des gaz), le fragment en bleu, et la
  **fenêtre visible** dans un cadre. Glissez le cadre pour vous déplacer, tirez ses poignées pour l'élargir ou la
  resserrer, cliquez ailleurs pour y sauter. **Vue entière** montre tout le log, **Cadrer le fragment** le
  fragment seul.
- **Mesurer** : mesure l'écart de temps entre deux points, en millisecondes, et la fréquence correspondante
  en hertz (1 / écart), par exemple la période d'une oscillation. Enfoncez le bouton, puis cliquez deux
  points sur un graphique roll, pitch ou yaw : deux repères violets apparaissent sur les trois axes, et le
  résultat s'affiche en haut des graphiques et à droite de la barre d'outils. Glissez un repère pour ajuster ; un
  nouveau clic recommence la mesure ; désenfoncez le bouton pour l'effacer.
- **Traces** (panneau de droite) : une case par courbe, avec sa couleur ; les traces que le log ne contient pas
  sont masquées.
- **Affichage** (section repliée) : échelle verticale, lissage et épaisseur des traces.
- **Lecteur** : début et fin du fragment, image par image, ▶ pour rejouer le vol ; **Au curseur** (section
  repliée) : les manches et les valeurs à l'instant choisi.
- Zoom : molette ; déplacement : clic-glisser sur un graphique.
- **Tous / Roll / Pitch / Yaw** : un seul axe sur toute la hauteur.

## 5. Les outils

Chaque outil s'affiche par son entrée du rail, à gauche. Le titre ci-dessous est celui de l'entrée ;
entre parenthèses, le nom plus court que les pages du Guide emploient pour le même outil.

**Les boutons communs à tous les outils**, en haut de chaque page :

- **Calculer** : relance le calcul. Il se fait déjà tout seul quand vous changez un réglage ; ce bouton sert à le
  refaire à la demande.
- **Tout voir** : remet le graphique à sa vue d'ensemble après un zoom.
- **Exporter image** : enregistre la page en PNG (nom proposé `OTB_FIGS_AAAA-MM-JJ-N.png`). L'icône tout à droite de
  la barre, elle, capture la fenêtre entière.
- Les vols montrés, leur ordre et leurs couleurs viennent de la **barre de fragments** du haut : une page n'a pas
  de liste à elle. Tout désactiver laisse la page vide.

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

### Réponse indicielle
La réaction du drone à un ordre de manche « en marche d'escalier », moyennée sur tout le vol.
**L'ordre des courbes est le vôtre** : celui des pastilles de la barre de fragments, que vous changez en les
glissant ; chaque courbe garde la couleur de sa pastille, la même que sur les autres pages. Désactivez une pastille
pour retirer sa courbe, toutes pour vider la page.
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
S'ouvre par l'onglet **Et si** de l'entrée **Bruit et filtres** du rail.
Reconstitue la chaîne de filtres de Betaflight à partir du log. Modifiez un filtre, puis cliquez sur
**Appliquer** : le programme montre la réponse (gain, phase), le retard ajouté et le bruit qui passerait, en rejouant
les filtres sur le gyro non filtré du vol, comme le firmware (notch dynamique compris : il suit les pics avec vos
nouveaux réglages ; une à quelques secondes de calcul). Les spectres ne bougent **pas** tant que vous n'avez pas
cliqué sur Appliquer ; **Réinitialiser** revient aux réglages du log. Chaque spectre montre le signal **avant filtres**,
**filtré** (le log), **simulé modifié** et, case « simulation réglages actuels » cochée, **simulé actuel** : les filtres du
log rejoués. Plus « simulé actuel » se confond avec le log filtré, plus la simulation est fiable sur ce vol ; elle l'est
le plus avec un log enregistré à chaque boucle PID (0,1 à 0,2 dB d'écart en général), un peu moins une boucle sur deux, et
beaucoup moins sous 1 600 Hz. « Simulé modifié » part du spectre du log : sans changement, il le recouvre exactement. Sans
gyro non filtré, il n'y a rien à rejouer : la simulation se contente des réponses des filtres, sans « simulé actuel ». Copiez ensuite les commandes CLI. Les deux spectres (gyro et terme D) sont sur
l'échelle de PIDtoolbox, **−50 à 20 dB**, la même que la page Spectre : les deux lignes et les deux pages
se lisent l'une contre l'autre.

### Infos de configuration (Configuration)
Entrée **Configuration** du rail. Tous les réglages des logs côte à côte ; les différences sont surlignées (« Seulement les différences »).

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
- **Où sont rangées vos fiches** : dans un fichier `drones.json`, à côté des réglages de l'application —
  `%APPDATA%\OpenTuneBox\OpenTuneBox\` sous Windows, `~/.local/share/OpenTuneBox/OpenTuneBox/` sous Linux. C'est ce
  fichier qu'il faut copier pour sauvegarder votre avancement ou le reprendre sur une autre machine. S'il devient
  illisible, l'application ne l'écrase pas : elle le met de côté sous `drones.json.illisible` et le dit dans le Guide.
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
