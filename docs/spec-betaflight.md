# Betaflight reference spec (extracted from source)

Sources: firmware clone `~/Documents/Github/betaflight` (master e796d7f871, 2026-09-30),
configurator clone `~/Documents/Github/betaflight-configurator`. Tags used per family:
4.4.3, 4.5.5, 2025.12.5, 2026.6.1, master. Firmware family is identified from the
`Firmware revision` header (`Betaflight 4.5.2 (…) STM32F7X2`); `Firmware type` is
"Cleanflight" in 4.x and "Betaflight" from 2025.12.

## 1. Debug modes

Header key `debug_mode` is a **number** in every release (master additionally writes
`debug_mode_name`). Map the number per version. The configurator holds equivalent tables in
`src/js/debug_modes_table.ts` (MSP 1.45=4.4, 1.46=4.5, 1.47=2025.12, 1.48=2026.6) — beware its
1.45 list has 79=GYRO_CALIBRATION while 4.4.x release tags have 79=DSHOT_TELEMETRY_COUNTS.

Number of debug fields: 4 in 4.4.x, 8 from 4.5.

Same in all versions: 0 NONE, 1 CYCLETIME, 2 BATTERY, 3 GYRO_FILTERED, 4 ACCELEROMETER, 5 PIDLOOP.

| idx | 4.4.x / 4.5.x | 2025.12+ (GYRO_SCALED removed → 6..27 shift −1) |
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

29–74 identical everywhere: 29 LIDAR_TF, 30 ADC_INTERNAL, 31 RUNAWAY_TAKEOFF, 32 SDIO,
33 CURRENT_SENSOR, 34 USB, 35 SMARTAUDIO, 36 RTH, 37 ITERM_RELAX, 38 ACRO_TRAINER,
39 RC_SMOOTHING, 40 RX_SIGNAL_LOSS, 41 RC_SMOOTHING_RATE, 42 ANTI_GRAVITY, 43 DYN_LPF,
44 RX_SPEKTRUM_SPI, 45 DSHOT_RPM_TELEMETRY, 46 RPM_FILTER, 47 D_MIN (D_MAX in 2025.12+),
48 AC_CORRECTION, 49 AC_ERROR, 50 DUAL_GYRO_SCALED (MULTI_GYRO_SCALED 2025.12+),
51 DSHOT_RPM_ERRORS, 52 CRSF_LINK_STATISTICS_UPLINK, 53 CRSF_LINK_STATISTICS_PWR,
54 CRSF_LINK_STATISTICS_DOWN, 55 BARO, 56 GPS_RESCUE_THROTTLE_PID (AUTOPILOT_ALTITUDE 2025.12+),
57 DYN_IDLE, 58 FEEDFORWARD_LIMIT, 59 FEEDFORWARD, 60 BLACKBOX_OUTPUT, 61 GYRO_SAMPLE,
62 RX_TIMING, 63 D_LPF, 64 VTX_TRAMP, 65 GHST, 66 GHST_MSP, 67 SCHEDULER_DETERMINISM,
68 TIMING_ACCURACY, 69 RX_EXPRESSLRS_SPI, 70 RX_EXPRESSLRS_PHASELOCK, 71 RX_STATE_TIME,
72 GPS_RESCUE_VELOCITY, 73 GPS_RESCUE_HEADING, 74 GPS_RESCUE_TRACKING.

Tails:
- 4.4.x: 75 ATTITUDE, 76 VTX_MSP, 77 GPS_DOP, 78 FAILSAFE, 79 DSHOT_TELEMETRY_COUNTS.
- 4.5.x and 2025.12+: 75 GPS_CONNECTION, 76 ATTITUDE, 77 VTX_MSP, 78 GPS_DOP, 79 FAILSAFE,
  80 GYRO_CALIBRATION, 81 ANGLE_MODE, 82 ANGLE_TARGET, 83 CURRENT_ANGLE,
  84 DSHOT_TELEMETRY_COUNTS, 85 RPM_LIMIT, 86 RC_STATS, 87 MAG_CALIB, 88 MAG_TASK_RATE, 89 EZLANDING.
- 2025.12.x: 90 TPA, 91 S_TERM, 92 SPA, 93 TASK, 94 GIMBAL, 95 WING_SETPOINT,
  96 AUTOPILOT_POSITION, 97 CHIRP, 98 FLASH_TEST_PRBS, 99 MAVLINK_TELEMETRY.
