# Plan de travail

**Le seul plan du projet.** Ce qu'il reste à faire est en haut ; l'historique des lots déjà faits est un
tableau en bas, une ligne par lot. Il n'y a **pas d'autre document de plan** : l'état d'un travail reporté
se lit dans [travaux-reportes.md](travaux-reportes.md), jamais ici (L-092).

**État au 2026-11-08, vérifié** : la **0.9.0 est publiée** (page « dernière version », AppImage et
installateur Windows), les huit outils de PIDtoolbox Pro V0.94 sont tous couverts
([spec-ecrans.md](spec-ecrans.md), `rust/otb-gui/src/tools/mod.rs:all_pages`), `main` est propre, la CI est
verte, et `cargo run -p otb-dev -- verify --strict` avec le corpus, les références figées et la comparaison
au bit près : **0 saut, tout passe**.

Le programme est donc complet et publié. Ce qui reste tient en trois familles : **vérifier ce que seule une
vraie machine vérifie** (lot 13), **finir la calibration de l'assistant** (lot 14), et **du confort**
(lots 11 et 15).

## Comment s'en servir

- **Une session = un lot = une PR.** Faire `/clear`, puis demander « Lot N de `docs/plan.md` ». La branche
  porte le nom indiqué dans le lot ; le titre de la PR est exactement ce nom (`CONTRIBUTING.md`).
- Chaque lot suit les phases **explorer → planifier → implémenter → vérifier → livrer → consigner**.
  Explorer = lire les fichiers listés dans « À lire ». Planifier = présenter le découpage et attendre
  l'accord de Cédric. Consigner = cocher les cases du lot, mettre à jour `CHANGELOG.md` (section *Non
  publié*) et les registres ([decisions.md](decisions.md), [lecons.md](lecons.md),
  [travaux-reportes.md](travaux-reportes.md)) selon la compétence `docs-sync`.
- **Un nouveau test se prouve par contre-preuve** : réintroduire le défaut (ou le casser à la main), voir le
  test rougir, le noter dans la PR (L-071).
- **Vérification de tout lot** : `cargo run -p otb-dev -- verify` **sans tube** (un `| tail` masque son
  échec, L-022), vert avant de livrer.
- **Critère d'arrêt** : si une hypothèse d'un lot tombe, s'arrêter, consigner ce qui a été vu, et revenir
  vers Cédric ; ne pas improviser un autre découpage.
- Modèle conseillé par lot : **Sonnet** pour le mécanique, la documentation, les tests ; **Opus** pour le
  décodeur, la mémoire et le débogage difficile.
- La numérotation des lots est une **série unique**, jamais réutilisée : les lots 0 à 10, 12 et 16 sont
  faits (tableau en bas), le prochain numéro libre est **18**.

## Ordre des étapes

| # | Lot | Sujet | Dépend de |
|---|---|---|---|
| 1 | **13** | Nettoyage d'après publication : `_internal`, vrai Windows, licences relues | **à faire — en tête** |
| 2 | **14** | Derniers seuils « Estimation » de l'assistant (W-003) et source de W-005 | à faire (corpus requis) |
| 3 | **11** | Ouverture de plusieurs logs en parallèle (W-050) | à faire (plafond tranché : D-051) |
| 4 | **15** | Dette de l'interface (W-052, cache, pages spectrales) — **conditionnel** | attend un déclencheur mesuré |

Raison de cet ordre, arrêtée le 2026-11-08 après la publication : le **lot 13 passe en tête** parce qu'il
porte le seul point qui peut gêner un vrai utilisateur aujourd'hui — une mise à jour par-dessus la 0.8.0
**jamais essayée** (W-044) — et parce qu'il demande des essais de Cédric, qui peuvent se faire pendant
qu'autre chose avance. Le **lot 14** vient ensuite : c'est le seul qui touche à la **justesse des conseils
donnés au pilote**. Les lots 11 et 15 sont du confort, mesuré comme tel (le budget de 60 images par seconde
est tenu avec une marge de 3) : ils attendent que les retours des pilotes disent ce qui gêne vraiment.

---

## Lot 13 : nettoyage d'après publication (Sonnet ; essais réels par Cédric)

