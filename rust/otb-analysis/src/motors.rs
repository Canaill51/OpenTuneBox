//! Fréquences moteur, harmoniques, notchs dynamiques et bruit moteur (port de la partie Python de `analysis/motors.py`, phase 4c-3).
//!
//! Les boucles par fenêtre (`rpm_response`, `harmonic_levels`, `masked_mean`, `fold_mean`) sont portées depuis la phase 3b
//! (`lib.rs`) ; ce module en est la « colle » numpy : moyennes par fenêtre, minima et maxima par colonne, dB, médianes, pics.
//!
//! ⚠️ Les fréquences moteur `mhz` sont en `float32` (journaux, D-029) : chaque moyenne, minimum ou maximum reste dans cette
//! précision comme dans numpy (NEP 50, L-041) ; `mhz` en `float64` n'est pas porté (aucun appelant). `mean(axis=0)` d'un tableau
//! `(moteurs, n)` additionne dans l'ordre des moteurs (L-042, L-052).

use crate::bf_filters::rpm_notch_freq_weight;
use crate::npstat::{arange_f64, clip_f32, max_f32, mean_f32, median_f64, min_f32, nanargmax_f64, py_min, std_f64};
use crate::spectral::{Mode, clear_of_breaks, py_round, row_mean, seg_spectra, segments, to_db};
use crate::{fold_mean, harmonic_levels, masked_mean, numpy_mean, rpm_response};
use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::Settings;

/// Marge autour des harmoniques retirées, en part de la fréquence moteur.
pub const MOTOR_LINE_MARGIN: f64 = 0.05;

// ---------------------------------------------------------------- fréquences moteur

/// Fréquence de rotation de chaque moteur (Hz) et indicateur « estimée » : mesurée par la télémétrie eRPM, sinon estimée à partir
/// du KV et de la tension (BF 2025.12+), sinon `multiplicateur × % moteur`.
pub fn motor_hz(log: &FlightLog, multiplier: Option<f64>) -> (Option<Rows>, bool) {
    if let Some(m) = &log.motor_hz {
        return (Some(m.clone()), false);
    }
    if log.n_motors() == 0 {
        return (None, true);
    }
    let kv = log.settings.motor_kv;
    if multiplier.is_none()
        && kv > 0
        && let Some(vbat) = &log.vbat
    {
        // `kv * vbat[None, :] * frac * 0.85 / 60.0`, tout en float32 (nombres Python faibles)
        let mut out = Rows::zeros(log.motors.k, log.motors.n);
        for k in 0..log.motors.k {
            for (i, o) in out.row_mut(k).iter_mut().enumerate() {
                let frac = clip_f32(log.motors.row(k)[i] / 100.0f32, 0.0, 1.0);
                *o = (kv as f32 * vbat[i] * frac * 0.85f32) / 60.0f32;
            }
        }
        return (Some(out), true);
    }
    let m = multiplier.unwrap_or(2.1) as f32;
    let mut out = log.motors.clone();
    out.data.iter_mut().for_each(|v| *v = (m * *v) * 4.0f32);
    (Some(out), true)
}

/// Centres des notchs dynamiques `(count, n)` en Hz, si le journal est en debug FFT_FREQ.
pub fn dyn_notch_centres(log: &FlightLog) -> Option<Vec<Vec<f64>>> {
    let s = &log.settings;
    if s.debug_mode != "FFT_FREQ" || s.dyn_notch_count <= 0 || log.debug.k == 0 {
        return None;
    }
    let count = s.dyn_notch_count as usize;
    let k = log.debug.k;
    let rows: Vec<usize> = if matches!(s.family(), "2025.12" | "2026.6") {
        (1..k.min(1 + count)).collect()
    } else {
        // 4.x : debug[3] est écrasé par le gyro avant notch si count ≥ 4
        (0..k.min(count)).filter(|&p| p != 3).collect()
    };
    if rows.is_empty() {
        return None;
    }
    Some(
        rows.iter()
            .map(|&r| log.debug.row(r).iter().map(|&v| v as f64).collect())
            .collect(),
    )
}

