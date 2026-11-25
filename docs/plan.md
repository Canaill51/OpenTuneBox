# Plan de travail

**Le seul plan du projet.** Ce qu'il reste à faire est en haut ; l'historique des lots déjà faits est un
tableau en bas, une ligne par lot. Il n'y a **pas d'autre document de plan** : l'état d'un travail reporté
se lit dans [travaux-reportes.md](travaux-reportes.md), jamais ici (L-092).

**État au 2026-11-10, vérifié** : la **0.9.0 est publiée** (page « dernière version », AppImage et
installateur Windows), les huit outils de PIDtoolbox Pro V0.94 sont tous couverts
([spec-ecrans.md](spec-ecrans.md), `rust/otb-gui/src/tools/mod.rs:all_pages`), `main` est propre, la CI est
verte, et `cargo run -p otb-dev -- verify --strict` avec le corpus, les références figées et la comparaison
au bit près : **0 saut, tout passe**.

Le programme est donc complet et publié, et depuis le 2026-11-10 **il ne reste que du confort** (lot 15 ;
le lot 11 est fait le même jour). Les deux autres familles ont disparu le même jour : vérifier ce que seule une vraie machine vérifie
(lot 13) n'avait plus d'objet, **personne n'ayant jamais utilisé la 0.8.0** ; et **plus aucun seuil de
l'assistant n'est sans origine ni condition** (lot 14), sans qu'une seule valeur ait eu à changer. Le 2026-11-12, le
lot 21 a relu les licences livrées et les sources, en vue de la **0.10.0** (la 1.0 est gardée pour plus tard).

**Chantier ouvert le 2026-11-18 — bibliothèque, refonte et USB.** Deux études de Cédric, hors dépôt,
ouvrent un chantier bien plus proche de l'utilisateur : un **canevas de neuf maquettes**
du 2026-11-16 (tableau de bord par drone, navigation à deux niveaux, espace « Bruit et filtres » fusionné, écran
d'extraction d'un **fragment** de vol, assistant en cartes) et une **spécification** du 2026-11-14 (récupération
automatique des logs quand on branche un drone en USB, **stockage** des logs et des fragments, petite base
**SQLite**, suivi des mesures dans le temps contre une fiche cible). Repris en lots **28 à 32** : la bibliothèque de
données d'abord (lot 28, crate `otb-store`), parce que les fragments, la refonte et l'USB s'appuient tous dessus.
**Les lots 28 à 31 sont faits** (2026-11-18 au 2026-11-23) ; le 32 a son protocole et son écran, et n'attend plus que
des essais sur du vrai matériel. Ce chantier
**change la vie d'un pilote** — moins de gestes entre l'atterrissage et l'analyse, et des logs qui ne se perdent
plus. Deux arbitrages sont déjà rendus (2026-11-18) : le stockage se fait en **fichiers JSON** et non en base
(lot 28), et **D-015 est amendée** pour laisser une entrée de navigation regrouper plusieurs vues (lot 30). La seule
dépendance nouvelle, pour l'USB, est `serial2` (lot 32a).

**Repris d'un fork le 2026-11-22.** Les lots 28, 29a et 30a à 30c bis ont été écrits du 2026-11-18 au 2026-11-19 dans
le fork `GerryNatif/OpenTuneBox` (sa demande de fusion n° 1), parti de la 0.10.0 ; ils sont repris ici par-dessus le lot 22,
avec leurs registres renumérotés — deux décisions devenues **D-060** et **D-061**, un travail reporté devenu **W-069** —
parce que leurs numéros du fork étaient déjà pris ici. Le même fork porte un second chantier, **scripts firmware** (ses
lots 22 à 27 et sa décision sur la forme d'un script), écrit le
2026-11-18 **avant** le lot 22 de ce dépôt. Il est **repris dans les registres le 2026-11-22** (sa demande de fusion
n° 2), relu contre le rejeu de la chaîne de filtres (D-058) : décision **D-062**, travaux reportés **W-071** et **W-072**,
contrat d'un script en [spec-betaflight.md](spec-betaflight.md) §8. Cédric a confirmé ses quatre arbitrages du 2026-11-18
et en a ajouté trois : le chantier est **conditionnel** (lots 23 à 25, en attente de vrais logs INAV ou ArduCopter :
pour Betaflight seul, le lot 22 a déjà vérifié le simulateur sur de vrais vols) ; le lot 24 se prouvera par le banc
**et** par le contrôle par étage du lot 22 ; et le seul écart qui touche déjà l'écran, l'**ordre du terme D**, est
corrigé à part (lot 33). Ses lots 26 (INAV) et 27 (ArduCopter) restent sortis du plan. Le 2026-11-23, Cédric a levé la
condition pour le **lot 23 seul** (D-062 amendée) : fait le même jour, sans changer un seul résultat. Le 2026-11-24, il
l'a levée pour les **étapes 24a à 24c** (rien de branché) : 24a, 24b et 24c faits le même jour ; puis, au vu de leurs chiffres
et de la cause de l'écart du D (24c bis), pour la **bascule** (24d, 24e), faite le même jour. L'écart au vol affiché (dernier point du
lot 24) est fait le même jour, à sa demande ; le lot 25 reste conditionnel.

## Comment s'en servir

- **Une session = un lot = une PR.** Faire `/clear`, puis demander « Lot N de `docs/plan.md` ». La branche
  porte le nom indiqué dans le lot ; la PR a pour titre une phrase en français qui dit l'effet du changement et
  une seule étiquette `RN: …` (`CONTRIBUTING.md`, D-056).
- Chaque lot suit les phases **explorer → planifier → implémenter → vérifier → livrer → consigner**.
  Explorer = lire les fichiers listés dans « À lire ». Planifier = présenter le découpage et attendre
  l'accord de Cédric. Consigner = cocher les cases du lot, mettre à jour les registres ([decisions.md](decisions.md), [lecons.md](lecons.md),
  [travaux-reportes.md](travaux-reportes.md)) selon la compétence `docs-sync`.
- **Un nouveau test se prouve par contre-preuve** : réintroduire le défaut (ou le casser à la main), voir le
  test rougir, le noter dans la PR (L-071).
- **Vérification de tout lot** : `cargo run -p otb-dev -- verify` **sans tube** (un `| tail` masque son
  échec, L-022), vert avant de livrer.
- **Critère d'arrêt** : si une hypothèse d'un lot tombe, s'arrêter, consigner ce qui a été vu, et revenir
  vers Cédric ; ne pas improviser un autre découpage.
- Modèle conseillé par lot : **Sonnet** pour le mécanique, la documentation, les tests ; **Opus** pour le
  décodeur, la mémoire et le débogage difficile.
- La numérotation des lots est une **série unique**, jamais réutilisée : les lots 0 à 14, 16 à 23, 28 à 31
  et 33 sont au tableau en bas (faits, non retenus ou supprimés), le 15, le 32, le 34, le 35 et le 36 sont ouverts (les parties
  faites du 32 y sont déjà) ; les **24 et 25** restent du chantier des scripts firmware, **conditionnel** (D-062 ; 24a à 24e faits, reste l'écart au vol affiché), et les **26 et 27** en sont
  sortis le 2026-11-18 ; le prochain numéro libre est **37**.

## Ordre des étapes

| # | Lot | Sujet | Dépend de |
|---|---|---|---|
| 1 | **32** | Récupération des logs par USB (MSP / MSC) — le plus risqué | 32a (protocole, sans interface) **fait le 2026-11-22** (crate `otb-usb`, essai sur une vraie carte par `otb-dev usb`) ; 32b (écran, voie MSP) **fait le 2026-11-23** ; 32c-1 (stockage de masse et cartes SD, protocole) et 32c-2 (leur écran) **faits le 2026-11-23** (D-075), mode disque essayé sur la G474 ; reste une carte SD réelle et Windows |
| 2 | **15** | Dette de l'interface (W-052, pages spectrales) — **conditionnel** | le volet « cache » est soldé (L-094, D-053) ; le reste attend un déclencheur mesuré |
| 3 | **34** | Emplacement de la bibliothèque au choix, avec déplacement sûr | **fait le 2026-11-25** (D-081) ; reste l'essai à la main sous Linux et Windows |
| 4 | **35** | Tri des fenêtres de la réponse indicielle, puis recalibrage de l'assistant | **fait le 2026-11-25** (D-082), essayé à la main par Cédric |
| 5 | **36** | Retirer la page « Curseurs PID » | lot 35 (fait le 2026-11-25 : mêmes fichiers de la réponse indicielle) ; demandé par Cédric le 2026-11-25 |
| — | **25** | Plages de versions et veille des changements du firmware — **conditionnel** | lot 24 (fait le 2026-11-24) |

Il ne reste que du **confort**, mesuré comme tel : le budget de 60 images par seconde est tenu avec une marge
de 3 sur la plus longue session du corpus. Les lots qui touchaient à un utilisateur réel ont été réglés le
2026-11-10 : le **13** s'est avéré sans objet (personne n'a eu la 0.8.0), le **14** est fait sans qu'aucune
valeur de seuil ait eu à changer, et le **11** a livré l'ouverture de plusieurs logs à la fois (× 2,26). Il ne
reste que le **15**, dont le volet « cache des outils » est **soldé le 2026-11-10** : mesuré (L-094), allégé, puis borné
(D-053) — **W-035 est clos**. Ses autres volets (tracé du viewer, pages spectrales) attendent toujours leur déclencheur
mesuré : ils peuvent attendre que les retours des pilotes disent ce qui gêne vraiment.

Le **2026-11-22**, Cédric a ouvert le **lot 22** : ce n'est pas du confort, mais la **précision du simulateur de
filtres**, jusqu'ici jamais vérifiée filtre par filtre contre le vrai firmware. Il est **terminé le même jour** (22a à
22f, historique en bas) : la page Filtres et l'assistant rejouent la chaîne du firmware (D-058, D-059).

---

## Lot 15 : dette de l'interface (Opus) — **conditionnel**

**But** : reprendre les trois dettes connues de l'interface. **Ce lot ne se fait pas « pour faire bien »** :
le budget de 60 images par seconde est tenu avec une marge de 3 sur la plus longue session du corpus
(5,6 ms contre 16,7). Il attend un **déclencheur mesuré**.

