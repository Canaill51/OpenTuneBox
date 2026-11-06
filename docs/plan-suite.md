# Plan de la suite : publier, puis solder la dette

Suite de [plan-chantiers.md](plan-chantiers.md), dont les lots 0 à 5 et 8 à 10 sont faits. Les cinq chantiers de l'audit du
2026-11-04 sont traités ; ce qui reste tient en trois familles : **publier la première version Rust**, **solder les travaux
reportés** (`W-xxx`), et **finir la calibration de l'assistant**.

Point de départ, le 2026-11-06 : `main` propre, aucune PR ni branche en cours, aucun ticket ouvert, et les huit outils de
PIDtoolbox Pro V0.94 sont tous couverts (`docs/spec-ecrans.md`, `rust/otb-gui/src/tools/mod.rs:all_pages`). Le programme est
donc complet et jamais publié : **la publication est la priorité**.

## Comment s'en servir

Les règles de travail sont celles de [plan-chantiers.md](plan-chantiers.md) (« Comment s'en servir ») et ne sont pas répétées
ici : une session = un lot = une PR, phases explorer → planifier → implémenter → vérifier → livrer → consigner, contre-preuve
pour tout nouveau test, `cargo run -p otb-dev -- verify` vert avant de livrer, critère d'arrêt si une hypothèse tombe.

La numérotation des lots **continue la même série** : les lots 6, 7 et 11 restent décrits dans `plan-chantiers.md`, les lots 12
à 15 ci-dessous. Demander « Lot N » suffit : les deux fichiers ne réutilisent jamais un numéro.

## Ordre des étapes

| # | Lot | Sujet | Où il est décrit | Dépend de |
|---|---|---|---|---|
| 1 | **6** | Bilan des cinq chantiers, W clos, leçons | `plan-chantiers.md` | — |
| 2 | **7** | Première publication Rust **0.9.0** (7a licences, 7b–7e publication) | `plan-chantiers.md` | lot 6 |
| 3 | **12** | Ménage des registres : solder les `W` devenus caducs sans le Python | ici | — (peut passer avant le 7) |
| 4 | **13** | Nettoyage d'après publication : `_internal`, vrai Windows, AppImage, licences relues | ici | **0.9.0 publiée** |
| 5 | **11** | Ouverture de plusieurs logs en parallèle (W-050) | `plan-chantiers.md` | plafond à trancher |
| 6 | **14** | Derniers seuils « Estimation » de l'assistant (W-003) et source de W-005 | ici | corpus |
| 7 | **15** | Dette de l'interface (W-052, cache, pages spectrales) — **conditionnel** | ici | déclencheur mesuré |

Raison de cet ordre : les lots 6 et 7 amènent le programme entre les mains de pilotes, ce qu'aucun autre lot n'apporte ; le lot
12 ne touche presque pas au code et peut se glisser n'importe où ; le lot 13 n'a de sens qu'une fois la 0.9.0 publiée (il
retire les restes de la version Python et vérifie une mise à jour par-dessus la 0.8.0) ; les lots 11, 14 et 15 sont du
confort et de la justesse, à faire quand les retours des pilotes auront dit ce qui gêne vraiment.

## Ce qui revient à Cédric (à trancher)

1. **Plafond du lot 11.** Ouvrir plusieurs logs en parallèle va ×2,4 (6 logs : 0,830 s → 0,345 s), mais chaque fichier décodé
   en même temps coûte 168 à 181 Mo de pic mémoire (mesuré). Il faut un plafond : soit un nombre de fichiers décodés
   simultanément (2, 3, 4…), soit une mémoire maximale. Repère : au lot 10, le plafond accepté pour l'index du viewer était
   10 % du pic. **Sans ce chiffre, le lot 11 ne démarre pas.**
2. **AppImage et bureau (W-045).** Une AppImage lancée telle quelle n'est pas connue du bureau : icône générique. Faut-il
   proposer une intégration au premier lancement (le programme écrit lui-même son `.desktop` dans `~/.local`), ou laisser
   ainsi ? L'icône vide sous X11 reste à creuser seulement si quelqu'un utilise X11.
3. **Aide en ligne de commande (W-031).** `opentunebox -h` n'écrit que la ligne d'usage. Veux-tu une vraie aide détaillée
   (une page par commande, comme `git -h`) ? Sinon W-031 se clôt sans suite au lot 12.
4. **Textes des licences (W-043).** `THIRD-PARTY-LICENSES.txt` est livré, mais les obligations des licences n'ont pas été
   relues avec toi, et 22 composants (surtout la famille egui) n'ont pas de fichier de licence dans leur paquet. À relire
   ensemble une fois, au lot 13.
5. **Après la 0.9.0 : attendre ou enchaîner ?** Soit on attend des retours de pilotes avant les lots 11, 14 et 15, soit on
   enchaîne. Mon conseil : attendre une à deux semaines, les retours changeront l'ordre.

---

## Lot 12 : ménage des registres (Sonnet, une courte session)

**But** : plus aucun `W-xxx` au statut « à voir en phase N » alors que les phases sont finies. Depuis la suppression du Python
(7-5), une dizaine d'entrées comparent le Rust à un Python qui n'existe plus, ou décrivent un paramètre sans appelant. Chacune
se termine par un geste : **corriger** (petit), **retirer** (code mort) ou **clore** avec sa raison écrite.

**Branche** : `chore/registers-cleanup`.