/// Fraction du temps où la fréquence est inférieure à chaque valeur de `freqs` (0 → 1).
pub fn distribution(values: &[f32], freqs: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = values
        .iter()
        .filter(|x| x.is_finite() && **x > 0.0)
        .map(|&x| x as f64)
        .collect();
    if v.is_empty() {
        return vec![0.0; freqs.len()];
    }
    v.sort_by(f64::total_cmp);
    freqs
        .iter()
        .map(|&f| if f.is_nan() { v.len() } else { v.partition_point(|&x| x <= f) } as f64 / v.len() as f64)
        .collect()
}

// ---------------------------------------------------------------- fenêtres et réductions float32

/// Débuts des fenêtres de `length` points tous les `hop`, sans celles qui chevauchent un raccord (`_segments`).
fn window_starts(n: usize, length: usize, hop: usize, breaks: Option<&[i64]>) -> Vec<usize> {
    if n < length {
        return Vec::new();
    }
    let all: Vec<usize> = (0..=n - length).step_by(hop.max(1)).collect();
    let keep = clear_of_breaks(
        &all.iter().map(|&s| s as i64).collect::<Vec<_>>(),
        length as i64,
        breaks,
    );
    all.into_iter().zip(keep).filter(|(_, k)| *k).map(|(s, _)| s).collect()
}

/// `mhz.mean(axis=0)` en float32 : somme dans l'ordre des moteurs, divisée en float64 par leur nombre.
pub fn mean_axis0(m: &Rows) -> Vec<f32> {
    (0..m.n)
        .map(|i| {
            let s = (1..m.k).fold(m.row(0)[i], |s, k| s + m.row(k)[i]);
            (s as f64 / m.k as f64) as f32
        })
        .collect()
}

/// `mhz.min(axis=0)` ou `.max(axis=0)` : un NaN se propage.
fn extreme_axis0(m: &Rows, max: bool) -> Vec<f32> {
    (0..m.n)
        .map(|i| {
            let col: Vec<f32> = (0..m.k).map(|k| m.row(k)[i]).collect();
            (if max { max_f32(&col) } else { min_f32(&col) }).unwrap_or(f32::NAN)
        })
        .collect()
}

/// Réduction par fenêtre (`.mean(axis=1)`, `.min(axis=1)`, `.max(axis=1)`) d'une courbe float32.
#[derive(Clone, Copy)]
enum Reduce {
    Mean,
    Min,
    Max,
}

fn per_window(x: &[f32], length: usize, hop: usize, breaks: Option<&[i64]>, how: Reduce) -> Vec<f32> {
    window_starts(x.len(), length, hop, breaks)
        .into_iter()
        .map(|s| {
            let w = &x[s..s + length];
            match how {
                Reduce::Mean => mean_f32(w),
                Reduce::Min => min_f32(w).unwrap_or(f32::NAN),
                Reduce::Max => max_f32(w).unwrap_or(f32::NAN),
            }
        })
        .collect()
}

fn win_len(fs: f64, seg_ms: f64) -> Result<usize, String> {
    Ok(py_round(fs * seg_ms / 1000.0)?.max(16) as usize)
}

fn check_motors(x: &[f64], mhz: &Rows) -> Result<(), String> {
    if mhz.k == 0 {
        return Err("mhz : au moins un moteur attendu".into());
    }
    if mhz.n != x.len() {
        return Err(format!("mhz : {} points pour {} échantillons", mhz.n, x.len()));
    }
    Ok(())
}

// ---------------------------------------------------------------- bruit moteur

#[derive(Debug, Clone, PartialEq)]
pub struct MotorNoise {
    pub harmonics: Vec<i64>,
    pub mean_db: Vec<f64>,
    pub std_db: Vec<f64>,
}

