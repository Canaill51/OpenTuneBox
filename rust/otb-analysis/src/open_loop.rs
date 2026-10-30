//! Boucle ouverte (port de `analysis/loop.py`, phase 4c-B) : marges de stabilité, méthode approchée, modèle du terme D.
//!
//! Étape (a) : `margins`, `loop_from_t` (T = consigne → gyro, L = T / (1 − T)) et `dterm_model`. Les nombres complexes sont des
//! couples `(re, im)` ; la division reproduit celle de numpy (Smith, `npstat::cdiv`), le module est `hypot` (L-042).
//!
//! Décisions discrètes (indice du croisement, de la phase à −180°, du pic de sensibilité, bord de bande, nombre de points
//! valides) : à exiger **identiques** à Python ; valeurs continues (logs, atan2, exp) à tolérance mesurée (L-048, L-051).

use crate::npstat::{argmax_f64, cdiv, unwrap};
use otb_log::settings::Settings;
use std::f64::consts::PI;

pub type C = (f64, f64);

pub const MEASURED: &str = "mesurée";
pub const APPROX: &str = "approchée";
/// Bande de la mesure (Hz), plafonnée à 0,4 × fréquence du log.
pub const F_MIN: f64 = 2.0;
/// Lissage des spectres (points), méthode approchée.
pub const SMOOTH: i64 = 5;
/// flight/pid.h : Kd = DTERM_SCALE × D.
pub const DTERM_SCALE: f64 = 0.000529;

fn abs(z: C) -> f64 {
    z.0.hypot(z.1)
}

/// `Margins` : les NaN de Python sont des NaN ici.
#[derive(Debug, Clone, Copy)]
pub struct Margins {
    pub crossover_hz: f64,
    pub phase_margin_deg: f64,
    pub gain_margin_db: f64,
    pub phase_crossover_hz: f64,
    pub sensitivity_peak_db: f64,
    pub sensitivity_peak_hz: f64,
    pub sensitivity_peak_at_edge: bool,
}

impl Default for Margins {
    fn default() -> Self {
        Margins {
            crossover_hz: f64::NAN,
            phase_margin_deg: f64::NAN,
            gain_margin_db: f64::NAN,
            phase_crossover_hz: f64::NAN,
            sensitivity_peak_db: f64::NAN,
            sensitivity_peak_hz: f64::NAN,
            sensitivity_peak_at_edge: false,
        }
    }
}

/// Passage de `g` par zéro entre les points `i` et `i + 1` : (log f, log |L|, phase) interpolés linéairement.
fn interp(logf: &[f64], logmag: &[f64], phase: &[f64], g: &[f64], i: usize) -> (f64, f64, f64) {
    let t = if g[i] != g[i + 1] {
        g[i] / (g[i] - g[i + 1])
    } else {
        0.0
    };
    (
        logf[i] + t * (logf[i + 1] - logf[i]),
        logmag[i] + t * (logmag[i + 1] - logmag[i]),
        phase[i] + t * (phase[i + 1] - phase[i]),
    )
}

