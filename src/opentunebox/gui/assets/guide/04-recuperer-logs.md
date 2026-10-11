# 3. Récupérer et ouvrir les logs

## Copier les fichiers sur l'ordinateur

1. Branchez le drone (sans batterie) et ouvrez le configurateur, onglet **Blackbox**.
2. Cliquez **Activer le Mode de périphérique de stockage de masse**. Le contrôleur redémarre et apparaît
   comme une clé USB.
3. Copiez les fichiers `.bbl` (ou `.bfl`) dans un dossier de votre ordinateur, **un dossier par drone**.
4. Éjectez le lecteur, débranchez puis rebranchez le drone.
5. Mémoire flash : onglet **Blackbox**, **Effacer la mémoire interne** une fois les fichiers copiés. Carte
   SD : supprimez les anciens fichiers.

Le mode stockage de masse est bien plus rapide que « Enregistrer sous… » de l'onglet Blackbox, qui passe
par la liaison série et peut abîmer les fichiers.

## Bien nommer les fichiers

Renommez chaque fichier d'après ce qu'il contient : `stationnaire.bbl`, `mouvements.bbl`,
`damping-0.8.bbl`, `damping-1.0.bbl`, `master-1.2.bbl`… Dans six mois, vous saurez encore lequel est
lequel.

## Ouvrir dans OpenTuneBox

- Bouton **Ouvrir** de la barre d'outils (Ctrl+O), ou glissez les fichiers dans la fenêtre.
- Un fichier peut contenir plusieurs vols (un par armement) : chacun devient un log, avec sa couleur.
  10 logs au plus en même temps.
- Le [visualiseur](otb:tool/viewer) montre les courbes du vol. La zone claire est la **période d'analyse** :
  du décollage à l'atterrissage, en s'arrêtant avant un crash. Déplacez ses bords à la souris pour écarter
  un moment à part (atterrissage raté, passage au sol).

Étape suivante : [vérifier mes logs](otb:step/verifier).
