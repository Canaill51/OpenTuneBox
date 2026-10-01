# Analysis methods spec

Reimplemented from public descriptions (PIDtoolbox v0.55 public code/wiki, PID-Analyzer,
Blackbox Explorer, BlackBox_CSV_Render, pid-tuna). **No code is copied.** PTB Pro internals
(filter sim, PID slider tool, RPM estimator, AI assistant) are not public: designed here from
Betaflight's own formulas (`spec-betaflight.md`).

## Loading conventions (PTB)

- Log rate `Fs = 1e6 / median(diff(time_us))`; all windows sized in ms × Fs.
- Throttle % = `setpoint[3] / 10`. Motors % = `(motor−48)/1999·100` (DShot).
- PID error = `gyroADC − setpoint` (PTB sign).
- **D prefilter** (synthetic): `Dpf = −diff(gyroADC)` scaled by `median(smooth(axisD,100)/smooth(Dpf,100))`
  over finite positive ratios (roll/pitch only). Better: compute exactly from Kd, PID rate, TPA (we know gains).
- **Gyro prefilter**: `gyroUnfilt` (4.5+), else debug GYRO_SCALED [0..2] (4.4), else unavailable.

## Step response (Wiener deconvolution)

Inputs SP = setpoint[axis], GY = gyroADC[axis] over the selected period.
1. Optional gyro smoothing (spans 1/20/40/60 samples).
2. Segments of 2 s (configurable), hop = len/subsample: subsample 10 (log ≤ 20 s), 7 (20–60 s), 3 (> 60 s).
3. Keep segment if max|SP| ≥ 20 deg/s.
4. Hann window on both, zero-pad 100 samples each side.
5. `G = FFT(gy)/N`, `H = FFT(sp)/N`, `imp = real(IFFT(G·conj(H)/(|H|² + 1e-4)))`, step = cumsum(imp).
   (PTB regularisation is amplitude dependent; PID-Analyzer alternative: freq mask, low reg < 25 Hz;
   pid-tuna: λ = 0.01·mean|X|². Offer PTB-compatible default + robust option.)
6. Y correction: m = mean(resp, 200–500 ms); resp *= (2 − m) (PTB); true normalisation resp/m optional.
7. QC: keep if min(steady) > 0.5 and max(steady) < 3.
8. Response 500 ms; plot mean ± SD and n. Split by max|SP| < / ≥ 500 deg/s optional.
9. Metrics on mean: peak = max(t < 150 ms), peak time; latency = first t where resp > 0.5, minus 1 ms;
   rise time 10–90 %; settling ±5 %.

Loop metrics (our design, from the same deconvolution): T(f) = closed-loop SP→gyro frequency
response (average of segment cross-spectra / auto-spectra); sensitivity S = 1 − T, peak |S| dB and its
frequency (thresholds 4 dB yellow / 6 dB red as in PTB); open loop L = T/(1−T): crossover where |L|=1,
phase margin = 180° + ∠L; bandwidth = −3 dB of |T|. SP→gyro delay by cross-correlation.

## Spectra

- **2D spectrum**: Hann-windowed periodogram over the selection (we use Welch-averaged for stability, PTB-like
  smoothing option). Amplitude `|X|/N` or PSD `|X|²/(Fs·N)` one-sided ×2 → `10·log10`. PTB y-range −50..20 dB.
- **Freq × Throttle**: 300 ms segments, hop 300/subsample (5 if ≤20 s, 3 for 20–60 s, 1 > 60 s), mean throttle
  per segment, 100 bins with ±1 % overlap, Hann, amplitude or PSD averaged per bin → 3.33 Hz resolution.
  Display: Gaussian smoothing kernel [5s × s], σ=4 (s = smoothing 1..); colormap "hot"; scale amplitude 0–0.5,
  PSD −40..10 dB. Annotations mean & peak over 100–1000 Hz (or Flim1–Flim2 = 20–60 Hz in <100 Hz mode).
  X axis % throttle or % motor (or motor Hz from eRPM, like BBE).
- Presets (PTB): 1 Gyro pf | Gyro | Dterm pf | Dterm; 2 Gyro pf | Gyro | Pterm | Dterm;
  3 Gyro | Dterm | SP | PID err; 4 A|A|B|B Gyro pf|Gyro; 5 A|A|B|B Dterm pf|Dterm;
  6–9 A|B|C|D Gyro pf / Gyro / Dterm / PID error.
- **Freq × Time**: 300 ms centred windows, hop 300/{1,2,5,10}, PSD, frequency smoothing {1,5,10,20} bins,
  colormap hot, Z −30..10 dB. Overlays motor harmonics (eRPM) and dyn notch centres (FFT_FREQ debug).
- Motor noise panel: mean ± SD of PSD at motor harmonic frequencies (1st/2nd/3rd), filtered vs prefiltered;
  text "Filter weights | Q" from RPM filter header.
- RPM estimate when no telemetry: motor Hz ≈ multiplier(2.1) × motor % (PTB "RPM est.") — calibrate per craft.

## Delays

Signals smoothed (50-sample moving average), lag = argmax cross-correlation within ±30 ms; resolution 1 sample.
Pairs: gyro filter (gyroUnfilt → gyroADC), D filter (Dpf → axisD), SP→gyro per axis, SP smoothing.
Blank below 0.1 ms. We add sub-sample parabolic interpolation and a phase-slope (group delay) estimate.

## Other tools

- PID error: histogram per axis, SD; mean |error| vs stick deflection; limited to |SP| ≤ 100 deg/s by default.
- Flight stats: % of flight histograms of stick positions/throttle, mean ± SD of |gyro|, |P|, |I|, |D|, |F|, motors.
- Setup info: table of header settings per log.

## Logging recommendations
2 kHz log rate (1 kHz enough for step response); debug FFT_FREQ (or GYRO_SCALED on 4.4);
20–30 s flights with full-stick snaps on every axis for step response; hover + throttle sweeps for noise.

## Tuning heuristics (guidance, lower confidence)

- Step peak: < 1.00 overdamped; 1.02–1.08 optimal; 1.08–1.12 acceptable; 1.12–1.20 add D ~10–30 %;
  > 1.20 add D more; > 1.5 suspect mechanical. Bounce-back after flips → more D. Higher P = faster, more overshoot.
- Bands (5"): < 20 Hz commanded motion; 20–100 Hz propwash / oscillation; 100–250 Hz frame resonance /
  loose parts; > 250 Hz motors/props. Gyro noise floor < −30 dB above 50 Hz; D-term < −10 dB (BBE PSD).
  Total filter latency ideally < 5 ms. Resonance = horizontal line on freq×throttle; motor = diagonal.
- Motor fundamental by size (first-order physics, RPM ≈ KV·V·0.75): whoop 65–75 mm 550–800 Hz hover /
  1.1–1.4 kHz full; 2.5–3.5" 350–450 / 800–950; 5" 200–270 / 500–650; 7" 130–170 / 350–420;
  10" 85–110 / 250–300. Whoops need high log rates; 7"/10" motor band overlaps propwash (dyn notch min to 20 Hz).
- Dyn notch with RPM filter: Q 300–450, 1–2 notches; without RPM: 3–4 notches Q 120–150.
- FF: 250 Hz link → smooth 40–50 2-point; 500 Hz → 65–70; jitter 12–15 for cinematic.