- 2026.6.x: 90–95 as 2025.12, 96 CHIRP, 97 FLASH_TEST_PRBS, 98 MAVLINK_TELEMETRY,
  99 AUTOPILOT_PID, 100 POSITION_NAV, 101 AUTOPILOT_STOP.
- master adds 102 PITOT, 103 POSITION_EST, 104 AUTOPILOT_HEADING, 105 RX_REDPINE_SPI,
  106 LAUNCH, 107 VELOCITY_EST, 108 SAG_COMPENSATION, 109 UPT1, 110 FLIGHT_PLAN.

### Debug field contents
"dbg-axis" = header `gyro_debug_axis` (2025.12+) / `gyro_filter_debug_axis` (4.x), default roll.

- **FFT_FREQ**: 2025.12+: [0] gyro pre-dyn-notch (dbg-axis, deg/s), [1..7] notch 1..7 centre Hz.
  4.4/4.5: [p] = centre Hz of notch p (p<count), [3] = gyro pre-dyn-notch (overwrites notch 4).
- **GYRO_SCALED** (4.x only): [0..2] unfiltered gyro deg/s (only unfiltered source in 4.4).
- **RPM_FILTER**: [m] motor m Hz after rpm_filter_lpf.
- **DSHOT_RPM_TELEMETRY**: [m] eRPM/100.
- **FEEDFORWARD** 2025.12+: [0] setpoint, [1] setpointSpeed×0.01, [2] boost×0.01,
  [3] |rcCommand delta|×10, [4] jitter attenuator %, [5] duplicate flag, [6][7] yaw FF parts ×0.01.
- **D_LPF**: [0,1] unfiltered D delta roll/pitch (units 25 deg/s²), [2,3] D pre-TPA (×10 in 4.5+).
- **DYN_LPF**: [0] gyro pre-dyn-notch, [1] notch-1 Hz, [2] dyn gyro LPF1 cutoff Hz, [3] gyro post-notch.
- **CHIRP**: [0] phase×5000, [1] axis or −1, [2] freq×10, [3] excitation×1000.
- **GYRO_SAMPLE**: [0] raw before downsampling, [1] after downsampling, [2] after RPM,
  [3] after static notches + LPF1, [4] CPU % (2025.12+).

## 2. Filters (`src/main/common/filter.c`)

- **PT1**: `k = ω/(ω+1)`, `ω = 2π fc dT`; `y += k(x−y)`; `H(z) = k/(1−(1−k)z⁻¹)`.
- **PT2/PT3**: cascade of n identical PT1 with cutoff corrected `k = pt1Gain(fc·C, dT)`,
  `C = 1/sqrt(2^(1/n)−1)`: C_PT2 = 1.553773974, C_PT3 = 1.961459177.
- **Biquad (4.4–2025.12)** RBJ: `ω=2πf Ts, sn=sin ω, cs=cos ω, α=sn/(2Q), a0=1+α`.
  LPF (Q=1/√2): b0=b2=(1−cs)/2, b1=1−cs, a1=−2cs, a2=1−α.
  Notch: b0=1, b1=−2cs, b2=1, a1=−2cs, a2=1−α. Bandpass: b0=α, b1=0, b2=−α. Normalise by a0.
  RPM notch weighted: `y = w·notch(x) + (1−w)·x`.
- **SVF/TPT (2026.6+)**: bilinear with prewarp at f0 of the same analog prototype → same
  frequency response as RBJ with same f0/Q; simulate with RBJ coefficients (minor prewarp diff).
- Filter type enum: 0 PT1, 1 BIQUAD (SVF from 2026.6), 2 PT2, 3 PT3.
- Static notch Q: `Q = f0·fc/(f0²−fc²)` (f0 = notch_hz, fc = notch_cutoff); centre capped 0.95·Nyquist.
- **Dynamic LPF vs throttle** (t ∈ [0,1], quantised 0.01, updated ≤ every 5 ms):
  expo>0: `curve = t(1−t)(expo/10) + t`, `fc = (max−min)·curve + min`;
  expo=0: `fc = max(min, max·t(1−t²/3)·1.5)`. When dyn_min>0 it replaces LPF1 static. Default expo 5.

## 3. Filter chains