**À lire** : `docs/travaux-reportes.md` (les entrées ci-dessous), `docs/validation.md` (« Refaire une référence Python »),
`rust/otb-cli/src/lib.rs:parse`, `rust/otb-analysis/src/delay.rs`, `rust/otb-log/src/flightlog.rs`, `rust/otb-advisor/src/log_check.rs`.

**Constat de départ (2026-11-06, lu dans le code)** : W-018 est écrit pour l'ancien module Python `cli.py`, supprimé en 7-5 ; le
binaire Rust écrit de l'UTF-8 sur sa sortie quoi qu'il arrive (`otb_cli::run_stdio`, aucun appel à l'API console de Windows),
donc le défaut est très probablement **sans objet** — à confirmer par un test qui redirige la sortie, pas par raisonnement seul.

- [ ] **W-018** (`report` redirigé sous Windows) : ajouter un test qui lance le binaire avec sa sortie redirigée vers un
      fichier et relit le « ≈ » ; s'il passe sous Windows, clore W-018 « sans objet depuis 7-5 ». S'il échoue, le corriger
      dans ce lot (c'est un défaut visible par un pilote).
- [ ] **W-024** (`xcorr_delay_ms` refuse `smooth > 1`) et **W-025** (`motors` n'accepte que du `float32`) : paramètre sans
      appelant depuis 7-5. Décider pour chacun : retirer le paramètre (plus simple à lire) ou le porter. Mon avis : retirer,
      et le noter dans la leçon correspondante.
- [ ] **W-020** (`LogHeader.raw` trié par clé), **W-021** (entrées irréalistes, ARM), **W-022** (une seule tolérance par
      fonction dans `otb-parity`), **W-026**, **W-027** : plus de Python à égaler. Les clore « sans objet » une par une, ou
      les requalifier en défaut propre du Rust si le code actuel est réellement faux (le vérifier, ne pas supposer).
- [ ] **W-029 points 2 à 4** (`pid_khz` toujours flottant, `auto_status` en erreur si un usage manque, `log_rate` panique sur
      une taille inconnue) : ce sont des défauts du Rust **sans appelant réel**. Une panique reste une panique : corriger au
      moins `log_rate` (rendre un `Err` ou une valeur de repli), tester, puis clore.
- [ ] **W-031** (aide en ligne) : selon la réponse de Cédric (point 3 ci-dessus), l'écrire ou clore sans suite.
- [ ] **W-041** (test sensible à la charge) et **W-042** (4 avis « non maintenu » hérités d'eframe) : laisser ouverts, mais
      mettre leur condition de reprise à jour (« si la CI échoue dessus » / « à la prochaine mise à jour d'eframe »).
- [ ] **W-028** : garder ouvert, c'est le prochain réenregistrement des références ; vérifier seulement que la procédure de
      `docs/validation.md` est encore juste sans Python dans le dépôt.
- [ ] **Critère d'arrêt** : si l'une de ces entrées cache un vrai défaut de calcul (pas seulement un écart avec Python),
      s'arrêter, le consigner, et revenir vers Cédric : il mérite son propre lot.
- [ ] **Consigner** : `docs/travaux-reportes.md` (statuts et index), `CHANGELOG.md` si du code change, leçon si un piège
      coûte du temps.

**Valeur** : l'index des travaux reportés redevient lisible — ce qui y reste est ce qui compte vraiment. **Risque** : faible,
presque aucun code livré.

---

## Lot 13 : nettoyage d'après publication (Sonnet ; essais réels par Cédric)

**But** : retirer les derniers restes de la version Python et vérifier ce que seule une vraie machine peut vérifier.
**Ne démarre qu'une fois la 0.9.0 publiée et installée par au moins un pilote** (W-044 le dit : « la version qui suit la
première publication Rust »).

**Branche** : `chore/post-release-cleanup`.

**À lire** : `docs/travaux-reportes.md` W-043, W-044, W-045 ; `packaging/windows/opentunebox.iss` ;
`packaging/windows/construire_windows.ps1` ; `packaging/linux/build_appimage.sh` et `install_linux.sh` ;
`rust/otb-dev/src/licenses.rs`.

- [ ] **Mise à jour par-dessus la 0.8.0 (W-044)** : installer la 0.8.0 Python puis la 0.9.0 Rust sur le vrai Windows de
      Cédric, vérifier qu'aucun dossier `_internal` ne reste, puis **retirer** le code de nettoyage de `_internal` du `.iss`
      une fois la 0.9.0 diffusée. Ordre important : d'abord prouver que ça marche, ensuite seulement retirer le filet.
- [ ] **`construire_windows.ps1` (W-044)** : jamais exécuté sur un vrai Windows (seul le `.iss` l'a été, par la CI). Cédric
      le lance une fois ; consigner la sortie et corriger ce qui bloque.
- [ ] **AppImage (W-045)** : selon la réponse de Cédric (point 2), écrire l'intégration au bureau au premier lancement, ou
      clore W-045 côté AppImage « écart assumé » et le garder ouvert seulement pour X11.
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
`docs/assistant.md`, et la présenter à Cédric avant tout calcul. W-003 a déjà été largement traité ; on ne sait pas combien
de lignes restent, et le lot se dimensionne là-dessus.

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
| 6 | À faire (décrit dans `plan-chantiers.md`) |
| 7 | À faire (décrit dans `plan-chantiers.md`) |
| 11 | À faire (décrit dans `plan-chantiers.md`), après la réponse de Cédric sur le plafond |
| 12 | À faire |
| 13 | À faire, après la publication de la 0.9.0 |
| 14 | À faire |
| 15 | Conditionnel : attend un déclencheur mesuré |
