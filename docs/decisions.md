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
| D-056 | Notes de version **générées par GitHub** : titre de PR = phrase en français, une étiquette `RN: …` par PR (rubriques de `.github/release.yml`, contrôle `etiquette-pr.yml`) ; `CHANGELOG.md` supprimé ; remplace « titre de PR = nom de la branche » | 2026-11-11 |
| D-055 | Workflows relus par `actionlint` **dans la CI** (job de `tests.yml`, version figée vérifiée par empreinte), pas installé sur le poste | 2026-11-11 |
| D-054 | Sécurité : réglages GitHub (protections, secrets, versions immuables, CodeQL, actions autorisées), Claude refusé sur une demande de fusion venue d'un fork, `--locked` en CI, aucun `unsafe`, crochet `guard-git.sh`, essai de mutations | 2026-11-11 |
| D-053 | Cache des outils borné **par outil**, en nombre d'entrées (3 réponses indicielles, 20 simulations de filtres, 6 rapports, 2 vérifications du Guide), la moins récemment utilisée jetée la première | 2026-11-10 |
| D-052 | Données de test : seul `step_tuning.json` est stocké compressé (`.gz`) ; les autres gros fichiers de cas restent en clair | 2026-11-08 |
| D-051 | Ouverture de plusieurs logs : **3 fichiers décodés en même temps** au plus, plafond posé en nombre de fichiers et non en mémoire | 2026-11-08 |
| D-050 | Les sources ne sont plus nommées : titres, liens et citations gardés, aucun nom de personne dans le dépôt **(précisée le 2026-11-08 : les ouvrages publiés gardent leurs auteurs ; le 2026-11-12 : toute personne, pseudonymes compris)** | 2026-11-07 |
| D-049 | Réponse indicielle : ordre des courbes et couleurs donnés par l'ordre où l'utilisateur coche les fichiers ; sélection vide permise ; les autres pages gardent la couleur attribuée à l'ouverture | 2026-11-06 |
| D-048 | Tracé du viewer : index multi-résolution min/max par courbe (`otb-dsp::minmax`), sortie identique point pour point à l'ancienne décimation ; un paquet contenant un trou est tracé comme un trou ; W-049 clos | 2026-11-05 |
| D-047 | `golden.rs` compare chaque fonction portée avec la tolérance mesurée de `registry::tolerance` (bit près si aucune n'est déclarée), comme `otb-parity check` ; W-048 clos | 2026-11-05 |
| D-046 | Polices embarquées (Ubuntu Light, Hack) gardées, chacune sous sa propre licence ; licences propres aux polices admises malgré « aucun copyleft » (D-045) ; W-046 clos | 2026-11-05 |
| D-045 | Licence PolyForm Noncommercial 1.0.0 : code ouvert, tout usage commercial interdit, sans double licence ni exception vendue | 2026-11-04 |
| D-044 | Décodeur : en-tête refusé au-delà de 512 champs ou hors de 1 à 65 535 pour les intervalles ; mémoire insuffisante = erreur ; réglages à zéro signalés | 2026-11-04 |
| D-043 | Dépôt public sous licence GPL 3 ; anciennes versions publiées supprimées ; adresse noreply ; noms de tiers neutres **(amendée par D-045)** | 2026-11-04 |
| D-042 | macOS abandonné : l'application est livrée pour Windows et Linux seulement ; plus d'installateur ni d'essai macOS | 2026-11-03 |
| D-041 | Simulateur de filtres : les notchs dynamiques ajoutés au-delà de ceux du log sont placés sur les pics prévus, sinon à leur centre initial | 2026-11-02 |
| D-040 | Interface egui : écarts voulus avec l'interface Qt validés (liste de fichiers à cocher, diagnostic en widgets, « -> », Bode zoomables, calcul automatique) et limites gardées telles quelles (groupes repliables, date d'export en UTC, barres d'erreur sans capuchon) | 2026-11-02 |
| D-039 | Guide egui : mémoire du guide dans la persistance d'eframe, `drones.json` lu avec tolérance et jamais écrasé s'il est illisible, police Hack de secours | 2026-11-02 |
| D-038 | Configuration egui : lignes alignées entre logs par nom de réglage (Python : par position) | 2026-11-02 |
| D-037 | Outils egui : cache des résultats par log, partagé avec le thread de calcul, clé = réglages de calcul + période | 2026-11-02 |
| D-036 | Phase 6 : ligne de commande Rust (5c) d'abord ; fiches drone lues avec tolérance, même fichier ; réglages Qt non repris | 2026-11-01 |
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
| D-025 | Paliers du pic de réponse selon le feedforward en service (0 en mode Angle) | 2026-10-13 |
| D-024 | Notch statique conseillé seulement si le notch dynamique ne peut pas traiter la raie | 2026-10-13 |
| D-023 | « Q trop haut » seulement si une raie passe ; marge de 5 % de la fréquence moteur autour des harmoniques | 2026-10-13 |
| D-022 | Notch dynamique : réponse des notchs RPM du firmware ; `dyn_notch_min_hz` jamais relevé | 2026-10-13 |
| D-021 | Pas des curseurs conseillés, choisis par Cédric | 2026-10-12 |
| D-020 | Aucune fenêtre d'analyse ne chevauche un trou du log | 2026-10-12 |
| D-019 | Le vol chirp de Betaflight n'est pas conseillé : méthode des séries de vols | 2026-10-12 |
| D-018 | Décisions, leçons et travaux reportés ont chacun leur registre numéroté | 2026-10-12 |
| D-017 | Deux fréquences minimales du log : filtres et séries PID | 2026-10-12 |
| D-016 | Les spectres s'affichent sur l'échelle de PIDtoolbox, les calculs restent en densité physique | 2026-10-11 |
| D-015 | Un outil = un onglet de la fenêtre principale | 2026-10-10 |
| D-014 | Le feedforward n'est considéré comme nul qu'en mode Angle | 2026-10-10 |
| D-013 | Linux : fenêtres de fichiers par le portail XDG | 2026-10-05 |
| D-012 | Boucle ouverte mesurée par estimateur instrumental | 2026-10-05 |
| D-011 | Seuils de l'assistant : 90e percentile du corpus par taille, origine notée | 2026-10-05 |
| D-010 | Retard des filtres jugé seulement sur un vol avec des mouvements **(amendée par D-026)** | 2026-10-05 |
| D-009 | Style Qt Fusion imposé sur tous les systèmes **(remplacée par D-035)** | 2026-10-01 |
| D-008 | Simulateur de filtres en méthode relative | 2026-10-01 |
| D-007 | Retards mesurés par la phase, pas par l'intercorrélation | 2026-10-01 |
| D-006 | Aucune dépendance à compiler **(amendée par D-031)** | 2026-10-01 |
| D-005 | Exception à D-004 : icônes Lucide | 2026-10-01 |
| D-004 | Aucune copie de code de PIDtoolbox ni d'autres projets **(amendée par D-005)** | 2026-10-01 |
| D-003 | Assistant à règles, 100 % hors ligne | 2026-10-01 |
| D-002 | Décodeur Python accéléré par numba **(remplacée par D-035)** | 2026-10-01 |
| D-001 | Python + PySide6 + pyqtgraph, projet privé, interface en français **(amendée par D-035)** | 2026-10-01 |

---

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
