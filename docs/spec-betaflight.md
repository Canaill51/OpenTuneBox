# Référence Betaflight (tirée du code source)

Sources : clone du firmware `~/Documents/Github/betaflight` (master e796d7f871, 2026-09-30), clone du
configurateur `~/Documents/Github/betaflight-configurator`. Étiquettes utilisées par famille : 4.4.3, 4.5.5,
2025.12.5, 2026.6.1, master. La famille du firmware se lit dans l'en-tête `Firmware revision`
(`Betaflight 4.5.2 (…) STM32F7X2`) ; `Firmware type` vaut « Cleanflight » en 4.x et « Betaflight » à partir
de 2025.12.

## 1. Modes debug

La clé d'en-tête `debug_mode` est un **numéro** dans toutes les versions (master écrit en plus
`debug_mode_name`). Le numéro se traduit selon la version. Le configurateur a des tables équivalentes dans
`src/js/debug_modes_table.ts` (MSP 1.45 = 4.4, 1.46 = 4.5, 1.47 = 2025.12, 1.48 = 2026.6) ; attention, sa
liste 1.45 donne 79 = GYRO_CALIBRATION alors que les étiquettes 4.4.x ont 79 = DSHOT_TELEMETRY_COUNTS.

Nombre de champs debug : 4 en 4.4.x, 8 à partir de 4.5.

Identiques dans toutes les versions : 0 NONE, 1 CYCLETIME, 2 BATTERY, 3 GYRO_FILTERED, 4 ACCELEROMETER, 5 PIDLOOP.

| n° | 4.4.x / 4.5.x | 2025.12+ (GYRO_SCALED retiré → 6..27 décalés de −1) |
|---|---|---|
|6|GYRO_SCALED|RC_INTERPOLATION|
|7|RC_INTERPOLATION|ANGLERATE|
|8|ANGLERATE|ESC_SENSOR|
|9|ESC_SENSOR|SCHEDULER|
|10|SCHEDULER|STACK|
|11|STACK|ESC_SENSOR_RPM|
|12|ESC_SENSOR_RPM|ESC_SENSOR_TMP|
|13|ESC_SENSOR_TMP|ALTITUDE|
|14|ALTITUDE|FFT|
|15|FFT|FFT_TIME|
|16|FFT_TIME|FFT_FREQ|
|17|FFT_FREQ|RX_FRSKY_SPI|
|18|RX_FRSKY_SPI|RX_SFHSS_SPI|
|19|RX_SFHSS_SPI|GYRO_RAW|
|20|GYRO_RAW|MULTI_GYRO_RAW|
|21|DUAL_GYRO_RAW|MULTI_GYRO_DIFF|
|22|DUAL_GYRO_DIFF|MAX7456_SIGNAL|
|23|MAX7456_SIGNAL|MAX7456_SPICLOCK|
|24|MAX7456_SPICLOCK|SBUS|
|25|SBUS|FPORT|
|26|FPORT|RANGEFINDER|
|27|RANGEFINDER|RANGEFINDER_QUALITY|
|28|RANGEFINDER_QUALITY|OPTICALFLOW|

29–74 identiques partout : 29 LIDAR_TF, 30 ADC_INTERNAL, 31 RUNAWAY_TAKEOFF, 32 SDIO,
33 CURRENT_SENSOR, 34 USB, 35 SMARTAUDIO, 36 RTH, 37 ITERM_RELAX, 38 ACRO_TRAINER,
39 RC_SMOOTHING, 40 RX_SIGNAL_LOSS, 41 RC_SMOOTHING_RATE, 42 ANTI_GRAVITY, 43 DYN_LPF,
44 RX_SPEKTRUM_SPI, 45 DSHOT_RPM_TELEMETRY, 46 RPM_FILTER, 47 D_MIN (D_MAX en 2025.12+),
48 AC_CORRECTION, 49 AC_ERROR, 50 DUAL_GYRO_SCALED (MULTI_GYRO_SCALED en 2025.12+),
51 DSHOT_RPM_ERRORS, 52 CRSF_LINK_STATISTICS_UPLINK, 53 CRSF_LINK_STATISTICS_PWR,
54 CRSF_LINK_STATISTICS_DOWN, 55 BARO, 56 GPS_RESCUE_THROTTLE_PID (AUTOPILOT_ALTITUDE en 2025.12+),
57 DYN_IDLE, 58 FEEDFORWARD_LIMIT, 59 FEEDFORWARD, 60 BLACKBOX_OUTPUT, 61 GYRO_SAMPLE,
62 RX_TIMING, 63 D_LPF, 64 VTX_TRAMP, 65 GHST, 66 GHST_MSP, 67 SCHEDULER_DETERMINISM,
68 TIMING_ACCURACY, 69 RX_EXPRESSLRS_SPI, 70 RX_EXPRESSLRS_PHASELOCK, 71 RX_STATE_TIME,
72 GPS_RESCUE_VELOCITY, 73 GPS_RESCUE_HEADING, 74 GPS_RESCUE_TRACKING.

