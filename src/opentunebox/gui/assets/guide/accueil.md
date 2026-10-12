# Régler son drone avec OpenTuneBox

Ce guide vous accompagne du drone d'origine au réglage fini, case par case. Il suppose que vous savez vous
servir du configurateur Betaflight ; il vous apprend à enregistrer les bons vols, à lire les logs et à
déplacer les bons curseurs.

## La boucle de réglage

1. **Enregistrer** un vol court, dans des conditions précises ;
2. **Ouvrir** le log ici : le guide vérifie qu'il convient et coche ce qu'il prouve ;
3. **Changer une seule chose** dans le configurateur (un curseur, le plus souvent) ;
4. **Revoler** de la même façon, et comparer les deux logs.

## Les phases

| Phase | Vols à prévoir | Ce qu'on règle |
|---|---|---|
| [1. Préparer le drone](otb:phase/prep) | aucun | configurateur, enregistrement, profil de rates « LOG » |
| [2. Filtres](otb:phase/filters) | 2 à 6 vols | filtre RPM, notch dynamique, passe-bas du gyro et du D |
| [3. PID par séries](otb:phase/pid) | 3 à 6 vols par série, 2 à 4 séries | Damping, Master, équilibre roll / pitch |
| [4. Finitions](otb:phase/finish) | 1 à 3 vols | feedforward, terme I, ressenti |

La frise en haut de la page montre où vous en êtes. Une case se coche à la main, ou toute seule quand un log
du drone le prouve ; « ▲ À revoir » signale ce qu'un log contredit.

## Avant de commencer

- Le drone en état de vol, son câble USB, le configurateur Betaflight (Betaflight 4.4 ou plus récent).
- 4 à 6 batteries chargées pareil, un endroit dégagé pour du vol stationnaire et des wobbles en mode Angle.
- Comptez une séance pour la préparation et les filtres, une ou deux pour les séries PID.

## La fiche du drone

Bouton **Nouveau drone** en haut : nom (le même que dans Betaflight, onglet Configuration : les logs sont
reconnus grâce à lui), version de Betaflight, taille, boucle PID, ESC. Si un log est ouvert, la fiche se
remplit d'après lui. Les valeurs du guide (fréquence d'enregistrement, listes des séries, plages des
finitions) s'adaptent à la fiche, et chaque drone garde son avancement.

Commencez par [Préparer le drone](otb:phase/prep).
