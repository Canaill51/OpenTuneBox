//! Retards par intercorrélation et par phase (port de `analysis/delay.py`, phase 4c-2).
//!
//! `xcorr_delay_ms` : intercorrélation par FFT (`otb_dsp::numeric::xcorr_full`, 4b-1), maximum dans ±`max_ms`, interpolation
//! parabolique. `phase_delay_ms` : fonction de transfert de Welch (fenêtres de 0,5 s), retard = −phase / (2π f) moyenné
//! pondéré par la cohérence. Les produits complexes reproduisent numpy (FMA, division de Smith : L-051).
//!
//! ⚠️ Le paramètre `smooth` de `xcorr_delay_ms` n'est pas porté : aucun appelant de l'application ne le fournit
//! (`np.convolve` passe par BLAS) ; une valeur > 1 rend une erreur (W-024).

use crate::npstat::{argmax_f64, cdiv, conj_mul_fma, percentile_f32, std_f32, unwrap};
use crate::numpy_mean;
use crate::spectral::{py_round, rfftfreq, row_mean, segments};
use otb_dsp::numeric::{hann, rfft_rows, xcorr_full};
use otb_dsp::pairwise_sum;
use otb_log::flightlog::{FlightLog, clamp_range};
use std::f64::consts::PI;
use std::ops::Range;

/// La consigne dépasse 30 °/s au moins 5 % du temps en roll ou en pitch (D-026, L-007).
pub const MOVING_SETPOINT_DPS: f64 = 30.0;

/// Retard (ms) de `y` par rapport à `x` (positif si `y` est en retard). NaN si moins de 32 points ou un signal constant.
pub fn xcorr_delay_ms(x: &[f64], y: &[f64], fs: f64, max_ms: f64, smooth: i64) -> Result<f64, String> {
    if smooth > 1 {
        return Err("xcorr_delay_ms : smooth > 1 non porté (W-024)".into());
    }
    let n = x.len().min(y.len());
    if n < 32 {
        return Ok(f64::NAN);
    }
    let center = |s: &[f64]| {
        let m = numpy_mean(s);
        s.iter().map(|v| v - m).collect::<Vec<f64>>()
    };
    let (x, y) = (center(&x[..n]), center(&y[..n]));
    if !x.iter().any(|&v| v != 0.0) || !y.iter().any(|&v| v != 0.0) {
        return Ok(f64::NAN);
    }
    let max_lag = py_round(max_ms * fs / 1000.0)?.max(1);
    let (c, lags) = xcorr_full(&y, &x)?;
    let (cw, lw): (Vec<f64>, Vec<i64>) = c
        .iter()
        .zip(&lags)
        .filter(|&(_, &l)| (-max_lag..=max_lag).contains(&l))
        .map(|(&v, &l)| (v, l))
        .unzip();
    let i = argmax_f64(&cw).ok_or("fenêtre de retards vide")?;
    let mut shift = lw[i] as f64;
    let mut interpolated = false; // `shift` devient un `np.float64` : une division par 0 rend ±inf au lieu de lever
    if 0 < i && i < cw.len() - 1 {
        let (a, b, d) = (cw[i - 1], cw[i], cw[i + 1]);
        let den = a - 2.0 * b + d;
        if den != 0.0 {
            shift += 0.5 * (a - d) / den;
            interpolated = true;
        }
    }
    if fs == 0.0 && !interpolated {
        return Err("ZeroDivisionError : float division by zero".into());
    }
    Ok(shift * 1000.0 / fs)
}

