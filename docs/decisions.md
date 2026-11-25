# Journal des décisions

Les choix qui engagent le projet : technologie, architecture, méthode, périmètre. **Une décision en vigueur
ne se remet pas en cause sans l'accord de Cédric** (humains comme agents IA) ; pour changer d'avis, on écrit
une nouvelle décision qui la remplace, et l'ancienne garde son texte avec la marque « REMPLACÉE ».

Trois registres, **un maître par sujet** ; les autres documents y renvoient au lieu de recopier :

| Registre | Préfixe | Répond à | Fichier |
|---|---|---|---|
| Ce journal | `D-xxx` | Qu'a-t-on décidé, pourquoi, qu'est-ce qui le prouve ? | `docs/decisions.md` |
| Leçons | `L-xxx` | Quel piège a déjà coûté du temps ? | [lecons.md](lecons.md) |
| Travaux reportés | `W-xxx` | Qu'a-t-on vu et laissé de côté, et quand le reprendre ? | [travaux-reportes.md](travaux-reportes.md) |

Les **règles** sans discussion (commandes, conventions de commit, couleurs…) restent dans
[AGENTS.md](../AGENTS.md) ; l'**avancement** dans les branches, les demandes de fusion et les
pages Releases (liste générée par GitHub, D-056) ; l'origine de chaque **seuil** de l'assistant dans
[assistant.md](assistant.md) ; les **mesures** de justesse dans [validation.md](validation.md).

## Protocole

**Quand écrire une décision** — dès qu'un choix remplit l'un de ces critères :
- il **écarte une option** qu'un agent suivant pourrait reprendre de bonne foi ;
- il a été **tranché par Cédric** (réponse à une question, retour sur un écran, consigne) ;
- il **remplace ou amende** une décision antérieure ;
- il fixe un **contrat** entre deux parties du code (format de données, unité, nom de réglage enregistré).

**Ne pas écrire** : un détail d'implémentation sans alternative réelle, un piège (→ `L-xxx`), une idée
remise à plus tard (→ `W-xxx`).

**Numérotation** : une seule série `D-001`, `D-002`… dans ce seul fichier, jamais réutilisée, même pour une
décision abandonnée. Avant de prendre un numéro, relire le dernier de l'index. `tests/test_docs.py` vérifie
que chaque numéro est unique et que chaque `D-`, `L-` ou `W-` cité dans la documentation ou le code existe.

**Ordre** : la plus récente en haut, sous un en-tête daté `## AAAA-MM-JJ — <sujet>`.

**Statut** : implicite = en vigueur. Sinon, le titre commence par l'une de ces marques :

| Marque | Sens |
|---|---|
| ⏸ EN PAUSE | décidée, rien d'écrit ; dire la condition de reprise |
| 🔁 REMPLACÉE par D-yyy | ne fait plus foi ; le texte reste |
| ✏️ AMENDÉE par D-yyy | fait foi, sauf le point que D-yyy précise |
| 🚫 ABANDONNÉE | retirée avant d'être livrée ; dire pourquoi |

La décision qui remplace ou amende le dit dans son corps : « Remplace D-xxx. » / « Amende D-xxx : … ».

**Format d'une entrée**

```markdown
### D-0XX : <la décision en une phrase, au présent>

**Contexte.** D'où vient la question : retour de Cédric (citation courte entre « »), constat mesuré, besoin.

**Décision.**
- Un point par choix, formulé comme ce qui EST désormais.
- L'option écartée quand elle est tentante : « pas X : Y casse ».

**Témoins.** Ce qui PROUVE que la décision tient : mesure chiffrée (validation.md), test, commit, capture.
Un témoin sans chiffre ni identifiant n'en est pas un.

**Ouvert.** (facultatif) Renvoi vers un W-xxx, sans développer.
```

**Renvois** : dans le code, un commentaire `# ⚠️ <ce qui casse si on l'ignore> (D-xxx)` ; dans une
demande de fusion, la liste des `D-xxx` créés ou modifiés ; le récit (qui a dit quoi, revirements) reste
ici, jamais dans le code.

## Index

