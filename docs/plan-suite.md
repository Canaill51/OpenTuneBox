# Plan de la suite : après la 0.9.0

Suite de [plan-chantiers.md](plan-chantiers.md), dont **les lots 0 à 10 sont faits** (seul le lot 11 reste) : les cinq chantiers de
l'audit du 2026-11-04 (lot 6, bilan le 2026-11-06) et la **première publication Rust** (lot 7, 0.9.0 publiée le 2026-11-08).

**État au 2026-11-08, vérifié** : `main` propre, aucune branche ni demande de fusion en cours, aucun ticket ouvert, CI verte, et
`cargo run -p otb-dev -- verify --strict` avec le corpus, les références figées et la comparaison au bit près : **0 saut, tout
passe**. La 0.9.0 est en ligne (page « dernière version », AppImage + installateur Windows). Les huit outils de PIDtoolbox Pro
V0.94 sont tous couverts (`docs/spec-ecrans.md`, `rust/otb-gui/src/tools/mod.rs:all_pages`).

Le programme est donc complet et publié. Ce qui reste tient en trois familles : **vérifier ce que seule une vraie machine
vérifie** (lot 13), **finir la calibration de l'assistant** (lot 14), et **du confort** (lots 11 et 15).

## Comment s'en servir

Les règles de travail sont celles de [plan-chantiers.md](plan-chantiers.md) (« Comment s'en servir ») et ne sont pas répétées
ici : une session = un lot = une PR, phases explorer → planifier → implémenter → vérifier → livrer → consigner, contre-preuve
pour tout nouveau test, `cargo run -p otb-dev -- verify` vert avant de livrer, critère d'arrêt si une hypothèse tombe.

La numérotation des lots **continue la même série** : les lots 6, 7 et 11 restent décrits dans `plan-chantiers.md`, les lots 12
à 15 ci-dessous. Demander « Lot N » suffit : les deux fichiers ne réutilisent jamais un numéro.

## Ordre des étapes

| # | Lot | Sujet | Où il est décrit | Dépend de |
|---|---|---|---|---|
| 1 | **6** | Bilan des cinq chantiers, W clos, leçons | `plan-chantiers.md` | ✅ fait le 2026-11-06 |
| 2 | **7** | Première publication Rust **0.9.0** (7a licences, 7b–7e publication) | `plan-chantiers.md` | ✅ fait le 2026-11-08 |
| 3 | **12** | Ménage des registres : solder les `W` devenus caducs sans le Python | ici | ✅ fait le 2026-11-08 |
| 4 | **13** | Nettoyage d'après publication : `_internal`, vrai Windows, licences relues | ici | **à faire — en tête** |
| 5 | **14** | Derniers seuils « Estimation » de l'assistant (W-003) et source de W-005 | ici | à faire (corpus) |
| 6 | **11** | Ouverture de plusieurs logs en parallèle (W-050) | `plan-chantiers.md` | à faire (plafond tranché : D-051) |
| 7 | **15** | Dette de l'interface (W-052, cache, pages spectrales) — **conditionnel** | ici | attend un déclencheur mesuré |

Raison de cet ordre, revu le 2026-11-08 après la publication : le **lot 13 passe en tête** parce qu'il porte le seul point qui
peut gêner un vrai utilisateur aujourd'hui — une mise à jour par-dessus la 0.8.0 **jamais essayée** (W-044) — et parce qu'il
demande des essais de Cédric, qui peuvent se faire pendant qu'autre chose avance. Le **lot 14** vient ensuite : c'est le seul
qui touche à la **justesse des conseils donnés au pilote**. Les lots 11 et 15 sont du confort, mesuré comme tel (le budget de
60 images par seconde est tenu avec une marge de 3) : ils attendent que les retours des pilotes disent ce qui gêne vraiment.

## Ce qui revenait à Cédric : tranché le 2026-11-08

Les cinq points en attente depuis le 2026-11-06 ont été tranchés :

1. **Plafond du lot 11** → **3 fichiers décodés en même temps** (≈ 540 Mo au pire), plafond en **nombre de fichiers** et non en
   mémoire : un compteur se vérifie dans un test, une mesure de mémoire non. Écrit en **D-051** ; le lot 11 peut démarrer.
2. **AppImage et bureau (W-045)** → **soldé** : l'AppImage écrit son raccourci au premier lancement (fait le 2026-11-06,
   confirmé sur la machine de Cédric le 2026-11-07, Wayland). L'icône vide **sous X11** est close **sans suite** : cause non
   trouvée, aucun utilisateur X11 connu. W-045 est clos en entier.
