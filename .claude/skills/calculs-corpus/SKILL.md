---
name: calculs-corpus
description: Lancer vite et sans erreur un calcul sur tout le corpus de vrais logs d'OpenTuneBox (calibration de l'assistant, taux de déclenchement des constats, tableau d'étiquettes, mesure ponctuelle sur tous les logs) — passages en parallèle sur tous les cœurs, réutilisation des résultats, essai sur un échantillon d'abord. À charger avant tout passage sur le corpus ou tout script qui boucle sur les logs.
---

# Calculs sur le corpus : parallèles, réutilisés, vérifiés

Les outils du corpus sont des sous-commandes de `otb-dev` (`rust/otb-dev/src/corpus/`). Un seul processus : les fichiers
se répartissent sur un pool rayon par `corpus::map_files` (les plus gros d'abord ; `-j 1` = un fichier à la fois, pour
comparer), et le noyau de `step_response` partage ce même pool. Toujours en `--release`.

| Commande (dans `rust/`) | Rôle | Durée |
|---|---|---|
| `cargo run --release -p otb-dev -- corpus calibrate 600 SCRATCH/stats.json` | mesures brutes des seuils ; `--resume SCRATCH/stats.json` : percentiles p10 / p50 / p90 par taille | 40 s, corpus entier (2026-11-24) |
| `… corpus rates SCRATCH/rates.json` | assistant complet sur chaque session, taux de chaque constat ; `--resume` : tableau seul ; pas de `--dir` | 45 s, corpus entier (2026-11-24, après la bascule du lot 24d) |
| `… corpus labels [--csv COPIE]` | tableau `$OTB_LOGS/corpus.csv` | non mesurée (moins d'une minute sous Python) |
| `… corpus rates --compare AVANT.json APRÈS.json` | sessions dont un constat a changé (texte complet, chiffres compris) ; code 1 si différence | quasi instantanée |
| `… corpus validate [--limit N] [--start N] [-j N]` | décodeur contre `blackbox_decode` | 6 min, corpus entier, 4 fichiers à la fois |
| `… corpus stages SCRATCH/stages.json [--dir SOUS-DOSSIER]` | modèle de filtres contre le firmware, étage par étage (lot 22) ; `--resume` : tableau seul | 22 s, corpus entier (2026-11-22, lot 22b, fichiers déjà lus une fois ; 14 s avant les filtres rejoués à leur cadence, 43 s au lot 22a) |
| `… corpus dyntrack SCRATCH/dyntrack.json [--dir SOUS-DOSSIER]` | suivi du notch dynamique rejoué contre les centres `FFT_FREQ` (lot 22c) ; `--resume` : tableau seul | 21 s, corpus entier (2026-11-22, fichiers déjà lus une fois) |
| `… corpus replay SCRATCH/replay.json [--dir SOUS-DOSSIER]` | simulateur de la page Filtres : rejeu « actuel » contre le spectre enregistré (lot 22e, D-058) ; `--resume` : tableau seul | 68 s, corpus entier (2026-11-22, 675 sessions) |
| `… corpus stages --chain model …` puis `… corpus stages --compare AVANT.json APRÈS.json` | contrôle par étage avec l'ancienne chaîne (par défaut depuis le lot 24d : celle du firmware), puis comparaison des deux passages (critère d'arrêt du lot 24c, code 1 s'il est atteint) | 13 s (firmware), 23 s (ancienne), corpus entier (2026-11-24) |
| `… corpus replay --chain model SCRATCH/replay.json` | rejeu « actuel » par l'ancienne chaîne (par défaut depuis le lot 24d : celle du firmware) | 26 s, corpus entier (2026-11-24 ; 66 s pour l'ancienne chaîne le même jour) |
| `… corpus chaindiff SCRATCH/chaindiff.json [--dir SOUS-DOSSIER]` | écart entre la chaîne actuelle et celle à l'arithmétique du firmware, par version et bande (lot 24c) ; `--resume` : tableau seul | 69 s, corpus entier (2026-11-24, 676 sessions) |
| `… corpus thin SCRATCH/thin.json [--factor N] [--dir SOUS-DOSSIER]` | logs enregistrés à chaque boucle ramenés à une boucle sur N : d'où vient l'écart d'un rejeu sur les logs ralentis (W-083), chaque chaîne contre un rejeu idéal ; `--resume` : tableau seul | 19 s, corpus entier (2026-11-24, 32 sessions) |
| `… corpus predict AVANT.bbl APRÈS.bbl` | vérification en vol du simulateur (lot 22f) : filtres d'APRÈS appliqués à AVANT, rejeu et rapport des \|H\|² contre APRÈS, par bande ; deux vols à la même cadence | 0,8 s par paire (Air65, deux vols de 43 s, 2026-11-22) |

Relever la durée du premier passage et la noter ici : c'est une mesure à faire, pas à supposer. ⚠️ La durée **par session**
d'un passage en parallèle (`t_analyze`) est gonflée par les autres fils : la mesurer seule, `-j 1` (L-106).

