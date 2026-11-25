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
`~/.local/share/icons/hicolor/256x256/apps/`). Vos drones, leurs fiches et vos fragments ne sont pas touchés : ils
restent dans `~/.local/share/OpenTuneBox/`.

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

### Récupérer les logs par USB

**Récupérer par USB**, en haut du tableau de bord, copie les vols du drone — **mémoire interne** ou **carte SD** —
sans passer par le configurateur Betaflight (fermez-le : un seul programme à la fois peut tenir le port). **Retirez les hélices** avant
de brancher une batterie.

1. Branchez le drone en USB, puis **Se connecter** (**Chercher** si la carte n'est pas trouvée). L'écran montre son nom,
   sa carte, sa version de Betaflight et la place occupée. Un drone **armé** bloque tout : rien n'est lu ni effacé.
   Sous Linux, un refus de droits se règle une fois pour toutes (`sudo usermod -aG dialout $USER`, puis rouvrir la
   session) : l'écran donne la commande.
2. **Première connexion de cette carte** : choisissez le drone auquel la ranger (celui du même nom est proposé) ou
   **Nouveau drone**. La carte porte un identifiant gravé : la fois suivante, le drone est **reconnu tout seul**, même
   renommé.
3. **Copier les vols** (par l'USB série) : environ 3 minutes pour 8 Mo, une barre montre l'avancement ; **Arrêter** abandonne (rien n'est
   gardé, la copie suivante reprend au début). La copie est ensuite **vérifiée** — taille, lecture par le décodeur,
   fichier relu sur le disque — puis rangée sous le drone : ses vols apparaissent sur sa carte au tableau de bord, et
   **Ouvrir les vols** les met dans la barre d'analyse.
4. **Effacer la mémoire du drone…** n'apparaît qu'après une copie vérifiée et rangée, et demande une seconde
   confirmation (**Effacer définitivement**). Juste avant, OpenTuneBox relit l'identifiant de la carte, la place
   occupée et l'état armé : un vol enregistré depuis la copie, ou une autre carte, et l'effacement est refusé.
5. **Sauvegarder la configuration** lit le `diff all` de la carte et le garde s'il a changé depuis la dernière fois
   (la fiche du drone peut alors comparer deux versions de ses réglages) ; la carte redémarre ensuite.

**Copier en mode disque**, à côté de **Copier les vols**, fait redémarrer le drone en **disque USB** (mode « stockage de
masse ») : la copie est bien plus rapide, et passe par les mêmes vérifications. L'écran attend que le système monte le
disque ; s'il n'apparaît pas, ouvrez-le dans le gestionnaire de fichiers (disque « BETAFLT »), ou **Choisir le
dossier…**. Ce disque est en lecture seule : pour **effacer** ensuite la mémoire, **débranchez le drone** (USB et
batterie) **puis rebranchez-le** en USB. OpenTuneBox s'y reconnecte tout seul et propose l'effacement si c'est **la même
carte**, sans vol enregistré depuis.

**Carte SD dans le drone** : l'écran propose **Copier la carte SD (mode disque)**. Chaque log est copié et vérifié ; un
log illisible (vol coupé avant sa première trame) est **refusé** et reste sur la carte. **Supprimer N logs de la carte
SD…** puis **Supprimer définitivement** ne retire que les logs copiés et vérifiés, chacun relu juste avant ; les
autres fichiers de la carte ne sont pas touchés. **Éjectez la carte** depuis le système avant de débrancher.

**Carte SD dans un lecteur de cartes** : sans drone branché, la section **Carte SD dans un lecteur** trouve la carte
(sinon **Choisir le dossier…**, la racine de la carte, celle qui contient `LOGS`). Choisissez le drone auquel ranger
ses vols (le nom lu dans le log est proposé), puis **Copier la carte SD** ; la suite est la même.

OpenTuneBox **n'écrit jamais de réglage** sur la carte.

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
  « LOG » (ACTUAL 200 / 200 / 0, limite d'angle 30° conseillée en mode Angle, de 20 à 45° accepté), roll et pitch mêlés (retard des filtres, réponse des PID). OpenTuneBox découpe les deux parties tout
  seul.

## 4. La fenêtre, et les courbes du vol

- **Titre de la fenêtre** : le nom du programme suivi de sa **version** (« OpenTuneBox 0.9.0 ») — utile pour savoir
  laquelle vous utilisez avant de signaler un problème.
- **Barre du haut** : la **barre de fragments** (une pastille par vol en analyse, voir ci-dessous), puis
  **Ouvrir…**, l'icône qui exporte la fenêtre en image et la lune qui passe au **thème sombre** (ou clair) ;
  OpenTuneBox rouvre avec le dernier thème choisi.
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
  de réglage, son nombre de **fragments** et la date du dernier vol. L'étape et la barre d'avancement comptent les
  cases cochées à la main et celles que le Guide a vues vérifiées par un log **la dernière fois qu'il a été affiché**. Cliquer une carte ouvre à droite la liste de ses
  fragments, du plus récent en haut ; le rond **+** rouvre le vol correspondant sans passer par la boîte de fichiers,
  une coche verte marque ceux qui sont déjà ouverts, et le **crayon** ouvre les courbes du fragment pour en
  régler les limites. Le logiciel retient ces drones et ces vols **d'un lancement à l'autre** : un drone est
  reconnu par le nom de machine inscrit dans ses logs, et chaque session ouverte devient un fragment.
- **Fiche du drone** : le crayon en haut à droite d'une carte, ou **Nouveau drone**, ouvre la fiche — nom (le même que
  dans Betaflight, onglet Configuration : c'est lui qui relie les logs au drone), taille (elle choisit les seuils de
  l'Assistant et les valeurs du Guide), batterie, poids, taille et KV des moteurs, pôles moteur, hélices, ESC, version de
  Betaflight, boucle PID. La **taille** se tape librement (« 3,5″ », « 10″ », « 85 mm ») ou se choisit avec la flèche ;
  la ligne dessous dit quels seuils l'Assistant prendra. Les **pôles** et la **boucle PID** se lisent dans vos logs :
  la boucle PID suit le dernier log ; les pôles n'y sont pris que si vous ne les avez pas remplis, et si votre valeur
  diffère du log, une note orange le signale (un `motor_poles` faux dans Betaflight fausse le filtre RPM). Le **KV**,
  comme la taille du drone et celle des moteurs, est à remplir vous-même : il n'est jamais tiré du log. La
  carte de vol et son identifiant s'y lisent au premier branchement USB, comme l'historique des **configurations**
  (`diff all`) : **Voir** montre l'une, **Comparer** dit les réglages changés depuis la précédente. **Supprimer…** puis
  **Supprimer définitivement** retire le drone, ses fragments et ses configurations du logiciel. Vos fichiers de logs
  ne sont pas effacés ; ceux que le logiciel a **copiés par USB** ne le sont que si vous cochez **Supprimer aussi les
  fichiers copiés**, décochée d'office (ils sont alors la seule trace de ces vols). La **Fiche cible**, à droite, dit ce que ce drone doit atteindre : latence, zone du pic, temps de montée,
  marges de phase et de gain, bruit du gyro et du terme D, retard des filtres. Tant que vous n'y touchez pas, elle
  montre les seuils de l'Assistant pour la taille choisie (et les suit si vous changez la taille) ; une case sans seuil
  dans l'Assistant reste vide, à vous de la remplir. Changez une case et la fiche garde vos valeurs ; **Valeurs
  proposées** reprend celles de la taille. L'Assistant garde ses propres seuils : votre cible sert au suivi du drone.
  Sous elle, la section **Suivi des mesures** dit où en sont les mesures du drone : à l'ouverture de la fiche, le
  logiciel mesure en arrière-plan chaque fragment qui ne l'est pas encore (latence, pic, temps de montée, marges, bruit
  du gyro et du terme D, retard des filtres — les chiffres de l'Assistant) et les garde pour suivre le drone d'un vol à
  l'autre. Un fragment dont le log a été déplacé ou supprimé est signalé avec la raison. Dès qu'un fragment est mesuré,
  un **graphique par grandeur** montre son évolution au fil des vols (numérotés 1, 2, 3… par date) : une couleur par axe
  (roll, pitch, lacet), la zone visée par votre fiche cible teintée en vert et bordée de pointillés, « sans cible » quand
  la case est vide. Un point creux est une mesure moins sûre (moins de 200 fenêtres de réponse indicielle : peu de
  mouvements de sticks) ; sous 20 fenêtres, le vol n'a pas de point, comme dans l'Assistant. Survolez un point pour voir
  le vol, sa date et sa valeur. Changer une case de la fiche cible déplace la zone aussitôt. Les fiches faites avec une version précédente sont reprises au premier lancement ; l'ancien fichier
  `drones.json` est gardé sous le nom `drones.json.repris`.
- **Une entrée peut réunir plusieurs vues** : **Bruit et filtres** porte le **Spectre**, **Fréq. × gaz**,
  **Fréq. × temps** et le **Et si** des filtres. Ses onglets (icône et nom), en haut de la page, passent de l'une à
  l'autre sans repasser par le rail ; celui de la vue affichée est en bleu et souligné. En tête du panneau de droite,
  **Gyro | Terme D** choisit le signal des quatre vues à la fois — le gyro avant et après ses filtres, ou le terme D
  après ses filtres (le D avant filtres n'est pas enregistré par Betaflight, il n'est donc pas proposé ici) —, et les
  cases **R P Y** leurs axes : ce qui est choisi dans l'une se retrouve dans les autres. Les cases détaillées de
  chaque vue restent dessous (erreur PID, consigne, préréglages de Fréq. × gaz…) ; si vous y choisissez autre chose que
  le gyro ou le terme D seul, aucun des deux boutons n'est allumé, et seule cette vue change.

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
enlèvent : le gyro avant filtres (« préfiltre ») en fins pointillés quand le gyro filtré est aussi tracé, en trait
plein s'il est seul, comme dans PIDtoolbox. Volet « < 100 Hz » pour les oscillations et
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
Onglet de l'entrée **Bruit et filtres**. Carte du bruit selon le gaz. Elle s'ouvre sur le gyro **avant et après ses
filtres**, deux colonnes par fragment ; le **Terme D** donne une colonne par fragment. Les **Préréglages** de PIDtoolbox
restent dans la liste, sauf les deux qui montraient le D avant filtres. Une **diagonale** qui monte avec le gaz = bruit moteur (normal, doit être
retiré par le filtre RPM). Une **ligne horizontale** = résonance de châssis ou pièce qui vibre. Une
tache basse fréquence à mi-gaz = oscillation de réglage ou propwash.

### Fréquence × temps (Fréq. × temps)
Onglet de l'entrée **Bruit et filtres**. Le même bruit dans le temps : utile pour relier un bruit à un moment du vol (le curseur est lié au
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
Reconstitue la chaîne de filtres de Betaflight à partir du log. **Gyro | Terme D**, en tête du panneau, choisit les
filtres à régler ; ceux du gyro ont deux pages, **Passe-bas et notchs** et **Notch dyn / RPM**. En tête des passe-bas du
gyro et du terme D, le **multiplicateur du filtre**, comme dans Betaflight Configurator : coché, son curseur (de 0,10 à
2,00, par pas de 0,05) recalcule les coupures des passe-bas actifs comme le firmware, plus de filtrage à gauche, moins à
droite ; les fréquences sont alors grisées (type, mode et expo restent libres). Décochez-le pour les saisir à la main. Il
part de l'état du log ; coché dans le log mais avec des coupures changées depuis, il démarre décoché, comme dans le
configurateur. Aux extrémités (gyro jusqu'à 0,45 ou dès 1,55, terme D jusqu'à 0,75 ou dès 1,25), un avertissement rouge.
Les commandes CLI écrivent alors l'état du curseur **et** les coupures : un `set` dans le CLI ne les recalcule pas.
Modifiez un filtre, puis cliquez sur
**Appliquer** : le programme montre la réponse (gain, phase), le retard ajouté et le bruit qui passerait, en rejouant
les filtres sur le gyro non filtré du vol, comme le firmware (notch dynamique compris : il suit les pics avec vos
nouveaux réglages ; une à quelques secondes de calcul). Les spectres ne bougent **pas** tant que vous n'avez pas
cliqué sur Appliquer ; **Réinitialiser** revient aux réglages du log. Chaque spectre montre le signal **avant filtres**,
**filtré** (le log), **simulé modifié** et, case « simulation réglages actuels » cochée, **simulé actuel** : les filtres du
log rejoués. Plus « simulé actuel » se confond avec le log filtré, plus la simulation est fiable sur ce vol ; elle l'est
le plus avec un log enregistré à chaque boucle PID (0,1 à 0,2 dB d'écart en général), un peu moins une boucle sur deux, et
beaucoup moins sous 1 600 Hz. Au-dessus des courbes, la ligne **« Écart de la simulation au vol »** chiffre cet écart sur
le vol ouvert : pour le gyro et le terme D, la bande de fréquences où il est le plus grand (survolez-la pour le détail
de chaque bande). Quand la cause est connue, une phrase la dit : log qui ne garde qu'une boucle PID sur plusieurs (le
gyro non filtré y est incomplet, le terme D en souffre le plus), ou firmware d'avant la 4.3. « Simulé modifié » part du spectre du log : sans changement, il le recouvre exactement. Sans
gyro non filtré, il n'y a rien à rejouer : la simulation se contente des réponses des filtres, sans « simulé actuel ». Copiez ensuite les commandes CLI. Les deux spectres (gyro et terme D) sont sur
l'échelle de PIDtoolbox, **−50 à 20 dB**, la même que la page Spectre : les deux lignes et les deux pages
se lisent l'une contre l'autre.

### Infos de configuration (Configuration)
Entrée **Configuration** du rail. Tous les réglages des logs côte à côte ; les différences sont surlignées (« Seulement les différences »).

### Assistant de réglage (Assistant)
Diagnostic complet du fragment maître : qualité de l'enregistrement, réponse, bruit, filtres, moteurs,
mécanique. Chaque constat est une **carte**, de la plus grave à la moins grave (attention, conseil, info), avec sa
mesure, son seuil (calibré sur ~700 vols réels, en comparant votre drone à ceux de la même taille) et les actions
proposées. Quand le constat compare une mesure à un seul seuil, une **jauge** les place côte à côte : le rond est la
mesure, le trait en tirets le seuil. Les cartes « info » sont repliées (le chevron les ouvre), et les vérifications
réussies tiennent sur une ligne « N vérifications OK » qu'un clic déplie. En tête, une pastille compte les constats de
chaque gravité ; à droite, la réponse indicielle de chaque axe (pic et temps de réponse). La taille du drone se choisit
dans le panneau de droite si la détection se trompe. Le bruit et le retard des filtres sont aussi comparés
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

**D'un vol à l'autre.** En tête, le drone du fragment, puis **comparé à** : par défaut le **vol précédent** du même drone
(dans l'ordre de ses vols, celui du suivi de sa fiche), qu'il soit ouvert ou non ; le menu en choisit un autre (un autre vol
du drone, ou un fragment de la liste d'analyse). Sans vol précédent, l'en-tête le dit et les cartes restent seules. Sur
chaque jauge, un **rond creux** et une ligne (« FINAL.02 : −10,7 dB ») donnent la mesure du même constat sur le vol comparé ;
une carte absente du vol comparé porte **NOUVEAU**, et une ligne liste ce qui n'est **plus signalé** depuis.

**Vos choix sur les conseils.** Chaque conseil qui propose un geste a deux pouces. **Pouce levé** : vous avez appliqué le
conseil sur le drone (dans le configurateur : OpenTuneBox n'écrit jamais sur la carte) ; c'est inscrit tout de suite, avec la
date. **Pouce baissé** : le conseil ne convient pas ; écrivez pourquoi, puis **Enregistrer** (rien n'est inscrit sans raison).
Un second clic sur le pouce enfoncé annule le choix. Quand le drone a volé depuis le fragment, un bandeau **Fragment déjà
traité** le rappelle et liste vos choix : un conseil appliqué dit quel vol l'a **pris en compte** (le premier vol suivant,
daté du jour du choix ou après ; la date d'un vol étant au jour près, un vol du même jour fait avant le choix compte aussi).
Ces choix sont gardés dans la bibliothèque, avec le fragment.

## 6. Méthode de réglage

La méthode est le **parcours du guide intégré** (bouton **Guide**) : des cases à cocher, par drone.

- **Fiche drone** : **Nouveau drone** et **Modifier** (en haut de la page) ouvrent la fiche du drone (voir
  ci-dessous) ; la nouvelle est préremplie d'après le log ouvert. Les valeurs du guide s'y adaptent et chaque drone garde
  son avancement ; **Recommencer** décoche toutes ses cases.
- **Frise et liste des phases** : où vous en êtes (cases faites / total).
- **Cases** : un clic coche ; le bouton **ⓘ Aide** affiche l'aide ; **Ouvrir …** ouvre l'outil utile, avec la période
  d'analyse réglée sur la bonne partie du vol. Dès qu'un log du drone est ouvert, certaines cases se
  vérifient toutes seules (« ✔ Vérifié », ou « ▲ À revoir » avec la raison).
- **Logs du drone** (sous les cases), en deux parties : **logs des filtres** et **logs des séries PID**.
  Ouvrez-les avec le bouton de la case « Récupérer le log et l'ouvrir » (phase Filtres) ou « Ouvrir les logs
  de la série » (phase PID, et sa pareille pour la série Feedforward des Finitions) : chaque log garde ce rôle, et les cases d'une phase ne lisent que les logs de
  cette phase. Le rôle appartient au **fragment** (celui que montre l'écran d'extraction) : deux fragments d'un même
  vol peuvent servir l'un aux filtres, l'autre à une série. Un fragment sans rôle est classé tout seul (partie
  stationnaire → filtres, sinon série PID) ; le lien « → classer dans les … » corrige le classement. Seuls les
  fragments **actifs** de la barre du haut sont vérifiés : un fragment désactivé est nommé sous les logs, sans plus. Pour chaque log : à quoi il sert (identifier le
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