| ID | Décision | Date |
|---|---|---|
| D-082 | **Tri des fenêtres de la réponse indicielle** : notre propre tri (jamais celui de PIDtoolbox, D-004) — réponse gardée **entre 0,5 et 1,5 dès 150 ms**, puis fenêtres trop loin de la **courbe médiane** écartées (médiane + 3 MAD normalisés) ; mesures de boucle sur les seules fenêtres gardées ; **tri de Python** (0,5–3 sur 200–500 ms) gardé pour les références figées (`StepOptions::python`, `rules::Method::python`) ; l'assistant est recalibré (pic 1,15 / 1,20, avec feedforward 1,30 / 1,45 ; D-025 amendée) | 2026-11-25 |
| D-081 | **Emplacement de la bibliothèque au choix** : pointeur à côté du dossier par défaut ; logs copiés écrits **relatifs** à la bibliothèque ; déplacement = copie relue, bascule, **puis** ancien dossier effacé ; bibliothèque trouvée dans le dossier choisi utilisée telle quelle ; dossier introuvable au lancement : rien d'écrit ; témoin d'ouverture qui **avertit** (sans bloquer) quand une autre session l'a écrite depuis moins de 10 min ; écran hors rail « Bibliothèque », **écart au prototype** (D-061) | 2026-11-25 |
| D-080 | **Et si : multiplicateurs de filtres gyro et D comme Betaflight Configurator** : interrupteur et curseur (0,10 à 2,00, pas de 0,05) en tête des passe-bas, fréquences recalculées à l'arithmétique du firmware et grisées ; interrupteur repris du log seulement si ses coupures sont celles du curseur ; CLI = état du curseur + coupures | 2026-11-25 |
| D-079 | **Une conversation par sujet, chacune dans son worktree** (dossier `worktrees` de `.claude`, `EnterWorktree`) ; dossier principal sur `main`, au pilotage (tableau, ordre des fusions) ; numéros réservés par `otb-dev numero` ; `check-docs.sh` vérifie le dossier de la conversation ; compétence `chantiers` | 2026-11-25 |
| D-078 | **« Bruit et filtres » sans le terme D préfiltre** : le D avant filtres, reconstruit et non enregistré, n'est plus proposé dans le Spectre, Fréq. × gaz et Fréq. × temps ; « Terme D » montre le D filtré seul (une colonne par fragment dans × gaz) ; préréglages 1 et 5 de PIDtoolbox retirés ; onglets de l'espace dessinés comme le prototype ; **amende D-069** | 2026-11-25 |
| D-077 | **Fiche du drone : taille libre, taille des moteurs, machine lue dans le log** : la taille se tape (la liste propose), son profil d'assistant se lit avec les bornes de la taille par le nom et s'écrit sous le champ ; champ « Taille des moteurs » ; pôles et boucle PID lus dans l'en-tête du dernier log (par date) : la fiche suit la boucle PID, ne remplit les pôles que vides, et signale un écart sans le corriger ; le KV reste saisi par le pilote, jamais tiré du log **(amende D-068 et D-061)** | 2026-11-25 |
| D-076 | **Terme D des logs ralentis : celui du firmware** : à la bascule 24d, le D rejoué se calcule à la cadence PID comme le firmware (puis une boucle sur N gardée), **y compris** quand le log ne garde qu'une boucle sur N ; la dérivée à la cadence du log est écartée, même si elle colle souvent mieux au D enregistré par compensation | 2026-11-24 |
| D-075 | **Récupération par USB, mode stockage de masse et cartes SD** : la carte redémarre en disque (seul mode envoyé : 2) et OpenTuneBox attend que le système monte le disque, avec « Choisir le dossier… » en secours ; deux voies au choix pour la mémoire interne (disque rapide, à rebrancher pour effacer ; MSP lente) ; carte SD dans le drone **ou dans un lecteur** ; logs SD supprimés en deux clics, seulement ceux copiés et vérifiés, chacun relu juste avant ; l'écran (32c-2) : lecteur sur le même écran, reconnexion seule, USB série en bouton principal ; **amende D-074** (copie par le disque de la même carte) | 2026-11-23 |
| D-074 | **Récupération par USB, l'écran** **(amendée par D-075)** : hors rail depuis le tableau de bord ; voie MSP (stockage de masse et SD : lot 32c) ; drone reconnu par l'identifiant gravé de sa carte, demandé à la première connexion ; effacement seulement après une copie vérifiée, rangée et écrite, en deux clics ; `diff all` par un bouton à part ; « Ouvrir les vols » ; W-075 : case décochée pour les fichiers copiés | 2026-11-23 |
| D-073 | **L'assistant d'un vol à l'autre** : vol comparé = vol précédent du drone (ou choisi), repère du même constat sur chaque jauge, cartes nouvelles et disparues, pas de « drones de même taille » ; pouce levé (appliqué) inscrit au clic, pouce baissé avec sa raison, dans la bibliothèque ; « Fragment déjà traité » et son historique ; pas de lignes CLI (W-078) | 2026-11-23 |
| D-072 | **Mesures suivies dans le temps** : dix grandeurs par fragment lues dans le rapport de l'assistant (jamais un second calcul), gardées avec les bornes du fragment et la version du calcul, mesurées en arrière-plan à l'ouverture de la fiche du drone ; la cible du drone part des seuils de l'assistant, une case sans seuil reste vide, l'assistant garde ses seuils ; le suivi est dans la fiche du drone | 2026-11-23 |
| D-071 | **Le Guide lit le rôle sur le fragment, et la carte du drone compte son dernier relevé** : rôle d'un log = rôle de son fragment dans la bibliothèque (plus par fichier, W-074) ; le Guide ne vérifie que les fragments actifs de la barre ; les cases que les logs vérifient sont rangées sur la fiche, et la carte du tableau de bord les compte sans analyse (amende D-039, D-061 et D-066) | 2026-11-23 |
| D-070 | **L'assistant en cartes, et la mesure contre son seuil en jauge** : une carte par constat triée par gravité, les « info » et les vérifications OK repliées ; un constat qui compare **une** mesure à **un** seul seuil porte une jauge (`Gauge`), hors des références Python ; le bouton « Appliquer » du prototype n'est pas repris (D-063 gardée) | 2026-11-23 |
| D-069 | **Un seul choix « Gyro \| Terme D » et des axes partagés pour l'espace « Bruit et filtres »** : choisis dans une vue, ils valent pour les quatre ; les cases détaillées de chaque vue restent dessous, et un autre signal choisi là n'allume aucun des deux boutons (précise D-060) **(amendée par D-078)** | 2026-11-23 |
| D-068 | **Une seule fiche par drone, dans la bibliothèque** : écran hors rail « fiche du drone » (Identité, Machine, Configurations Betaflight, suppression) ouvert par le tableau de bord ou le Guide ; le Guide n'a plus de fichier, il lit une copie ; l'ancien `drones.json` est repris une fois puis renommé `.repris` ; c'est le nom qui relie un log à son drone ; « Fiche cible » au lot 31 (amende D-036, D-039 et D-061) **(amendée par D-077)** | 2026-11-23 |
| D-067 | **Plusieurs fragments par session** : « Enregistrer comme nouveau fragment » dans l'écran d'extraction en range un autre, posé aussitôt en pastille sur les mêmes échantillons ; seul un fragment ajouté se supprime ; « Ouvrir… » ne met en analyse que le fragment de la session (amende D-061) | 2026-11-23 |
| D-066 | La **barre de fragments** (liste d'analyse à l'écran) décide de ce que montrent toutes les pages : fragments actifs dans l'ordre de la barre, maître pour les pages à un seul vol, couleur fixe par fragment ; plus de liste « Fichiers » par page ; liste retrouvée au lancement (remplace D-049) | 2026-11-23 |
| D-065 | Le visualiseur **devient l'écran d'extraction** d'un fragment, hors rail (« Courbes » quitte le rail, W-069) ; les **bornes du fragment sont la période d'analyse** : enregistrées par « Enregistrer le fragment », retrouvées à la réouverture ; un fragment non choisi suit les limites automatiques | 2026-11-23 |
| D-064 | Bibliothèque : **fichiers JSON gardés**, table des mesures comprise, lue **à la demande et hors du fil de l'interface** ; pas de SQLite | 2026-11-22 |
| D-063 | Récupération par USB : OpenTuneBox **lit, vérifie et efface**, n'écrit **jamais** de réglage sur la carte (les « Et si » restent des lignes CLI) ; port série par `serial2` ; état armé relu par OpenTuneBox avant d'effacer | 2026-11-22 |
| D-062 | Scripts firmware : chaîne décrite en **données**, briques **compilées en Rust** (pas de moteur de script) ; rien du code des firmwares dans le dépôt ; banc de référence qui télécharge et ne garde que des vecteurs ; bascule d'un coup ; **Betaflight seul** ; chantier **conditionnel** (logs INAV ou ArduCopter réels ; le lot 23, la description, fait sans attendre le 2026-11-23 ; les étapes 24a à 24c, sans rien brancher, levées le 2026-11-24, puis la bascule 24d–24e le même jour), prouvé par le banc **et** les vrais vols | 2026-11-18 |
| D-061 | **Le dessin de l'interface suit le prototype au détail près** : mesures, couleurs et contenu relevés dans la maquette qui fait foi ; thème clair identique, thème sombre dérivé ; un écart se justifie par écrit **(amendée par D-067, D-068 et D-077)** | 2026-11-19 |
| D-060 | Amende D-015 : une entrée de navigation est **un outil ou un espace** qui regroupe plusieurs vues ; les boutons deviennent un **rail à gauche** ; deux écrans hors rail sont permis. Ce qui reste : aucune fenêtre séparée, une vue cachée ne calcule pas | 2026-11-19 |
| D-059 | Assistant : bruit prévu après un conseil de filtres par le **rejeu** de la page Filtres, vérifié en vol (amende D-008) | 2026-11-22 |
| D-058 | Page Filtres : chaîne du firmware rejouée ; « modifié » = spectre du log × rejeu modifié / rejeu actuel (amende D-008, remplace D-041 pour la page) | 2026-11-22 |
| D-057 | Contrôle du modèle de filtres par étage : seuls les logs enregistrés à **1,6 kHz ou plus** sont jugés (complète D-017, sans changer ses conseils) | 2026-11-22 |
| D-056 | Notes de version **générées par GitHub** : titre de PR = phrase en français, une étiquette `RN: …` par PR (rubriques de `.github/release.yml`, contrôle `etiquette-pr.yml`) ; `CHANGELOG.md` supprimé ; remplace « titre de PR = nom de la branche » | 2026-11-11 |
| D-055 | Workflows relus par `actionlint` **dans la CI** (job de `tests.yml`, version figée vérifiée par empreinte), pas installé sur le poste | 2026-11-11 |
| D-054 | Sécurité : réglages GitHub (protections, secrets, versions immuables, CodeQL, actions autorisées), Claude refusé sur une demande de fusion venue d'un fork, `--locked` en CI, aucun `unsafe`, crochet `guard-git.sh`, essai de mutations | 2026-11-11 |
| D-053 | Cache des outils borné **par outil**, en nombre d'entrées (3 réponses indicielles, 20 simulations de filtres, 6 rapports, 2 vérifications du Guide), la moins récemment utilisée jetée la première | 2026-11-10 |
| D-052 | Données de test : seul `step_tuning.json` est stocké compressé (`.gz`) ; les autres gros fichiers de cas restent en clair | 2026-11-08 |
| D-051 | Ouverture de plusieurs logs : **3 fichiers décodés en même temps** au plus, plafond posé en nombre de fichiers et non en mémoire | 2026-11-08 |
| D-050 | Les sources ne sont plus nommées : titres, liens et citations gardés, aucun nom de personne dans le dépôt **(précisée le 2026-11-08 : les ouvrages publiés gardent leurs auteurs ; le 2026-11-12 : toute personne, pseudonymes compris)** | 2026-11-07 |
| D-049 | Réponse indicielle : ordre des courbes et couleurs donnés par l'ordre où l'utilisateur coche les fichiers ; sélection vide permise ; les autres pages gardent la couleur attribuée à l'ouverture **(remplacée par D-066)** | 2026-11-06 |
| D-048 | Tracé du viewer : index multi-résolution min/max par courbe (`otb-dsp::minmax`), sortie identique point pour point à l'ancienne décimation ; un paquet contenant un trou est tracé comme un trou ; W-049 clos | 2026-11-05 |
| D-047 | `golden.rs` compare chaque fonction portée avec la tolérance mesurée de `registry::tolerance` (bit près si aucune n'est déclarée), comme `otb-parity check` ; W-048 clos | 2026-11-05 |
| D-046 | Polices embarquées (Ubuntu Light, Hack) gardées, chacune sous sa propre licence ; licences propres aux polices admises malgré « aucun copyleft » (D-045) ; W-046 clos | 2026-11-05 |
| D-045 | Licence PolyForm Noncommercial 1.0.0 : code ouvert, tout usage commercial interdit, sans double licence ni exception vendue | 2026-11-04 |
| D-044 | Décodeur : en-tête refusé au-delà de 512 champs ou hors de 1 à 65 535 pour les intervalles ; mémoire insuffisante = erreur ; réglages à zéro signalés | 2026-11-04 |
| D-043 | Dépôt public sous licence GPL 3 ; anciennes versions publiées supprimées ; adresse noreply ; noms de tiers neutres **(amendée par D-045)** | 2026-11-04 |
| D-042 | macOS abandonné : l'application est livrée pour Windows et Linux seulement ; plus d'installateur ni d'essai macOS | 2026-11-03 |
| D-041 | Simulateur de filtres : les notchs dynamiques ajoutés au-delà de ceux du log sont placés sur les pics prévus, sinon à leur centre initial **(amendée par D-058)** | 2026-11-02 |
| D-040 | Interface egui : écarts voulus avec l'interface Qt validés (liste de fichiers à cocher, diagnostic en widgets, « -> », Bode zoomables, calcul automatique) et limites gardées telles quelles (groupes repliables, date d'export en UTC, barres d'erreur sans capuchon) | 2026-11-02 |
| D-039 | Guide egui : mémoire du guide dans la persistance d'eframe, `drones.json` lu avec tolérance et jamais écrasé s'il est illisible, police Hack de secours **(amendée par D-068 : fiches dans la bibliothèque)** | 2026-11-02 |
| D-038 | Configuration egui : lignes alignées entre logs par nom de réglage (Python : par position) | 2026-11-02 |
| D-037 | Outils egui : cache des résultats par log, partagé avec le thread de calcul, clé = réglages de calcul + période | 2026-11-02 |
| D-036 | Phase 6 : ligne de commande Rust (5c) d'abord ; fiches drone lues avec tolérance, même fichier ; réglages Qt non repris **(amendée par D-068)** | 2026-11-01 |
| D-035 | Migration complète vers Rust, plus aucun Python ; interface egui ; noyau Rust autonome | 2026-10-30 |
| D-034 | Phase 3 « par le gain mesuré » : ouverture des logs en Rust d'abord ; le reste de analysis/ et log/ repoussé **(amendée par D-035)** | 2026-10-29 |
| D-033 | Phase 2b limitée à un pilote : le noyau de `step_response` en Rust (FFT seule écartée : gain ≤ 5 %) | 2026-10-29 |
| D-032 | Phase 2 de la migration en deux temps : scipy remplacé par numpy, noyaux de filtres en Rust, scipy et numba réservés aux tests | 2026-10-29 |
| D-031 | Migration vers Rust sur une branche d'intégration, `main` reste en Python **(amendée par D-035)** | 2026-10-29 |
| D-030 | Lint par ruff limité aux règles pyflakes ; pas de formatage automatique pour l'instant **(remplacée par D-035)** | 2026-10-14 |
| D-029 | Les courbes d'un `FlightLog` sont en float32 ; le temps reste en float64 | 2026-10-13 |
| D-028 | Les constats de l'assistant s'identifient par une clé stable, jamais par leur texte | 2026-10-13 |
| D-027 | Raie fixe près de la fondamentale : test des deux trajets d'une tranche de gaz à l'autre | 2026-10-13 |
| D-026 | Retard des filtres jugé dès 30 °/s de consigne (amende D-010) | 2026-10-13 |
| D-025 | Paliers du pic de réponse selon le feedforward en service (0 en mode Angle) **(amendée par D-082 : paliers recalculés avec le tri des fenêtres)** | 2026-10-13 |
| D-024 | Notch statique conseillé seulement si le notch dynamique ne peut pas traiter la raie | 2026-10-13 |
| D-023 | « Q trop haut » seulement si une raie passe ; marge de 5 % de la fréquence moteur autour des harmoniques | 2026-10-13 |
| D-022 | Notch dynamique : réponse des notchs RPM du firmware ; `dyn_notch_min_hz` jamais relevé | 2026-10-13 |
| D-021 | Pas des curseurs conseillés, choisis par Cédric | 2026-10-12 |
| D-020 | Aucune fenêtre d'analyse ne chevauche un trou du log | 2026-10-12 |
| D-019 | Le vol chirp de Betaflight n'est pas conseillé : méthode des séries de vols | 2026-10-12 |
| D-018 | Décisions, leçons et travaux reportés ont chacun leur registre numéroté | 2026-10-12 |
| D-017 | Deux fréquences minimales du log : filtres et séries PID | 2026-10-12 |
| D-016 | Les spectres s'affichent sur l'échelle de PIDtoolbox, les calculs restent en densité physique | 2026-10-11 |
| D-015 | Un outil = un onglet de la fenêtre principale **(amendée par D-060)** | 2026-10-10 |
| D-014 | Le feedforward n'est considéré comme nul qu'en mode Angle | 2026-10-10 |
| D-013 | Linux : fenêtres de fichiers par le portail XDG | 2026-10-05 |
| D-012 | Boucle ouverte mesurée par estimateur instrumental | 2026-10-05 |
| D-011 | Seuils de l'assistant : 90e percentile du corpus par taille, origine notée | 2026-10-05 |
| D-010 | Retard des filtres jugé seulement sur un vol avec des mouvements **(amendée par D-026)** | 2026-10-05 |
| D-009 | Style Qt Fusion imposé sur tous les systèmes **(remplacée par D-035)** | 2026-10-01 |
| D-008 | Simulateur de filtres en méthode relative **(amendée par D-058, D-059)** | 2026-10-01 |
| D-007 | Retards mesurés par la phase, pas par l'intercorrélation | 2026-10-01 |
| D-006 | Aucune dépendance à compiler **(amendée par D-031)** | 2026-10-01 |
| D-005 | Exception à D-004 : icônes Lucide | 2026-10-01 |
| D-004 | Aucune copie de code de PIDtoolbox ni d'autres projets **(amendée par D-005)** | 2026-10-01 |
| D-003 | Assistant à règles, 100 % hors ligne | 2026-10-01 |
| D-002 | Décodeur Python accéléré par numba **(remplacée par D-035)** | 2026-10-01 |
| D-001 | Python + PySide6 + pyqtgraph, projet privé, interface en français **(amendée par D-035)** | 2026-10-01 |

---

## 2026-11-25 — Le tri des fenêtres de la réponse indicielle (lot 35)

### D-082 : la réponse indicielle écarte les fenêtres aberrantes par un tri qui nous est propre

**Tranché par Cédric** le 2026-11-25, après avoir comparé notre page « Réponse indicielle » à PIDtoolbox web (version 0.3)
sur `Cedric/Air65/Test BF Chirp/CHIRP-65Hz.bbl` : écrire **notre propre tri** (D-004 interdit de copier le sien, et des noms
de réglages ne disent pas la méthode), qui ira dans le même sens sans retomber exactement sur ses chiffres ; **recalibrer**
l'assistant.

**Contexte.** Les deux courbes avaient la même forme (mêmes creux et bosses aux mêmes instants : même déconvolution), mais
la nôtre était plus haute de 0,08 à 0,1 (roulis : pic 1,17 contre ≈ 1,05, palier 1,11 contre ≈ 1,03), parce qu'elle
moyennait bien plus de fenêtres (378 contre 224). Notre seul contrôle, hérité de Python (régime établi de 200 à 500 ms
strictement entre 0,5 et 3), laissait passer les nombreuses fenêtres de roulis qui montent au-dessus de 1,4. PIDtoolbox web
expose désormais un tri réglable (début du régime établi, largeur de bande, départ maximal, distance à la médiane).

**Décision.**
- **Bande** (`otb_dsp::step::Accept`) : une fenêtre est gardée si sa réponse reste **strictement entre 1 − l/2 et 1 + l/2**
  (largeur l = 1, donc 0,5 à 1,5) **de 150 ms à la fin** de la réponse ; un NaN l'écarte. Contrôle avant la correction Y,
  comme le tri de Python.
- **Distance à la médiane** (`otb_analysis::step::median_keep`) : courbe médiane point par point des fenêtres gardées, écart
  quadratique moyen de chaque fenêtre à elle sur toute la réponse ; écartée au-delà de **médiane + k × MAD normalisé**
  (k = 3, MAD × 1,4826, statistique robuste classique). Sous 8 fenêtres, pas de tri par la médiane.
- **Second passage** : si la médiane écarte des fenêtres, le noyau est relancé sur les seules fenêtres gardées, pour que les
  mesures de boucle (bande passante, marges : sommes spectrales faites au fil du calcul) ne comptent pas les écartées.
- **Tri de Python gardé** : `StepOptions::python()`, `rules::Method::python()` (rapport des réponses et tri historique),
  `step_series::flight_step_with`, `log_check::check_log_with` ; `otb-parity` et les tests sur les références figées les
  prennent. L'application (écran, assistant, guide, ligne de commande) prend le nouveau tri par défaut.
- Valeurs du tri (150 ms, largeur 1, k = 3) : celles du premier essai, gardées ; réglables à l'écran (« Tri des fenêtres »).
- **Recalibrage de l'assistant** (choix de Cédric, même méthode qu'avant, mêmes 678 sessions passées avec les deux tris) :
  paliers du pic 1,15 / 1,20 sans feedforward et 1,30 / 1,45 avec (D-025 amendée), réponse molle 1,00, latence du 5"
  11–22 ms, écart de pic roll / pitch 0,08 commun et 0,07 en 5" (le 7" prend le seuil commun, 17 drones). Les cas figés
  de Python gardent leurs paliers (`rules::StepThresholds::PYTHON`, `rules::Method::python`). Les mesures suivies sont
  refaites (`tracked::CALC_VERSION` 2, D-072). Chiffres : [assistant.md](assistant.md#recalibrage-après-le-tri-des-fenêtres-2026-11-25-lot-35-d-082).

**Options écartées.** Copier le tri de PIDtoolbox web (D-004). Garder le seul tri de Python en resserrant ses bornes (ne
voit pas une fenêtre dans la bande mais d'une forme isolée). Écarter les fenêtres par la médiane **sans** second passage
(les mesures de boucle compteraient des fenêtres que la courbe ne montre plus). Un « départ maximal » (`|r(0)|` borné) :
n'écarte rien sur les logs CHIRP, la somme cumulée part de la première valeur de la réponse impulsionnelle.

**Preuve.** `rust/otb-analysis/tests/step_rejection.rs` (vol fabriqué dont la moitié répond 1,8 fois trop fort : palier
1,39 avec le tri de Python, 0,99 avec le nouveau ; une minorité à 1,35 écartée par la médiane, mesures de boucle
recalculées), chacun vu échouer en cassant sa protection (bande neutralisée, second passage supprimé, sommes du premier
passage gardées) ; `rust/otb-dsp/src/step.rs:kept_ranks_cross_batches`. Sur les trois logs CHIRP d'Air65 :
[validation.md](validation.md#tri-des-fenêtres-de-la-réponse-indicielle-2026-11-25-lot-35).

---

## 2026-11-25 — L'emplacement de la bibliothèque au choix (lot 34)

### D-081 : la bibliothèque se déplace sans rien perdre, et un dossier introuvable n'est jamais remplacé en silence

**Tranché par Cédric** le 2026-11-25, au début du lot 34 (demandé le 2026-11-24 pendant l'essai de `essai-v0.10.0`),
sur quatre questions : chemins des logs copiés, dossier ouvert sur deux PC, place du réglage, sort de l'ancien dossier.
Il a retenu à chaque fois l'option recommandée.

**Contexte.** Le dossier de la bibliothèque était fixe (`<données>/OpenTuneBox/OpenTuneBox/bibliotheque/`), et
`logs.json` gardait le chemin **complet** de chaque log copié par USB : déplacer le dossier à la main rendait ces logs
introuvables. Pour un disque plus grand, une sauvegarde ou la même bibliothèque sur deux PC, il faut pouvoir la déplacer.

**Décision.**
- **Pointeur** (`otb_store::place`) : `emplacement-bibliotheque.json`, dans le dossier **parent** du dossier par défaut
  (il survit au déplacement), dit où est la bibliothèque. Absent : le dossier par défaut, rien ne change pour qui ne
  déplace rien. Revenir au dossier par défaut retire le pointeur.
- **Chemins relatifs** : un log **copié** s'écrit `logs/<drone>/<fichier>` (toujours avec `/`), relatif à la bibliothèque ;
  en mémoire il reste complet, le reste du programme ne change pas. À la lecture, un ancien chemin complet ailleurs (autre
  poste, autre système, dossier copié à la main) est ramené à ses trois derniers éléments dans ce dossier
  (`otb_store::copied_path_in`). Un log ouvert sur place garde son chemin complet.
- **Déplacement sûr** : la bibliothèque est écrite puis ses écritures **suspendues** ; tout est copié dans le nouveau
  dossier, marqué `deplacement-en-cours` tant que ce n'est pas fini (une copie interrompue n'est jamais prise pour une
  bibliothèque) ; chaque fichier est **relu et comparé** à l'original ; puis la bibliothèque bascule (écrite dans le
  nouveau dossier, marque retirée, pointeur écrit) et **seulement ensuite** l'ancien dossier est effacé. Une étape qui
  échoue avant la bascule laisse l'ancienne bibliothèque seule en service et retire la copie partielle. La table des
  mesures est lue avant la copie (D-064 : une lecture en retard sur l'effacement rendrait une table vide). Refusé pendant
  une récupération USB, dont les copies s'écrivent dans la bibliothèque.
- **Dossier choisi** : vide, il reçoit la bibliothèque ; sinon son sous-dossier `OpenTuneBox`. S'il contient déjà une
  bibliothèque (lui, ou son `OpenTuneBox`, ou son `bibliotheque`), elle est proposée **telle quelle** : jamais fusionnée ni
  écrasée, l'actuelle reste intacte à sa place, les logs ouverts sont fermés.
- **Introuvable au lancement** (disque débranché, pointeur illisible, dossier sans bibliothèque) : la bibliothèque reste en
  mémoire et **n'écrit rien** ; l'écran « Bibliothèque » s'ouvre avec « Réessayer », « Choisir un autre dossier… » (une
  bibliothèque existante seulement) et « Revenir à l'emplacement par défaut ». L'ancien fichier des fiches du Guide n'est
  pas repris dans ce cas (la reprise le renomme, D-068).
- **Deux PC sur un dossier synchronisé** : un témoin `ouverte-par.json` (poste, session, instant) s'écrit à l'ouverture et
  à chaque enregistrement, et se retire à la fermeture. À l'ouverture, une **autre** session qui l'a écrit il y a moins de
  10 minutes fait ouvrir l'écran « Bibliothèque » avec un avertissement ; **rien n'est bloqué**, un plantage ne laisse
  qu'un avertissement de 10 minutes au plus. La ligne de commande n'écrit pas de témoin.
- **Écran hors rail « Bibliothèque »** (`rust/otb-gui/src/library_page.rs`), ouvert par un bouton du tableau de bord à côté de
  « Récupérer par USB », le rail gardant « Tableau de bord » choisi ; pendant un déplacement, la fenêtre y reste. **Écart
  au prototype** (D-061), qui n'a pas d'écran de réglages : mise en page de l'écran USB (sections encadrées, une colonne),
  icône Lucide `hard-drive`.

**Options écartées.** Réécrire les chemins complets au déplacement (casse au premier dossier copié à la main ou à un
changement de lettre de disque) ; un verrou bloquant (un plantage laisse un verrou à lever à la main) ; ne rien faire pour
les deux PC ; le réglage en bas du tableau de bord ou derrière une icône de la barre du haut ; garder l'ancien dossier
(les logs copiés en double) ou demander à chaque fois ; fusionner deux bibliothèques ; recréer une bibliothèque vide à la
place d'un dossier introuvable.

**Témoins.** `rust/otb-store/tests/emplacement.rs` (11 tests, 8 contre-preuves vues en échec : dossier absent, chemins
complets, `relocate`, relecture, témoin ancien, écritures suspendues, copie partielle) ; `rust/otb-gui/tests/bibliotheque_emplacement.rs`
(6 tests, 3 contre-preuves : bascule, refus pendant l'USB, bibliothèque trouvée) ; captures des deux thèmes
(`--example capture` : `bibliotheque-*.png`). **À essayer à la main** : un vrai déplacement sous Linux et sous Windows
(disque externe, puis débranché au lancement).

---

## 2026-11-25 — Les multiplicateurs de filtres dans l'Et si

### D-080 : l'Et si des filtres a les multiplicateurs gyro et D de Betaflight Configurator

**Tranché par Cédric** le 2026-11-25 : « dans et si je voudrais que l'onglet Gyro et terme D soit comme dans Betaflight
Configurator. C'est-à-dire qu'il modifie effectivement les fréquences de coupure avec les sliders. »

**Contexte.** Les pages Gyro et Terme D de l'Et si n'avaient que des cases de fréquences. Les pilotes règlent pourtant
leurs passe-bas par les deux multiplicateurs (`simplified_gyro_filter_multiplier`, `simplified_dterm_filter_multiplier`),
dont l'arithmétique était déjà réimplémentée (`rust/otb-analysis/src/sliders.rs:filters_from_sliders`, page Curseurs PID).

**Décision.**
- En tête des passe-bas de chaque page : un interrupteur « Multiplicateur du filtre gyro / D » et son curseur, de 0,10 à
  2,00 par pas de 0,05, « plus de filtrage » à gauche (`rust/otb-gui/src/tools/filter_sim.rs:mult_slider`). Proposés
  seulement si l'en-tête du log contient les deux multiplicateurs (Betaflight 4.3 et après).
- Curseur mis : les coupures des passe-bas actifs suivent le firmware (`config/simplified_tuning.c` : seulement les
  réglages non nuls ; LPF1 selon son mode, LPF2 s'il est actif) et sont grisées ; type, mode et expo restent libres, et
  un LPF2 désactivé reste saisissable pour l'activer (`rust/otb-gui/src/tools/filter_sim_calc.rs:Editor::follow_sliders`).
- À l'ouverture d'un log, l'interrupteur est mis seulement si le log le dit **et** que ses coupures sont celles que donne
  le curseur, comme le configurateur (`validateTuningSliders`) ; la position du log reste connue dans tous les cas.
- Commandes CLI (`rust/otb-gui/src/tools/filter_sim_calc.rs:filter_cli_lines_with`) : un curseur mis écrit `ON` et sa position quand
  ils diffèrent du log, **plus** les coupures, puisque le CLI ne les recalcule pas sur un `set` ; un type changé sous le
  curseur ne le coupe plus. Curseur coupé : règle d'avant (un passe-bas changé coupe le curseur du log).
- Avertissement aux positions où le configurateur avertit (gyro ≤ 0,45 ou ≥ 1,55 ; D ≤ 0,75 ou ≥ 1,25), sans bornes
  « hors mode expert » : l'Et si ne règle rien sur le drone.

**Écartés.** Remplacer les cases par des curseurs (on perd la saisie fine que permet Betaflight hors curseur) ; les bornes
réduites du configurateur hors mode expert (0,50 à 1,50 et 0,80 à 1,20) : utiles pour qui écrit sur la carte, pas pour
une simulation ; écrire seulement l'état du curseur dans le CLI (les coupures resteraient celles d'avant jusqu'à
`simplified_tuning apply`).

**Témoins.** `rust/otb-gui/tests/filter_tools.rs` : `les_curseurs_de_filtres_recalculent_les_frequences_comme_le_firmware`,
`l_interrupteur_suit_le_log_seulement_si_ses_frequences_sont_celles_du_curseur`,
`le_cli_ecrit_le_curseur_mis_et_les_frequences_qu_il_donne`, `le_curseur_de_filtre_grise_les_frequences_et_les_recalcule_a_l_ecran` ;
quatre contre-preuves vues en échec (L-071) : sans recalcul après saisie, champs jamais grisés, interrupteur repris du log
sans comparer, curseur ignoré par le CLI. Captures de l'Et si sur `CHIRP-25Hz.bbl`.

---

## 2026-11-25 — La fiche du drone lit le log

### D-077 : taille saisie librement, taille des moteurs, pôles et boucle PID lus dans le dernier log

**Tranché par Cédric** le 2026-11-25, après son essai de la fiche : « la partie taille, c'est un menu déroulant où les
chiffres sont verrouillés […] je voudrais qu'on puisse modifier ça. La partie des pôles moteurs, […] tu peux récupérer
l'information à partir du log. La boucle PID, c'est la même chose. […] on voit le KV, mais on ne voit pas la taille du
moteur. » Entre deux propositions pour chaque point, il a retenu : saisie libre avec la liste en proposition (plutôt
qu'une liste plus « Autre… » et un profil choisi à la main) ; « suivre le log, avertir » (plutôt que toujours écraser par
le log, ou des champs en lecture seule). **Précisé** le même jour, après la première version qui lisait aussi le
KV : « Les KV moteurs ne doivent pas être tirés du log, ça doit être quelque chose qui est ajouté par l'utilisateur.
Comme la taille du drone, la taille des moteurs. » Amende D-068 (champs de la fiche) et D-061 (écarts au prototype).

**Contexte.** La taille choisit le profil de seuils de l'assistant : la liste figée des huit tailles garantissait un
profil connu, mais refusait un 3,5″ ou un 10″. Les pôles et la boucle PID se tapaient à la main alors que l'en-tête de
chaque log les porte (`motor_poles`, `looptime` et `pid_process_denom`).
⚠️ L'en-tête dit ce que **Betaflight** croit : un `motor_poles` faux y est recopié, et c'est justement ce qu'il faut voir.

**Décision.**
- **Taille libre** : un champ à saisir, la flèche déroule les huit tailles. Le profil se lit par
  `otb_advisor::profiles::size_profile_key`, avec les lectures et les bornes de la taille par le nom (3,8 / 5,8 / 7,8
  pouces, `profile_of_inches`, une seule fonction pour les deux) ; en plus : le signe `″`, un nombre en mm jusqu'à 100 →
  whoop, un nombre seul sous 20 → pouces. Sous le champ : « Seuils de l'Assistant : <profil> », ou « Taille non
  reconnue » et le profil par défaut (`drone_sheet::size_note`). La taille reste gardée telle que tapée.
- **Taille des moteurs** (`DroneSheet::motor_size`, clé `taille_moteurs`), texte libre (stator, `2207`) : aucun calcul
  ne s'en sert. La section Machine passe à quatre lignes : taille, batterie, poids ; taille des moteurs, KV, pôles ;
  hélices, ESC, Betaflight ; boucle PID.
- **Lu dans le log** (`Drone::from_log`, `model::LogMachine`, clé `lu_dans_le_log`) : à chaque log rangé
  (`App::remember`), pôles et boucle PID de l'en-tête (`drone_sheet::machine_of_log`, au dixième de kHz). Une valeur
  absente de l'en-tête reste **vide** : `Settings` met 14 pôles et 125 µs par défaut, on lit donc l'en-tête brut. Un
  log plus ancien que le dernier lu ne change rien (`take_log`).
- **La fiche suit le log pour la boucle PID** (un réglage dont le log fait foi), mais **ne remplit les pôles que
  vides** : le pilote sait ce qu'il a monté. Une différence s'écrit sous le champ, en couleur « attention » (« Le dernier
  log dit 14 : vérifiez motor_poles dans Betaflight, le filtre RPM s'en sert. »), jamais corrigée en silence
  (`log_note`).
- Un drone **sans fiche n'en reçoit pas** (D-068 reste) : la fiche qu'on lui ouvre part des valeurs du log plutôt que de
  celles d'un drone neuf ; de même pour « Nouveau drone » depuis le Guide, avec le log ouvert.
- **Le KV n'est jamais tiré du log**, comme la taille du drone et celle des moteurs : il décrit le matériel monté, que
  seul le pilote connaît ; le `motor_kv` de l'en-tête (Betaflight 4.5 et plus) n'est qu'un réglage, que rien n'oblige à
  tenir juste.

**Options écartées.** Liste plus « Autre… » avec un profil choisi à la main (un choix de plus, que la taille tapée suffit
à faire) ; écraser toujours par le log (un `motor_poles` faux effacerait la bonne valeur du pilote) ; champs en lecture
seule (un drone sans log ne pourrait plus être décrit, et l'écart ne se verrait plus) ; lire la taille des moteurs dans
le log (l'en-tête ne la porte pas) ; lire le KV dans le log (`motor_kv`), comme le faisait la première version : écarté
par Cédric, c'est au pilote de le dire.

**Témoins.** `rust/otb-advisor/tests/profiles.rs:taille_saisie_librement` (17 cas) ; `rust/otb-gui/tests/drone_sheet.rs` :
`une_taille_libre_choisit_son_profil_et_le_dit`, `le_log_dit_les_poles_et_la_boucle_pid` (sur
`tests/data/jeno5_5s.bbl` : 14 pôles, 3,2 kHz), `la_fiche_suit_la_boucle_pid_du_log_et_signale_les_poles_differents`,
`un_log_ouvert_renseigne_la_fiche`, `la_taille_et_la_taille_des_moteurs_se_tapent_a_l_ecran` ;
`rust/otb-store/tests/bibliotheque.rs:la_fiche_du_drone_se_retrouve_et_un_drone_sans_fiche_le_reste` (aller-retour, et
fichier d'avant lu sans les nouveaux champs). Trois contre-preuves vues en échec (L-071) : lire `motor_poles` sans
regarder l'en-tête brut, écrire les pôles du log par-dessus ceux du pilote, recopier `motor_kv` dans la fiche
(`un_log_ouvert_renseigne_la_fiche` : l'en-tête de `jeno5_5s.bbl` dit 1960, la fiche garde son KV vide). Captures `fiche-drone-*.png` des deux thèmes :
section Machine alignée sur Identité (un champ à unité qui dépassait sa colonne est corrigé au passage, L-111).

---

## 2026-11-25 — Plusieurs conversations en parallèle

### D-079 : une conversation par sujet, chacune dans son worktree, pilotées depuis le dossier principal

**Tranché par Cédric** le 2026-11-25 : « je veux pouvoir ouvrir plusieurs conversations avec toi pour […] optimiser
l'utilisation des tokens. Et chaque conversation sur ce projet sera sur un point spécifique du projet […] pour que tout ça
n'interfère pas dans le travail des agents. Je veux que tu sois capable de piloter tout ça. »

**Contexte.** Le 2026-11-25, trois conversations travaillaient en même temps (interface, fiche drone, bruit et filtres), deux
dans le même dossier : l'une a changé de branche sous les pieds de l'autre et restauré un fichier qu'elle modifiait ; le
crochet de documentation bloquait les commits de la troisième, pourtant dans son propre worktree, pour une décision citée par
une autre et pas encore écrite ; ce numéro, absent du registre, aurait été repris par une conversation qui se fiait à l'index (L-125).

**Décision.**
- Une conversation = un sujet = un worktree = une branche = une PR. Elle commence par « Chantier <sujet> » et crée son
  worktree (`EnterWorktree`, dans le dossier `worktrees` de `.claude`, ignoré par git) depuis `origin/main` ; elle n'en sort pas.
- Le dossier principal reste sur `main`, sans changement en cours : il est à la conversation de pilotage, qui tient le
  tableau des chantiers (`.git/otb-chantiers.md`), voit les recouvrements, propose l'ordre des fusions et prévient les
  autres conversations après chaque fusion. Procédure : compétence `chantiers`.
- Un numéro de registre se réserve par `otb-dev numero` (vu de tous les worktrees, réservations dans le dossier `.git`
  commun), jamais d'après le seul index de son dossier.
- `check-docs.sh` vérifie le dossier de la conversation (`cwd` de l'entrée du crochet), plus `$CLAUDE_PROJECT_DIR`.
- Les règles de confirmation (push, étiquette, PR, release) ne changent pas.

**Options écartées.** Des clones séparés (`OpenTuneBox-bruit`…) : la mémoire de Claude Code et les crochets dépendent du
dossier de lancement, et les réservations ne seraient plus communes. Un seul dossier avec des branches successives : c'est ce
qui a cassé. Un dossier `target` commun aux worktrees : les compilations simultanées s'attendraient (verrou de cargo) et les
crates du projet se recompileraient quand même ; on paie la place (≈ 20 Go par worktree, 630 Go libres le 2026-11-25).

**Témoins.** `rust/otb-dev/tests/numero.rs:deux_chantiers_ne_recoivent_jamais_le_meme_numero` (deux contre-preuves) ; essai du
crochet sur quatre dossiers, ancien et nouveau (L-125) ; première réservation réelle : L-125, parce que le numéro précédent était déjà cité dans le brouillon de ce chantier, écrit avant la réservation.

---

## 2026-11-25 — L'espace « Bruit et filtres » sans le terme D préfiltre

### D-078 : le terme D préfiltre n'est plus proposé dans « Bruit et filtres » ; « Terme D » montre le D filtré seul

**Tranché par Cédric** le 2026-11-25 : « le champ Terme D Préfiltre n'est plus utile », puis, entre trois propositions
(D filtré seul, D filtré avec le gyro avant filtres, case seulement masquée), « le D filtré seul ». Amende D-069 (ce que
montre le bouton « Terme D »).

**Contexte.** Betaflight n'enregistre pas le terme D avant ses filtres : OpenTuneBox le reconstruit à partir du gyro
(`rust/otb-log/src/flightlog.rs:FlightLog::d_term_prefilter`). D-069 en faisait l'étage « avant filtres » du bouton « Terme D », dans
les trois vues à courbes et cartes de l'espace.

**Décision.**
- Le Spectre, Fréq. × gaz et Fréq. × temps ne proposent plus « Terme D préfiltre » (`rust/otb-gui/src/tools/common.rs:NOT_IN_NOISE_SPACE`) :
  ni case, ni ligne des listes, et les fonctions qui cochent un jeu l'ignorent. Il reste calculé là où il sert : retard
  des filtres D, courbe « D avant filtres » du « Et si » (entrée de la simulation), visualiseur.
- `NoiseSignal::DTerm` n'a plus qu'un jeu, `dterm`. Changer de signal garde l'étage filtré ; tous les jeux d'un signal
  deviennent tous ceux de l'autre (le D seul redevient le gyro avant et après filtres).
- Fréq. × gaz : les préréglages 1 et 5 de PIDtoolbox (« … D pf | D ») sont retirés, les autres renumérotés de 1 à 7 ;
  « Terme D » prend « A|B|C|D D », une colonne par fragment (`PRESET_DTERM`), sauf depuis le gyro filtré seul, où
  chaque colonne garde son fragment.
- Dans la même demande, les onglets de l'espace suivent le prototype (`tabOn` : icône, gris, bleu souligné de 2 px
  pour la vue affichée), avec la main au survol ; « Et si », absent du prototype, prend l'icône `funnel`.

**Écartés.** Le D filtré avec le gyro avant filtres en repère (deux signaux sur un même bouton) ; masquer la case en
gardant le D reconstruit sous le bouton (une courbe sans case pour la retirer).

**Témoins.** `rust/otb-gui/tests/spectral.rs` (`le_terme_d_prefiltre_nest_pas_propose`), `rust/otb-gui/tests/noise_space.rs`
(`changer_de_signal_garde_letage_choisi`, `frequence_gaz_suit_le_signal_par_ses_colonnes`,
`les_onglets_de_lespace_sont_des_boutons`), contre-preuves vues en échec (L-071) : sans le filtre de la liste, sans la
main au survol. Captures des quatre vues dans les deux thèmes (`CHIRP-25Hz.bbl`).

---

## 2026-11-23 — Le mode stockage de masse et les cartes SD (lot 32c)

### D-076 : terme D des logs ralentis — celui du firmware, même quand le log est ralenti

**Tranché par Cédric** le 2026-11-24, après la mesure de W-083 point 1 ([validation.md](validation.md#terme-d-des-logs-ralentis--cause-mesurée-2026-11-24-w-083)) :
« je veux que le D soit le plus correct possible ».

**Contexte.** Sur les logs qui ne gardent qu'une boucle PID sur N, le D de la chaîne à l'arithmétique du firmware (lot 24c)
s'écarte davantage du D enregistré que celui de la chaîne actuelle (0,64 → 0,91 dB en médiane entre 100 et 250 Hz). Mesuré
sur des logs enregistrés à chaque boucle puis réduits : avec le **vrai** gyro, le D du firmware colle au D enregistré comme
sur le log complet ; l'excès vient du gyro non filtré sous-échantillonné sans anti-repliement (W-067). Le D calculé à la
cadence du log (dérivée et filtres du D à cette cadence) est 0,4 à 0,7 dB trop bas **même avec le vrai gyro** : il ne colle
que par compensation, et pas sur un drone qui replie peu (Air65 : 0,63 dB contre 0,21 dB).

**Décision.** À la bascule (lot 24d), le terme D rejoué (page Filtres, assistant) est celui de `firmware_chain::dterm` :
dérivée et filtres du D à la cadence PID, une boucle sur N gardée ensuite, pour **tous** les logs. L'écart qui reste sur un
log ralenti est une perte d'information du log, pas un défaut de calcul ; il ne se corrige pas, il se dit (point « écart au
vol réel mesuré et affiché » du lot 24).

**Options écartées.** Dériver à la cadence du log pour les logs ralentis (faux en lui-même, juste par compensation) ; choisir
la cadence de la dérivée selon le drone ou la cadence du log ; corriger l'excès par un gain empirique (le repliement dépend du
bruit de chaque drone au-dessus de la demi-cadence du log, que le log ne contient pas).

### D-075 : le mode stockage de masse, pour la mémoire interne au choix et pour toute carte SD, avec suppression vérifiée

**Tranché par Cédric** le 2026-11-23, au début du lot 32c : les quatre questions de la phase « planifier », après lecture
du firmware ([spec-usb.md](spec-usb.md) §1 et §5).

**Contexte.** Le mode stockage de masse lit la mémoire interne bien plus vite que MSP (3,25 Mo par minute mesurés en
MSP), mais son disque est en lecture seule ; c'est la seule voie pour une carte SD, que MSP n'efface pas. Le disque ne
porte pas l'identifiant de la carte, et il se monte selon le système.

**Décision.**
- **Le redémarrage** : `MSP_REBOOT` avec le **seul** mode 2 (stockage de masse), après avoir lu l'identité, l'état armé
  (refus) et ce que la carte enregistre (`rust/otb-usb/src/msc.rs:reboot_to_disk`). Jamais le chargeur de firmware.
  `MSP_REBOOT` et `MSP_SDCARD_SUMMARY` s'ajoutent à la liste fermée des commandes (`rust/otb-usb/tests/contrats.rs`) ; aucune n'écrit
  de réglage (D-063 tient).
- **Le disque** : OpenTuneBox **attend que le système le monte** et le repère seul (points de montage FAT sous Linux,
  lettres de lecteur sous Windows ; `BTFL_ALL.BBL` à la taille de la place occupée, ou dossier `LOGS`) ; s'il ne vient
  pas, un message dit de l'ouvrir dans le gestionnaire de fichiers, et **« Choisir le dossier… »** sert de secours.
  Aucun programme extérieur (pas de `udisksctl`).
- **Mémoire interne** : **deux voies au choix**, « Copier en mode disque » (rapide ; pour effacer, débrancher puis
  rebrancher, puis effacement MSP qui vérifie le même identifiant gravé — refusé si l'identifiant est nul) et « Copier par
  USB série » (MSP, lot 32b). La copie par le disque passe par les mêmes contrôles (`verify_and_save`).
- **Carte SD** : prise en charge **dans le drone** (mode stockage de masse) **et dans un lecteur de cartes** du PC ; dans
  ce dernier cas, pas d'identifiant : le drone se choisit à la main (le nom lu dans le log est proposé).
- **Effacer une carte SD** : en deux clics, comme la mémoire interne ; seuls les logs copiés **et vérifiés** partent,
  chacun **relu et comparé octet pour octet** à sa copie juste avant d'être supprimé ; les autres fichiers ne sont pas
  touchés (`rust/otb-usb/src/sd.rs:delete_sd_logs`, seul chemin qui supprime un fichier). Le pilote éjecte le disque avant
  de débrancher.

**Options écartées.** Monter le disque soi-même sous Linux (`udisksctl`, dépend d'un programme du système) ; le mode
disque d'office, ou MSP d'office, pour la mémoire interne ; la carte SD dans un lecteur reportée ; ne jamais effacer la
carte SD (le pilote le ferait à la main, sans contrôle).

**Témoins.** `rust/otb-usb/tests/stockage_de_masse.rs` (14 tests) et `rust/otb-usb/tests/contrats.rs` (un seul
redémarrage, en mode 2 ; une seule suppression de fichier, sous permis), avec **4 contre-preuves vues en échec** (L-071) :
relecture avant suppression neutralisée, disque d'une autre taille accepté, état armé non relu, seconde suppression
ajoutée ailleurs. Essai sur une vraie carte : procédure de [spec-usb.md](spec-usb.md) (`otb-dev usb msc`, `usb sd`).

**L'écran (32c-2, tranché par Cédric le 2026-11-23).** La carte SD d'un lecteur est sur **le même écran** (section
« Carte SD dans un lecteur », visible sans drone branché) ; après une copie de la mémoire par le disque, OpenTuneBox
**guette le retour de la carte et s'y reconnecte seul** ; pour la mémoire interne, **« Copier les vols » (USB série)
reste le bouton principal**, « Copier en mode disque » à côté. **Amende D-074** : l'effacement n'exige plus une copie
vérifiée sur **cette** connexion, mais sur cette connexion **ou** une copie faite par le disque d'une carte au **même
identifiant connu** et à la même place occupée (`rust/otb-gui/src/usb.rs:Held`), relue par `erase_flash`. Écartés : un
bouton à part au tableau de bord pour le lecteur ; une reconnexion au clic du pilote ; le mode disque en bouton principal.
Témoins : `rust/otb-gui/tests/usb_disque.rs` (8 tests, 3 contre-preuves vues en échec : carte rebranchée sans vérifier
l'identifiant, log ouvert depuis la carte pris pour une copie, suppression proposée avant le rangement), et
`rust/otb-usb/tests/stockage_de_masse.rs:carte_sd_copie_rangee_qui_est_le_fichier_de_la_carte` (contre-preuve vue).

**Ouvert.** W-079 (carte à mémoire interne **et** carte SD).

---

## 2026-11-23 — L'écran de la récupération par USB (lot 32b)

### D-074 : la récupération par USB, un écran hors rail qui range sous le drone de la carte et efface en deux temps

**AMENDÉE par D-075** (2026-11-23) : une copie faite par le mode disque d'une carte au même identifiant connu ouvre aussi l'effacement, après rebranchement.

**Tranché par Cédric** le 2026-11-23, au début du lot 32b : six questions de la phase « planifier » (aucune planche du
prototype ne dessine cet écran ; la fiche du drone y montre seulement l'identifiant et la carte « lus au branchement USB »).

**Décision.**
- **L'écran** est **hors rail**, ouvert par « Récupérer par USB » en haut du tableau de bord, comme la fiche du drone ; il
  revient au tableau de bord. Une colonne d'étapes : carte branchée, drone, vols, configuration.
- **Découpage** : **32b = la voie MSP** (copie bloc par bloc, contrôles, effacement, rangement, configuration) ; le **mode
  stockage de masse** et les **cartes SD** (redémarrage en MSC, disque monté selon le système, effacement par le système)
  forment le **lot 32c**.
- **Le drone** est reconnu par l'**identifiant gravé de sa carte** (`rust/otb-store/src/model.rs:Drone`, champ `uid`). À
  la première connexion d'une carte, l'écran **demande** : le drone du même nom est proposé, un autre drone ou « Nouveau
  drone » se choisissent (nom vide ou déjà pris refusé) ; le choix range l'identifiant, la carte et le firmware sur le
  drone. Un identifiant nul ne reconnaît rien (D-063). Un log déjà rangé garde son drone quand on le rouvre
  (`App::remember`), même si son nom de machine en désigne un autre.
- **L'effacement** n'est proposé qu'**après** une copie vérifiée (taille, décodeur, fichier relu : `otb_usb::verify_and_save`),
  **rangée** dans la bibliothèque et la bibliothèque **écrite sur le disque** ; un bouton « Effacer la mémoire du drone… »
  puis « Effacer définitivement » (deux clics, rappel des hélices). Le fil de travail garde la copie vérifiée : sans elle,
  sur **cette** connexion, l'effacement est refusé sans rien envoyer (`rust/otb-gui/src/usb.rs:Connection`). Drone armé :
  rien n'est lu ni effacé (copie refusée par le fil, boutons grisés, et `erase_flash` relit l'état juste avant).
- **La configuration** (`diff all`) se sauvegarde par **un bouton à part**, « Sauvegarder la configuration » (la carte
  redémarre ensuite) ; gardée seulement si elle a changé (`Library::record_config`).
- **Après la copie** : les vols sont rangés sous le drone (un fragment par vol) et **« Ouvrir les vols »** les met dans la
  barre d'analyse, sans rien remplacer d'office.
- **W-075** : la confirmation de suppression d'un drone propose **« Supprimer aussi les N fichiers copiés par USB »**,
  **décochée** ; seuls les fichiers marqués copiés, et seulement dans le dossier des copies de la bibliothèque, partent
  (`Library::delete_copies_of_drone`).
- Copie rangée dans `bibliotheque/logs/<drone>/<AAAA-MM-JJ_HH-MM-SS>.bbl` (instant en temps universel), jamais écrasée.

**Options écartées.** Une entrée « Récupération » dans le rail (écart de plus avec le prototype) ; le bouton dans la fiche
du drone (un drone inconnu n'y aurait pas accès) ; les deux voies de copie dans le même lot ; le drone reconnu par son nom
sans question (deux drones de même nom se mélangent, un drone renommé en devient un autre) ; une carte = toujours un
nouveau drone ; une case « effacer après la copie » cochée avant le départ ; pas d'effacement à l'écran ; le `diff all` à
chaque récupération ; garder ou supprimer d'office les fichiers copiés d'un drone supprimé ; ouvrir d'office les vols copiés.

**Témoins.** `rust/otb-gui/tests/usb.rs` (15 tests sur la carte simulée d'`otb-usb`, dont le parcours complet à l'écran)
et `rust/otb-store/tests/bibliotheque.rs` (3 tests), avec **5 contre-preuves vues en échec** (L-071) : log copié rouvert
rangé par son nom, copie d'un drone armé, effacement proposé sans rangement, fichiers copiés effacés sans la case, fichier
hors du dossier des copies effacé. Captures des deux thèmes sur la carte simulée. **Essayé à l'écran sur une vraie carte** (G474, Linux, 2026-11-23) :
tout le parcours passe ([spec-usb.md](spec-usb.md), résultats).

---

## 2026-11-23 — L'assistant d'un vol à l'autre (lot 31d)

### D-073 : l'assistant compare au vol précédent sur ses jauges, et les choix du pilote s'inscrivent au clic

**Tranché par Cédric** le 2026-11-23, au début du lot 31d (W-076) : les quatre questions de la phase « planifier ».

**Contexte.** Le prototype ajoute aux cartes de l'assistant un menu « comparé à », un badge du drone, un pouce levé ou
baissé par conseil, un historique « appliqué / écarté » et le bandeau « Fragment déjà traité » (D-070 les avait reportés
au lot 31). Il ne dit pas ce que la comparaison change aux cartes, et ses votes partent avec le bouton « Appliquer », refusé
par D-063.

**Décision.**
- **Le vol comparé** est par défaut le **vol précédent du drone**, dans l'ordre des vols du suivi
  (`rust/otb-store/src/lib.rs:Library::flights_of_drone` : date, nom du log, session, début), qu'il soit ouvert ou non ; le
  menu choisit un autre vol du drone ou un fragment de la liste d'analyse. Le choix se perd quand le fragment analysé change.
  Son rapport se calcule **avec le profil du fragment analysé** (mêmes seuils, donc mêmes cartes), sur le fil de calcul de la
  page, sur le log ouvert s'il l'est, sinon en lisant son fichier.
- **Ce que la comparaison montre** (`rust/otb-gui/src/advice.rs:compare_cards`) : sur chaque jauge, un **repère creux** et la
  valeur du **même constat** (clé et axe, D-028) dans le rapport du vol comparé — jamais un second calcul de la mesure (D-070),
  et rien si l'unité diffère ; « NOUVEAU » sur une carte absente du vol comparé ; une ligne « Plus signalé depuis … » pour
  ses cartes disparues. Les vérifications OK ne se comparent pas.
- **Pas de « drones de même taille »** : sans vol précédent, l'en-tête dit « Aucun vol à comparer — pas de vol précédent pour
  ce drone ». Les seuils de l'assistant viennent déjà des drones de même taille.
- **Les votes s'inscrivent au clic** (`rust/otb-store/src/model.rs:Advice`, table `conseils.json`) : pouce levé = conseil
  **appliqué** à la main sur le drone, inscrit tout de suite avec l'instant (ISO, temps universel) ; pouce baissé = conseil
  **écarté**, inscrit seulement avec sa raison (« Enregistrer » refusé sans elle). Un second clic sur le pouce enfoncé annule.
  Un choix par carte et par fragment ; il suit le fragment s'il est supprimé, reste si ses bornes changent (c'est un geste
  fait sur le drone, pas une mesure). Seuls les conseils qui proposent un geste se votent (`can_vote`).
- **La bibliothèque reste à la fenêtre** : la page la lit avant son affichage (`Tool::read_library`) et rend ses changements
  (`Tool::take_library_edits`), que `App` applique.
- **« Fragment déjà traité »** quand le drone a des vols rangés après le fragment, avec ses choix ; un conseil appliqué est
  « pris en compte » par le premier vol suivant **daté du jour du choix ou après** (`taken_by`). Limite assumée : la date d'un
  vol est au jour près, un vol du même jour fait avant le choix compte aussi. Les dates s'affichent au jour (l'heure, en
  temps universel, serait fausse d'une ou deux heures en France).
- **Lignes CLI** (`set …` par conseil) : **pas dans ce lot** (W-078).

**Écarts avec le prototype, voulus.** Pas de bouton « Appliquer » ni d'envoi au drone (D-063) : le pouce levé dit « je l'ai
réglé », il n'envoie rien. Pas d'état « en attente d'envoi ». Les pouces sont sur les conseils qui ont un geste, pas sur ceux
qui ont des lignes CLI (il n'y en a pas). Heure du choix non affichée.

**Options écartées.** Les écarts chiffrés lus dans la table des mesures suivies (22 grandeurs seulement, et ni carte nouvelle
ni carte disparue) ; le nom du vol comparé dans l'en-tête seul (sans effet sur les cartes) ; « Drones de même taille »
(doublon des seuils) ; des votes en attente inscrits par un bouton « Inscrire à l'historique (N) ».

**Témoins.** `rust/otb-store/tests/bibliotheque.rs` (4 tests) et `rust/otb-gui/tests/filter_tools.rs` (9 tests : 4 purs, 5 de
la page), avec **5 contre-preuves vues en échec** (L-071) : choix gardé après la suppression de son fragment, vols non rangés,
unité de la jauge non vérifiée, vol « pris en compte » sans la date du choix, « Enregistrer » sans raison. Captures des deux
thèmes sur `FINAL.bbl` et `LAST.bbl` du JeNo 5.

---

## 2026-11-23 — Mesures suivies dans le temps (lot 31a)

### D-072 : les mesures suivies viennent du rapport de l'assistant, et sa cible part de ses seuils

**Tranché par Cédric** le 2026-11-23, au début du lot 31 : les quatre questions de la phase « planifier ».

**Contexte.** Le lot 31 doit répondre à « est-ce que ce drone s'améliore ? ». Le prototype dessine une « Fiche cible » dans
la fiche du drone (dix valeurs, et un bouton « Valeurs proposées » par taille), mais **aucun graphique de suivi**, et ses
valeurs proposées (8, 10, 14 ms de latence, 45° de marge…) n'ont pas de source. Le plan demandait de « dire lequel
prime » entre la cible du drone et le seuil de l'assistant (D-011), deux chiffres qui pourraient se contredire.

**Décision.**
- **Les grandeurs suivies sont lues dans le rapport de l'assistant** (`rust/otb-advisor/src/tracked.rs:from_report`), là où
  ses règles les lisent : latence, pic, temps de montée, marges de phase et de gain, nombre de fenêtres (par axe) ; bruit du
  gyro et du terme D (le plus haut de roll et pitch, comme la règle), retard des filtres gyro et D (seulement si le vol a
  assez de mouvements, comme la règle). Toujours les mêmes 22 lignes par fragment ; une grandeur non mesurée vaut NaN
  (`null` dans le fichier) et sa ligne est gardée, pour que le fragment ne se recalcule pas à chaque ouverture.
- **Une mesure porte les bornes de son fragment**, ses réglages de calcul et `CALC_VERSION` : elle est jetée quand les bornes
  changent (même avant la lecture de la table, D-064) ou quand la version monte. Elle doit se recalculer **au bit près** à
  l'identique (critère d'arrêt du lot).
- **Calcul en arrière-plan à l'ouverture de la fiche d'un drone**, fragment par fragment, profil de taille détecté comme
  l'assistant le fait par défaut ; un résultat n'est rangé que si le fragment a encore les bornes mesurées.
- **Lequel prime** (pour 31b et 31c) : les « Valeurs proposées » de la fiche cible sont **les seuils de l'assistant** pour la
  taille du drone ; par défaut les deux chiffres sont donc les mêmes. L'assistant garde ses seuils dans ses constats et ses
  jauges (« seuil ») ; la cible du pilote, s'il la change, ne sert que la fiche et le suivi.
- **Une grandeur sans seuil dans l'assistant** (temps de montée, marge de gain, bas de la zone du pic) n'a **pas de valeur
  proposée** : la case reste vide jusqu'à ce que le pilote la remplisse, et le suivi trace la mesure sans bande.
- **Le suivi est dans la fiche du drone**, sous la fiche cible ; pas de nouvelle entrée du rail.
- **W-076** (comparaison avec le vol précédent, votes et historique des conseils) vient **après** : lot 31d.
- **Précisé au lot 31b** (Cédric, 2026-11-23) : le **haut de la zone du pic** proposé est le seuil **sans** feedforward
  (`STEP_PEAK_HIGH`, 1,20 ; 1,15 depuis D-082) — la fiche ne sait pas si le drone vole avec, le pilote qui en a la monte lui-même ; la latence
  proposée est le haut de la plage habituelle du profil (`latency_ms.1`). Tant que le pilote n'a **rien changé**, rien
  n'est enregistré (`DroneSheet::target` vaut `None`) : les cases **suivent** la taille et les seuils de l'assistant s'ils
  sont recalibrés ; la première case changée fige les dix, et « Valeurs proposées » les remet à suivre. Le sous-titre du
  prototype (« les jauges, le guide et le suivi s'y comparent ») devient « le suivi de ses vols s'y compare ; l'assistant
  garde ses propres seuils ». Liste des cases et valeurs proposées : `rust/otb-advisor/src/target.rs:proposed`.
- **Précisé au lot 31c** (Cédric, 2026-11-23) : un graphique par grandeur de la fiche cible (le pic une fois, ses deux
  bords), dans la colonne de droite ; en abscisse le **numéro du vol** (fragments mesurés rangés par date : la date d'un
  fichier est au jour près, plusieurs vols d'un jour s'empileraient sur une vraie échelle de temps) ; la **zone visée
  teintée** et ses bords en pointillés (une cible « ≤ » teint le dessous). Les points suivent l'assistant : aucun sous
  `STEP_MIN_WINDOWS` fenêtres, creux (« moins sûr ») sous `STEP_SURE_WINDOWS`, sauf les marges de la boucle ouverte. La
  bande suit les cases du **formulaire**, avant même l'enregistrement. Séries : `rust/otb-gui/src/tracking.rs:charts_of`.
  Écartés : la vraie date en abscisse, un trait seul sans teinte (on voit moins vite de quel côté être), les graphiques
  pleine largeur sous les deux colonnes (un écart de plus avec ce qui avait été tranché).

**Options écartées.** La cible du drone à la place du seuil dans les jauges des cartes, comme le prototype : une carte
pourrait alerter alors que sa jauge est dans la cible. Les valeurs du prototype : sans source (D-005, D-011). Calibrer sur le
corpus les trois grandeurs sans seuil : un passage de calibration de plus pour des valeurs que le pilote fixe lui-même. Un
écran « Suivi » dans le rail : un écart de plus avec le prototype (D-061). Au lot 31b : des valeurs proposées **recopiées**
à la création de la fiche, comme le prototype (elles cesseraient d'égaler les seuils dès un changement de taille ou une
recalibration) ; le pic haut lu sur le dernier log (1,60 avec feedforward), qui changerait d'un vol à l'autre.

**Témoins.** `rust/otb-advisor/tests/tracked.rs` (grandeurs identiques à celles du rapport, recalcul au bit près après
fermeture de la bibliothèque) ; `rust/otb-gui/tests/tracking.rs` (calcul en arrière-plan, échec dit et pas relancé,
fragment déplacé pendant le calcul) ; `rust/otb-gui/tests/tracking_charts.rs` (ordre des vols, fenêtres, bande, fiche) ; 11 sessions réelles identiques au bit près ([validation.md](validation.md#mesures-suivies-dans-le-temps-2026-11-23-lot-31a)).

## 2026-11-23 — Avancement du parcours (lot 30g)

### D-071 : le Guide lit le rôle sur le fragment ; la carte du drone compte le dernier relevé du Guide

**Tranché par Cédric** le 2026-11-23, au début du lot 30g, entre trois propositions pour la carte (dernier relevé du
Guide, retenu ; seulement les logs ouverts ; rien de changé) et deux pour les logs du Guide (fragments actifs, retenu ;
tous les fragments ouverts). Amende D-039 (mémoire des rôles), D-061 (troisième écart au prototype, l'avancement) et
précise D-066 pour le Guide.

**Contexte.** Le Guide rangeait le rôle d'un log (« filtres », « série PID ») par **chemin de fichier** (D-039), alors
que depuis D-067 une session peut porter plusieurs fragments, chacun avec son rôle dans la bibliothèque : un fragment
« Série PID » taillé dans un log de filtres était vérifié comme un log de filtres (W-074). Et la carte d'un drone au
tableau de bord ne comptait que les cases cochées à la main (D-061) : les cases que le Guide vérifie seul demandent
une analyse par log, trop chère en ouvrant le tableau de bord.

**Décision.**
- **Le rôle est celui du fragment** (`otb_store::model::Fragment::role`), seule source : « → classer dans les … » du
  Guide l'écrit dans la bibliothèque (`GuideAction::Role`), comme l'écran d'extraction. Un fragment sans rôle garde le
  rôle **deviné** par la vérification du log.
- **Anciens rôles par fichier** (clé `guide.roles` de la persistance d'eframe, `GuidePage::old_roles`) : relus et
  réécrits tels quels, plus jamais changés ; à l'ouverture d'un fichier, un fragment de ses sessions **sans rôle** prend
  le sien (`App::give_role`). Un ancien rôle ne remplace jamais celui d'un fragment.
- **« Ouvrir les logs des filtres… »** (ou de la série) donne ce rôle aux fragments des sessions ouvertes, à la place
  de celui qu'ils avaient : le pilote vient de le dire (`App::open_as`).
- **Le Guide ne vérifie que les fragments actifs** du drone, comme toutes les pages (D-066) ; les désactivés sont
  nommés sous le panneau (« Désactivés dans la barre, non vérifiés »), jamais rangés « autre drone ».
- **Relevé pour la carte** : quand chaque fragment actif du drone a sa vérification, le Guide rend les clés des cases
  que ces logs vérifient (`GuideAction::Verified`, `guide::verified_keys`) ; `App` les range sur la fiche
  (`DroneSheet::verified`, clé `cases_verifiees`) seulement s'il a changé. La carte compte cases cochées et relevé
  (`dashboard::stage`, même compte que le Guide par `guide::progress_with`), sans aucune analyse. Sans fragment actif
  du drone, le relevé n'est pas touché : la carte garde le dernier. « Recommencer » l'efface avec les cases, et le
  Guide le refait aussitôt pour les logs encore ouverts.
- La barre d'avancement d'une carte porte un nom accessible (« Avancement de JeNo Ced : 19 % »).

**Écartés.** Pour la carte : compter seulement les logs ouverts et déjà vérifiés (la carte changerait selon ce qui est
ouvert et retomberait aux cases cochées à chaque lancement) ; vérifier tous les fragments d'un drone en ouvrant le
tableau de bord (une ouverture de fichier par log, ≈ 0,3 s chacune). Pour les rôles : garder la mémoire par fichier à
côté de celle des fragments (deux sources qui se contredisent dès qu'une session porte deux fragments). Pour le Guide :
vérifier aussi les fragments désactivés (la barre ne déciderait plus pour toutes les pages).

**Limite assumée.** La carte dit ce que le Guide a vu **à son dernier passage** : un log ajouté depuis ne compte qu'une
fois le Guide affiché de nouveau.

**Témoins.** `rust/otb-gui/tests/guide.rs` (`le_guide_lit_le_role_de_chaque_fragment_et_laisse_les_desactives`,
`le_releve_du_guide_va_sur_la_fiche_et_la_carte_le_compte`,
`un_ancien_role_va_au_fragment_qui_n_en_a_pas_et_le_role_choisi_le_remplace`, et
`les_logs_du_drone_sont_verifies_classes_et_les_commandes_copiables` récrit sur le rôle du fragment),
`rust/otb-gui/src/dashboard.rs` (`l_etape_compte_les_cases_cochees_et_le_releve_du_guide`),
`rust/otb-store/tests/bibliotheque.rs` (`la_fiche_du_drone_se_retrouve_et_un_drone_sans_fiche_le_reste`, relevé
compris) ; 8 contre-preuves vues en échec (L-071). Captures `tableau-de-bord-releve-clair.png` et `-sombre.png` de
l'exemple `capture`.

## 2026-11-23 — Fiche d'un drone

### D-070 : l'assistant en cartes, et la mesure contre son seuil en jauge

**Tranché par Cédric** le 2026-11-23, au début du lot 30f : les **cartes seules** dans ce lot (la comparaison avec un
autre vol, les votes et l'historique attendent le lot 31, W-076) ; **D-063 gardée** (le bouton « Appliquer » du prototype,
qui enverrait les réglages au drone, n'est pas repris) ; une jauge pour **chaque constat à un seul seuil**. Applique D-061
à l'écran Assistant.

**Contexte.** Le prototype montre l'assistant en cartes : une par constat, triée par gravité, avec la mesure contre son
seuil en jauge (« 13,8 ms mesuré », « 10 ms cible »), les vérifications OK sur une seule ligne, et à droite la réponse
indicielle de chaque axe en petit. Un constat (`rust/otb-advisor/src/rules.rs:Finding`) ne portait que du texte : la
mesure et le seuil étaient écrits dans son détail, jamais rendus en nombres.

**Décision.**
- **Contrat de la jauge** (`rust/otb-advisor/src/rules.rs:Gauge`) : un constat qui compare **une** mesure à **un** seuil porte
  `gauge` — la mesure et le seuil **tirés des mêmes variables que la règle et son texte** (jamais un second calcul),
  l'unité, les décimales, l'échelle et deux libellés (« mesuré », « seuil », « repère », « cible »…). Échelle par défaut :
  de 0 à deux fois le seuil ; en décibels (`Gauge::db`), 10 dB de part et d'autre du seuil. `Gauge::marks` borne la mesure
  à la jauge, et ne rend rien pour une valeur non finie : la carte s'affiche alors sans jauge. Pas de jauge quand
  plusieurs mesures ou deux seuils décident ensemble (série de vols, plage, liste de raies) : mieux vaut pas de jauge
  qu'une jauge fausse. La liste des constats avec et sans jauge est dans la PR du lot.
- **Hors des références Python** : `otb-parity` ne compare pas la jauge ; les textes restent la seule sortie vérifiée au
  bit près (0 échec sur 1 732 appels avec les jauges posées).
- **La page** (`rust/otb-gui/src/tools/advisor.rs`) : en-tête du fragment à sa couleur (taille du drone, période,
  firmware), une pastille par gravité, une carte par constat triée par gravité puis dans l'ordre des règles
  (`split_cards`), la ligne « N vérifications OK » au bord pointillé. Une carte se reconnaît par sa clé et son axe,
  jamais par son titre (`card_id`, D-028). Les catégories (« Réponse », « Bruit »…) ne sont plus des titres de la page.

**Écarts avec le prototype, voulus.**
- Les cartes **se replient et se déplient** (chevron en haut à droite), et la ligne des OK se déplie en cartes : le
  prototype fige les « info » repliées et les OK sur une ligne, sans geste pour en lire le détail.
- Les **actions restent des phrases entières** (« → … »), et non une étiquette courte suivie de son explication : leurs
  textes sont comparés à ceux de Python.
- Ni badge de drone ni « comparé à » dans l'en-tête, ni votes, historique ou lignes CLI dans les cartes : W-076.
- Le **choix de la taille du drone** reste dans le panneau de droite commun aux pages, absent du prototype : sans lui, un
  drone mal détecté n'a pas de recours.
- Le bouton **« Appliquer »** n'est pas repris : la carte n'est jamais écrite (D-063).

**Écartés.** Couper chaque action en « geste + explication » (les textes changeraient, et avec eux les références
Python) ; une jauge pour tous les constats, même à plusieurs seuils (une jauge fausse trompe plus qu'une absence) ;
recalculer la mesure dans l'interface à partir du rapport (deux calculs qui peuvent diverger).

**Témoins.** `rust/otb-gui/tests/filter_tools.rs` (4 tests purs, 3 tests de la page) et `rust/otb-advisor/tests/gauge.rs`
(5 tests, dont la mesure de la jauge égale au nombre du texte sur `tests/data/jeno5_5s.bbl`), avec **9 contre-preuves vues
en échec** (L-071) : sans le tri, une info ouverte au départ (calcul et écran), les OK dépliés au départ, une pastille
d'alerte vide, un sigle mis en minuscules, une carte reconnue par son titre, la jauge non bornée, l'échelle en décibels
non centrée. Captures des deux thèmes ; 0,4 ms par cadre (`--example capture`, `FINAL.bbl`).

**Ouvert.** W-076 (traité au lot 31d, D-073, sauf les lignes CLI : W-078).

### D-069 : un seul choix « Gyro | Terme D » et des axes partagés pour l'espace « Bruit et filtres »

✏️ AMENDÉE par D-078 (le bouton « Terme D » montre le D filtré seul : le D préfiltre, reconstruit, n'est plus proposé
dans l'espace ; × gaz prend « A|B|C|D D » ; le reste de D-069 reste en vigueur).

**Tranché par Cédric** le 2026-11-23, au début du lot 30e : les cases détaillées de chaque vue sont **gardées dessous**
(plutôt que repliées ou supprimées), et le partage des **axes** entre dans le même lot. Précise D-060 (ce qu'un espace
partage entre ses vues).

**Contexte.** Le prototype porte en tête du panneau de droite de l'espace un seul choix « Gyro | Terme D », qui change
à la fois le Spectre, Fréq. × gaz, Fréq. × temps et la chaîne de filtres du « Et si ». Chez nous, chaque vue avait ses
propres cases : jeux de données du Spectre, préréglages de PIDtoolbox pour × gaz, liste « Données » pour × temps, onglets
« Gyro | D | Notch dyn / RPM » du « Et si ». Couvrir PIDtoolbox reste l'objectif : l'erreur PID, la consigne ou les
comparaisons A|B ne doivent pas disparaître.

**Décision.**
- **Le choix appartient à l'espace** (`rust/otb-gui/src/state.rs:SpaceChoice`, dans `AppState`) : un signal
  (`NoiseSignal`) et trois axes, avec un numéro qui change à chaque choix. Il est dessiné en tête du panneau de droite
  de chacune des quatre vues (`rust/otb-gui/src/tools/common.rs:signal_tabs`, commande segmentée du prototype).
- **Une vue se met au choix quand elle ne l'a pas encore vu** (`ToolPage::follow_space`), à son premier affichage puis
  après chaque choix fait ailleurs — avant de lancer son calcul. Une vue cachée ne calcule toujours pas (D-060).
- **Un changement fait dans une vue devient le choix de l'espace** (`publish_space`) : un clic sur « Gyro » ou
  « Terme D », un axe coché, ou un signal choisi dans ses cases. Un choix qui n'est ni le gyro ni le D seul (l'erreur
  PID, le gyro et le D mêlés) **n'allume aucun bouton** et ne change pas les autres vues.
- **Changer de signal garde l'étage choisi** (`NoiseSignal::counterpart`) : le gyro filtré seul devient le D filtré
  seul, et chaque colonne de × gaz reste sur son fragment. Un choix mêlé laisse place au signal avant et après ses
  filtres : les deux courbes du Spectre ; pour × gaz, le préréglage « A|A|B|B » du signal (`PRESET_GYRO`,
  `PRESET_DTERM`). × gaz s'ouvre donc sur « Gyro pf | Gyro » par fragment, et non plus sur le préréglage 1 de PIDtoolbox
  (gyro et D mêlés), toujours dans la liste.
- **Le « Et si » perd ses onglets « Gyro » et « D »**, doublons du choix de l'espace : la chaîne du gyro garde deux pages,
  « Passe-bas et notchs » et « Notch dyn / RPM ». Ses deux lignes de graphiques (gyro et D) restent affichées.

**Écartés.** Supprimer les cases détaillées (on perdait des vues de PIDtoolbox) ; les replier sous « Avancé » (un geste
de plus pour un pilote qui s'en sert) ; un choix par vue (le prototype le partage) ; ne mettre au choix que les vues qui
le montrent déjà, sans toucher aux réglages des autres (deux vues voisines montreraient deux signaux).

**Témoins.** `rust/otb-gui/tests/noise_space.rs` (5 tests), avec 5 contre-preuves vues en échec (L-071) : sans
`follow_space`, sans la recopie des axes, `of_keys` qui saute un autre signal, sans `counterpart`, `PRESET_GYRO` sur le
préréglage 1. Captures des quatre vues dans les deux thèmes. Temps d'un cadre (`--example capture`, `FINAL.bbl`, deux
fragments) : Spectre 7,8 ms (7,8 avant), Fréq. × gaz 12 ms contre 9,3 avant, **à cause de son contenu** (deux fragments,
avant et après filtres, au lieu d'un log en quatre colonnes : 9,0 ms avec l'ancien préréglage sur la même branche), sous
les 16,7 ms d'un écran à 60 images par seconde.

### D-068 : une seule fiche par drone, rangée dans la bibliothèque ; le Guide la lit

**AMENDÉE par D-077** (2026-11-25) : taille saisie librement, champ « Taille des moteurs », pôles et boucle PID lus dans le dernier log (la fiche suit la boucle PID, signale les écarts).

**Tranché par Cédric** le 2026-11-23, au début du lot 30d, entre deux propositions : une seule fiche (retenue) ou deux
fichiers gardés ; et la « Fiche cible » repoussée au lot 31. Amende D-036 et D-039 (le fichier des fiches) et D-061
(trois écarts au prototype).

**Contexte.** Deux « drones » cohabitaient : la fiche du Guide (`drones.json` : taille, version, boucle PID, ESC, cases
cochées), créée à la main, et le drone de la bibliothèque (lot 28 : nom, firmware, dates), créé à l'ouverture d'un log.
Le prototype n'a **qu'une** fiche, dans un écran hors rail (D-060), avec des champs que ni l'un ni l'autre n'avait
(batterie, poids, KV, pôles, hélices, carte de vol) et une « Fiche cible ».

**Décision.**
- **La bibliothèque tient la fiche** (`otb_store::model::DroneSheet`, champ `fiche` du drone ; `None` tant qu'elle n'a
  jamais été remplie — jamais de valeurs inventées, elles donneraient des conseils faux). Les valeurs sont gardées
  telles qu'affichées ; l'interface les traduit pour l'assistant (`rust/otb-gui/src/drone_sheet.rs:card_of`).
- **Le Guide n'a plus de fichier** : `guide::Cards` est une copie, relue quand la table des drones change
  (`Library::drones_revision`, `App::sync_cards`). Il ne change que ses cases ; `App` les écrit dans la bibliothèque.
  Sa fenêtre « Fiche drone » et son bouton « Supprimer » disparaissent : « Nouveau drone » et « Modifier » ouvrent l'écran.
- **L'écran « fiche du drone »** (`DroneSheetPage`, `Screen::DroneSheet`) : Identité (nom, identifiant et carte lus au
  branchement USB), Machine, Configurations Betaflight (« Voir » le texte, « Comparer » au précédent, réglage par
  réglage : `otb_store::model::config_changes`), suppression en deux temps. Ouvert par le crayon d'une carte ou
  « Nouveau drone » du tableau de bord, ou par le Guide ; il revient d'où l'on venait, le rail garde « Tableau de bord ».
- **Le nom relie un log à son drone** (`craft_name`, comme l'écran le dit) : un log rejoint le drone qui porte son nom
  (`Library::drone_by_name`), même créé à la main sous un autre identifiant. Deux drones ne partagent jamais un nom
  (refusé à l'enregistrement) ; un nouveau drone du nom d'un drone connu par ses seuls logs lui donne sa fiche.
- **Supprimer un drone** retire ses logs connus, leurs fragments (mesures et liste d'analyse comprises), ses
  configurations, et ferme ses logs ouverts. Les fichiers **copiés** restent sur le disque (W-075).
- **Reprise de l'ancien fichier** au lancement (`drone_sheet::import_old_file`) : fiches reprises (un drone qui a déjà la
  sienne la garde), **bibliothèque écrite, puis** fichier renommé `drones.json.repris` — dans cet ordre, sinon une coupure
  perdrait les fiches, ou un drone supprimé reviendrait au lancement suivant. Un fichier illisible n'est ni repris ni touché.
- **Écarts au prototype** (D-061) : pas de « Fiche cible » avant le lot 31, seul à s'en servir (sa colonne reste vide
  pour garder la largeur de la colonne de gauche) ; taille « 8″ et plus » en plus (l'assistant a ce profil) ; boucle PID à
  1,6 kHz en plus (la moitié d'un gyro BMI270, déjà proposée avant). Le texte blanc sur un bouton rouge plein du thème
  sombre devient sombre (`Theme::on_accent`) : 2,3:1 seulement, illisible ; même correction sur l'écran d'extraction.

**Écartés.** Garder deux fichiers que l'écran écrirait tous les deux (deux sources pour un même drone finissent par se
contredire) ; remplir la fiche d'un drone vu dans un log avec des valeurs par défaut (conseils faux sans que personne
ne les ait choisis) ; relier les logs par l'identifiant (un drone renommé perdait ses logs suivants) ; effacer l'ancien
fichier après reprise (on n'efface rien de l'utilisateur).

**Témoins.** `rust/otb-store/tests/bibliotheque.rs` (`la_fiche_du_drone_se_retrouve_et_un_drone_sans_fiche_le_reste`,
`supprimer_un_drone_retire_ses_logs_fragments_mesures_et_configurations`, `deux_configurations_se_comparent_reglage_par_reglage`),
`rust/otb-gui/tests/drone_sheet.rs` (8 tests), `rust/otb-gui/tests/guide.rs`
(`l_ancien_fichier_est_repris_dans_la_bibliotheque_puis_mis_de_cote`, `creer_modifier_recommencer_et_supprimer_une_fiche`) ;
6 contre-preuves vues en échec (L-071). Captures `fiche-drone-*.png` et `fiche-drone-bas-*.png` de l'exemple `capture` ;
0,2 ms par cadre.

**Ouvert.** La « Fiche cible » (lot 31) ; l'identifiant et la carte lus au branchement (lot 32b). L'avancement du parcours
sur la carte du tableau de bord : fait, D-071.

## 2026-11-23 — Second fragment d'une session

### D-067 : une session peut porter plusieurs fragments ; on les crée dans l'écran d'extraction

**Tranché par Cédric** le 2026-11-23, à la fin du lot 29, entre deux propositions pour chacune des quatre questions.
Amende D-061 (« chaque session ouverte devient un fragment ») : elle le reste, et peut en porter d'autres.

**Contexte.** Le prototype qui fait foi (D-061) n'a qu'un écran « Modifier le fragment » et ne montre aucun geste pour
découper un même vol en plusieurs morceaux ; la bibliothèque, elle, savait déjà en ranger plusieurs (lot 29a). Or
comparer deux passages d'un même log (une rafale de gaz, un stationnaire) est l'usage même d'un fragment.

**Décision.**
- **Le geste** : un bouton « Enregistrer comme nouveau fragment » dans la barre du haut de l'écran d'extraction, à
  côté de « Enregistrer le fragment ». Le fragment affiché ne change pas ; un autre est rangé dans la même session
  avec les bornes, le nom (rendu distinct par « (2) »…) et le rôle du brouillon. Écart au prototype, assumé.
- **Il rejoint la liste d'analyse** aussitôt : pastille juste après son origine, couleur à lui, devenu maître, sur
  les **mêmes échantillons** (pas de relecture du fichier).
- **Suppression** d'un fragment **ajouté** seulement, en deux temps (« Supprimer ce fragment… », puis « Supprimer
  définitivement » ou « Garder »), en bas de la section Fragment : il part avec ses mesures et sa place dans la liste.
  Le fragment de la session n'a pas ce bouton : il reviendrait à la prochaine ouverture du fichier.
- **« Ouvrir… »** ne met en analyse que le fragment de chaque session ; un fragment ajouté revient par son rond
  « + », son crayon ou la liste relue au lancement.
- **Identifiants** : `<empreinte>-<session>` pour le fragment de la session (inchangé : les bibliothèques existantes
  restent valables), `<empreinte>-<session>-<k>` (k ≥ 2, premier libre) pour un fragment ajouté
  (`rust/otb-store/src/link.rs`).

**Écartés.** Un bouton « Nouveau fragment » sur la fiche du drone (un écran de plus à traverser pour le même
résultat) ; un fragment ajouté seulement rangé, sans pastille (il faudrait aller le chercher au tableau de bord pour
le comparer) ; pas de suppression (une erreur encombrerait la fiche pour toujours) ; rouvrir tous les fragments d'un
fichier à son ouverture (la barre se remplirait d'un coup, 10 pastilles au plus).

**Témoins.** `rust/otb-store/tests/fragments.rs`
(`un_fragment_ajoute_prend_un_numero_libre_et_se_distingue_de_celui_de_la_session`,
`supprimer_un_fragment_emporte_ses_mesures_et_sa_place_dans_la_liste`), `rust/otb-gui/tests/analysis_list.rs`
(`enregistrer_comme_nouveau_fragment_garde_l_origine_et_ajoute_une_pastille`,
`un_fragment_ajoute_se_rouvre_par_son_rond_son_crayon_et_au_lancement`,
`seul_un_fragment_ajoute_se_supprime_apres_confirmation`) ; 8 contre-preuves vues en échec (L-071). Captures
`fragment-ajoute-clair.png` et `-sombre.png` de l'exemple `capture`.

**Ouvert.** Le Guide retenait le rôle d'un log **par fichier** (D-039) : deux fragments d'une même session aux rôles
différents n'en faisaient voir qu'un au Guide (W-074). Fait : le rôle est lu sur le fragment, D-071.

## 2026-11-23 — Liste d'analyse à l'écran

### D-066 : la barre de fragments décide de ce que montrent toutes les pages

**Tranché par Cédric** le 2026-11-23, au début du lot 29c : « la barre remplace » les listes de chaque page (plutôt que
de les faire coexister) ; la liste est **rouverte au lancement** ; « Ouvrir… » et l'export **restent** dans la barre du
haut. Remplace D-049.

**Contexte.** Le prototype qui fait foi (D-061) porte en haut de tous les écrans une barre de pastilles, une par fragment
en analyse : rond de couleur, nom, drone, « × » ; clic pour désactiver, clic sur le rond pour le fragment maître, glisser
pour réordonner. La bibliothèque savait déjà tenir cette liste (lot 29a), l'interface ne s'en servait pas : chaque page
avait sa liste « Fichiers » à cocher, et la Réponse indicielle sa propre règle d'ordre et de couleurs (D-049).

**Décision.**
- Les **logs ouverts sont la liste d'analyse** : leur ordre est celui des courbes, chacun peut être **désactivé** sans
  être fermé (`LoadedLog::active`). Une page à plusieurs vols montre les actifs dans l'ordre de la barre ; une page à un
  seul vol montre le **fragment maître** — le log courant s'il est actif, le premier actif sinon (`AppState::master`).
  Tout désactiver laisse les pages vides.
- **Plus de liste « Fichiers » dans les pages** (`tools/common.rs:FileSelection` lit la barre).
- **Couleur fixe par fragment**, reçue à l'ouverture, la même sur toutes les pages ; glisser change l'ordre, jamais la
  couleur. Ce que D-049 demandait — l'ordre choisi par l'utilisateur, une page vide possible — passe par la barre, pour
  toutes les pages ; l'inconvénient qu'elle assumait (une couleur par page) disparaît.
- La liste est **recopiée dans la bibliothèque** (ordre, indice de couleur, actif, maître) et **rouverte au lancement**
  en arrière-plan ; un fragment dont le fichier a disparu est signalé dans la barre d'état, pas ouvert. Un log rangé
  sous aucun drone (sans nom de machine) n'a pas de fragment et n'est pas gardé.
- Le **« + » d'un fragment** au tableau de bord l'ajoute à la liste en n'ouvrant **que sa session**, ou l'en retire ; il
  prend la couleur de sa pastille.
- **Écarts au prototype, assumés** (D-061) : « Ouvrir… » et l'export restent dans la barre du haut (la maquette n'ouvre
  qu'au tableau de bord et n'exporte pas) ; les boutons « Fermer » et « Tout fermer » disparaissent (le « × » d'une
  pastille) ; la fiche du fragment s'ouvre en **infobulle** au survol, au lieu de déplier la pastille ; pendant un glisser,
  une **barre d'insertion** montre où la pastille tombera, sans pastille fantôme sous le pointeur.
- Option écartée : garder les listes des pages en plus de la barre. Deux endroits pour le même geste, et une courbe qui
  pouvait changer de couleur d'une page à l'autre.

**Témoins.** `rust/otb-gui/tests/analysis_list.rs` (gestes de la barre, « + » qui n'ouvre que sa session, liste retrouvée
au lancement, fragment introuvable, doublon, place d'une pastille lâchée), `rust/otb-gui/tests/tools_common.rs`
(`la_selection_d_une_page_suit_la_liste_d_analyse`), `rust/otb-gui/tests/freq_throttle.rs`
(`une_colonne_garde_son_log_quand_l_ordre_de_la_barre_change`) ; 9 contre-preuves vues en échec, une par protection
nouvelle (gestes, filtre des sessions, relecture, doublon, pages, recalcul, colonnes ; L-071).
Captures `barre-desactivee-clair.png` et `-sombre.png` de l'exemple `capture`.

**Ouvert.** Créer un second fragment dans une même session : fait, D-067.

## 2026-11-23 — Écran d'extraction d'un fragment

### D-065 : le visualiseur devient l'écran d'extraction ; les bornes du fragment sont la période d'analyse

**Tranché par Cédric** le 2026-11-23, au début du lot 29b, entre deux propositions pour chaque question.

**Contexte.** Le prototype qui fait foi (D-061) n'a pas d'entrée « Courbes » dans son rail (W-069), et son écran
d'extraction d'un fragment reprend tout le visualiseur : mêmes graphiques, traces, lecteur, mesure, choix d'axe, avec
les deux bornes du fragment et une barre de navigation en plus. Par ailleurs, chaque log ouvert avait une **période
d'analyse** (bords bleus des courbes, lue par tous les outils) et la bibliothèque rangeait à part un fragment par
session, couvrant le vol entier : deux notions pour la même chose.

**Décision.**
- **Le visualiseur est l'écran d'extraction**, hors rail (D-060) : sa propre barre du haut (retour, « Modifier le
  fragment … », Annuler, Enregistrer le fragment), pas de rail ni de barre des logs. Il s'ouvre en ouvrant un log, par
  le crayon d'un fragment au tableau de bord et par le Guide, et revient à l'écran d'où l'on venait. « Courbes » quitte
  le rail, qui a les sept entrées du prototype dans son ordre ; « Configuration » y gagne son entrée.
- **Les bornes du fragment sont la période d'analyse.** Elles vivent dans un brouillon tant qu'on les règle (rien
  n'atteint les outils) ; « Enregistrer le fragment » en fait la période du log et les écrit dans la bibliothèque comme
  **choisies** ; rouvrir le fichier les retrouve. « Annuler » ne change rien.
- **Un fragment non choisi suit les limites automatiques** (décollage → atterrissage, crash exclu) à chaque ouverture —
  y compris ceux rangés avant ce lot, sur le vol entier : personne n'avait pu les choisir. Le champ `bornes_choisies`
  s'ajoute au fichier sans changer son format (absent = faux).
- Le Guide peut encore resserrer la période le temps d'une analyse (partie d'un vol) sans toucher au fragment.

**Écartés.** Deux écrans de courbes presque identiques (un dans le rail, un hors rail) : deux écrans à entretenir.
Garder « Courbes » dans le rail avec les poignées : écart au prototype. Fragment et période séparés : deux bornes à
tenir pour le même morceau de vol.

**Témoins.** `rust/otb-gui/tests/viewer_page.rs` (`les_bornes_se_tirent_depuis_n_importe_quel_graphique_sans_rien_appliquer`,
`enregistrer_applique_les_bornes_et_annuler_ne_change_rien`, `la_barre_de_navigation_deplace_la_fenetre_visible`,
`les_bornes_se_posent_sur_un_echantillon_et_gardent_leur_ecart`), `rust/otb-gui/tests/app.rs`
(`un_fragment_range_d_office_suit_les_limites_automatiques`, `des_bornes_choisies_survivent_a_la_reouverture_et_donnent_la_periode`,
`le_crayon_d_un_fragment_ouvre_l_ecran_d_extraction`, `le_rail_porte_les_entrees_de_navigation`), `rust/otb-store/tests/fragments.rs`
(`des_bornes_choisies_se_relisent_et_un_ancien_fragment_ne_lest_pas`) ; chacun vu en échec, protection retirée (L-071).

## 2026-11-22 — Bibliothèque et récupération par USB

### D-064 : la bibliothèque reste en fichiers JSON, table des mesures comprise

**Contexte.** Le lot 28 a choisi des fichiers JSON (`otb-store`) et renvoyé au lot 31 la question d'une vraie base :
au volume plein du corpus, la table des mesures pèse 8 Mo et met 959 ms à se lire ([validation.md](validation.md)).
Question posée à Cédric le 2026-11-22.

**Décision.**
- Les six tables restent des **fichiers JSON**, lus et écrits par le seul module d'accès d'`otb-store`.
- La table des mesures se lit **à la première question posée**, **hors du fil de l'interface** : l'écran ne gèle pas
  pendant la seconde de lecture.
- Pas SQLite : une dépendance de plus et un format qu'on ne relit plus d'un coup d'œil, pour un volume qu'un
  chargement à la demande suffit à absorber.

**Témoins.** Mesure du lot 28 (8 Mo, 959 ms, 535 logs, 77 040 mesures) ; chargement à la demande depuis le lot 31a
(`Library::measures_source`, `Library::attach_measures`), prouvé par
`rust/otb-store/tests/bibliotheque.rs:la_table_des_mesures_se_lit_a_la_demande_et_nest_jamais_videe_avant` : lecture de 308 ms
(`--release`) au volume plein, hors du fil de l'écran ([validation.md](validation.md#mesures-suivies-dans-le-temps-2026-11-23-lot-31a)).

**Ouvert.** À reprendre seulement si une mesure du lot 31 montre un temps de lecture qui gêne malgré le chargement en
arrière-plan.

### D-063 : la récupération par USB lit et efface, elle n'écrit jamais de réglage sur la carte

**Contexte.** Vérification du firmware avant le lot 32 ([spec-usb.md](spec-usb.md)) : Betaflight n'empêche ni la
lecture ni l'effacement de la mémoire quand le drone est armé, et la bibliothèque série Rust la plus connue,
`serialport`, est sous MPL-2.0. Cédric a tranché les trois points le 2026-11-22.

**Décision.**
- OpenTuneBox **lit** l'identité, les logs et la configuration (`diff all`), **vérifie** la copie, puis **efface**
  après confirmation. Il n'écrit **jamais** un réglage sur la carte : les réglages « Et si » et les conseils de
  l'assistant restent des **lignes CLI à copier** — pas d'écriture directe, parce qu'elle change la façon dont le
  drone vole sans que le pilote ait relu la commande.
- L'**état armé se relit juste avant l'effacement**, pas seulement au branchement : le firmware ne s'en charge pas.
- Le port série passe par **`serial2`** (BSD-2-Clause OR Apache-2.0) ; pas `serialport` : MPL-2.0, copyleft
  refusé par `rust/deny.toml` (D-043, D-045).
- La partie **protocole** (32a : trames MSP, port simulé, contrôles avant effacement) peut se faire **avant la fin du
  lot 30** : elle ne touche pas l'interface.

**Témoins.** [spec-usb.md](spec-usb.md) (points 3, 4, 6 et 8, lus dans Betaflight au commit `1b94a3871`) ;
`cargo info serialport` (MPL-2.0) et `cargo info serial2` (BSD-2-Clause OR Apache-2.0) le 2026-11-22.

## 2026-11-18 — Scripts firmware

### D-062 : un script par firmware et par plage de versions — chaîne décrite en données, briques compilées en Rust

Numérotée D-057 dans le fork `GerryNatif/OpenTuneBox` (sa demande de fusion n° 2), écrite le 2026-11-18 **avant** le lot 22
de ce dépôt, renumérotée à la reprise le 2026-11-22. **Confirmée par Cédric** le même jour : les quatre arbitrages du
2026-11-18 sont bien les siens. Il en a ajouté trois le 2026-11-22, après avoir relu la décision à la lumière du lot 22
(« Reprise du 2026-11-22 » ci-dessous).

**Contexte.** L'étude du 2026-11-17 (document de travail de Cédric, hors dépôt) propose d'étendre OpenTuneBox à
INAV et à ArduCopter au moyen d'un « script » par firmware et par version : le script décrit la chaîne de filtres
et de PID, et fournit les briques de calcul avec l'arithmétique exacte de sa famille ; le cœur assemble et exécute.
Elle laisse sept questions ouvertes. Trois avaient déjà leur réponse dans les registres (la licence, D-045 ; le
hors ligne, D-003 ; l'ordre de travail, Betaflight étant le seul firmware du corpus) ; Cédric a tranché les quatre
autres le 2026-11-18, une par une.

**Décision.**
- **Forme d'un script.** La chaîne est décrite dans un **fichier de données versionné** du dépôt : étapes
  ordonnées (brique, cadence, paramètres, en service ou non), **prises** des signaux enregistrés par le log, et
  position de la **dérivation** du terme D ([spec-betaflight.md](spec-betaflight.md) §8). Les **briques de calcul sont
  du Rust compilé** dans le programme, rangées par famille de firmware. **Pas** de moteur de script embarqué (Rhai, Lua)
  ni de WASM : ils n'achètent qu'un rechargement à chaud, au prix de la vitesse sur des millions d'échantillons, d'une
  surface de chargement de code exécutable (D-054) et d'une dépendance de plus. Conséquence assumée : **ajouter un
  firmware demande de recompiler et de publier une version.**
- **Rien du code des firmwares n'entre dans le dépôt.** Les briques sont réimplémentées d'après la lecture du code
  (D-004), jamais copiées. Betaflight, INAV et ArduPilot sont sous GPL 3, que D-045 refuse : aucun fichier de
  firmware versionné, et rien de compilé depuis leur code dans ce qui est livré. L'option de l'étude « compiler
  directement le code du firmware » (en WASM) est **écartée pour cette raison**, pas pour une raison technique.
- **Banc de référence dans le dépôt.** Un script **écrit par le projet** télécharge le firmware au moment de
  s'exécuter, le compile sur le poste, et ne verse dans le dépôt que des **vecteurs de nombres**. La preuve reste
  refaisable par quiconque, la CI comprise, sans redistribuer de code sous GPL. Option écartée : un banc hors
  dépôt — les vecteurs seraient là sans que personne puisse les refaire.
- **Bascule d'un coup, pas de cohabitation.** La chaîne float32 à l'arithmétique du firmware **remplace** la chaîne
  `f64` actuelle dans un seul lot, qui réenregistre les références figées (W-028) et revérifie les seuils de
  l'assistant (D-011). Option écartée : garder les deux chaînes le temps de prouver la nouvelle. Le risque — un
  seuil qui change l'**avis** de l'assistant sur un vrai log — est **accepté par Cédric** et borné par l'ordre des
  étapes du lot 24 de [plan.md](plan.md) : l'écart est chiffré sur tout le corpus **avant** que l'interface ne
  change, et un avis qui bascule arrête le lot. Depuis le lot 22, la chaîne remplacée porte aussi le rejeu de la page
  Filtres (D-058) et le bruit prévu par l'assistant (D-059).
- **Périmètre : Betaflight seul.** W-001 reste fermé (INAV, logs KISS) et ArduCopter est consigné en W-071. Raison :
  le corpus ne contient aucun log de ces firmwares, et le projet valide sur de vrais logs en plus des tests
  synthétiques. Reprise : des logs réels, à la demande de Cédric.
- **Trois niveaux de fidélité, jamais confondus** dans les textes affichés : *même algorithme* (même ordre, mêmes
  formules de coefficients, mêmes approximations) est l'engagement du logiciel ; *bit à bit contre le firmware
  compilé sur PC* est le critère de test de chaque script ; *identité au vol réel* est **mesurée et affichée,
  jamais promise**, car le log ne contient pas toujours l'entrée des filtres à pleine cadence.

**Reprise du 2026-11-22** (Cédric, après le lot 22, D-057 à D-059) :
- **Chantier conditionnel.** OpenTuneBox lira **peut-être un jour** d'autres firmwares que Betaflight : les lots 23 à 25
  restent au plan, **en attente de vrais logs INAV ou ArduCopter** et de la demande de Cédric. Raison : pour un pilote
  Betaflight, le lot 22 a déjà vérifié le simulateur contre ses vols (rejeu à 0,1–0,2 dB du log, étages sous 0,5 dB
  jusqu'à 500 Hz, [validation.md](validation.md)), et le calcul au bit près ne réduirait pas cet écart : 0,1 dB, c'est
  environ 1 % d'amplitude, quand l'arrondi du float32 pèse 1e-7. L'écart restant vient de ce que le log n'enregistre
  pas (arrondis, L-103), pas de l'arithmétique.
- **Preuve du lot 24 : le banc et les vrais vols.** Le banc au bit près vérifie le **calcul** ; le contrôle par étage
  du lot 22 (`otb-dev corpus stages`, `corpus replay`) vérifie le **drone**, et **ne doit pas se dégrader** à la
  bascule. Les erreurs trouvées au lot 22 le montrent : la cadence du LPF2 (L-104), un banc l'aurait vue aussi ; le
  contrôleur surchargé (L-101), l'arrondi du log (L-103) et le TPA du D, seuls de vrais vols les ont montrés.
- **L'ordre du terme D n'attend pas le lot 23** : corrigé à part, mesuré sur le corpus (lot 33, W-072), parce qu'il est
  maintenant visible dans la page Filtres et l'assistant.

**Amendement du 2026-11-23** (Cédric, en lançant le lot 23) : la condition des vrais logs INAV ou ArduCopter est **levée
pour le lot 23 seul**, parce qu'il ne change aucun résultat et qu'il prépare le terrain. Les lots 24 et 25 restent
conditionnels. Fait le même jour : la chaîne Betaflight est décrite une seule fois, dans
`rust/otb-analysis/chains/betaflight.json` (intégré à la compilation, lu par `rust/otb-analysis/src/chain.rs`) ; champs en
anglais (D-001), forme en [spec-betaflight.md](spec-betaflight.md) §8. Preuve d'iso-résultat dans
[validation.md](validation.md#chaîne-décrite-en-données-2026-11-23-lot-23).

**Amendement du 2026-11-24** (Cédric, en lançant le lot 24) : la condition est **levée pour les étapes 24a à 24c seules**
— les briques à côté des actuelles, le banc et ses vecteurs, l'écart chiffré sur tout le corpus —, parce qu'elles ne changent
rien à l'écran et qu'on peut tout annuler à ce stade. La **bascule** (24d, 24e) et le lot 25 restent conditionnels : Cédric
décidera au vu des chiffres de 24c. Découpage choisi par lui le même jour : 24a et 24b dans une même demande de fusion (des
briques ne se prouvent pas sans le banc), 24c dans la suivante. Fait le même jour (24a, 24b) : quatre arithmétiques
(4.3–4.4, 4.5, 2025.12, 2026.6 et après) dans `rust/otb-dsp/src/firmware/betaflight/`, banc `otb-dev firmware-bench` sur six
versions, toutes identiques au bit près ([validation.md](validation.md#briques-à-larithmétique-du-firmware-2026-11-24-lots-24a-et-24b)).
Le suivi des pics du notch dynamique et l'ordonnancement des recalculs (notchs RPM, LPF dynamiques) restent hors du banc
(W-081). Fait le même jour (24c) : la chaîne assemblée `rust/otb-analysis/src/firmware_chain.rs`, à côté de l'actuelle et choisie
par `Engine` (`Engine::Model` par défaut : rien ne change à l'écran), recopie l'ordonnancement version par version ; écart chiffré
sur tout le corpus, critère d'arrêt non atteint ([validation.md](validation.md#chaîne-assemblée-à-larithmétique-du-firmware-2026-11-24-lot-24c),
W-082, W-083). Au vu des chiffres, Cédric a demandé le même jour de corriger **dans la chaîne en service**, avant la demande de
fusion, les deux écarts expliqués : notch RPM plaqué au plafond et gardé (W-082), ordre des notchs RPM du firmware (L-120). Le
rejeu, l'assistant et le contrôle par étage changent donc dès ce lot ; `simulate_gyro` garde les règles de l'étalon (références
figées), et aucun avis de l'assistant ne change sur le corpus.

**Amendement du 2026-11-24, bascule** (Cédric, au vu des chiffres de 24c et de la cause de l'écart du D, D-076) : la condition est
**levée pour 24d et 24e**, livrés dans une même demande de fusion (choix de Cédric). La page Filtres, l'assistant et le contrôle par
étage rejouent par `rust/otb-analysis/src/firmware_chain.rs` dès la 4.3 (`Engine::in_service`). **Cohabitation limitée, choisie par
Cédric** : l'ancienne chaîne (`bf_filters`, `Engine::Model`) reste un **outil de mesure** (`otb-dev corpus … --chain model`, étalon
des références Python) et sert encore aux logs **d'avant la 4.3**, que la chaîne du firmware refuse (arithmétique non vérifiée ;
aucun au corpus) : pour eux, rien ne change. La réponse en fréquence (repli sans gyro non filtré, retards, boucle ouverte) reste
celle de `gyro_chain` / `dterm_chain` : même réponse, l'arithmétique n'y joue pas. **Références figées non réenregistrées**
(choix de Cédric) : `$OTB_GOLDEN` ne porte que les fonctions de l'étalon, que la bascule ne touche pas (`otb-parity check` :
0 échec) ; W-028 et W-072 restent ouverts. **Un avis a basculé** (critère d'arrêt de ce lot) : « Le LPF1 du gyro peut être
coupé » sur `1. PTB/Kris/13. Hover.bbl`, gyro prévu à 0,1 dB du seuil sur un log enregistré une boucle sur 4 ; expliqué et
**accepté par Cédric** le même jour. Seuils (24e) : la calibration ne passe pas par le rejeu, mesures identiques au bit près,
aucun seuil changé. Chiffres : [validation.md](validation.md#bascule-sur-la-chaîne-du-firmware-2026-11-24-lots-24d-et-24e).
Le lot 25 reste conditionnel.

**Témoins.** Code relu le 2026-11-22 (le fork le décrivait avant le lot 22). La chaîne est écrite **deux fois** :
- en **réponse en fréquence**, par `rust/otb-analysis/src/bf_filters.rs:gyro_chain` et
  `rust/otb-analysis/src/bf_filters.rs:dterm_chain` : rapport des |H|² de `whatif_spectrum` (repli de la page Filtres
  pour un log sans gyro non filtré, `rust/otb-gui/src/tools/filter_sim_calc.rs` ; repli de l'assistant,
  `rust/otb-advisor/src/rules.rs`), retard gagné par un conseil, boucle ouverte (`rust/otb-analysis/src/open_loop.rs`) ;
- **dans le temps**, par les étages `rust/otb-analysis/src/bf_filters.rs:GyroStage`, communs à `simulate_gyro`, au
  contrôle par étage (`rust/otb-analysis/src/filter_stages.rs`) et au rejeu (`rust/otb-analysis/src/filter_replay.rs`).
  Le rejeu applique chaque filtre à sa cadence (L-104) ; `simulate_gyro` garde la cadence du log, mais n'est appelé que
  par les tests et `otb-parity`. Le terme D du rejeu passe par `rust/otb-analysis/src/bf_filters.rs:simulate_dterm_at`, qui
  filtre puis dérive comme le firmware (`flight/pid.c`) depuis le lot 33 ; `simulate_dterm`, réservé aux références Python,
  dérive encore avant de filtrer (W-072).
- **Depuis le lot 23** (2026-11-23), les deux lisent la même description : `gyro_chain` et `dterm_chain` la parcourent, un
  étage `GyroStage` est l'intervalle entre deux prises du log, la cadence de chaque étage et la position de la dérivée du D
  y sont lues. `simulate_gyro` (cadence du log) et `simulate_dterm` (dérivée d'abord) gardent leurs approximations pour les
  références Python, mais prennent l'ordre des étapes dans la description.

Refus du copyleft : liste `allow` de `rust/deny.toml`, onze licences de code, aucune copyleft, plus les deux licences de
polices de D-046. Licence GPL 3 de Betaflight **vérifiée** le 2026-11-22 (fichier `LICENSE` du dépôt du firmware) ;
celles d'INAV et d'ArduPilot restent lues dans l'étude, à confirmer au lot qui monte le banc (lot 24).

**Ouvert.** W-001 (INAV, KISS), W-071 (ArduCopter), W-072 (simulation dans le temps), lots 23 à 25 et 33 de
[plan.md](plan.md).

## 2026-11-19 — Fidélité au prototype

### D-061 : le dessin de l'interface suit le prototype au détail près

**AMENDÉE par D-077** (2026-11-25) : section Machine de la fiche du drone (taille libre, taille des moteurs, notes tirées du log).

Numérotée D-059 dans le fork `GerryNatif/OpenTuneBox` où elle a été écrite, renumérotée à la reprise dans ce dépôt le 2026-11-22. **Confirmée par Cédric** le même jour : les arbitrages ci-dessous ont été discutés avec lui pendant l'écriture
du fork.

Complète D-060, qui ne fixait que la **structure** de la navigation. **AMENDÉE par D-068** (2026-11-23) : trois écarts de
plus, sur la fiche du drone (voir D-068). **AMENDÉE par D-067** (2026-11-23) : chaque
session ouverte devient toujours un fragment, et peut en porter d'autres.

**Contexte.** Le 2026-11-19, après la livraison du rail, de l'espace « Bruit et filtres » et du tableau de bord,
Cédric reprend : « le design du logiciel doit correspondre à 100 % à la maquette ». Les trois lots avaient repris
la **structure** du prototype (entrées, regroupements, écrans) mais pas son **dessin** : cartes au cadre egui
ordinaire, pas de pastille d'état, pas de barre d'avancement, entrée de rail au style egui plutôt qu'au fond bleu
pâle de la maquette.

**Décision.**
- Le **prototype complet de l'application** (canevas du 2026-11-16) fait foi pour le dessin comme pour la
  structure : **mesures** (tailles de texte, coins, marges, hauteurs), **couleurs** et **contenu** des écrans s'y
  relèvent, et le code les porte en constantes nommées, groupées en tête du fichier de l'écran.
- **Thème clair identique à la maquette** ; la maquette n'existe qu'en clair, donc le **thème sombre garde ses
  formes et ses mesures** et prend les couleurs sombres de `rust/otb-gui/src/theme.rs`. ⚠️ Toute couleur passe par
  le thème, jamais en dur (L-016) : la palette de la maquette s'y trouvait déjà presque entière — le bleu
  `highlight` `#1f57c3`, `muted` `#5c5c5c`, `mid` `#c4c4c4`, `hover` `#e4e7ec`, les accents vert et orange — et
  seuls `select` (`#e8eefa`, fond d'une entrée choisie) et `border` (`#e1e3e8`) ont été ajoutés.
- **Un écart au dessin se justifie par écrit**, dans le code et dans le plan, sinon il se corrige.
- Trois écarts sont ainsi assumés, tranchés par Cédric le même jour : le **thème sombre** est dérivé ; chaque
  **session ouverte devient un fragment** couvrant le vol entier (sinon le compteur « Fragments » de la carte
  afficherait 0 partout jusqu'au lot 29b — depuis D-065, il couvre les limites automatiques tant qu'on n'a pas choisi
  ses bornes) ; et l'**avancement** d'une carte ne compte que les cases cochées à la
  main, les cases vérifiées par un log demandant une analyse par log. **AMENDÉ par D-071** (2026-11-23) : elle compte
  aussi les cases que le Guide a vues vérifiées à son dernier passage, rangées sur la fiche.
- Option écartée : « assez proche ». Une maquette qui fait foi ne se respecte pas à moitié — c'est elle qui tient
  l'unité de l'interface quand les écrans s'ajoutent un par un.

**Témoins.** `rust/otb-gui/src/dashboard.rs` (mesures de la maquette en constantes nommées en tête de fichier) et
`rust/otb-gui/src/app.rs:rail_entry` ; les deux couleurs ajoutées dans `rust/otb-gui/src/theme.rs` ;
`rust/otb-gui/tests/app.rs` (`le_tableau_de_bord_montre_une_carte_par_drone_et_ses_fragments`, contre-preuve vue
en échec) ; captures hors écran du tableau de bord dans les deux thèmes, comparées à la maquette.

**Maquette.** W-070, clos le 2026-11-22 : la maquette est retrouvée dans le canevas claude.ai privé de Cédric (planche « Prototype complet — toute l'application », `Application.dc.html`) ; copiée le même jour dans [maquettes/](maquettes/README.md).

## 2026-11-19 — Navigation de la fenêtre principale

### D-060 : une entrée de navigation est un outil ou un espace ; la barre devient un rail à gauche

Numérotée D-058 dans le fork `GerryNatif/OpenTuneBox` où elle a été écrite, renumérotée à la reprise dans ce dépôt le 2026-11-22. **Confirmée par Cédric** le même jour : les arbitrages ci-dessous ont été discutés avec lui pendant l'écriture
du fork.

Amende D-015 (« un outil = un onglet de la fenêtre principale ») : ce qui comptait y reste en vigueur, seule la
forme change.

**Contexte.** Le canevas de maquettes du 2026-11-16 (document de travail de Cédric, hors dépôt) propose un **rail de
navigation à gauche** de sept entrées — Tableau de bord, Guide, Assistant, Bruit et filtres, Réponse indicielle,
Curseurs PID, Configuration — et deux écrans **hors rail** : l'extraction d'un fragment et la fiche d'un drone.
L'entrée « Bruit et filtres » réunit quatre outils d'aujourd'hui : l'Analyse spectrale, la Fréquence × gaz, la
Fréquence × temps et le Simulateur de filtres. Le 2026-11-18, Cédric a désigné le **prototype complet de
l'application** comme la maquette qui fait foi et a demandé de trancher le sort de D-015.

**Ce que le code fait déjà.** Le regroupement n'est pas une nouveauté : `rust/otb-gui/src/tools/freq_throttle.rs` et
`rust/otb-gui/src/tools/freq_time.rs` sont des pages **cachées** (`HIDDEN`) dont le bouton mis en valeur est celui de
l'Analyse spectrale (`TAB`), et `rust/otb-gui/src/tools/setup.rs` est cachée sous « Courbes ». La lettre de D-015 —
une entrée par outil — était donc déjà contournée par le mécanisme même du trait de page. Cette décision la met en
accord avec le code et avec les maquettes.

**Décision.**
- **Ce qui ne change pas, et qui était l'objet de D-015** : tout s'affiche dans la **fenêtre principale**, jamais
  dans une fenêtre séparée ; et **une vue cachée ne calcule pas**. Cette seconde règle compte désormais davantage :
  un espace n'affiche qu'une vue sur quatre, donc trois calculs sur quatre ne partent pas.
- Une **entrée de navigation** est soit un outil, soit un **espace** qui regroupe plusieurs vues d'un même sujet.
  Le mécanisme reste celui du trait de page (`rust/otb-gui/src/tool.rs`) : une vue d'espace est une page `HIDDEN`
  dont `TAB` nomme l'entrée. `rust/otb-gui/src/tools/mod.rs:all_pages` reste **le seul endroit** où une page
  s'enregistre.
- Les boutons de navigation passent de la barre du haut à un **rail vertical à gauche**. La barre du haut garde ce
  qui agit sur les logs et sur la fenêtre : ouvrir, fermer, thème, export.
- **Deux écrans hors rail** sont permis, ouverts par un geste puis refermés : l'extraction d'un fragment et la
  fiche d'un drone. Hors rail ne veut pas dire hors fenêtre : D-015 tient.
- Option écartée : garder une entrée par outil et retoucher les maquettes. Le prototype fait foi (arbitrage du
  2026-11-18), et neuf entrées de rail pour sept sujets n'aident personne.

**Témoins.** Rail et entrées : `rust/otb-gui/tests/app.rs` (`le_rail_porte_les_entrees_de_navigation`,
`cliquer_une_entree_du_rail_change_decran`) ; le contrat des icônes reste tenu par
`rust/otb-gui/tests/contracts.rs` (`chaque_onglet_de_la_barre_a_son_icone_et_sa_couleur_comme_python`) ; captures
hors écran dans les deux thèmes, regardées avant et après. Le regroupement déjà en place est lisible dans les
constantes `HIDDEN` et `TAB` des trois pages citées plus haut.

**Ouvert.** W-069 : le prototype n'a **pas** d'entrée « Courbes » dans son rail, alors que le visualiseur est la page
la plus utilisée aujourd'hui ; où vont les traces reste à trancher. **Tranché le 2026-11-23 par D-065** : le
visualiseur devient l'écran d'extraction, hors rail.

## 2026-11-22 — Simulateur de filtres rejoué

### D-059 : l'assistant prévoit le bruit après un conseil de filtres par le rejeu de la page Filtres

**Tranché par Cédric** le 2026-11-22, après la vérification en vol du lot 22f : « le plus proche de la réalité ».

**Contexte.** Depuis D-058, la page Filtres rejoue la chaîne du firmware ; l'assistant (`rules.rs:predict_filter_change`,
conseils « couper le LPF1 du gyro » et « relever les filtres du D ») gardait le rapport des réponses |H|² de `whatif_spectrum`
(W-068). Le vol de l'Air65 avec un notch dynamique de plus a tranché : entre 100 et 250 Hz, le rejeu se trompe de 0,3 dB en
médiane, le rapport de 2,0 dB, et celui-ci change selon le mode debug du vol
([validation.md](validation.md#prédiction-vérifiée-en-vol-2026-11-22-lot-22f)).

**Décision.**
- Le bruit prévu (variation du niveau moyen et du sommet entre 80 Hz et la borne haute, gyro et D, roulis et tangage) vient de
  `filter_replay::before_after` : spectre « modifié » contre le spectre enregistré, comme la page Filtres. Les retards gagnés
  restent calculés sur les chaînes de filtres au gaz médian.
- Sans gyro non filtré : rapport des réponses |H|² comme avant (D-008, D-041), seul possible.
- Le rejeu des réglages du log se calcule une fois par analyse (`rules.rs:Predictor`) ; quand seuls les filtres du D changent,
  le gyro rejoué est repris (`filter_replay::replay_from`, identique au bit près au rejeu complet).
- Le rapport reste disponible (`FilterModel::Ratio`) pour comparer aux références Python (tests, `otb-parity`), qui ne
  connaissent que lui ; tout le reste des textes s'y compare toujours au caractère près.

**Écartés.** Garder le rapport (moins juste en vol, dépend du mode debug) ; rejeu temporel pur sans rapport au log (D-058).

**Témoins.** Tests `rust/otb-advisor/tests/predict_replay.rs` (sans changement : variation nulle ; sans gyro non filtré :
le rapport), `rust/otb-advisor/tests/rules_delay.rs` (`analyze` ne rend plus le texte du rapport),
`rust/otb-analysis/tests/filter_replay.rs` (`replay_from` = rejeu complet) ; corpus : un conseil en moins sur 676 sessions,
162 chiffres changés ([validation.md](validation.md#assistant-passé-au-rejeu-2026-11-22-d-059)).

### D-058 : la page Filtres rejoue la chaîne de filtres du firmware ; « modifié » = spectre du log × rejeu modifié / rejeu actuel

**Tranché par Cédric** le 2026-11-22 (lot 22e de `docs/plan.md`), parmi trois propositions : garder le rapport des réponses
|H|² en y branchant le suivi du notch dynamique, tout passer en rejeu temporel pur, ou rejeu + rapport (retenu).

**Contexte.** Avant le lot 22, la courbe « simulé actuel » de la page Filtres recopiait le log (rapport de deux réponses
identiques) et ne prouvait rien ; « simulé modifié » multipliait le spectre enregistré par |H_modifié|² / |H_actuel|², notchs
posés à leur position moyenne par fenêtre (W-059) et, sans debug `FFT_FREQ`, notchs dynamiques à leurs centres initiaux
(W-057). Le lot 22 a vérifié le modèle étage par étage (0,5 dB jusqu'à 500 Hz, `validation.md`) et rejoué le suivi des pics
du notch dynamique (22c).

**Décision.**
- `filter_replay::replay` rejoue la chaîne du firmware sur le gyro non filtré, avec les règles du contrôle par étage (cadence
  nominale, filtres à leur cadence, TPA du D) et le suivi du notch dynamique rejoué sur chaque axe **avec les réglages
  simulés** (spec-methodes.md, « Simulateur de filtres »).
- « Simulé actuel » = ce rejeu avec les réglages du log, arrondi comme le Blackbox : il juge le modèle sur chaque vol.
- « Simulé modifié » = spectre enregistré × (rejeu modifié / rejeu actuel), case par case et fenêtre par fenêtre, rapport tiré
  des rejeux exacts (L-105). Amende D-008 : la méthode reste relative (exacte quand rien ne change), mais le rapport vient du
  rejeu et non plus des réponses |H|².
- Remplace D-041 pour la page Filtres : les notchs dynamiques ajoutés suivent les pics comme en vol, sans pics prévus ni
  centres initiaux. D-041 reste en vigueur pour `bf_filters::whatif_spectrum` (assistant, logs sans gyro non filtré).
- Sans gyro non filtré : rapport des réponses |H|² comme avant, sans courbe « actuel ».
- Pas de rejeu temporel pur pour « modifié » : l'erreur du modèle (repliement des logs enregistrés une boucle sur deux,
  W-067) s'y verrait même sans rien changer.
- L'assistant garde `whatif_spectrum` jusqu'à la vérification en vol de 22f (W-068). ✏️ AMENDÉ par D-059 : il rejoue lui aussi.

**Témoins.** Tests `rust/otb-analysis/tests/filter_replay.rs` (réglages inchangés = log au bit près ; LPF1 faussé vu par la
courbe « actuel » ; filtrer plus ne prévoit jamais plus de bruit ; notchs ajoutés sans creux aux centres initiaux, contre −6 dB
et plus pour l'ancienne méthode) ; `otb-dev corpus replay` sur 675 sessions : rejeu « actuel » à 0,1–0,2 dB du log en médiane
sur les logs enregistrés à chaque boucle PID ([validation.md](validation.md#simulateur-de-filtres-rejoué-2026-11-22-lot-22e)).

## 2026-11-22 — Calibration des filtres

### D-057 : le modèle de filtres ne se juge que sur des logs enregistrés à 1,6 kHz ou plus

**Tranché par Cédric** le 2026-11-22 (lot 22a de `docs/plan.md`), en validant le découpage du lot.

**Contexte.** Le lot 22 compare le modèle de filtres au firmware étage par étage (`rust/otb-analysis/src/filter_stages.rs`).
Un log plus lent ne montre pas assez du signal gyro pour juger un filtre : ce qui dépasse la moitié de sa cadence s'y replie
(L-008), et le modèle est rejoué à la cadence du log (L-011).

**Décision.**
- `filter_stages::MIN_LOG_RATE_HZ` = 1 600 Hz (cadence annoncée par l'en-tête, `Settings::log_rate_hz`) : en dessous, tous les
  étages sont sautés avec la raison « sous 1600 Hz (D-057) », et `otb-dev corpus stages` ne les compte pas.
- Ne change ni les conseils d'enregistrement de D-017 (2 kHz pour un gyro à 8 kHz, `rules.log_rate_needed`), ni les autres
  outils.

**Témoins.** `rust/otb-analysis/tests/filter_stages.rs:logs_trop_lents_ou_d_variable_sautes_avec_la_raison` (log à 1 kHz : aucun
étage contrôlé, chaque raison cite D-057).

## 2026-11-11 — Notes de version

### D-056 : les notes de version sont générées par GitHub d'après les titres et les étiquettes `RN: …` des PR

**Tranché par Cédric** le 2026-11-11 (lot 20 de `docs/plan.md`).

**Contexte.** La page Releases recopiait toute la section du `CHANGELOG.md` : ≈ 30 Ko pour la 0.9.0, 46 paragraphes écrits
pour des développeurs, où un pilote ne retrouve pas ce qui change pour lui. Le CHANGELOG doublait aussi chaque demande de
fusion d'une saisie à la main. Le dépôt de Betaflight fait autrement : un titre de PR court, une étiquette `RN: …` par PR, et un
`release.yml` qui range les PR en rubriques dans la liste que GitHub génère.

**Décision.**
- **Le titre d'une PR est une phrase en français** qui dit l'effet du changement : elle devient la ligne des notes. Cela
  **remplace la règle « titre de la PR = nom de la branche »** de `CONTRIBUTING.md` ; le nommage des branches
  (`type/description-courte`) ne change pas.
- **Une étiquette `RN: …` par PR**, et une seule : `RN: nouveauté majeure`, `RN: nouveauté` → Nouveautés ; `RN: amélioration`,
  `RN: sécurité`, `RN: refonte` → Améliorations ; `RN: correction` → Corrections ; `RN: ignorer` (CI, outils de
  développement, plans, registres) → absente des notes. Rubriques dans `.github/release.yml`, mots en français (D-001).
- **Dependabot exclu des notes** (étiquette `dependencies`), sans rubrique de fin.
- Pas de rubrique attrape-tout : une PR sans rubrique disparaîtrait des notes. C'est le workflow `etiquette-pr.yml` qui
  l'empêche : il échoue sans exactement une étiquette `RN: …` connue, ou sur un titre en forme de nom de branche.
- **Notes générées par GitHub** à la publication (`construire.yml`, tâche `brouillon`) entre la dernière version publiée et la
  nouvelle étiquette ; `otb-dev release notes` traduit les mots fixes de GitHub (« What's Changed »…), ajoute en tête le bloc
  Téléchargements et refuse une liste vide ou restée en anglais.
- **`CHANGELOG.md` supprimé.** L'ancien journal se lit dans git : `git show 2cd2416:CHANGELOG.md` (dernier commit qui l'a,
  sur `main` avant ce lot) est le **seul** endroit qui garde les notes des versions 0.1.0 à 0.8.0 (leurs pages Releases
  n'existent plus) et le détail des PR #128 à #146. `git show v0.9.0:CHANGELOG.md` s'arrête à la 0.9.0.

**Options écartées.**
- Garder le CHANGELOG à côté des notes générées : double saisie, et deux textes qui finissent par se contredire.
- Une rubrique « Autres » pour les PR sans étiquette : les notes se rempliraient de lignes de maintenance sans intérêt pour un
  pilote ; mieux vaut que l'oubli fasse rougir la PR.
- Laisser les mots fixes de GitHub en anglais : l'interface et la documentation sont en français (D-001).

**Témoins.** `rust/otb-dev/tests/release.rs` : traduction d'une liste au format relevé le 2026-11-11, refus d'une liste vide ou
restée en anglais, mêmes étiquettes dans `release.yml` et `etiquette-pr.yml` ; chaque protection cassée à la main fait
rougir son test. Essai à blanc de `generate-notes` joint à la PR #147 (5 PR en 2 rubriques, ni `RN: ignorer` ni
Dependabot) ; contrôle d'étiquette rouge sans étiquette et vert avec, sur la même PR. 86 PR étiquetées (#60 à #147).

**Contrôle exigé** (2026-11-12, réglage de Cédric après la fusion de #148) : `etiquette` est l'un des cinq contrôles de la
règle `protection-main`, avec `tests`, `windows`, `cargo-deny` et `actionlint` ; une PR sans étiquette `RN: …` ne peut donc
pas être fusionnée. Relu par `gh api repos/…/rules/branches/main` le même jour ; premier passage exigé vert sur la PR #150,
étiquetée juste après son ouverture (cas de L-098).

**Exception de la 0.9.0** (version de rupture, étiquetée avant `release.yml`, que GitHub ne lit qu'au commit de l'étiquette :
L-097) : sa page garde en tête les cinq points écrits à la main de « Ce qui change pour vous », puis le bloc Téléchargements
et une liste fabriquée en local au format de GitHub depuis les étiquettes des PR, passée par `otb-dev release notes`
(36 lignes ; 50 Ko → 6 Ko, le 2026-11-11). Les versions suivantes n'ont que la liste générée.

---

## 2026-11-11 — Relecture des workflows

### D-055 : les workflows sont relus par `actionlint` dans la CI, pas sur le poste

**Tranché par Cédric** le 2026-11-11. Complète D-054.

**Contexte.** La compétence `opentunebox-dev` demandait de lancer `actionlint` avant de modifier un workflow, en le disant
« installé sur le poste » : il ne l'était pas (constaté le 2026-11-11). Un contrôle qui dépend de la mémoire d'une session et
d'un outil local finit sauté sans bruit ; les workflows changent rarement, mais une erreur dans `construire.yml` ne se voit
qu'au jour de la publication.

**Décidé.** Un job `actionlint` dans `tests.yml`, sur chaque envoi sur `main` et chaque demande de fusion : `actionlint`
1.7.12 téléchargé sur la machine de GitHub, **vérifié par son empreinte SHA-256 écrite dans le workflow** (même méthode que
`appimagetool` dans `construire.yml`), puis lancé sur tous les workflows. Il passe aussi les scripts bash des étapes à
`shellcheck`, présent sur les machines Ubuntu de GitHub. Rien à installer chez Cédric.

**Options écartées.**
- Installer `actionlint` sur le poste : contrôle laissé à la mémoire de chacun, et un outil de plus à tenir à jour à la main.
- Une action tierce qui l'enveloppe : un éditeur de plus à qui faire confiance, à ajouter à la liste des actions autorisées
  (D-054), pour trois lignes de script.

⚠️ **Conséquence à tenir** : Dependabot ne voit pas cette version (ce n'est pas une action). Pour la monter, changer le numéro
**et** l'empreinte, relevée dans le fichier `actionlint_X.Y.Z_checksums.txt` de la version.

**Contrôle exigé** (2026-11-11, réglage de Cédric après la fusion de #144) : `actionlint` est l'un des quatre contrôles
de la règle `protection-main`, avec `tests`, `windows` et `cargo-deny` ; un workflow en erreur empêche donc la fusion. Relu
par `gh api` le même jour.

**Témoins.** Avant l'envoi, les 19 scripts bash des workflows passés à `shellcheck` comme le fait `actionlint` (expressions
`${{ }}` remplacées, mêmes exclusions) : rien à signaler. Premier passage du job : voir la demande de fusion du lot 19.

---

## 2026-11-11 — Sécurité du dépôt et de la chaîne de publication

### D-054 : réglages de sécurité de GitHub, Claude refusé sur les forks, `--locked` en CI, aucun `unsafe`

Tranche le point (3) laissé ouvert par D-043. **Tranché par Cédric** le 2026-11-11, à la suite d'un audit de sécurité
(GitHub, Claude Code, Rust) : il a fait lui-même les réglages de GitHub et demandé le reste.

**Contexte.** Le dépôt est public (D-043) et ce qu'il publie est téléchargé et lancé par des pilotes. Audit du 2026-11-11 :
0 alerte de secret, 0 alerte Dependabot ouverte, aucun `unsafe` dans le code livré, décodeur déjà borné (D-044). Les risques
restants : un fichier `.bbl` piégé (la porte d'entrée du programme), une dépendance compromise, un fichier publié remplacé, et
Claude Code amené à exécuter du code écrit par un tiers.

**Décidé — réglages de GitHub** (faits par Cédric ; relus par `gh api` le 2026-11-11) :
- `main` : règle `protection-main` (depuis le 2026-11-04) — demande de fusion obligatoire, contrôles `tests`, `windows` et
  `cargo-deny` verts et à jour, ni réécriture ni suppression. Étiquettes `v*`, `essai-v*`, `python-*` : règle
  `protection-etiquettes`, création, déplacement et suppression réservés à l'administrateur.
- Analyse des secrets avec blocage à l'envoi ; alertes et mises à jour de sécurité Dependabot ; signalement privé de faille
  ([SECURITY.md](../SECURITY.md)).
- **Versions immuables** : une version publiée ne peut plus changer de fichiers ni d'étiquette, même avec un jeton volé.
  Compatible avec `construire.yml`, qui envoie les fichiers pendant le brouillon puis publie.
- **CodeQL**, configuration par défaut (Rust et workflows).
- Actions autorisées : celles de GitHub, plus `anthropics/claude-code-action`, `EmbarkStudios/cargo-deny-action` et
  `oven-sh/setup-bun` (appelée en coulisse par l'action Claude). Une action absente de la liste fait échouer le workflow :
  l'y ajouter (Settings → Actions → General). CI d'un contributeur extérieur : approbation de Cédric **à chaque fois**.
  Jeton par défaut en lecture, sans droit d'approuver une demande de fusion.

**Décidé — dans le dépôt** :
- `claude.yml` **refuse une demande de fusion venue d'un fork** (`isCrossRepository`), avant d'en récupérer le code ; une
  réponse illisible vaut refus. Raison : Claude y lancerait `cargo`, donc les `build.rs` et les tests d'un inconnu, avec le
  jeton de l'abonnement dans l'environnement, et un `.claude/settings.json` modifié y ajouterait ses crochets. Le filtre sur
  l'auteur du commentaire ne protège pas de ce cas : c'est Cédric qui écrit « @claude », mais sur le code d'un autre.
- `--locked` sur chaque commande `cargo` de la CI qui compile : l'exécutable publié est fait avec exactement les versions de
  `Cargo.lock`, celles que `cargo-deny` contrôle ; un `Cargo.lock` incohérent fait échouer au lieu d'être recalculé en silence.
- `permissions: contents: read` écrit dans `tests.yml` et `gui-build.yml` (le réglage du dépôt le donnait déjà).
- `unsafe_code = "deny"` pour tout l'espace de travail (`rust/Cargo.toml`, `[lints] workspace = true` dans chaque crate) ;
  seule exception, levée dans le fichier : `rust/otb-gui/examples/banc_cache.rs`, jamais livré.
- Crochet `.claude/hooks/guard-git.sh` : fait demander la confirmation pour `git push`, `git tag`, `gh release`,
  `gh pr merge`, `gh api` et `gh workflow run` même quand une option les précède (`git -C rust push`) ou qu'ils sont glissés
  dans `sh -c "…"`, ce que les règles « ask » de `.claude/settings.json`, qui comparent le début de la commande, laissaient
  passer.
- Essai de mutations : six sortes de fichiers abîmés (octets isolés, en-tête, coupure, trou, morceau doublé, 1 %) passent
  par le décodeur (`rust/otb-decoder/tests/robustness.rs`) et par tout le programme jusqu'à l'assistant
  (`rust/otb-cli/tests/robustesse.rs`) ; quelques cas en CI, `OTB_MUTATIONS=N` pour un essai prolongé
  ([validation.md](validation.md), « Robustesse face à un en-tête hostile »).

**Options écartées.**
- `cargo-fuzz` : il exige une chaîne Rust *nightly*, contraire à la version figée (`rust/rust-toolchain.toml`). L'essai de
  mutations tourne sur la chaîne stable, sans dépendance.
- `forbid` au lieu de `deny` pour `unsafe_code` : `forbid` ne se lève pas localement, et le banc du cache en a besoin.
- Séparer la compilation des versions (sans droit d'écriture) de leur envoi : les fichiers devraient passer par les
  artefacts, que la méthode de publication proscrit, et une crate piégée capable de voler le jeton pourrait aussi bien piéger
  l'exécutable lui-même. La défense est en amont : `cargo-deny`, `--locked`, versions immuables.

**Laissé tel quel.** L'installateur Windows n'est pas signé (SmartScreen avertit) ; une signature est payante, et Cédric a
choisi le 2026-11-12 de la laisser ainsi. L'épinglage des actions par SHA est **exigé** par le dépôt depuis le 2026-11-11 (case cochée par Cédric, relue par
`gh api`) : une action référencée par une étiquette (`@v4`) fait échouer le workflow.

**Témoins.** Réglages relus par `gh api` (règles, `immutable-releases`, `code-scanning/default-setup`,
`actions/permissions`). Refus des forks : script de l'étape lancé avec un faux `gh` (`false` → passe ; `true` et erreur
réseau → code 1). Crochet : 8 commandes à confirmer et 8 commandes ordinaires, toutes classées comme attendu. `unsafe` :
l'exception retirée, l'exemple est refusé (3 erreurs). Mutations : voir [validation.md](validation.md).

---

## 2026-11-10 — Mémoire du cache des outils

### D-053 : le cache des outils est borné par outil, en nombre d'entrées

Clôt le point 9 de W-035 et la dernière case « cache » du lot 15 de [plan.md](plan.md). **Tranché par Cédric** le
2026-11-10, sur la mesure du même jour (L-094, [validation.md](validation.md), « Mémoire du cache des outils »).

**Contexte.** Le cache des résultats (`rust/otb-gui/src/state.rs:Cache`, une table par log) n'était vidé qu'au changement de
période : entre deux réglages, il **n'évinçait rien**. Mesuré sur tout le corpus (535 logs, 642 sessions), une entrée pèse
7,4 Mo (p50) et jusqu'à **71,3 Mo** pour la réponse indicielle, dont 72 jeux de réglages sont atteignables (3 fenêtres ×
4 lissages × 3 consignes × 2 « Y corr. ») : **5,1 Go pour un seul log ouvert**.

**Décidé : un plafond par outil, exprimé en nombre d'entrées**, la moins récemment utilisée jetée la première
(`state.rs:Famille::plafond`) :

| Outil | Entrées gardées | Pire cas avant | Pire cas après |
|---|---|---|---|
| Réponse indicielle | **3** | 72 × 71,3 Mo = 5 112 Mo | 214 Mo |
| Simulateur de filtres | **20** | non borné | 4 Mo |
| Assistant | **6** (tous les profils) | 5 Mo | 5 Mo (rien n'est jamais évincé) |
| Guide | **2** (rapide, complète) | 0,02 Mo | 0,02 Mo |
| **Un log ouvert** | | **≈ 5,1 Go** | **≈ 223 Mo** |

Le **3** de la réponse indicielle est le seul chiffre qui se voit à l'usage : revenir sur un réglage essayé quatre réglages
plus tôt relance un calcul au lieu d'être instantané. Un cache ne fait jamais perdre que du **temps** : le résultat recalculé
est identique.

**Option écartée : un plafond global en Mo** (« ne pas dépasser 500 Mo par log »). Elle demanderait d'écrire à la main le
poids de chaque type rangé (`dyn Any` ne le dit pas) : du code qui dérive dès qu'un champ est ajouté, et qu'aucun test ne
rattrape. Même arbitrage que D-051 : un compteur se vérifie dans un test, une mémoire non.

**Option écartée : un plafond unique pour tous les outils.** Vingt coups de curseur du simulateur de filtres (90 ko l'entrée)
chasseraient les résultats de la réponse indicielle (71 Mo l'entrée), c'est-à-dire exactement ceux qu'il faut garder.

⚠️ **Conséquence à tenir** : baisser un plafond sous le nombre d'entrées qu'un **même cadre** demande ferait évincer un
résultat en cours d'affichage, donc recalculer sans fin. Le Guide en demande deux (rapide et complète) : son plafond ne peut
pas descendre sous 2.

**Témoin** : `rust/otb-gui/tests/state.rs`, module `plafond` — quatre tests (le plafond est tenu, un accès rajeunit l'entrée
affichée, un outil ne chasse pas les autres, le Guide garde ses deux vérifications). Contre-preuves vues en échec : retirer
l'éviction fait rougir trois d'entre eux, retirer le rajeunissement fait rougir celui qui le vise. Les 28 captures hors écran
(toutes les pages, les deux thèmes) sont **identiques au bit près** avant et après.

---

## 2026-11-08 — Stockage des données de test

### D-052 : seul `step_tuning.json` est stocké compressé

**Tranché par Cédric** le 2026-11-08, sur les mesures de l'étape 16g du lot 16 ([plan.md](plan.md)).

**Contexte.** Les 31 fichiers de cas de `rust/*/tests/data/` pèsent **133 Mo**, soit la quasi-totalité du poids du dépôt.
`common::cas` (`rust/otb-analysis/tests/common/mod.rs`) lit déjà indifféremment `<nom>.json` ou `<nom>.json.gz`, et
`log_check.json.gz` prouvait le procédé (26 → 5,2 Mo). Mesure sur les dix plus gros : 93 Mo → 31 Mo (÷3,0) pour 215 ms de
décompression sur une suite qui dure 126 s.

**Décidé : `step_tuning.json` seul passe en `.gz`** — 10,5 Mo → 0,53 Mo, soit **÷19,4**, mesuré avec `gzip -9 -n`. C'est le seul
fichier dont le gain sort de l'ordinaire : les neuf autres tournent autour de ÷2,2 à ÷2,6, parce qu'ils portent des flottants
écrits en hexadécimal, presque incompressibles. Le dépôt perd 10 Mo pour 7 ms de décompression.

**Option écartée : compresser les dix plus gros.** Elle gagnerait ~60 Mo au clone, mais un `.gz` est **opaque pour git** :
aujourd'hui, régénérer un fichier de cas ne stocke que l'écart entre deux versions ; compressé, chaque régénération stocke un
fichier entier. Le réenregistrement des références (W-028) ferait alors grossir le dépôt plus vite qu'il ne l'allège, pour un
gain par fichier de ÷2,5 seulement.

**Option écartée : ne rien changer.** Elle laissait 10 Mo sur la table sans raison : le gain de ce fichier-là est hors norme et
sa lecture est déjà prévue par le code.

**Conséquence assumée.** Une régénération de ce fichier depuis l'étalon Python (`scripts/step_tuning_cases.py`, qui écrit en
clair) doit passer par `gzip -9 -n` — `-n` sans quoi l'horodatage change les octets à contenu identique. La procédure le dit
(`docs/validation.md`, « Refaire une référence Python »), et `.gitattributes` déclare `*.gz binary` pour que git ne tente jamais
de normaliser ses fins de ligne.

**Témoin.** `cargo test -p otb-advisor --test step_tuning` passe sur le fichier compressé (2 tests, 0,80 s) ; `common::cas`
panique en nommant les deux chemins essayés si aucun des deux n'existe.

---

## 2026-11-08 — Ouverture de plusieurs logs en parallèle

### D-051 : au plus trois logs décodés en même temps

Débloque W-050 (lot 11 de [plan.md](plan.md)). **Tranché par Cédric** le 2026-11-08, sur les mesures du lot 5.

**Contexte.** L'interface ouvre les fichiers **les uns après les autres** (`rust/otb-gui/src/app.rs`, une file `queue` et un
seul `opening`). Décoder en parallèle va **×2,40** (6 logs : 0,830 s → 0,345 s), mais chaque fichier décodé en même temps
coûte **168 à 181 Mo** de pic mémoire : sans plafond, ouvrir dix logs d'un coup demanderait près de 2 Go.

**Décidé : au plus trois fichiers décodés en même temps.** Le gain mesuré sur six logs est conservé pour l'essentiel, et une
machine modeste reste servie.

⚠️ **Chiffre de mémoire corrigé le 2026-11-10 (lot 11, mesuré)** : la décision annonçait « environ 540 Mo au pire », en comptant
180 Mo par fichier. Le pic réel à trois fichiers est de **663 Mo**, parce que le plus gros log du corpus (`LOG00008.BFL`) en
coûte 324 Mo à lui seul. Cédric a **gardé le plafond de trois** en connaissance de ce chiffre : le gain est de × 2,26 (1,835 s
→ 0,811 s), et dans l'interface le surcoût du parallélisme n'est que de + 164 Mo, les logs déjà ouverts pesant de toute façon.
Mesures et conditions : [validation.md](validation.md), « Ouverture de plusieurs logs ».

**Option écartée : un plafond exprimé en mémoire** (« ne pas dépasser 1 Go »). Elle est plus souple, mais elle demande de
mesurer le pic du processus pendant le décodage pour décider s'il faut lancer le fichier suivant — une mesure qui n'est pas
portable entre Windows et Linux, et qui se teste mal. Un compteur de fichiers se **vérifie dans un test** (lot 11), une
mémoire non.

**Option écartée : décoder en parallèle les sessions d'un même fichier.** ×1,10 seulement sur `FINAL.bbl`, et elle cumulerait
plusieurs tables compactes du même log.

**Conséquence assumée.** Au-delà de trois logs ouverts d'un coup, les suivants attendent : le temps total ne descend plus en
dessous d'un tiers du temps séquentiel, quel que soit le nombre de cœurs.

**Témoin** (lot 11, livré le 2026-11-10) : `rust/otb-log/tests/parallel.rs` compte les fichiers en cours de décodage et exige
qu'il n'en passe jamais plus de trois ; `rust/otb-parity/tests/parallel_identique.rs` compare les empreintes complètes des
`FlightLog` entre ouverture parallèle et séquentielle. Le plafond est tenu par un pool `rayon` **propre à l'appel**
(`otb_log::parallel::map_bounded`), jamais par le pool global : il ne dépend donc pas du nombre de cœurs de la machine.

---

## 2026-11-05 — Tracé du viewer indépendant du nombre d'échantillons

### D-050 : les sources sont citées sans nommer leurs auteurs

**Tranché par Cédric** le 2026-11-07 : plus aucune information du projet ne doit indiquer le nom de l'auteur de PIDtoolbox.

**Contexte.** Les méthodes de réglage, les repères de bruit et les objectifs de retard venaient de vidéos, d'un wiki et du
README de PIDtoolbox, cités dans `docs/spec-methodes.md` sous la forme « [S33] Prénom Nom, *titre*, lien ». Le nom apparaissait
aussi dans 78 endroits du dépôt : commentaires du code, tableau d'origine des seuils de `docs/assistant.md`, textes vus par
l'utilisateur (assistant, guide intégré, rapport en ligne de commande), CHANGELOG et attendus de tests.

**Décision.**
- Aucun nom de personne, nulle part : ni dans le code, ni dans la documentation, ni dans les textes affichés.
- Les **sources restent citées** : chaque entrée `[S…]` garde son titre, sa date, son lien et sa citation exacte. D-005
  (chaque formule et chaque seuil justifiés par une méthode publiée) tient donc toujours, et la calibration reste vérifiable.
- Dans les textes, « méthode de … » devient **« méthode des séries de vols »** (ce que la méthode fait : un seul curseur change
  d'un vol à l'autre, par pas de 0,2) ; « repères de … » devient « repères usuels » ou « repères de PIDtoolbox » quand le
  repère vient bien de l'outil.
- Une citation de Cédric qui portait le nom le marque comme coupé (« se concentrer sur la méthode […] ») plutôt que de le
  remplacer en silence.

**Option écartée.** Remplacer le nom par « l'auteur de PIDtoolbox » : le nom propre disparaissait, mais la personne restait
identifiable. Écartée par Cédric le 2026-11-07. Écartée aussi : supprimer les sources elles-mêmes, ce qui aurait laissé les
seuils de l'assistant sans aucune justification écrite et contredit D-005.

**Conséquence.** `docs/spec-methodes.md` reste la preuve de chaque repère ; les numéros `[S…]` ne changent pas, donc toutes
les références du code et de `docs/assistant.md` restent valables.

**Précisé le 2026-11-08 (revue intégrale, accord de Cédric).** La formulation « aucun nom de personne, nulle part » visait
**l'auteur de PIDtoolbox**, et elle seule. Les **ouvrages publiés** gardent le nom de leurs auteurs : c'est ainsi qu'on cite
un livre, et D-005 exige que chaque seuil soit justifié par une méthode publiée et vérifiable à la page. `docs/assistant.md`
peut donc continuer à écrire « Skogestad et Postlethwaite, *Multivariable Feedback Control*, § 2.4.3 » ou « Åström et Murray,
*Feedback Systems*, § 9.3 » — sans ces noms, la source serait introuvable et la justification du seuil, invérifiable.
La règle reste entière pour les sources dont la personne est elle-même le sujet (vidéos, wiki, README de PIDtoolbox) : titre,
date, lien et citation, jamais de nom.

**Précisé le 2026-11-12 (lot 21, choix de Cédric, W-055).** La règle vaut pour **toute personne**, pas seulement l'auteur de
PIDtoolbox : guides, articles, presets, outils, pages de wiki et notes signées sont cités sans nom (ni nom réel ni pseudonyme),
et désignés par leur marque, leur site ou leur chemin. Une périphrase qui désigne la personne (« l'auteur de … ») est écartée
pour la même raison qu'en 2026-11-07. Seule exception : les **ouvrages publiés** (livres). Écartée : « tout document publié et
signé garde ses auteurs », qui aurait fait de presque chaque source une exception. Hors champ : `THIRD-PARTY-LICENSES.txt`, où
les licences exigent elles-mêmes les noms et copyrights.

### D-049 : la Réponse indicielle est ordonnée et colorée par l'utilisateur

🔁 REMPLACÉE par D-066 (la barre de fragments ordonne les courbes de toutes les pages ; chaque fragment garde sa couleur partout).

**Tranché par Cédric** le 2026-11-06, à l'essai de la 0.9.0 : « la réponse indicielle doit afficher les courbes en fonction
de l'ordre de sélection de l'utilisateur. Il doit être possible de ne pas sélectionner de courbe. C'est l'utilisateur qui
doit définir l'ordre d'affichage des courbes. Leurs couleurs doivent s'adapter à cet ordre / sélection. »

**Contexte.** Jusqu'ici, un log recevait sa couleur **à l'ouverture** (première couleur libre de la palette,
`state.rs:add`) et la gardait sur toutes les pages ; les courbes étaient tracées dans l'ordre d'ouverture des fichiers,
et une page sans choix retombait sur le log courant (impossible de n'afficher aucune courbe). Comparer deux vols dans
l'ordre voulu, ou isoler une courbe, demandait de fermer et rouvrir les logs dans le bon ordre.

**Décidé : la règle change pour cette page seulement** (`FileSelection::user_order`, `tools/step.rs`) :

- les courbes, la légende et les colonnes de mesures suivent **l'ordre des clics**, pas l'ordre d'ouverture ;
- la couleur d'une courbe est son **rang dans la sélection** (1re cochée = 1re couleur de la palette) ; la liste des
  fichiers de la page montre ces couleurs-là, et un fichier non coché y est écrit en gris ;
- **tout décocher est un choix** : la page reste vide au lieu de revenir au log courant. Avant le premier clic, le log
  courant reste coché, comme sur les autres pages.

**Option écartée : appliquer la règle partout** (comme PIDtoolbox). Un même log changerait alors de couleur d'une page
à l'autre selon la sélection de chaque page, ce qui casse la lecture croisée Spectre ↔ Réponse indicielle ↔ Courbes.
Autre option écartée : garder les couleurs fixes et ne changer que l'ordre — elle ne répond pas à la demande.

**Conséquence assumée.** Un même log peut avoir une couleur sur la Réponse indicielle et une autre ailleurs. Si cela
gêne à l'usage, la décision se remplace (ici, ou partout).

**Témoins.** `rust/otb-gui/tests/tools_common.rs::la_selection_ordonnee_suit_les_clics_et_peut_rester_vide` et
`rust/otb-gui/tests/step_tools.rs::le_libelle_est_celui_du_fichier_et_la_couleur_le_rang_dans_la_selection`
(vus en échec en rétablissant l'ancienne règle).

### D-048 : index multi-résolution min/max, et un trou par paquet qui en contient un

Clôt W-049 (lot 10 de [plan.md](plan.md)). **Tranché par Cédric** le 2026-11-05 : piste (b),
index multi-résolution, plafond de mémoire « +10 % du pic ».

**Contexte.** Le viewer trace ses courbes décimées min/max : la fenêtre visible est coupée en paquets et chaque
paquet ne donne que son point le plus bas et son point le plus haut (≈ 3 840 points pour 1 920 pixels). L'ancienne
`otb-gui::plots::visible_points` parcourait **tous les échantillons visibles** à chaque cadre : 10,5 à 12,1 ms par
cadre sur la plus longue session du corpus (620 079 échantillons), et autant pendant un glissement, où les bornes
changent à chaque image ([validation.md](validation.md), « Temps de cadre de l'interface »).

**Option écartée (a) : cache des points décimés**, invalidé quand les bornes, la largeur, le lissage ou la courbe
changent. Elle ne gagne **rien** pendant un glissement ou un zoom — c'est-à-dire dans le seul cas qui saccade —
puisque les bornes y changent à chaque cadre.

**Décidé (b) : un index multi-résolution par courbe** (`rust/otb-dsp/src/minmax.rs`, `MinMaxPyramid`) : minimum et
maximum de chaque bloc de 8 échantillons, puis de chaque bloc de 8 blocs, etc. Une requête combine les plus gros
blocs qui tiennent dans le paquet et ne parcourt que ses bords. Conséquences :

- **le calcul reste hors de l'interface** (`otb-dsp`, règle d'[AGENTS.md](../AGENTS.md)) : il est testable sans écran,
  et `otb-gui` n'en garde que le cache ;
- **l'index vit avec la courbe lissée**, dans le cache du viewer (`viewer_page.rs:Series`), pas à l'ouverture du log :
  rien n'est payé pour les courbes qu'on n'affiche pas, et changer de lissage refait les deux ensemble ;
- **la sortie est identique point pour point** à celle de l'ancienne fonction sur toute courbe sans trou : ce n'est
  pas « la même enveloppe », c'est la même liste de points (`rust/otb-dsp/tests/minmax.rs` compare à une copie figée
  de l'ancienne implémentation) ; les captures hors écran du viewer sont identiques **au bit près**, aux deux thèmes ;
- **un paquet qui contient au moins un `NaN` est tracé comme un trou** (un seul point `NaN`, la ligne est coupée).
  L'ancienne règle, née d'une comparaison nue (`y[i] < y[lo]`), donnait un résultat qui dépendait de la position du
  trou dans le découpage : un `NaN` en tête de paquet effaçait le minimum et le maximum réels, un `NaN` ailleurs
  était ignoré. La nouvelle règle est déterministe et *associative*, condition nécessaire pour qu'un index soit exact
  (L-085). Elle ne concerne que les courbes à trous (rotation des moteurs en Hz) ;
- **8 blocs par niveau** (`minmax::FACTOR`) : l'index pèse 43 % d'une courbe `f64`. Mesuré à 16 blocs il tomberait à
  20 %, mais coûterait 27 % de calcul en plus ; la mémoire n'étant pas le point dur ici (voir plus bas), la vitesse
  l'emporte. Changer ce nombre ne change aucun résultat, seulement le compromis.

**Preuves** (poste Linux, i7-11800H, release ; détail et plages dans [validation.md](validation.md)) :

| Mesure | Avant | Après |
|---|---|---|
| Cadre du viewer, 620 079 échantillons, vue entière | 10,5 à 12,1 ms | **4,6 à 4,8 ms** |
| idem, pendant un glissement | 10,3 à 10,8 ms | **4,8 à 4,9 ms** |
| Cadre du viewer, 321 729 échantillons | 6,9 à 8,7 ms | **3,1 à 3,2 ms** |
| Décimation seule, 620 079 échantillons (`otb-dsp --example banc_minmax`) | 0,99 ms | **0,17 ms** |
| Pic de mémoire du processus (les deux logs) | 180 et 286 Mo | **180 et 286 Mo** |

**Plafond de mémoire tenu.** L'index pèse 11,6 Mo (`FINAL.bbl`) et 22,3 Mo (`LOG00008.BFL`), soit 6,4 % et 7,8 % du
pic, sous les 10 % fixés par Cédric. Mieux : le pic du processus **ne bouge pas** (mesuré avec et sans l'index) ; il
est atteint pendant le décodage, avant que le viewer ne construise ses courbes.

**Ce qui reste.** Le cadre n'est plus proportionnel au nombre d'échantillons mais il n'est pas constant pour autant :
à nombre de points rendus égal, le même log mesure 4,7 ms en vue entière et 2,1 ms zoomé sur un huitième. Ce qui
reste est le **tracé** des ≈ 3 840 points de chaque courbe (indépendant du log) et les bords de paquets, qui touchent
la courbe sur toute son étendue (défauts de cache, L-086). Aller plus loin demanderait de réduire le nombre de points
rendus, donc de toucher au rendu : W-052.

## 2026-11-05 — Concordance avec les références figées

### D-047 : `golden.rs` applique la tolérance mesurée de chaque fonction portée

Remplace le choix du 2026-11-04 noté dans W-048 et au lot 9 de [plan.md](plan.md) (« le test garde
`rtol: 0.0` »), et clôt W-048.

**Contexte.** `rust/otb-parity/tests/golden.rs` rejoue les 1 728 appels figés des références Python (`OTB_GOLDEN`). Écrit le 2026-10-30
quand toutes les fonctions portées étaient identiques au bit près, il exigeait `rtol: 0.0` pour toutes. Le même jour (phase 4b-1),
les fonctions qui passent par une FFT, `sin` / `cos`, `exp` ou une SVD ont reçu une tolérance **mesurée** dans
`rust/otb-parity/src/registry.rs:tolerance` (Rust et numpy n'additionnent pas dans le même ordre), appliquée par `otb-parity check` ;
`golden.rs` n'a pas suivi. La CI n'a pas les références : le test y sautait sans bruit. Mesure du 2026-11-05 : 243 appels sur 1 728
diffèrent au bit près, dans 8 fonctions (`spectral.spectrum`, `freq_throttle`, `freq_time`, `to_db`, `_numeric.xcorr_full`,
`bf_filters.whatif_spectrum`, `sliders.slider_view`, `sliders.sliders_from_pids`), **tous** dans leur tolérance ; le test ne pouvait
donc pas passer, et `verify --strict` non plus sur le poste qui a les références. Le 2026-11-04, seul le premier écart était connu
(le test s'arrêtait dessus) et Cédric avait refusé d'affaiblir le test.

**Décision** (accord de Cédric le 2026-11-05).
- `golden.rs` compare chaque appel avec `registry::tolerance` de sa fonction : **le bit près** pour toute fonction sans tolérance
  déclarée (la règle par défaut), la tolérance mesurée et documentée pour les autres ; c'est le contrat de `otb-parity check`. Aucune
  fonction n'est comparée moins strictement que ce que le projet promet.
- Le test liste **tous** les écarts au lieu de s'arrêter au premier.
- Il reste plus strict que `check` sur un point : un appel dont Python ou Rust rend une erreur fait échouer le test.
- Écarté : garder `rtol: 0.0` (le test serait rouge par construction, donc ignoré) ; une tolérance unique pour toutes les fonctions
  (affaiblirait les fonctions exactes).

**Preuve.** Avec `OTB_GOLDEN` : vert, 1 728 appels rejoués. Contre-preuves : tolérance remise à zéro → 243 écarts ; un bit de plus sur
une valeur de `uniform_filter1d` (fonction exacte) → 40 écarts, un par appel. Leçon L-083.

## 2026-11-05 — Polices embarquées

### D-046 : les polices d'egui restent embarquées, chacune sous sa propre licence

Complète D-045 (sur les licences de polices seulement) et clôt W-046.

**Contexte.** Sous GPL 3 (D-043), l'exécutable posait un conflit possible avec la police **Ubuntu Light** (Ubuntu Font Licence 1.0,
crate `epaint_default_fonts`) : le copyleft de la GPL demandait de distribuer l'ensemble sous GPL, alors que l'article 4 de `UFL.txt` dit
« The Font Software, modified or unmodified, in part or in whole, must be distributed entirely under this licence, and must not be
distributed under any other licence. » D-045 remplace la GPL par PolyForm Noncommercial 1.0.0, qui n'est pas un copyleft : elle porte
sur « the software the licensor makes available under these terms » (section *Definitions* de `LICENSE`), c'est-à-dire le code de
Cédric, et n'exige rien des composants tiers. La police reste distribuée sous sa licence, ce que demande justement l'article 4. Reste une
tension de vocabulaire : D-045 refuse toute dépendance « à copyleft », or les licences de polices (Ubuntu Font Licence, OFL) imposent
qu'une police **modifiée** reste sous leur licence ; `rust/deny.toml` les acceptait déjà, sans décision écrite.

**Décision** (accord de Cédric le 2026-11-05).
- **Ubuntu Light et Hack restent embarquées** (polices par défaut d'egui, `rust/otb-gui/src/app.rs:setup` inchangé) ; chacune garde sa
  licence, reproduite dans `THIRD-PARTY-LICENSES.txt` (`otb-dev licenses`), livré par les installateurs.
- Les **licences propres aux polices** (`Ubuntu-font-1.0`, `OFL-1.1` dans `rust/deny.toml`) sont admises : leur condition ne porte que sur
  le fichier de police, qui n'est jamais redistribué sous la licence du projet. Le refus de D-045 vaut toujours pour le **code** sous
  copyleft (GPL, LGPL, AGPL, MPL). Une nouvelle licence de police s'ajoute seulement après lecture de son texte, avec Cédric.
- W-046 est clos ; le lot 7 (première publication Rust) n'est plus bloqué.

**Options écartées.** (a) Retirer Ubuntu Light et passer Hack en police du texte normal : la raison (le copyleft de la GPL) a disparu ;
Hack porte elle aussi une licence de police (Bitstream Vera, en plus de MIT : `Hack-Regular.txt`), donc rien n'aurait été simplifié, et
l'interface aurait pris un aspect à chasse fixe pour ≈ 365 Ko gagnés (Ubuntu Light : 361 676 octets, egui-icons : 3 484 octets). (b) DejaVu
Sans (même licence Bitstream Vera, couvre tous les caractères des textes mais 759 720 octets). (c) Avis juridique : jugé inutile une fois la
cause du conflit disparue.

**Témoins.** Textes lus le 2026-11-05 dans le registre de cargo (`epaint_default_fonts` 0.37.0 : `UFL.txt`, `Hack-Regular.txt`) et dans
`LICENSE`. Couverture mesurée avec `fc-query` sur les sources (`rust/otb-gui/src`, `rust/otb-gui/assets/guide`, `rust/otb-advisor/src`,
`rust/otb-analysis/src`, `rust/otb-format/src`) : tout caractère non ASCII qu'Ubuntu Light dessine, Hack le dessine aussi. Ce n'est pas un avis juridique.

## 2026-11-04 — Licence non commerciale

### D-045 : la licence est PolyForm Noncommercial 1.0.0 ; tout usage commercial est interdit

Amende D-043 (la licence seulement ; le dépôt reste public et le reste de D-043 reste en vigueur).

**Contexte.** Le 2026-11-04, quelques heures après le passage sous GPL 3 (D-043), Cédric reprend la question : il veut que le code reste ouvert
mais que l'application ne puisse pas être vendue — « je ne souhaite pas du tout qu'il soit possible de la vendre non plus. Personne ne pourra en
tirer une valeur commerciale ». La GPL 3 ne peut pas l'imposer : elle **autorise** la vente et oblige seulement à livrer les sources. Une licence
qui interdit la vente sort de la définition « open source » (OSI, FSF), qui exige d'autoriser l'usage commercial : le choix est donc entre un
copyleft fort (vente légale mais sans intérêt) et une licence « source disponible » (vente interdite). Cédric tranche pour l'interdiction nette.

**Décision.**
- Licence **PolyForm Noncommercial 1.0.0** (texte officiel dans `LICENSE`, précédé de la ligne `Required Notice:` que prévoit la licence).
  Copyright : Cédric. Tout usage **non commercial** est permis (personnel, loisir, club, association, enseignement, recherche publique, santé
  et secours) ; l'usage commercial est interdit : revente, version modifiée vendue, service payant, usage professionnel.
- **Pas de double licence, pas d'exception commerciale vendue, l'auteur compris.** Portée réelle de cet engagement, écrite sans illusion : une
  contribution reçue sous cette licence **lie** l'auteur du projet (il ne peut pas la commercialiser sans l'accord de son auteur), tandis que sur
  son propre code aucune licence ne contraint le titulaire du copyright — l'engagement y est une **promesse publique**, consignée ici et dans le
  README, non un verrou juridique.
- Les contributions sont publiées **sous la seule licence du projet** : pas de CLA, pas de cession supplémentaire (`CONTRIBUTING.md`).
- **Aucune dépendance à copyleft** n'est désormais acceptable (GPL, LGPL, AGPL, MPL) : du code sous copyleft ne peut pas être redistribué sous une
  licence qui interdit l'usage commercial. C'est un refus, plus une discussion (`rust/deny.toml`, `CONTRIBUTING.md`).
- Les installateurs livrent `LICENSE` à côté de `THIRD-PARTY-LICENSES.txt` (section « Notices » de la licence : quiconque reçoit une copie reçoit
  les termes et la ligne `Required Notice:`) ; l'installateur Windows l'affiche aussi à l'installation (`LicenseFile`).
- Conséquence assumée : OpenTuneBox n'est plus « open source » au sens OSI/FSF mais **« source disponible »**. Il sort des dépôts Debian, Fedora et
  Flathub, GitHub n'affiche plus de licence reconnue, et certains contributeurs refuseront de travailler sur du non-libre. Le « Open » du nom
  désigne le code ouvert, pas la définition OSI (dit dans le README).
- Les droits GPL 3 **déjà accordés ne sont pas révocables** : qui aurait cloné le dépôt entre son passage en public et ce commit garderait les
  droits GPL sur cet instantané, et les commits concernés restent sous GPL dans l'historique. Les témoins ci-dessous rendent le risque nul en
  pratique (personne n'a copié le dépôt).
- Options écartées : **AGPL 3 + double licence** (reste open source et ferme le trou du service web, mais n'empêche pas un tiers d'en vivre tant
  qu'il publie son code : contraire à la volonté de Cédric) ; **Commons Clause** sur Apache 2.0 (le mot « vendre » n'y est pas défini, licence mal
  vue) ; **CC BY-NC-SA** (Creative Commons la déconseille elle-même pour du logiciel : ni code source ni brevets) ; **BUSL 1.1** et **FSL** (elles
  rebasculent en licence libre après 2 à 4 ans, ce qui rouvrirait la vente).

**Témoins.** `LICENSE` : 4 647 octets. Texte officiel récupéré par
`gh api repos/polyformproject/polyform-licenses/contents/PolyForm-Noncommercial-1.0.0.md` (4 563 octets, 73 lignes, 16 sections) et vérifié
identique par `diff` après ajout de la ligne `Required Notice:`. Relicenciement possible sans l'accord de personne : `git log --format='%an <%ae>'`
donne 433 commits de Cédric et 1 de dependabot (mise à jour de version, pas une œuvre). Aucune dépendance copyleft : la liste `allow` de
`rust/deny.toml` (MIT, Apache-2.0, BSD-2/3, ISC, Zlib, Unlicense, BSL-1.0, Unicode-3.0, OFL-1.1, Ubuntu-font-1.0) et `cargo deny check`.
Exposition du code déjà publié sous GPL 3 : `gh api repos/Canaill51/OpenTuneBox` donne 0 fork, 0 étoile, 0 abonné et 0 version publiée le jour du
changement. Livraison de la licence par les installateurs : `cargo test -p otb-dev --test install_linux`
(`la_licence_du_projet_est_installee_avec_les_mentions_tierces`, rouge avant la correction des trois scripts, vert après).

**Ouvert.** W-046 (police Ubuntu Light) : la cause du conflit disparaît avec cette licence ; clos le 2026-11-05 par D-046.

## 2026-11-04 — Bornes du décodeur

### D-044 : le décodeur refuse un en-tête qui annonce plus de 512 champs ou un intervalle hors de 1 à 65 535

**Contexte.** L'en-tête annonce trois nombres que le décodeur croyait sur parole : le nombre de champs par trame (`Field I name`…), `I interval` et `P interval`. Mesuré le 2026-11-04 sur la source `tests/data/jeno5_5s.bbl` modifiée en mémoire : 20 000 champs → 3,7 s et des tables gigantesques ; intervalles à 9·10¹⁸ → boucles de recherche de l'itération attendue sans borne. Valeurs réelles relevées sur les 525 logs du corpus : `I interval` ∈ {102, 128, 256}, `P interval` ∈ {1, 2, 4, 8}, 35 à 60 champs.

**Décision** (accord de Cédric le 2026-11-04).
- Au plus **512 champs** par type de trame ; `I interval` et chaque terme de `P interval` entre **1 et 65 535** (entiers 16 bits du firmware, de mémoire : non vérifié dans les sources Betaflight). Hors bornes : `DecodeError::HeaderOutOfBounds` (message en français), session refusée, donc fichier refusé ; `decode_file` la rend en `io::ErrorKind::InvalidData`, et l'interface l'affiche dans la barre d'état.
- `decode_bytes` rend `Result<Vec<DecodedSession>, DecodeError>` (option A de Cédric : erreur typée, plutôt qu'écarter la session en silence).
- Une allocation ratée devient `DecodeError::OutOfMemory` (`try_reserve` avant chaque ajout), pas un abandon du programme.
- La recherche des itérations sautées d'une trame P est bornée par `MAX_ITERATION_JUMP`.
- Réglages d'en-tête à zéro : `otb-log` garde ses `.max(1)` ; `Settings::implausible` (gyro > 100 kHz, soit trois fois les 32 kHz des vrais logs) fait signaler « réglages de l'en-tête invraisemblables » par la CLI et par la barre d'état, sans inventer de valeur.

**Option écartée.** Écarter en silence la session hors bornes : aucun message n'arrive à l'utilisateur.

**Témoins.** `rust/otb-decoder/tests/robustness.rs` (10 cas, contre-preuve : plafond retiré → rouge) ; `otb-parity check` 0 échec sur 3123 fichiers ; les 70 fichiers abîmés de `decode_fixtures.rs` inchangés ; `corpus validate` : voir [validation.md](validation.md#décodeur).

## 2026-11-04 — Dépôt public et licence

### D-043 : le dépôt est public, sous licence GPL 3

✏️ AMENDÉE par D-045 (la licence : PolyForm Noncommercial 1.0.0 au lieu de la GPL 3 ; le dépôt reste public et le reste de D-043 reste en vigueur).

Amende D-001 (« projet privé, tous droits réservés, pas de licence libre » ; le reste de D-001 reste en vigueur) et le résumé d'`AGENTS.md`.

**Contexte.** Le 2026-11-04, la CI du dépôt privé s'arrête avant de démarrer un seul travail : « The job was not started because recent account payments have failed or your spending limit needs to be increased » (annotation lue sur les exécutions de la PR #96). La documentation de GitHub indique que les runners standard sont gratuits dans un dépôt public. Cédric passe le dépôt en public le 2026-11-04 et tranche quatre points (questions posées le même jour).

**Décision.**
- Licence **GNU GPL version 3** (texte officiel dans `LICENSE`). Copyright : Cédric.
- Pas « tous droits réservés » (choix de Cédric : GPL 3) ; pas MIT ni Apache 2.0 non plus : la GPL oblige quiconque redistribue une version modifiée à publier son code.
- Les versions publiées v0.2.0 à v0.8.0 (builds Python avec PySide6/Qt et macOS, ~375 Mo chacune) sont **supprimées** de la page Releases (7 versions, 0 restante au 2026-11-04) ; les étiquettes git `v0.2.0` à `v0.8.0` restent. Raison : elles devenaient téléchargeables par tous et leurs obligations de licence n'étaient pas vérifiées.
- Les nouveaux commits utilisent l'adresse noreply de GitHub, pas l'adresse personnelle ; l'historique n'est pas réécrit (les copies déjà faites depuis le passage en public garderaient l'ancienne adresse).
- Les pseudos de pilotes tiers sont remplacés par des noms neutres dans la documentation (l'historique garde les anciens noms) ; la liste `SHARED_NAMES` de `rust/otb-dev/src/corpus/mod.rs` garde les noms réels de drones, car l'outil `corpus` doit les reconnaître dans le vrai corpus.

**Témoins.** `gh repo view` : visibilité PUBLIC ; `gh api repos/Canaill51/OpenTuneBox/releases` : 0 version ; CI des PR #96 et #97 verte après le passage en public (la #97 : `tests` 6 min 7, `windows` 7 min 3, `cargo-deny` 33 s). Le texte de `LICENSE` vient de `gh api licenses/gpl-3.0` (35 150 octets).

**Ouvert.** (1) W-046 : la licence de la police Ubuntu Light (Ubuntu Font Licence 1.0, « the Font Software … must be distributed entirely under this licence ») face à la GPL, non tranchée ; (2) la compatibilité avec la GPL 3 est lue sur la liste de la FSF pour MIT, ISC et Apache 2.0 seulement ; BSD, Zlib, Unicode-3.0, BSL-1.0 et 0BSD ne sont pas vérifiées ; ce n'est pas un avis juridique ; (3) réglages de sécurité GitHub (protection de `main` et des étiquettes, analyse des secrets, signalement privé) : tranché par D-054 (2026-11-11).

### D-042 : macOS abandonné, application livrée pour Windows et Linux

Amende D-001 (« Windows, macOS et Linux » devient « Windows et Linux ») et D-035 (le « Linux, Windows, macOS » de la phase 6 et
de la fusion finale).

**Contexte.** Aucune machine macOS n'est disponible pour essayer l'application ni un installateur ; la CI peut les construire
mais personne ne peut les lancer. Cédric (2026-11-03, parmi : fabriquer sans essai / abandonner macOS / bloquer la fusion) :
« Abandonner macOS ».

**Décision.**
- L'application est livrée pour **Windows et Linux** ; `construire.yml` n'a plus de ligne macOS à la phase 7-4.
- Plus d'icône `.icns`, plus de `packaging/macos/` ni de `.dmg`.
- Le code reste portable (egui, rfd) : rien n'est retiré de `rust/` pour macOS, mais il n'est ni compilé en CI ni garanti.
- Pas X : fabriquer un `.dmg` jamais essayé promettrait un système non vérifié (règle « ce qui n'est pas vérifié se dit non vérifié »).

**Témoins.** Réponse de Cédric du 2026-11-03 ; l'inventaire de `packaging/` (spec, `build_dmg.sh`, `.icns`) est dans le plan de la phase 7.

**Ouvert.** W-021 (écarts sur ARM, à voir au premier test macOS) et le chemin macOS de `drones.json` (W-037 point 4) deviennent sans objet.

### D-041 : notchs dynamiques ajoutés placés sur les pics prévus, sinon à leur centre initial

✏️ AMENDÉE par D-058 (la page Filtres rejoue le suivi du firmware ; D-041 ne vaut plus que pour `bf_filters::whatif_spectrum`).

**Contexte.** Quand on monte `dyn_notch_count` au-delà des centres enregistrés dans le log (debug FFT_FREQ), `_gyro_h2` plaçait les
notchs ajoutés aux centres initiaux du firmware, `(p + 0,5)(max − min)/n + min`. Cédric a vu, sur `1.Stock.bbl` (Air65, 1 notch
enregistré à 207 Hz, plage 100–600 Hz), des creux de −28 dB (2 notchs) à −60 dB (3 notchs) à 350 et 517 Hz, sans pic à ces fréquences :
le rapport |H_modifié|² / |H_actuel|² creusait là où le firmware n'aurait pas mis de notch. Le firmware attache le notch `p` au `p`-ième
pic le plus haut (`dyn_notch_filter.c`).

**Mesure** (2026-11-02, 383 sessions de 64 drones du corpus en debug FFT_FREQ avec au moins 2 notchs, un seul notch supposé connu, axe roll,
une voix par drone ; positions vérifiées dans le temps passé par chaque notch, cases de 18,5 Hz, tolérance ± 1,5 case) :

| Modèle | Positions réelles retrouvées | Positions prédites où un notch passe (≥ 5 % du temps) |
|---|---|---|
| A : centres initiaux (précédent) | 0,28 | 0,76 |
| B : pics seuls | 0,35 | 0,85 |
| **B' : pics, sinon centre initial (retenu)** | **0,45** | **0,81** |
| C : emplacements au hasard (témoin) | 0,18 | 0,68 |

Critère fixé avant de lire les résultats : gagner au moins 0,15 de rappel sur A et sur C, sans perdre en précision, sur au moins 20 drones.
B' le remplit (60 drones), B seul non (+ 0,07). Positions sur un pic : précision 0,87 ; positions de repli : 0,73.
Aucun notch ajouté n'est « garé » à son centre initial (0 sur 522) ; 78 sessions sur 383 n'ont aucun pic ajouté prévu.
Script de la mesure : jetable (non versionné) ; la refaire au besoin avec `motors.dyn_notch_targets` et `bf_filters.fft_freq_centres`.

**Décision.** `bf_filters.complete_dyn_centres` : centres enregistrés d'abord (ramenés dans la plage), puis les pics prévus les plus hauts
(`motors.dyn_notch_targets` sur le roll du gyro non filtré de toute la période, réponse des notchs RPM actuels appliquée, comme la validation de
D-022) à plus de 1,5 case d'un centre déjà pris, puis les centres initiaux. Sans gyro non filtré : centres initiaux, comme avant. La chaîne
« actuelle » n'est jamais complétée : réglages inchangés = log exact (L-024). Le Bode et le tableau des retards gardent les centres initiaux
(case « notchs dyn. (centres initiaux) »), sans log. L'assistant n'utilise pas cette branche (il ne modifie ni le nombre ni la plage) : `advisor_rates.py` avant / après sur 632 sessions du corpus, 0 conseil différent.

**Limites connues.** Le rappel de B' reste sous 0,5 ; l'axe de debug est supposé être le roll (défaut du firmware, réglage non lu) ; les pics sont
ceux de la période entière, pas de chaque fenêtre de 300 ms ; un repli sur un centre initial peut encore creuser sans pic réel (27 % des replis).

**Choix de Cédric** (2026-11-02, après sa capture du Simulateur) : nouveau modèle voulu, validé sur le corpus, appliqué à Python d'abord.

**Rust** (2026-11-02, W-040) : même modèle dans `rust/otb-analysis/src/bf_filters.rs` (`complete_dyn_centres`, `dyn_peaks`, pics gardés dans `WhatIfCache.peaks`), même résultat que Python sur cas fabriqués et sur le log réel de 5 s des tests (`bf_filters`, `bf_sim`, `filter_tools`) ; pas encore comparé sur un vrai log long (W-040, W-028). Aucun message de plus dans l'interface : sans gyro non filtré, le Simulateur n'a de toute façon pas de simulation (titre d'alerte existant).

---

## 2026-11-02 — Contrats de la phase 6-6b

### D-040 : Interface egui : écarts voulus avec l'interface Qt et limites gardées

Questions posées à Cédric le 2026-11-02 (tri de W-034 à W-038) ; il a choisi les options recommandées.

**Contexte.** Les phases 6-2 à 6-6a ont laissé des écarts connus avec les fenêtres Qt (`docs/travaux-reportes.md`). Avant la phase 7
(suppression de Python), chacun est classé : voulu, gardé tel quel, ou corrigé.

**Décision.**
- **Écarts voulus (définitifs)** : liste des fichiers d'un outil en cases à cocher (Qt : clic remplace, Ctrl ajoute) ; diagnostic en
  widgets egui et non en HTML ; « -> » à la place de « → » dans les textes (absent des polices d'egui, L-068) ; diagrammes de Bode
  zoomables ; calcul automatique à chaque changement (le bouton « Calculer » relance à la demande).
- **Écarts voulus ajoutés le 2026-11-03 (W-039)** : titres d'onglets longs (« Analyse spectrale », « Réponse indicielle », « Simulateur de
  filtres ») et non les libellés courts de Qt ; boutons « Fermer » et « Tout fermer » grisés quand aucun log n'est ouvert (Qt les laisse actifs
  sans effet).
- **Gardé tel quel** : seuls le chevron et le titre d'un groupe de la Configuration se replient, et l'état n'est pas gardé entre deux
  lancements ; la date du nom d'export est celle d'UTC (un jour d'écart possible autour de minuit) ; les barres d'erreur du bruit moteur
  n'ont pas de capuchon.
- **Corrigé le 2026-11-03** (branche `feat/rust-toolbar-close`, Cédric a choisi les options recommandées) : boutons « Fermer » (croix, log
  affiché) et « Tout fermer » (corbeille) ajoutés après « Ouvrir… », que rien ne remplaçait (l'interface egui ne pouvait fermer aucun log) ;
  « Ouvrir… » plein, couleur d'accent, comme le bouton principal de Qt.
- **Corrigé en 6-6b** : un résultat du Simulateur de filtres qui vient d'un autre log n'est plus affiché pendant le calcul du nouveau
  (`result_is_for_other_log`, W-036 (8)).

**Options écartées.** Reproduire le clic / Ctrl+clic de Qt ; dessiner les capuchons à la main ; une dépendance de dates pour l'heure
locale (non vérifiée) ; garder l'état replié de la Configuration dans la persistance d'eframe.

**Témoins.** `result_is_for_other_log` et `FilterSimResult::log_id` : `rust/otb-gui/tests/filter_tools.rs`. L'appel dans `render` n'a pas
de test d'interface (règle d'affichage en fonction pure, L-071).

## 2026-11-02 — Contrats de la phase 6-5

### D-039 : Guide egui : mémoire dans la persistance d'eframe, fiches tolérantes et protégées, Hack en police de secours

**AMENDÉE par D-071** (2026-11-23) : le rôle d'un log est celui de son fragment dans la bibliothèque ; la clé
`guide.roles` n'est plus que relue, pour les fragments qui n'ont pas encore de rôle.

**AMENDÉE par D-068** (2026-11-23) : les fiches ne vivent plus dans `drones.json` mais dans la bibliothèque ; l'ancien
fichier n'est plus écrit, seulement lu une fois pour être repris, puis renommé `.repris`. Les points « mêmes octets » et
« Supprimer » ci-dessous ne valent plus ; la lecture tolérante, la protection d'un fichier illisible et la confirmation de
« Recommencer » tiennent.

Écarts volontaires avec `gui/guide.py` ; questions posées à Cédric le 2026-11-02, il a choisi les options recommandées.

**Contexte.** Python garde la dernière phase, la dernière fiche et le rôle de chaque log dans `QSettings` (D-036 : non repris), lit
`drones.json` sans rien dire quand il est abîmé, puis l'écrase au premier clic (toutes les fiches perdues), et n'a pas à choisir de
police. Les polices d'egui n'ont ni « ✔ » ni « ✖ » ni « → ▲ » dans le texte normal (L-068).

**Décision.**
- **Mémoire** : la persistance d'eframe (`App::save`, clés `guide.phase`, `guide.card`, `guide.roles`), pas les réglages Qt ; une phase
  ou un rôle inconnus sont ignorés. Premier lancement : accueil, première fiche, rôles devinés.
- **`drones.json`** : même chemin que Python (`QStandardPaths.AppDataLocation` + `OpenTuneBox/OpenTuneBox`), mêmes octets
  (`json.dumps(ensure_ascii=False, indent=1)`), `pid_khz` entier réécrit entier. Lecture **tolérante** (`DroneCard::from_value`, W-029
  point 1) : clé absente ou d'un type faux = valeur par défaut, seuls un `name` absent ou qui n'est pas un texte et un fichier qui n'est
  pas une liste sont des erreurs.
- **Mêmes octets pour les fiches écrites par l'application** (cases booléennes). Une case non booléenne lue (`{"done": {"a": 1}}`) est réécrite `true` / `false`, alors que Python garde la valeur lue ; un `pid_khz` booléen est ramené à `4.0` : fichiers jamais produits par l'application, écart assumé (W-037).
- **Confirmations** : « Supprimer » et « Recommencer » visent la fiche **du clic** (numéro et nom gardés) et ne font rien si la liste a changé entre-temps ; Python vise la fiche choisie au moment du « Oui », qui peut avoir changé si un log d'un autre drone finit de se charger (défaut de Python, non reproduit).
- **Fichier illisible** : jamais écrasé en silence. Message dans le Guide, puis, avant la première écriture, mis de côté sous
  `drones.json.illisible` (`.1`, `.2`… si une copie existe déjà). Écriture par fichier temporaire puis renommage.
- **Symboles** : la police Hack, déjà livrée avec egui, devient le dernier recours du texte normal (`app::setup` ; « → ▲ ● ○ ↑ ↓ ← ◇ ■ »
  mesurés), sans octet de plus. « ✔ ✖ ℹ ⚠ » restent absents : icônes Lucide `check`, `x`, `info`, `triangle-alert` (même version 1.49.0 que
  les autres, exception D-005). Le « ✔ » du texte d'intro de la phase 1 est retiré à l'affichage (`guide::display_text`). Les polices
  d'emoji d'egui (≈ 740 Ko) ne sont pas activées : la taille de l'exécutable est l'objectif n° 1.
- **Vérification des logs** : rapide dans le fil de l'interface (mesuré en release : 2 ms sur 5 s de vol, 26 ms sur 185 000 points ; une
  par cadre au plus), complète en arrière-plan, une à la fois. Clé de cache = période (+ taille de la fiche pour les repères des
  filtres) ; la vérification en cours est identifiée par (`uid`, clé) et non par l'identité de l'objet (L-072), ce qui corrige le défaut de
  Python où une période changée pendant un calcul laissait la page sur la version rapide.

**Témoins.** `rust/otb-gui/tests/guide.rs` (aller-retour octet pour octet de 27 fichiers écrits par `scripts/guide_cards_cases.py`
avec le vrai `load_cards` / `save_cards` de Python ; fichier abîmé mis de côté, jamais écrasé ; mémoire restaurée ; signes dessinés sans
police manquante) ; `rust/otb-advisor/tests/drone_card.rs` (un test par type faux) ; captures des deux thèmes lues à l'œil.

## 2026-11-02 — Contrats de la phase 6-4

### D-038 : page Configuration egui : les lignes sont alignées entre logs par nom de réglage, pas par position

Écart volontaire avec `gui/tools/setup_info.py`.

**Contexte.** Python écrit les noms de lignes du **premier** log puis range les valeurs des autres logs **par rang** (`setup_info.py`,
l.159-171). Deux logs dont les « Curseurs simplifiés » n'ont pas les mêmes clés (versions de Betaflight différentes) voient alors une
valeur face au nom d'un autre réglage. Question posée à Cédric (2026-11-02) : copier Python tel quel ou aligner par nom ; il a répondu
« Aligner par nom ».

**Décision.**
- Les lignes sont alignées par nom : ordre du premier log, puis les noms propres aux autres logs à la suite de leur groupe ; « — » quand
  un log n'a pas le réglage (`tools/setup.rs::align`).
- La ligne « — / absents » des curseurs d'un log sans curseurs est écartée dès qu'un autre log en a de vrais : elle ne crée pas une
  fausse ligne qui « diffère ».
- Pas de copie de l'alignement par position, même « pour la parité » : il fausse la comparaison qui est le but de la page.

**Témoins.** `rust/otb-gui/tests/filter_tools.rs` (alignement de deux logs aux curseurs différents, pas de ligne « — » fantôme) ;
`_sections` de Python rejoué sur `tests/data/jeno5_5s.bbl` : sorties identiques ligne à ligne pour un seul log ; capture à deux logs
(`jeno5_5s` en 2026.6.0 et `Leo/stock` en 4.5.1) vérifiée à l'œil.

## 2026-11-01 — Ordre et contrats de la phase 6

### D-037 : cache des résultats des outils egui : un par log, partagé avec le thread de calcul, clé = réglages de calcul + période

Précise D-035 (phase 6) ; tranche le point 1 de W-032.

**Contexte.** En Python, `compute` d'une fenêtre d'outil range son résultat dans `entry.extras` du log, avec une clé qui contient les
réglages (`step_window.py`) : changer un réglage d'affichage ne refait pas le calcul, et le vol « Damping 0.8 » n'est pas recalculé quand on
ajoute « Damping 1.2 » à la série. En egui, `Tool::compute` n'a pas de `self` : il ne reçoit que ses réglages et des copies des logs (`LogInput`).

**Décision.**
- `state::Cache` (`Arc<Mutex<HashMap<String, Arc<dyn Any + Send + Sync>>>>`) : un par log chargé (`LoadedLog.extras`), cloné dans
  `LogInput.cache` ; un clone voit le même contenu.
- La **clé** est écrite par l'outil et contient tout ce qui change le résultat : réglages de calcul **et période** (`step:{réglages:?}:{période:?}`).
- Le cache est vidé quand la période change (`AppState::set_period`), pour borner la mémoire, mais ce n'est pas lui qui garantit l'exactitude.
- Le cache ne range que ce qui ne dépend pas du rang du fichier : libellé et couleur sont remis à chaque affichage (`FileStep`, comme Python
  remet `fs.color` et `fs.label`).

**Options écartées.** Cache dans la page de l'outil (perdu dès qu'on ajoute ou retire un fichier, et un seul fichier recalculé ne profite à
personne) ; clé sans la période, le vidage suffisant (un calcul parti avant le changement de période pourrait écrire un résultat de l'ancienne
période après le vidage : course entre le thread de calcul et l'interface).

**Origine.** Conséquence de W-032 (1) ; choix technique, pas une question à Cédric.

**Témoins.** `rust/otb-gui/tests/step_tools.rs::le_cache_sert_au_second_calcul_mais_ni_apres_un_changement_de_periode_ni_de_reglage` ;
deux mutations tuées (clé sans la période, clé sans les réglages).

### D-036 : 5c avant la phase 6 ; fiches drone lues avec tolérance, même fichier ; réglages Qt non repris

**AMENDÉE par D-068** (2026-11-23) : le même fichier n'est plus que lu, à la reprise des fiches dans la bibliothèque.

Précise D-035 (phases 5c et 6) ; tranche le point 1 de W-029.

**Contexte.** Les phases 4, 5a et 5b sont fusionnées (PR #73). La phase 6 remplace l'interface Qt (≈ 6 000 lignes) par egui ;
la page Guide doit lire et écrire les fiches drone (`drones.json`), et l'interface Qt garde ses réglages dans `QSettings`.

**Décision.**
- **5c d'abord** : la ligne de commande (`opentunebox info` / `report`) devient le binaire Rust `otb-cli`, rapports identiques
  caractère par caractère, avant d'écrire l'interface.
- **Fiches drone tolérantes** : même fichier `drones.json`, même emplacement, même format que Python. Un champ de type faux
  prend sa valeur par défaut, une fiche n'est jamais perdue ; `pid_khz` entier s'écrit `4`, pas `4.0`.
- **Réglages d'interface** (thème, dernier dossier, courbes, tailles de fenêtres) : l'interface egui repart de zéro, sans lire
  `QSettings`. Seules les fiches drone sont reprises.

**Options écartées.** Fiches strictes (une fiche au champ faux disparaîtrait de la liste) ; lecture unique des `QSettings`
au premier lancement (code en plus, à vérifier sur trois systèmes, pour des choix qui se refont en une minute).

**Choix de Cédric** (2026-11-01, parmi : 5c d'abord / directement la 6 ; tolérante / stricte ; repartir à zéro / reprendre).

**Témoins.** Plan : [migration-rust.md](migration-rust.md) ; W-029.

## 2026-10-30 — Migration complète vers Rust

### D-035 : migration complète vers Rust, plus aucun Python ; interface egui ; noyau Rust autonome

Amende D-031 (méthode : plus de pont PyO3 module par module à partir de la phase 4), D-034 (le reste de `analysis/` et
`log/` n'est plus repoussé) et D-001 (la technologie Python + PySide6 + pyqtgraph est remplacée à la phase 6 ; le reste de D-001, projet
privé et interface en français, reste en vigueur). D-002 et D-006 sont aussi amendées (plus de Python, dépendances compilées permises).

**Contexte.** Après les phases 3a et 3b (D-034), porter le reste de `analysis/` et de `log/` rapporte peu en vitesse (W-019),
mais Cédric veut un binaire autonome, sans Python. Mesuré le 2026-10-30 : ≈ 13 800 lignes de Python d'application (interface
5 637, assistant 3 346, analyses 2 749, décodeur et logs ≈ 2 000), ≈ 9 900 lignes de tests, ≈ 960 lignes de scripts,
contre ≈ 2 900 lignes de Rust. Le poste a `gcc` et `g++`.

**Décision.**
- **Périmètre : tout.** Application, tests, scripts du corpus, outils d'icônes et de packaging passent en Rust. À la fin,
  `git ls-files '*.py'` est vide. L'ancien Python reste seulement sur `main` et dans un tag figé `python-etalon`.
- **Interface : egui + egui_plot**, sous réserve d'un prototype d'essai (« spike ») sur de vraies données qui doit établir pour
  chaque point : axe logarithmique, barre de couleurs de heatmap, lignes et régions déplaçables, barres d'erreur, second axe Y,
  séries de 10⁶ points, Markdown/HTML riche, thèmes clair et sombre, fenêtres de fichiers, test d'interface sans écran. Un point
  bloquant sans contournement raisonnable revient devant Cédric avant d'aller plus loin sur l'interface.
- **Méthode : noyau Rust autonome.** Bibliothèques `otb-log`, `otb-analysis`, `otb-advisor`, binaires `otb-cli` et `otb-gui`,
  outil de parité `otb-parity`. Pas de pont PyO3 module par module : le Python de la branche reste intact et sert d'étalon, par
  des références enregistrées sur le corpus (`golden.py record-modules`).
- **Preuve.** Tolérance de chaque module fixée avant d'écrire le code ; assistant : 0 session aux constats différents sur 630 ;
  rapports de la ligne de commande identiques au caractère ; recette de la 3b pour chaque module (audit des dtypes, tests avant le
  code, mutations vues en échec, relecture indépendante, parité sur le corpus).
- **Ordre.** Décision, prototype d'interface, filet de parité, phase 4 (journaux et analyses), phase 5 (assistant et ligne de
  commande), phase 6 (interface), phase 7 (basculement et suppression du Python). Plan : [migration-rust.md](migration-rust.md).

**Options écartées.** Slint (la documentation consultée ne décrit aucun composant de graphiques : les exemples dessinent dans un
tampon de pixels, toute l'interaction serait à écrire) ; cxx-qt (garde Qt, à installer et à embarquer, et `qmake` est absent du
poste : ce n'est pas du « tout Rust ») ; pont PyO3 module par module (adaptateurs jetables pour `FlightLog`, `Settings`, `Report`) ;
périmètre « application seulement » (le dépôt aurait gardé du Python).

**Choix de Cédric** (2026-10-30, parmi : egui / Slint / cxx-qt ; tout / application seulement ; noyau Rust autonome / pont
PyO3 module par module).

**Ouvert.** ~~Verdict du prototype d'interface~~ (rendu le 2026-10-30 : aucun point bloquant, cinq contournements, voir [migration-rust.md](migration-rust.md#verdict-du-prototype-dinterface-e1)) ; tolérance de chaque module ; reproduction exacte des formats de nombres de Python
par `format!` (à tester sur des valeurs enregistrées) ; W-019 repris.

## 2026-10-29 — Migration vers Rust

### D-034 : phase 3 « par le gain mesuré » : ouverture des logs, puis `motors.py` ; le reste de `analysis/` et `log/` est repoussé

✏️ AMENDÉE par D-035 (le reste de `analysis/` et `log/` n'est plus repoussé).

**Contexte.** Exploration du 2026-10-29 (carte des 16 modules, profilage sur une session de 72 s et trois gros logs) :
porter tout `analysis/` et `log/` rapporterait peu en vitesse. Le reste est de la FFT numpy (déjà en C), ou de la
logique métier (dicts de réglages, `round()` demi-pair de Python, seuils discontinus, regex). Le gain est dans
deux postes : l'ouverture d'un gros log (`from_session` = 51–56 % du chargement) et `motors.py` avec son noyau
`sections_response` (24 % + 17 % des 574 ms de l'assistant).

**Décision.** Phase 3 dans cet ordre, une branche et une PR par étape, un critère chiffré fixé avant le Rust :
- **3a** : ouverture des logs (`_drop_implausible`, `axes()`, `col()`) en Rust, crate `rust/otb-log` (Rust pur).
- **3b** : `motors.py` (boucles par fenêtre × harmonique) et `sections_response` en Rust, crate `rust/otb-analysis`.
- **3c** (`whatif_spectrum` de filter_sim) décidée seulement après la mesure de 3b : elle exige de porter la
  construction des chaînes de filtres (`gyro_chain`, `Settings`).
- **Repoussé** (W-019) : `spectral`, `loop`, `delay`, `stats`, `sliders`, `step_series`, `settings`, `datasets`,
  `period`, `FlightLog` lui-même (objet Python gardé : seuls des tableaux contigus et des scalaires passent à Rust).
- Les anciennes fonctions restent **inchangées** dans `tests/ref_*.py` comme étalons de parité.

**Choix de Cédric** (2026-10-29, parmi : par le gain mesuré / bas en haut, tout porter / seulement 3a).

**3a, résultats** (2026-10-29, fichier de 33,6 Mo, un chargement par processus) : chargement **838 ms → 502–548 ms
(1,6×)**, `from_session` 413 → 147 ms (2,8×) ; sorties **identiques au bit près** à l'ancien code ; mémoire conservée après
ouverture +154 Mo → +138 Mo (−10 %), pic +11 Mo. Parité sur le corpus (`golden.py`) : décodeur 0 différence, assistant
**0 session aux constats différents** sur 630 (durée cumulée 1879 s → 1357 s), outils : structure identique, écart relatif
maximal 2,4e-10 (inchangé). `tests/test_native_log.py` (245 cas, six mutations vues en échec, L-034).
Le premier critère visait « moins de mémoire » : atteint seulement après correction (L-040) ; le gain vient d'une
**seule passe** sur la matrice (L-039).

**3b, résultats** (2026-10-29, 15 sessions tirées au hasard, 818 s de vol, un fil) : assistant **666 → 515 ms par
session (1,29×)**, critère ≥ 1,25× atteint. Le gain vient de `harmonic_prominence` (1,9 s → 0,28 s sur le lot), de
`_masked_spectrum` (0,41 → 0,10 s) et de `rpm_attenuation` (z⁻¹ calculé une fois pour toutes les fenêtres, et plus
d'appel à `hypot` par case : même décision de plancher de gain, testée dans la zone ambiguë). Sorties **identiques au
bit près** à l'ancien code (étalon `tests/ref_motors.py`) : 80 comparaisons sur de vrais logs, float32 et float64.
Parité sur le corpus (`golden.py`) : décodeur 0 différence (499 fichiers), assistant **0 session aux constats
différents** sur 630, outils 0 différence à `rtol=1e-9` (écart scipy→numpy connu). `tests/test_native_motors.py`
(675 cas) et huit mutations vues en échec (L-034). Deux pièges coûteux : les fréquences moteur sont en float32 dans de
vrais logs, et l'ancien code calculait les bandes dans cette précision (L-041) ; `np.abs` complexe et `np.mean` ne se
reproduisent pas avec `hypot` et une somme naïve (L-042). `FFT` et découpage restent en numpy.

**3c abandonnée** (2026-10-30, mesures sans profileur, un fil sauf mention, 15 sessions tirées au hasard, 818 s de vol,
après la 3b). Assistant : 516 ms par session (7,74 s au total) ; 348 ms avec 16 cœurs. `whatif_spectrum` : 0,84 s sur
7,52 s, soit 0,15 s de FFT (`_seg_spectra`), 0,50 s pour les chaînes de filtres des fenêtres (`_gyro_h2`) et 0,19 s
d'autre. La construction des chaînes (`gyro_chain`), la seule partie que la 3c aurait portée en Rust, pèse 0,31 s :
4,0 % de l'assistant à un fil (5,8 % avec 16 cœurs). Même supprimée entièrement, le gain ne dépasse donc pas
**1,04× à un fil (1,06× avec 16 cœurs)**, loin du critère de 1,25× ; et il faudrait réécrire les `Settings` et les
chaînes de filtres (logique métier à variantes selon la version de Betaflight). Piste de mémorisation essayée en
prototype (cache gardé pour toute l'analyse : 1 544 évaluations redondantes sur 2 701 pour la chaîne actuelle, 837 pour
la modifiée) : 7,51 → 7,06 s, **1,06×**, mêmes clés de constats (177) ; sous le seuil de 1,1× fixé, non retenue.
Le premier chiffre avancé (« 1,4 s » puis « environ 1 s de construction ») venait d'un profil pris sous `cProfile`,
qui gonfle les petits appels Python (L-039).

**Ouvert.** W-015 ; W-019.

### D-033 : phase 2b réduite à un pilote : le noyau de `step_response` en Rust ; porter la FFT seule est écarté

**Contexte.** Profilage du 2026-10-29 (log de 72 s à 2016 Hz, assistant complet 720 ms) : la FFT ne pèse que 27 %
de l'assistant et `rustfft` va à peu près aussi vite que `pocketfft` (numpy) : porter **la FFT seule** donnerait
5 % au plus. Le temps part dans de grosses opérations numpy limitées par la mémoire : `step_response` prend 348 ms
(48 % de l'assistant) dont 38 % de FFT seulement.

**Décision.** Pilote limité à `step_response` (fenêtrage, FFT, déconvolution de Wiener, `cumsum`, contrôle
qualité et sommes fusionnés en un noyau Rust, `rust/otb-dsp/src/step.rs`) ; `_seg_spectra`, `loop_spectra` et
`xcorr` ne sont portés qu'après mesure, avec leur contexte, en phase 3.
- Dépendances Rust pures (pas de compilateur C, D-006) : `realfft` (FFT réelles) et `rayon` (fenêtres en parallèle).
- **Résultat indépendant du nombre de fils** : les fenêtres se calculent en parallèle, les sommes se font dans
  l'ordre des fenêtres (par paquets de 256, comme la boucle numpy).
- **Un processus par cœur = un fil par processus** : `scripts/corpus_labels.py:map_files` fixe
  `RAYON_NUM_THREADS=1` (sinon cœurs² fils).
- La boucle numpy d'origine reste dans `tests/ref_step.py`, **inchangée**, comme étalon de parité.
- Critère fixé avant d'écrire le Rust : `step_response` ≥ 1,5× et assistant ≥ 1,2× plus rapides à un seul fil.

**Choix de Cédric** (2026-10-29, parmi : pilote `step_response` / tout le noyau FFT d'un coup / sauter à la phase 3).

**Témoins.** `tests/test_native_step.py` (72 cas face à l'étalon numpy : écart mesuré 1,7e-14 relatif sur les
réponses, 6e-16 sur les sommes ; déterminisme à 1 et 8 fils ; fenêtres hors tableau → `ValueError`) ; six
mutations du noyau (`reg`, seuils 0,5 et 3,0, 1/nfft, `y_correction`, somme) vues en échec (L-034) ; banc du
2026-10-29 sur la session de 72 s : `step_response` ×3 axes 315 ms → 185 ms à un fil (**1,70×**), 80 ms à
4 fils (3,9×), 64 ms avec tous les cœurs (**4,9×**) ; assistant 763 ms → 635 ms (**1,20×**, tout juste au seuil)
à un fil, 539 ms (1,42×) avec tous les cœurs.

**Parité sur le corpus** (`scripts/golden.py check`, 2026-10-29) : décodeur 0 différence (499 fichiers) ; assistant
**0 session aux constats différents** sur 630, durée cumulée 1879 s → 1410 s (références de `main` : numba, scipy et
décodeur Python) ; outils (20 logs) : structure identique, donc mêmes fenêtres retenues, écart relatif maximal
**2,4e-10** (boucle ouverte `loop.L`, identique à la mesure de la phase 2a : le noyau n'ajoute rien de mesurable).

**Ouvert.** W-015 ; `_seg_spectra`, `loop_spectra`, `xcorr` : phase 3.

### D-032 : phase 2 en deux temps ; scipy remplacé par numpy, noyaux des filtres en Rust, scipy et numba réservés aux tests

**Contexte.** L'inventaire du 2026-10-29 montre que scipy ne sert presque plus : Welch, CSD et cohérence sont
déjà écrits à la main avec `np.fft`, et scipy ne fournit que la fenêtre de Hann, deux lissages (`uniform_filter1d`,
`gaussian_filter`) et l'intercorrélation. numba ne reste que dans 4 noyaux de `analysis/bf_filters.py` et dans
l'ancien décodeur. Poids installés dans le `.venv` : scipy 140 Mo, llvmlite 173 Mo, numba 34 Mo (numpy 70 Mo reste).

**Décision.** Phase 2 découpée (branche `feat/rust-dsp`) :
- **2a** : scipy remplacé par `analysis/_numeric.py` (numpy seul) ; les 4 noyaux numba réécrits dans `rust/otb-dsp`
  (Rust pur) ; scipy et numba passent dans l'extra `dev` (références des tests) et ne sont plus importés par
  l'application (`tests/test_no_heavy_deps.py`) ; `decoder/core.py` (numba) n'est chargé que par `OTB_BACKEND=python`.
- **2b** (après mesure du poids des installateurs) : bibliothèque Rust de FFT, Welch, CSD, Wiener, réutilisée
  par la phase 3.
- Les anciens noyaux numba restent dans `tests/ref_bf_kernels.py`, **inchangés**, comme étalon de parité.
- Les fenêtres de Hann des deux conventions restent distinctes (périodique : `spectral`, `delay` ; symétrique :
  `step`, `loop`), comme avec scipy.

**Choix de Cédric** (2026-10-29, parmi : 2a puis 2b / tout en Rust tout de suite / 2a seulement ; ancien
décodeur : gardé pour les tests / supprimé).
Amende D-002 (numba ne sert plus qu'aux tests) sur la branche Rust seulement.

**Témoins.** `tests/test_numeric.py` (69+ cas face à scipy, écart ≈ 1e-14 relatif) ; `tests/test_native_dsp.py`
(Rust = numba : identique au bit près pour `biquad`, `run_section`, `pt1_track`, 1,2e-15 relatif pour la réponse
en fréquence) ; `scripts/golden.py check` : décodeur 0 différence sur 499 fichiers, assistant 0 session aux
constats différents sur 630, outils : 20 logs, aucune différence de structure, écart relatif maximal 2,4e-10
(boucle ouverte `loop.L`), donc `--rtol 1e-9` suffit ; mutations vues en échec (L-034).

**Poids mesuré** (Linux, 2026-10-29) : dossier de l'exécutable 206 Mo au lieu de 463 Mo.

**Ouvert.** Poids des installateurs compressés des trois systèmes ; phase 2b ; W-015.

### D-031 : migration vers Rust sur une branche d'intégration, `main` reste en Python

✏️ AMENDÉE par D-035 (méthode à partir de la phase 4 : noyau Rust autonome, sans pont PyO3 module par module).

**Contexte.** Numba impose `llvmlite` (≈ 37 % des installateurs) et une compilation au premier lancement
(W-015) ; Cédric veut des installateurs plus légers, plus de rapidité et, à terme, un binaire autonome. Le
poste de développement a maintenant `cargo` 1.99 et `gcc`.

**Décision.** Le code est réécrit en Rust, module par module, **sans régression** :
- **Branches.** `main` reste en Python, publiable à tout moment. `feat/rust-migration` est la branche
  d'intégration ; chaque étape a sa branche (`chore/rust-safety-net`, `feat/rust-decoder`…) créée depuis elle
  et fusionnée par PR vers elle, jamais vers `main`. Les correctifs de `main` y sont rapatriés par
  `git merge main`. Une seule PR finale `feat/rust-migration` → `main`, une fois tout testé et validé par Cédric.
- **Ordre.** Décodeur → noyaux de calcul (FFT, Welch, CSD, filtres Betaflight, Wiener) → analyses → assistant
  → ligne de commande → interface (bibliothèque choisie à ce moment-là ; PySide6 reste en attendant).
  Pont PyO3 + maturin, module `opentunebox._native`, commutateur `OTB_BACKEND=python|rust` pendant la transition.
- **Équivalence.** Décodeur identique au bit près ; analyses à tolérance mesurée (≈ 1e-5 relatif, FFT
  différente de scipy) ; assistant : mêmes constats sur tout le corpus (`advisor_rates.py --compare`).
  Références figées avant d'écrire du Rust (`scripts/golden.py`, stockées dans `$OTB_LOGS/golden`).
- **Systèmes.** Windows fait partie des livrables à chaque étape (CI sous Linux et Windows).
- **Comportement défini là où numba était indéfini** : une lecture hors tableau (prédicteur `MOTOR_0` dans une
  trame S/G/H, coordonnée GPS d'une trame H incomplète) rend 0 ; une session à moins de 2 champs I est ignorée ;
  une panique du Rust devient une `RuntimeError`. Les tests de parité excluent ces cas (L-032).
- Pas de repli vers numba si `_native` manque : le Rust est le moteur par défaut dès la phase 1
  (`OTB_BACKEND=python` garde numba le temps de la migration).
- Amende D-002 et D-006 **sur la branche Rust seulement** ; D-001 sera amendée pour l'interface à la dernière phase.

**Choix de Cédric** (2026-10-29, parmi : hybride / tout en Rust par étapes / tout d'un coup ; interface
PySide6 gardée en attendant ; tolérance « décodeur exact, analyses à tolérance »).
Plan complet : [migration-rust.md](migration-rust.md).

**Témoins.** `scripts/golden.py`, branche `chore/rust-safety-net`.

**Ouvert.** W-015.

---

## 2026-10-14 — Lint de la CI

### D-030 : lint par ruff limité aux règles pyflakes ; pas de formatage automatique pour l'instant

🔁 REMPLACÉE par D-035 (plus de Python à linter ; `cargo fmt` et `cargo clippy -D warnings` tiennent ce rôle, et le formatage **est** automatique).

**Contexte.** Superview (autre projet de Cédric) a une CI plus fournie qu'OpenTuneBox. En comparant, on a
remplacé `pyflakes` par `ruff`, plus rapide et extensible (formatage, règles de style, sécurité). Mesures du
2026-10-14 (ruff 0.15.0) : règles `F` (celles de pyflakes) → 0 signalement ; règles par défaut → 51 signalements
de style (E701/E702 plusieurs instructions sur une ligne, E741 noms ambigus, E401) ; `ruff format` reformaterait
87 fichiers sur 103.

**Décision.** `ruff check` avec `select = ["F"]` (`pyproject.toml`) remplace `pyflakes`, avec exactement les
mêmes contrôles : aucun code modifié. `ruff` est borné (`>=0.15,<0.18`) : une version future pourrait durcir une règle
F et casser la CI sans changement de code ; relever la borne à la main après essai. Pas de `ruff format` ni de règles de style pour l'instant : un reformatage
de 87 fichiers noierait l'historique (`git blame`) et obligerait à rebaser les branches en cours.

**Choix de Cédric** (2026-10-14, parmi : lint pyflakes seul / règles par défaut / lint + formatage).
Voir W-017 pour la suite.

## 2026-10-13 — Mémoire du chargement des logs

### D-029 : les courbes d'un `FlightLog` sont en float32 ; le temps reste en float64

**Contexte.** Chantier performance demandé par Cédric (« rapidité, robustesse, maintenance, taille,
mémoire »). Le plus gros log du corpus (`3. BBLR FB/James Godley/LOG00024.BFL`, 39 Mo, 965 000 trames)
occupait 2047 Mo au pic du chargement et 708 Mo une fois chargé, dont 360 Mo de courbes en float64.
L'interface garde jusqu'à 10 logs ouverts (`gui/state.py:MAX_LOGS`).

**Décision.**
- `log/flightlog.py:SAMPLE_DTYPE` = float32 pour toutes les courbes (gyro, consigne, termes PID, moteurs,
  debug, tension…) : 7 chiffres significatifs, bien au-delà de la résolution des capteurs loggés (gyro au
  dixième de °/s au mieux). `mode_flags` en int32 (masque 32 bits du firmware).
- `t` reste en float64 : en float32, la microseconde se perd après 16 s de vol.
- Les analyses gardent leurs calculs en float64 (`np.asarray(x, dtype=np.float64)` à l'entrée, FFT de numpy) ;
  une somme cumulée sur tout un log se fait en float64, jamais sur le tableau float32 lui-même.
- Pas float64 partout : le double de mémoire pour une précision que les capteurs n'ont pas.

**Témoins.** Plus gros log : 458 Mo une fois chargé au lieu de 708 Mo, pic du chargement 1347 Mo au lieu
de 2047 Mo (avec les copies évitées du décodeur et de `_drop_implausible`). Assistant sur les 705 sessions
du corpus : mêmes constats, mêmes gravités ; 9 sessions voient un chiffre affiché bouger d'une unité
(3e harmonique à 597 Hz au lieu de 598, raie fixe à +15 dB au lieu de +14 : valeurs sur une limite d'arrondi).

## 2026-10-13 — Clés des constats de l'assistant

### D-028 : les constats de l'assistant s'identifient par une clé stable, jamais par leur texte

**Contexte.** Relecture du code après la version 0.7.0 : plusieurs règles décidaient en relisant le titre
français ou le texte d'action d'un constat déjà produit (`x.title == "Filtrage du D peut-être trop fort"`,
`"rpm_filter_q" in action`, `title.startswith("Raie fixe")`, étapes de la stratégie de filtrage, parties du
diagnostic du bruit) ; les tests épinglaient aussi les titres. Reformuler un texte, même d'une virgule,
pouvait désactiver une règle sans erreur. Cédric a demandé la refactorisation (branche `refactor/finding-keys`).

**Décision.**
- Chaque `Finding` (`advisor/rules.py`) porte une clé `key` : identifiant stable, en anglais, **littéral** à
  la création (`Finding("dterm_noisy", "Bruit", …)`). Un même constat sur deux axes partage sa clé, l'axe
  reste dans `axis`. Les règles, le diagnostic du bruit (`noise.CONDITION_KEYS`, `DTERM_KEYS`, `GYRO_KEYS`,
  blocs par filtre), les étapes de la stratégie (`rules.FILTER_STAGES`) et les tests se réfèrent à la clé ;
  jamais au titre, au détail ni aux actions. Les textes se reformulent librement.
- Quand une règle doit savoir qu'un réglage est déjà conseillé par un constat précédent (`rpm_filter_q`,
  `dyn_notch_count` : `rules.TRACKED_SETTINGS`), le constat le déclare dans `Finding.settings` ; pas de
  recherche du nom dans les actions.
- Pas de registre central des clés : la clé vit à côté du texte qu'elle identifie ; les tests vérifient que
  chaque clé citée par une règle existe et que les clés sont des littéraux.
- `analyze` et `check_log` sont découpés en sections (`_recording_rules`, `_step_section`, `_noise_rules`,
  `_delay_rules`, `_filter_rules`, `_motor_rules`, `_slider_rules` ; `_recording_items`, `_flight_parts`,
  `_protocol_items`, `_general_items`, `_usages`) qui partagent un état (`_Analysis`, `_Check`) ; l'ordre des
  constats est inchangé.

**Témoins.** `scripts/advisor_rates.py` avant / après sur tout le corpus : mêmes constats (clé, gravité,
action) dans chaque session (comparaison du 2026-10-13, branche `refactor/finding-keys`) ;
`tests/test_advisor_rules.py` (`test_finding_keys_are_literal_and_referenced_keys_exist`,
`test_noise_report_classifies_every_key`, `test_tracked_settings_declared_on_synthetic_log`) et
`tests/test_advisor.py:test_tracked_settings_declared_on_corpus`.

## 2026-10-13 — Derniers seuils estimés (W-003)

### D-027 : raie fixe près de la fondamentale : test des deux trajets d'une tranche de gaz à l'autre

**Contexte.** W-013 : « Raie fixe » écartait toute raie à ± 8 % d'une harmonique de la vitesse moyenne en
stationnaire ; les raies écartées mêlaient raies fixes et raies moteur. Une mesure de pente (fréquence de la
raie face à la vitesse des moteurs) s'est révélée fragile quand la fondamentale est faible : témoin moteur à
0,57 en médiane, 43 % sous 0,5 (2026-10-13).

**Décision.**
- Fondamentale (± 15 % de la vitesse en stationnaire) : test des deux trajets (`rules.line_follows_motors`).
  Dans chaque tranche de gaz, hauteur de la raie au-dessus du bruit voisin à la vitesse des moteurs de la
  tranche, moins sa hauteur à f0 ; moyenne sur les tranches où les deux trajets se séparent (4 % de f0 et
  3 cases au moins). Raie fixe si la moyenne est au plus −2 dB (`RESONANCE_MOTOR_SCORE_DB`) ; raie moteur
  au-dessus, ou si moins de 3 tranches les séparent (prudence). Choix de Cédric (seuil −2 dB plutôt que 0).
- Harmoniques 2 et 3 : exclusion de ± 8 % gardée (`RESONANCE_HARMONIC_MARGIN`) : ni la pente ni les trajets
  n'y sont fiables.
- Pas « trajet moteur = f0 × vitesse / vitesse en stationnaire » : une raie moteur trouvée à quelques % de la
  vitesse en stationnaire (spectre médian étalé) est à la vitesse de chaque tranche, pas à un multiple fixe.

**Options écartées.** Seuil 0 dB (95–96 % des raies fixes reconnues, mais 11–15 % des raies moteur prises
pour fixes) ; garder l'exclusion ± 8 % sur la fondamentale.

**Témoins.** Corpus du 2026-10-13 (705 sessions) : fondamentales visibles (≥ 3 dB, 281 sessions) jugées
depuis f0 décalé de 3–8 % / 8–15 % : 93 % / 94 % reconnues raies moteur ; raies fixes de 8 dB ajoutées aux
spectres réels à la même distance : 90 % / 89 % reconnues fixes. « Raie fixe » : 45 → 59 sessions (+17, −3,
`advisor_rates.py`). `tests/test_advisor_rules.py:test_resonance_near_fundamental_two_paths`. Clôt W-013.

### D-026 : retard des filtres jugé dès 30 °/s de consigne

**Contexte.** W-003 : le seuil de 50 °/s (D-010) était une estimation. Mesure du 2026-10-13 (705 sessions) : 22
sessions entre 40 et 50 °/s, dont 15 en mode Angle, avec 422 fenêtres de réponse indicielle en médiane : des
vols de wobbles, souvent ceux du protocole du guide (mode Angle limité à 30°, consigne plafonnée vers 47 °/s ;
`Cedric/JeNo 5/MM`, `PD`, `Cedric/2026.6 RC2`). L'assistant les prenait pour des vols stationnaires et ne
jugeait pas leurs retards.

**Décision.**
- Amende D-010 : seuil de `delay.MOVING_SETPOINT_DPS` 50 → 30 °/s (95e centile de |consigne|, roll ou pitch),
  partagé par la découpe des parties du vol du guide (`period.flight_parts`). Choix de Cédric.
- Juste au-dessus des vrais vols stationnaires : 83 sessions sous 25 °/s, une seule avec 200 fenêtres de
  réponse indicielle ; 2 sessions entre 25 et 30 °/s.
- Pas de rupture physique à 50 °/s : par tranches de 8 s d'un même vol, l'écart du retard gyro à celui des
  tranches à plus de 150 °/s diminue progressivement (médiane 0,09–0,13 ms sous 50 °/s, 0,08–0,10 ms de 50 à
  100 °/s, 0,05 ms de 100 à 150 °/s) ; vol stationnaire contre vol avec mouvements du même drone (mêmes
  filtres, 18 drones) : +0,015 ms en médiane.

**Options écartées.** 40 °/s (93 sessions sous le seuil, sans gain mesuré) ; garder 50 °/s (les vols du
protocole restent sans jugement du retard).

**Témoins.** `tests/test_analysis.py` (wobble plafonné à 49 °/s compté avec mouvements) ;
[validation.md](validation.md) (« Retard des filtres et mouvements »).

### D-025 : paliers du pic de réponse selon le feedforward en service (0 en mode Angle)

**AMENDÉE par D-082** (2026-11-25) : même méthode, paliers recalculés avec le nouveau tri des fenêtres — 1,15 / 1,20
sans feedforward, 1,30 / 1,45 avec.

**Contexte.** Les paliers du pic (2026-10-05) séparaient les vols selon le F de l'en-tête. Or le firmware coupe
le feedforward en mode Angle (D-014) : 483 axes de vols en Angle avec F > 0 (50 drones) étaient comptés « avec
feedforward », leur terme F enregistré étant nul dans 480 cas. Leurs pics ressemblent à ceux des vols sans
feedforward (p90 par axe 1,19 contre 1,18 ; avec feedforward en Acro : 1,58). Ils étaient jugés avec les paliers
du feedforward (1,40 au lieu de 1,15) et pouvaient recevoir « Dépassement dû au feedforward ».

**Décision.**
- Le feedforward en service vaut 0 en mode Angle (`FlightConditions.ff_gain`), pour les règles de dépassement et
  pour `scripts/calibrate_advisor.py` ; le constat le dit (« sans feedforward (coupé par le mode Angle) »).
- Paliers recalculés avec la même méthode (conseil = p90 une voix par drone, attention = p95 par axe, arrondis à
  0,05) : sans feedforward 1,20 / 1,25 (p90 1,186, p95 1,235 ; 990 axes, 76 drones), avec feedforward
  1,60 / 1,70 (p90 1,589, p95 1,710 ; 288 axes, 47 drones). Choix de Cédric (option A).
- « Dépassement dû au feedforward » (F en service > 0 et latence sous le p10 de la taille) est gardé : sur les
  séries du corpus où seul F change (16 axes, 8 drones), +100 de F monte le pic de 0,24 et baisse la latence de
  3,6 ms (médianes), et le repère de latence reconnaît 7 des 8 dépassements que le vol à F plus bas n'a pas.

**Options écartées.** Corriger le mode Angle en gardant 1,15 / 1,25 et 1,40 / 1,55 : 21 % des sessions
signalées au lieu de 13 % (les vols en Angle passent sous des paliers calibrés sans eux) ; un troisième groupe
pour le mode Angle : son p90 par drone (1,19) est celui des vols sans feedforward.

**Témoins.** `tests/test_advisor_synthetic.py:test_step_peak_angle_mode_uses_thresholds_without_feedforward` ;
sessions signalées pour un dépassement : 13 % → 11 % (dépouillement du 2026-10-13, `docs/assistant.md`, « Pic de
réponse indicielle »).

## 2026-10-12 — Trous d'enregistrement

### D-024 : notch statique conseillé seulement si le notch dynamique ne peut pas traiter la raie

**Contexte.** « Raie fixe vers X Hz » et « Raie à X Hz encore visible après filtrage » proposaient tous un notch
statique (`gyro_notch1`) en plus de la mécanique et du notch dynamique : 112 constats sur 84 sessions du corpus.
Règle de Cédric, pilote (2026-10-13) : éviter au maximum le notch statique, sauf cas vraiment exceptionnel.

**Décision.**
- Mécanique d'abord, puis le notch dynamique (plage, nombre de notchs, ou remise en service s'il est coupé).
- Notch statique seulement si le notch dynamique ne peut pas traiter la raie (`rules.static_notch_case`) :
  boucle PID sous 2 kHz, 7 notchs déjà en service, raie au-delà de 980 Hz.
- Jamais sous 100 Hz (`rules.HOVER_LOW_LINE_HZ`) : un notch y retarde la bande où travaille le PID, et la raie
  vient souvent du réglage ; « Raie fixe » y ajoute la même mise en garde que les raies résiduelles.
- Effet sur le corpus : 0 conseil de notch statique sur les 112 constats (705 sessions).

**Options écartées.** Garder le notch statique en dernier recours dans chaque constat (contraire au choix de
Cédric) ; l'autoriser pour les raies hors de portée basse du notch dynamique, sous 40 Hz (2 constats : 7"
« CINE » à 37 Hz, « JeNo Ced » à 30 Hz ; retard dans la bande du PID).

**Témoins.** `tests/test_advisor_rules.py:test_static_notch_only_when_dynamic_notch_cannot`.

### D-023 : « Q trop haut » seulement si une raie passe ; marge de 5 % de la fréquence moteur autour des harmoniques

**Contexte.** W-012 : la forme des pics du notch dynamique (Q conseillé, coupure du notch statique) et les raies
hors moteurs du vol stationnaire retiraient encore les harmoniques ± 5 % de l'harmonique (estimation). Mesures
sur le corpus dans [assistant.md](assistant.md#notch-dynamique-2026-10-05) et « Stratégie de filtrage et vol
stationnaire ». Choix de Cédric (2026-10-13 : C1 et D1).

**Décision.**
- Forme des pics : mesure actuelle (harmoniques retirées) quand elle existe ; sinon, spectre après filtre RPM
  (`motors.line_shapes`, `rpm_settings`).
- « Notch dynamique : Q trop haut pour les raies à suivre » seulement si une raie à suivre dépasse encore
  `HARMONIC_VISIBLE_DB` dans le gyro filtré enregistré (`dyn_notch_view` : `left`). Amende la règle du
  2026-10-10 (Q calculé d'après la forme des raies), qui reste.
- Harmoniques retirées ± 5 % de la fréquence moteur, pour toutes (`motors.MOTOR_LINE_MARGIN`).

**Options écartées.** Spectre après filtre RPM pour tous les pics (hauteur +1,7 dB en médiane loin des
moteurs : Q trop bas) ; mesure hybride sans contrôle (« Q trop haut » 19 → 54 sessions, une raie ne passait
que dans 6 des 35 ajoutées) ; garder 5 % de l'harmonique (cache des raies 5 à 15 % autour des harmoniques 2
et 3, sans flanc mesuré).

**Témoins.** `tests/test_analysis.py:test_line_shapes_measure_rpm_leftovers`,
`tests/test_advisor_rules.py:test_dyn_notch_q_too_high_needs_a_passing_line`.

### D-022 : notch dynamique : réponse des notchs RPM du firmware ; `dyn_notch_min_hz` jamais relevé

**Contexte.** Deux « petites marges » d'origine « Estimation » (W-003) : les bandes des harmoniques retirées
± 5 % avant de chercher ce que le notch dynamique suit, et un gain minimal de 30 Hz pour conseiller de relever
`dyn_notch_min_hz`. Le debug FFT_FREQ de 598 sessions du corpus donne les fréquences que le firmware a
vraiment suivies ([assistant.md](assistant.md#notch-dynamique-2026-10-05)). Validé par Cédric (2026-10-13).

**Décision.**
- `motors.dyn_notch_targets` reçoit les réglages du log et multiplie chaque fenêtre par |H|² des notchs RPM du
  firmware (`motors.rpm_attenuation` : Q, poids, fondu, cadence PID) ; aucune bande retirée, sauf les images
  repliées des harmoniques quand le log est plus lent que la boucle.
- L'assistant ne propose plus de relever `dyn_notch_min_hz` pour le retard (gain mesuré ≈ 0).

**Options écartées.** Bandes ± 5 % (cachent ce que le filtre RPM laisse passer : 0,48 des fréquences du
firmware retrouvées) ; bandes sans marge (0,58, choix A de Cédric avant la mesure suivante) ; marge égale au
débordement de la fenêtre (0,50) ; filtre RPM rejoué dans le temps, avec ou sans notch statique et LPF1 (0,60 ;
W-011) ; seuil en retard gagné au lieu des 30 Hz (ne se déclencherait presque jamais).

**Témoins.** `tests/test_analysis.py:test_dyn_notch_targets_follow_rpm_weights`,
`tests/test_advisor_rules.py:test_rule_constants_snapshot` ; L-028.

### D-021 : pas des curseurs conseillés, choisis par Cédric

**Contexte.** Les conseils à un seul vol de l'assistant proposaient presque toujours ±0,1 (« Estimation »,
W-003), alors que le Diagnostic du réglage suit les séries de vols [S32] : un pas de 0,1 ne se lit pas sur la
réponse indicielle, les séries avancent par 0,2. Règle de Cédric, pilote (2026-10-12).

**Décision.**
- Filtres : curseur « filtre gyro » par pas de 0,2 ; curseur « filtre D » par 0,1 à 0,2 (relèvement prévu
  par le simulateur, 0,1 par vol d'essai).
- Damping par 0,2 pour dégrossir (fort dépassement), 0,1 en réglage fin (léger dépassement, réponse molle,
  marge faible, propwash) ; écart roll / pitch par 0,2 (Pitch Damping, Pitch Tracking) ; Master par 0,2
  pour trouver la limite, puis retour de 0,1 à 0,2 (oscillations, marge faible).
- Feedforward : aucun pas chiffré dans un conseil à un seul vol, renvoi à la série du guide (curseur
  « Réponse des sticks » 0, 0,5, 1… en Acro, `rules.FF_SERIES_ADVICE`).
- Tracking (P et I) n'est plus conseillé seul.

**Options écartées.** Garder 0,1 partout (illisible sur la réponse, contraire aux séries) ; pas fonction de
l'écart au seuil (pas de mesure qui le justifie) ; ±0,5 de feedforward dans un conseil isolé (la série
compare le retard consigne → gyro, un vol seul ne dit pas où s'arrêter) ; Tracking −0,1 / +0,1 (correction
secondaire, absente de la méthode de Cédric).

**Témoins.** `tests/test_advisor_synthetic.py:test_slider_steps` ;
[assistant.md](assistant.md#pas-des-curseurs-2026-10-12).

### D-020 : aucune fenêtre d'analyse ne chevauche un trou du log

**Contexte.** Une pause d'écriture de la carte SD ou de la mémoire flash laisse un trou dans le log (7,5 %
des sessions du corpus). Les analyses recollaient le vol de part et d'autre : chaque raccord est un saut du
signal, donc un faux bruit, jusqu'à +18 dB entre 80 et 500 Hz sur un vol de wobbles (W-010, L-027,
`docs/assistant.md`, « Trous dans l'enregistrement »). Choix C de Cédric (2026-10-12) : prévenir d'abord le
pilote, puis corriger les analyses.

**Décision.**
- `FlightLog.breaks(sel)` donne les raccords de la période (indices comptés depuis son début ; trou = pas de
  temps de plus de 4 périodes, comme `FlightLog.segments` et `FlightLog.recording_gaps`).
- Toute fonction de `analysis/` qui découpe des fenêtres prend un argument `breaks` (défaut `None` : aucun
  raccord, résultat inchangé) et écarte les fenêtres qui en chevauchent un (`spectral.clear_of_breaks`) :
  spectres, fréquence × gaz, raies et bruit moteur, notch dynamique, réponse indicielle et boucle ouverte,
  retards des filtres, simulateur de filtres. Les appelants (assistant, guide, outils, scripts) passent
  `log.breaks(sel)` ; `whatif_spectrum` et `compute_delays`, qui reçoivent le log, le font eux-mêmes.
- Fréquence × temps garde ses colonnes à leur place et laisse vides celles qui chevauchent un trou.
- Le retard des filtres (`phase_delay_ms`) calcule lui-même ses spectres croisés, avec les fenêtres de
  `scipy.signal.csd` (résultat identique sans raccord, test à 1e-9 près).

**Options écartées.** Découper le log en morceaux et moyenner les résultats : chaque analyse aurait sa
propre règle de fusion. Remplir les trous (interpolation) : invente des données. Passer les raccords
dans les tableaux eux-mêmes (sous-classe de `ndarray`) : perdus au premier calcul.

**Limite.** L'intercorrélation consigne → gyro (`xcorr_delay_ms`) porte sur tout le signal : un trou n'y
décale qu'un petit morceau, effet négligeable. Un vol très haché (un trou par seconde) n'a presque plus de
fenêtre de 2 s : la réponse indicielle le dit (« Pas assez de mouvements de manche »).

**Témoins.** `tests/test_gaps.py` (spectre et réponse indicielle d'un signal troué égaux à ceux du signal
entier, faux bruit de plus de 5 dB sans les raccords) ; passage de l'assistant sur le corpus
(`scripts/advisor_rates.py`) : aucune session sans trou ne change, 31 des 47 sessions trouées perdent de faux
constats (13 « Oscillations basses fréquences », 5 « Gyro filtré bruité »…).

## 2026-10-12 — Vol chirp mis de côté

### D-019 : le vol chirp de Betaflight n'est pas conseillé, OpenTuneBox suit la méthode des séries de vols

**Contexte.** W-004 proposait de conseiller un vol chirp quand le pic de sensibilité tombe au bord de la
bande mesurée. Le constat essayé se déclenchait sur 44 % des sessions du corpus (310 sur 705), avec des
conseils de réglage du chirp peu sûrs. Cédric : « mettre de côté la fonction CHIRP […] encore en mode
expérimental » et « se concentrer sur la méthode […] ». Vérifié le 2026-10-12 sur le dépôt de
Betaflight : le chirp n'y est pas étiqueté expérimental (PR #13105, « MAJOR FEATURE », fusionnée le
2025-01-21), mais il n'est pas dans le firmware de base (option de compilation `chirp` / `USE_CHIRP`) ; ses
canaux de debug ont changé en avril 2026 (PR #15113) et son point d'injection en mode angle en
septembre 2026 (PR #15196) ; l'outil d'analyse qui en tire un réglage est encore en préparation (PR #15196) ;
un mainteneur le juge « not a basic user option » (ticket #15012).

**Décision.**
- Ni l'assistant, ni le guide intégré, ni le guide utilisateur ne conseillent de vol chirp ni ne disent
  comment le régler. Pour faire aller la mesure plus loin, ils conseillent des wobbles plus rapides et plus
  secs (protocole des séries de vols, [S33] de `docs/spec-methodes.md`).
- La mesure reste capable de lire un log chirp (`log/flightlog.py:FlightLog.chirp_excitation`, D-012) :
  la retirer dégraderait la mesure des vols chirp déjà enregistrés, sans rien gagner.
- Pas de constat séparé « marge mesurée en partie » : 44 % des sessions, il deviendrait du bruit ; l'action
  s'ajoute au seul constat « Marge de stabilité faible » quand son pic est au bord.

**Témoins.** `tests/test_advisor_synthetic.py:test_stability_margin_rules` (action des wobbles, aucun mot
« chirp ») ; taux du constat essayé mesuré par `scripts/advisor_rates.py` le 2026-10-12.

**Ouvert.** W-004.

## 2026-10-12 — Organisation de la documentation

### D-018 : décisions, leçons et travaux reportés ont chacun leur registre numéroté

**Contexte.** Les décisions étaient réparties entre « Décisions fixées » (`AGENTS.md`) et le tableau de
`docs/architecture.md`, sans les options écartées ni les preuves ; les pièges ne gardaient que leur règle ;
le travail laissé de côté tenait en une ligne (« Hors périmètre »). Gabarits repris d'un autre dépôt de
Cédric et adaptés (sans suivi par sprints, sans format de commit à numéros).

**Décision.**
- Décisions → ce journal (`D-xxx`) ; pièges → [lecons.md](lecons.md) (`L-xxx`) ; travail laissé de côté →
  [travaux-reportes.md](travaux-reportes.md) (`W-xxx`). Une information n'est écrite qu'à un endroit.
- `AGENTS.md` garde la règle courte suivie de son numéro ; `docs/architecture.md` renvoie ici.
- Pas de numéros dans les sujets de commit (convention de `CONTRIBUTING.md` inchangée) : la demande de
  fusion cite les numéros dans sa description.
- Pas les travaux reportés dans les tickets GitHub : le fichier se lit hors ligne, avec le code ; les
  tickets restent pour les problèmes signalés par les utilisateurs.

**Témoins.** `tests/test_docs.py` : `test_register_ids_unique`, `test_register_ids_cited_exist`.

### D-017 : deux fréquences minimales du log, filtres et séries PID

**Contexte.** Un vol de série PID enregistré à 1 kHz était signalé « trop lent » alors qu'il suffit pour
la réponse indicielle ; le minimum de 2 kHz ne vaut que pour voir le bruit des filtres.

**Décision.**
- Filtres : au moins 2 kHz pour un gyro à 8 kHz, 1,6 kHz à 3,2 kHz, plus pour les petits drones
  (`rules.log_rate_needed`). Séries PID : 1 kHz (`rules.pid_log_rate_needed`).
- Le guide range les logs par rôle (`log_check.FILTERS` / `PID`) : chaque case ne juge que ceux de sa phase.

**Témoins.** Commit `4787496` ; leçon L-008 (repliement au-dessus de fs/2).

## 2026-10-11 — Échelle des spectres

### D-016 : les spectres s'affichent sur l'échelle de PIDtoolbox, les calculs restent en densité physique

**Contexte.** Les repères courants (gyro sous −30 dB, terme D sous −10 dB) sont lus sur PIDtoolbox, dont
l'échelle est 6,5 dB plus basse que la densité de `scipy.signal.welch`.

**Décision.**
- `analysis/` calcule la densité physique ; l'interface convertit dans `compute` des outils
  (`spectral.to_ptb_db`) ; l'assistant convertit lui-même (`rules.band_peak_ptb_db`).
- Pas de conversion dans `analysis/` : les calculs physiques (bruit, simulateur) resteraient faux.

**Témoins.** `spectral.PTB_PSD_OFFSET_DB` = 6,5 dB mesuré sur les mêmes logs que PIDtoolbox
([validation.md](validation.md#recoupement-avec-pidtoolbox-pro-v094-mêmes-logs)) ; commits `7642145`,
`2e4a602`. Piège associé : L-009.

## 2026-10-10 — Diagnostic des séries PID, onglets

### D-015 : un outil = un onglet de la fenêtre principale

**Contexte.** Les outils s'ouvraient chacun dans une fenêtre séparée (`00767e9`) ; une page par outil
dans la fenêtre principale évite d'ouvrir de nouvelles fenêtres, et un outil caché ne recalcule qu'à son
affichage.

**Décision.** Chaque outil s'affiche dans une page de la fenêtre principale
(`gui/main_window.py:MainWindow.open_tool`), boutons de la barre = onglets ; pas de fenêtre séparée. Un outil
hérite de `gui/tools/base.py:ToolWindow` et s'enregistre dans `gui/tools/__init__.py:REGISTRY`.

**Témoins.** Commit `346e52c`, demande de fusion n° 28 (`feat/tool-tabs`).

✏️ AMENDÉE par D-060 (une entrée de navigation est un outil ou un **espace** de plusieurs vues, dans un rail à gauche ;
pas de fenêtre séparée et une vue cachée ne calcule pas, comme ici).

### D-014 : le feedforward n'est considéré comme nul qu'en mode Angle

**Contexte.** Le firmware force le feedforward à 0 en mode Angle, pas en Horizon ; une série jugée en
Horizon aurait été faussée par le feedforward.

**Décision.**
- Masque `flightlog.ANGLE_MODE_MASK` pour savoir si le feedforward est nul ; `ANGLE_HORIZON_MASK` reste
  l'instrument de la boucle ouverte (D-012).
- Seuils du diagnostic tirés de 10 séries Master du corpus, choix des pilotes relus dans les vols suivants.

**Témoins.** Commit `3e75e1d` ; piège associé : L-005, incertitude : L-012.

## 2026-10-05 — Linux, boucle ouverte, assistant

### D-013 : Linux, fenêtres d'ouverture et d'enregistrement par le portail XDG

**Contexte.** Sans portail, Qt affiche sa propre fenêtre de fichiers, dont les icônes du thème du bureau
étaient invisibles en thème clair sur un bureau sombre ; le portail donne la fenêtre du bureau, quel qu'il soit.

**Décision.**
- `QT_QPA_PLATFORMTHEME=xdgdesktopportal` dans `gui/app.py`, quel que soit le bureau. Choix validé par Cédric.
- Sous KDE, la fenêtre du portail porte le logo de KDE : accepté, le portail ne permet pas de le changer.

**Témoins.** Commit `8dfb661`. Piège associé : L-017.

### D-012 : boucle ouverte mesurée par estimateur instrumental

**Contexte.** L = T/(1 − T) suppose une boucle simple ; Betaflight dérive le gyro (D) et pousse la
consigne (FF), la formule est donc fausse.

**Décision.**
- L = G·(Ce + Cd) mesurée à partir des termes P, I, D, F loggés (`analysis/loop.py`).
- G par estimateur instrumental : consigne, ou `rcCommand` en mode Angle / Horizon, plus l'excitation du
  debug CHIRP. Pas S(u, gyro)/S(u, u) : biaisé en boucle fermée.
- T/(1 − T) reste un repli marqué « approché ». L'onglet Autotune du configurateur n'est pas une source.

**Témoins.** Commit `7536105` ; méthode dans [spec-methodes.md](spec-methodes.md). Piège associé : L-010.

**Ouvert.** W-004 (bande mesurable limitée à 20–40 Hz).

### D-011 : seuils de l'assistant = 90e percentile du corpus par taille, origine notée

**Contexte.** Un constat de l'assistant doit dire « plus que la plupart des drones de cette taille », et
chaque valeur doit pouvoir être justifiée à Cédric.

**Décision.**
- Seuil = 90e percentile du corpus par taille de drone, recalculé avec `scripts/calibrate_advisor.py`.
- Une voix par drone quand quelques drones dominent une taille ; moins de ~20 drones → seuil commun ou
  profil provisoire.
- Chaque seuil a une origine notée dans [assistant.md](assistant.md) (maître de ces valeurs) ; méthode dans
  la compétence `assistant-seuils`.

**Témoins.** Seuils mécaniques calibrés sur 90 drones (commit `95e41d6`) ; taux de déclenchement par `scripts/advisor_rates.py`. Piège associé : L-013.

**Ouvert.** W-002 (profils provisoires), W-003 (seuils « Estimation »).

### D-010 : retard des filtres jugé seulement sur un vol avec des mouvements

✏️ AMENDÉE par D-026 (seuil 30 °/s).

**Contexte.** En vol stationnaire, les filtres dynamiques ne travaillent qu'à un régime : le retard mesuré
ne représente pas le vol réel.

**Décision.** Retard représentatif seulement si la consigne dépasse 50 °/s au moins 5 % du temps
(`delay.MOVING_SETPOINT_DPS`). Vol stationnaire = identifier le bruit ; vol avec mouvements = juger le retard.

**Témoins.** Les vols stationnaires du corpus restent sous 25 °/s (commit `14010b4`) ; en stationnaire,
l'intercorrélation se trompe d'au moins 0,5 ms une fois sur deux ([validation.md](validation.md)). Piège
associé : L-007.

## 2026-10-01 — Socle du projet

### D-009 : style Qt Fusion imposé sur tous les systèmes

🔁 REMPLACÉE par D-035 (l'interface est en egui depuis la 0.9.0 : il n'y a plus de Qt).

**Contexte.** Sous Windows 11, les textes des boutons étaient gris et à peine lisibles (L-014).

**Décision.**
- Style « Fusion » partout (`gui/theme.py`), palettes complètes claire et sombre.
- Les défauts de Fusion sont corrigés par la feuille de style globale plutôt qu'en revenant au style natif
  (L-015).

**Témoins.** Commits `861660d` (couleurs d'accent à contraste ≥ 4,5:1 dans les deux thèmes), `d000968`.

### D-008 : simulateur de filtres en méthode relative

✏️ AMENDÉE par D-058 (dans la page Filtres, le rapport vient du rejeu de la chaîne, et non plus des réponses |H|²), puis par
D-059 (l'assistant aussi).

**Contexte.** Le simulateur doit montrer l'effet d'un changement de filtres sur le spectre loggé, et
retomber exactement sur ce spectre quand rien ne change.

**Décision.**
- `bf_filters.whatif_spectrum` : spectre loggé × |H_modifié|²/|H_actuel|² à la cadence de la boucle PID,
  fenêtre par fenêtre ; exacte quand rien ne change.
- La simulation temporelle `simulate_gyro` reste un aperçu (L-011).

**Témoins.** Commit `adc63c9`.

### D-007 : retards mesurés par la phase, pas par l'intercorrélation

**Contexte.** L'intercorrélation donnait des retards instables, dominés par le bruit haute fréquence (L-006).

**Décision.** Méthode de phase entre 10 et 100 Hz, pondérée par la cohérence (`delay.phase_delay_ms`).

**Témoins.** Retard des filtres gyro 1,2 ms contre 1,25 ms pour PIDtoolbox sur le même log
([validation.md](validation.md#recoupement-avec-pidtoolbox-pro-v094-mêmes-logs)).

### D-006 : aucune dépendance à compiler

✏️ AMENDÉE par D-031 et D-035 (sur `feat/rust-migration` seulement).

**Contexte.** Pas de compilateur C sur le poste de développement ; les exécutables se fabriquent sur trois
systèmes.

**Décision.** Seulement des paquets livrés précompilés (numba, numpy, scipy, PySide6…).

**Témoins.** `pyproject.toml` ; fabrication des trois installateurs par `construire.yml`.

### D-005 : exception à D-004, icônes Lucide

**Contexte.** Barre d'outils à icônes de la fenêtre principale.

**Décision.** Amende D-004 : les icônes [Lucide](https://lucide.dev) (licence ISC) sont permises dans
`gui/assets/icons/`, avec leur fichier de licence. Validé par Cédric. Aucune autre exception.

**Témoins.** Commit `3c7c0c2`.

### D-004 : aucune copie de code de PIDtoolbox ni d'autres projets

✏️ AMENDÉE par D-005.

**Contexte.** PIDtoolbox est un binaire chiffré ; le projet est privé, tous droits réservés.

**Décision.** Tout est réimplémenté à partir des formules du firmware Betaflight et de méthodes publiées
([spec-betaflight.md](spec-betaflight.md), [spec-methodes.md](spec-methodes.md)). PIDtoolbox sert de
référence de résultats, pas de source.

**Témoins.** Recoupement chiffré dans [validation.md](validation.md).

### D-003 : assistant à règles, 100 % hors ligne

**Contexte.** Les logs de vol restent chez le pilote ; un conseil doit pouvoir s'expliquer mesure par mesure.

**Décision.** Règles intégrées (`advisor/`), aucune API, aucune donnée envoyée. Pas d'IA en ligne.

**Témoins.** Aucune dépendance réseau dans `pyproject.toml`.

### D-002 : décodeur Python accéléré par numba

🔁 REMPLACÉE par D-035 (plus de Python ni de numba dans le dépôt depuis la phase 7-5 : le décodeur est en Rust).

✏️ AMENDÉE par D-031 et D-035 (sur `feat/rust-migration` seulement).

**Contexte.** `blackbox_decode` n'existe pas sur toutes les plateformes et a ses propres défauts (L-001).

**Décision.** Décodeur Python accéléré par numba (`cache=True`), aucun binaire externe ;
`blackbox_decode` ne sert qu'à valider (`scripts/validate_corpus.py`).

**Témoins.** [validation.md](validation.md#décodeur) ; commit `28c709c`.

### D-001 : Python + PySide6 + pyqtgraph, projet privé, interface en français

✏️ AMENDÉE par D-035 (la technologie, à la phase 6) et par D-043 (le dépôt est public sous GPL 3 ; le reste reste en vigueur).

**Contexte.** Un seul code pour Windows, macOS et Linux, maintenable par un débutant aidé d'agents IA,
pour un pilote francophone.

**Décision.**
- Python + PySide6 (Qt) + pyqtgraph : courbes fluides sur plus de 100 000 points.
- Projet privé, tous droits réservés, pas de licence libre.
- Interface, messages et documentation en français ; code et identifiants en anglais ; commentaires en français.

**Témoins.** Trois installateurs fabriqués et essayés par `construire.yml`.
