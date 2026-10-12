# Travaux reportés

Ce qu'on a **vu et laissé de côté** : un défaut relevé en relecture, une limite assumée pour livrer, une
idée de Cédric à reprendre plus tard, un sujet hors périmètre. Sans ce fichier, une idée écartée revient
sans cesse, ou tombe dans l'oubli alors qu'elle était prévue. Registre jumeau du
[journal des décisions](decisions.md) (`D-xxx`) et des [leçons](lecons.md) (`L-xxx`).

## Protocole

**Quand ouvrir un `W-xxx`** :
- une relecture relève un défaut **réel** mais hors du sujet de la branche en cours ;
- une limite est **assumée** pour livrer et doit être reprise à une condition précise ;
- Cédric demande quelque chose qui ne se fait pas maintenant ;
- un sujet entier est repoussé ou hors périmètre.

**Ne pas ouvrir** : un piège de débogage (→ `L-xxx`), un choix déjà tranché (→ `D-xxx`), une tâche de la
branche en cours (elle se fait). Les problèmes signalés par les utilisateurs restent dans les tickets
GitHub (D-018).

**Numérotation** : une seule série `W-001`, `W-002`…, jamais réutilisée ; relire le dernier numéro avant
d'en prendre un.

**Ordre** : regroupés par origine, la plus récente en bas, sous un en-tête
`## Origine : <relecture, retour de Cédric, mesure…> (AAAA-MM-JJ)`.

**Statut** : implicite = ouvert (relevé, rien de décidé). Sinon, une marque en tête de l'entrée :

| Marque | Sens |
|---|---|
| ⏸ BLOQUÉ — condition | retenu, attend une condition écrite |
| 🔧 EN COURS — branche | une branche `type/description-courte` existe |
| ✅ CLOS (date) — commit, D-xxx | fait et vérifié ; l'entrée est barrée |
| 🚫 SANS SUITE (date) — raison | abandonné : caduc, sans objet, coûte plus qu'il ne rapporte ; barré |
| ⏩ REPRIS par W-yyy / D-yyy | repris sous un autre numéro ; barré |

Une entrée close **reste** dans le fichier, barrée, avec sa raison : elle empêche de rouvrir une question
déjà réglée. Ne jamais supprimer un `W-xxx`.

**Format**

```markdown
- **W-0XX** — <le constat en une phrase : ce qui manque ou ce qui est faux>.
  **Pourquoi reporté** : hors périmètre / limite assumée / attend D-xxx / demandé plus tard par Cédric.
  **Risque si on n'y touche pas** : ce que le pilote ou le code subit, et à quelle fréquence.
  **Reprise** : la condition qui le rend urgent, ou le geste qui le déclenche.
  **Piste** : la correction envisagée. (facultatif)
  [`fichier.py:fonction`]
```

Règles d'écriture :
- Le constat est **vérifié** (lu dans le code, mesuré) ; une supposition se dit telle.
- Toujours l'**ancre** `fichier:fonction` (ou un document et sa section) : sans elle, l'entrée ne se
  retrouve plus après une refonte.
- Un arbitrage de Cédric se note sous l'entrée, daté : `**Arbitrage de Cédric (AAAA-MM-JJ)** — …` ; s'il
  fixe un choix durable, ouvrir un `D-xxx`.
- Commencer le travail = créer la branche et passer l'entrée « EN COURS » ; la demande de fusion cite le
  `W-xxx` dans sa description.

## Index

| ID | Sujet | Statut |
|---|---|---|
| W-010 | Analyses recollées de part et d'autre des trous du log | clos |
| W-009 | Extrait de vrai log Blackbox versionné pour la CI | clos |
| W-008 | Couleurs en dur dans `gui/` : liste d'exceptions à valider | clos |
| W-007 | Tests du corpus en échec quand un log a été déplacé | clos |
| W-006 | Écart de 2 dB sous 100 Hz avec PIDtoolbox sur un vol | sans suite |
| W-005 | Référence de l'erreur aléatoire non vérifiée à la page | ouvert |
| W-004 | Boucle ouverte mesurable seulement jusqu'à 20–40 Hz | bloqué |
| W-003 | Seuils de l'assistant d'origine « Estimation » | ouvert |
| W-002 | Profils provisoires whoop, 2–3,5" et 8"+ | bloqué |
| W-001 | Logs KISS et INAV | ouvert |