**But** : retirer les derniers restes de la version Python et vérifier ce que seule une vraie machine peut
vérifier. Sa condition de démarrage est remplie depuis le 2026-11-08 (0.9.0 publiée).

**Branche** : `chore/post-release-cleanup`.

**À lire** : [travaux-reportes.md](travaux-reportes.md) W-043, W-044, W-045 ;
`packaging/windows/opentunebox.iss` ; `packaging/windows/construire_windows.ps1` ;
`packaging/linux/build_appimage.sh` et `packaging/linux/install_linux.sh` ; `rust/otb-dev/src/licenses.rs`.

- [ ] **Mise à jour par-dessus la 0.8.0 (W-044)** : installer la 0.8.0 Python puis la 0.9.0 Rust sur le vrai
      Windows de Cédric, vérifier qu'aucun dossier `_internal` ne reste, puis **retirer** le code de
      nettoyage de `_internal` du `.iss` une fois la 0.9.0 diffusée. Ordre important : d'abord prouver que
      ça marche, ensuite seulement retirer le filet.
- [ ] **`construire_windows.ps1` (W-044)** : jamais exécuté sur un vrai Windows (seul le `.iss` l'a été, par
      la CI). Cédric le lance une fois ; consigner la sortie et corriger ce qui bloque.
- [ ] **Licences (W-043)** : relire avec Cédric les obligations des licences livrées, et traiter les
      22 composants sans fichier de licence (les lister avec leur licence déclarée suffit-il ? le vérifier
      texte en main). Échéance fixée : **avant la 1.0**.
- [ ] **Vérifier** : `verify` vert ; installateur reconstruit et réinstallé après le retrait du `_internal`.
- [ ] **Consigner** : W-043, W-044 ; `CHANGELOG.md` ; [validation.md](validation.md) si un essai réel donne
      un chiffre.

**Déjà fait, rien à reprendre ici** : W-045 (AppImage) est **clos** — elle écrit son raccourci de bureau au
premier lancement, confirmé sur la machine de Cédric le 2026-11-07 sous Wayland ; l'icône vide sous X11 est
sans suite (cause non trouvée, aucun utilisateur X11 connu). ⚠️ Revers connu : un raccourci laissé par une
AppImage **supprimée** empêche la suivante d'en écrire un bon.

---

## Lot 14 : derniers seuils « Estimation » de l'assistant (Opus ; corpus requis)

**But** : qu'aucun seuil de l'assistant ne soit plus « ni calibré ni sourcé ». C'est le seul lot qui touche
à la **justesse des conseils donnés au pilote** : un seuil mal placé fait déclencher un constat trop souvent
(le pilote n'y croit plus) ou jamais (le défaut passe).

**Branche** : `feat/calibrate-last-thresholds` (une branche par seuil si le lot est trop gros :
`feat/calibrate-<seuil>`).

**À lire d'abord** : la compétence `assistant-seuils` (c'est elle qui donne la méthode), la compétence
`calculs-corpus` (pour ne pas gaspiller des heures de calcul), [assistant.md](assistant.md) section
« Origine des seuils », [travaux-reportes.md](travaux-reportes.md) W-003 (son « Avancement » liste tout ce
qui a déjà été calibré depuis le 2026-10-12 : **ne pas refaire ce travail**).

**Compté le 2026-11-08** : il reste **deux seuils** réellement marqués « Estimation » (les trois autres
occurrences du mot sont la définition de l'origine, un renvoi à W-003 et une estimation d'effet). Ce lot est
donc plus petit qu'on ne le croyait, et sa seconde ligne est bloquée par les données : l'essentiel du travail
restant est **W-005**.

| Ligne d'`assistant.md` | Seuil | Ce qu'on sait déjà |
|---|---|---|
| « Taille par le nom » | bornes de détection de la taille d'après le nom du fichier (3,8 / 5,8 / 7,8 pouces, « whoop », 65, 75) | repli quand ni l'eRPM ni la tension ne donnent la taille ; lecture corrigée le 2026-10-12. **Calibration des bornes mise de côté par Cédric le 2026-10-12**, à reprendre à sa demande |
| « Seuils du profil 8"+ » | seuils du profil 8 pouces et plus | **6 drones, 20 sessions** seulement : c'est W-002 (manque de données), pas un défaut de calcul — plus de logs, pas plus de calcul |