Fins de liste :
- 4.4.x : 75 ATTITUDE, 76 VTX_MSP, 77 GPS_DOP, 78 FAILSAFE, 79 DSHOT_TELEMETRY_COUNTS.
- 4.5.x et 2025.12+ : 75 GPS_CONNECTION, 76 ATTITUDE, 77 VTX_MSP, 78 GPS_DOP, 79 FAILSAFE,
  80 GYRO_CALIBRATION, 81 ANGLE_MODE, 82 ANGLE_TARGET, 83 CURRENT_ANGLE,
  84 DSHOT_TELEMETRY_COUNTS, 85 RPM_LIMIT, 86 RC_STATS, 87 MAG_CALIB, 88 MAG_TASK_RATE, 89 EZLANDING.
- 2025.12.x : 90 TPA, 91 S_TERM, 92 SPA, 93 TASK, 94 GIMBAL, 95 WING_SETPOINT,
  96 AUTOPILOT_POSITION, 97 CHIRP, 98 FLASH_TEST_PRBS, 99 MAVLINK_TELEMETRY.
- 2026.6.x : 90–95 comme 2025.12, 96 CHIRP, 97 FLASH_TEST_PRBS, 98 MAVLINK_TELEMETRY,
  99 AUTOPILOT_PID, 100 POSITION_NAV, 101 AUTOPILOT_STOP.
- master ajoute 102 PITOT, 103 POSITION_EST, 104 AUTOPILOT_HEADING, 105 RX_REDPINE_SPI,
  106 LAUNCH, 107 VELOCITY_EST, 108 SAG_COMPENSATION, 109 UPT1, 110 FLIGHT_PLAN.

### Contenu des champs debug
« axe-dbg » = en-tête `gyro_debug_axis` (2025.12+) / `gyro_filter_debug_axis` (4.x), roll par défaut.

- **FFT_FREQ** : 2025.12+ : [0] gyro avant notch dynamique (axe-dbg, °/s), [1..7] centre des notchs 1..7 en Hz.
  4.4/4.5 : [p] = centre en Hz du notch p (p < nombre), [3] = gyro avant notch dynamique (remplace le notch 4).
- **GYRO_SCALED** (4.x seulement) : [0..2] gyro non filtré en °/s (seule source non filtrée en 4.4).
- **RPM_FILTER** : [m] fréquence du moteur m en Hz après rpm_filter_lpf.
- **DSHOT_RPM_TELEMETRY** : [m] eRPM/100.
- **FEEDFORWARD** 2025.12+ : [0] consigne, [1] setpointSpeed × 0,01, [2] boost × 0,01,
  [3] |écart de rcCommand| × 10, [4] atténuation du jitter en %, [5] indicateur de doublon, [6][7] parts du FF yaw × 0,01.
- **D_LPF** : [0,1] delta D non filtré roll/pitch (unités de 25 °/s²), [2,3] D avant TPA (× 10 en 4.5+).
- **DYN_LPF** : [0] gyro avant notch dynamique, [1] notch 1 en Hz, [2] coupure du LPF1 gyro dynamique en Hz, [3] gyro après notch.
- **CHIRP** : [0] phase × 5000, [1] axe ou −1, [2] fréquence × 10, [3] excitation × 1000.
- **GYRO_SAMPLE** : [0] brut avant sous-échantillonnage, [1] après sous-échantillonnage, [2] après RPM,
  [3] après notchs statiques + LPF1, [4] CPU % (2025.12+).