## Origine : périmètre initial (2026-10-01)

- **W-001** — OpenTuneBox ne lit que les logs Betaflight ; les logs KISS (`.TXT`) et INAV ne sont pas décodés.
  **Pourquoi reporté** : hors périmètre ; l'objectif est d'abord de couvrir PIDtoolbox sur Betaflight.
  **Risque si on n'y touche pas** : un pilote KISS ou INAV ne peut pas utiliser l'application.
  **Reprise** : quand Cédric le demande, ou quand un pilote du corpus vole en INAV.
  **Piste** : un décodeur par format dans `decoder/`, qui produit la même `DecodedSession`.
  [`decoder/`, [architecture.md](architecture.md#hors-périmètre-pour-linstant)]

## Origine : calibration de l'assistant (2026-10-05)

- **W-002** — Les profils whoop, 2–3,5" et 8"+ reposent sur trop peu de drones : seuils provisoires.
  ⏸ **BLOQUÉ — attendre environ 20 drones par taille dans le corpus** (D-011).
  **Pourquoi reporté** : limite assumée ; le constat « Seuils provisoires pour cette taille » prévient le pilote.
  **Risque si on n'y touche pas** : conseils moins sûrs sur ces tailles.
  **Reprise** : à chaque ajout de logs au corpus, recompter les drones par taille
  (`scripts/corpus_labels.py`) ; recalibrer avec la compétence `assistant-seuils`.
  [`advisor/profiles.py:CraftProfile`, [assistant.md](assistant.md#calibration-2026-10-05)]

- **W-003** — Plusieurs seuils de l'assistant ont l'origine « Estimation » : ni calibrés ni sourcés.
  **Pourquoi reporté** : choisis en écrivant les règles ; la documentation les marque « à vérifier en priorité ».
  **Risque si on n'y touche pas** : un constat déclenché trop ou trop peu souvent, sans justification.
  **Reprise** : avant toute nouvelle règle de l'assistant ; un seuil à la fois, avec
  `scripts/advisor_rates.py` pour son taux de déclenchement.
  **Piste** : calibrer sur le corpus, ou trouver une source (compétence `assistant-seuils`).
  [[assistant.md](assistant.md#origine-des-seuils), lignes « Estimation »]
  **Avancement** : 2026-10-12, raie moteur « visible » calibrée sur le corpus (mesure là où il n'y a aucune
  raie) et passée de 3 à 4 dB avec l'accord de Cédric (`rules.HARMONIC_VISIBLE_DB`, branche
  feat/calibrate-harmonic-visible) ; raie hors moteurs du vol stationnaire (`rules.HOVER_LINE_MIN_DB`)
  gardée à 6 dB, origine vérifiée par simulation.
  Réponse molle : pic < 1,01 (p10 du corpus) au lieu de 0,98 ; minimum de 20 fenêtres gardé, « mesure
  moins sûre » sous 200 sur les constats du pic (précision mesurée sur le corpus).
  Notch statique : coupure calculée d'après la raie (même méthode que le Q du notch dynamique), repli 0,7.
  Vagues du Diagnostic du réglage : 2,5 gardé, mesuré à vide sur le corpus et par injection ; pas lues sous
  200 fenêtres (branche feat/calibrate-step-waves). « D dominant » au croisement : 1,0 gardé, mesuré sur
  les séries Damping du corpus (branche feat/calibrate-d-dominant). « Taille par le nom » mis de côté par
  Cédric (2026-10-12), repris à sa demande. Trames corrompues : compteur remplacé par le constat « Trous dans
  l'enregistrement » (branche feat/calibrate-corrupt-frames), défaut des analyses ouvert en W-010. Restent,
  dans l'ordre choisi avec Cédric : taille pour l'objectif de retard, pas des curseurs, petites marges.

- **W-004** — La boucle ouverte mesurée s'arrête vers 20–40 Hz (énergie du manche) : le pic de
  sensibilité tombe au bord de la bande pour 57 axes sur 133 et n'est alors qu'un minorant.
  ⏸ **BLOQUÉ — le chirp de Betaflight entre dans le firmware de base, avec une méthode d'analyse publiée** (D-019).
  **Pourquoi reporté** : limite de la mesure, signalée à l'écran (`sensitivity_peak_at_edge`) ; depuis le
  2026-10-12, le constat « Marge de stabilité faible » conseille des wobbles plus rapides et plus secs quand
  son pic est au bord (`advisor/rules.py:_edge_actions`). Le vol chirp, seule excitation large bande à la
  portée du pilote, est mis de côté (D-019).
  **Risque si on n'y touche pas** : pic de sensibilité sous-estimé sur les vols sans excitation large bande.
  **Reprise** : quand le chirp est compilé dans le firmware de base et qu'une méthode publiée (Betaflight,
  Brian White) dit comment le voler.
  **Piste** : conseiller un vol chirp quand le pic est au bord de la bande ; constat séparé essayé le
  2026-10-12 : 44 % des sessions du corpus, à réserver aux marges proches du seuil.
  [`analysis/loop.py` (`sensitivity_peak_at_edge`), [validation.md](validation.md#autres-contrôles), D-012]

- **W-005** — La formule de l'erreur aléatoire d'une fonction de transfert est attribuée à Bendat et
  Piersol (*Random Data*, chap. 9) sans vérification à la page.
  **Pourquoi reporté** : formule classique, résultats recoupés par simulation ; source non relue.
  **Risque si on n'y touche pas** : citation inexacte dans la documentation, pas de calcul faux connu.
  **Reprise** : quand l'ouvrage est disponible.
  [[spec-methodes.md](spec-methodes.md), boucle ouverte, étape 5]

## Origine : recoupement avec PIDtoolbox (2026-10-11)

- ~~**W-006** — Sur un vol, notre spectre est 2 dB plus haut que celui de PIDtoolbox sous 100 Hz.~~
  🚫 **SANS SUITE (2026-10-11)** — écart absent sur les logs Fred28 et Apri : propre à ce vol ; ni la
  moyenne en dB ni la médiane des fenêtres ne l'expliquent.
  [[validation.md](validation.md#recoupement-avec-pidtoolbox-pro-v094-mêmes-logs)]

## Origine : relecture de la branche docs/project-registers (2026-10-12)

- ~~**W-007** — 6 tests marqués `corpus` échouent (`FileNotFoundError`) au lieu d'être ignorés : ils ne
  vérifient que l'existence du dossier des logs, pas celle de chaque fichier ; des logs ont été déplacés
  (`Cedric/Air65/1.Stock.bbl`, `MM.bbl`, `FF.bbl`, `D.bbl` ; dossier `1. PTB/Allykill` absent).~~
  ✅ **CLOS (2026-10-12)** — logs supprimés par Cédric, remplacés par des logs du corpus aux mêmes
  propriétés (vérifiées sur tout le corpus) : `Cedric/Air65/stock.bbl`, `d.bbl`, `m1.bbl`, `m2.bbl`,
  `1. PTB/Apri/btfl_008.bbl`, `1. PTB/cheussure/FF0.0_0.5_1.0.bbl` ; un log absent fait désormais ignorer
  le test en le nommant (`tests/corpus_paths.py:corpus_log`). Branche fix/corpus-tests-missing-logs, L-023.
  **Pourquoi reporté** : hors du sujet de la branche ; échec identique sur `main` (`54d6cb8`).
  **Risque si on n'y touche pas** : `pytest` rouge à chaque commit, ce qui masque un vrai échec.
  **Reprise** : avant le prochain commit de code ; demander à Cédric où sont passés ces logs.
  **Piste** : remettre les chemins à jour, et ignorer un test dont le fichier précis est absent
  (`pytest.skip`) plutôt que d'échouer.
  [`tests/test_log_check.py`, `tests/test_advisor.py:test_profiles_detected_by_motor_speed`,
  `tests/test_advisor.py:test_rpm_and_dyn_notch_q_steps`, `tests/test_decoder.py:test_real_log_loads`]

## Origine : chantier des tests anti-régression, branche test/foundations (2026-10-12)

- ~~**W-008** — Dix endroits de `gui/` écrivent une couleur en dur alors que L-016 demande celles du thème.~~
  ✅ **CLOS (2026-10-12)** — plus aucune exception : toutes les couleurs fixes vivent dans `gui/theme.py`
  (`PLOT_COLORS` courbes et repères, `SEVERITY_COLORS` badges, `ON_ACCENT`, `OVERLAY_FILL`, `NOTCH_TRACE`,
  `ROW_HIGHLIGHT`, fond de légende par thème) ; les textes verts / rouges des tableaux (curseurs PID, retards du
  simulateur) et le titre d'alerte du simulateur passent par `theme.accent`, le contour des points de série
  prend la couleur du texte du thème. `tests/test_gui_contracts.py:test_every_color_lives_in_theme` refuse
  désormais tout code couleur, nom CSS ou couleur numérique hors de `theme.py` (28 occurrences trouvées
  avant la correction, 0 après). Vérifié par captures des sept pages dans les deux thèmes sur
  `Cedric/JeNo 5/FINAL.bbl`. Branche fix/gui-theme-colors.
  Badges de gravité de l'assistant (`SEV_STYLE` de `gui/tools/advisor_panel.py`, aujourd'hui `gui/theme.py:SEVERITY_COLORS` ; fond coloré
  et texte blanc), couleurs des courbes du simulateur (`gui/tools/filter_sim.py:COL_MOD`, `COL_PF`, `COL_SIM_CUR`,
  titre rouge et cellules vertes / rouges du tableau), écarts verts / rouges du tableau des curseurs
  (`gui/tools/pid_slider.py`), lignes de repère de la réponse indicielle (`gui/tools/step_window.py`), traits
  blancs et cyan du Fréquence × temps (`gui/tools/freq_time.py`), texte blanc du coin du Fréquence × gaz
  (`gui/tools/freq_throttle.py`), trait noir des points de série (`gui/tools/_step_common.py`), courbe grise de
  référence du Spectre (`gui/tools/spectral_window.py`), icône blanche du bouton principal (`gui/main_window.py`).
  Vérifié par `grep` le 2026-10-12 ; la plupart sont des couleurs de courbes ou de badges sur fond coloré,
  pas des textes sur le fond du panneau : elles ne cassent sans doute pas un thème, mais rien ne le garantit.
  **Pourquoi reporté** : décider lesquelles sont légitimes relève de Cédric (effet visuel voulu) ; un test
  qui interdirait tout code couleur hors de `theme.py` sans liste d'exceptions casserait l'interface actuelle.
  **Risque si on n'y touche pas** : une couleur de texte en dur ajoutée plus tard passe inaperçue jusqu'à la
  relecture visuelle des deux thèmes (L-016 déjà payé une fois).
  **Reprise** : Cédric relit la liste ci-dessus dans les deux thèmes et dit ce qui reste ; ensuite, test dans
  `tests/test_gui_contracts.py` qui balaye `gui/` et n'accepte que les exceptions listées.
  **Piste** : déplacer les couleurs légitimes dans `theme.ACCENTS` ou un dictionnaire de `theme.py` (une par
  thème si besoin), et faire échouer le test sur tout code `#rrggbb` ou nom de couleur ailleurs.
  [`src/opentunebox/gui/theme.py:accent_style`, `tests/test_gui_contracts.py`]

- ~~**W-009** — La CI ne décode jamais un vrai fichier Blackbox.~~
  ✅ **CLOS (2026-10-12)** — `tests/data/jeno5_5s.bbl` : 365 702 octets, les 5,1 premières secondes de la 1re
  session de `Cedric/JeNo 5/FINAL.bbl` (sans GPS), coupées sur une trame I avec un événement de fin de log
  ajouté ; décodage identique au début de la session complète et à `blackbox_decode` (statut `ok`).
  `tests/test_real_log.py` fige l'empreinte du fichier, l'en-tête, la première et la dernière trame, les
  statistiques, les unités physiques et la ligne de commande ; la CI le passe aussi avec `NUMBA_DISABLE_JIT=1`.
  Branche test/real-log-excerpt. Tous les tests sur de vrais logs sont marqués `corpus` et ignorés sur GitHub (L-025) ; seuls des fichiers fabriqués
  (`tests/bbl_factory.py`) y passent, avec les seuls encodages que la fabrique écrit.
  **Pourquoi reporté** : versionner un extrait d'un log de Cédric (quelques secondes d'un vol, de l'ordre de
  quelques centaines de kilooctets, par exemple tiré de `Cedric/JeNo 5/FINAL.bbl`) ajoute un fichier binaire
  au dépôt et publie une partie de ses données de vol : c'est son choix.
  **Risque si on n'y touche pas** : une régression du décodeur sur un vrai en-tête (champs, encodages ou
  prédicteurs que la fabrique n'écrit pas) n'est vue qu'en lançant `pytest` sur un poste qui a le corpus.
  **Reprise** : accord de Cédric sur le log et la durée de l'extrait ; couper le fichier à une frontière de
  trame I (le décodeur resynchronise de toute façon), le mettre dans un dossier de données des tests à créer, et y comparer quelques
  valeurs connues (cadence, réglages, premier et dernier `time`, nombre de trames) sans marque `corpus`.
  [`tests/corpus_paths.py:corpus_log`, `tests/test_bbl_decode.py`]

## Origine : calibration des trames corrompues, branche feat/calibrate-corrupt-frames (2026-10-12)

- ~~**W-010** — Les analyses (spectres, réponse indicielle, boucle ouverte, moteurs, retards) recollent le vol
  de part et d'autre d'un trou d'enregistrement : chaque raccord ajoute un faux bruit, jusqu'à +18 dB entre
  80 et 500 Hz sur un vol de wobbles du corpus (mesures dans [assistant.md](assistant.md#trous-dans-lenregistrement-2026-10-12)).~~
  ✅ **CLOS (2026-10-12)** — D-020 : aucune fenêtre d'analyse ne chevauche plus un trou (`FlightLog.breaks`) ;
  constat « Trous dans l'enregistrement » redevenu informatif. Branche fix/gap-aware-windows.
  **Pourquoi reporté** : correctif distinct du seuil ; Cédric a choisi de prévenir d'abord le pilote (constat
  « Trous dans l'enregistrement »), puis de corriger les analyses sur une branche à part (2026-10-12).
  **Risque si on n'y touche pas** : 7 % des sessions du corpus ont un bruit surestimé, parfois assez pour
  déclencher à tort un constat de bruit ou de filtrage.
  **Reprise** : branche suivante (`fix/…`), dès la fusion de feat/calibrate-corrupt-frames.
  **Piste** : découper les fenêtres de calcul aux trous (`FlightLog.segments`, aujourd'hui inutilisé) dans
  `analysis/spectral.py:_segments` et les fenêtres de la réponse indicielle ; puis revoir le constat, qui
  pourra redevenir informatif.
  [`src/opentunebox/log/flightlog.py:FlightLog.segments`, `src/opentunebox/analysis/spectral.py:_segments`]