**Déclencheurs** (l'un suffit) : le viewer accroche sur un log plus long que le corpus ; on veut afficher
beaucoup plus de courbes à la fois ; un pilote signale une lenteur. (La mémoire du cache des outils était le
quatrième : elle a été mesurée le 2026-11-10, et le cache est borné depuis — D-053.)

**Branche** : `perf/viewer-draw` (W-052) ou `refactor/spectral-pages` selon le déclencheur.

**À lire** : [travaux-reportes.md](travaux-reportes.md) W-052 (W-035 est clos : son point 9 a été mesuré puis borné, D-053),
`rust/otb-gui/src/viewer_page.rs:ui`, `rust/otb-gui/src/tools/freq_time.rs` (la texture en cache, le modèle
à suivre), [decisions.md](decisions.md) D-037 et D-048, [lecons.md](lecons.md) L-085, L-086.

- [ ] **W-052** — le cadre est désormais dominé par le **tracé** des ≈ 3 840 points par courbe (4,7 ms en
      vue entière contre 2,1 ms zoomé sur un huitième du même log, à nombre de points égal). Deux pistes, à
      mesurer avant de choisir : un seul segment vertical par colonne de pixels au lieu de deux points, ou la
      courbe décimée dessinée dans une texture en cache comme `freq_time`. Preuve exigée, comme au lot 10 :
      **tracé identique** (captures au bit près, les deux thèmes) et chiffres de `banc_cadre` avant / après.
- [x] **Cache des outils : mesuré** (2026-11-10, `examples/banc_cache.rs`, tout le corpus — 535 logs, 642 sessions ;
      [validation.md](validation.md), « Mémoire du cache des outils »). Une entrée pèse 7,5 Mo (p50) à **72,6 Mo** pour
      l'assistant et 7,4 à **71,3 Mo** pour la réponse indicielle ; le simulateur de filtres, seul suspect de départ
      parce que son nombre d'entrées n'est pas borné, ne pèse que 90 ko. **69 % du rapport de l'assistant** était les
      fenêtres individuelles de la réponse indicielle, que personne n'y lit : libérées, l'entrée tombe d'un facteur 24
      (médiane) à 165 (L-094, `rust/otb-advisor/tests/rapport_memoire.rs`, contre-preuve vue en échec).
- [x] **Cache des outils borné** (2026-11-10, D-053) : plafond **par outil**, en nombre d'entrées — 3 réponses
      indicielles, 20 simulations de filtres, 6 rapports, 2 vérifications du Guide — la moins récemment utilisée jetée la
      première. Le pire cas d'un log ouvert passe de **5,1 Go à ≈ 223 Mo**. Quatre tests dans
      `rust/otb-gui/tests/state.rs` (module `plafond`), contre-preuves vues en échec ; 28 captures identiques au bit
      près. **W-035 est clos.**
- [ ] **Factorisation des pages spectrales** : les pages `spectral`, `freq_throttle` et `freq_time` répètent
      la même mécanique, comme la découpe de période (×6), `breaks` (×5), la découpe bornée (×9), les listes
      déroulantes (×5) et `reset_view` (×5). À ne faire que si une page de plus est prévue, sinon c'est du
      remaniement sans effet.
- [ ] **Lints pédantiques ciblés** : activer quelques lints `clippy::pedantic` utiles, **jamais
      `suboptimal_flops`** (L-051 : il change le résultat des calculs).
- [ ] **Critère d'arrêt** : un tracé qui diffère d'un pixel entre avant et après, ou un gain inférieur
      à ×1,5.

---

## Lot 32 : récupération des logs par USB (Opus) — **le plus risqué**

**But** : brancher le drone, récupérer, vérifier, puis seulement effacer. Un log perdu est un vol à refaire ; un
effacement reporté ne coûte rien.

**Branche** : `feat/usb-retrieve`. **Dépend du lot 30** ; la bibliothèque et le refus des doublons existent déjà.

**À lire** : [spec-usb.md](spec-usb.md) (ce que fait le firmware, vérifié le 2026-11-22), [decisions.md](decisions.md) D-063.

**Découpage** (Cédric, 2026-11-22, D-063) : **32a — le protocole**, sans interface et avant la fin du lot 30 : trames MSP
v1 (jumbo) et v2, identité, lecture par blocs, effacement, `diff all`, port simulé pour les tests, et la suite des
contrôles avant effacement ; port série par `serial2`. **32b — l'écran**, après le lot 30. **Aucune écriture de
réglage sur la carte** : les « Et si » restent des lignes CLI.

**32a fait le 2026-11-22** (branche `feat/usb-protocol`, crate `otb-usb`, détail dans [architecture.md](architecture.md)) :
trames, identité, copie MSP par blocs avec progression et arrêt, contrôles avant effacement et seul chemin qui efface,
`diff all`, port simulé, verrou du port sous Linux, erreurs d'ouverture expliquées. **Reste pour 32b** : l'écran, la voie
du stockage de masse (redémarrage en MSC, fichiers `BTFL_*.BBL`, carte SD effacée par le système avec les mêmes
contrôles), le rangement dans la bibliothèque (drone reconnu par l'identifiant de la carte, `diff all` gardé seulement
s'il change : `rust/otb-usb/src/cli.rs:config_changed`), et les essais restants sur une vraie carte de [spec-usb.md](spec-usb.md)
(procédure d'essai à la main, par `otb-dev usb`).

**Découpage de 32b** (Cédric, 2026-11-23, D-074) : **32b — l'écran et la voie MSP** ; **32c — le mode stockage de masse
et les cartes SD**.

**32b fait le 2026-11-23** (branche `feat/usb-retrieve`, D-074, W-075 traité) : écran « Récupérer par USB » hors rail
depuis le tableau de bord (`usb_page.rs`), connexion tenue par un fil de travail (`usb.rs`), drone reconnu par
l'identifiant de sa carte (`Drone::uid`, demandé la première fois), copie rangée sous le drone et bibliothèque écrite
avant de proposer l'effacement (deux clics), `diff all` par un bouton, « Ouvrir les vols ». Parcours complet essayé
sur la carte G474 sous Linux. **Reste pour 32c** : la voie du stockage de masse et les cartes SD ; à essayer encore :
drone armé, port occupé, Windows ([spec-usb.md](spec-usb.md)).

**Découpage de 32c** (Cédric, 2026-11-23, D-075) : **32c-1 — le protocole**, sans écran ; **32c-2 — l'écran**. Arbitrages :
deux voies au choix pour la mémoire interne, disque attendu du système avec « Choisir le dossier… » en secours, carte SD
dans le drone **et** dans un lecteur, suppression des logs SD vérifiés en deux clics.

**32c-1 fait le 2026-11-23** (branche `feat/usb-mass-storage`, D-075, W-079 noté) : `otb-usb` — redémarrage en disque
(`msc.rs:reboot_to_disk`, seul mode 2), disques repérés (`mounted_volumes`, `inspect`, `new_disks`), copie de
`BTFL_ALL.BBL` par les mêmes contrôles et permis d'effacer après rebranchement (identifiant connu seulement), logs d'une
carte SD copiés et vérifiés un par un, et **seul chemin qui supprime un fichier** (`sd.rs:delete_sd_logs`, relecture
comparée juste avant) ; essai à la main par `otb-dev usb msc` et `usb sd`. **Reste pour 32c-2** : l'écran (choix de la
voie, attente du disque et « Choisir le dossier… », copie rangée sous le drone, rebranchement pour effacer la mémoire,
suppression SD en deux clics, carte SD dans un lecteur sans drone branché), et l'essai sur une vraie carte
([spec-usb.md](spec-usb.md), procédure).

**32c-2 fait le 2026-11-23** (branche `feat/usb-mass-storage-screen`, D-075 complétée, D-074 amendée) : « Copier en mode
disque » à côté de « Copier les vols » (USB série, bouton principal), « Copier la carte SD (mode disque) » pour une carte
sans mémoire, attente du disque et « Choisir le dossier… », copie de la mémoire gardée jusqu'au rebranchement
(`rust/otb-gui/src/usb.rs:Held`) et reconnexion seule, suppression des logs SD en deux clics, section « Carte SD dans un
lecteur ». Essayé à l'écran sur la carte G474 sous Linux (mode disque, puis effacement après rebranchement : tout passe).
**Reste** : la carte SD sur une vraie carte (quand Cédric en aura une), drone armé, port occupé, Windows
([spec-usb.md](spec-usb.md)).

- [x] **Identité de la machine** par MSP (32a ; le rangement par identifiant : 32b) : nom, identifiant de la carte, version du firmware, état armé,
      occupation de la mémoire. L'identifiant de la carte est la clé ; le nom seul ne suffit pas (deux drones
      peuvent le porter, et il se change en une commande).
- [ ] **Deux voies de copie** : mode stockage de masse quand la carte le permet (rapide, seule voie pour une carte
      SD ; protocole **fait en 32c-1**, écran **fait en 32c-2**), repli sur la lecture bloc par bloc en MSP (lent : plusieurs minutes pour 16 Mo ; **fait en 32b**,
      barre de progression et temps restant, « Arrêter » ne garde rien et la copie suivante reprend au début).
- [x] ⚠️ **Jamais d'effacement sans vérification** (32a : `rust/otb-usb/src/erase.rs`, l'écran de confirmation : 32b), dans cet ordre : taille copiée = place annoncée, le décodeur
      ouvre chaque log et en extrait au moins un fragment, les fichiers sont écrits sur le disque, **puis**
      l'utilisateur confirme. Un échec de lecture interdit l'effacement, quelle que soit l'option choisie.
