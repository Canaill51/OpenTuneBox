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
| W-026 | Rust / Python : zéros signés de `np.max` / `np.min`, et entrées jamais fournies par l'application (`fs` NaN dans `xcorr`…) | ouvert (à voir en phase 7) |
| W-027 | Rust / Python : `margins` avec un masque d'un élément, `estimate` sur une grille de fréquences à pas nul (entrées jamais construites par l'application) | ouvert (à voir en phase 7) |
| W-028 | Rust / Python : références `analysis/` sans `Hover_Test5.bbl` (celles de `advisor/` et `flightlog.json` refaites le 2026-11-01), erreurs de la couche du réglage comparées par leur seule présence, `step_tuning` avec un NaN dans les curseurs | ouvert (prochain réenregistrement des références) |
| W-032 | Socle egui (6-0) : écarts connus avec `ToolWindow.run` de Python | 6-3 |
| W-031 | Ligne de commande Rust : pas de texte d'aide détaillé, messages d'usage et d'erreur rédigés autrement que ceux d'argparse | ouvert (à voir en phase 7) |
| W-030 | `rules.py:analyze` plantait (`np.convolve` d'un tableau vide) sur 2 logs du corpus | TRAITÉ le 2026-11-01 (Python sur `main`, PR #75 ; Rust sur `fix/rust-short-session`) |
| W-029 | Rust / Python : types faux refusés par `DroneCard::from_value`, `pid_khz` toujours en flottant, `auto_status` en erreur si un usage manque, `log_rate` panique sur une taille inconnue, pas de crash réel dans le corpus du guide | ouvert (phase 6 / 7) |
| W-025 | `motors` (Rust) n'accepte que des vitesses moteur `float32` (journaux) ; Python accepte aussi `float64` et choisit alors la précision des noyaux | ouvert (à voir en phase 6) |
| W-024 | `delay::xcorr_delay_ms` (Rust) refuse `smooth > 1` : l'argument existe en Python mais aucun appelant de l'application ne le fournit | ouvert (à voir en phase 7) |
| W-023 | `FlightLog::d_term_prefilter` (Rust) planteait (index hors limites) pour un journal d'un point au terme D non nul, Python lève `ValueError` (`np.gradient`) | TRAITÉ le 2026-10-30 (4c-A : `Err`, relecture) |
| W-022 | `otb-parity check` : une seule tolérance par fonction, y compris pour les champs exacts (`freqs`, `bins`) | ouvert (à voir en phase 7) |
| W-021 | Écarts connus Rust / Python sur entrées irréalistes (`nbins = 0`, `fs = 0`, `astype(int)` sur ARM, message d'erreur) | ouvert (à voir au premier test macOS ARM) |
| W-020 | `LogHeader.raw` et `Settings.raw` du Rust sont triés par clé, le dict Python garde l'ordre du fichier | ouvert (à voir en phase 5c / 6) |
| W-019 | Phase 3 repoussée : `spectral`, `loop`, `delay`, `stats`, `sliders`, `step_series`, `settings`, `datasets`, `period` restent en Python | EN COURS (D-035 : repris par la phase 4) |
| W-018 | `opentunebox report` plante sous Windows quand sa sortie est redirigée (cp1252) | ouvert |
| W-017 | Lint : règles de style, formatage ruff et seuil de couverture reportés | TRAITÉ sur `feat/rust-migration` (D-031) |
| W-016 | Pic de mémoire du décodeur : tableau provisoire deux fois trop grand | TRAITÉ sur `feat/rust-migration` (D-031) |
| W-015 | Taille : numba impose llvmlite (37 % de l'exécutable) ; extension compilée à décider | EN COURS (D-031, D-032 : numba et scipy hors de l'application) |
| W-014 | Décodeur : 3,4 µs par trame, plus grosse part de l'ouverture d'un log | ouvert |
| W-013 | Résonances : raies fixes et raies moteur mal séparées près de la fondamentale | clos |
| W-012 | Marge de ± 5 % autour des harmoniques dans la recherche de raies et la forme des pics | clos |
| W-011 | LPF1 et notch statique absents du modèle du notch dynamique | sans suite |
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
  l'enregistrement » (branche feat/calibrate-corrupt-frames), défaut des analyses ouvert en W-010.
  Pas des curseurs : choisis par Cédric (D-021, branche feat/calibrate-slider-steps). Petites marges (2026-10-13,
  D-022) : bandes ± 5 % du notch dynamique remplacées par la réponse des notchs RPM, mesurée contre le debug
  FFT_FREQ ; relèvement de `dyn_notch_min_hz` (30 Hz) retiré, gain de retard mesuré ≈ 0. La liste choisie avec
  Cédric est faite ; les autres lignes « Estimation » restent.
  Poids RPM (2026-10-13, branche feat/calibrate-rpm-weights) : marge 6 → 3 dB et gain minimal 0,5 gardé, mesurés
  sur les harmoniques à poids déjà réduit du corpus et par le retard calculé. Taille pour l'objectif de retard :
  3" et 8" confirmés par la médiane des drones de taille connue du corpus (branche feat/calibrate-delay-size).
  Bandes de bruit (2026-10-13, branche feat/calibrate-remaining-estimates) : 80–500 Hz et 20–80 Hz gardés,
  vérifiés sur le corpus (sensibilité des sessions signalées aux bornes, logs à 1 kHz, cohérence avec le manche).
  Q minimum du notch dynamique (2026-10-13) : 100 → 250, plus bas Q vu avec filtre RPM (presets, corpus).
  Filtre du D (2026-10-13) : LPF1 dynamique proposé (max = 2 × min) sourcé par le firmware et le corpus ;
  relèvement du D propre gardé à deux crans, limite de prudence choisie par Cédric sur simulation.
  Q maximum du notch dynamique : 450 → 500 (guide RPM, configurateur, presets). Notch statique : seulement si
  le notch dynamique ne peut pas traiter la raie (D-024).
  Méthode des résonances vérifiée et gardée (8 tranches, 25–900 Hz) ; limite de l'exclusion ± 8 % ouverte en W-013.
  Dépassement dû au feedforward (2026-10-13, branche feat/calibrate-last-estimates) : repère de latence
  vérifié sur les séries où seul F change ; paliers du pic recalculés avec le feedforward coupé en mode
  Angle (D-025).
  Vol avec mouvements : seuil 50 → 30 °/s (D-026), les wobbles du protocole en mode Angle plafonnant vers
  47 °/s ; repères de retard gardés. Raies près de la fondamentale : test des deux trajets (D-027, W-013 clos).

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

## Origine : petites marges du notch dynamique, branche feat/calibrate-slider-steps (2026-10-13)

- ~~**W-011** — Le modèle du notch dynamique part du gyro non filtré moins le filtre RPM ; le firmware analyse le
  gyro après le filtre RPM, le notch statique **et** le passe-bas LPF1 (`sensors/gyro_filter_impl.c`).~~
  🚫 **SANS SUITE (2026-10-13)** — mesuré sur 598 sessions en debug FFT_FREQ : ajouter le notch statique et LPF1
  ne change presque rien (fréquences du firmware retrouvées 0,60 dans les deux cas, pics prévus suivis 0,80 →
  0,82) ; le modèle retenu (D-022) fait mieux sans eux.
  [`analysis/motors.py:dyn_notch_targets`, [assistant.md](assistant.md#notch-dynamique-2026-10-05)]

- ~~**W-012** — La recherche des raies hors moteurs (`motors.spectrum_lines`) et la forme des pics du notch
  dynamique (`motors.line_shapes`, pour le Q conseillé) retirent encore les bandes des harmoniques ± 5 %
  (`motors._masked_spectrum`), alors que D-022 montre que le filtre RPM en laisse passer une partie (L-028).~~
  ✅ **CLOS (2026-10-13)** — D-023 : forme des pics mesurée sur le spectre après filtre RPM quand le masque la
  rend inconnue, « Q trop haut » seulement si une raie passe encore les filtres ; marge de 5 % de la fréquence
  moteur pour toutes les harmoniques, mesurée sur 119 vols stationnaires. Branche feat/calibrate-line-margins.
  **Pourquoi reporté** : autre usage que les pics du notch dynamique ; sa marge n'a pas été mesurée.
  **Risque si on n'y touche pas** : un pic résiduel près d'une harmonique a une largeur inconnue (NaN) et
  n'entre pas dans le Q calculé ; une raie hors moteurs à moins de 5 % d'une harmonique n'est pas listée.
  **Reprise** : prochaine calibration d'un seuil de raies ou du Q du notch dynamique.
  **Piste** : même méthode que D-022 (réponse RPM au lieu du masque, comparée au debug FFT_FREQ).
  [`src/opentunebox/analysis/motors.py:line_shapes`]

## Origine : vérification de la méthode des résonances, branche feat/calibrate-remaining-estimates (2026-10-13)

- ~~**W-013** — « Raie fixe » (`rules.frame_resonance`) écarte toute raie à ± 8 % d'une harmonique de la vitesse
  moyenne en stationnaire : mesuré par la pente de la raie face à la vitesse des moteurs d'une tranche de gaz à
  l'autre, les 39 raies écartées du corpus mêlent raies fixes (36 %) et raies moteur (33 %), et les raies
  signalées entre 8 et 15 % d'une harmonique sont mêlées (39 % fixes, 22 % moteur).~~
  ✅ **CLOS (2026-10-13)** — D-027 : test des deux trajets sur la fondamentale (± 15 %, seuil −2 dB), exclusion
  ± 8 % gardée sur les harmoniques 2 et 3 ; « Raie fixe » sur 59 sessions au lieu de 45. Branche
  feat/calibrate-last-estimates.
  **Pourquoi reporté** : le test de pente est fiable sur la fondamentale (témoin 0,82 pour 1) mais pas sur les
  harmoniques 2 et 3 (0,40 pour 2, 1,79 pour 3) ; choix de Cédric (2026-10-13) : garder la méthode.
  **Risque si on n'y touche pas** : une vraie résonance proche de la fondamentale n'est pas signalée (environ 14
  sessions du corpus), une raie moteur l'est parfois à sa place (environ 5) ; les conseils (mécanique, notch
  dynamique) restent sensés dans les deux cas.
  **Reprise** : prochaine modification de la recherche des résonances, ou si un pilote signale une résonance
  manquée près de la fréquence moteur.
  **Piste** : pente sur la fondamentale seulement (± 15 %), seuils calibrés sur les raies du vol stationnaire
  et le debug FFT_FREQ ; script de mesure à refaire comme une fonction de `rules` (même calcul que la règle).
  [`src/opentunebox/advisor/rules.py:frame_resonance`, [assistant.md](assistant.md#règles)]

## Origine : chantier performance demandé par Cédric (2026-10-13)

- **W-014** — Le décodeur prend environ 3,4 µs par trame (3,3 s pour les 965 000 trames du plus gros log du
  corpus, 39 Mo) : c'est maintenant la plus grosse part de l'ouverture d'un log dans l'interface (5,2 s).
  **Pourquoi reporté** : la boucle principale est validée trame par trame contre `blackbox_decode`
  ([validation.md](validation.md#décodeur)) ; la remanier pour un gain incertain demande un passage complet
  de `scripts/validate_corpus.py` (plusieurs heures, la référence se bloque sur certains fichiers).
  **Risque si on n'y touche pas** : quelques secondes d'attente à l'ouverture des très gros logs ; les logs
  courants (quelques Mo) s'ouvrent en moins d'une seconde.
  **Reprise** : prochaine modification du décodeur, ou si un pilote se plaint de l'ouverture.
  **Piste** : profiler `decode_stream` par type de trame (copie `main[n_rows, :] = cur`, lectures d'octets
  via le tableau d'état `st`) ; ou le réécrire hors numba (voir W-015).
  **Traité le 2026-10-29** sur `feat/rust-migration` : le décodeur Rust (D-031) ouvre le plus gros log
  (965 000 trames) en 0,58 s au lieu de 2,44 s, identique au bit près.
  [`src/opentunebox/decoder/core.py:decode_stream`]

- **W-015** — *(Mesure du 2026-10-29 après D-032 : dossier Linux 206 Mo au lieu de 463 Mo, voir `migration-rust.md`.)* Numba impose `llvmlite` (170 Mo sur les 463 Mo du dossier Linux, ≈ 37 %) et une compilation au
  premier lancement : c'est le seul levier restant sur la taille des installateurs. Le reste est Qt (102 Mo,
  déjà allégé), scipy (71 Mo) et numpy (41 Mo) ; nettoyer `libllvmlite.so` par `strip` ne gagne que 18 Mo.
  **Pourquoi reporté** : supprimer numba demande de réécrire le décodeur et les noyaux de filtres dans un
  langage compilé (Rust par une extension PyO3, ou C), fabriqué pour chaque système par `construire.yml` :
  cela remplace D-002 et D-006, décision réservée à Cédric (proposée dans le rapport du chantier).
  **Risque si on n'y touche pas** : installateurs de 88 à 167 Mo selon le système ; 5 à 15 s de compilation
  au tout premier lancement (mesuré : 6 s pour un rapport à cache numba vide sous Linux).
  **Reprise** : accord de Cédric pour une extension compilée.
  **Piste** : extension Rust (PyO3 + maturin) pour `decoder/core.py` et `bf_filters` (`_sections_response`,
  simulation temporelle) ; le reste reste en Python ; validation par `scripts/validate_corpus.py` et
  `scripts/advisor_rates.py --compare`.
  **Arbitrage de Cédric (2026-10-29)** — EN COURS : migration complète vers Rust sur `feat/rust-migration`
  (D-031, plan dans [migration-rust.md](migration-rust.md)), pas seulement une extension.
  [`src/opentunebox/decoder/core.py`, `packaging/opentunebox.spec`]

- **W-016** — Le décodeur réserve ses trames d'après la taille du fichier (`est_rows = octets // 20`), soit
  environ deux fois trop pour les logs courants (≈ 40 octets par trame) : le pic de mémoire à l'ouverture
  (1347 Mo pour le plus gros log) vient de ce tableau provisoire plus la copie à la taille exacte.
  **Pourquoi reporté** : une estimation plus juste force des agrandissements (copie à chaque fois) sur les
  logs aux trames très compressées ; gain mesuré seulement sur les plus gros logs (> 30 Mo), rares.
  **Risque si on n'y touche pas** : un ordinateur à 4 Go de mémoire peut peiner sur un log de plus de 50 Mo.
  **Reprise** : un pilote signale un manque de mémoire, ou W-014 est repris.
  **Piste** : tableaux par blocs de 65 536 trames, réunis à la fin (pic ≈ 2 × la taille utile au lieu de 3 ×).
  **Traité le 2026-10-29** sur `feat/rust-migration` : réserve moitié moindre puis copie à la taille exacte ;
  pic de mémoire du plus gros log, interpréteur compris, 507 Mo → 364 Mo.
  [`src/opentunebox/decoder/reader.py:_decode_session`]

## Origine : comparaison des CI d'OpenTuneBox et de Superview (2026-10-14)

- **W-017** — Le lint se limite aux règles `F` de ruff (D-030) : les 51 signalements de style des règles par
  défaut (E701, E702, E741, E401) ne sont pas corrigés, le code n'est pas formaté par `ruff format` (87 fichiers
  sur 103 changeraient), et la couverture de tests est publiée par la CI sans seuil bloquant.
  **Pourquoi reporté** : un gros commit de reformatage brouille `git blame` et les branches en cours ; le
  seuil de couverture de la CI (sans corpus, ≈ 35 % mesuré le 2026-10-12) ne dirait rien de la qualité réelle
  (L-025).
  **Risque si on n'y touche pas** : un style hétérogène ; pas de garde-fou chiffré contre une baisse de couverture.
  **Reprise** : avant de passer le dépôt en public, ou quand aucune branche n'est en cours.
  **Piste** : corriger les 51 lignes, activer `ruff format` en un commit séparé (à ajouter à `.git-blame-ignore-revs`),
  puis `ruff format --check` en CI ; seuil de couverture à fixer d'après la mesure CI du moment.
  [`pyproject.toml`, `.github/workflows/tests.yml`]

## Origine : CI Windows de la phase 2a (2026-10-29)

- **W-018** — `opentunebox report` écrit des caractères hors cp1252 (« ≈ ») sur la sortie standard : sous Windows,
  avec la sortie redirigée vers un fichier ou un tube (`opentunebox report log.bbl > rapport.txt`),
  `UnicodeEncodeError` ; vu seulement dans la CI (sous-processus de `test_no_heavy_deps.py`), pas en console.
  **Pourquoi reporté** : hors du sujet de la phase 2a ; le test contourne par `PYTHONUTF8=1`.
  **Risque si on n'y touche pas** : un rapport redirigé échoue sous Windows ; en console, rien.
  **Reprise** : phase 5 (ligne de commande en binaire Rust) ou dès qu'on touche à `cli.py`.
  **Piste** : `sys.stdout.reconfigure(encoding="utf-8")` au début de `cli.main` (en vérifiant `sys.stdout is not None`,
  L-021), avec un test qui redirige la sortie. [`src/opentunebox/cli.py`]

## Origine : exploration de la phase 3 (2026-10-29)

- **W-019** — Porter en Rust `spectral`, `loop`, `delay`, `stats`, `sliders`, `step_series`, `settings`, `datasets`,
  `period` (et `FlightLog` lui-même) n'accélère presque rien : ces modules sont dominés par la FFT de numpy (déjà en C),
  par des opérations numpy sur des tableaux limités par la mémoire, ou par de la logique métier (dicts de réglages
  copiés et modifiés, `round()` demi-pair de Python, seuils discontinus, regex, tris). Mesuré le 2026-10-29 : `spectral`
  13 % de l'assistant, `loop` 11 %, `_numeric` 10 % (FFT), `delay` 3,5 %, `stats` 4,5 %, le reste moins de 1 %.
  **Pourquoi reporté** : gain de vitesse nul ou faible pour un risque d'écart élevé (D-034).
  **Risque si on n'y touche pas** : Python et numpy restent nécessaires à l'application ; pas de binaire 100 % Rust.
  **Reprise** : décidée le 2026-10-30 (D-035) : phases 4 et 5 (journaux, analyses, assistant, ligne de commande en binaire Rust) : porter alors
  ces modules avec leurs consommateurs, avec les étalons `tests/ref_*.py` et `golden.py` ; commencer par les feuilles
  (`_numeric`, `spectral`, `settings` sans la partie modifiable).
  **Piste** : `PidTerms` doit d'abord quitter `analysis/loop.py` (dépendance circulaire avec `log/flightlog.py`) ;
  garder `Settings.raw` exposé (lu par clés arbitraires hors de `log/`). [`docs/migration-rust.md`]

## Origine : relecture indépendante de la phase 4a (2026-10-30)

- **W-020** — Dans `rust/otb-decoder/src/header.rs` et `rust/otb-log/src/settings.rs`, `raw` est un `BTreeMap` (trié par clé) ;
  le dict Python garde l'ordre des lignes du fichier. Les couches de parité comparent clé par clé, donc l'ordre n'est
  jamais vérifié.
  **Pourquoi reporté** : aucun code Rust ne parcourt `raw` pour l'instant (recherches par clé seulement).
  **Risque si on n'y touche pas** : l'affichage de l'en-tête (ligne de commande, phase 5c ; interface, phase 6) sortirait
  dans un autre ordre que l'étalon.
  **Reprise** : quand une sortie parcourt `raw` : passer à une liste ordonnée avec index (ou `indexmap`) et comparer l'ordre
  dans `layers.rs:check_header`.

## Origine : relecture indépendante de la phase 4b (2026-10-30)

- **W-021** — Écarts connus entre Rust et Python sur des entrées qu'aucun appel réel ne produit : (a) `freq_throttle` avec
  `nbins = 0` : Python rend un `z` de forme (151, 0), le registre de parité refuse `nbins < 1` (la bibliothèque, elle, ne
  panique plus) ; (b) `fs = 0` : Python lève `ZeroDivisionError` pour un `float` Python mais rend un résultat pour un
  `np.float64`, Rust rend toujours un résultat ; (c) `to_bin` reproduit `astype(int)` de x86 (NaN et débordement →
  `INT_MIN`), sur ARM numpy sature (+∞ → `INT_MAX`) : non vérifié sur ARM ; (d) message d'erreur de `freq_time` pour `t`
  vide (« index 0 » contre « index -1 »), sans effet car le comparateur n'exige que « erreur des deux côtés ».
  **Pourquoi reporté** : entrées irréalistes ou plate-forme non utilisée par Cédric.
  **Risque si on n'y touche pas** : un écart de résultat sur Mac ARM pour des gaz NaN.
  **Reprise** : au premier test sur macOS ARM (phase 7), ou si une entrée réelle produit l'un de ces cas.
- **W-022** — `otb-parity check` applique une seule tolérance à tout le résultat d'une fonction, y compris les champs
  documentés « identiques au bit près » (`freqs`, `bins`, `resolution_hz`). Les tests de cas écrits, eux, les exigent exacts.
  **Pourquoi reporté** : demande une tolérance par champ dans `registry::tolerance` ; les fréquences sont identiques
  partout où on les a comparées.
  **Reprise** : quand on resserre la couche corpus (phase 7), ou si un écart sur ces champs est soupçonné.
- **W-023** — **TRAITÉ le 2026-10-30 (relecture de la 4c-A).** Pour un journal d'un **seul point** dont le terme D n'est pas nul, `FlightLog.d_term_prefilter`
  de Python lève `ValueError` (`np.gradient` demande au moins 2 points). Le Rust (4a-5) **plantait** (`index out of bounds` dans `gradient`, et non
  « rendait des zéros » comme écrit d'abord) ; `datasets.available` et `delay.compute_delays` en héritaient. Depuis, `d_term_prefilter` rend un
  `Result` (`Err` pour n < 2, et aussi pour `fs` NaN ou infini : `int(fs / 60)` de `_slow_gain`), propagé par `datasets` et `delay` ; les cas écrits
  (`n1`, `fs_nan`, `fs_inf`, `un_point`, `deux_points`) exigent que Rust lève comme Python.
- **W-024** — `analysis/delay.py:xcorr_delay_ms(..., smooth)` lisse x et y par `np.convolve(..., mode="same")` quand `smooth > 1`. Le port Rust
  (`otb-analysis/src/delay.rs`) rend une erreur dans ce cas : `np.convolve` passe par BLAS (ordre de sommation propre à la bibliothèque), le
  décalage de la sortie « same » change la longueur si `smooth > n`, et **aucun appelant** de l'application (`step_series`, `log_check`,
  `_step_common`, `compute_delays`) ne fournit `smooth`.
  **Pourquoi reporté** : code mort côté application ; le porter demanderait une tolérance de plus sans rien protéger.
  **Reprise** : si un outil de la phase 6 veut lisser les signaux avant l'intercorrélation (alors porter la convolution avec sa tolérance), ou à
  la suppression du Python en phase 7 (retirer le paramètre).
- **W-025** — les fonctions de `analysis/motors.py` acceptent une vitesse moteur `mhz` en `float32` ou en `float64` (`_mhz_args`, L-041) ; le port Rust
  (`otb-analysis/src/motors.rs`) ne prend que des `Rows` `float32`, ce que rendent tous les journaux (D-029) et `motor_hz`. Aucun appelant de
  l'application ne fournit du `float64`.
  **Pourquoi reporté** : double chemin de test sans appelant réel.
  **Reprise** : si un outil de la phase 6 passe des vitesses moteur calculées en `float64` (alors rendre les fonctions génériques `Real`).
- **W-026** — écarts de détail connus entre le Rust et numpy, **inatteignables** depuis l'application, relevés par la relecture de la 4c-A :
  (1) `npstat::max_f32` / `min_f32` gardent le premier de deux zéros de signes opposés alors que numpy rend le dernier (`max([-0.0, 0.0])` = +0,
  `max([0.0, -0.0])` = −0, selon l'ordre de réduction) : les valeurs passent par `abs` et aucun −0 ne sort du décodeur ; (2) `crash_time`, `phase_delay_ms`,
  `rpm_filtered_spectrum` : non fini ou NaN dans des arguments que les appelants ne passent jamais ne sont pas tous reproduits (seuls ceux de la liste
  des cas écrits le sont) ; (3) les valeurs en dB (`motor_noise`, `harmonic_prominence`, `dyn_notch_targets`) sont comparées relativement au plus grand
  élément, pas case par case (leurs valeurs sont peu nombreuses et proches de la dizaine de dB).
  **Pourquoi reporté** : aucun appelant ne les atteint ; les corriger sans cas réel serait deviner la règle de numpy.
  **Reprise** : si un outil de la phase 6 passe des valeurs à zéros signés ou des arguments non finis, ou à la suppression du Python (phase 7).
- **W-027** — `open_loop.rs` (port de `analysis/loop.py`) suppose les entrées que `loop_spectra` produit ; deux écarts à numpy sur des entrées que l'application ne
  construit jamais, relevés par la relecture de la 4c-B : (1) `margins` avec un masque `valid` d'**un seul élément** : numpy le diffuse, Rust rend une erreur
  « tailles différentes » ; (2) `estimate` sur des spectres à la main dont la grille de fréquences a un **pas nul** (fréquences en double) : `astype(int)` de NaN
  rend `INT_MIN` en numpy (largeur de lissage énorme, moyenne sur toute la grille), Rust garde la largeur minimale de 5 points.
  **Pourquoi reporté** : `loop_spectra` rend toujours une grille strictement croissante et un masque de la longueur de la grille ; reproduire ces cas sans appelant
  réel serait deviner.
  **Reprise** : si un outil de la phase 6 construit des spectres ou un masque à la main, ou à la suppression du Python (phase 7).

- **W-028** — trois écarts de couverture connus de la 5a-4 / 5a-5, relevés par la relecture : (1) `1. PTB/Moribot/Hover_Test5.bbl` est dans le corpus (500 fichiers) mais pas dans `flightlog.json` (499) ni dans `advisor/` ni dans `analysis/` : les références de ces couches sont plus anciennes que le corpus, seul le réglage par séries (`advisor_tuning/`) le couvre ; (2) `tuning_layer.rs` compare les erreurs par leur seule présence (Python lève ⇒ Rust lève), sans vérifier que c'est à la même étape, et le corpus actuel n'en contient aucune ; (3) `step_tuning` sur des curseurs contenant un NaN : Python trie sans erreur dans un ordre arbitraire, Rust rend une erreur (inatteignable depuis `find_series`).
  **Pourquoi reporté** : (1) demande de réenregistrer `advisor/` et `analysis/` (≈ 25 min) sans changer le code ; (2) et (3) n'ont aucun appelant réel.
  **Reprise** : (1) au prochain réenregistrement des références (`golden.py record-flightlog`, `record-analysis`, `record-advisor`) ; (2) et (3) si un cas d'erreur apparaît dans le corpus ou à la phase 7.
  *Réglé le 2026-11-01 pour `flightlog.json` et `advisor/`* : `golden.py record-flightlog` et `record-advisor` refaits (669 journaux, 631 sessions de l'assistant, `Hover_Test5.bbl` comprise) ; `otb-parity check-advisor` : 631 sessions, 362 089 clés, 0 écart. Reste le point (1) pour `analysis/` (`record-analysis`, 337 Mo), à refaire au prochain besoin.

- **W-029** — quatre limites connues de la 5b : (1) `DroneCard::from_value` (Rust) refuse un type faux (`name` = 3, `pid_khz` = "4", `done` = liste, `bf` = null) alors que `from_dict` de Python les accepte sans rien vérifier ; `pid_khz` s'écrit toujours en flottant (`4.0`), Python réécrit ce qu'il a lu (`4` si le fichier disait `4`) ; (2) `auto_status` rend une erreur si l'usage de la case manque dans un `LogCheck` (Python lève `AttributeError`), cas que `check_log` ne produit jamais ; (3) `DroneCard::log_rate` panique (Python : `KeyError`) si la taille est inconnue et la fiche construite à la main (`from_value` la ramène à `five`) ; (4) le corpus ne contient aucun crash au sens de `crash_time` dans les sessions rejouées par `check-guide` : la branche « crash » repose sur les cas fabriqués.
  **Pourquoi reporté** : (1) les fiches sont écrites par l'application avec des choix du menu (textes et flottants) ; (2) à (4) n'ont pas d'appelant réel.
  **Reprise** : (1) à l'écriture de la page Guide en Rust (phase 6), qui choisira le format des fiches ; (2) à (4) à la suppression du Python (phase 7).
  *Réglé le 2026-10-31 (hors liste)* : `rust/otb-advisor/tests/data/log_check.json` pesait 26 Mo. Colonnes que `check_log` ne lit pas (termes P, I, F, commande radio, debug, moteurs) écrites en zéros — **les réponses de Python, calculées sur ces journaux-là, sont identiques** (la gaz, `d_term` et `gyro_unfilt` restent : les retirer change des réponses) — puis gzip : `log_check.json.gz`, 5,2 Mo ; lu par `flate2` (Rust pur). Les 66 mutations restent détectées (une perdue en raccourcissant des journaux, retrouvée par un cas ajouté : `fenêtres : pitch sans consigne`).

## Origine : phase 5c, comparaison de la ligne de commande sur le corpus (2026-11-01)

- **W-030** — **TRAITÉ le 2026-11-01.** `opentunebox report` (et donc l'onglet Assistant) plantait sur 2 des 500 logs du corpus
  (`1. PTB/cheussure/Dgain-1.0-1.2-1.4-1.6.bbl`, `1. PTB/cheussure/Q_750_(100_40_80)-fade30_DN2-500.bbl`) : `ValueError: a cannot be empty`
  dans `advisor/rules.py`, `predict_filter_change`. Cause : les sessions de 0,1 et 0,2 s (restes d'enregistrement) n'ont aucune fréquence entre
  80 Hz et la limite des filtres. Correction : l'axe sans fréquence dans la bande est ignoré, la prévision est absente, donc `gyro_lpf1_off` et
  `dterm_filter_too_strong` ne sont pas émis ; Python sur `main` (PR #75, 0 conseil différent sur 631 sessions), puis Rust (`whatif_deltas` rend
  `None`). Preuve sur les données : `otb-parity check-cli`, où ces 2 fichiers donnaient une erreur. L-065.
  [`advisor/rules.py`, `rust/otb-advisor/src/rules.rs`]

- **W-031** — le binaire Rust `opentunebox` écrit à la main son analyse des arguments (aucune dépendance) en suivant `argparse` de Python 3.14.4 :
  même code de sortie sur 792 combinaisons (cas fixes et tirage au hasard) (`scripts/cli_args_check.py` : `--`, `-h`, `-hX`, `--help=`, options abrégées,
  `-5` ou texte à espace pris pour un fichier, une seule série de fichiers, valeur de `--profil` refusée aussitôt, option inconnue seulement à la fin).
  Différences connues : (1) `-h` écrit seulement la ligne d'usage, pas l'aide détaillée d'argparse ; (2) les messages d'usage et d'erreur sont rédigés
  autrement ; (3) le message entre parenthèses d'une ligne « illisible (…) » vient de l'exception de chaque langage ; (4) `char::is_numeric` de Rust
  est un peu plus large que `\d` de Python pour des chiffres non ASCII rares ; (5) le comportement d'argparse sur ces cas dépend de la version de Python
  (mesuré sur 3.14.4). La sortie s'écrit au fil de l'eau, session par session, comme Python.
  **Pourquoi reporté** : aucune de ces différences ne change un résultat.
  **Reprise** : à la phase 7 (suppression de Python : `scripts/cli_args_check.py` disparaît avec lui), ou si Cédric veut une vraie aide en ligne.
  [`rust/otb-cli/src/lib.rs:parse`]

- **W-032** — Socle egui (6-0), relecture du 2026-11-01. Écarts connus avec `ToolWindow.run`, laissés pour la sous-étape qui en a besoin :
  (1) `LogInput` ne transmet pas les caches `extras` ; la Réponse indicielle de Python s'en sert dans `compute` avec une clé qui contient les réglages (`step_window.py:232-235`) : à concevoir en 6-3 (cache partagé `Arc<Mutex<…>>` à clé complète) ;
  (2) l'erreur de calcul n'est pas tronquée (Python garde les 600 derniers caractères) ;
  (3) un réglage `f64::NAN` dans `Options` relance le calcul à chaque cadre (`NaN != NaN`) : les outils n'en mettent pas dans leurs réglages, à vérifier à chaque outil ;
  (4) après une erreur, la demande en attente est quand même lancée (Python l'oublie) : jugé préférable ;
  (5) `default_period` remplace l'erreur d'`auto_period` par tout le log, et `LoadedLog::sel()` rend `0..1` sur un log vide (`slice_time`) ;
  (6) tailles en points écrites hors de `theme.rs` (icônes, barre de couleurs, `GRAB_PX`) : la règle d'AGENTS.md n'est vérifiée que pour les couleurs ;
  (7) le Guide reste affiché à l'ouverture d'un log (Python bascule vers les courbes) : avec le visualiseur, 6-1.
  **Reprise** : en 6-1 pour (5) à (7), en 6-3 pour (1) à (4).
