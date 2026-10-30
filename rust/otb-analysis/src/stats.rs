//! Statistiques de vol et erreur PID (port de `analysis/stats.py`, phase 4c-1).
//!
//! Tout se calcule en `float32` comme numpy sur les courbes du journal (voir [`crate::npstat`]) : moyennes, écarts-types,
//! médianes, rapports de l'imbalance moteur ; seuls les histogrammes (bornes en `float64`), les pourcentages et les
//! milliampères-heure passent en `float64`.

use crate::npstat::{clip_f32, digitize, histogram_counts, linspace, max_f32, mean_f32, median_f32, min_f32, std_f32};
use otb_dsp::pairwise_sum;
use otb_log::flightlog::{FlightLog, Rows, clamp_range};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub struct MeanSd {
    pub mean: f64,
    pub sd: f64,
    pub max: f64,
}

/// `_ms` : moyenne, écart-type et maximum (NaN partout si vide).
fn ms(x: &[f32]) -> MeanSd {
    match max_f32(x) {
        None => MeanSd {
            mean: f64::NAN,
            sd: f64::NAN,
            max: f64::NAN,
        },
        Some(m) => MeanSd {
            mean: mean_f32(x) as f64,
            sd: std_f32(x) as f64,
            max: m as f64,
        },
    }
}