- [x] ⚠️ **Sécurité** (32b) : drone armé → aucune opération (copie refusée par le fil, boutons grisés, état relu avant
      d'effacer) ; rappel des hélices en tête de l'écran et à la confirmation ; un seul accès au port à la fois et droits
      ou pilote expliqués (messages d'`otb_usb::link::PortError`, affichés tels quels). À essayer sur une vraie carte :
      [spec-usb.md](spec-usb.md).
- [x] **Sauvegarde de la configuration** (32b, bouton à part, D-074) : `diff all`, gardée seulement si elle diffère de la
      précédente ; la fiche du drone compare alors deux versions des réglages.
- [x] **Les huit points à vérifier** de l'étude, dans le code et la documentation de Betaflight, **avant** d'écrire
      (elle a été rédigée de mémoire) : faits le 2026-11-22, branche « docs/usb-firmware-check », consignés dans
      [spec-usb.md](spec-usb.md). Six tranchés par le code ; le **débit MSP** et la **durée d'un effacement** se mesurent
      sur une vraie carte (procédure dans le même document) ; **appliquer les réglages « Et si » sur la carte** reste
      à Cédric. Deux constats changent le dessin : le firmware **n'empêche ni la lecture ni l'effacement quand le drone
      est armé** (l'état armé se relit juste avant d'effacer), et la mémoire interne est **en lecture seule en mode
      stockage de masse** (effacer demande de revenir en MSP). Bibliothèque série : `serial2` (BSD-2 ou Apache-2.0),
      `serialport` étant refusée par sa licence MPL-2.0.
- [x] ⚠️ **Nouvelle dépendance et nouvelle surface** (32a : `cargo deny` vert, port simulé, procédure dans [spec-usb.md](spec-usb.md)) : licence vérifiée par `cargo deny` (`rust/deny.toml`), et
      **aucun matériel en CI** — donc un port simulé pour les tests automatiques et une procédure d'essai à la main
      écrite dans la documentation, comme pour les installateurs.
- [ ] **Critère d'arrêt** : un seul chemin de code où l'effacement pourrait partir sans vérification complète →
      s'arrêter net. **Non atteint en 32b** : la copie vérifiée ne quitte pas le fil de travail, seule la demande
      d'effacement en fait un permis, et l'écran ne la propose qu'après rangement et écriture de la bibliothèque
      (`rust/otb-gui/tests/usb.rs`). À revoir en 32c (l'effacement d'une carte SD passera par le système). **Non atteint
      en 32c-1** : une seule suppression de fichier dans `otb-usb`, sous permis, après relecture comparée
      (`rust/otb-usb/tests/contrats.rs:un_seul_chemin_supprime_un_fichier`) ; à revoir en 32c-2 pour l'écran. **Non
      atteint en 32c-2** : les copies SD vérifiées ne quittent pas le fil du disque, et la copie de la mémoire ne sort de
      `Held` que pour une connexion à la même carte (`rust/otb-gui/tests/usb_disque.rs`).

---

## Lot 34 : emplacement de la bibliothèque au choix (Opus)

**But** : laisser l'utilisateur **choisir où vit sa bibliothèque** (fiches, fragments, mesures, logs copiés par USB) et
l'y **déplacer** sans rien perdre : un disque plus grand pour les logs copiés, une sauvegarde simple, la même bibliothèque
d'un PC à l'autre. Demandé par Cédric le 2026-11-24, pendant l'essai de `essai-v0.10.0`. Jusque-là le dossier était fixe :
`<données>/OpenTuneBox/OpenTuneBox/bibliotheque/` (aujourd'hui le dossier par défaut, `rust/otb-store/src/lib.rs:default_dir`), le doublon `OpenTuneBox/OpenTuneBox`
venant du chemin de Qt de la version Python, gardé pour reprendre l'ancien `drones.json` (D-068).

**Branche** : `feat/library-location`.

**À lire** : `rust/otb-store/src/lib.rs` (`default_dir`, `dir_in`, `Library::open`, `logs_dir`, `delete_copies_of_drone`),
`rust/otb-gui/src/main.rs` (ouverture au lancement), `rust/otb-gui/src/guide.rs:cards_path_in` (reprise de l'ancien
fichier), [decisions.md](decisions.md) D-064 et D-068, [architecture.md](architecture.md) (`otb-store`).

Fait le 2026-11-25 ([D-081](decisions.md)), sauf l'essai à la main.

- [x] **Pointeur** : `emplacement-bibliotheque.json` à côté du dossier par défaut (`rust/otb-store/src/place.rs:locate`) ;
      absent, rien ne change pour qui ne déplace rien.
- [x] **Réglage** : écran hors rail « Bibliothèque » ouvert du tableau de bord (`rust/otb-gui/src/library_page.rs`) :
      chemin, contenu, « Ouvrir le dossier », « Changer d'emplacement… », « Revenir à l'emplacement par défaut ».
- [x] **Déplacement sûr** : écritures suspendues, copie relue fichier par fichier, bascule, **puis** ancien dossier
      effacé ; refusé pendant une récupération USB. Logs copiés écrits **relatifs** à la bibliothèque (choix de Cédric) ;
      les anciens chemins complets se relisent dans le nouveau dossier. Les logs ouverts sur place ne bougent pas.
- [x] **Bibliothèque déjà présente** dans le dossier choisi : proposée telle quelle, jamais fusionnée ni écrasée.
- [x] **Dossier introuvable au lancement** : bibliothèque en mémoire qui n'écrit rien, écran ouvert avec « Réessayer »,
      « Choisir un autre dossier… », « Revenir à l'emplacement par défaut ».
- [x] **Dossier synchronisé ouvert sur deux PC** : témoin d'ouverture et **avertissement** à l'ouverture, sans blocage
      (choix de Cédric).
- [x] Tests sur dossiers temporaires (`rust/otb-store/tests/emplacement.rs`, `rust/otb-gui/tests/bibliotheque_emplacement.rs`) ;
      guide utilisateur et [architecture.md](architecture.md) à jour.
- [ ] **Essai à la main** sous Linux **et** Windows : déplacer vers un disque externe, relancer, débrancher le disque et
      relancer (à faire par Cédric).
- [ ] **Critère d'arrêt** : un seul fichier perdu ou un log copié introuvable après un déplacement. **Non atteint** dans
      les tests : chaque fichier comparé après déplacement, chaque log copié retrouvé sur le disque.

---

## Lot 35 : tri des fenêtres de la réponse indicielle (Opus)

**But** : une réponse indicielle qui ne soit plus tirée vers le haut par des fenêtres de 2 s aberrantes. Constaté par Cédric
le 2026-11-25 en comparant à PIDtoolbox web (version 0.3) sur `Cedric/Air65/Test BF Chirp/CHIRP-65Hz.bbl` : **même forme**
(creux et bosses aux mêmes instants, donc même déconvolution), mais notre courbe est **plus haute de 0,08 à 0,1** (roulis :
pic 1,17 contre ≈ 1,05, palier 1,11 contre ≈ 1,03) parce qu'elle moyenne **bien plus de fenêtres** (roulis n = 378 contre
224). Notre seul tri (`rust/otb-dsp/src/step.rs`, `0,5 < min et max < 3` sur 200–500 ms) laisse passer les nombreuses
fenêtres de roulis qui montent au-dessus de 1,4 (vue « Brutes »). PIDtoolbox web expose désormais un tri réglable
(début du régime établi, largeur de la bande, départ maximal, distance à la médiane). **Choix de Cédric (2026-11-25)** :
D-004 interdit de le copier, et des noms de réglages ne disent pas la méthode ; on écrit **notre propre tri**, sur des
statistiques classiques, qui ira dans le même sens sans retomber exactement sur ses chiffres ; l'assistant sera
**recalibré**.

**Branche** : `feat/step-window-rejection`.

**À lire** : `rust/otb-dsp/src/step.rs` (`step_windows`, contrôle des fenêtres), `rust/otb-analysis/src/step.rs`
(`StepOptions`, `step_response`), `rust/otb-analysis/src/step_series.rs`, `rust/otb-gui/src/tools/step.rs` et
`step_common.rs`, [spec-methodes.md](spec-methodes.md) « Réponse indicielle », [assistant.md](assistant.md) (seuils de pic
et de latence), `rust/otb-advisor/src/profiles.rs`, `rust/otb-dev/src/corpus/calibrate.rs`, `rust/otb-parity/src/analysis_layer.rs`
et `advisor_layer.rs` ; compétences `assistant-seuils` et `calculs-corpus`.

**Premier essai (2026-11-25, hors dépôt, CHIRP-65Hz)** : garder une fenêtre seulement si sa réponse reste **entre 0,5 et
1,5 dès 150 ms** ; en tangage, écarter en plus les fenêtres dont l'écart quadratique à la **courbe médiane** dépasse
médiane + 3 × MAD (écart absolu médian × 1,4826).

| Tri | Roulis n | Pic | Palier | Tangage n | Pic | Palier |
|---|---|---|---|---|---|---|
| Actuel | 381 | 1,17 | 1,11 | 287 | 1,12 | 1,06 |
| Bande 0,5–1,5 dès 150 ms | 282 | 1,11 | 1,02 | 250 | 1,12 | 1,00 |
| + distance à la médiane | 282 | 1,11 | 1,02 | 237 | 1,11 | 0,98 |
| PIDtoolbox web (capture de Cédric) | 224 | ≈ 1,05 | ≈ 1,03 | 274 | ≈ 1,11 | ≈ 1,02 |

Un départ maximal (`|r(0)| ≤ 0,1`) n'écarte rien sur ce log : à garder seulement s'il sert sur le corpus.

- [x] **Tri** (35a, [D-082](decisions.md)) : bande sur le régime établi (`RobustRejection` : début, largeur) et distance à
      la courbe médiane (k) ; second passage pour les mesures de boucle ; décrit dans [spec-methodes.md](spec-methodes.md).
      Valeurs par défaut à confirmer sur le corpus (recalibrage).
- [x] **Concordance Python** (35a) : tri de Python gardé (`StepOptions::python`, `rules::Method::python`,
      `flight_step_with`, `check_log_with`) pour `otb-parity` et les tests sur références ; l'application prend le nouveau.
- [x] **Écran** (35b) : « Tri des fenêtres » sous « Calcul » (régime établi dès, bande gardée, écart à la médiane) ; la
      légende dit « n=281 sur 456 » et passe sur deux lignes par fichier, dans la moitié droite : elle ne cache plus la
      montée (`rust/otb-gui/tests/step_tools.rs`).
- [x] **Recalibrage de l'assistant** (35c, choix de Cédric) : `otb-dev corpus calibrate` et `rates` sur les 678 sessions avec
      les deux tris (`--tri-python`) ; paliers du pic 1,15 / 1,20 et 1,30 / 1,45 (D-025 amendée), réponse molle 1,00,
      latence du 5" 11–22 ms, écart roll / pitch 0,08 commun et 0,07 en 5" ; dépassements signalés 11 → 15 % des sessions ;
      mesures suivies refaites (`CALC_VERSION` 2). Chiffres : [assistant.md](assistant.md).
- [x] **Validation** : logs CHIRP d'Air65 (25, 45, 65 Hz) dans le sens de PIDtoolbox web ([validation.md](validation.md)) ;
      série BlackSummer (captures `PTB_FIGS_2026-08-16`) : au plus 0,015 d'écart sur ses vols propres, repère 1,03 gardé ;
      contre-preuves des tests du tri, de l'écran et du rang des fenêtres.
- [x] **Essai à la main** par Cédric (2026-11-25, « c'est ok ») : page Réponse indicielle (réglages du tri, légende), Assistant et fiche cible d'un
      drone (mesures suivies refaites à l'ouverture de la fiche).
- [ ] **Critère d'arrêt** : un tri qui écarte la majorité des fenêtres d'un vol propre, ou qui change le sens d'un
      conseil de l'assistant sans que la mesure l'explique → s'arrêter et revenir vers Cédric. **Non atteint** : 60 à 92 %
      des fenêtres gardées sur les logs CHIRP et BlackSummer ; chaque taux qui bouge suit un seuil recalculé.

---

## Lot 36 : retirer la page « Curseurs PID » (Sonnet)

**But** : enlever de la barre de gauche la page **Curseurs PID** (positions des 8 curseurs du réglage simplifié,
simulation « et si… », commandes CLI à coller). Demandé par Cédric le 2026-11-25 : elle n'est pas vraiment utile et
**peut prêter à confusion**. Il a choisi de retirer **la page seule** :

- **reste** le graphique « Curseurs PID Betaflight » sous le diagnostic de la page **Réponse indicielle** ;
- **reste** la ligne « Curseurs simplifiés » de la page **Configuration** ;
- **reste** tout le calcul (`rust/otb-analysis/src/sliders.rs`) : il sert à l'assistant (`rules.rs`), à la vérification
  des logs et au guide (`log_check.rs`, `guide_page.rs`), au simulateur de filtres (`filter_sim_calc.rs`) et aux
  références figées (`otb-parity`, `check-sliders`).

**Branche** : `feat/remove-pid-sliders-page`.

**À lire** : `rust/otb-gui/src/tools/pid_slider.rs`, `rust/otb-gui/src/tools/mod.rs:all_pages`, `rust/otb-gui/src/app.rs`
(icône et ordre du rail), `rust/otb-gui/src/guide.rs:TOOLS`, `rust/otb-gui/src/tools/step_common.rs:slider_plot`,
[decisions.md](decisions.md) D-015, D-060 et D-061, [spec-ecrans.md](spec-ecrans.md), [architecture.md](architecture.md).

- [ ] **Décision** (numéro pris par `otb-dev numero D`) : la page est retirée ; l'objectif « couvrir tous les outils de
      PIDtoolbox Pro V0.94 » devient « tous sauf Curseurs PID » (AGENTS.md, README, état en tête de ce plan) ; l'écart
      au prototype (D-061, qui dessine cette page) est écrit.
- [ ] **Code** : supprimer `pid_slider.rs`, son entrée dans `all_pages`, son icône et sa place dans le rail, son bouton
      dans `guide.rs:TOOLS`. La case `sliders` du parcours (`rust/otb-advisor/src/tuning_path.rs`, « Curseurs de
      réglage PID en mode RPY ») est une étape de réglage dans le configurateur, pas un lien : elle reste. Simplifier ce
      qui ne servait qu'à la page (le losange de simulation de `slider_plot`, par exemple).
- [ ] **Tests** : ôter ceux de la page (`rust/otb-gui/tests/step_tools.rs`, section « Curseurs PID » ; `rust/otb-gui/tests/app.rs`,
      ordre du rail ; `rust/otb-gui/tests/contracts.rs`, icône ; `rust/otb-gui/examples/check_tools.rs`) en le disant dans la PR — retirer le test
      d'un écran supprimé n'est pas affaiblir un test ; garder ceux du calcul et du graphique de la réponse indicielle ;
      un test vérifie que le rail n'a plus la page.
- [ ] **Documentation** : [guide-utilisateur.md](guide-utilisateur.md) (section « Curseurs PID » et liste du rail),
      README, [architecture.md](architecture.md), [spec-ecrans.md](spec-ecrans.md) ; compétence `docs-sync`.
- [ ] **Critère d'arrêt** : un texte de l'assistant ou du guide qui envoie l'utilisateur vers cette page (aucun trouvé
      le 2026-11-25) : s'arrêter et demander à Cédric où le renvoyer.

**Recouvrement** : le lot 35 (fusionné le 2026-11-25) a touché les mêmes fichiers de la réponse indicielle
(`rust/otb-gui/src/tools/step.rs`, `step_common.rs`, `rust/otb-gui/tests/step_tools.rs`) : partir du `main` du jour.

---

## Lots 23 à 25 : scripts firmware — **conditionnels** (le 23 et le 24 sont faits)

Repris du fork `GerryNatif/OpenTuneBox` (sa demande de fusion n° 2, écrite le 2026-11-18 avant le lot 22), relus le
2026-11-22 (D-062). **Déclencheur** : de vrais logs INAV ou ArduCopter, et la demande de Cédric. Pour un pilote Betaflight
seul, ces lots ne corrigent rien : le lot 22 a déjà vérifié le simulateur sur de vrais vols, et le calcul au bit près ne
réduirait pas l'écart qui reste (il vient de ce que le log n'enregistre pas). L'ordre du terme D, seul écart visible,
est sorti dans le lot 33.

### Lot 23 : la chaîne Betaflight décrite en données — **fait le 2026-11-23**

Voir l'historique en bas : la chaîne est décrite une seule fois, dans `rust/otb-analysis/chains/betaflight.json`
(`rust/otb-analysis/src/chain.rs`), forme en [spec-betaflight.md](spec-betaflight.md) §8.

### Lot 24 : briques à l'arithmétique du firmware (Opus) — **fait le 2026-11-24**

**But** : atteindre le bit à bit contre le firmware compilé sur PC, **et basculer l'application sur cette nouvelle
chaîne dans le même lot** (D-062). C'est le plus gros lot du plan : il déplace un chiffre dans presque tous les écrans,
y compris le rejeu de la page Filtres (D-058) et le bruit prévu par l'assistant (D-059).

**Branche** : `feat/firmware-exact-bricks`. **Dépend du lot 23** (fait le 2026-11-23).

**À lire** : `rust/otb-dsp/src/lib.rs` (les noyaux actuels, D-033), [decisions.md](decisions.md) D-029 (float32),
D-047 et D-011, [travaux-reportes.md](travaux-reportes.md) W-028, [lecons.md](lecons.md) L-048 et L-051
(⚠️ jamais `suboptimal_flops` : il change le résultat des calculs), [spec-betaflight.md](spec-betaflight.md) §2 (biquads
RBJ jusqu'à la 2025.12, filtres SVF ensuite : même réponse, autre arithmétique).

⚠️ **Pourquoi l'ordre des étapes n'est pas négociable.** La chaîne actuelle est la référence de tout : les
références figées, les seuils de l'assistant, les captures au bit près et le contrôle par étage du lot 22. La bascule
d'un coup a été choisie en connaissance de cause ; elle ne tient que si chaque étape ci-dessous est **livrée et vérifiée
avant la suivante**, et qu'aucune ne saute.

- [x] **24a — les briques, sans rien brancher.** Fait le 2026-11-24 : `rust/otb-dsp/src/firmware/betaflight/`, quatre
      arithmétiques (4.3–4.4, 4.5, 2025.12, 2026.6 et après) ; l'ordonnancement des recalculs dans la boucle (notchs RPM,
      LPF dynamiques) passe en 24c, avec la chaîne assemblée (W-081). Float32 à côté des actuelles, rangées par famille de firmware même si
      **seule celle de Betaflight est écrite** : tout en float32 sauf là où le firmware calcule lui-même en double ;
      approximations du firmware recopiées ; **ordre exact des opérations** (`état + k × (entrée − état)` n'a pas les
      mêmes bits que `(1 − k) × état + k × entrée`) ; initialisations et moment du recalcul des coefficients recopiés ;
      **deux domaines de cadence** réellement exécutés (cadence gyro, décimation, cadence PID). Rien de visible.
- [x] **24b — le banc et les vecteurs de référence.** Fait le 2026-11-24 : `otb-dev firmware-bench`, six versions,
      toutes identiques au bit près ([validation.md](validation.md#briques-à-larithmétique-du-firmware-2026-11-24-lots-24a-et-24b)) ;
      ne compile que `filter.c` et `maths.c` (W-081) ; licence d'INAV et d'ArduPilot non vérifiée : aucune de leurs briques n'est écrite. Le banc écrit par le projet télécharge et compile le firmware,
      ne garde que des vecteurs (D-062) ; licences d'INAV et d'ArduPilot vérifiées à leur source à ce moment-là.
      Balayage de fréquence, bruit, échelons, fréquences de notch variables. La CI les rejoue et exige l'**égalité bit à
      bit** ; elle n'a pas le corpus, donc tout se prouve sur vecteurs et sur `tests/data/jeno5_5s.bbl` (L-023).
- [x] **24c — l'écart chiffré sur tout le corpus, avant de brancher quoi que ce soit.** Fait le 2026-11-24 :
      `rust/otb-analysis/src/firmware_chain.rs`, choisie par `Engine` dans le contrôle par étage et le rejeu, `Engine::Model` par
      défaut ; `otb-dev corpus chaindiff`, `--chain firmware`, `corpus stages --compare`. Critère d'arrêt non atteint ; deux écarts
      expliqués et **corrigés dans la chaîne en service** à la demande de Cédric (W-082 : notch RPM au plafond ignoré ; L-120 :
      ordre des notchs RPM), aucun avis de l'assistant changé ; deux écarts non expliqués (W-083). Texte d'origine : Assembler la chaîne nouvelle à côté
      de l'ancienne (recalculs recopiés du firmware, W-081 point 1), sans rien changer à l'écran. Différence entre l'ancienne
      et la nouvelle chaîne sur tout le corpus, et `otb-dev corpus stages` / `corpus replay` relancés : le contrôle par
      étage **ne doit pas se dégrader** (preuve par les vrais vols, D-062). Chiffres dans [validation.md](validation.md).
      **C'est le point de non-retour** : un écart plus grand que prévu s'explique **ici**, pas après la bascule.
- [x] **24d — la bascule.** Fait le 2026-11-24 (branche `feat/firmware-chain-switch`, avec 24e, D-062 amendée) : page Filtres,
      assistant et contrôle par étage rejouent par la chaîne du firmware dès la 4.3 (`Engine::in_service`) ; l'ancienne reste pour
      les logs d'avant la 4.3 et les mesures (`--chain model`) ; **références figées non réenregistrées**, au choix de Cédric
      (elles ne portent que l'étalon, intact : W-028 et W-072 restent ouverts) ; captures refaites sur les deux thèmes. Un avis
      a basculé (`1. PTB/Kris/13. Hover.bbl`, seuil frôlé sur un log ralenti), accepté par Cédric
      ([validation.md](validation.md#bascule-sur-la-chaîne-du-firmware-2026-11-24-lots-24d-et-24e)). Texte d'origine :
      L'interface passe à la nouvelle chaîne, les références figées sont réenregistrées (W-028 et W-072 point 2 en profitent),
      et les captures sont refaites sur les deux thèmes.
- [x] **24e — les seuils de l'assistant revérifiés.** Fait le 2026-11-24 : `corpus calibrate` identique au bit près avant et
      après (aucune mesure de la calibration ne passe par le rejeu), aucun seuil changé. Texte d'origine : (`otb-dev corpus
      calibrate`, compétence `calculs-corpus`). ⚠️ Si un seuil change l'**avis** de l'assistant sur un vrai log, **s'arrêter**
      et revenir vers Cédric avec le log et le chiffre : c'est exactement le risque accepté en choisissant la bascule d'un coup.
- [x] **Écart au vol réel mesuré et affiché.** Fait le 2026-11-24 (branche `feat/flight-gap-display`) :
      `rust/otb-analysis/src/flight_gap.rs` (la mesure de `corpus replay`, qui l'appelle désormais) ; la page Filtres
      affiche au-dessus des spectres la pire bande du gyro et du terme D, le détail en bulle d'aide, et la cause quand le log
      est ralenti ou d'avant la 4.3. Ligne absente du prototype (D-061) : texte seul au-dessus des spectres qu'elle juge,
      rien du dessin déplacé. Texte d'origine : écart entre gyro simulé et gyro enregistré, jamais promis, montré dans
      l'interface avec sa cause quand le log ne contient pas l'entrée des filtres à pleine cadence.
- [ ] **Critère d'arrêt** : un vecteur qui ne tombe pas au bit près sans cause identifiée (approximation, ordre des
      opérations, initialisation), un étage du lot 22 qui se dégrade, ou un avis de l'assistant qui change sans
      explication → s'arrêter, consigner, revenir vers Cédric.

### Lot 25 : plages de versions et veille du firmware (Sonnet)

**But** : qu'un script dise de quelles versions il répond, et qu'un changement d'arithmétique en amont soit vu
**avant** qu'un pilote ne voie une simulation fausse.

**Branche** : `feat/version-ranges`. **Dépend du lot 24.**

- [ ] **Plage déclarée** par script : première version couverte, dernière version **vérifiée**, « ouverte » tant
      qu'aucun changement n'est connu. Au-delà de la dernière vérifiée, la simulation reste possible mais
      **marquée « non vérifiée »** dans l'interface.
- [ ] **Ce qui coupe une plage** ([spec-betaflight.md](spec-betaflight.md) §8) : arithmétique d'une brique, ordre de la
      chaîne, cadence, façon de calculer le terme D, noms ou plages des réglages. **Ce qui ne la coupe pas** : un nouveau
      réglage par défaut, un champ de log sans effet sur le calcul.
- [ ] **Veille** : un travail de CI compare, d'une version du firmware à la suivante, les fichiers qui portent le
      calcul, puis rejoue les vecteurs. Il **télécharge et compare, ne garde rien** (D-045), et prend `--locked`
      comme le reste de la CI.
- [ ] **Critère d'arrêt** : une veille qui crie à chaque version sans rien apprendre → la retirer plutôt que la
      laisser grincer.

## Hors plan, volontairement

Ces travaux reportés sont **vus et laissés** : aucun n'empêche un pilote d'utiliser OpenTuneBox aujourd'hui.
Leur détail et leur condition de reprise sont dans [travaux-reportes.md](travaux-reportes.md) ; ne pas les
rouvrir ici sans cette condition. Deux d'entre eux (W-043 et W-044) viennent du lot 13 supprimé : ils n'ont
jamais dépendu de la 0.8.0, et gardent donc leur condition de reprise. **W-041 a quitté ce tableau le
2026-11-10** : sa condition de reprise — un échec au hasard de la CI sur ce test — a été atteinte, et il est
clos (L-095). C'est à cela que sert une condition de reprise écrite d'avance. **W-043 l'a quitté le 2026-11-12** : il
a été traité par le lot 21.

| ID | Pourquoi il n'est pas dans un lot |
|---|---|
| W-001 | Logs KISS (`.TXT`) et INAV non décodés : hors périmètre, à rouvrir si Cédric change de radio ou de firmware |
| W-071 | ArduCopter (logs DataFlash) : périmètre **Betaflight seul** (D-062), aucun log ArduCopter réel dans le corpus |
| W-002 | Profils whoop, 2–3,5" et 8"+ appuyés sur trop peu de drones : il faut **plus de logs**, pas plus de calcul |
| W-004 | La boucle ouverte ne se mesure que jusqu'à 20–40 Hz : limite physique de l'énergie du manche |
| W-022 | Tolérance par fonction et non par champ dans `otb-parity` : reprise si un écart est soupçonné sur `freqs` / `bins` / `resolution_hz` |
| W-028 | Écarts des références figées : se règle au **prochain réenregistrement**, pas avant |
| W-029 (4) | Branche « crash » du guide : aucun crash réel dans le corpus, seuls des cas fabriqués |
| W-042 | Avis RustSec hérités d'eframe : ⏸ attend une version d'eframe qui change ses dépendances |
| W-044 (2) | `construire_windows.ps1` jamais essayé sur un vrai Windows : la CI fabrique l'installateur livré, ce script ne sert qu'à construire chez soi — à l'occasion. Son point 1 (`_internal`) est **clos** |
| W-051 | Pistes de performance écartées **par la mesure** le 2026-11-04 : ne pas les rouvrir sans mesure nouvelle |
| W-053 | 45 messages d'erreur imitant les exceptions de Python : atteignabilité non établie |
| W-054 | Huit des dix comparateurs d'`otb-parity` sans test : fabriquer une référence Python pour ces couches est le vrai coût |

**Écarté par arbitrage de Cédric** (« rien sans bénéfice démontrable ») : découper `rules.rs` (3 631 lignes),
`log_check.rs` (1 356) et `guide_page.rs` (1 053) — testés et validés au bit près, les remanier est un risque
sans effet visible ; les ~300 `pub` inutilement publiques (exposition trop large, pas du code mort) ; un cache
Cargo en CI (une mécanique à entretenir pour une CI qui dure 7 min).

---

## Historique des lots

Une ligne par lot fait — ou par lot abandonné, pour que son numéro ne serve jamais à autre chose. Le détail est dans les demandes de fusion et les pages Releases (jusqu'au lot 20 : l'ancien journal,
`git show 2cd2416:CHANGELOG.md`), les mesures dans [validation.md](validation.md),
les pièges dans [lecons.md](lecons.md) et les choix dans [decisions.md](decisions.md) — ce tableau ne fait
que dire quand et par quoi chaque lot est prouvé.

Les lots 0 à 6 viennent de l'**audit du dépôt du 2026-11-04**, qui avait relevé cinq chantiers : décodeur
robuste face à un en-tête hostile (C1), résultat périmé affiché par le socle des outils (C2), commande unique
de vérification (C3), mémoire du décodeur (C4), consignes et CI périmées (C5). **Les cinq sont traités et
prouvés.**

| Lot | Fait le | Sujet et preuve | Ce qu'il en reste |
|---|---|---|---|
| 0 | 2026-11-04 | Sonde du corpus et des références (`chore/corpus-probe`) : `otb-parity` 0 écart, `corpus validate` 697/712, départ mémoire 491 Mo. La ligne de base n'était **pas** « 0 écart » comme le plan le supposait | rien |
| 1 | 2026-11-04 | **C3** — `otb-dev verify [--quick] [--strict]` (`feat/dev-verify`, L-087), marqueur `[OTB-SAUT]`, crochet sans `jq`. 4 contre-preuves ; 74 exécutables de test avec et sans la liste des `-p` (elle était inutile) | rien |
| 2 | 2026-11-04 | **C5** — consignes et CI remises à jour (PR #103) : `AGENTS.md` de 12,6 à 8,4 Ko | rien |
| 3 | 2026-11-04 | **C2** — résultat périmé du socle des outils (PR #104, L-088). Contre-preuve : l'ancien `update` rougit les deux tests | W-036 (8) : garde-fou propre au Simulateur gardé |
| 4 | 2026-11-04 | **C1** — décodeur robuste (`fix/decoder-hostile-header`, D-044, L-080) : bornes 512 champs / intervalles 1 à 65 535, `try_reserve`. 10 cas dont **6 rouges avant** ; 20 000 champs : 3,7 s puis abandon → refus instantané | limite écrite : sous Linux une réservation énorme peut réussir (surengagement) |
| 5 | 2026-11-04 | **C4** — mémoire du décodeur (`perf/decoder-memory`, L-081) : `CompactTable` en `i32` dès le décodeur, **481 → 274 Mo (÷ 1,75)** et 0,63 → 0,54 s ; `FlightLog` identiques sur 528 fichiers | un décodage par blocs vers `f32` irait plus loin (non fait, non chiffré) ; W-014 et W-016 clos |
| 6 | 2026-11-06 | Bilan des cinq chantiers (branche « docs/plan-suite ») : L-087 et L-088 écrites, W-014 et W-016 clos. Aucun code livré | rien |
| 7 | 2026-11-08 | **Première publication Rust, 0.9.0.** 7a `otb-dev licenses --check` (`feat/licenses-check`) ; 7b CHANGELOG et captures (`chore/release-0.9.0`) ; 7c **quatre tours** d'essai sur de vraies machines et **sept défauts** corrigés (PR #118 à #126), **aucun vu par les tests** ; 7d–7e publication, page « dernière version », brouillon d'essai supprimé | W-043 (licences non relues, échéance repoussée à la 1.0) et W-044 (mise à jour depuis la 0.8.0 jamais essayée) → passés au lot 13, puis « Hors plan » quand il a été supprimé (ligne 13) |
| 8 | 2026-11-05 | Banc de temps de cadre (`chore/viewer-frame-bench`) : `examples/banc_cadre.rs`. Étalon retrouvé sur `FINAL.bbl` (7,4 à 8,3 ms) ; `LOG00008.BFL` 12,6 à 13,0 ms à 620 079 échantillons, sous les 16,7 ms | les 0,34 ms de carte graphique de W-049 ne se reproduisent pas (L-084) |
| 9 | — | **Non retenu** : W-048 ne devait pas se régler en affaiblissant un test. Repris hors lot le 2026-11-05 (`test/golden-tolerance`, D-047, L-083) | W-048 clos |
| 10 | 2026-11-05 | Tracé du viewer sans parcourir toute la série (`perf/viewer-pyramid`, D-048, L-085, L-086) : index multi-résolution min/max dans `otb-dsp`, cadre **÷ 2,2 à ÷ 6,5**, tracé **identique au bit près**, pic de mémoire inchangé | W-049 clos ; **W-052 ouvert** (le tracé domine maintenant) → lot 15 |
| 11 | 2026-11-10 | **Ouverture de plusieurs logs en parallèle** (branche `perf/open-parallel`, D-051, L-093) : `otb_log::parallel`, paquets d'au plus 3 fichiers décodés en même temps dans un pool `rayon` propre à l'appel. 6 logs : **1,835 s → 0,811 s (× 2,26)**, un seul fichier non dégradé, `FlightLog` identiques (empreintes) ; 4 contre-preuves | **W-050 clos** ; le chiffre de mémoire de D-051 était trop bas (663 Mo et non ≈ 540), plafond gardé par Cédric |
| 12 | 2026-11-08 | Ménage des registres (PR #129, `chore/registers-cleanup`) : 10 travaux reportés clos ou requalifiés, une **panique** corrigée (`log_rate`, W-029 point 3), un paramètre mort retiré (W-024), D-051 écrite. `verify --strict` 0 saut (125 s) | W-053 ouvert au passage |
| 13 | — | **Supprimé le 2026-11-10** : « nettoyage d'après publication ». Cédric a indiqué que **personne n'a jamais utilisé la 0.8.0** — l'essai de mise à jour 0.8.0 → 0.9.0 et son filet dans l'installateur devenaient sans objet. Le filet (`[InstallDelete]` de `opentunebox.iss`) est retiré dans la foulée (branche `chore/drop-python-upgrade-net`), W-044 point 1 clos | W-043 (licences, échéance 1.0) et W-044 point 2 (script Windows) ne dépendaient pas de la 0.8.0 → « Hors plan », avec leur condition |
| 14 | 2026-11-10 | **Derniers seuils sans origine** (branche docs/source-last-thresholds) : **aucune valeur changée**, parce qu'aucun des deux seuils restants ne peut être calibré — le corpus n'emprunte **jamais** la détection de taille par le nom (0 drone sur 96 sans fréquence moteur) et le profil 8"+ n'a que 6 drones (W-002). Chacun porte désormais sa **condition de levée**. W-005 clos : formule de la bande lue à la page sur une reproduction ([S37]), avec ses trois limites écrites. `verify --strict` 0 saut (126 s) | W-055 ouvert (cinq sources nomment une personne : arbitrage D-050) ; W-003 reste ouvert, mais borné à deux lignes assumées |
| 16 | 2026-11-08 | **Revue intégrale** du code, des tests, de la documentation et de l'outillage (PR #129) : 4 relevés (~41 000 lignes de code, ~20 500 de tests, 30 `.md`, 4 workflows), 53 cases, **2 défauts réels** corrigés (`compare_header`, `slice_time`), **6 constats de relevé faux** corrigés après vérification, chaîne Rust figée (`rust-toolchain.toml`), clippy sous Windows, D-052 écrite, D-050 précisée, récidive de L-079 bridée | **W-054 ouvert** : seuls 2 des 10 comparateurs d'`otb-parity` ont reçu un test |
| 17 | 2026-11-08 | **Rassemblement des plans** (branche « docs/rassembler-les-plans ») : les quatre documents de plan deviennent ce seul `docs/plan.md`, L-092 écrite | rien |
| 18 | 2026-11-11 | **Audit de sécurité** (branche `chore/security-hardening`, D-054) : réglages GitHub relus et complétés par Cédric (versions immuables, CodeQL, actions autorisées, approbation des contributions extérieures) ; `claude.yml` refuse les forks ; `--locked` en CI ; `unsafe_code = "deny"` ; crochet `guard-git.sh` ; essai de mutations (100 000 cas pour le décodeur, 20 000 de bout en bout, 0 plantage). Contre-épreuves notées dans D-054 | signature de l'installateur Windows : ouverte dans D-054 (l'épinglage SHA est exigé depuis le 2026-11-11) |
| 19 | 2026-11-11 | **Relecture des workflows en CI** (branche `chore/lint-workflows`, D-055) : job `actionlint` 1.7.12 dans `tests.yml`, vérifié par empreinte ; rien à installer sur le poste. Avant l'envoi : 19 scripts bash passés à `shellcheck`, rien à signaler | rien : `actionlint` est un contrôle exigé de `protection-main` depuis le 2026-11-11 |
| 20 | 2026-11-11 | **Notes de version à la manière de Betaflight** (PR #147, D-056) : titre de PR en phrase et une étiquette `RN: …` (8 étiquettes créées, dont `dependencies`, absente jusque-là : L-096) ; `.github/release.yml` ; contrôle `etiquette-pr.yml` (rouge sans étiquette, vert avec, vu sur #147) ; `otb-dev release notes` traduit la liste de GitHub ; `CHANGELOG.md` supprimé. 86 PR étiquetées et retitrées, Dependabot excepté (#60 à #147). Page de la 0.9.0 refaite (50 Ko → 6 Ko) depuis une liste fabriquée en local, GitHub ne lisant `release.yml` que sur l'étiquette (L-097) | `etiquette` exigé par `protection-main` depuis le 2026-11-12 ; l'appel de `generate-notes` par le jeton de la CI sera éprouvé à la première étiquette `essai-v…` ou `v…` |
| 21 | 2026-11-12 | **Licences livrées et sources sans nom** (branche « docs/licenses-and-sources-review », D-050 précisée) : textes des 22 composants sans fichier de licence copiés de leur dépôt d'origine (`rust/otb-dev/upstream-licenses/`, table `UPSTREAM`), dont la licence BSD du code d'AccessKit issu de Chromium, **absente du fichier livré avec la 0.9.0** ; polices relues (Noto Emoji, seule police OFL, non compilée) ; partie 3 du fichier vide et refusée par un test, contre-preuve vue en échec. Noms de personnes retirés de 13 sources (5 relevées, 8 trouvées en relisant), pseudonymes compris | W-043 et W-055 clos ; reste, avant la 0.10.0 : rien de ce lot |

La migration Python → Rust elle-même, terminée le 2026-11-04, garde son propre document :
[migration-rust.md](migration-rust.md) (phases, preuves d'équivalence, verdict du prototype d'interface).
| 28 | 2026-11-18 | **Bibliothèque de données** (branche `feat/data-library`) : crate `otb-store`, six tables en **fichiers JSON** dans `bibliotheque/` — drones, logs connus, fragments, `diff all` datés, mesures avec leur version de calcul, liste d'analyse. Un seul module d'accès ; un fichier illisible ou plus récent est mis de côté, jamais écrasé. 15 tests, **5 contre-preuves vues en échec** (mise de côté, version plus récente, péremption des mesures, doublon de log, réglages identiques). Volumes mesurés dans [validation.md](validation.md) | la table des mesures pèse 8 Mo et met 959 ms à se lire au volume plein : **à charger à la demande au lot 31** (noté dans ce lot) |
| 29a | 2026-11-18 | **Fragments, le cœur** (repris du fork par la PR #165, `rust/otb-store/src/link.rs`) : index des fragments (log, session, bornes en échantillons, nom, rôle du Guide, date), période d'un fragment dans un log ouvert et message clair si le fichier a changé, liste d'analyse à indices de couleur stables. Preuve : un fragment de `jeno5_5s.bbl` relu après redémarrage rend les mêmes échantillons ; 26 tests d'`otb-store`, 8 contre-preuves vues en échec | l'écran (29b) |
| 30a | 2026-11-19 | **Rail de navigation** (PR #165, D-060, D-015 amendée) : sept entrées dans une colonne à gauche, la barre du haut ne garde que ce qui agit sur les logs et la fenêtre. Preuve : 2 tests avec contre-preuve vue en échec, captures des deux thèmes ; une phase du Guide rognée corrigée au passage | espace « Bruit et filtres » (30b) |
| 30b | 2026-11-19 | **Espace « Bruit et filtres »** (PR #165) : une entrée de rail pour le Spectre, Fréq. × gaz, Fréq. × temps et le « Et si », onglets en haut de page (`space_tabs`, notion `SPACE`). Preuve : 1 test avec contre-preuve vue en échec, 280 tests de l'interface verts, captures des deux thèmes. Le calcul des filtres n'a pas bougé : le lot 23 reste indépendant | choix « Gyro \| Terme D » (30e) |
| 30c | 2026-11-19 | **Tableau de bord** (PR #165, `rust/otb-gui/src/dashboard.rs`) : une carte par drone de la bibliothèque, panneau de ses logs avec un « + » ; l'application range chaque fichier ouvert dans la bibliothèque (en mémoire hors du vrai lancement). Preuve : 2 tests, 2 contre-preuves vues en échec, captures des deux thèmes ; un « ✓ » absent des polices, qui faisait paniquer l'application, attrapé par la capture (L-068) | dessin de la maquette (30c bis) |
| 30c bis | 2026-11-19 | **Tableau de bord au dessin de la maquette** (PR #165, D-061) : carte de drone et entrée de rail aux mesures du prototype, deux couleurs ajoutées au thème, trois icônes ; trois écarts assumés et écrits (thème sombre dérivé, un fragment par session, avancement compté à la main) | « + » vers la liste d'analyse (29c) ; avancement du parcours (30g) |
| 22a | 2026-11-22 | **Modèle de filtres comparé au firmware, étage par étage** (PR #159, branche `feat/filter-stage-check`, D-057) : `rust/otb-analysis/src/filter_stages.rs` et `otb-dev corpus stages`, gain, phase, retard, cohérence et résidu par étage et par bande, décalage d'horloge de L-100 retiré. Logs chirp de l'Air65 : 0,2 à 0,8 dB sous 250 Hz. Critère d'arrêt atteint : 7 sessions sur 18 à pleine cadence au-delà de 1 dB | 22a bis |
| 22a bis | 2026-11-22 | **Alertes de 22a expliquées** (PR #160, branche `fix/filter-stage-alerts`, L-101) : 24 alertes → 2 — contrôleurs surchargés dont la boucle tourne moins vite que l'en-tête, récupération de vrille qui annule le D, TPA du D ; les 2 restantes (−1,3 dB sur le D) acceptées par Cédric | corrections reportées dans le simulateur affiché (W-064, 22e) |
| 22c | 2026-11-22 | **Suivi du notch dynamique rejoué** (PR #161, branche `feat/dyn-notch-tracking`, L-102) : `rust/otb-analysis/src/dyn_notch.rs` d'après le firmware, chaque axe suivi à part, `otb-dev corpus dyntrack`. 1 099 notchs du corpus : 0,3 à 0,4 Hz d'écart médian aux centres enregistrés à chaque boucle PID ; 14,4 Hz depuis le gyro modélisé, contre 78 Hz pour les centres initiaux | W-065 (variantes jamais confrontées à un log) |
| 22b | 2026-11-22 | **Vols de calibration** (PR #162, branche `fix/filter-stage-measurement`, L-103, L-104) : 12 vols `GYRO_SAMPLE` de l'Air65 séparent LPF2, RPM et LPF1 ; deux défauts du contrôle corrigés (arrondi de l'entrée ; LPF2, RPM et LPF1 calculés à la cadence du log au lieu de la leur). 0 alerte sur le corpus, tous les étages sous 0,5 dB jusqu'à 500 Hz | W-067 (drones qui vibrent fort, logs à cadence réduite) ; W-080 (second vol jamais fait) |
| 22d–22e | 2026-11-22 | **Retard gyro sans décalage d'horloge et page Filtres rejouée** (PR #163, branche `feat/filter-sim-before-after`, D-058 ; W-057, W-059 et W-064 traités) : `compute_delays` retire ⌊denom/2⌋ / fréquence gyro (16 sessions sur 675 changent de constat, repère du whoop 1,5 → 1,4 ms) ; `rust/otb-analysis/src/filter_replay.rs` rejoue la chaîne du firmware, gyro et D avant et après filtrage. 0,1 à 0,2 dB entre rejeu et log à chaque boucle PID | 22f |
| 22f | 2026-11-22 | **Prédiction vérifiée en vol** (PR #164, branche `feat/filter-sim-flight-check`, D-059, W-068 traité) : `otb-dev corpus predict`, un notch dynamique de plus sur l'Air65 ; entre 100 et 250 Hz, le rejeu se trompe de 0,3 dB en médiane contre 2,0 dB pour le rapport des \|H\|² ; l'assistant passe au rejeu. **Lot 22 terminé** | W-060 (LPF, RPM et autres drones jamais vérifiés en vol) ; W-073 (conseils d'enregistrement du Guide) |
| 33 | 2026-11-22 | **Terme D rejoué dans l'ordre du firmware** (branche `fix/dterm-filter-order`, W-072 point 1) : `simulate_dterm_at` (page Filtres, assistant, contrôle par étage) filtre puis dérive ; `simulate_dterm` garde l'ancien ordre pour les références Python (`otb-parity` 0 écart). Mesuré sur 676 sessions : quelques centièmes de dB au plus sur les spectres exacts, contrôle par étage du D inchangé, **aucun constat de l'assistant ne bascule** ([validation.md](validation.md#ordre-du-terme-d-2026-11-22-lot-33)). Contre-preuve : l'ancien ordre s'écarte de 1,3 % d'un D « firmware » écrit pas à pas (test rouge), L-108 | W-072 point 2 et l'ordre de `simulate_dterm` : au réenregistrement des références (W-028) ou au lot 23 |
| 32a | 2026-11-22 | **Récupération par USB, protocole** (branche `feat/usb-protocol`, D-063) : crate `otb-usb` (trames MSP v1 jumbo et v2, `serial2`, copie par blocs, contrôles avant effacement, `diff all`, carte simulée) et `otb-dev usb` pour l'essai à la main. Preuve : 38 tests, dont un par refus d'effacer (drone armé, vol enregistré pendant ou après la copie, autre carte, identifiant nul sur une autre connexion, copie incomplète, log sans vol, fichier existant) ; chaque protection retirée à la main fait rougir un test (L-071) ; le vrai port essayé par un pseudo-terminal | 32b (écran, stockage de masse, bibliothèque) ; essais restants sur une vraie carte (drone armé, configurateur ouvert, Windows). **Essayé sur une G474** : 8 Mo copiés à 3,25 Mo par minute, vérifiés, effacés en 42,5 s ([spec-usb.md](spec-usb.md#résultats)) |
| 29b | 2026-11-23 | **Écran d'extraction d'un fragment** (branche `feat/fragment-extraction`, D-065, W-069 clos) : le visualiseur devient l'écran hors rail du prototype (barre « Modifier le fragment », bornes à onglets et poignées, barre de navigation, panneau Fragment / Traces / Lecteur) ; les bornes du fragment sont la période d'analyse, enregistrées et retrouvées à la réouverture ; rail aux sept entrées du prototype. Preuve : 13 tests nouveaux ou récrits, dont 9 vus en échec protection retirée, captures des deux thèmes, 2,2 ms par cadre sur `FINAL.bbl` | liste d'analyse à l'écran, second fragment d'une session ; fiche drone (lot 30) |
| 29c | 2026-11-23 | **Liste d'analyse à l'écran** (branche `feat/analysis-list`, D-066, D-049 remplacée) : barre de fragments du prototype (désactiver, maître, glisser, retirer), les pages la lisent au lieu de leurs listes « Fichiers », « + » du tableau de bord qui n'ouvre que sa session, liste retrouvée au lancement. Preuve : 10 tests, 9 contre-preuves vues en échec, captures des deux thèmes | second fragment d'une session ; fiche drone (lot 30) |
| 29d | 2026-11-23 | **Second fragment d'une session** (branche `feat/second-fragment`, D-067, D-061 amendée) : « Enregistrer comme nouveau fragment » dans l'écran d'extraction (pastille posée sur les mêmes échantillons, juste après l'origine), suppression d'un fragment ajouté en deux temps, réouverture par « + », crayon ou liste du lancement. Preuve : 5 tests, 8 contre-preuves vues en échec, captures des deux thèmes. **Lot 29 terminé** | W-074 (rôle vu par le Guide) ; fiche drone (lot 30) |
| 30d | 2026-11-23 | **Fiche d'un drone** (branche `feat/drone-sheet`, D-068, D-036, D-039 et D-061 amendées) : écran hors rail au dessin du prototype (Identité, Machine, Configurations Betaflight avec « Voir » et « Comparer », suppression en deux temps) ; la fiche vit dans la bibliothèque et le Guide en lit une copie ; ancien `drones.json` repris puis renommé ; un log rejoint le drone qui porte son nom ; bouton rouge plein lisible en thème sombre. Preuve : 17 tests nouveaux (5 de la bibliothèque, 8 de la fiche, 4 du Guide), les tests du Guide récrits sur la bibliothèque, 6 contre-preuves vues en échec, captures des deux thèmes, 0,2 ms par cadre | « Fiche cible » (lot 31) ; identifiant et carte au branchement (lot 32b) ; W-075 |
| 30e | 2026-11-23 | **Choix « Gyro \| Terme D » de l'espace** (branche `feat/signal-choice`, D-069) : un seul choix du signal et des axes partagés par le Spectre, Fréq. × gaz, Fréq. × temps et le « Et si », en tête de leur panneau ; un changement d'une vue vaut pour les autres, l'étage choisi (avant ou après filtres) est gardé ; cases détaillées gardées dessous ; × gaz s'ouvre sur « Gyro pf \| Gyro » par fragment ; doublons « Gyro » et « D » du « Et si » retirés. Preuve : 5 tests, 5 contre-preuves vues en échec, captures des deux thèmes ; temps d'un cadre mesuré pour chaque page par l'exemple `capture` | assistant en cartes (30f) ; avancement du parcours (30g, W-074) |
| 30f | 2026-11-23 | **Assistant en cartes** (branche `feat/advisor-cards`, D-070) : une carte par constat triée par gravité, « info » et vérifications OK repliées, pastilles par gravité, réponse indicielle de chaque axe en petit à droite ; jauge « mesure contre seuil » sur chaque constat à un seul seuil (`Gauge`, mêmes variables que le texte), hors des références Python (`otb-parity` 0 échec sur 1 732 appels). Preuve : 12 tests nouveaux ou récrits (7 de la page, 5 de la jauge), 9 contre-preuves vues en échec, captures des deux thèmes, 0,4 ms par cadre | W-076 (comparaison, votes, historique, lignes CLI) ; avancement du parcours (30g, W-074) |
| 30g | 2026-11-23 | **Avancement du parcours** (branche `feat/guide-progress`, D-071, D-039, D-061 et D-066 amendées, W-074 traité) : le Guide lit le rôle de chaque log sur son fragment et l'y écrit (anciens rôles par fichier repris pour les fragments qui n'en ont pas, « Ouvrir les logs des filtres… » donne le sien) ; seuls les fragments actifs sont vérifiés ; la carte du tableau de bord compte le dernier relevé du Guide (`DroneSheet::verified`), barre au nom accessible. Preuve : 4 tests nouveaux et 2 récrits, 8 contre-preuves vues en échec, captures des deux thèmes. **Lot 30 terminé** | « Fiche cible » (lot 31) ; écran de la récupération par USB (lot 32b) |
| 31a | 2026-11-23 | **Mesures suivies, le calcul** (branche `feat/metrics-history`, D-072, D-064 prouvée) : `otb_advisor::tracked` lit 22 lignes par fragment dans le rapport de l'assistant ; la table des mesures se lit sur un autre fil (ouverture 5 ms, lecture 308 ms au volume plein en `--release`), ne se vide jamais avant sa lecture et ne se réécrit que si elle change ; la fiche d'un drone fait mesurer ses fragments en arrière-plan. Preuve : 16 tests nouveaux (7 de la bibliothèque, 2 du calcul, 5 de l'interface, 2 récrits), 9 contre-preuves vues en échec, recalcul au bit près sur 11 sessions réelles, captures des deux thèmes. La CI Windows a trouvé une relecture JSON fausse d'un bit, corrigée par `float_roundtrip` (L-116 ; `otb-parity` 0 échec) | fiche cible (31b) ; graphiques du suivi (31c) ; W-076 (31d) |
| 31b | 2026-11-23 | **Fiche cible du drone** (branche `feat/drone-target`, D-072 précisé) : dix cases au dessin du prototype dans la colonne de droite de la fiche, valeurs proposées lues dans les seuils de l'assistant pour la taille (`otb_advisor::target`), vides sans seuil ; rien d'enregistré tant que le pilote n'a rien changé (les cases suivent la taille), une case changée fige les dix, « Valeurs proposées » les remet à suivre ; une case illisible refuse l'enregistrement. Preuve : 9 tests nouveaux (5 de l'assistant, 1 de la bibliothèque, 3 de la fiche), 4 contre-preuves vues en échec, captures des deux thèmes. Récidive de L-114 | graphiques du suivi (31c) ; W-076 (31d) |
| 31c | 2026-11-23 | **Graphiques du suivi** (branche `feat/metrics-charts`, D-072 précisé) : neuf graphiques sous « Suivi des mesures » de la fiche du drone, un par grandeur de la fiche cible, vols numérotés par date, une couleur par axe, zone visée teintée lue dans les cases du formulaire (vide = « sans cible ») ; pas de point sous 20 fenêtres, point creux sous 200, comme l'assistant ; repères à virgule décimale. Preuve : 8 tests nouveaux (2 de l'assistant, 5 du suivi, 1 des repères), 10 contre-preuves vues en échec, captures des deux thèmes sur trois sessions réelles du JeNo 5 ; défaut hors sujet noté : W-077 (exemple `capture` à quatre logs) | W-076 (31d) |
| 31d | 2026-11-23 | **L'assistant d'un vol à l'autre** (branche `feat/advice-history`, D-073, W-076 traité) : en tête le badge du drone et le vol comparé (le précédent du drone par défaut, ouvert ou non, ou choisi), un repère creux du même constat sur chaque jauge, « NOUVEAU » et « Plus signalé depuis » ; un pouce levé (appliqué) ou baissé (écarté, avec sa raison) par conseil, inscrit au clic dans la septième table de la bibliothèque (`conseils.json`) ; bandeau « Fragment déjà traité » et historique des choix. La page lit la bibliothèque et rend ses changements à la fenêtre (`Tool::read_library`, `take_library_edits`). Preuve : 13 tests nouveaux (4 de la bibliothèque, 4 purs, 5 de la page), 5 contre-preuves vues en échec, captures des deux thèmes sur `FINAL.bbl` et `LAST.bbl` du JeNo 5 | lignes CLI (W-078) ; lot 32b |
| 32b | 2026-11-23 | **Récupération par USB, l'écran** (branche `feat/usb-retrieve`, D-074, W-075 traité) : « Récupérer par USB » au tableau de bord ouvre un écran hors rail ; la connexion vit dans un fil de travail qui garde la copie vérifiée (seule la demande d'effacement en fait un permis) ; drone reconnu par l'identifiant gravé de sa carte (`Drone::uid`), demandé à la première connexion ; copie rangée sous le drone, bibliothèque écrite, puis effacement en deux clics ; `diff all` par un bouton ; « Ouvrir les vols » ; un log déjà rangé garde son drone à la réouverture ; case « Supprimer aussi les fichiers copiés » à la suppression d'un drone. Preuve : 18 tests nouveaux (15 de l'écran et du fil sur la carte simulée, 3 de la bibliothèque), 5 contre-preuves vues en échec, captures des deux thèmes ; parcours complet essayé à l'écran sur la carte G474 sous Linux | 32c (stockage de masse, cartes SD) ; drone armé, port occupé et Windows à essayer |
| 32c-1 | 2026-11-23 | **Mode stockage de masse et cartes SD, le protocole** (branche `feat/usb-mass-storage`, D-075, W-079 noté) : `otb-usb` fait redémarrer la carte en disque (seul mode 2, refus armé), repère le disque monté (Linux, Windows), copie `BTFL_ALL.BBL` par les mêmes contrôles qu'en MSP (effacement après rebranchement, identifiant connu seulement), copie et vérifie un par un les logs d'une carte SD (drone ou lecteur), et ne les supprime que par un seul chemin, sous permis, chacun relu et comparé à sa copie ; `otb-dev usb msc` et `usb sd` pour l'essai à la main. Preuve : 14 tests nouveaux et 2 contrats, 4 contre-preuves vues en échec | 32c-2 (l'écran) ; essai sur une vraie carte |
| 32c-2 | 2026-11-23 | **Mode stockage de masse et cartes SD, l'écran** (branche `feat/usb-mass-storage-screen`, D-075 complétée, D-074 amendée) : « Copier en mode disque » à côté de « Copier les vols », attente du disque et « Choisir le dossier… », copie de la mémoire gardée jusqu'au rebranchement puis reconnexion seule et effacement si c'est la même carte, logs d'une carte SD copiés un par un (refusés gardés sur la carte) et supprimés en deux clics, section « Carte SD dans un lecteur » (drone choisi à la main, nom lu dans le log) ; `otb-usb` refuse de prendre le fichier de la carte pour sa propre copie. Preuve : 9 tests nouveaux (8 de l'écran, 1 d'`otb-usb`), 4 contre-preuves vues en échec, captures des deux thèmes sur la carte simulée ; parcours du mode disque essayé à l'écran sur la carte G474 sous Linux | carte SD réelle, Windows |
| 23 | 2026-11-23 | **La chaîne Betaflight décrite en données** (branche `refactor/chain-as-data`, D-062 amendée : condition des vrais logs levée pour ce lot seul) : `rust/otb-analysis/chains/betaflight.json`, intégré à la compilation et lu par `chain.rs`, donne les étapes des chaînes gyro et D (brique, cadence, réglage lu), les prises du log et la position de la dérivée du D ; la réponse en fréquence, les étages `GyroStage` (intervalle entre deux prises), le rejeu, le suivi du notch dynamique, le contrôle par étage et le terme D la lisent au lieu de porter l'ordre en dur ; `dterm_chain` rend un `Result`. Preuve d'iso-résultat ([validation.md](validation.md#chaîne-décrite-en-données-2026-11-23-lot-23)) : `otb-parity` 0 échec au bit près sur 1 732 appels, `corpus replay`, `stages`, `dyntrack` identiques, 0 constat changé sur 676 sessions, 42 captures identiques octet pour octet ; 6 tests nouveaux ; contre-preuves : LPF1 déplacé (6 tests rouges), LPF2 à la cadence PID (14 rouges) | lots 24 et 25, toujours conditionnels |
| 24a, 24b | 2026-11-24 | **Briques à l'arithmétique du firmware et banc de référence** (branche `feat/firmware-exact-bricks`, D-062 amendée : condition levée pour 24a à 24c) : briques float32 de `rust/otb-dsp/src/firmware/betaflight/` (sinus approchés, PT1/2/3, biquad, SVF, notch RPM, décimation), **non branchées** ; banc `otb-dev firmware-bench` (télécharge et compile `filter.c` et `maths.c` hors du dépôt, ne garde que des vecteurs) ; 6 versions, 23 ou 24 cas chacune, **toutes identiques au bit près**, rejouées par la CI (`rust/otb-dsp/tests/firmware_vectors.rs`) ; contre-preuves, dont un trou du jeu de cas (L-118) | W-081 ouvert ; 24c suivant |
| 24c | 2026-11-24 | **Chaîne assemblée à l'arithmétique du firmware, écart chiffré** (branche `feat/firmware-chain-compare`, D-062 amendée, W-081 point 1 traité, W-082 et W-083 ouverts, L-119, L-120) : `rust/otb-analysis/src/firmware_chain.rs` enchaîne les briques comme la boucle du firmware (deux cadences, notchs RPM à tour de rôle, LPF dynamiques par le mixeur, D de `pid.c`), à côté de la chaîne actuelle, rien de changé à l'écran ; cas « deux cadences » du banc rendus au bit près ; critère d'arrêt non atteint (contrôle par étage, chaque boucle PID : pire bande +0,035 dB) ; gyro rejoué égal ou plus proche du vol partout, D des logs ralentis plus loin (W-083) ; corrigés dans la chaîne en service à la demande de Cédric : notch RPM au plafond gardé (W-082, jusqu'à 24 dB d'écart au vol en moins sur deux vols) et ordre des notchs RPM du firmware (L-120), aucun avis de l'assistant changé, références Python intactes | bascule 24d, 24e : décision de Cédric |
| 24c bis | 2026-11-24 | **Écart du D des logs ralentis expliqué** (branche `test/thinned-log-dterm`, W-083 point 1) : `otb-dev corpus thin` réduit les 32 sessions enregistrées à chaque boucle à une boucle sur deux ; rejouée avec le vrai gyro, la chaîne du firmware colle au D enregistré comme sur le log complet, l'excès (+0,48 dB entre 100 et 250 Hz hors Air65) vient du gyro non filtré sous-échantillonné sans anti-repliement, et le D de la chaîne actuelle, dérivé à la cadence du log, est trop bas de 0,4 à 0,7 dB et le compense par hasard ; rien de changé à l'écran ; Cédric garde le D du firmware pour tous les logs (D-076) | bascule 24d |
| 24d, 24e | 2026-11-24 | **Bascule sur la chaîne du firmware** (branche `feat/firmware-chain-switch`, D-062 amendée, L-121) : page Filtres, assistant et contrôle par étage rejouent par `firmware_chain.rs` dès la 4.3 (`Engine::in_service`), l'ancienne chaîne gardée pour les logs d'avant la 4.3 et les mesures ; références figées intactes (`otb-parity check` : 0 échec), non réenregistrées ; un avis bascule sur 676 sessions (`1. PTB/Kris/13. Hover.bbl`, seuil frôlé, log ralenti), accepté par Cédric ; calibration identique au bit près, aucun seuil changé ; assistant 1 019 → 681 s cumulées sur le corpus | écart au vol affiché (reste du lot 24), lot 25 : conditionnels |
| 24 (fin) | 2026-11-24 | **Écart de la simulation au vol affiché** (branche `feat/flight-gap-display`, demandé par Cédric) : `otb_analysis::flight_gap` mesure, par bande, le rejeu des réglages du log contre le spectre enregistré (roulis et tangage) et dit la cause connue (log ralenti, firmware d'avant la 4.3) ; la page Filtres l'affiche au-dessus des spectres ; `corpus replay` passe par la même fonction, résultats identiques sur 676 sessions. Preuve : 7 tests nouveaux, 3 contre-preuves vues en échec, captures des deux thèmes sur `FINAL.bbl` du JeNo 5 | lot 25 : conditionnel |
| 30h | 2026-11-25 | **Fiche du drone : taille libre, taille des moteurs, machine lue dans le log** (branche `feat/drone-sheet-from-log`, D-077, D-068 et D-061 amendées, demandé par Cédric) : la taille se tape (la liste propose) et la fiche dit les seuils qu'elle choisit, lus avec les bornes de la taille par le nom ; champ « Taille des moteurs » ; pôles et boucle PID lus dans l'en-tête du dernier log, la boucle PID suivie, les pôles remplis seulement vides, un écart signalé sous le champ ; le KV reste saisi par le pilote (première version qui le lisait aussi, corrigée à la demande de Cédric). Preuve : 6 tests nouveaux et 2 complétés, 3 contre-preuves vues en échec, captures des deux thèmes sur `FINAL.bbl` du JeNo 5 (0,4 ms par cadre). Les captures ont montré la section Machine plus large que sa colonne, la Fiche cible rognée à droite : un champ à unité réservait trop peu de place à « KV » (défaut d'avant, révélé par la nouvelle disposition) et la flèche de la taille gardait sa marge ; l'unité se pose maintenant d'abord, à droite (récidive de L-111) | — |
| 34 | 2026-11-25 | **Emplacement de la bibliothèque au choix** (branche `feat/library-location`, D-081, L-126) : pointeur à côté du dossier par défaut, logs copiés écrits relatifs à la bibliothèque, déplacement par copie relue puis effacement de l'ancien dossier, bibliothèque trouvée utilisée telle quelle, dossier introuvable au lancement sans rien écrire, avertissement quand un autre poste l'a ouverte ; écran hors rail « Bibliothèque » ouvert du tableau de bord. Preuve : 17 tests nouveaux (11 de la bibliothèque, 6 de l'écran), 11 contre-preuves vues en échec, captures des deux thèmes | essai à la main sous Linux et Windows |
| 35 | 2026-11-25 | **Tri des fenêtres de la réponse indicielle** (branche `feat/step-window-rejection`, D-082, D-025 amendée, L-127, demandé par Cédric après comparaison à PIDtoolbox web) : bande 0,5–1,5 dès 150 ms puis distance à la courbe médiane, second passage pour les mesures de boucle, tri de Python gardé pour les références ; réglages « Tri des fenêtres » et légende sur deux lignes à l'écran ; assistant recalibré (pic 1,15 / 1,20 et 1,30 / 1,45, réponse molle 1,00, latence du 5" 11–22 ms, écart roll / pitch 0,08 / 0,07), `CALC_VERSION` 2 ; `otb-dev corpus calibrate\|rates --tri-python` |
