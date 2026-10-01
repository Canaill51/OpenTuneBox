# PIDtoolbox Pro V0.94 screens (from the user's 638 exported screenshots)

Paths relative to `~/Documents/Drones/Logs/`. Exports are full-window screenshots `PTB_FIGS_YYYY-MM-DD-N.png`.
Visible screens: Log Viewer, Spectral Analyzer (2 right-pane modes), Freq x Throttle, Freq x Time,
Step Response (incl. BF sliders view). PID slider tool, Filter Sim, Setup Info windows never exported
(buttons only) — designed by us. PTB module names (encrypted binaries) also list: PTplotFlightStats,
PTdispSetupInfo, PTplotFilterSim, PTpidSliderTool, PTcalcDelays, PTgetBandwidth, PTB_AIAssistant_*.

Conventions: grey background, white axes with grid; multi-file colours red, orange, yellow, light green,
cyan, blue, dark blue… (max 10 files); motor colours M1/5 red, M2/6 orange, M3/7 blue, M4/8 cyan.
Every tool: Run, Reset, Save Fig, Save Settings, Markup, Quick Guide, "select files (max 10)".
`PTBdefaults.txt` holds persisted UI defaults (LogViewer Ymax 500, line width 3, spec2D smoothing 3,
FreqXthr presets/colormap/smoothing, FreqxTime smoothing, StepResp Ymax 1.5, theme, font…).

## 1. Log Viewer
- Top time scrollbar; top-right firmware + board. 3 stacked axes R/P/Y (deg/s, ±yscale default 500),
  4th axis "Throttle | Motor (%)" 0–100. Flight-mode labels under time axis ("|ARM|ANGLE").
- Grey shading outside the analysis Period; dashed vertical player cursor.
- Selecting R/P/Y radio expands that axis full height (throttle panel kept).
- Selection panel: y scale; checkboxes Gyro(pf), Gyro, P-term, I-term, D-term(pf), D-term, F-term,
  Set point, PID sum, PID error, Motor 1/5..4/8, RPS 1/5..4/8, Throttle, Debug; debug mode text.
  Colours: gyro black, setpoint red, P green, I blue, D(pf) olive, D yellow, F brown-red, PID sum purple,
  PID error cyan. Defaults: Gyro, Set point, Motors, Throttle.
- Control panel: Open / Restore / Reset, file dropdown, R/P/Y radios, Period, Markup, line colour, font,
  line smoothing, line width, theme, buttons Spectral Analyzer, Step Resp Tool, PID slider tool, Filter Sim,
  Setup Info, Save Fig, Save Settings, website, Quick Guide.
- Player: << < - > >>, "time: x sec", two stick widgets (T/Y left, P/R right), motor % and gyro readouts at cursor.
- Ref: `1. PTB/Allykill/PTB_FIGS_2026-09-28/PTB_FIGS_2026-09-28-1.png`, `Cedric/BF Chirp/PTB_FIGS_2026-08-09/PTB_FIGS_2026-08-09-12.png`.

## 2. Spectral Analyzer
- 3×2 grid rows R/P/Y. Left "Full Spectrum" 0–Nyquist, PSD dB −50..20; legend "<dataset> | <file>".
- Right pane: "Sub 100Hz" (0–100 Hz, red text "Gyro: 1.25ms | Dterm: 1ms" filter delays) or
  "Motor Noise" (x = 1st/2nd/3rd harmonic, mean ± error bars, solid filtered / dotted prefiltered,
  text "Filter Weights | Q" e.g. "100,40,75% | Q800").
- Filtered solid thick, prefiltered dotted same colour. Overlays as CDF-like S-curves rising to 0 dB:
  motor harmonics per motor (1st solid, 2nd dashed, 3rd dotted), dyn notch distribution, RPM notch range.
- Controls: file list ×10; datasets Gyro, Gyro prefilt, Dterm, Dterm prefilt, Pterm, PID error, Set point,
  Fterm, PIDsum, Motors, debug; Run, Reset; Freq x Throttle, Freq x Time buttons; filter delay & smoothing
  dropdowns; R/P/Y; PSD; Y min/max; motor toggles; harmonic dropdown (1st,2nd,3rd / RPM notch);
  Dyn notch dropdown; line width; right-pane mode; RPM est. + multiplier 2.1.
- Ref: `1. PTB/Allykill/PTB_FIGS_2026-09-28/PTB_FIGS_2026-09-28-3.png` (sub100), `-6.png` (motor noise).

## 3. Freq x Throttle
- Up to 4 columns each with dataset + file dropdowns and "<100Hz" toggle; rows R/P/Y.
  Heatmap x "% Throttle" 0–100, y Freq 0–Nyquist, colormap hot, horizontal colorbar 0..scale (0.5),
  white text "mean=…", "peak=…".
- Params: Run, Reset, file, Save, Presets, Markup, colormap, smoothing, "2D"/"PSD" radios,
  freq lims 20 / 60 Hz, Quick Guide.
- Ref: `1. PTB/schteven/PTB_FIGS_2026-07-27/PTB_FIGS_2026-07-27-3.png`.

## 4. Freq x Time
- 3 spectrograms R/P/Y, x time s, y freq, colormap hot, colorbar "Power Spectral density (dB)" Zmin −30 / Zmax 10.
- Overlays: motor harmonic traces per motor colour (1st solid, 2nd dashed, 3rd dotted), dyn notch centres (red).
- Controls: file, dataset, Run/Reset, smooth freq, smooth time, colormap, line width, R/P/Y, <100Hz,
  Z min/max, motor toggles, harmonic dropdown, dyn notches, RPM est. + multiplier, Player/Traces radios, < - >.
- Ref: `Cedric/JeNo 5/PTB_FIGS_2026-07-27/PTB_FIGS_2026-07-27-8.png`.

## 5. Step Response
- Left 3 rows "Step Response Functions": y 0..Ymax (1.5) dashed line at 1, x 0–500 ms; mean curve per file;
  "Raw" shows all segments.
- Text per row: header "P, I, D, Dm, F", per file "1) 45,15,17,17,0 (n=391)" or "insufficient data".
- Two metric columns (dropdowns) vs test index with error bars: SR Peak, SR Latency, SP-Gyro Delay,
  Sensitivity Peak (dB; lines 4 & 6 dB), Sensitivity Peak Freq, Phase Margin, Gain Crossover Freq,
  Rise Time, Bandwidth.
- "BF sliders" view: categories Damping, Tracking, Stick Resp., Dynamic D., Drift-Wobble, Pitch D., Pitch T.,
  Master; x Slider Position 0–2; dot per file; "MODE: RPY".
- Controls: files ×10, Run/Reset, Save, Period, Markup, smoothing, metric dropdowns, R/P/Y, Expand, Raw,
  BF sliders, Y corr., Y max, "2sec deconv. window" dropdown.
- Values seen: latency 13–22 ms; SP-gyro delay 12–21 ms; peak 1.0–1.22; PM ~66°; crossover ~11 Hz;
  rise 15–22 ms; bandwidth 14–20 Hz.
- Ref: `1. PTB/BlackSummer/PTB_FIGS_2026-08-16/PTB_FIGS_2026-08-16-1.png`, `-4.png` (sliders),
  `Cedric/JeNo 5/PTB_FIGS_2026-08-09/PTB_FIGS_2026-08-09-2.png`..`-4.png`.

## Setup Info reference
Users' Blackbox Explorer header screenshots show the groups tuners check: PIDs, rates, filters
(LPF cutoffs, dyn notch Q/min/max, RPM Q/min), RC smoothing, motors, debug mode, sliders.