## 2. Filtres (`src/main/common/filter.c`)

- **PT1** : `k = ω/(ω+1)`, `ω = 2π fc dT`; `y += k(x−y)`; `H(z) = k/(1−(1−k)z⁻¹)`.
- **PT2/PT3** : cascade de n PT1 identiques, coupure corrigée `k = pt1Gain(fc·C, dT)`,
  `C = 1/sqrt(2^(1/n)−1)`: C_PT2 = 1.553773974, C_PT3 = 1.961459177.
- **Biquad (4.4–2025.12)** RBJ : `ω=2πf Ts, sn=sin ω, cs=cos ω, α=sn/(2Q), a0=1+α`.
  LPF (Q = 1/√2) : b0=b2=(1−cs)/2, b1=1−cs, a1=−2cs, a2=1−α.
  Notch : b0=1, b1=−2cs, b2=1, a1=−2cs, a2=1−α. Passe-bande : b0=α, b1=0, b2=−α. Tout est divisé par a0.
  Notch RPM pondéré : `y = w·notch(x) + (1−w)·x`.
- **SVF/TPT (2026.6+)** : transformation bilinéaire avec précompensation à f0 du même prototype analogique →
  même réponse en fréquence que RBJ à f0/Q égaux ; simulé avec les coefficients RBJ (écart de précompensation minime).
- Types de filtre : 0 PT1, 1 BIQUAD (SVF à partir de 2026.6), 2 PT2, 3 PT3.
- Q du notch statique : `Q = f0·fc/(f0²−fc²)` (f0 = notch_hz, fc = notch_cutoff) ; centre limité à 0,95 × Nyquist.
- **LPF dynamique selon les gaz** (t ∈ [0,1], pas de 0,01, mis à jour au plus toutes les 5 ms) :
  expo > 0 : `curve = t(1−t)(expo/10) + t`, `fc = (max−min)·curve + min`;
  expo = 0 : `fc = max(min, max·t(1−t²/3)·1.5)`. Si dyn_min > 0, il remplace le LPF1 statique. Expo 5 par défaut.

## 3. Chaînes de filtres

- Tâche d'échantillonnage du gyro à `looptime` (cadence gyro) ; filtrage + PID à `looptime × pid_process_denom`.
- À la cadence gyro : brut → alignement → mise à l'échelle → `gyroADC` (loggé `gyroUnfilt`) → LPF2 à chaque
  échantillon ; sous-échantillonnage = dernière sortie du LPF2 (ou moyenne si LPF2 désactivé).
- À la cadence PID, dans l'ordre : filtre RPM → notch statique 1 → notch statique 2 → LPF1 (statique / dynamique)
  → notchs dynamiques → `gyroADCf` (loggé `gyroADC`).
- **Filtre RPM** : un notch par moteur × harmonique à `(h+1)·motorHz`, borné à `[min_hz, 0.48/looptime_s]`,
  Q = rpm_filter_q/100, poids `rpm_filter_weights[h]/100` × atténuation `(fh−min)/fade_range` sous la zone de fondu.
  motorHz = PT1(rpm_filter_lpf_hz) de `erpm·100/60/(poles/2)`.
  Défauts : 3 harmoniques, min 100, fondu 50, q 500, lpf 150, poids 100,100,100 (4.5+).
- **Notch dynamique** : défauts min 100, max 600, q 300, 3 notchs (7 au maximum) ; désactivé sous 2 kHz de PID.
  SDFT sur 72 échantillons après moyenne par groupes de `max(1, floor(nyq/maxHz))` ; binHz = (pidRate/sampleCount)/72 ;
  recherche des pics : maxima locaux > 2 × plancher de bruit, interpolation parabolique, centre lissé par un PT1
  de 4 Hz × clamp(pic/seuil, 1, 10) à 12/pidRate. Centres initiaux `(p+0.5)(max−min)/count + min`.