/// `max(a, b)` de Python : `a` sauf si `b > a`.
fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// `margins` : marges de phase et de gain, et pic de sensibilité, de la boucle ouverte `l` sur les points `valid`.
pub fn margins(freqs: &[f64], l: &[C], valid: Option<&[bool]>) -> Result<Margins, String> {
    if l.len() != freqs.len() || valid.is_some_and(|v| v.len() != freqs.len()) {
        return Err("margins : tailles différentes".into());
    }
    let mut m = Margins::default();
    let (mut f, mut lv): (Vec<f64>, Vec<C>) = (Vec::new(), Vec::new());
    for i in 0..freqs.len() {
        let z = l[i];
        let ok = z.0.is_finite() && z.1.is_finite() && abs(z) > 0.0 && freqs[i] > 0.0 && valid.is_none_or(|v| v[i]);
        if ok {
            f.push(freqs[i]);
            lv.push(z);
        }
    }
    if f.len() < 2 {
        return Ok(m);
    }
    let logf: Vec<f64> = f.iter().map(|v| v.ln()).collect();
    let logmag: Vec<f64> = lv.iter().map(|&z| abs(z).ln()).collect();
    let angle: Vec<f64> = lv.iter().map(|z| z.1.atan2(z.0)).collect();
    let mut phase = unwrap(&angle);
    let shift = 2.0 * PI * (phase[0] / (2.0 * PI)).ceil(); // premier point dans (−360°, 0]
    for p in phase.iter_mut() {
        *p -= shift;
    }
    let n = logf.len();

    // premier passage de |L| au-dessous de 1 : marge de phase
    let down = (0..n - 1).find(|&i| logmag[i] >= 0.0 && logmag[i + 1] < 0.0);
    if let Some(d) = down {
        let (x, _, ph) = interp(&logf, &logmag, &phase, &logmag, d);
        m.crossover_hz = x.exp();
        m.phase_margin_deg = 180.0 + ph.to_degrees();
    }

    // premier passage de la phase par −180° (− k·360°) après le croisement, |L| < 1 : marge de gain
    let level: Vec<f64> = phase.iter().map(|p| (p + PI) / (2.0 * PI)).collect();
    let start = down.map_or(0, |d| d + 1);
    for i in start..n - 1 {
        let k = py_max(level[i], level[i + 1]).floor();
        let lo = if level[i + 1] < level[i] {
            level[i + 1]
        } else {
            level[i]
        }; // min(a, b) de Python
        if lo <= k {
            let g: Vec<f64> = level.iter().map(|v| v - k).collect();
            let (x, lm, _) = interp(&logf, &logmag, &phase, &g, i);
            if lm < 0.0 {
                m.gain_margin_db = -20.0 * lm / 10f64.ln();
                m.phase_crossover_hz = x.exp();
                break;
            }
        }
    }

    let s: Vec<f64> = lv
        .iter()
        .map(|&z| abs(cdiv((1.0, 0.0), (1.0 + z.0, 0.0 + z.1))))
        .collect();
    let k = argmax_f64(&s).ok_or("pic de sensibilité : tableau vide")?;
    m.sensitivity_peak_db = 20.0 * s[k].log10();
    m.sensitivity_peak_hz = f[k];
    m.sensitivity_peak_at_edge = k == 0 || k == s.len() - 1;
    Ok(m)
}

/// `LoopMetrics` de la méthode approchée (la mesurée arrive aux étapes (b) et (c)).
#[derive(Debug, Clone)]
pub struct LoopMetrics {
    pub method: &'static str,
    pub notes: Vec<String>,
    pub band_hz: (f64, f64),
    pub bandwidth_hz: f64,
    pub tracking_peak_db: f64,
    pub sensitivity_peak_db: f64,
    pub sensitivity_peak_hz: f64,
    pub sensitivity_peak_at_edge: bool,
    pub crossover_hz: f64,
    pub phase_margin_deg: f64,
    pub gain_margin_db: f64,
    pub d_ratio: f64,
    pub freqs: Vec<f64>,
    pub t: Vec<C>,
    pub coherence: Vec<f64>,
    pub l: Vec<C>,
    pub valid: Vec<bool>,
}

impl LoopMetrics {
    pub fn new(freqs: &[f64], t: &[C], coh: &[f64]) -> Self {
        LoopMetrics {
            method: APPROX,
            notes: Vec::new(),
            band_hz: (f64::NAN, f64::NAN),
            bandwidth_hz: f64::NAN,
            tracking_peak_db: f64::NAN,
            sensitivity_peak_db: f64::NAN,
            sensitivity_peak_hz: f64::NAN,
            sensitivity_peak_at_edge: false,
            crossover_hz: f64::NAN,
            phase_margin_deg: f64::NAN,
            gain_margin_db: f64::NAN,
            d_ratio: f64::NAN,
            freqs: freqs.to_vec(),
            t: t.to_vec(),
            coherence: coh.to_vec(),
            l: Vec::new(),
            valid: Vec::new(),
        }
    }

    fn fill(&mut self, mg: &Margins) {
        self.sensitivity_peak_db = mg.sensitivity_peak_db;
        self.sensitivity_peak_hz = mg.sensitivity_peak_hz;
        self.sensitivity_peak_at_edge = mg.sensitivity_peak_at_edge;
        self.crossover_hz = mg.crossover_hz;
        self.phase_margin_deg = mg.phase_margin_deg;
        self.gain_margin_db = mg.gain_margin_db;
    }
}

