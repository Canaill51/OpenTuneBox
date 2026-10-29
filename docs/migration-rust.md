# Migration vers Rust

Plan et suivi de la réécriture d'OpenTuneBox en Rust, **sans régression** (décision D-031, travail reporté W-015).
Ce document ne vit que sur `feat/rust-migration` : `main` reste en Python jusqu'à la validation finale.

## Branches

```
main                          Python, publiable à tout moment
 └─ feat/rust-migration       branche d'intégration (longue durée)
      ├─ chore/rust-safety-net      phase 0 : références, outillage, décisions
      ├─ feat/rust-decoder          phase 1
      ├─ feat/rust-dsp              phase 2a (2b : nouvelle branche)
      └─ …                          une branche par module ou par étape
```

- Une branche par étape, créée depuis `feat/rust-migration` ; la PR va vers `feat/rust-migration` (jamais
  `main`) et son titre est le nom de la branche.
- Les correctifs de `main` sont rapatriés dans `feat/rust-migration` par `git merge main`, régulièrement et
  toujours avant une mesure de parité.
- Fin du projet : une seule PR `feat/rust-migration` → `main`, après validation complète (tests, parité sur
  tout le corpus, installateurs Linux, Windows et macOS) et accord de Cédric.

## Principe

Chaque module Python est réécrit en Rust, branché par PyO3 (module `opentunebox._native`) puis prouvé
équivalent avant de supprimer l'ancien. Pendant la transition, `OTB_BACKEND=python|rust` choisit
l'implémentation et les tests de parité comparent les deux. Espace de travail Cargo prévu :

```
rust/  otb-decoder  otb-dsp  otb-log  otb-analysis  otb-advisor  otb-py (pont PyO3)  otb-gui (phase 6)
```

## Preuves d'équivalence

| Couche | Exigence | Outil |
|---|---|---|
| Décodeur | identique au bit près sur tout le corpus | `scripts/golden.py` (empreintes), `scripts/validate_corpus.py` |
| Calculs et outils | écart mesuré, ≈ 1e-5 relatif (FFT différente de scipy) | `scripts/golden.py check --rtol X`, comparaison directe à scipy |
| Assistant | mêmes constats (clés, textes, chiffres) | `scripts/advisor_rates.py --compare` |
| Interface | mêmes thèmes, mêmes graphiques, Windows compris | captures hors écran, tests `test_gui_*` |

### Références figées (`scripts/golden.py`)

Enregistrées **avant** d'écrire du Rust, avec le code Python de `main`, dans `$OTB_LOGS/golden/` (hors du dépôt) :

```sh
.venv/bin/python scripts/golden.py record "$OTB_LOGS/golden"            # ≈ 15 min, plusieurs centaines de Mo
.venv/bin/python scripts/golden.py check  "$OTB_LOGS/golden"            # code 1 si une différence subsiste
.venv/bin/python scripts/golden.py check  "$OTB_LOGS/golden" --rtol 1e-5   # tolérance pour une réécriture de FFT
```

Trois couches : empreintes SHA-256 de chaque tableau décodé (tout le corpus), sortie de `compute` de chaque
outil du registre sur 20 logs répartis dans le corpus (tous les nombres), constats de l'assistant sur tout le
corpus. `record` décode deux fois et refait seul tout fichier discordant (inversions de bits du poste,
compétence `calculs-corpus`) ; `check` refait seul tout fichier qui diffère avant de le signaler.
Le filet a été éprouvé : une valeur altérée à la main est détectée (décodeur et outils).

## Phases

| # | Contenu | Risque | Preuve |
|---|---|---|---|
| 0 | Filet : références, décisions, CI de la branche | faible | `golden.py` éprouvé |
| 1 | Décodeur (`decoder/`) ; règle aussi W-014 et W-016 | faible | empreintes identiques, `validate_corpus.py` |
| 2a | scipy → numpy (`_numeric.py`), noyaux des filtres Betaflight en Rust (numba et scipy quittent l'application) | moyen | parité mesurée : outils 2,4e-10, filtres identiques |
| 2b | Pilote : noyau de `step_response` en Rust (la FFT seule ne rapporte que 5 %, D-033) | moyen | parité à tolérance mesurée ; gain ≥ 1,5× |
| 3 | Analyses (`analysis/`, `log/`) module par module | moyen | `golden.py check --rtol` |
| 4 | Assistant (`advisor/`) sans changer un seuil | faible | `advisor_rates.py --compare` |
| 5 | Ligne de commande en binaire Rust | faible | rapports identiques caractère par caractère |
| 6 | Interface (bibliothèque choisie alors : egui, Slint, cxx-qt) | très élevé | captures, thèmes, Windows |

Règles de chaque étape : tests Python et Rust verts, `ruff` et `clippy` propres, un test de parité qui
échoue si le Rust diffère, aucun test affaibli, code Python supprimé seulement après recherche de ses usages
et parité prouvée, documentation et `CHANGELOG.md` à jour, pièges notés dans `lecons.md`.

Après la phase 2, **mesurer le poids réel des installateurs** (objectif n° 1) avant de poursuivre.

## Avancement

| Phase | État |
|---|---|
| 0 | Fait le 2026-10-29 (branche `chore/rust-safety-net`) : références enregistrées (499 fichiers, 680 sessions, 630 sessions de l'assistant, 20 logs d'outils, 705 Mo) ; `check` relancé sur tout le corpus : identique |
| 1 | Fait le 2026-10-29 (branche `feat/rust-decoder`) : décodeur Rust par défaut, identique au bit près sur tout le corpus, 6 fois plus rapide ; relu par un agent indépendant (L-032). Reste : CI Windows à voir tourner après envoi sur GitHub |
| 2a | Fait le 2026-10-29 (branche `feat/rust-dsp`, D-032) : scipy remplacé par `analysis/_numeric.py`, noyaux des filtres Betaflight en Rust (`rust/otb-dsp`), scipy et numba hors de l'application (extra `dev`). Parité : décodeur 0 différence (499 fichiers), assistant 0 différence (630 sessions), outils : écart relatif maximal 2,4e-10 sur 20 logs. Relu par un agent indépendant (L-036, L-037) |
| Poids | **Mesuré le 2026-10-29** (Linux, `feat/rust-dsp`, environnement neuf `pip install . pyinstaller`, PySide6 6.12, numpy 2.5.4) : dossier PyInstaller **206 Mo, contre 463 Mo** avant la migration (−55 %) ; PySide6 102 Mo, numpy 14 Mo + `numpy.libs` 27 Mo (OpenBLAS de numpy), `opentunebox` 0,7 Mo ; ni scipy, ni numba, ni llvmlite ; l'exécutable reste ouvert 25 s. Reste à mesurer : installateurs compressés (AppImage, Windows, macOS), à lire sur `construire.yml` quand le quota d'artefacts GitHub le permet |
| 2b | Pilote `step_response` en Rust (branche `feat/rust-fft`, D-033) : noyau fait, 1,70× à un fil et 4,9× avec tous les cœurs ; parité sur le corpus : assistant 0 différence (630 sessions), outils 2,4e-10, relu par un agent indépendant (L-038). `_seg_spectra`, `loop_spectra`, `xcorr` : décidés après ce pilote (phase 3) |
| 3 à 6 | à faire |