/// Niveau de bruit (PSD, dB) aux fréquences des harmoniques moteur, fenêtre par fenêtre (moyenne ± écart-type).
pub fn motor_noise(
    x: &[f64],
    mhz: &Rows,
    fs: f64,
    n_harm: i64,
    seg_ms: f64,
    min_hz: f64,
    breaks: Option<&[i64]>,
) -> Result<MotorNoise, String> {
    check_motors(x, mhz)?;
    let l = win_len(fs, seg_ms)?;
    let hop = l / 2;
    let harm: Vec<i64> = (1..=n_harm.max(0)).collect();
    let segs = segments(x, l, hop, breaks);
    if segs.nseg == 0 {
        if n_harm < 0 {
            return Err("ValueError : negative dimensions are not allowed".into());
        }
        let nan = vec![f64::NAN; harm.len()];
        return Ok(MotorNoise {
            harmonics: harm,
            mean_db: nan.clone(),
            std_db: nan,
        });
    }
    let (f, p) = seg_spectra(&segs, fs, Mode::Psd)?;
    let p = to_db(&p);
    let nf = f.len();
    let fm = per_window(&mean_axis0(mhz), l, hop, breaks, Reduce::Mean);
    let df = f[1] - f[0];
    let (mut means, mut stds) = (Vec::new(), Vec::new());
    for &h in &harm {
        let mut vals = Vec::new();
        for (w, &m) in fm.iter().enumerate() {
            let fh = m as f64 * h as f64; // un entier numpy est fort : float32 × int64 → float64
            if fh >= min_hz && fh < f[nf - 1] {
                let idx = (fh / df).round_ties_even() as i64;
                let idx = if idx < 0 { idx + nf as i64 } else { idx };
                if idx < 0 || idx >= nf as i64 {
                    return Err("IndexError : indice de fréquence hors du spectre".into());
                }
                vals.push(p[w * nf + idx as usize]);
            }
        }
        means.push(if vals.is_empty() { f64::NAN } else { numpy_mean(&vals) });
        stds.push(if vals.is_empty() { f64::NAN } else { std_f64(&vals) });
    }
    Ok(MotorNoise {
        harmonics: harm,
        mean_db: means,
        std_db: stds,
    })
}

/// Hauteur (dB) de chaque harmonique moteur au-dessus du bruit voisin, médiane des fenêtres.
pub fn harmonic_prominence(
    x: &[f64],
    mhz: &Rows,
    fs: f64,
    n_harm: i64,
    seg_ms: f64,
    min_hz: f64,
    breaks: Option<&[i64]>,
) -> Result<Vec<f64>, String> {
    if n_harm < 0 {
        return Err("ValueError : negative dimensions are not allowed".into());
    }
    check_motors(x, mhz)?;
    let l = win_len(fs, seg_ms)?;
    let hop = l / 2;
    let nh = n_harm as usize;
    let segs = segments(x, l, hop, breaks);
    if segs.nseg == 0 {
        return Ok(vec![f64::NAN; nh]);
    }
    let (f, p) = seg_spectra(&segs, fs, Mode::Psd)?;
    let lo = per_window(&extreme_axis0(mhz, false), l, hop, breaks, Reduce::Mean);
    let hi = per_window(&extreme_axis0(mhz, true), l, hop, breaks, Reduce::Mean);
    let mean = per_window(&mean_axis0(mhz), l, hop, breaks, Reduce::Mean);
    let lv = harmonic_levels(&f, &p, &lo, &hi, &mean, nh, min_hz, fs)?;
    let nw = lo.len();
    Ok((0..nh)
        .map(|h| {
            let pick = |v: &[f64]| -> Vec<f64> {
                (0..nw)
                    .filter(|&k| lv.used[h * nw + k])
                    .map(|k| v[h * nw + k])
                    .collect()
            };
            let (l_db, f_db) = (to_db(&pick(&lv.line)), to_db(&pick(&lv.floor)));
            let vals: Vec<f64> = l_db.iter().zip(&f_db).map(|(a, b)| a - b).collect();
            if vals.is_empty() { f64::NAN } else { median_f64(&vals) }
        })
        .collect())
}