- **Chaîne du terme D** (cadence PID, sur gyroADCf) : notch D → LPF1 D (statique / dynamique) → LPF2 D.
  D sur la mesure : `delta = −(g[n]−g[n−1])·pidFreq`, `D = Kd·delta·dMaxMult·TPA`.
  Le P du yaw passe par un PT1 `yaw_lowpass_hz` (100 par défaut).
- Défauts : LPF1 gyro PT1 dynamique 250–500 expo 5 ; LPF2 gyro PT1 500 ; LPF1 D PT1 dynamique 75–150 expo 5 ;
  LPF2 D PT1 150 ; notchs désactivés.

## 4. Clés d'en-tête

- Cadences : `looptime` (période gyro en µs), `pid_process_denom`, `P interval` (= 1<<sample_rate ; une boucle PID
  loggée sur N ; les en-têtes anciens peuvent écrire `1/2`), `I interval`, `P ratio`.
  **Fréquence du log = 1e6 / (looptime·pid_process_denom·P_interval)**.
- PID : `rollPID`, `pitchPID`, `yawPID` = "P,I,D", `levelPID`, `ff_weight` = F r,p,y.
- D boost : 4.4/4.5 `d_min` (D min par axe ; D de rollPID = D max) ; 2025.12+ `d_max` (max par axe ; D de rollPID = D de base).
  `d_max_gain`, `d_max_advance` partout.
- Feedforward : `feedforward_transition/_averaging/_smooth_factor/_jitter_factor/_boost/_max_rate_limit`
  (+ `feedforward_yaw_hold_gain/_time` 2025.12+).
  En mode **Angle**, le feedforward de la boucle de vitesse est forcé à 0 sur les axes nivelés
  (`pidSetpointDelta = 0`, `flight/pid.c`, vérifié au commit cc1bb9d28 ; déjà le cas avant 4.5 selon le
  commentaire du code) ; `angle_feedforward` n'agit que sur la consigne d'angle, en amont. Pas en Horizon :
  le feedforward y reste actif. Le terme F loggé vaut 0 dans les vols en Angle du corpus.
- Réglage simplifié : `simplified_pids_mode`, `simplified_master_multiplier`, `simplified_i_gain`,
  `simplified_d_gain`, `simplified_pi_gain`, `simplified_feedforward_gain`, `simplified_pitch_d_gain`
  (= roll_pitch_ratio), `simplified_pitch_pi_gain`, `simplified_dterm_filter(_multiplier)`,
  `simplified_gyro_filter(_multiplier)`, `simplified_dmax_gain` (4.x) / `simplified_d_max_gain` (2025.12+).
- Filtres gyro : `gyro_lpf1_type`, `gyro_lpf1_static_hz`, `gyro_lpf1_dyn_hz` ("min,max"),
  `gyro_lpf1_dyn_expo` (4.5+), `gyro_lpf2_type`, `gyro_lpf2_static_hz`, `gyro_notch_hz` ("h1,h2"),
  `gyro_notch_cutoff`.
- Filtres D : `dterm_lpf1_type`, `dterm_lpf1_static_hz`, `dterm_lpf1_dyn_hz`, `dterm_lpf1_dyn_expo`,
  `dterm_lpf2_type`, `dterm_lpf2_static_hz`, `dterm_notch_hz`, `dterm_notch_cutoff`, `yaw_lowpass_hz`.
- Notch dynamique / RPM : `dyn_notch_max_hz/_count/_q/_min_hz`, `dshot_bidir`, `motor_poles`,
  `rpm_filter_harmonics/_weights/_q/_min_hz/_fade_range_hz/_lpf_hz`.
- TPA : `tpa_mode`, `tpa_rate`, `tpa_breakpoint` (+ `tpa_low_*` 4.5+, `tpa_curve_*` 2025.12+).
- Anti-gravity / terme I : `anti_gravity_gain/_cutoff_hz/_p_gain`, `iterm_relax(_type/_cutoff)`, `iterm_windup`.
- Rates : `rates_type` (0 BF, 1 RACEFLIGHT, 2 KISS, 3 ACTUAL, 4 QUICK), `rc_rates`, `rc_expo`, `rates`,
  `rate_limits`, `thr_mid`, `thr_expo` (+ `thr_hover` 2025.12+).