3. **Aide en ligne de commande (W-031)** → close **sans suite** : l'interface graphique est la voie normale, la ligne de commande
   sert au diagnostic. `opentunebox -h` garde sa ligne d'usage.
4. **Textes des licences (W-043)** → **à relire ensemble au lot 13**, seul point qui demande encore du temps de Cédric en plus
   des essais Windows. Son échéance d'origine (« avant la première version Rust ») est passée ; nouvelle échéance : **avant la
   1.0**.
5. **Après la 0.9.0 : attendre ou enchaîner ?** → **enchaîner le ménage** (lot 12, fait le 2026-11-08), puis les lots dans
   l'ordre du tableau ci-dessus.

---

## Lot 12 : ménage des registres — ✅ fait le 2026-11-08

**But** : plus aucun `W-xxx` au statut « à voir en phase N » alors que les phases sont finies, et plus aucune panique ni message
de langage étranger laissés par la parité. **Branche** : `chore/registers-cleanup`.

**Fait**, entrée par entrée, chacune avec le geste qui la termine :

- [x] **W-018** (`report` redirigé sous Windows) → **clos, prouvé** et non supposé : `rust/otb-cli/tests/sortie_redirigee.rs`
      lance le vrai binaire avec sa sortie redirigée vers un fichier, exige de l'UTF-8 valide, le même rapport **octet pour
      octet** que `report()` en mémoire, et la présence des quatre caractères absents de cp1252 (« → », « − », « ≈ », « ≤ »),
      sans lesquels le test ne prouverait rien sous Windows. `tests.yml` le lance aussi sous Windows. Contre-preuve vue en
      échec (« ≈ » remplacé par « ~ » dans `run_stdio`).
- [x] **W-024** (`smooth` de `xcorr_delay_ms`) → **retiré** de la signature et des cinq appelants, qui passaient tous `0`.
      Aucun résultat ne change. Les cas de référence qui demandaient un lissage restent comptés par le test de parité, pour
      qu'un fichier de cas appauvri ne passe pas inaperçu.
- [x] **W-025** (`motors` en `float32` seulement) → **sans suite** : la phase 6 est écrite et aucun outil ne calcule de vitesse
      moteur en `float64`. Un second chemin de calcul sans appelant serait un second chemin à tester.
- [x] **W-020** (`raw` trié par clé) → **clos, et pas comme prévu** : la condition de reprise **est arrivée** (la Configuration
      affiche les paramètres bruts, `tools/setup.rs:LogSetup::new`) et le tri par nom a été **retenu**, parce qu'il est le choix
      d'affichage de cette page (D-038). Aucun passage à `indexmap`.
- [x] **W-021**, **W-026**, **W-027** → **sans suite** : écarts avec un étalon qui n'existe plus, sur des entrées qu'aucun
      appelant ne produit ; les corriger sans cas réel serait deviner la règle de numpy. Les écarts restent notés dans le code.
- [x] **W-022** (une seule tolérance par fonction) → **reste ouvert**, requalifié : D-047 a réglé la tolérance *par fonction*,
      pas *par champ* ; les références figées servent toujours.
- [x] **W-029 points 2 et 3** → **corrigés**. (3) `log_rate` ne **panique** plus : `profile()` se replie sur `five`
      (`DEFAULT_SIZE`), avec test et contre-preuve vue en échec. (2) l'erreur reste (le cas est inatteignable et une erreur
      signale franchement un défaut), mais son message n'imite plus Python. **Point 4 toujours ouvert** (aucun crash réel dans
      le corpus).