/// Poids RPM (0–1) juste suffisant pour ramener une raie au niveau du bruit voisin.
pub fn rpm_weight_needed(prominence_db: f64) -> f64 {
    if !prominence_db.is_finite() || prominence_db <= 0.0 {
        return 0.0;
    }
    1.0 - 10f64.powf(-prominence_db / 20.0)
}

// ---------------------------------------------------------------- filtre RPM

/// |H| des notchs RPM du firmware aux fréquences `f`, `(fenêtres, len(f))` à plat ; `motor_hz` : `(moteurs, fenêtres)` en float64.
/// ⚠️ Le module `np.abs` d'un tableau complexe est un `hypot` SIMD chez numpy (L-042) : ici le `hypot` de Rust, à un dernier bit
/// près ; l'écart est mesuré par les tests.
pub fn rpm_attenuation(f: &[f64], motor_hz: &[f64], k: usize, nwin: usize, s: &Settings) -> Result<Vec<f64>, String> {
    if motor_hz.len() != k * nwin {
        return Err("motor_hz : forme (moteurs, fenêtres) attendue".into());
    }
    let (q, rate) = (s.rpm_q as f64 / 100.0, s.pid_rate_hz());
    let nh = s.rpm_harmonics.max(0) as usize;
    if nh == 0 {
        return Err("ValueError : need at least one array to stack".into());
    }
    let fw: Vec<(Vec<f64>, Vec<f64>)> = (0..nh)
        .map(|h| rpm_notch_freq_weight(motor_hz, h as i64, s, rate))
        .collect::<Result<_, _>>()?;
    // (fenêtres, moteurs × harmoniques), moteur puis harmonique : l'ordre d'étages de l'ancien produit
    let ns = k * nh;
    let (mut fk, mut wk) = (vec![0.0; nwin * ns], vec![0.0; nwin * ns]);
    for w in 0..nwin {
        for m in 0..k {
            for (h, (fr, wt)) in fw.iter().enumerate() {
                fk[w * ns + m * nh + h] = fr[m * nwin + w];
                wk[w * ns + m * nh + h] = wt[m * nwin + w];
            }
        }
    }
    let (re, im) = rpm_response(f, &fk, &wk, nwin, ns, q, rate)?;
    Ok(re.iter().zip(&im).map(|(a, b)| a.hypot(*b)).collect())
}

/// DSP moyenne (linéaire) des fenêtres de `seg_ms` de `x`, après le filtre RPM du firmware.
/// Sans `rpm` ou sans filtre RPM : DSP moyenne de `x`. `Ok(([], []))` si `x` est plus court qu'une fenêtre.
pub fn rpm_filtered_spectrum(
    x: &[f64],
    mhz: Option<&Rows>,
    fs: f64,
    rpm: Option<&Settings>,
    seg_ms: f64,
    breaks: Option<&[i64]>,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    let l = win_len(fs, seg_ms)?;
    let hop = l / 2;
    let segs = segments(x, l, hop, breaks);
    if segs.nseg == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let (f, mut p) = seg_spectra(&segs, fs, Mode::Psd)?;
    let nf = f.len();
    if let (Some(m), Some(s)) = (mhz, rpm)
        && s.rpm_harmonics > 0
    {
        check_motors(x, m)?;
        // moyenne de chaque moteur par fenêtre, en float64 (chaque courbe est d'abord convertie)
        let mut mean_m = Vec::new();
        for k in 0..m.k {
            let xs: Vec<f64> = m.row(k).iter().map(|&v| v as f64).collect();
            let sg = segments(&xs, l, hop, breaks);
            mean_m.extend((0..sg.nseg).map(|i| row_mean(sg.row(i))));
        }
        let nwin = segs.nseg;
        // ⚠️ réponse du filtre RPM, pas un masque : un poids réduit laisse un reste que le firmware suit (L-028)
        let att = rpm_attenuation(&f, &mean_m, m.k, nwin, s)?;
        for (v, a) in p.iter_mut().zip(&att) {
            *v *= a * a;
        }
        let lo = per_window(&extreme_axis0(m, false), l, hop, breaks, Reduce::Min);
        let hi = per_window(&extreme_axis0(m, true), l, hop, breaks, Reduce::Max);
        let lobe = 2.0 * fs / l as f64;
        let mean = fold_mean(&f, &p, &lo, &hi, s.rpm_harmonics as usize, lobe, fs)?;
        return Ok((f, mean));
    }
    let zeros = vec![0.0f64; segs.nseg];
    debug_assert_eq!(p.len(), segs.nseg * nf);
    let mean = fold_mean(&f, &p, &zeros, &zeros, 0, 0.0, fs)?;
    Ok((f, mean))
}