/// `_smooth_complex` : moyenne glissante sur parties réelle et imaginaire, bords prolongés.
fn smooth_complex(z: &[C], width: i64) -> Result<Vec<C>, String> {
    if width <= 1 || (z.len() as i64) < width {
        return Ok(z.to_vec());
    }
    let re: Vec<f64> = z.iter().map(|c| c.0).collect();
    let im: Vec<f64> = z.iter().map(|c| c.1).collect();
    let re = otb_dsp::numeric::uniform_filter1d(&re, &[re.len()], width, 0)?;
    let im = otb_dsp::numeric::uniform_filter1d(&im, &[im.len()], width, 0)?;
    // `real + 1j * imag` : 1j * x = (0·x − 1·0, 0·0 + 1·x) → la partie réelle s'ajoute à 0·x (NaN si x infini)
    Ok(re
        .iter()
        .zip(&im)
        .map(|(&a, &b)| (a + (0.0 * b - 0.0), 0.0 + b))
        .collect())
}

/// `loop_from_T` : méthode approchée, bande passante, puis L = T / (1 − T) et ses marges, T = consigne → gyro.
pub fn loop_from_t(freqs: &[f64], t: &[C], coh: &[f64], f_max: f64, min_coh: f64) -> Result<LoopMetrics, String> {
    if t.len() != freqs.len() || coh.len() != freqs.len() {
        return Err("loop_from_t : tailles différentes".into());
    }
    let mut lm = LoopMetrics::new(freqs, t, coh);
    let valid: Vec<bool> = (0..freqs.len())
        .map(|i| {
            freqs[i] >= F_MIN && freqs[i] <= f_max && t[i].0.is_finite() && t[i].1.is_finite() && coh[i] >= min_coh
        })
        .collect();
    let sel: Vec<usize> = (0..freqs.len()).filter(|&i| valid[i]).collect();
    if sel.len() < 5 {
        return Ok(lm);
    }
    let f: Vec<f64> = sel.iter().map(|&i| freqs[i]).collect();
    let tv = smooth_complex(&sel.iter().map(|&i| t[i]).collect::<Vec<_>>(), SMOOTH)?;
    let half = 1.0 / 2f64.sqrt();
    if let Some(b) = tv.iter().position(|&z| abs(z) < half) {
        lm.bandwidth_hz = f[b];
    }
    let lo: Vec<C> = tv.iter().map(|&z| cdiv(z, (1.0 - z.0, 0.0 - z.1))).collect();
    lm.fill(&margins(&f, &lo, None)?);
    lm.tracking_peak_db = lm.sensitivity_peak_db; // ici S = 1 − T
    lm.band_hz = (f[0], f[f.len() - 1]);
    let mut l = vec![(f64::NAN, 0.0); freqs.len()]; // np.full(…, nan, dtype=complex) : nan + 0j
    for (k, &i) in sel.iter().enumerate() {
        l[i] = lo[k];
    }
    lm.l = l;
    lm.valid = valid;
    Ok(lm)
}

/// `dterm_model` : Cd attendu d'après l'en-tête, Kd · dérivée discrète · filtres du D (cadence PID), pour recouper Ĉd.
pub fn dterm_model(s: &Settings, freqs: &[f64], d_gain: f64, throttle_frac: f64) -> Result<Vec<C>, String> {
    let fp = s.pid_rate_hz();
    let chain = crate::bf_filters::dterm_chain(s, throttle_frac);
    let (hr, hi) = crate::bf_filters::chain_response(&chain, freqs)?;
    let scale = DTERM_SCALE * d_gain * fp;
    Ok(freqs
        .iter()
        .enumerate()
        .map(|(i, &f)| {
            // -2j * np.pi * f / fp : (0 − 2π·i) · f, puis division complexe par le réel fp (Smith, `cdiv`), puis exp complexe
            let prod = (0.0 * f - (-2.0 * PI) * 0.0, 0.0 * 0.0 + (-2.0 * PI) * f);
            let (tr, ti) = cdiv(prod, (fp, 0.0));
            let r = tr.exp();
            let (zr, zi) = if ti == 0.0 {
                (r, ti)
            } else {
                (r * ti.cos(), r * ti.sin())
            };
            let (a, b) = (1.0 - zr, 0.0 - zi);
            let d = (scale * a - 0.0 * b, scale * b + 0.0 * a);
            (d.0.mul_add(hr[i], -(d.1 * hi[i])), d.0.mul_add(hi[i], d.1 * hr[i]))
        })
        .collect())
}
