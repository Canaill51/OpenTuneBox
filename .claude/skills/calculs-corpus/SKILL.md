---
name: calculs-corpus
description: Lancer vite et sans erreur un calcul sur tout le corpus de vrais logs d'OpenTuneBox (calibration de l'assistant, taux de déclenchement des constats, tableau d'étiquettes, mesure ponctuelle sur tous les logs) — passages en parallèle sur tous les cœurs, réutilisation des résultats, essai sur un échantillon d'abord. À charger avant tout passage sur le corpus ou tout script qui boucle sur les logs.
---

# Calculs sur le corpus : parallèles, réutilisés, vérifiés

Les scripts du corpus répartissent les fichiers sur **tous les cœurs** par `scripts/corpus_labels.py:map_files`
(les plus gros fichiers d'abord ; `-j 1` = un fichier à la fois, pour comparer). Durées mesurées le
2026-10-05 sur le poste de Cédric (Ryzen 9 5900X, 12 cœurs / 24 fils, 539 logs, 681 sessions) :

| Script | Rôle | Durée |
|---|---|---|
| `scripts/calibrate_advisor.py 600 SCRATCH/stats.json` | mesures brutes des seuils (`--resume` : percentiles, < 1 s) | 2 min 40 (≈ 10 min un fichier à la fois) |
| `scripts/advisor_rates.py SCRATCH/rates.json` | assistant complet sur chaque session, taux de chaque constat (`--resume` : tableau seul) | 2 min 10 |
| `scripts/corpus_labels.py` | tableau `$OTB_LOGS/corpus.csv` | 40 s (≈ 3 min un fichier à la fois) |

`SCRATCH` = répertoire temporaire de la session (jamais dans le dépôt).

## Procédure

1. **Réutiliser avant de recalculer.** Un JSON d'une session précédente reste valable si aucun fichier du
   chemin mesuré n'a changé depuis : comparer sa date (`ls -l`) avec `git log -1 --format=%ci -- src scripts`.
   Les dossiers temporaires des sessions précédentes sont dans `/tmp/claude-1000/<projet>/*/scratchpad/`.
   Dans le doute, recalculer : c'est maintenant rapide.
2. **Essai sur un échantillon** : `calibrate_advisor.py 40 …` ou `advisor_rates.py … --limit 40`
   (≈ 20 s). Corriger les erreurs avant le passage complet.
3. **Passage complet en arrière-plan** (Bash `run_in_background`, notification à la fin : ne pas
   sonder en boucle). Un seul passage complet à la fois : deux passages se disputent les 12 cœurs.
   Pendant ce temps, préparer la suite (lecture du code, rédaction).
4. **Lire les erreurs** (`ERR` sur la sortie d'erreur). Un fichier en erreur dans un processus est refait
   seul à la fin ; une erreur qui reste est réelle.

## Nouvelle mesure ponctuelle sur tout le corpus

Ne pas écrire de boucle séquentielle jetable : une fonction **par fichier**, au niveau du module, et
`map_files` sous `if __name__ == "__main__"` (obligatoire sous Windows / macOS, qui relancent le module
dans chaque processus) :

```python
import json, os, sys
from functools import partial
sys.path.insert(0, "scripts")
from corpus_labels import log_files, logs_root, map_files
from opentunebox.log.flightlog import load

def measure(f, root):
    return [{"file": os.path.relpath(f, root), "sess": lg.session, ...} for lg in load(f) if lg.duration >= 8]

if __name__ == "__main__":
    root = logs_root()
    res = map_files(partial(measure, root=root), log_files(root), root=root)
    json.dump([r for _, rows in res for r in rows], open(sys.argv[1], "w"))
```

**Enregistrer les valeurs brutes, dépouiller à part.** Ajouter au passage complet les champs bruts utiles
(gains PID, curseurs, réglages qui peuvent fausser la mesure), puis écrire le dépouillement (percentiles,
groupes, taux de paliers candidats) comme un petit script qui lit le JSON, dans le répertoire temporaire :
il tourne en moins d'une seconde et s'essaie autant de fois que nécessaire, sans relancer le corpus
(exemple du 2026-10-05 : pic de réponse séparé selon le feedforward, cinq jeux de paliers comparés).
Confirmer ensuite le choix avec `advisor_rates.py` (vraies règles).

Si la mesure doit servir plusieurs fois (taux, seuil), l'ajouter à `calibrate_advisor.py` ou
`advisor_rates.py` plutôt qu'à un script jetable.

## Pièges

- **Inversions de bits sur le poste de Cédric** (diagnostic du 2026-10-05) : sous pleine charge, environ
  un décodage sur 70 contient **un bit inversé** (bit 3 ou 23 d'une valeur, à un endroit différent à chaque
  fois), recopié dans les trames P jusqu'à la trame I suivante. Pas de lecture hors limites
  (`NUMBA_BOUNDSCHECK=1`), pas d'ordre de `set` (`PYTHONHASHSEED`) : matériel (RAM ou processeur instable,
  pas d'ECC). Effets : quelques sessions aux valeurs légèrement différentes entre deux passages, et de rares
  `IndexError` aux indices énormes. Les percentiles n'en sont pas changés ; pour une session précise qui
  surprend, la recalculer seule (`-j 1`). Ne pas chercher de défaut logiciel au décodeur pour ce symptôme.
- Sortie d'un passage en arrière-plan : ne pas la filtrer par `| tail` ou `| grep` si l'on veut suivre
  l'avancement (rien n'apparaît avant la fin) ; attendre la notification de fin.
- Arrêter un passage : `pgrep -f "scripts/calibrate_advisor.py"` puis `kill PID`. Jamais
  `pkill -f MOTIF` dans une commande dont le texte contient ce motif : elle se tue elle-même.
- Limiter les fils de numpy / numba par processus (`OMP_NUM_THREADS=1`…) ne fait rien gagner ici (essai du
  2026-10-05) : la durée est fixée par les cœurs physiques et le plus gros fichier.
- `scripts/validate_corpus.py` (comparaison avec `blackbox_decode`) reste séquentiel : il pilote un
  programme externe avec un délai de 180 s par fichier et se reprend par `--start`.
