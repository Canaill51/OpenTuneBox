# 5. Appliquer une modification

Chaque conseil d'OpenTuneBox se traduit par un changement dans le configurateur Betaflight. Règle d'or :
**une seule modification entre deux vols**, sinon on ne sait plus laquelle a fait quoi.

## D'abord les curseurs et les onglets du configurateur

Presque tout se règle sans taper de commande :

| Réglage | Où le changer |
|---|---|
| PID (Atténuation, Suivi, Réponse des sticks, Multiplicateur Maître…) | onglet **Réglages de PID**, curseurs de réglage PID |
| Force des passe-bas du gyro et du terme D | onglet **Réglages de PID**, partie filtres : « Multiplicateur du Filtre Gyro », « Multiplicateur de filtre D Term » |
| Filtre RPM (harmoniques, Q, fréquence min, poids) | onglet **Réglages de PID**, partie filtres : « Filtre Gyro RPM » |
| Notch dynamique (nombre, Q, fréquences min et max) | onglet **Réglages de PID**, partie filtres : « Filtre coupe bande dynamique » |
| Passe-bas un par un (type, fréquence) | onglet **Réglages de PID**, partie filtres, cases de chaque filtre |
| Fréquence d'enregistrement, mode de débogage | onglet **Blackbox** |
| DShot bi-directionnel, pôles moteur | onglet **Moteurs** |

Après chaque changement : bouton **Sauvegarder** en bas de l'onglet.

Pour voir l'effet d'un curseur avant de voler, ouvrez l'outil [Curseurs PID](otb:tool/sliders) : il donne
les PID et les filtres obtenus pour chaque position, et les commandes CLI correspondantes.

## Le CLI en repli

Pour ce que les onglets ne proposent pas (réglages du chirp, certains réglages fins), ou si vous préférez
copier-coller : onglet **Ligne de commande (CLI)**, collez les commandes (une par ligne), puis tapez `save`
et Entrée. Sans `save`, rien n'est gardé. L'étape [Vérifier mes logs](otb:step/verifier) a un bouton
« Copier les commandes CLI » qui ajoute `save` à la fin.

## Revenir en arrière

- Un changement dégrade le vol (bruit, moteurs chauds, oscillations, ressenti) : remettez la valeur
  précédente. L'outil Configuration (bouton en bas du panneau de droite du
  [visualiseur](otb:tool/viewer)) montre, logs côte à côte, la valeur de chaque réglage avant et après.
- Tout recommencer : collez dans le CLI le `diff all` sauvegardé au départ (voir
  [Prérequis](otb:step/prerequis)), puis `save`.

Étape suivante : [régler les filtres](otb:step/filtres).
