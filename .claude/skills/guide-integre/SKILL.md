---
name: guide-integre
description: Modifier ou relire le guide intégré d'OpenTuneBox (page Guide de l'interface, textes gui/assets/guide/*.md, étape « Vérifier mes logs » calculée par advisor/log_check.py). À charger avant de changer la procédure de réglage montrée à l'utilisateur, un texte du guide, un contrôle de log, ou quand une règle, un seuil ou un outil cité par le guide change.
---

# Guide intégré

## Où vit quoi

- Textes : dans `src/opentunebox/gui/assets/guide/`, un fichier `NN-nom.md` par étape, en Markdown simple (titres,
  listes, tableaux, `code`) affiché par `QTextBrowser.setMarkdown`. Pas d'image, pas de HTML.
- Liste des étapes : `gui/guide.py:STEPS` (clé, titre, fichier). Ajouter une étape = une ligne + un fichier.
- Liens internes : `[texte](otb:step/<clé>)` vers une étape, `[texte](otb:tool/<clé>)` vers un outil de
  `gui/tools/__init__.py:REGISTRY` (`viewer` pour le visualiseur). `tests/test_docs.py:test_guide_links`
  vérifie fichiers et liens.
- Étape calculée « Vérifier mes logs » : `advisor/log_check.py` (`check_log`, `check_logs`), rendue en HTML
  par `GuidePage._check_html`. Calculs rapides seulement (< 0,5 s par log) : pas de spectre ni de réponse
  indicielle complète (`analysis/step.py:input_windows` compte les fenêtres).
- `docs/guide-utilisateur.md` §3 et §6 ne font que résumer et renvoyer aux fichiers du guide : la procédure
  n'existe qu'à un endroit.

## Règles d'écriture

- Pour l'utilisateur qui découvre : phrases courtes, l'effet concret avant le terme technique, une action par
  ligne. Noms des onglets, curseurs et champs **tels qu'affichés par le configurateur en français**, avec le
  nom anglais entre parenthèses pour les curseurs PID.
- **Configurateur d'abord, CLI en repli** (choix de Cédric) : chaque correction indique l'onglet, le curseur
  ou le champ ; la commande CLI vient après, comme alternative, ou seule quand le configurateur ne propose
  rien (réglages du chirp…). Correspondances vérifiées : [reference-configurateur.md](reference-configurateur.md).
- Aucun seuil propre au guide : les valeurs citées (fréquence d'enregistrement, 0,2–0,3 pour le curseur I,
  pas de 0,2, −30 dB / −10 dB…) viennent de l'assistant (`docs/assistant.md`). Si une règle ou un seuil
  change, chercher la valeur dans `gui/assets/guide/` et `advisor/log_check.py`.
- Un texte d'un `Finding` déjà écrit dans `advisor/rules.py` ou `step_tuning.py` se réutilise plutôt que de
  le réécrire autrement.

## Vérifier

```sh
.venv/bin/pytest -q tests/test_docs.py tests/test_gui_guide.py tests/test_log_check.py
```

Puis captures hors écran (compétence `opentunebox-dev`) : démarrage sans log (page Guide), l'étape modifiée,
« Vérifier mes logs » avec quelques logs du corpus, **dans les deux thèmes** ; regarder chaque image.
Cédric relit les textes avant fusion : c'est lui qui connaît le vol.
