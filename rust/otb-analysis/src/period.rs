//! Période de vol, découpe « stationnaire puis mouvements » et détection de crash (port de `analysis/period.py`, 4c-1).
//!
//! Les courbes sont en `float32` : les seuils Python (`8.0`, `98.0`, `300.0`, `30.0`…) sont convertis en `float32` avant la
//! comparaison (NEP 50, L-041), pas l'inverse.

use crate::npstat::{max_f32, percentile_f32};
use crate::spectral::py_round;
use otb_log::flightlog::{FlightLog, clamp_range};
use std::ops::Range;

/// Un vol stationnaire ou avec mouvements retenu doit durer au moins 5 s.
pub const MIN_PART_S: f64 = 5.0;
pub use crate::delay::MOVING_SETPOINT_DPS;

/// `(début, fin)` en secondes. `Err` si le journal est vide (Python lève `IndexError`).
pub fn auto_period(log: &FlightLog, takeoff_throttle: f64, margin_s: f64) -> Result<(f64, f64), String> {
    let n = log.n();
    if n == 0 {
        return Err("IndexError : journal vide".into());
    }
    let t = &log.t;
    if n < 10 {
        return Ok((t[0], t[n - 1]));
    }
    let armed = log.armed();
    let thr = takeoff_throttle as f32;
    let flying: Vec<bool> = armed.iter().zip(&log.throttle).map(|(&a, &g)| a && g > thr).collect();
    let Some(i0) = flying.iter().position(|&f| f) else {
        return Ok((t[0], t[n - 1]));
    };
    let i1 = flying.iter().rposition(|&f| f).unwrap();
    let crash = crash_time(log, 300.0, 60.0)?;
    let t0 = t[i0] + margin_s;
    let mut t1 = t[i1] - margin_s;
    if let Some(c) = crash {
        let lim = c - 0.3;
        if lim < t1 {
            t1 = lim;
        }
    }
    if t1 - t0 < 1.0 {
        return Ok((t[0], t[n - 1])); // vol trop court : on garde tout
    }
    Ok((t0, t1))
}

/// Instant du premier crash détecté, ou `None`.
/// `Err` si `min_ms × fs / 1000` n'est pas fini (`int()` de Python lève).
pub fn crash_time(log: &FlightLog, err_dps: f64, min_ms: f64) -> Result<Option<f64>, String> {
    let n = log.n();
    let err = log.pid_error();
    let (e_thr, hi_thr, lo_thr, g_thr) = (err_dps as f32, 98.0f32, 8.0f32, 1900.0f32);
    let km = log.n_motors();
    let mut bad = vec![false; n];
    for (i, b) in bad.iter_mut().enumerate() {
        let e = max_f32(&(0..err.k).map(|a| err.row(a)[i].abs()).collect::<Vec<_>>());
        let (mut sat, mut low) = (false, false);
        for k in 0..km {
            let m = log.motors.row(k)[i];
            sat |= m >= hi_thr;
            low |= m <= lo_thr;
        }
        let g = max_f32(&(0..log.gyro.k).map(|a| log.gyro.row(a)[i].abs()).collect::<Vec<_>>());
        *b = (e.is_some_and(|e| e > e_thr) && sat && low) || g.is_some_and(|g| g > g_thr);
    }
    let wf = min_ms * log.fs / 1000.0;
    if !wf.is_finite() {
        return Err("ValueError : cannot convert float NaN or infinity to integer".into());
    }
    let w = (wf as i64).max(1) as usize;
    if bad.iter().filter(|&&b| b).count() < w {
        return Ok(None);
    }
    // `np.convolve(bad, ones(w), "valid")` : sommes de w valeurs 0 / 1, exactes
    let need = 0.8 * w as f64;
    let mut run: usize = bad[..w].iter().filter(|&&b| b).count();
    let mut k = 0;
    loop {
        if run as f64 >= need {
            return Ok(Some(log.t[k]));
        }
        if k + w >= n {
            return Ok(None);
        }
        run = run + usize::from(bad[k + w]) - usize::from(bad[k]);
        k += 1;
    }
}

/// Parties retenues d'un vol : `(début, fin)` en secondes de la plus longue partie stationnaire (`hover`) et de la plus
/// longue partie avec mouvements (`moving`), ou `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct FlightParts {
    pub hover: Option<(f64, f64)>,
    pub moving: Option<(f64, f64)>,
}

pub fn flight_parts(log: &FlightLog, t0: f64, t1: f64, block_s: f64, moving_dps: f64) -> Result<FlightParts, String> {
    let n = log.n();
    if n == 0 {
        return Err("IndexError : journal vide".into());
    }
    let r = log.slice_time(t0, t1);
    let sel: Range<usize> = clamp_range(r.start, r.end, n);
    let i0 = sel.start;
    let (r0, r1) = (log.setpoint.row(0), log.setpoint.row(1));
    // max(|consigne roll|, |consigne pitch|) : `np.max` propage un NaN
    let sp: Vec<f32> = sel
        .clone()
        .map(|i| {
            let (a, b) = (r0[i].abs(), r1[i].abs());
            if a.is_nan() || b.is_nan() {
                f32::NAN
            } else if b > a {
                b
            } else {
                a
            }
        })
        .collect();
    let l = py_round(block_s * log.fs)?.max(1) as usize;
    let nb = sp.len() / l;
    let mut out = FlightParts {
        hover: None,
        moving: None,
    };
    if nb < 3 {
        return Ok(out);
    }
    let thr = moving_dps as f32;
    let flags: Vec<bool> = (0..nb)
        .map(|b| percentile_f32(&sp[b * l..(b + 1) * l], 95.0) >= thr)
        .collect();
    // lissage par majorité sur 3 blocs (les bords sont prolongés)
    let pad: Vec<i32> = std::iter::once(flags[0])
        .chain(flags.iter().copied())
        .chain(std::iter::once(flags[nb - 1]))
        .map(i32::from)
        .collect();
    let moving: Vec<bool> = (0..nb).map(|k| pad[k] + pad[k + 1] + pad[k + 2] >= 2).collect();
    for (is_moving, slot) in [(false, &mut out.hover), (true, &mut out.moving)] {
        let (mut best, mut start) = ((0usize, 0usize), None::<usize>);
        // la sentinelle `!is_moving` (rang nb) ferme la dernière partie
        for (k, &m) in moving.iter().chain(std::iter::once(&!is_moving)).enumerate() {
            match (m == is_moving, start) {
                (true, None) => start = Some(k),
                (false, Some(s)) => {
                    if k - s > best.1 - best.0 {
                        best = (s, k);
                    }
                    start = None;
                }
                _ => {}
            }
        }
        let (a, b) = best;
        if (b - a) as f64 * block_s >= MIN_PART_S {
            *slot = Some((log.t[i0 + a * l], log.t[(i0 + b * l).min(n) - 1]));
        }
    }
    Ok(out)
}