- Gyro sample task at `looptime` (gyro rate); filtering + PID at `looptime × pid_process_denom`.
- At gyro rate: raw → align → scale → `gyroADC` (logged as `gyroUnfilt`) → LPF2 every sample;
  downsample = last LPF2 output (or mean if LPF2 off).
- At PID rate, in order: RPM filter → static notch 1 → static notch 2 → LPF1 (static/dynamic)
  → dynamic notches → `gyroADCf` (logged `gyroADC`).
- **RPM filter**: notch per motor×harmonic at `(h+1)·motorHz`, clamped `[min_hz, 0.48/looptime_s]`,
  Q = rpm_filter_q/100, weight `rpm_filter_weights[h]/100` × fade `(fh−min)/fade_range` if below fade.
  motorHz = PT1(rpm_filter_lpf_hz) of `erpm·100/60/(poles/2)`.
  Defaults: harmonics 3, min 100, fade 50, q 500, lpf 150, weights 100,100,100 (4.5+).
- **Dynamic notch**: defaults min 100, max 600, q 300, count 3 (max 7); disabled < 2 kHz PID.
  SDFT 72 samples after averaging groups of `max(1, floor(nyq/maxHz))`; binHz = (pidRate/sampleCount)/72;
  peak picking: local maxima > 2× noise floor, parabolic interp, centre smoothed by PT1 4 Hz×clamp(peak/thr,1,10)
  at 12/pidRate. Initial centres `(p+0.5)(max−min)/count + min`.
- **D-term chain** (PID rate, on gyroADCf): D notch → D LPF1 (static/dynamic) → D LPF2.
  D on measurement: `delta = −(g[n]−g[n−1])·pidFreq`, `D = Kd·delta·dMaxMult·TPA`.
  Yaw P has PT1 `yaw_lowpass_hz` (default 100).
- Defaults: gyro LPF1 PT1 dyn 250–500 expo 5; gyro LPF2 PT1 500; D LPF1 PT1 dyn 75–150 expo 5;
  D LPF2 PT1 150; notches off.

## 4. Header keys

- Timing: `looptime` (gyro period µs), `pid_process_denom`, `P interval` (= 1<<sample_rate; log every
  N-th PID loop; older headers may write `1/2`), `I interval`, `P ratio`.
  **Log rate = 1e6 / (looptime·pid_process_denom·P_interval)**.
- PIDs: `rollPID`, `pitchPID`, `yawPID` = "P,I,D", `levelPID`, `ff_weight` = F r,p,y.
- D boost: 4.4/4.5 `d_min` (per-axis D-min; rollPID D = max D); 2025.12+ `d_max` (per-axis max; rollPID D = base D).
  `d_max_gain`, `d_max_advance` in all.
- Feedforward: `feedforward_transition/_averaging/_smooth_factor/_jitter_factor/_boost/_max_rate_limit`
  (+ `feedforward_yaw_hold_gain/_time` 2025.12+).
- Simplified: `simplified_pids_mode`, `simplified_master_multiplier`, `simplified_i_gain`,
  `simplified_d_gain`, `simplified_pi_gain`, `simplified_feedforward_gain`, `simplified_pitch_d_gain`
  (= roll_pitch_ratio), `simplified_pitch_pi_gain`, `simplified_dterm_filter(_multiplier)`,
  `simplified_gyro_filter(_multiplier)`, `simplified_dmax_gain` (4.x) / `simplified_d_max_gain` (2025.12+).
- Gyro filters: `gyro_lpf1_type`, `gyro_lpf1_static_hz`, `gyro_lpf1_dyn_hz` ("min,max"),
  `gyro_lpf1_dyn_expo` (4.5+), `gyro_lpf2_type`, `gyro_lpf2_static_hz`, `gyro_notch_hz` ("h1,h2"),
  `gyro_notch_cutoff`.
- D filters: `dterm_lpf1_type`, `dterm_lpf1_static_hz`, `dterm_lpf1_dyn_hz`, `dterm_lpf1_dyn_expo`,
  `dterm_lpf2_type`, `dterm_lpf2_static_hz`, `dterm_notch_hz`, `dterm_notch_cutoff`, `yaw_lowpass_hz`.
- Dyn notch / RPM: `dyn_notch_max_hz/_count/_q/_min_hz`, `dshot_bidir`, `motor_poles`,
  `rpm_filter_harmonics/_weights/_q/_min_hz/_fade_range_hz/_lpf_hz`.
