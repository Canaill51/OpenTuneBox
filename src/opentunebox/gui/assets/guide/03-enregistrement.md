# 2. Préparer l'enregistrement

Un log mal enregistré donne de fausses conclusions. Ces réglages ne changent pas le comportement du drone :
ils décident seulement de ce qui est écrit dans le log.

## Dans le configurateur : onglet Blackbox

1. **Fréquence d'enregistrement de la Blackbox** : choisissez la valeur du tableau ci-dessous.
2. **Mode débug de la Blackbox** :
   - Betaflight 4.5 et plus : **FFT_FREQ** (le log contient alors les fréquences suivies par le notch
     dynamique) ;
   - Betaflight 4.4 : **GYRO_SCALED** (seul moyen d'avoir le gyro non filtré dans cette version).
3. Betaflight 4.5 et plus, **Champs de débogage inclus** : laissez **Gyro (Unfiltered)** coché, ainsi que
   Gyro, PID, Setpoint, RC Commands, Motors et RPM.
4. **Enregistrer et redémarrer**.

## Quelle fréquence d'enregistrement ?

Le blackbox garde un échantillon sur N sans filtrer : tout bruit plus rapide que la moitié de la fréquence
d'enregistrement se « replie » et crée de fausses raies dans le spectre.

| Drone | Gyro | Fréquence à choisir |
|---|---|---|
| 5 pouces, 7 pouces, cinéwhoop… | 8 kHz | **2 kHz** au moins |
| Contrôleur à gyro BMI270 | 3,2 kHz | **1,6 kHz** au moins |
| Whoop, tiny, petits moteurs très rapides | 8 kHz | **4 kHz** (plus les moteurs tournent vite, plus il en faut) |

L'étape [Vérifier mes logs](otb:step/verifier) calcule la valeur exacte pour votre drone dès que vous ouvrez
un de ses logs.

## En CLI, si vous préférez

Onglet **Ligne de commande (CLI)**, puis par exemple :

```
set blackbox_sample_rate = 1/2
set debug_mode = FFT_FREQ
save
```

`1/2` = la moitié de la fréquence de la boucle PID (2 kHz avec une boucle à 4 kHz), `1/4` = le quart, etc.
En 4.4, remplacez FFT_FREQ par GYRO_SCALED. Avec la 4.5 et plus, si le gyro non filtré manque :
`set blackbox_disable_gyrounfilt = OFF`.

## Avant chaque vol

- Le blackbox enregistre dès l'armement (réglage par défaut) : un log = un armement.
- Mémoire vide ou presque, pour ne pas couper le vol en route.
- Batterie chargée, hélices en bon état, même charge que les vols à comparer.

## Option avancée : le chirp (Betaflight 2025.12 et plus)

Le mode CHIRP fait osciller le drone tout seul sur une plage de fréquences : les marges de stabilité de
l'outil [Réponse indicielle](otb:tool/step) deviennent plus sûres. Il demande `debug_mode = CHIRP`, un
interrupteur affecté au mode CHIRP (onglet **Modes**) et, en CLI, `set chirp_frequency_end_deci_hz = 600`
(60 Hz) au moins. Facultatif : la méthode de ce guide s'en passe.

Étape suivante : [récupérer et ouvrir les logs](otb:step/recuperer).