// ---------------------------------------------------------------- notch dynamique

#[derive(Debug, Clone, PartialEq)]
pub struct NotchTargets {
    /// Fréquences des pics que le notch dynamique suivrait (Hz, du plus fort au plus faible).
    pub freqs: Vec<f64>,
    /// Hauteur de chaque pic au-dessus du bruit moyen de la plage (dB).
    pub ratios_db: Vec<f64>,
    pub band: (f64, f64),
}

/// Pics que le notch dynamique de Betaflight suivrait dans `band`, d'après le gyro non filtré.
#[allow(clippy::too_many_arguments)]
pub fn dyn_notch_targets(
    x: &[f64],
    mhz: Option<&Rows>,
    fs: f64,
    band: (f64, f64),
    rpm: Option<&Settings>,
    seg_ms: f64,
    bin_hz: f64,
    breaks: Option<&[i64]>,
) -> Result<NotchTargets, String> {
    let (lo, hi) = (band.0, py_min(band.1, 0.95 * fs / 2.0));
    let empty = NotchTargets {
        freqs: Vec::new(),
        ratios_db: Vec::new(),
        band: (lo, hi),
    };
    let (f, s) = rpm_filtered_spectrum(x, mhz, fs, rpm, seg_ms, breaks)?;
    if f.is_empty() || hi <= lo {
        return Ok(empty);
    }
    // regroupement en cases de la largeur de celles de la SDFT
    let edges = arange_f64(lo, hi + bin_hz, bin_hz)?;
    let (mut centres, mut power) = (Vec::new(), Vec::new());
    for w in edges.windows(2) {
        let (a, b) = (w[0], w[1]);
        let v: Vec<f64> = f
            .iter()
            .zip(&s)
            .filter(|(fi, _)| **fi >= a && **fi < b)
            .map(|(_, si)| *si)
            .filter(|si| si.is_finite())
            .collect();
        if !v.is_empty() {
            centres.push(0.5 * (a + b));
            power.push(numpy_mean(&v));
        }
    }
    let (c, p) = (centres, power);
    if p.len() < 5 {
        return Ok(empty);
    }
    let peaks: Vec<usize> = (1..p.len() - 1)
        .filter(|&i| p[i] > p[i - 1] && p[i] >= p[i + 1])
        .collect();
    // bruit moyen hors pics, au-dessus de 60 Hz comme le firmware (DYN_NOTCH_FLOOR_MIN_HZ)
    let mut above: Vec<bool> = c.iter().map(|&v| v >= 60.0).collect();
    if !above.iter().any(|&a| a) {
        above = vec![true; p.len()];
    }
    let mean_of = |keep: &[bool]| {
        numpy_mean(
            &p.iter()
                .zip(keep)
                .filter(|(_, k)| **k)
                .map(|(v, _)| *v)
                .collect::<Vec<_>>(),
        )
    };
    let mut floor = mean_of(&above);
    for _ in 0..3 {
        let mut keep = above.clone();
        for &i in &peaks {
            if p[i] > 2.0 * floor {
                for slot in keep.iter_mut().take(i + 2).skip(i.saturating_sub(1)) {
                    *slot = false;
                }
            }
        }
        if keep.iter().any(|&k| k) {
            floor = mean_of(&keep);
        }
    }
    let mut kept: Vec<usize> = peaks.iter().copied().filter(|&i| p[i] > 2.0 * floor).collect();
    kept.sort_by(|&a, &b| p[b].total_cmp(&p[a])); // stable : à égalité, l'ordre des indices
    Ok(NotchTargets {
        freqs: kept.iter().map(|&i| c[i]).collect(),
        ratios_db: kept.iter().map(|&i| 10.0 * (p[i] / floor).log10()).collect(),
        band: (lo, hi),
    })
}

