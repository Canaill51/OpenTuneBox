# 1. Prérequis

À vérifier une seule fois, avant le premier vol de test.

## Logiciels

- **Betaflight 4.4 ou plus récent** sur le contrôleur de vol (4.5, 2025.12, 2026.6…). Les versions plus
  anciennes ne sont pas lues.
- **Configurateur Betaflight** (application Betaflight App ou Betaflight Configurator) sur l'ordinateur.
- Dans le configurateur, fenêtre **Options** : cochez **Mode Expert**. Sans lui, les curseurs principaux
  sont limités à 0,7–1,4 et les curseurs Multiplicateur Maître, Dérive - Oscillations, Atténuation du Pitch
  et Suivi du Pitch restent bloqués.

## Sauvegarde

Avant tout changement, gardez une copie du réglage actuel : onglet **Ligne de commande (CLI)**, tapez
`diff all`, puis **Enregistrer sous**. C'est votre point de retour si un réglage tourne mal
(voir [Appliquer une modification](otb:step/appliquer)).

## Le drone

- **Mécanique saine** : hélices neuves et droites, vis du châssis et des moteurs serrées, moteurs qui
  tournent librement, câbles et antennes qui ne battent pas. Aucun filtre ne rattrape une hélice tordue.
- **Mémoire blackbox** : une carte SD ou une mémoire flash sur le contrôleur. Onglet **Blackbox** :
  « Stockage de la blackbox » sur la carte SD ou la mémoire embarquée. Une flash de 16 Mo ne garde que
  quelques minutes à 2 kHz : videz-la entre les séances.
- **DShot bi-directionnel** : onglet **Moteurs**, case « DShot bi-directionnel » (ESC BLHeli_32, Bluejay ou
  AM32). Il active le filtre RPM et donne la vitesse réelle des moteurs dans le log. Réglez aussi **Pôles
  moteur** (nombre d'aimants de la cloche, souvent 14, 12 sur les petits moteurs).
- **Curseurs de réglage PID actifs** : onglet **Réglages de PID**, « Mode » : RPY. OpenTuneBox conseille des
  positions de curseurs ; s'ils sont désactivés, les PID se règlent à la main. Attention : activer les
  curseurs recalcule les PID à partir d'eux. Notez vos PID avant si vous y tenez.
- **Mêmes batteries**, chargées pareil, pour tous les vols d'une même phase.

Étape suivante : [préparer l'enregistrement](otb:step/enregistrement).
