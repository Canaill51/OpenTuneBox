---
name: guide-integre
description: Modifier ou relire le guide intégré d'OpenTuneBox (page Guide : fiches drone, parcours à cocher en 4 phases, vérification automatique des logs). À charger avant de changer une case, un texte, une valeur conseillée, un contrôle de log (advisor/log_check.py, drone_card.py, tuning_path.py, gui/guide.py), ou quand une règle, un seuil ou un outil cité par le guide change.
---

# Guide intégré

## Où vit quoi

- **Parcours** : `advisor/tuning_path.py:phases` (4 phases, une `Step` par case : titre et aide en Markdown,
  éventuellement fonctions de la fiche ; `checks` / `usage` / `flight` / `targets` pour l'état automatique,
  `tool` + `part` pour le bouton « Ouvrir … »). Sans Qt, testé par `tests/test_tuning_path.py`.
- **Fiche drone** : `advisor/drone_card.py` (version, taille, boucle PID, ESC ; fréquence d'enregistrement,
  mode de débogage, listes des séries, plages de finitions, filtres par défaut du firmware). Fiches en JSON
  dans le dossier de données de l'utilisateur ; un log est relié à la fiche par `craft_name`.
- **Contrôles des logs** : `advisor/log_check.py:check_log` (un point par clé ; parties du vol par
  `analysis/period.py:flight_parts` ; `exact=True` et `filter_targets` sont lents : arrière-plan).
- **Interface** : `gui/guide.py` (fiche, frise, phases, cases, logs du drone). Accueil :
  `gui/assets/guide/accueil.md`, seul fichier Markdown ; liens `otb:phase/<clé>` et `otb:tool/<clé>`.
- `tests/test_docs.py` vérifie les liens de l'accueil, les outils, parties et points de contrôle cités par
  le parcours.

## Règles

- Lecteur : un pilote qui connaît Betaflight et son configurateur. Une action par case, titre court, aide
  courte ; l'effet concret avant le terme technique. Libellés **tels qu'affichés par le configurateur en
  français** (anglais entre parenthèses pour les curseurs PID, `log_check.CONFIGURATOR_SLIDERS`).
- **Configurateur d'abord, CLI en repli** ; un changement = une position de curseur quand c'est possible.
  Correspondances vérifiées : [reference-configurateur.md](reference-configurateur.md).
- Protocole et valeurs décidés par Cédric le 2026-10-12 : ne pas les changer sans lui demander. Chaque valeur
  a son origine dans `docs/assistant.md` (« Origine des seuils », lignes « Guide : … » et « Série
  Feedforward ») et ses sources dans `docs/spec-methodes.md` ([S24], [S25], [S33]–[S36]).
- Nouveau point de contrôle : une clé stable dans `check_log`, citée par la case dans `checks` ; un seuil
  nouveau passe par la compétence `assistant-seuils` (chiffres du corpus montrés à Cédric).

## Vérifier

```sh
.venv/bin/pytest -q tests/test_docs.py tests/test_tuning_path.py tests/test_log_check.py tests/test_gui_guide.py
```

Puis captures hors écran (compétence `opentunebox-dev`, `QStandardPaths.setTestModeEnabled(True)` pour ne pas
toucher aux fiches de l'utilisateur) : accueil sans fiche, fiche, chaque phase avec un log du corpus, aides
dépliées, **dans les deux thèmes** ; regarder chaque image. Cédric relit les textes avant fusion.