/// Retard (ms) de `y` par rapport à `x` déduit de la phase de la fonction de transfert H = Sxy / Sxx.
/// `breaks` : raccords du journal, aucune fenêtre ne les chevauche (D-020).
pub fn phase_delay_ms(
    x: &[f64],
    y: &[f64],
    fs: f64,
    band: (f64, f64),
    min_coherence: f64,
    breaks: Option<&[i64]>,
) -> Result<f64, String> {
    let n = x.len().min(y.len());
    // `int(min(n, 2 ** int(np.ceil(np.log2(fs * 0.5)))))`
    let lg = (fs * 0.5).log2().ceil();
    if !lg.is_finite() {
        return Err("phase_delay_ms : fs illisible".into());
    }
    let k = lg as i64;
    let pow = if k < 0 {
        0
    } else if k >= 62 {
        i64::MAX
    } else {
        1i64 << k
    };
    let nper = (n as i64).min(pow);
    if nper < 64 {
        return Ok(f64::NAN);
    }
    let nper = nper as usize;
    let hop = nper / 2;
    let (sx, sy) = (
        segments(&x[..n], nper, hop, breaks),
        segments(&y[..n], nper, hop, breaks),
    );
    if sx.nseg == 0 {
        return Ok(f64::NAN);
    }
    let w = hann(nper as i64, false);
    let windowed = |s: &crate::spectral::Segments| {
        let mut out = Vec::with_capacity(s.nseg * nper);
        for i in 0..s.nseg {
            let row = s.row(i);
            let mean = row_mean(row);
            out.extend(row.iter().zip(&w).map(|(v, wk)| (v - mean) * wk));
        }
        out
    };
    let (xs, ys) = (
        rfft_rows(&windowed(&sx), sx.nseg, nper),
        rfft_rows(&windowed(&sy), sy.nseg, nper),
    );
    let nf = nper / 2 + 1;
    let f = rfftfreq(nper, fs);
    let rows = sx.nseg;
    // sommes sur l'axe des fenêtres, dans l'ordre (`np.mean(axis=0)`), puis division par leur nombre
    let (mut sxy, mut sxx, mut syy) = (vec![(0.0, 0.0); nf], vec![0.0; nf], vec![0.0; nf]);
    for r in 0..rows {
        for j in 0..nf {
            let (a, b) = (xs[r * nf + j], ys[r * nf + j]);
            let p = conj_mul_fma((a.re, a.im), (b.re, b.im));
            let (ha, hb) = (a.re.hypot(a.im), b.re.hypot(b.im));
            if r == 0 {
                sxy[j] = p;
                sxx[j] = ha * ha;
                syy[j] = hb * hb;
            } else {
                sxy[j] = (sxy[j].0 + p.0, sxy[j].1 + p.1);
                sxx[j] += ha * ha;
                syy[j] += hb * hb;
            }
        }
    }
    let cnt = rows as f64;
    for j in 0..nf {
        sxy[j] = cdiv(sxy[j], (cnt, 0.0));
        sxx[j] /= cnt;
        syy[j] /= cnt;
    }
    let coh: Vec<f64> = (0..nf)
        .map(|j| {
            if sxx[j] * syy[j] > 0.0 {
                let a = sxy[j].0.hypot(sxy[j].1);
                a * a / (sxx[j] * syy[j])
            } else {
                0.0
            }
        })
        .collect();
    let sel: Vec<usize> = (0..nf)
        .filter(|&j| f[j] >= band.0 && f[j] <= band.1 && coh[j] >= min_coherence && sxx[j] > 0.0)
        .collect();
    if sel.len() < 3 {
        return Ok(f64::NAN);
    }
    let ang: Vec<f64> = sel
        .iter()
        .map(|&j| {
            let h = cdiv(sxy[j], (sxx[j], 0.0));
            h.1.atan2(h.0)
        })
        .collect();
    let ph = unwrap(&ang);
    let d: Vec<f64> = sel.iter().zip(&ph).map(|(&j, p)| -p / (2.0 * PI * f[j])).collect();
    let wgt: Vec<f64> = sel.iter().map(|&j| coh[j]).collect();
    let den = 0.0 + pairwise_sum(&wgt);
    if den == 0.0 {
        return Err("ZeroDivisionError : Weights sum to zero, can't be normalized".into());
    }
    let num = 0.0 + pairwise_sum(&d.iter().zip(&wgt).map(|(a, b)| a * b).collect::<Vec<_>>());
    Ok(num / den * 1000.0)
}