- [x] **W-031** (aide en ligne) → **sans suite**, arbitrage de Cédric.
- [x] **W-041**, **W-042** → laissés ouverts, passés en **⏸ BLOQUÉ** avec leur condition écrite (un échec au hasard de la CI ;
      une version d'eframe qui change ses dépendances) : ils n'attendent pas du travail, ils attendent un événement.
- [x] **W-045** → **clos en entier** : AppImage soldée et confirmée sur la machine de Cédric, X11 sans suite (arbitrage).
- [x] **W-028** → gardé ouvert, et la procédure de `docs/validation.md` **corrigée** : elle supposait le dossier `../otb-etalon`
      absent alors qu'il est déjà en place sur le poste de Cédric (`git worktree add` échoue dans ce cas), et elle ne disait pas
      que son `.venv` déjà installé vaut la peine d'être gardé entre deux réenregistrements.
- [x] **Critère d'arrêt** : aucune de ces entrées ne cachait un défaut de **calcul**. Le seul vrai défaut trouvé était une
      **panique** (W-029 point 3), corrigée ici.
- [x] **Vu au passage, non traité** : **W-053** ouvert — 45 messages d'erreur du Rust imitent encore les exceptions de Python
      et certains peuvent remonter dans la barre d'état ou dans `report`. Hors du sujet d'un lot de statuts ; l'atteignabilité
      reste à établir.
- [x] **Consigné** : `docs/travaux-reportes.md` (index et entrées), `docs/decisions.md` (D-051), `docs/plan-suite.md`,
      `docs/validation.md`, `CHANGELOG.md`. Aucune leçon : rien n'a coûté de temps inattendu.

**Vérifié** : `cargo run -p otb-dev -- verify --strict` avec le corpus, `$OTB_GOLDEN` et `OTB_BITEXACT=1` — **0 saut, tout
passe** (125 s).

---

## Lot 13 : nettoyage d'après publication (Sonnet ; essais réels par Cédric)

**But** : retirer les derniers restes de la version Python et vérifier ce que seule une vraie machine peut vérifier.
**Sa condition de démarrage est remplie depuis le 2026-11-08** (0.9.0 publiée) : c'est maintenant le **premier lot à faire**,
parce qu'il porte le seul point qui peut gêner un vrai utilisateur — la mise à jour par-dessus la 0.8.0, jamais essayée.

**Branche** : `chore/post-release-cleanup`.

**À lire** : `docs/travaux-reportes.md` W-043, W-044, W-045 ; `packaging/windows/opentunebox.iss` ;
`packaging/windows/construire_windows.ps1` ; `packaging/linux/build_appimage.sh` et `install_linux.sh` ;
`rust/otb-dev/src/licenses.rs`.

- [ ] **Mise à jour par-dessus la 0.8.0 (W-044)** : installer la 0.8.0 Python puis la 0.9.0 Rust sur le vrai Windows de
      Cédric, vérifier qu'aucun dossier `_internal` ne reste, puis **retirer** le code de nettoyage de `_internal` du `.iss`
      une fois la 0.9.0 diffusée. Ordre important : d'abord prouver que ça marche, ensuite seulement retirer le filet.
- [ ] **`construire_windows.ps1` (W-044)** : jamais exécuté sur un vrai Windows (seul le `.iss` l'a été, par la CI). Cédric
      le lance une fois ; consigner la sortie et corriger ce qui bloque.
- [x] **AppImage (W-045)** : **déjà fait et clos** le 2026-11-08 (intégration au premier lancement, confirmée sur la machine de
      Cédric le 2026-11-07 ; X11 sans suite). Plus rien à faire dans ce lot. ⚠️ Revers connu : un raccourci laissé par une
      AppImage **supprimée** empêche la suivante d'en écrire un bon.
- [ ] **Licences (W-043)** : relire avec Cédric les obligations des licences livrées, et traiter les 22 composants sans
      fichier de licence (les lister avec leur licence déclarée suffit-il ? le vérifier texte en main).
- [ ] **Vérifier** : `verify` vert ; installateur reconstruit et réinstallé après le retrait du `_internal`.
- [ ] **Consigner** : W-043, W-044, W-045 ; `CHANGELOG.md` ; `docs/validation.md` si un essai réel donne un chiffre.

---

## Lot 14 : derniers seuils « Estimation » de l'assistant (Opus ; corpus requis)

**But** : qu'aucun seuil de l'assistant ne soit plus « ni calibré ni sourcé ». C'est le seul lot qui touche à la **justesse
des conseils donnés au pilote** : un seuil mal placé fait déclencher un constat trop souvent (le pilote n'y croit plus) ou
jamais (le défaut passe).

**Branche** : `feat/calibrate-last-thresholds` (une branche par seuil si le lot est trop gros : `feat/calibrate-<seuil>`).

**À lire d'abord** : la compétence `assistant-seuils` (c'est elle qui donne la méthode), la compétence `calculs-corpus`
(pour ne pas gaspiller des heures de calcul), `docs/assistant.md` section « Origine des seuils », `docs/travaux-reportes.md`
W-003 (son « Avancement » liste tout ce qui a déjà été calibré depuis le 2026-10-12 : ne pas refaire ce travail).

**Première étape obligatoire** : établir la **liste exacte** des lignes encore marquées « Estimation » dans
`docs/assistant.md`, et la présenter à Cédric avant tout calcul. W-003 a déjà été largement traité, et le lot se dimensionne
sur cette liste.

**Compté le 2026-11-08** : il reste **deux seuils** réellement marqués « Estimation » (les trois autres occurrences du mot sont
la définition de l'origine, un renvoi à W-003 et une estimation d'effet) :

| Ligne | Seuil | Ce qu'on sait déjà |
|---|---|---|
| `assistant.md` « Taille par le nom » | bornes de détection de la taille d'après le nom du fichier (3,8 / 5,8 / 7,8 pouces, « whoop », 65, 75) | repli quand ni l'eRPM ni la tension ne donnent la taille ; lecture corrigée le 2026-10-12. **Calibration des bornes mise de côté par Cédric le 2026-10-12**, à reprendre à sa demande |
| `assistant.md` « Seuils du profil 8"+ » | seuils du profil 8 pouces et plus | **6 drones, 20 sessions** seulement : c'est W-002 (manque de données), pas un défaut de calcul — plus de logs, pas plus de calcul |

**Conséquence : ce lot est plus petit qu'on ne le croyait**, et sa seconde ligne est bloquée par les données. L'essentiel du
travail restant est **W-005** (la source de la formule de l'erreur aléatoire).

- [ ] Lister les seuils « Estimation » restants, avec pour chacun : le constat qu'il déclenche, son taux actuel sur le corpus
      (`otb-dev corpus rates`), et s'il existe une source publiée (firmware, guide RPM, configurateur).
- [ ] **Accord de Cédric sur la liste et l'ordre** avant de calibrer (critère d'arrêt du plan : ne pas improviser).
- [ ] Un seuil à la fois : mesure sur le corpus, taux avant / après, valeur proposée, et **ce qui change pour le pilote**
      (quels logs gagnent ou perdent le constat). Chaque changement de valeur est tranché par Cédric et devient un `D-xxx`.
- [ ] **W-005** (formule de l'erreur aléatoire attribuée à Bendat & Piersol sans vérification à la page) : soit on trouve
      l'ouvrage et on vérifie la page, soit on remplace la citation par ce qui est réellement prouvé (le recoupement par
      simulation, déjà fait). Pas de calcul faux connu : c'est une question d'honnêteté de la documentation.
- [ ] **Critère d'arrêt** : un seuil dont la calibration changerait les conseils sur plus de 10 % du corpus → s'arrêter et
      revenir vers Cédric avant de livrer.
- [ ] **Consigner** : `docs/assistant.md` (origine de chaque seuil), `docs/decisions.md`, W-003 (« Avancement »),
      `CHANGELOG.md`.

**Resteront ouverts après ce lot**, par manque de données et non par oubli : **W-002** (profils whoop, 2–3,5" et 8"+ appuyés
sur trop peu de drones — il faut plus de logs, pas plus de calcul) et **W-004** (la boucle ouverte ne se mesure que jusqu'à
20–40 Hz : limite physique de l'énergie du manche).

---

## Lot 15 : dette de l'interface (Opus) — **conditionnel**

**But** : reprendre les trois dettes connues de l'interface. **Ce lot ne se fait pas « pour faire bien »** : le budget de
60 images par seconde est tenu avec une marge de 3 sur la plus longue session du corpus (5,6 ms contre 16,7). Il attend un
**déclencheur mesuré**.

**Déclencheurs** (l'un suffit) : le viewer accroche sur un log plus long que le corpus ; on veut afficher beaucoup plus de
courbes à la fois ; un pilote signale une lenteur ; la mémoire du cache des outils devient visible.

**Branche** : `perf/viewer-draw` (W-052) ou `refactor/spectral-pages` selon le déclencheur.

**À lire** : `docs/travaux-reportes.md` W-052 et W-035 (point 9), `rust/otb-gui/src/viewer_page.rs:ui`,
`rust/otb-gui/src/tools/freq_time.rs` (la texture en cache, le modèle à suivre), `docs/decisions.md` D-037 et D-048,
`docs/lecons.md` L-085, L-086.

- [ ] **W-052** — le cadre est désormais dominé par le **tracé** des ≈ 3 840 points par courbe (4,7 ms en vue entière contre
      2,1 ms zoomé sur un huitième du même log, à nombre de points égal). Deux pistes, à mesurer avant de choisir : un seul
      segment vertical par colonne de pixels au lieu de deux points, ou la courbe décimée dessinée dans une texture en cache
      comme `freq_time`. Preuve exigée, comme au lot 10 : **tracé identique** (captures au bit près, les deux thèmes) et
      chiffres de `banc_cadre` avant / après.
- [ ] **Cache des outils borné (W-035 point 9)** : le cache n'évince rien entre réglages et sa taille n'a jamais été mesurée.
      La mesurer d'abord sur 10 fichiers ; la borner seulement si le chiffre le justifie.
- [ ] **Factorisation des pages spectrales** : les pages `spectral`, `freq_throttle` et `freq_time` répètent la même
      mécanique. Proposé au lot 6 ; à ne faire que si une page de plus est prévue, sinon c'est du remaniement sans effet.
- [ ] **Lints pédantiques ciblés** : activer quelques lints `clippy::pedantic` utiles, **jamais `suboptimal_flops`** (L-051 :
      il change le résultat des calculs).
- [ ] **Critère d'arrêt** : un tracé qui diffère d'un pixel entre avant et après, ou un gain inférieur à ×1,5.

---

## Hors plan (volontairement)

- **W-001** — logs KISS (`.TXT`) et INAV non décodés : hors périmètre, à rouvrir seulement si Cédric change de radio ou de
  firmware.
- **W-002**, **W-004** — bloqués par les données et par la physique, pas par le code (voir lot 14).
- **W-028** — écarts des références figées : se règle au prochain réenregistrement, pas avant.
- **W-051** — pistes de performance écartées par la mesure le 2026-11-04 : ne pas les rouvrir sans mesure nouvelle.

## Avancement

| Lot | État |
|---|---|
| 6 | ✅ Fait le 2026-11-06 (branche **docs/plan-suite**) : bilan des cinq chantiers, L-087 et L-088 écrites, W-014 et W-016 clos |
| 7 | ✅ Fait le 2026-11-08 : **0.9.0 publiée** (7a licences, 7b notes et captures, 7c quatre tours d'essai et sept défauts corrigés, 7d–7e publication). Détail dans `plan-chantiers.md` |
| 12 | ✅ Fait le 2026-11-08 (branche `chore/registers-cleanup`) : 10 entrées closes ou requalifiées, une panique corrigée, un paramètre mort retiré, W-053 ouvert, D-051 écrite |
| 13 | **À faire, en tête** : mise à jour depuis la 0.8.0 et `construire_windows.ps1` sur le vrai Windows de Cédric (W-044), licences relues avec lui (W-043) |
| 14 | À faire : deux seuils « Estimation » seulement, dont un bloqué par les données (W-002) ; l'essentiel est W-005 |
| 11 | À faire : plafond tranché (D-051, 3 fichiers) |
| 15 | Conditionnel : attend un déclencheur mesuré |

## Ce qui reste ouvert après ce plan

Pour mémoire, et sans y revenir : **W-002** et **W-004** (bloqués par les données et par la physique), **W-001** (logs KISS et
INAV, hors périmètre), **W-022** (tolérance par champ dans `otb-parity`), **W-028** (prochain réenregistrement des références),
**W-029 point 4** (aucun crash réel dans le corpus), **W-041** et **W-042** (attendent un événement), **W-053** (messages
d'erreur héritant de Python). Aucun de ces points n'empêche un pilote d'utiliser OpenTuneBox aujourd'hui.