- [ ] Lister les seuils « Estimation » restants, avec pour chacun : le constat qu'il déclenche, son taux
      actuel sur le corpus (`otb-dev corpus rates`), et s'il existe une source publiée (firmware, guide RPM,
      configurateur).
- [ ] **Accord de Cédric sur la liste et l'ordre** avant de calibrer (critère d'arrêt : ne pas improviser).
- [ ] Un seuil à la fois : mesure sur le corpus, taux avant / après, valeur proposée, et **ce qui change
      pour le pilote** (quels logs gagnent ou perdent le constat). Chaque changement de valeur est tranché
      par Cédric et devient un `D-xxx`.
- [ ] **W-005** (formule de l'erreur aléatoire attribuée à un ouvrage sans vérification à la page) : soit on
      trouve l'ouvrage et on vérifie la page, soit on remplace la citation par ce qui est réellement prouvé
      (le recoupement par simulation, déjà fait). Pas de calcul faux connu : c'est une question d'honnêteté
      de la documentation.
- [ ] **Critère d'arrêt** : un seuil dont la calibration changerait les conseils sur plus de 10 % du corpus
      → s'arrêter et revenir vers Cédric avant de livrer.
- [ ] **Consigner** : [assistant.md](assistant.md) (origine de chaque seuil), [decisions.md](decisions.md),
      W-003 (« Avancement »), `CHANGELOG.md`.

---

## Lot 11 : ouverture de plusieurs logs en parallèle (Opus) — W-050

**But** : ouvrir plusieurs fichiers à la fois (×2,4 mesuré sur 6 logs, 0,830 s → 0,345 s) **sans que le pic
de mémoire dépasse le plafond**. Le plafond est tranché : **3 fichiers décodés en même temps** au plus
(D-051), compté en **nombre de fichiers** et non en mémoire — un compteur se vérifie dans un test, une
mesure de mémoire non.

**Branche** : `perf/open-parallel`.

**À lire** : [travaux-reportes.md](travaux-reportes.md) W-050, [validation.md](validation.md) (« Ouverture
d'un log »), `rust/otb-gui/src/app.rs:open` et `rust/otb-gui/src/app.rs:poll_opening` (aujourd'hui **un
seul** fichier à la fois : la file `queue` et un unique `opening`), `rust/otb-log/src/flightlog.rs:load`,
`rust/otb-dev/src/bench_open.rs`, [lecons.md](lecons.md) L-081 et L-066, [decisions.md](decisions.md) D-051.

Le décodage de sessions d'un **même** fichier en parallèle est **écarté** : ×1,10 seulement sur `FINAL.bbl`,
et il cumulerait plusieurs tables compactes.

- [ ] Banc multi-fichiers d'abord (le lot 5 n'en mesurait qu'un seul par processus) : `otb-dev bench-open`
      accepte plusieurs logs et un nombre de fichiers simultanés, et donne durée totale et pic de mémoire du
      processus. Mesure « avant » (séquentiel) consignée sur 6 logs dont `LOG00008.BFL`.
- [ ] Ouverture parallèle avec plafond (`rayon`, ordre des logs conservé, `par_iter().collect()`), dans le
      calcul pur (`otb-log`) ; l'interface garde son rôle : afficher, annuler, remonter les erreurs
      **fichier par fichier**.
- [ ] Tests : ordre conservé quelle que soit la durée de chaque fichier ; un fichier illisible n'empêche pas
      les autres ; le plafond est respecté (compteur de fichiers en cours dans un test, pas une mesure de
      mémoire) ; résultat identique au séquentiel (empreinte de tous les `FlightLog`, comme au lot 5).
- [ ] Mesure « après » : ×2 attendu au moins sur les 6 logs ; **pic de mémoire ≤ plafond** ; un seul
      fichier : temps non dégradé.
- [ ] **Critères d'arrêt** : un `FlightLog` qui diffère d'un bit ; un pic au-dessus du plafond sur les
      6 logs ; un gain inférieur à ×1,5.
- [ ] **Consigner** : W-050 traité, [validation.md](validation.md), [architecture.md](architecture.md),
      `CHANGELOG.md`, leçon si un piège coûte du temps.

---

## Lot 15 : dette de l'interface (Opus) — **conditionnel**

**But** : reprendre les trois dettes connues de l'interface. **Ce lot ne se fait pas « pour faire bien »** :
le budget de 60 images par seconde est tenu avec une marge de 3 sur la plus longue session du corpus
(5,6 ms contre 16,7). Il attend un **déclencheur mesuré**.

**Déclencheurs** (l'un suffit) : le viewer accroche sur un log plus long que le corpus ; on veut afficher
beaucoup plus de courbes à la fois ; un pilote signale une lenteur ; la mémoire du cache des outils devient
visible.

**Branche** : `perf/viewer-draw` (W-052) ou `refactor/spectral-pages` selon le déclencheur.

**À lire** : [travaux-reportes.md](travaux-reportes.md) W-052 et W-035 (point 9),
`rust/otb-gui/src/viewer_page.rs:ui`, `rust/otb-gui/src/tools/freq_time.rs` (la texture en cache, le modèle
à suivre), [decisions.md](decisions.md) D-037 et D-048, [lecons.md](lecons.md) L-085, L-086.

- [ ] **W-052** — le cadre est désormais dominé par le **tracé** des ≈ 3 840 points par courbe (4,7 ms en
      vue entière contre 2,1 ms zoomé sur un huitième du même log, à nombre de points égal). Deux pistes, à
      mesurer avant de choisir : un seul segment vertical par colonne de pixels au lieu de deux points, ou la
      courbe décimée dessinée dans une texture en cache comme `freq_time`. Preuve exigée, comme au lot 10 :
      **tracé identique** (captures au bit près, les deux thèmes) et chiffres de `banc_cadre` avant / après.
- [ ] **Cache des outils borné (W-035 point 9)** : le cache n'évince rien entre réglages et sa taille n'a
      jamais été mesurée. La mesurer d'abord sur 10 fichiers ; la borner seulement si le chiffre le justifie.
- [ ] **Factorisation des pages spectrales** : les pages `spectral`, `freq_throttle` et `freq_time` répètent
      la même mécanique, comme la découpe de période (×6), `breaks` (×5), la découpe bornée (×9), les listes
      déroulantes (×5) et `reset_view` (×5). À ne faire que si une page de plus est prévue, sinon c'est du
      remaniement sans effet.
- [ ] **Lints pédantiques ciblés** : activer quelques lints `clippy::pedantic` utiles, **jamais
      `suboptimal_flops`** (L-051 : il change le résultat des calculs).
- [ ] **Critère d'arrêt** : un tracé qui diffère d'un pixel entre avant et après, ou un gain inférieur
      à ×1,5.

---

## Hors plan, volontairement

Ces travaux reportés sont **vus et laissés** : aucun n'empêche un pilote d'utiliser OpenTuneBox aujourd'hui.
Leur détail et leur condition de reprise sont dans [travaux-reportes.md](travaux-reportes.md) ; ne pas les
rouvrir ici sans cette condition.

| ID | Pourquoi il n'est pas dans un lot |
|---|---|
| W-001 | Logs KISS (`.TXT`) et INAV non décodés : hors périmètre, à rouvrir si Cédric change de radio ou de firmware |
| W-002 | Profils whoop, 2–3,5" et 8"+ appuyés sur trop peu de drones : il faut **plus de logs**, pas plus de calcul |
| W-004 | La boucle ouverte ne se mesure que jusqu'à 20–40 Hz : limite physique de l'énergie du manche |
| W-022 | Tolérance par fonction et non par champ dans `otb-parity` : reprise si un écart est soupçonné sur `freqs` / `bins` / `resolution_hz` |
| W-028 | Écarts des références figées : se règle au **prochain réenregistrement**, pas avant |
| W-029 (4) | Branche « crash » du guide : aucun crash réel dans le corpus, seuls des cas fabriqués |
| W-041 | Test sensible à la charge : ⏸ attend un échec au hasard de la CI ; le corriger avant reviendrait à deviner le délai |
| W-042 | Avis RustSec hérités d'eframe : ⏸ attend une version d'eframe qui change ses dépendances |
| W-051 | Pistes de performance écartées **par la mesure** le 2026-11-04 : ne pas les rouvrir sans mesure nouvelle |
| W-053 | 45 messages d'erreur imitant les exceptions de Python : atteignabilité non établie |
| W-054 | Huit des dix comparateurs d'`otb-parity` sans test : fabriquer une référence Python pour ces couches est le vrai coût |

**Écarté par arbitrage de Cédric** (« rien sans bénéfice démontrable ») : découper `rules.rs` (3 631 lignes),
`log_check.rs` (1 356) et `guide_page.rs` (1 053) — testés et validés au bit près, les remanier est un risque
sans effet visible ; les ~300 `pub` inutilement publiques (exposition trop large, pas du code mort) ; un cache
Cargo en CI (une mécanique à entretenir pour une CI qui dure 7 min).

---

## Historique des lots

Une ligne par lot fait. Le détail est dans `CHANGELOG.md`, les mesures dans [validation.md](validation.md),
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
| 7 | 2026-11-08 | **Première publication Rust, 0.9.0.** 7a `otb-dev licenses --check` (`feat/licenses-check`) ; 7b CHANGELOG et captures (`chore/release-0.9.0`) ; 7c **quatre tours** d'essai sur de vraies machines et **sept défauts** corrigés (PR #118 à #126), **aucun vu par les tests** ; 7d–7e publication, page « dernière version », brouillon d'essai supprimé | W-043 (licences non relues, échéance repoussée à la 1.0) et W-044 (mise à jour depuis la 0.8.0 jamais essayée) → **lot 13** |
| 8 | 2026-11-05 | Banc de temps de cadre (`chore/viewer-frame-bench`) : `examples/banc_cadre.rs`. Étalon retrouvé sur `FINAL.bbl` (7,4 à 8,3 ms) ; `LOG00008.BFL` 12,6 à 13,0 ms à 620 079 échantillons, sous les 16,7 ms | les 0,34 ms de carte graphique de W-049 ne se reproduisent pas (L-084) |
| 9 | — | **Non retenu** : W-048 ne devait pas se régler en affaiblissant un test. Repris hors lot le 2026-11-05 (`test/golden-tolerance`, D-047, L-083) | W-048 clos |
| 10 | 2026-11-05 | Tracé du viewer sans parcourir toute la série (`perf/viewer-pyramid`, D-048, L-085, L-086) : index multi-résolution min/max dans `otb-dsp`, cadre **÷ 2,2 à ÷ 6,5**, tracé **identique au bit près**, pic de mémoire inchangé | W-049 clos ; **W-052 ouvert** (le tracé domine maintenant) → lot 15 |
| 12 | 2026-11-08 | Ménage des registres (PR #129, `chore/registers-cleanup`) : 10 travaux reportés clos ou requalifiés, une **panique** corrigée (`log_rate`, W-029 point 3), un paramètre mort retiré (W-024), D-051 écrite. `verify --strict` 0 saut (125 s) | W-053 ouvert au passage |
| 16 | 2026-11-08 | **Revue intégrale** du code, des tests, de la documentation et de l'outillage (PR #129) : 4 relevés (~41 000 lignes de code, ~20 500 de tests, 30 `.md`, 4 workflows), 53 cases, **2 défauts réels** corrigés (`compare_header`, `slice_time`), **6 constats de relevé faux** corrigés après vérification, chaîne Rust figée (`rust-toolchain.toml`), clippy sous Windows, D-052 écrite, D-050 précisée, récidive de L-079 bridée | **W-054 ouvert** : seuls 2 des 10 comparateurs d'`otb-parity` ont reçu un test |
| 17 | 2026-11-08 | **Rassemblement des plans** (branche « docs/rassembler-les-plans ») : les quatre documents de plan deviennent ce seul `docs/plan.md`, L-092 écrite | rien |

La migration Python → Rust elle-même, terminée le 2026-11-04, garde son propre document :
[migration-rust.md](migration-rust.md) (phases, preuves d'équivalence, verdict du prototype d'interface).