- Moteurs : `motorOutput` = « bas,haut », `motor_output_limit`, `throttle_limit_type/_percent`,
  ralenti `dshot_idle_value` (4.x) / `motor_idle` (2025.12+) en 0,01 %, `motor_kv` (2025.12+), `dyn_idle_*`.
- Batterie : `vbatref`, `vbatcellvoltage` (« min,alerte,max » en 0,01 V). Autres : `acc_1G`,
  `blackbox_high_resolution`, `fields_disabled_mask`, `features`, `rc_smoothing*`.

## 5. Réglage simplifié (`config/simplified_tuning.c`)

Tous les curseurs sont divisés par 100 : M master, PI pi_gain, Dg d_gain, FF feedforward_gain, Ig i_gain ;
pitch seulement : pr = roll_pitch_ratio, pp = pitch_pi_gain (1 en roll / yaw). Axes concernés selon
`simplified_pids_mode` (1 = RP, 2 = RPY). Résultats tronqués à l'entier.

- `P = Pdef·M·PI·pp`, `I = Idef·M·PI·Ig·pp`, `F = Fdef·M·pp·FF`.
- 2025.12+ : défauts R {45,80,30,120}, P {47,84,34,125}, Y {45,80,0,120}; D_MAX_DEFAULT {40,46,0}.
  `D = Ddef·M·Dg·pr`; `dMaxGain = s + (1−s)·Ddef/DmaxDef` (s = curseur d_max_gain / 100) ;
  `d_max = DmaxDef·M·Dg·pr·dMaxGain`.
- 4.4/4.5 : défauts R {45,80,40,120}, P {47,84,46,125}; D_MIN_DEFAULT {30,34,0}.
  `d_min = DminDef·M·Dg·pr`; `D(max) = DminDef·M·Dg·pr·(1 + (Ddef−DminDef)/DminDef·s)`.
- Multiplicateurs de filtres m : gyro dynamique 250m–500m, statique 250m, LPF2 500m ; D dynamique 75m–150m,
  statique 75m, LPF2 150m (appliqués seulement aux réglages non nuls, limités à 1000).
- Défauts : curseurs 100, mode RPY ; d_max_gain/advance 37/20 (4.4–2025.12), 0/35 (2026.6+) ;
  ff smooth 25 (4.x) / 65 (2025.12+) ; ff averaging OFF / 2_POINT ; iterm_windup 85 / 80.

## 6. Sortie moteurs et eRPM

- `motor[i]` : DShot 48..2047 armé, 0 désarmé. Bornes dans l'en-tête `motorOutput`.
  Pourcentage : `(v−48)/1999·100` (configurateur) ou `(v−bas)/(haut−bas)`.
- `eRPM[i]` = eRPM/100 (4.5+ ; en 4.4 par le debug DSHOT_RPM_TELEMETRY).
  `RPM = v·200/poles`; `Hz = v·100/60/(poles/2) = v·3.333/poles`.

## 7. Grandeurs loggées et unités

- `hr = 10` si `blackbox_high_resolution`, sinon 1.
- `gyroADC` = gyro entièrement filtré × hr (°/s) ; `gyroUnfilt` = dernier échantillon brut × hr (4.5+, repliement possible).
- `accSmooth` en valeurs brutes, / `acc_1G` → g.
- `rcCommand[0..2]` ±500 × hr (signe inversé en yaw) ; `rcCommand[3]` 1000..2000.
- `setpoint[0..2]` °/s × hr ; `setpoint[3]` = gaz du mixeur × 1000.
- `axisP/I/D/F` en unités de somme PID (1000 = toute l'autorité du mixeur) :
  `P = 0.032029·Kp·err·TPA` ; I croît de `0.244381·Ki·err` par seconde (yaw × 2,5) ;
  `D = 0.000529·Kd·(−dgyro/dt)·dMax·TPA`; `F = 0.013754·(F/100)·setpointSpeed`.
- `vbatLatest` en 0,01 V, `amperageLatest` en 0,01 A (convertis par le décodeur).