- TPA: `tpa_mode`, `tpa_rate`, `tpa_breakpoint` (+ `tpa_low_*` 4.5+, `tpa_curve_*` 2025.12+).
- AG / iterm: `anti_gravity_gain/_cutoff_hz/_p_gain`, `iterm_relax(_type/_cutoff)`, `iterm_windup`.
- Rates: `rates_type` (0 BF, 1 RACEFLIGHT, 2 KISS, 3 ACTUAL, 4 QUICK), `rc_rates`, `rc_expo`, `rates`,
  `rate_limits`, `thr_mid`, `thr_expo` (+ `thr_hover` 2025.12+).
- Motors: `motorOutput` = "low,high", `motor_output_limit`, `throttle_limit_type/_percent`,
  idle `dshot_idle_value` (4.x) / `motor_idle` (2025.12+) in 0.01 %, `motor_kv` (2025.12+), `dyn_idle_*`.
- Battery: `vbatref`, `vbatcellvoltage` ("min,warn,max" in 0.01 V). Other: `acc_1G`,
  `blackbox_high_resolution`, `fields_disabled_mask`, `features`, `rc_smoothing*`.

## 5. Simplified tuning (`config/simplified_tuning.c`)

All sliders /100: M master, PI pi_gain, Dg d_gain, FF feedforward_gain, Ig i_gain;
pitch only: pr = roll_pitch_ratio, pp = pitch_pi_gain (1 on roll/yaw). Axes up to `simplified_pids_mode`
(1=RP, 2=RPY). Results truncated to int.

- `P = Pdef·M·PI·pp`, `I = Idef·M·PI·Ig·pp`, `F = Fdef·M·pp·FF`.
- 2025.12+: defaults R {45,80,30,120}, P {47,84,34,125}, Y {45,80,0,120}; D_MAX_DEFAULT {40,46,0}.
  `D = Ddef·M·Dg·pr`; `dMaxGain = s + (1−s)·Ddef/DmaxDef` (s = d_max_gain slider/100);
  `d_max = DmaxDef·M·Dg·pr·dMaxGain`.
- 4.4/4.5: defaults R {45,80,40,120}, P {47,84,46,125}; D_MIN_DEFAULT {30,34,0}.
  `d_min = DminDef·M·Dg·pr`; `D(max) = DminDef·M·Dg·pr·(1 + (Ddef−DminDef)/DminDef·s)`.
- Filter multipliers m: gyro dyn 250m–500m, static 250m, LPF2 500m; D dyn 75m–150m, static 75m, LPF2 150m
  (only applied to nonzero settings, clamp 1000).
- Defaults: sliders 100, mode RPY; d_max_gain/advance 37/20 (4.4–2025.12), 0/35 (2026.6+);
  ff smooth 25 (4.x) / 65 (2025.12+); ff averaging OFF / 2_POINT; iterm_windup 85 / 80.

## 6. Motor output and eRPM

- `motor[i]`: DShot 48..2047 armed, 0 disarmed. Endpoints `motorOutput` header.
  Percent: `(v−48)/1999·100` (configurator) or `(v−low)/(high−low)`.
- `eRPM[i]` = eRPM/100 (4.5+; 4.4 via debug DSHOT_RPM_TELEMETRY).
  `RPM = v·200/poles`; `Hz = v·100/60/(poles/2) = v·3.333/poles`.

## 7. Logged quantities and units

- `hr = 10` if `blackbox_high_resolution` else 1.
- `gyroADC` = fully filtered gyro ×hr (deg/s); `gyroUnfilt` = latest raw sample ×hr (4.5+, may alias).
- `accSmooth` raw counts, /`acc_1G` → g.
- `rcCommand[0..2]` ±500 ×hr (yaw sign-inverted); `rcCommand[3]` 1000..2000.
- `setpoint[0..2]` deg/s ×hr; `setpoint[3]` = mixer throttle ×1000.
- `axisP/I/D/F` in PID-sum units (1000 = full mixer authority):
  `P = 0.032029·Kp·err·TPA`; I rate `0.244381·Ki·err`/s (yaw ×2.5);
  `D = 0.000529·Kd·(−dgyro/dt)·dMax·TPA`; `F = 0.013754·(F/100)·setpointSpeed`.
- `vbatLatest` 0.01 V, `amperageLatest` 0.01 A (decoder tools convert).
