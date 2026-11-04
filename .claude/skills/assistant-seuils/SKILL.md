---
name: assistant-seuils
description: Méthode pour créer, modifier, calibrer ou justifier un seuil, une règle ou un profil de taille de l'assistant de réglage d'OpenTuneBox (rust/otb-advisor/src/profiles.rs, rules.rs, `otb-dev corpus calibrate`, docs/assistant.md). À charger avant de toucher à un seuil ou à une règle de l'assistant, ou quand Cédric demande d'où vient une valeur.
---

# Seuils de l'assistant : calibrer et sourcer

Principe fixé avec Cédric : **aucun seuil sans origine notée**. Chaque valeur de `rust/otb-advisor/src/profiles.rs` et
de `rules.rs` figure dans `docs/assistant.md`, section « Origine des seuils », avec l'une des
origines : **Corpus**, **Communauté**, **Théorie**, **Firmware**, **Estimation**. Une estimation est
permise, mais elle doit être déclarée comme telle et listée pour être vérifiée plus tard.

## 1. Calibrer sur le corpus

```sh
(cd rust && cargo run --release -p otb-dev -- corpus labels)                      # met à jour $OTB_LOGS/corpus.csv
(cd rust && cargo run --release -p otb-dev -- corpus calibrate 600 SCRATCH/stats.json)   # tout le corpus
(cd rust && cargo run --release -p otb-dev -- corpus calibrate --resume SCRATCH/stats.json)   # p10/p50/p90 par taille
(cd rust && cargo run --release -p otb-dev -- corpus rates SCRATCH/rates.json)   # taux de chaque constat
```

Les outils tournent en parallèle sur tous les cœurs : charger la compétence `calculs-corpus` avant de les
lancer (essai sur un échantillon, réutilisation d'un JSON récent, bits inversés sur le poste de Cédric).

`SCRATCH` = répertoire de travail temporaire de la session (jamais dans le dépôt). Lancer le calcul en
arrière-plan. `--resume` reclasse les sessions avec les limites de taille actuelles
(`profiles.profile_key`, fréquence moteur et cellules), sans tout recalculer. **Tout le corpus compte** :
Cédric ne peut pas dire quels drones sont réglés ou d'origine, il n'y a donc pas de corpus de référence
(décision du 2026-10-05, `docs/assistant.md`, « Calibration ») ; ne pas lui redemander ces étiquettes ni la
taille réelle. `corpus labels` (tableau `corpus.csv`) ne réécrit jamais les colonnes `etat`, `taille_reelle` et `remarque`. Il
signale les noms de drone trouvés dans plusieurs dossiers : demander à Cédric si ce sont des machines
différentes : oui → `SHARED_NAMES`, non → `SAME_DRONE_NAMES`. Les outils lisent les logs par
`corpus::log_files` d'otb-dev (sans doublons de contenu ni `__MACOSX`).

Pour une **nouvelle mesure** : l'ajouter dans la boucle de l'outil (`calibrate.rs`) (même calcul que la règle ; si la règle
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
- Vérifier le **taux de déclenchement** d'un seuil sur le corpus (part des sessions signalées,
  `otb-dev corpus rates`, avant / après le changement) : très
  au-dessus de 10 % → seuil trop sévère ; proche de 0 % → règle inutile.
- **Séparer selon le réglage qui fausse la mesure** : le feedforward gonfle le pic de réponse (p90 1,19
  sans, 1,59 avec, une voix par drone, D-025) → deux jeux de paliers (`rules.STEP_PEAK_HIGH`, `STEP_PEAK_HIGH_FF`).
  Vérifier ce cas pour toute mesure liée à un réglage (F, filtres, RPM…), et juger le réglage **en service**
  (le feedforward est coupé en mode Angle), pas celui de l'en-tête (L-005).
- Arrondis : bruit au 0,1 dB ; latence p10 vers le bas, p90 vers le haut (ms entière) ; retards de filtres
  au dixième supérieur (+0,1 ms).
- **Ne changer aucun seuil sans montrer les chiffres à Cédric et avoir son accord.**

## 3. Sourcer une valeur qui ne vient pas du corpus

- **Firmware / configurateur** : clones dans `/home/cedric/Documents/Github/betaflight` et
  `betaflight-configurator`. Citer fichier et commit dans `docs/assistant.md` (« Sources vérifiées »).
- **Théorie** : citer l'ouvrage, la section et la page, vérifiées dans le texte (pas de mémoire).
  Déjà vérifiés : Skogestad et Postlethwaite 2005 § 2.4.3 (Ms ≤ 2) ; Åström et Murray 2008 § 9.3
  (marge de phase 30–60°).
- **Communauté** : seulement avec une source précise (URL, auteur, date, citation exacte lue sur la page),
  ajoutée à `docs/spec-methodes.md`, « Sources des repères » ([S1]…). Sans source, c'est une
  **Estimation**, même si beaucoup de pilotes la répètent (cas des anciens paliers 1,02–1,08 / 1,20 du pic
  de réponse, remplacés le 2026-10-05 par les percentiles du corpus). Préférer la source primaire (auteur de l'outil, développeurs
  Betaflight, code) au blog qui la reprend ; une page disparue se lit sur la Wayback Machine.
- **Repère lu dans PIDtoolbox** : ses spectres sont 6,5 dB sous les nôtres (`spectral.PTB_PSD_OFFSET_DB`,
  mesure `rules.band_peak_ptb_db`) et son retard D 0,3 à 0,9 ms sous le nôtre ; recouper avec les captures
  `PTB_FIGS_*` avant d'adopter un autre repère chiffré (`docs/validation.md`).
- **Fonctionnalité récente d'un autre outil** : vérifier son historique (PR, version publiée ou non,
  essais en vol, tickets ouverts) avant de la citer. L'onglet Autotune du configurateur (2026) a été
  examiné et **n'est pas une source** (voir `docs/assistant.md`).

