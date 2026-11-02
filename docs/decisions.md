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
[AGENTS.md](../AGENTS.md) ; l'**avancement** dans les branches, les demandes de fusion et le
[CHANGELOG.md](../CHANGELOG.md) ; l'origine de chaque **seuil** de l'assistant dans
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
| D-041 | Simulateur de filtres : les notchs dynamiques ajoutés au-delà de ceux du log sont placés sur les pics prévus, sinon à leur centre initial | 2026-11-02 |
| D-030 | Lint par ruff limité aux règles pyflakes ; pas de formatage automatique pour l'instant | 2026-10-14 |
| D-029 | Les courbes d'un `FlightLog` sont en float32 ; le temps reste en float64 | 2026-10-13 |
| D-028 | Les constats de l'assistant s'identifient par une clé stable, jamais par leur texte | 2026-10-13 |
| D-027 | Raie fixe près de la fondamentale : test des deux trajets d'une tranche de gaz à l'autre | 2026-10-13 |
| D-026 | Retard des filtres jugé dès 30 °/s de consigne (amende D-010) | 2026-10-13 |
| D-025 | Paliers du pic de réponse selon le feedforward en service (0 en mode Angle) | 2026-10-13 |
| D-024 | Notch statique conseillé seulement si le notch dynamique ne peut pas traiter la raie | 2026-10-13 |
| D-023 | « Q trop haut » seulement si une raie passe ; marge de 5 % de la fréquence moteur autour des harmoniques | 2026-10-13 |
| D-022 | Notch dynamique : réponse des notchs RPM du firmware ; `dyn_notch_min_hz` jamais relevé | 2026-10-13 |
| D-021 | Pas des curseurs conseillés, choisis par Cédric | 2026-10-12 |
| D-020 | Aucune fenêtre d'analyse ne chevauche un trou du log | 2026-10-12 |
| D-019 | Le vol chirp de Betaflight n'est pas conseillé : méthode de Brian White | 2026-10-12 |
| D-018 | Décisions, leçons et travaux reportés ont chacun leur registre numéroté | 2026-10-12 |
| D-017 | Deux fréquences minimales du log : filtres et séries PID | 2026-10-12 |
| D-016 | Les spectres s'affichent sur l'échelle de PIDtoolbox, les calculs restent en densité physique | 2026-10-11 |
| D-015 | Un outil = un onglet de la fenêtre principale | 2026-10-10 |
| D-014 | Le feedforward n'est considéré comme nul qu'en mode Angle | 2026-10-10 |
| D-013 | Linux : fenêtres de fichiers par le portail XDG | 2026-10-05 |
| D-012 | Boucle ouverte mesurée par estimateur instrumental | 2026-10-05 |
| D-011 | Seuils de l'assistant : 90e percentile du corpus par taille, origine notée | 2026-10-05 |
| D-010 | Retard des filtres jugé seulement sur un vol avec des mouvements | 2026-10-05 |
| D-009 | Style Qt Fusion imposé sur tous les systèmes | 2026-10-01 |
| D-008 | Simulateur de filtres en méthode relative | 2026-10-01 |
| D-007 | Retards mesurés par la phase, pas par l'intercorrélation | 2026-10-01 |
| D-006 | Aucune dépendance à compiler | 2026-10-01 |
| D-005 | Exception à D-004 : icônes Lucide | 2026-10-01 |
| D-004 | Aucune copie de code de PIDtoolbox ni d'autres projets | 2026-10-01 |
| D-003 | Assistant à règles, 100 % hors ligne | 2026-10-01 |
| D-002 | Décodeur Python accéléré par numba | 2026-10-01 |
| D-001 | Python + PySide6 + pyqtgraph, projet privé, interface en français | 2026-10-01 |

---

## 2026-11-02 — Notchs dynamiques ajoutés du Simulateur de filtres

### D-041 : notchs dynamiques ajoutés placés sur les pics prévus, sinon à leur centre initial

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

## 2026-10-14 — Lint de la CI

### D-030 : lint par ruff limité aux règles pyflakes ; pas de formatage automatique pour l'instant

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
W-003), alors que le Diagnostic du réglage suit Brian White [S32] : un pas de 0,1 ne se lit pas sur la
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

### D-019 : le vol chirp de Betaflight n'est pas conseillé, OpenTuneBox suit la méthode de Brian White

**Contexte.** W-004 proposait de conseiller un vol chirp quand le pic de sensibilité tombe au bord de la
bande mesurée. Le constat essayé se déclenchait sur 44 % des sessions du corpus (310 sur 705), avec des
conseils de réglage du chirp peu sûrs. Cédric : « mettre de côté la fonction CHIRP […] encore en mode
expérimental » et « se concentrer sur la méthode de Brian White ». Vérifié le 2026-10-12 sur le dépôt de
Betaflight : le chirp n'y est pas étiqueté expérimental (PR #13105, « MAJOR FEATURE », fusionnée le
2025-01-21), mais il n'est pas dans le firmware de base (option de compilation `chirp` / `USE_CHIRP`) ; ses
canaux de debug ont changé en avril 2026 (PR #15113) et son point d'injection en mode angle en
septembre 2026 (PR #15196) ; l'outil d'analyse qui en tire un réglage est encore en préparation (PR #15196) ;
un mainteneur le juge « not a basic user option » (ticket #15012).

**Décision.**
- Ni l'assistant, ni le guide intégré, ni le guide utilisateur ne conseillent de vol chirp ni ne disent
  comment le régler. Pour faire aller la mesure plus loin, ils conseillent des wobbles plus rapides et plus
  secs (protocole de Brian White, [S33] de `docs/spec-methodes.md`).
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

**Contexte.** Sous Windows 11, les textes des boutons étaient gris et à peine lisibles (L-014).

**Décision.**
- Style « Fusion » partout (`gui/theme.py`), palettes complètes claire et sombre.
- Les défauts de Fusion sont corrigés par la feuille de style globale plutôt qu'en revenant au style natif
  (L-015).

**Témoins.** Commits `861660d` (couleurs d'accent à contraste ≥ 4,5:1 dans les deux thèmes), `d000968`.

### D-008 : simulateur de filtres en méthode relative

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

**Contexte.** `blackbox_decode` n'existe pas sur toutes les plateformes et a ses propres défauts (L-001).

**Décision.** Décodeur Python accéléré par numba (`cache=True`), aucun binaire externe ;
`blackbox_decode` ne sert qu'à valider (`scripts/validate_corpus.py`).

**Témoins.** [validation.md](validation.md#décodeur) ; commit `28c709c`.

### D-001 : Python + PySide6 + pyqtgraph, projet privé, interface en français

**Contexte.** Un seul code pour Windows, macOS et Linux, maintenable par un débutant aidé d'agents IA,
pour un pilote francophone.

**Décision.**
- Python + PySide6 (Qt) + pyqtgraph : courbes fluides sur plus de 100 000 points.
- Projet privé, tous droits réservés, pas de licence libre.
- Interface, messages et documentation en français ; code et identifiants en anglais ; commentaires en français.

**Témoins.** Trois installateurs fabriqués et essayés par `construire.yml`.
