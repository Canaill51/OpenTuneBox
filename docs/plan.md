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
l'assistant n'est sans origine ni condition** (lot 14), sans qu'une seule valeur ait eu à changer.

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
- La numérotation des lots est une **série unique**, jamais réutilisée : les lots 0 à 14 et 16 à 19
  sont au tableau en bas (faits, non retenus ou supprimés), le prochain numéro libre est **20**.

## Ordre des étapes

| # | Lot | Sujet | Dépend de |
|---|---|---|---|
| 1 | **15** | Dette de l'interface (W-052, pages spectrales) — **conditionnel** | le volet « cache » est soldé (L-094, D-053) ; le reste attend un déclencheur mesuré |

Il ne reste que du **confort**, mesuré comme tel : le budget de 60 images par seconde est tenu avec une marge
de 3 sur la plus longue session du corpus. Les lots qui touchaient à un utilisateur réel ont été réglés le
2026-11-10 : le **13** s'est avéré sans objet (personne n'a eu la 0.8.0), le **14** est fait sans qu'aucune
valeur de seuil ait eu à changer, et le **11** a livré l'ouverture de plusieurs logs à la fois (× 2,26). Il ne
reste que le **15**, dont le volet « cache des outils » est **soldé le 2026-11-10** : mesuré (L-094), allégé, puis borné
(D-053) — **W-035 est clos**. Ses autres volets (tracé du viewer, pages spectrales) attendent toujours leur déclencheur
mesuré : ils peuvent attendre que les retours des pilotes disent ce qui gêne vraiment.

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

## Hors plan, volontairement

Ces travaux reportés sont **vus et laissés** : aucun n'empêche un pilote d'utiliser OpenTuneBox aujourd'hui.
Leur détail et leur condition de reprise sont dans [travaux-reportes.md](travaux-reportes.md) ; ne pas les
rouvrir ici sans cette condition. Deux d'entre eux (W-043 et W-044) viennent du lot 13 supprimé : ils n'ont
jamais dépendu de la 0.8.0, et gardent donc leur condition de reprise. **W-041 a quitté ce tableau le
2026-11-10** : sa condition de reprise — un échec au hasard de la CI sur ce test — a été atteinte, et il est
clos (L-095). C'est à cela que sert une condition de reprise écrite d'avance.

| ID | Pourquoi il n'est pas dans un lot |
|---|---|
| W-001 | Logs KISS (`.TXT`) et INAV non décodés : hors périmètre, à rouvrir si Cédric change de radio ou de firmware |
| W-002 | Profils whoop, 2–3,5" et 8"+ appuyés sur trop peu de drones : il faut **plus de logs**, pas plus de calcul |
| W-004 | La boucle ouverte ne se mesure que jusqu'à 20–40 Hz : limite physique de l'énergie du manche |
| W-022 | Tolérance par fonction et non par champ dans `otb-parity` : reprise si un écart est soupçonné sur `freqs` / `bins` / `resolution_hz` |
| W-028 | Écarts des références figées : se règle au **prochain réenregistrement**, pas avant |
| W-029 (4) | Branche « crash » du guide : aucun crash réel dans le corpus, seuls des cas fabriqués |
| W-042 | Avis RustSec hérités d'eframe : ⏸ attend une version d'eframe qui change ses dépendances |
| W-043 | Relecture des textes de licence livrés (22 composants sans fichier de licence) : **échéance avant la 1.0** — laissée hors lot par Cédric le 2026-11-10, à reprendre en préparant la 1.0 |
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

Une ligne par lot fait — ou par lot abandonné, pour que son numéro ne serve jamais à autre chose. Le détail est dans `CHANGELOG.md`, les mesures dans [validation.md](validation.md),
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

La migration Python → Rust elle-même, terminée le 2026-11-04, garde son propre document :
[migration-rust.md](migration-rust.md) (phases, preuves d'équivalence, verdict du prototype d'interface).