## 4. Pièges

- Pic de sensibilité et marge de phase : boucle ouverte **mesurée** (`rust/otb-analysis/src/open_loop.rs`) quand les termes
  P, I, D, F sont loggés, sinon approchée par L = T/(1 − T) (faux pour Betaflight, `docs/assistant.md`,
  « Boucle ouverte mesurée »). Les percentiles calculés avec l'ancienne méthode ne sont pas comparables
  (marge de phase environ 10° plus haute, croisement environ 2 fois trop bas). Le pic de sensibilité est
  souvent un minorant (`smaxedge` dans `stats.json`). Les whoops vont plus loin en bande (46 Hz contre
  22–24 Hz) : leur pic est plus souvent le vrai, et leur marge faible est réelle (diagnostic du 2026-10-06,
  `docs/validation.md`) ; ne pas « corriger » leur taux par un percentile propre.
- Taille : la fréquence moteur seule confond les 2–3,5" et les 5" entre 200 et 290 Hz ; le nombre de
  cellules les départage (2–4S), sauf en 6S (recouvrement avec les 5" en vol freestyle). Évaluer une
  limite par drone **et** par session (la fréquence d'une session freestyle monte de 10 à 30 %), sur la
  liste de vérification de `docs/assistant.md` (« Taille du drone »).
- Le corpus est surtout fait de demandes d'aide (« 1. PTB », « 3. BBLR FB ») : un percentile du corpus
  est probablement trop indulgent. Le dire dans les textes et la doc.

## 5. Terminer

1. Valeurs dans `rust/otb-advisor/src/profiles.rs` (arguments nommés, commentaire de calibration daté).
2. `docs/assistant.md` : sections « Taille du drone », « Calibration » (tableaux), « Règles » et
   « Origine des seuils » ; `CHANGELOG.md` (*Non publié*). Puis la compétence `docs-sync`.
3. Tests : `rust/otb-advisor/tests/profiles.rs` et `rules.rs` comparent les seuils et les décisions à des cas figés
   (`rust/otb-advisor/tests/data/*.json`, écrits par l'ancienne version Python). Un changement **voulu** de valeur les fait échouer :
   mettre à jour le cas concerné à la main, dans le même commit, en écrivant pourquoi (valeur ancienne, valeur nouvelle, mesure du
   corpus). Ne jamais affaiblir un test pour le faire passer. `cargo test -p otb-dev --test docs` vérifie que `docs/assistant.md`
   porte les mêmes valeurs que `profiles.rs`.
4. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` et `cargo test` propres.