/// 95e centile de |consigne| (°/s), maximum du roll et du pitch : ampleur des mouvements de manche.
pub fn stick_p95_dps(log: &FlightLog, sel: Range<usize>) -> Result<f64, String> {
    let sel = clamp_range(sel.start, sel.end, log.n());
    if sel.is_empty() {
        return Err("IndexError : index -1 is out of bounds for axis 0 with size 0".into());
    }
    let p = |a: usize| {
        let v: Vec<f32> = log.setpoint.row(a)[sel.clone()].iter().map(|v| v.abs()).collect();
        percentile_f32(&v, 95.0)
    };
    // `max` de Python : le premier reste sauf si le suivant est strictement plus grand (un NaN ne remplace rien)
    let (p0, p1) = (p(0), p(1));
    Ok(if p1 > p0 { p1 } else { p0 } as f64)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Delays {
    /// Filtres gyro (gyro non filtré → gyro filtré).
    pub gyro_ms: f64,
    /// Filtres du D (D préfiltre → D).
    pub dterm_ms: f64,
    /// Consigne → gyro, par axe (latence de la boucle).
    pub sp_gyro_ms: [f64; 3],
    pub stick_p95_dps: f64,
}

impl Delays {
    /// Assez de mouvements pour que les retards des filtres soient représentatifs.
    pub fn moving(&self) -> bool {
        self.stick_p95_dps >= MOVING_SETPOINT_DPS
    }
}

/// `np.nanmean` de deux valeurs, NaN si aucune n'est finie (`np.any(np.isfinite(vals))` faux).
fn nanmean_if_finite(v: [f64; 2]) -> f64 {
    if !v.iter().any(|x| x.is_finite()) {
        return f64::NAN;
    }
    let cnt = v.iter().filter(|x| !x.is_nan()).count() as f64;
    let z = |x: f64| if x.is_nan() { 0.0 } else { x };
    (0.0 + ((-0.0 + z(v[0])) + z(v[1]))) / cnt
}

fn wide(r: &[f32]) -> Vec<f64> {
    r.iter().map(|&v| v as f64).collect()
}

/// Retards des filtres (méthode de phase, roll + pitch) et latence consigne → gyro (intercorrélation).
pub fn compute_delays(log: &FlightLog, sel: Range<usize>) -> Result<Delays, String> {
    let n = log.n();
    let sel = clamp_range(sel.start, sel.end, n);
    let fs = log.fs;
    let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let phase = |a: &[f32], b: &[f32]| phase_delay_ms(&wide(a), &wide(b), fs, (10.0, 100.0), 0.6, Some(&br));
    let mut gyro_ms = f64::NAN;
    if let Some(gu) = &log.gyro_unfilt {
        let v = [
            phase(&gu.row(0)[sel.clone()], &log.gyro.row(0)[sel.clone()])?,
            phase(&gu.row(1)[sel.clone()], &log.gyro.row(1)[sel.clone()])?,
        ];
        gyro_ms = nanmean_if_finite(v);
    }
    let mut dterm_ms = f64::NAN;
    if log.d_term.row(0).iter().any(|&v| v != 0.0) {
        let pre = log.d_term_prefilter()?;
        let v = [
            phase(&pre.row(0)[sel.clone()], &log.d_term.row(0)[sel.clone()])?,
            phase(&pre.row(1)[sel.clone()], &log.d_term.row(1)[sel.clone()])?,
        ];
        dterm_ms = nanmean_if_finite(v);
    }
    let mut sp = [f64::NAN; 3];
    for (a, slot) in sp.iter_mut().enumerate() {
        let x = &log.setpoint.row(a)[sel.clone()];
        // `np.std(x) < 5.0` : pas assez de mouvements de manche sur cet axe (faux pour un NaN : le retard est calculé)
        let sd = std_f32(x);
        if sd >= 5.0f32 || sd.is_nan() {
            *slot = xcorr_delay_ms(&wide(x), &wide(&log.gyro.row(a)[sel.clone()]), fs, 100.0, 0)?;
        }
    }
    Ok(Delays {
        gyro_ms,
        dterm_ms,
        sp_gyro_ms: sp,
        stick_p95_dps: stick_p95_dps(log, sel)?,
    })
}