// ---------------------------------------------------------------- raies (vol stationnaire)

#[derive(Debug, Clone, PartialEq)]
pub struct SpectrumLine {
    pub freq_hz: f64,
    /// Hauteur au-dessus du bruit voisin avant filtres (dB).
    pub raw_db: f64,
    /// Même chose après filtres (NaN si non mesurable).
    pub left_db: f64,
}

/// DSP moyenne (linéaire) des fenêtres de `seg_ms`, bandes des harmoniques moteur 1…n retirées (et leurs images repliées).
fn masked_spectrum(
    x: &[f64],
    mhz: &Rows,
    fs: f64,
    n_harm: usize,
    seg_ms: f64,
    breaks: Option<&[i64]>,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    check_motors(x, mhz)?;
    let l = win_len(fs, seg_ms)?;
    let hop = l / 2;
    let segs = segments(x, l, hop, breaks);
    if segs.nseg == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    let (f, p) = seg_spectra(&segs, fs, Mode::Psd)?;
    let lo = per_window(&extreme_axis0(mhz, false), l, hop, breaks, Reduce::Min);
    let hi = per_window(&extreme_axis0(mhz, true), l, hop, breaks, Reduce::Max);
    let mean = masked_mean(&f, &p, &lo, &hi, n_harm, MOTOR_LINE_MARGIN, fs)?;
    Ok((f, mean))
}

fn near_floor(f: &[f64], s: &[f64], f0: f64) -> Vec<f64> {
    f.iter()
        .zip(s)
        .filter(|(fi, _)| (**fi >= 0.8 * f0 && **fi <= 0.9 * f0) || (**fi >= 1.1 * f0 && **fi <= 1.2 * f0))
        .map(|(_, si)| *si)
        .filter(|v| v.is_finite())
        .collect()
}

/// Hauteur (dB) de la DSP à `f0` (± une case) au-dessus du bruit voisin (10 à 20 % de part et d'autre).
fn line_height(f: &[f64], s: &[f64], f0: f64) -> f64 {
    let df = f[1] - f[0];
    let line: Vec<f64> = f
        .iter()
        .zip(s)
        .filter(|(fi, _)| **fi >= f0 - df && **fi <= f0 + df)
        .map(|(_, si)| *si)
        .filter(|v| v.is_finite())
        .collect();
    let floor = near_floor(f, s, f0);
    if line.is_empty() || floor.len() < 2 || numpy_mean(&floor) <= 0.0 || numpy_mean(&line) <= 0.0 {
        return f64::NAN;
    }
    10.0 * (numpy_mean(&line) / numpy_mean(&floor)).log10()
}

