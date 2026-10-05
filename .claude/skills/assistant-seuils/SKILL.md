---
name: assistant-seuils
description: Méthode pour créer, modifier, calibrer ou justifier un seuil, une règle ou un profil de taille de l'assistant de réglage d'OpenTuneBox (advisor/profiles.py, advisor/rules.py, scripts/calibrate_advisor.py, docs/assistant.md). À charger avant de toucher à un seuil ou à une règle de l'assistant, ou quand Cédric demande d'où vient une valeur.
---

# Seuils de l'assistant : calibrer et sourcer

Principe fixé avec Cédric : **aucun seuil sans origine notée**. Chaque valeur de `advisor/profiles.py` et
de `advisor/rules.py` figure dans `docs/assistant.md`, section « Origine des seuils », avec l'une des
origines : **Corpus**, **Communauté**, **Théorie**, **Firmware**, **Estimation**. Une estimation est
permise, mais elle doit être déclarée comme telle et listée pour être vérifiée plus tard.

## 1. Calibrer sur le corpus

```sh
.venv/bin/python scripts/corpus_labels.py                                  # ≈ 3 min, met à jour $OTB_LOGS/corpus.csv
.venv/bin/python scripts/calibrate_advisor.py 600 SCRATCH/stats.json        # ≈ 10 min, tout le corpus
.venv/bin/python scripts/calibrate_advisor.py --resume SCRATCH/stats.json   # p10/p50/p90 par taille
```

`SCRATCH` = répertoire de travail temporaire de la session (jamais dans le dépôt). Lancer le calcul en
arrière-plan. `--resume` reclasse les sessions avec les limites de taille actuelles
(`profiles.profile_key_from_hz`), sans tout recalculer. Avec `corpus.csv` (tableau d'étiquettes rempli
par Cédric, `docs/assistant.md`, « Corpus de référence »), chaque groupe est suivi de sa partie
**référence** (drones `regle` ou `origine`) : la préférer pour fixer un seuil quand elle compte assez de
drones. `corpus_labels.py` ne réécrit jamais les colonnes `etat`, `taille_reelle` et `remarque`.

Pour une **nouvelle mesure** : l'ajouter dans la boucle du script (même calcul que la règle ; si la règle
mélange mesure et décision, séparer la mesure dans une fonction réutilisable, comme
`rules.frame_resonance`), puis relancer.

## 2. Choisir le seuil

- Seuil « élevé » = **90e percentile** de la taille. Il est relatif : « plus que 90 % des drones du
  corpus », pas « mauvais » dans l'absolu. Le dire ainsi dans les textes.
- **Compter les drones, pas seulement les sessions** : `--resume` donne les deux. Quand quelques drones
  font la majorité des sessions d'une taille (cas du whoop : un AIR65 = 63 sessions sur 78), utiliser
  une voix par drone.
- **Moins d'environ 20 drones** dans une taille : percentile instable (un seul drone le fixe). Utiliser le
  seuil commun à tout le corpus (seuils mécaniques), ou marquer le profil `provisional=True`.
- Rejeter un percentile sans sens physique (ex. saturation p90 = 0 % en 7" → la moindre saturation
  serait signalée) : seuil commun à la place, et l'écrire dans la doc.
- Vérifier le **taux de déclenchement** d'un seuil sur le corpus (part des sessions signalées) : très
  au-dessus de 10 % → seuil trop sévère ; proche de 0 % → règle inutile.
- Arrondis : bruit au 0,1 dB ; latence p10 vers le bas, p90 vers le haut (ms entière) ; retards de filtres
  au dixième supérieur (+0,1 ms).
- **Ne changer aucun seuil sans montrer les chiffres à Cédric et avoir son accord.**

## 3. Sourcer une valeur qui ne vient pas du corpus

- **Firmware / configurateur** : clones dans `/home/cedric/Documents/Github/betaflight` et
  `betaflight-configurator`. Citer fichier et commit dans `docs/assistant.md` (« Sources vérifiées »).
- **Théorie** : citer l'ouvrage, la section et la page, vérifiées dans le texte (pas de mémoire).
  Déjà vérifiés : Skogestad et Postlethwaite 2005 § 2.4.3 (Ms ≤ 2) ; Åström et Murray 2008 § 9.3
  (marge de phase 30–60°).
- **Communauté** : une valeur répétée par des pilotes reste « Communauté » tant qu'aucune source
  précise (URL, auteur, date) n'est notée.
- **Fonctionnalité récente d'un autre outil** : vérifier son historique (PR, version publiée ou non,
  essais en vol, tickets ouverts) avant de la citer. L'onglet Autotune du configurateur (2026) a été
  examiné et **n'est pas une source** (voir `docs/assistant.md`).

## 4. Pièges

- Pic de sensibilité et marge de phase utilisent L = T/(1 − T) : approché, car dans Betaflight le D agit
  sur le gyro et le feedforward sur la consigne (`docs/assistant.md`, « Limite connue »). Les limites des
  manuels s'y appliquent seulement par analogie.
- Limites de taille par fréquence moteur : peu fiables entre 250 et 400 Hz (whoops 2,5", 3,5" lents).
  Vérifier par drone (nom + fréquence médiane), pas par session.
- Le corpus mélange des logs de demandes d'aide (« 1. PTB », « 3. BBLR FB ») et des drones connus
  (dossiers des amis de Cédric) : un percentile du corpus entier est probablement trop indulgent.
  Comparer avec le groupe « référence » de `--resume`.

## 5. Terminer

1. Valeurs dans `advisor/profiles.py` (arguments nommés, commentaire de calibration daté).
2. `docs/assistant.md` : sections « Taille du drone », « Calibration » (tableaux), « Règles » et
   « Origine des seuils » ; `CHANGELOG.md` (*Non publié*). Puis la compétence `docs-sync`.
3. Test dans `tests/test_advisor.py` si une règle ou une limite de taille change.
4. `pytest` et `pyflakes` propres.
