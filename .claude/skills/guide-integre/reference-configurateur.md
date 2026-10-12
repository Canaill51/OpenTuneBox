# Réglage → où le changer (configurateur Betaflight, en français)

Vérifié le 2026-10-11 dans les sources clonées à côté du projet : `../betaflight-configurator`
(`locales/fr/messages.json`, `src/components/tabs/`) et `../betaflight` (`src/main/cli/settings.c`).
Revérifier après une nouvelle version du configurateur (libellés : `grep` de la clé dans
`locales/fr/messages.json`).

| Réglage | Configurateur | CLI |
|---|---|---|
| Curseurs PID | onglet « Réglages de PID », curseurs de réglage PID : Atténuation (Damping), Suivi (Tracking), Réponse des sticks (Stick Response), Atténuation dynamique, Dérive - Oscillations (Drift - Wobble), Atténuation du Pitch, Suivi du Pitch, Multiplicateur Maître | `simplified_d_gain`, `simplified_pi_gain`, `simplified_feedforward_gain`, `simplified_d_max_gain` (`simplified_dmax_gain` en 4.x), `simplified_i_gain`, `simplified_pitch_d_gain`, `simplified_pitch_pi_gain`, `simplified_master_multiplier` (en %) |
| Mode des curseurs | même onglet, « Mode » : Désactivé / RP / RPY | `simplified_pids_mode` |
| Passe-bas gyro et D (force) | même onglet, filtres : « Multiplicateur du Filtre Gyro », « Multiplicateur de filtre D Term » | `simplified_gyro_filter_multiplier`, `simplified_dterm_filter_multiplier` |
| Filtre RPM | « Filtre Gyro RPM » : Harmoniques, Fréquence Min, Distance de fondu, Facteur de Q (x100), Poids 1–3 | `rpm_filter_harmonics`, `rpm_filter_min_hz`, `rpm_filter_fade_range_hz`, `rpm_filter_q`, `rpm_filter_weights` |
| Notch dynamique | « Filtre coupe bande dynamique » : Nombre de coupures, Facteur de Q (x100), Fréquence Min / Max | `dyn_notch_count`, `dyn_notch_q`, `dyn_notch_min_hz`, `dyn_notch_max_hz` |
| Fréquence d'enregistrement | onglet « Blackbox », « Fréquence d'enregistrement de la Blackbox » (affichée en Hz / kHz) | `blackbox_sample_rate = 1/N` |
| Mode de débogage | onglet « Blackbox », « Mode débug de la Blackbox » | `debug_mode` |
| Gyro non filtré (4.5+) | onglet « Blackbox », « Champs de débogage inclus » : Gyro (Unfiltered) | `blackbox_disable_gyrounfilt = OFF` |
| DShot bidirectionnel, pôles | onglet « Moteurs » : « DShot bi-directionnel », « Pôles moteur » | `dshot_bidir`, `motor_poles` |
| Rates (profil « LOG ») | onglet « Réglages de PID », profil de rates : « Type de rates » ACTUAL, « Sensibilité au centre », « Max Vel », Expo ; « Nom du profil Rate » | `rates_type`, `roll_rc_rate` (centre ÷ 10), `roll_srate` (max ÷ 10), `roll_expo` (idem pitch, yaw) ; en-tête du log : `rates_type` (3 = ACTUAL), `rc_rates`, `rates`, `rc_expo` |
| Limite d'angle | onglet « Réglages de PID » : « Limite de l'Angle » (profil PID) | `angle_limit` (4.5+, dans l'en-tête du log) ; `level_limit` en 4.4 (absent de l'en-tête) |
| Choix du profil de rates en vol | onglet « Ajustements » : « Sélection des profils de Rate » | — |
| Valeurs par défaut | onglet « Réglages de PID » : « Réinitialiser ce profil » (PID, curseurs, filtres du D seulement) | filtres généraux par défaut (4.4 → 2026.6) : RPM 3 harmoniques, Q 500, 100 Hz, fondu 50 ; notch 3 coupures, Q 300, 100–600 Hz (`pg/rpm_filter.c`, `pg/dyn_notch.c`) |
| Récupérer les logs | onglet « Blackbox » : « Activer le Mode de périphérique de stockage de masse », « Effacer la mémoire interne » | — |
| Chirp (2025.12+, firmware compilé avec `chirp` ; pas conseillé, D-019) | onglet « Modes » : mode CHIRP sur un interrupteur ; le reste seulement en CLI | `chirp_frequency_end_deci_hz`, `chirp_amplitude_*`, `chirp_time_seconds` |

Mode Expert (fenêtre « Options ») : sans lui, curseurs principaux limités à 0,7–1,4 (filtre gyro 0,5–1,5,
filtre D 0,8–1,2) et curseurs avancés (Atténuation dynamique, Dérive - Oscillations, Atténuation du Pitch,
Suivi du Pitch, Multiplicateur Maître) bloqués (`src/composables/useTuningSliders.ts`,
`PidSubTab.vue`).

Sauvegarder : bouton « Sauvegarder » de l'onglet ; en CLI, `save`.