/// Largeur (Hz) d'une raie à mi-puissance de son excès sur le bruit voisin, interpolée entre cases ; NaN si la raie ne dépasse pas
/// le bruit ou touche une bande retirée.
fn half_power_width(f: &[f64], s: &[f64], f0: f64) -> f64 {
    let floor = near_floor(f, s, f0);
    let near: Vec<usize> = (0..f.len()).filter(|&i| (f[i] - f0).abs() <= 10.0).collect();
    if floor.len() < 2 || near.is_empty() || !near.iter().any(|&i| s[i].is_finite()) {
        return f64::NAN;
    }
    let sv: Vec<f64> = near.iter().map(|&i| s[i]).collect();
    let k = near[nanargmax_f64(&sv).unwrap()];
    let fm = numpy_mean(&floor);
    let level = fm + 0.5 * (s[k] - fm);
    if s[k] <= fm || s[k].is_nan() {
        return f64::NAN;
    }
    let mut edges = [0.0f64; 2];
    for (e, step) in edges.iter_mut().zip([-1i64, 1]) {
        let mut j = k as i64;
        loop {
            let nx = j + step;
            if nx >= 0 && (nx as usize) < f.len() && s[nx as usize].is_finite() && s[nx as usize] > level {
                j = nx;
            } else {
                break;
            }
        }
        let n = j + step;
        if !(n >= 0 && (n as usize) < f.len() && s[n as usize].is_finite()) {
            return f64::NAN;
        }
        let (j, n) = (j as usize, n as usize);
        *e = f[j] + (f[n] - f[j]) * (s[j] - level) / (s[j] - s[n]);
    }
    edges[1] - edges[0]
}

/// (largeur à mi-puissance en Hz, hauteur au-dessus du bruit voisin en dB) des raies `freqs` du gyro non filtré, bandes des
/// harmoniques moteur retirées ; avec `rpm`, une raie non mesurable l'est sur le spectre après filtre RPM.
#[allow(clippy::too_many_arguments)]
pub fn line_shapes(
    x_raw: &[f64],
    mhz: &Rows,
    fs: f64,
    freqs: &[f64],
    n_harm: i64,
    seg_ms: f64,
    breaks: Option<&[i64]>,
    rpm: Option<&Settings>,
) -> Result<Vec<(f64, f64)>, String> {
    let (f, s) = masked_spectrum(x_raw, mhz, fs, n_harm.max(1) as usize, seg_ms, breaks)?;
    if f.len() < 8 {
        return Ok(vec![(f64::NAN, f64::NAN); freqs.len()]);
    }
    let mut out: Vec<(f64, f64)> = freqs
        .iter()
        .map(|&f0| (half_power_width(&f, &s, f0), line_height(&f, &s, f0)))
        .collect();
    if let Some(r) = rpm
        && out.iter().any(|(w, _)| !w.is_finite())
    {
        let (fr, sr) = rpm_filtered_spectrum(x_raw, Some(mhz), fs, Some(r), seg_ms, breaks)?;
        for (o, &f0) in out.iter_mut().zip(freqs) {
            if !o.0.is_finite() {
                *o = (half_power_width(&fr, &sr, f0), line_height(&fr, &sr, f0));
            }
        }
    }
    Ok(out)
}

/// Hauteur (dB) au-dessus du bruit voisin des raies `freqs` dans la DSP moyenne de `x`, sans rien retirer.
pub fn line_heights(
    x: &[f64],
    fs: f64,
    freqs: &[f64],
    seg_ms: f64,
    breaks: Option<&[i64]>,
) -> Result<Vec<f64>, String> {
    let (f, s) = rpm_filtered_spectrum(x, None, fs, None, seg_ms, breaks)?;
    if f.len() < 8 {
        return Ok(vec![f64::NAN; freqs.len()]);
    }
    Ok(freqs.iter().map(|&f0| line_height(&f, &s, f0)).collect())
}