**Prouver qu'une modification ne change aucun conseil** (optimisation, refactorisation) : un passage de
`corpus rates` sur `main` (copie isolée par `git worktree add SCRATCH/main main`, lancée depuis ce dossier), un sur la
branche, puis `--compare`. Une différence se recalcule seule (`-j 1`, ou la session isolée) avant d'accuser le code :
elle peut venir des inversions de bits du poste (voir Pièges). Ne pas modifier le code pendant un passage : le binaire
est déjà compilé, mais le résultat ne correspondrait plus au code lu.

`SCRATCH` = répertoire temporaire de la session (jamais dans le dépôt).

## Procédure

1. **Réutiliser avant de recalculer.** Un JSON d'une session précédente reste valable si aucun fichier du
   chemin mesuré n'a changé depuis : comparer sa date (`ls -l`) avec `git log -1 --format=%ci -- rust`.
   Les dossiers temporaires des sessions précédentes sont dans `/tmp/claude-1000/<projet>/*/scratchpad/`.
   Dans le doute, recalculer.
2. **Essai sur un échantillon** : `corpus calibrate 40 …` ou `corpus rates … --limit 40`. Corriger les erreurs avant
   le passage complet.
3. **Passage complet en arrière-plan** (Bash `run_in_background`, notification à la fin : ne pas
   sonder en boucle). Un seul passage complet à la fois : deux passages se disputent les cœurs.
   Pendant ce temps, préparer la suite (lecture du code, rédaction).
4. **Lire les erreurs** (`ERR` sur la sortie d'erreur). Un fichier en erreur est refait seul à la fin ;
   une erreur qui reste est réelle.

## Nouvelle mesure ponctuelle sur tout le corpus

Ne pas écrire de boucle séquentielle jetable. Une fonction **par fichier** (`Fn(&Path) -> Result<R, String>`) passée à
`corpus::map_files(work, &log_files(&root)?, jobs, &root)` ; elle ouvre le log par `otb_log::flightlog::load` et ne garde
que les sessions de 8 s ou plus (`corpus::analysed` donne la période analysée par l'assistant). Modèle à suivre :
`calibrate.rs` (mesures brutes, JSON par `json.rs` pour les `NaN`).

**Enregistrer les valeurs brutes, dépouiller à part.** Ajouter au passage complet les champs bruts utiles
(gains PID, curseurs, réglages qui peuvent fausser la mesure), puis écrire le dépouillement (percentiles,
groupes, taux de paliers candidats) comme un petit script qui lit le JSON, dans le répertoire temporaire :
il tourne en moins d'une seconde et s'essaie autant de fois que nécessaire, sans relancer le corpus
(exemple du 2026-10-05 : pic de réponse séparé selon le feedforward, cinq jeux de paliers comparés).
Confirmer ensuite le choix avec `corpus rates` (vraies règles).

Si la mesure doit servir plusieurs fois (taux, seuil), l'ajouter à `calibrate.rs` ou `rates.rs` plutôt qu'à un
programme jetable.

## Pièges

- **Inversions de bits sur le poste de Cédric** (diagnostic du 2026-10-05) : sous pleine charge, environ
  un décodage sur 70 contient **un bit inversé** (bit 3 ou 23 d'une valeur, à un endroit différent à chaque
  fois), recopié dans les trames P jusqu'à la trame I suivante. Pas de lecture hors limites
  (le test d'alors portait sur l'ancien décodeur Python, avec ses propres garde-fous) : matériel (RAM ou processeur instable,
  pas d'ECC). Effets : quelques sessions aux valeurs légèrement différentes entre deux passages, et de rares
  `IndexError` aux indices énormes. Les percentiles n'en sont pas changés ; pour une session précise qui
  surprend, la recalculer seule (`-j 1`). Ne pas chercher de défaut logiciel au décodeur pour ce symptôme.
- Sortie d'un passage en arrière-plan : ne pas la filtrer par `| tail` ou `| grep` si l'on veut suivre
  l'avancement (rien n'apparaît avant la fin) ; attendre la notification de fin.
- Arrêter un passage : `pgrep -f "otb-dev corpus calibrate"` puis `kill PID`. Jamais
  `pkill -f MOTIF` dans une commande dont le texte contient ce motif : elle se tue elle-même.
- Fils : un seul processus et un seul pool rayon ; `RAYON_NUM_THREADS=1` par processus (L-038) ne sert plus. Ne jamais
  lancer deux passages complets en même temps. Pour `validate`, la mémoire (≈ 1 Go au pic pour 4 fichiers) limite
  le nombre de fils, pas les cœurs.
- `otb-dev corpus validate` (comparaison avec `blackbox_decode`) lance la référence avec un délai de 180 s par fichier
  (3 fichiers la bloquent : `refhang`), 4 fichiers à la fois par défaut (≈ 1 Go de mémoire au pic ; `-j N` pour changer),
  et se reprend par `--start N`. Corpus entier : ≈ 6 min.
