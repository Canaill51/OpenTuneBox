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
| W-007 | Tests du corpus en échec quand un log a été déplacé | clos |
| W-006 | Écart de 2 dB sous 100 Hz avec PIDtoolbox sur un vol | sans suite |
| W-005 | Référence de l'erreur aléatoire non vérifiée à la page | ouvert |
| W-004 | Boucle ouverte mesurable seulement jusqu'à 20–40 Hz | ouvert |
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

- **W-004** — La boucle ouverte mesurée s'arrête vers 20–40 Hz (énergie du manche) : le pic de
  sensibilité tombe au bord de la bande pour 57 axes sur 133 et n'est alors qu'un minorant.
  **Pourquoi reporté** : limite de la mesure, signalée à l'écran (`sensitivity_peak_at_edge`).
  **Risque si on n'y touche pas** : pic de sensibilité sous-estimé sur les vols sans excitation large bande.
  **Reprise** : quand les vols avec le debug CHIRP de Betaflight deviennent courants dans le corpus.
  **Piste** : conseiller un vol chirp dans le guide quand le pic est au bord de la bande.
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