fn abs_row(r: &[f32]) -> Vec<f32> {
    r.iter().map(|v| v.abs()).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlightStats {
    pub duration_s: f64,
    /// `(nom, centres en %, % du vol)` : Roll, Pitch, Yaw, Gaz.
    pub stick_hist: Vec<(&'static str, Vec<f64>, Vec<f64>)>,
    /// `gyro`, `P`, `I`, `D`, `F` → par axe, valeurs absolues.
    pub terms: Vec<(&'static str, Vec<MeanSd>)>,
    pub motors: Vec<MeanSd>,
    pub motor_saturation_pct: f64,
    pub motor_imbalance_pct: f64,
    pub throttle: MeanSd,
    pub vbat: Option<MeanSd>,
    pub vbat_min: Option<f64>,
    pub current: Option<MeanSd>,
    pub mah: Option<f64>,
}

fn centres(edges: &[f64]) -> Vec<f64> {
    edges.windows(2).map(|w| 0.5 * (w[0] + w[1])).collect()
}

/// `100.0 * h / max(1, h.sum())`
fn percent(h: &[i64]) -> Vec<f64> {
    let total = h.iter().sum::<i64>().max(1) as f64;
    h.iter().map(|&c| 100.0 * c as f64 / total).collect()
}

pub fn flight_stats(log: &FlightLog, sel: Range<usize>) -> FlightStats {
    let n = log.n();
    let sel = clamp_range(sel.start, sel.end, n);
    let rc = &log.rc_command;
    let thr = &log.throttle[sel.clone()];
    let mut hist = Vec::new();
    let edges = linspace(-100.0, 100.0, 41);
    for (i, name) in ["Roll", "Pitch", "Yaw"].into_iter().enumerate() {
        let h = histogram_counts(
            rc.row(i)[sel.clone()]
                .iter()
                .map(|&v| clip_f32(v / 5.0f32, -100.0, 100.0) as f64),
            &edges,
        );
        hist.push((name, centres(&edges), percent(&h)));
    }
    let te = linspace(0.0, 100.0, 41);
    let h = histogram_counts(thr.iter().map(|&v| clip_f32(v, 0.0, 100.0) as f64), &te);
    hist.push(("Gaz", centres(&te), percent(&h)));

    let term = |r: &Rows| (0..3).map(|a| ms(&abs_row(&r.row(a)[sel.clone()]))).collect::<Vec<_>>();
    let terms = vec![
        ("gyro", term(&log.gyro)),
        ("P", term(&log.p_term)),
        ("I", term(&log.i_term)),
        ("D", term(&log.d_term)),
        ("F", term(&log.f_term)),
    ];
    let km = log.n_motors();
    let len = sel.len();
    let motors: Vec<MeanSd> = (0..km).map(|k| ms(&log.motors.row(k)[sel.clone()])).collect();
    let (sat, imb) = if km * len > 0 {
        let hot = (sel.clone())
            .filter(|&i| (0..km).any(|k| log.motors.row(k)[i] >= 98.0f32))
            .count();
        let sat = 100.0 * (hot as f64 / len as f64);
        let armed = log.armed();
        let flying: Vec<usize> = sel.clone().filter(|&i| armed[i] && log.throttle[i] > 5.0f32).collect();
        let imb = if flying.len() > 10 {
            // ⚠️ `m[:, flying]` (indexation avancée sur le 2e axe) est rangé « Fortran » : `mean(axis=1)` y additionne
            // séquentiellement, pas par paires (L-052)
            let means: Vec<f32> = (0..km)
                .map(|k| {
                    let row = log.motors.row(k);
                    let mut it = flying.iter().map(|&i| row[i]);
                    let first = it.next().unwrap();
                    (it.fold(first, |s, v| s + v) as f64 / flying.len() as f64) as f32
                })
                .collect();
            let (mx, mm) = (max_f32(&means).unwrap(), mean_f32(&means));
            let floor = 1e-6f32;
            let denom = if mm > floor { mm } else { floor };
            (100.0f32 * (mx - mm) / denom) as f64
        } else {
            f64::NAN
        };
        (sat, imb)
    } else {
        (f64::NAN, f64::NAN)
    };

    let (mut vbat, mut vbat_min) = (None, None);
    if let Some(vb) = &log.vbat {
        let v = &vb[sel.clone()];
        if v.iter().any(|&x| x > 0.0) {
            let pos: Vec<f32> = v.iter().copied().filter(|&x| x > 0.0).collect();
            vbat = Some(ms(&pos));
            vbat_min = min_f32(&pos).map(|m| m as f64);
        }
    }
    let (mut current, mut mah) = (None, None);
    if let Some(cu) = &log.current {
        let c = &cu[sel.clone()];
        if c.iter().any(|&x| x != 0.0) {
            current = Some(ms(c));
            let t = &log.t[sel.clone()];
            // `np.diff(t, prepend=t[0])` : le premier pas est nul
            let prod: Vec<f64> = c
                .iter()
                .enumerate()
                .map(|(i, &x)| x as f64 * (t[i] - t[i.saturating_sub(1)]))
                .collect();
            mah = Some((0.0 + pairwise_sum(&prod)) / 3.6);
        }
    }
    let t = &log.t[sel.clone()];
    FlightStats {
        duration_s: if t.is_empty() { 0.0 } else { t[t.len() - 1] - t[0] },
        stick_hist: hist,
        terms,
        motors,
        motor_saturation_pct: sat,
        motor_imbalance_pct: imb,
        throttle: ms(thr),
        vbat,
        vbat_min,
        current,
        mah,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PidErrorStats {
    pub edges: Vec<f64>,
    pub hist: Vec<Vec<f64>>,
    pub sd: Vec<f64>,
    pub mean_abs: Vec<f64>,
    pub stick_bins: Vec<f64>,
    pub err_vs_stick: Vec<Vec<f64>>,
}

pub fn pid_error_stats(log: &FlightLog, sel: Range<usize>, max_setpoint: f64, err_range: f64) -> PidErrorStats {
    let n = log.n();
    let sel = clamp_range(sel.start, sel.end, n);
    let armed = log.armed();
    let flying: Vec<usize> = sel.filter(|&i| armed[i] && log.throttle[i] > 2.0f32).collect();
    let pe = log.pid_error();
    let col = |r: &Rows, a: usize| flying.iter().map(|&i| r.row(a)[i]).collect::<Vec<f32>>();
    let err: Vec<Vec<f32>> = (0..3).map(|a| col(&pe, a)).collect();
    let sp: Vec<Vec<f32>> = (0..3).map(|a| col(&log.setpoint, a)).collect();
    let rc: Vec<Vec<f32>> = (0..3)
        .map(|a| col(&log.rc_command, a).iter().map(|v| v.abs() / 5.0f32).collect())
        .collect();
    let edges = linspace(-err_range, err_range, 101);
    let (mut hist, mut sd, mut ma) = (Vec::new(), Vec::new(), Vec::new());
    let cap = max_setpoint as f32;
    for a in 0..3 {
        let e: Vec<f32> = err[a]
            .iter()
            .zip(&sp[a])
            .filter(|(_, s)| s.abs() <= cap)
            .map(|(&v, _)| v)
            .collect();
        let h = histogram_counts(e.iter().map(|&v| v as f64), &edges);
        let total = e.len().max(1) as f64;
        hist.push(h.iter().map(|&c| 100.0 * c as f64 / total).collect());
        sd.push(if e.is_empty() {
            f64::NAN
        } else {
            let med = median_f32(&e);
            let dev: Vec<f32> = e.iter().map(|&v| (v - med).abs()).collect();
            (1.4826f32 * median_f32(&dev)) as f64
        });
        ma.push(if e.is_empty() {
            f64::NAN
        } else {
            mean_f32(&abs_row(&e)) as f64
        });
    }
    let sb = linspace(0.0, 100.0, 11);
    let mut evs = Vec::new();
    for a in 0..3 {
        let k: Vec<usize> = rc[a]
            .iter()
            .map(|&v| digitize(v as f64, &sb).saturating_sub(1).min(9))
            .collect();
        let mut vals = vec![f64::NAN; 10];
        for (i, slot) in vals.iter_mut().enumerate() {
            let picked: Vec<f32> = err[a]
                .iter()
                .zip(&k)
                .filter(|&(_, &kk)| kk == i)
                .map(|(&v, _)| v.abs())
                .collect();
            if picked.len() > 20 {
                *slot = mean_f32(&picked) as f64;
            }
        }
        evs.push(vals);
    }
    PidErrorStats {
        edges,
        hist,
        sd,
        mean_abs: ma,
        stick_bins: centres(&sb),
        err_vs_stick: evs,
    }
}
