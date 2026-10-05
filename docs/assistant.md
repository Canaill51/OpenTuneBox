# Assistant de réglage — fonctionnement et calibration

L'assistant (`src/opentunebox/advisor/`) transforme les mesures d'un log en constats argumentés.
Chaque constat cite la mesure et le seuil qui le déclenchent ; les actions passent d'abord par les
curseurs du réglage simplifié de Betaflight, par petits pas (0,1), une chose à la fois.

## Taille du drone

Détectée par la fréquence de rotation des moteurs en vol (médiane de l'eRPM mesuré, drone armé, gaz > 5 %,
moteurs > 10 Hz) : ≥ 420 Hz whoop · 290–420 Hz 2–3,5" · 175–290 Hz 4–5" · 115–175 Hz 6–7" · < 115 Hz 8"+.
Sans eRPM, par le nom du drone : « whoop », « 65 », « 75 », « Meteor » → whoop ; sinon une taille en pouces
(« 5" », « 7 pouces ») : < 3,8 → 2–3,5", < 5,8 → 5", < 7,8 → 7", au-delà 8"+. À défaut : profil 5".
Le profil peut être forcé dans la fenêtre de l'assistant ou avec `opentunebox report --profil`.

## Calibration (2026-10-01)

`scripts/calibrate_advisor.py 600 stats.json` sur le corpus `~/Documents/Drones/Logs` : 708 sessions
exploitables (366 × 5", 180 × 7", 78 whoops, 64 × 2–3,5", 20 × 8"+), période automatique, roll + pitch.
Les seuils « élevé » sont le **90e percentile de chaque taille** : un drone est signalé quand il fait
partie des 10 % les plus bruités / lents de sa catégorie.

| Mesure (p10 / p50 / p90) | whoop | 2–3,5" | 5" | 7" |
|---|---|---|---|---|
| Gyro filtré 80–500 Hz (dB) | −34,1 / −31,4 / −28,4 | −37,3 / −33,2 / −27,9 | −37,8 / −33,3 / −28,5 | −39,4 / −36,9 / −30,2 |
| Gyro non filtré 80–500 Hz (dB) | −24,7 / −22,6 / −16,4 | −25,5 / −20,8 / −11,4 | −28,4 / −19,9 / −13,3 | −29,4 / −24,1 / −17,3 |
| Terme D 80–500 Hz (dB) | −23,9 / −16,7 / −11,7 | −26,8 / −21,1 / −13,0 | −27,7 / −20,4 / −13,6 | −33,4 / −24,6 / −16,2 |
| Gyro filtré 20–80 Hz (dB) | −24,1 / −20,5 / −15,6 | −27,5 / −24,1 / −12,3 | −30,0 / −24,9 / −15,3 | −32,5 / −28,5 / −21,2 |
| 1re harmonique moteur restante (dB) | −41,3 / −34,4 / −29,1 | −42,1 / −37,8 / −32,3 | −40,5 / −35,3 / −28,8 | −41,2 / −38,0 / −31,7 |
| Pic de réponse indicielle | 1,0 / 1,1 / 1,3 | 1,0 / 1,1 / 1,3 | 1,0 / 1,1 / 1,3 | 1,0 / 1,1 / 1,2 |
| Latence 50 % (ms) | 11,6 / 13,9 / 17,0 | 10,2 / 16,6 / 19,7 | 10,5 / 15,2 / 20,9 | 14,4 / 19,0 / 23,5 |
| Pic de sensibilité (dB) | 2,9 / 4,0 / 6,9 | 2,0 / 3,5 / 7,5 | 0,0 / 3,1 / 5,4 | 1,7 / 3,0 / 5,0 |
| Marge de phase (°) | 50 / 64 / 74 | 37 / 64 / 72 | 50 / 68 / 77 | 51 / 65 / 74 |
| Retard filtres gyro (ms) | 1,0 / 1,2 / 1,4 | 1,0 / 1,3 / 1,7 | 1,0 / 1,3 / 2,0 | 1,2 / 1,8 / 2,5 |
| Retard filtres D (ms) | 1,7 / 2,0 / 2,4 | 1,8 / 2,3 / 2,5 | 1,9 / 2,3 / 2,7 | 2,1 / 2,5 / 2,8 |

Unités : densité spectrale de puissance en dB de (°/s)²/Hz pour le gyro, d'unités PID²/Hz pour le D.

Profil 8"+ : 20 sessions seulement, trop peu pour un percentile fiable. Ses seuils sont provisoires
(gyro −31 dB, D −17 dB, 20–80 Hz −22 dB, harmonique −32 dB, latence 18–30 ms, retards 3,2 / 3,5 ms).

Dans `advisor/profiles.py`, les repères de latence sont arrondis à la milliseconde (p10 vers le bas, p90
vers le haut), ceux de retard des filtres vers le haut (+0,1 à +0,2 ms) ; les seuils de bruit sont les p90
du tableau.

## Règles

Niveaux : **attention** (à corriger), **conseil** (amélioration), **info**. Les règles de réponse et de bruit
portent sur le roll et le pitch (le yaw n'est pas jugé).

- **Enregistrement** : fréquence du log < 900 Hz (attention) ; 3e harmonique moteur au-delà de la moitié de
  la fréquence du log ; gyro non filtré absent ; pas de télémétrie RPM ; plus de 20 trames corrompues (info).
- **Vol** : crash détecté, la période d'analyse s'arrête avant (info).
- **Réponse** (au moins 20 fenêtres par axe, sinon « Pas assez de mouvements de manche ») :
  - dépassement : pic > 1,20 (attention), > 1,10 (conseil) ; attribué au feedforward si F > 0 et si la
    latence est plus courte que le repère bas de la taille ;
  - réponse molle : pic < 0,98 et latence au-delà du repère haut de la taille ;
  - roll et pitch différents : écart de pic > 0,08 (curseur « D pitch ») ;
  - stabilité : pic de sensibilité > 6 dB (attention) ; sinon marge de phase < 40° (conseil) ;
  - oscillations basses fréquences : gyro filtré 20–80 Hz > p90 de la taille.
- **Bruit** (80 Hz à min(500 Hz ; 0,95 × Nyquist), valeur maximale du roll et du pitch) : gyro filtré et
  terme D > p90 de la taille ; 1re harmonique moteur restante > p90 (seulement avec eRPM, gyro non filtré
  et filtre RPM actif).
- **Filtrage trop prudent** : retard gyro > repère de la taille alors que gyro et D sont peu bruités ;
  retard D > repère alors que le D est peu bruité.
- **Résonance** : raie ≥ 6 dB au-dessus de la moyenne glissante voisine (60 Hz à min(900 Hz ; 0,9 × Nyquist)),
  présente dans la médiane de toutes les tranches de gaz (au moins 8 tranches), à plus de ±8 % des
  harmoniques 1 à 3 du moteur.
- **Mécanique** : déséquilibre des moteurs > 12 % (lu avant/arrière, gauche/droite ou diagonale) ; terme I
  moyen décentré (moyenne > 40 en valeur absolue, roll ou pitch) ; saturation moteur (≥ 98 %) > 5 % du temps.
- **Réglage** : réglage simplifié désactivé, les conseils « curseur » sont indicatifs (info).

## Origine des seuils

Tous les seuils n'ont pas la même solidité. Chacun est classé selon son origine :

- **Corpus** : mesuré sur les vrais logs (percentile par taille), recalculable avec
  `scripts/calibrate_advisor.py` ;
- **Communauté** : repère de pilotes ou d'outils publics (wiki PIDtoolbox v0.55, Blackbox Explorer…,
  voir `docs/spec-methodes.md`, « Repères de réglage »), sans référence précise notée valeur par valeur ;
- **Théorie** : critère classique d'automatique ou de traitement du signal ;
- **Firmware** : valeur par défaut ou fonctionnement de Betaflight (`docs/spec-betaflight.md`) ;
- **Estimation** : choisi en écrivant les règles, ni calibré ni sourcé. À vérifier en priorité.

| Règle | Seuil | Origine | Remarque |
|---|---|---|---|
| Taille par l'eRPM | 420 / 290 / 175 / 115 Hz | Estimation | Inspiré de la table des fréquences moteur de `spec-methodes.md`, mais pas aligné : la table place un whoop à 550–800 Hz |
| Taille par le nom | « whoop », « 65 », « 75 », « Meteor » ; 3,8 / 5,8 / 7,8 pouces | Estimation | |
| Profil par défaut | 5" | Corpus | Taille la plus fréquente (366 sessions sur 708) |
| Fréquence du log trop basse | < 900 Hz | Communauté + estimation | 2 kHz recommandé, 1 kHz suffit pour la réponse indicielle ; 900 Hz laisse une marge sous 1 kHz |
| Harmoniques hors de la plage visible | 3 × f moteur > Fs / 2 | Théorie | Fréquence de Nyquist |
| Gyro non filtré absent, pas d'eRPM | présence des champs | Firmware | |
| Trames corrompues | > 20 | Estimation | |
| Mouvements de manche suffisants | ≥ 20 fenêtres | Estimation | |
| Fenêtre de réponse retenue | consigne ≥ 20 °/s | Communauté | Convention de PIDtoolbox |
| Dépassement | pic > 1,10 (conseil), > 1,20 (attention) ; objectif 1,02–1,08 | Communauté | `spec-methodes.md` juge « acceptable » jusqu'à 1,12 : la règle est plus sévère. Dans le corpus, le p90 vaut 1,2 à 1,3 : plus de 10 % des sessions sont en « attention » |
| Dépassement dû au feedforward | F > 0 et latence < p10 de la taille | Corpus + estimation | Repère de latence mesuré, raisonnement choisi |
| Réponse molle | pic < 0,98 et latence > p90 de la taille | Estimation (0,98) + corpus (latence) | |
| Roll et pitch différents | écart de pic > 0,08 | Estimation | |
| Stabilité | pic de sensibilité > 6 dB | Théorie | Pic ≤ 2 (6 dB) : critère de robustesse usuel. Dans le corpus, plus de 10 % des whoops et des 2–3,5" le dépassent (p90 6,9 et 7,5 dB) |
| Marge de phase | < 40° | Théorie + estimation | Les manuels visent 45–60° ; 40° est un compromis choisi. Le p10 des 2–3,5" vaut 37° |
| Bandes de bruit | 80–500 Hz ; 20–80 Hz | Communauté + estimation | `spec-methodes.md` situe le propwash entre 20 et 100 Hz ; bornes exactes choisies |
| Bruit gyro, terme D, 20–80 Hz, harmonique moteur | p90 de la taille | Corpus | Relatif au corpus : « plus bruité que 90 % », pas « mauvais » dans l'absolu |
| Retards des filtres gyro et D | p90 de la taille, arrondi vers le haut (+0,1 à +0,2 ms) | Corpus | |
| Latence (repères bas et haut) | p10 arrondi vers le bas, p90 vers le haut | Corpus | |
| Seuils du profil 8"+ | voir plus haut | Estimation | 20 sessions seulement |
| Résonance | raie ≥ 6 dB, 60–900 Hz, ≥ 8 tranches de gaz, à plus de ±8 % des harmoniques moteur | Estimation | |
| Déséquilibre des moteurs | > 12 % | Estimation | |
| Terme I décentré | moyenne > 40 (unités PID) | Estimation | |
| Saturation des moteurs | ≥ 98 % pendant > 5 % du temps | Estimation | Déjà mesurée par `calibrate_advisor.py`, pas encore exploitée |
| Pas des curseurs conseillés | 0,1 (0,1 à 0,2 pour les filtres) | Estimation | Prudence : un changement à la fois |
| `rpm_filter_q` conseillé | 500 → 350 | Firmware (500) + estimation (350) | |
| `motor_poles` | 14 (12 pour certains micro-moteurs) | Communauté | |
| Notch statique conseillé | coupure = 0,7 × fréquence de la raie | Estimation | |

Les champs `hover_hz` et `resonance_hz` des profils (`advisor/profiles.py`) sont renseignés mais
**aucune règle ne les utilise**.