/// Raies qui ne viennent pas des moteurs (résonances, bruit résiduel), avant et après filtres, la plus haute d'abord.
#[allow(clippy::too_many_arguments)]
pub fn spectrum_lines(
    x_raw: &[f64],
    x_filt: &[f64],
    mhz: &Rows,
    fs: f64,
    band: (f64, f64),
    min_db: f64,
    n_harm: i64,
    seg_ms: f64,
    breaks: Option<&[i64]>,
) -> Result<Vec<SpectrumLine>, String> {
    let nh = n_harm.max(0) as usize;
    let (f, s) = masked_spectrum(x_raw, mhz, fs, nh, seg_ms, breaks)?;
    if f.len() < 8 {
        return Ok(Vec::new());
    }
    let (_, sf) = masked_spectrum(x_filt, mhz, fs, nh, seg_ms, breaks)?;
    let (lo, hi) = (band.0, py_min(band.1, 0.9 * fs / 2.0 / 1.2));
    let mut cands: Vec<(f64, f64)> = Vec::new();
    for i in 1..f.len() - 1 {
        if lo <= f[i] && f[i] <= hi && s[i].is_finite() {
            // `S[i] >= np.nanmax(S[i-1:i+2])` : les voisins NaN sont ignorés
            let m = s[i - 1..i + 2]
                .iter()
                .copied()
                .filter(|v| !v.is_nan())
                .fold(f64::NEG_INFINITY, f64::max);
            if s[i] >= m {
                let h = line_height(&f, &s, f[i]);
                if h.is_finite() && h >= min_db {
                    cands.push((h, f[i]));
                }
            }
        }
    }
    // `sorted(cands, reverse=True)` : tuples (hauteur, fréquence) du plus grand au plus petit
    cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.total_cmp(&a.1)));
    let mut out: Vec<SpectrumLine> = Vec::new();
    for (h, f0) in cands {
        if out.iter().all(|o| (f0 - o.freq_hz).abs() > 0.1 * o.freq_hz) {
            out.push(SpectrumLine {
                freq_hz: f0,
                raw_db: h,
                left_db: line_height(&f, &sf, f0),
            });
        }
    }
    Ok(out)
}

/// Fréquence moteur typique en vol stationnaire (médiane quand les gaz sont stables), pour reconnaître la taille du drone.
pub fn hover_motor_hz(log: &FlightLog) -> Option<f64> {
    let (mhz, estimated) = motor_hz(log, None);
    let mhz = mhz?;
    if estimated {
        return None;
    }
    let m = mean_axis0(&mhz);
    let armed = log.armed();
    let flying: Vec<f32> = (0..m.len())
        .filter(|&i| armed[i] && log.throttle[i] > 5.0f32 && m[i] > 10.0f32)
        .map(|i| m[i])
        .collect();
    if flying.len() < 100 {
        return None;
    }
    Some(crate::npstat::median_f32(&flying) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L-055 : sur le spectre EXACT de Python (plat à 1e-22 près, `S[k]` dépasse la moyenne du bruit d'un ulp, `level == S[k]`), la largeur
    /// d'une raie inexistante vaut 0,0 comme en Python : le port est correct, l'écart vu sur le corpus vient du bruit de la FFT.
    #[test]
    fn largeur_sur_spectre_plat_comme_python() {
        let text =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/flat_spectrum.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let f: Vec<f64> = v["f"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        let s: Vec<f64> = v["S"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap_or(f64::NAN))
            .collect();
        assert!(
            s.iter().any(|x| x.is_nan()),
            "le spectre contient des bandes retirées (NaN)"
        );
        let w = half_power_width(&f, &s, v["f0"].as_f64().unwrap());
        assert_eq!(w.to_bits(), v["want"].as_f64().unwrap().to_bits(), "largeur {w:e}");
        assert_eq!(w, 0.0);
        // un ulp de moins sur le sommet (le bruit de la FFT) suffit à changer la décision : la largeur n'est pas continue
        let mut t = s.clone();
        let k = 125;
        t[k] = f64::from_bits(t[k].to_bits() - 1);
        assert_ne!(
            half_power_width(&f, &t, v["f0"].as_f64().unwrap()).to_bits(),
            w.to_bits()
        );
    }
}
