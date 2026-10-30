//! Réponse indicielle et métriques de la boucle (port de `analysis/step.py`, phase 4c-B(d)).
//!
//! Le noyau de déconvolution (`step_windows`) est celui de la phase 2b (`otb_dsp::step`) ; ici : sélection des fenêtres, moyenne et écart-type,
//! fonction de transfert en boucle fermée, boucle ouverte (`open_loop`), métriques (`step_metrics`) et forme de la montée (`shape_metrics`,
//! cassure et vagues avec incertitudes par jackknife). Les sommes de numpy sont reproduites dans leur ordre : sur l'axe 0 d'une matrice, ligne
//! après ligne ; sur un vecteur, par paires (`pairwise_sum`).
//!
//! ⚠️ `smooth > 1` (convolution par BLAS) n'est pas porté : aucun appelant ne le fournit ; une valeur > 1 rend une erreur.

use crate::npstat::{argmax_f64, cdiv, median_f64, py_min};
use crate::numpy_mean;
use crate::open_loop::{self, LoopMetrics, Terms};
use crate::spectral::{clear_of_breaks, py_round, rfftfreq};
use otb_dsp::numeric::{hann, uniform_filter1d};
use otb_dsp::pairwise_sum;

/// Zéros ajoutés de chaque côté de la fenêtre.
pub const PAD: usize = 100;
/// La montée s'arrête quand la pente retombe sous 10 % de son maximum.
pub const STALL_SLOPE: f64 = 0.1;
/// Lissage de la courbe avant d'en prendre la pente (ms).
pub const STALL_SMOOTH_MS: f64 = 2.5;
/// Tendance retirée avant de mesurer les vagues (ms).
pub const WAVES_TREND_MS: f64 = 15.0;
pub const WAVES_END_MS: f64 = 250.0;
pub const JACKKNIFE_BLOCKS: usize = 10;

/// `StepMetrics` : les NaN de Python sont des NaN ici.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepMetrics {
    pub peak: f64,
    pub peak_ms: f64,
    pub latency_ms: f64,
    pub rise_ms: f64,
    pub settling_ms: f64,
    pub overshoot_pct: f64,
    pub ripple: f64,
    pub ripple_std: f64,
    pub ripple_ms: f64,
    pub waves: f64,
    pub waves_ratio: f64,
}

impl Default for StepMetrics {
    fn default() -> Self {
        let n = f64::NAN;
        StepMetrics {
            peak: n,
            peak_ms: n,
            latency_ms: n,
            rise_ms: n,
            settling_ms: n,
            overshoot_pct: n,
            ripple: n,
            ripple_std: n,
            ripple_ms: n,
            waves: n,
            waves_ratio: n,
        }
    }
}

/// `max(a, b)` de Python : `a` sauf si `b > a`.
fn py_max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// `np.gradient(f, x)` : dérivée à coordonnées, ordre 1 aux bords. Comme numpy, des pas **tous égaux** (au bit près) prennent la formule à
/// pas constant `(f[i+1] − f[i−1]) / (2·dx)`, sinon la formule à trois points pour une grille quelconque. Moins de 2 points : erreur.
pub fn gradient(f: &[f64], x: &[f64]) -> Result<Vec<f64>, String> {
    let n = f.len();
    if x.len() != n {
        return Err("gradient : coordonnées et valeurs de tailles différentes".into());
    }
    if n < 2 {
        return Err("Shape of array too small to calculate a numerical gradient".into());
    }
    let dx: Vec<f64> = x.windows(2).map(|w| w[1] - w[0]).collect();
    let uniform = dx.iter().all(|&d| d == dx[0]);
    let mut g = vec![0.0; n];
    for i in 1..n - 1 {
        g[i] = if uniform {
            // numpy : out[1:-1] = (f[2:] − f[:-2]) / (2·dx)
            (f[i + 1] - f[i - 1]) / (2.0 * dx[0])
        } else {
            let (dx1, dx2) = (dx[i - 1], dx[i]);
            let a = -dx2 / (dx1 * (dx1 + dx2));
            let b = (dx2 - dx1) / (dx1 * dx2);
            let c = dx1 / (dx2 * (dx1 + dx2));
            a * f[i - 1] + b * f[i] + c * f[i + 1]
        };
    }
    g[0] = (f[1] - f[0]) / dx[0];
    g[n - 1] = (f[n - 1] - f[n - 2]) / dx[n - 2];
    Ok(g)
}

/// `_crossing` : premier instant (interpolé) où `r` atteint `level`.
fn crossing(t: &[f64], r: &[f64], level: f64) -> f64 {
    let Some(k) = r.iter().position(|&v| v >= level) else {
        return f64::NAN;
    };
    if k == 0 {
        return t[0];
    }
    let (r0, r1) = (r[k - 1], r[k]);
    let frac = if r1 != r0 { (level - r0) / (r1 - r0) } else { 0.0 };
    t[k - 1] + frac * (t[k] - t[k - 1])
}

/// `step_metrics` : métriques d'une réponse indicielle moyenne.
pub fn step_metrics(t_ms: &[f64], r: &[f64]) -> Result<StepMetrics, String> {
    let mut m = StepMetrics::default();
    if r.is_empty() || !r.iter().all(|v| v.is_finite()) {
        return Ok(m);
    }
    if t_ms.len() != r.len() {
        return Err("step_metrics : t_ms et r de tailles différentes".into());
    }
    m.latency_ms = crossing(t_ms, r, 0.5);
    // pic cherché dans la montée (150 ms comme PIDtoolbox, plus long pour les drones lents)
    let horizon = if m.latency_ms.is_finite() {
        py_max(150.0, 3.0 * m.latency_ms)
    } else {
        t_ms[t_ms.len() - 1]
    };
    let masked: Vec<f64> = t_ms
        .iter()
        .zip(r)
        .filter(|(t, _)| **t <= horizon)
        .map(|(_, v)| *v)
        .collect();
    // `r[i]` : l'indice vient du tableau masqué (un préfixe, puisque `t_ms` croît) ; reproduit tel quel
    let i = argmax_f64(&masked).ok_or("attempt to get argmax of an empty sequence")?;
    m.peak = r[i];
    m.peak_ms = t_ms[i];
    m.overshoot_pct = py_max(0.0, (m.peak - 1.0) * 100.0);
    let (t10, t90) = (crossing(t_ms, r, 0.1), crossing(t_ms, r, 0.9));
    m.rise_ms = t90 - t10;
    let out: Vec<bool> = r.iter().map(|v| (v - 1.0).abs() > 0.05).collect();
    if !out.iter().any(|&b| b) {
        m.settling_ms = 0.0;
    } else if !out[out.len() - 1] {
        let last = out.iter().rposition(|&b| b).unwrap();
        m.settling_ms = t_ms[last + 1];
    }
    Ok(m)
}

/// `_smooth` : moyenne glissante de largeur `max(1, n)` sur chaque ligne (`rows` × `len`).
fn smooth(x: &[f64], rows: usize, len: usize, n: i64) -> Result<Vec<f64>, String> {
    uniform_filter1d(x, &[rows, len], n.max(1), 1)
}

/// `_ripple` : (cassure, instant de l'arrêt de la montée) d'une réponse moyenne.
fn ripple(t_ms: &[f64], r: &[f64], latency_ms: f64) -> Result<(f64, f64), String> {
    let dt = t_ms[1] - t_ms[0];
    let rs = smooth(r, 1, r.len(), py_round(STALL_SMOOTH_MS / dt)?)?;
    let d = gradient(&rs, t_ms)?;
    let lim = py_max(100.0, 3.0 * latency_ms);
    let rise: Vec<f64> = t_ms
        .iter()
        .zip(&d)
        .filter(|(t, _)| **t <= lim)
        .map(|(_, v)| *v)
        .collect();
    let k0 = argmax_f64(&rise).ok_or("attempt to get argmax of an empty sequence")?;
    let Some(below) = d[k0..].iter().position(|&v| v < STALL_SLOPE * d[k0]) else {
        return Ok((f64::NAN, f64::NAN));
    };
    let k = k0 + below;
    let lo = py_max(50.0, 3.0 * latency_ms);
    let flat: Vec<f64> = t_ms
        .iter()
        .zip(r)
        .filter(|(t, _)| **t >= lo && **t <= 200.0)
        .map(|(_, v)| *v)
        .collect();
    if flat.is_empty() {
        return Ok((f64::NAN, f64::NAN));
    }
    let plateau = median_f64(&flat);
    Ok((py_min(plateau, 1.0) - rs[k], t_ms[k]))
}

/// `_detrended` : lignes de `x` (`rows` × `len`) moins leur tendance, restreintes aux instants de `w`.
fn detrended(t_ms: &[f64], x: &[f64], rows: usize, w: &[bool]) -> Result<Vec<f64>, String> {
    let len = t_ms.len();
    let dt = t_ms[1] - t_ms[0];
    let tr = smooth(x, rows, len, py_round(WAVES_TREND_MS / dt)?)?;
    let mut out = Vec::new();
    for r in 0..rows {
        for j in 0..len {
            if w[j] {
                out.push(x[r * len + j] - tr[r * len + j]);
            }
        }
    }
    Ok(out)
}

/// Somme des lignes d'une matrice dans l'ordre (axe 0 de numpy), `rows` × `len`.
///
/// ⚠️ Une matrice d'**une seule colonne** est aussi « Fortran » : numpy somme alors la colonne par paires (L-052).
fn sum_rows(m: &[f64], rows: usize, len: usize) -> Vec<f64> {
    if len == 1 {
        return vec![0.0 + pairwise_sum(&m[..rows])];
    }
    let mut acc = m[..len].to_vec();
    for r in 1..rows {
        for j in 0..len {
            acc[j] += m[r * len + j];
        }
    }
    acc
}

/// `R.mean(axis=0)`.
fn mean_rows(m: &[f64], rows: usize, len: usize) -> Vec<f64> {
    sum_rows(m, rows, len).into_iter().map(|v| v / rows as f64).collect()
}

/// `shape_metrics` : cassure et vagues de la réponse moyenne de `R` (`rows` réponses de `len` points, dans l'ordre du vol), avec leur
/// incertitude par jackknife ; complète `m` (latence déjà calculée).
pub fn shape_metrics(t_ms: &[f64], r: &[f64], rows: usize, m: &mut StepMetrics) -> Result<(), String> {
    let len = t_ms.len();
    if rows == 0 || len < 3 || !m.latency_ms.is_finite() {
        return Ok(());
    }
    if r.len() != rows * len {
        return Err("shape_metrics : R n'a pas la forme (k, len(t_ms))".into());
    }
    let mean = mean_rows(r, rows, len);
    let (rp, rp_ms) = ripple(t_ms, &mean, m.latency_ms)?;
    m.ripple = rp;
    m.ripple_ms = rp_ms;
    let w: Vec<bool> = t_ms
        .iter()
        .map(|&t| t >= 3.0 * m.latency_ms && t <= WAVES_END_MS)
        .collect();
    if !w.iter().any(|&b| b) {
        return Ok(());
    }
    let det = detrended(t_ms, &mean, 1, &w)?;
    let sq: Vec<f64> = det.iter().map(|v| v * v).collect();
    m.waves = numpy_mean(&sq).sqrt();
    let k = JACKKNIFE_BLOCKS.min(rows / 4);
    if k < 4 {
        return Ok(());
    }
    // np.array_split(np.arange(rows), k) : les `rows % k` premiers blocs ont un élément de plus
    let (q, rem) = (rows / k, rows % k);
    let mut bounds = Vec::with_capacity(k);
    let mut at = 0;
    for g in 0..k {
        let size = q + usize::from(g < rem);
        bounds.push((at, at + size));
        at += size;
    }
    let sums: Vec<Vec<f64>> = bounds
        .iter()
        .map(|&(a, b)| sum_rows(&r[a * len..b * len], b - a, len))
        .collect();
    let flat_sums: Vec<f64> = sums.concat();
    let total = sum_rows(&flat_sums, k, len);
    let mut loo = Vec::with_capacity(k * len);
    for (b, &(a0, a1)) in bounds.iter().enumerate() {
        let denom = (rows - (a1 - a0)) as f64;
        for j in 0..len {
            loo.push((total[j] - sums[b][j]) / denom);
        }
    }
    let mut rip = Vec::with_capacity(k);
    for b in 0..k {
        rip.push(ripple(t_ms, &loo[b * len..(b + 1) * len], m.latency_ms)?.0);
    }
    if rip.iter().all(|v| v.is_finite()) {
        let mean_rip = (0.0 + pairwise_sum(&rip)) / k as f64;
        let sq: Vec<f64> = rip.iter().map(|v| (v - mean_rip) * (v - mean_rip)).collect();
        m.ripple_std = ((k as f64 - 1.0) / k as f64 * (0.0 + pairwise_sum(&sq))).sqrt();
    }
    let det = detrended(t_ms, &loo, k, &w)?;
    let nw = det.len() / k;
    // ⚠️ `x[..., w]` est rangé « Fortran » : `mean(axis=0)` et `sum(axis=0)` somment chaque colonne **par paires** (pas dans l'ordre : L-052)
    let col = |m: &[f64], j: usize| -> Vec<f64> { (0..k).map(|b| m[b * nw + j]).collect() };
    let det_mean: Vec<f64> = (0..nw)
        .map(|j| (0.0 + pairwise_sum(&col(&det, j))) / k as f64)
        .collect();
    let noise: Vec<f64> = (0..nw)
        .map(|j| {
            let dev: Vec<f64> = col(&det, j)
                .iter()
                .map(|v| (v - det_mean[j]) * (v - det_mean[j]))
                .collect();
            ((k as f64 - 1.0) / k as f64 * (0.0 + pairwise_sum(&dev))).sqrt()
        })
        .collect();
    let nsq: Vec<f64> = noise.iter().map(|v| v * v).collect();
    let floor = numpy_mean(&nsq).sqrt();
    if floor > 0.0 {
        m.waves_ratio = m.waves / floor;
    }
    Ok(())
}

/// `_window_starts` : débuts des fenêtres de `l` échantillons (pas `hop_ms`) où |consigne| atteint `min_input` (et reste sous `max_input`).
pub fn window_starts(
    sp: &[f64],
    fs: f64,
    l: i64,
    hop_ms: f64,
    min_input: f64,
    max_input: Option<f64>,
    breaks: Option<&[i64]>,
) -> Result<Vec<i64>, String> {
    if (sp.len() as i64) < l {
        return Ok(Vec::new());
    }
    let hop = py_round(hop_ms * fs / 1000.0)?.max(1);
    let n = sp.len() as i64;
    let all: Vec<i64> = (0..=n - l).step_by(hop as usize).collect();
    let keep = clear_of_breaks(&all, l, breaks);
    let starts: Vec<i64> = all.iter().zip(&keep).filter(|(_, k)| **k).map(|(s, _)| *s).collect();
    if starts.is_empty() {
        return Ok(starts);
    }
    Ok(starts
        .into_iter()
        .filter(|&s| {
            // np.max(|sp|) de la fenêtre : un NaN la rend NaN, la comparaison est alors fausse
            let pk = sp[s as usize..(s + l) as usize]
                .iter()
                .map(|v| v.abs())
                .fold(
                    0.0f64,
                    |m, v| if m.is_nan() || v.is_nan() { f64::NAN } else { m.max(v) },
                );
            pk >= min_input && max_input.is_none_or(|m| pk < m)
        })
        .collect())
}

/// `input_windows` : nombre de fenêtres où la réponse indicielle a assez de consigne (sans la déconvolution).
pub fn input_windows(
    sp: &[f64],
    fs: f64,
    seg_s: f64,
    min_input: f64,
    hop_ms: f64,
    breaks: Option<&[i64]>,
) -> Result<usize, String> {
    Ok(window_starts(sp, fs, py_round(seg_s * fs)?, hop_ms, min_input, None, breaks)?.len())
}

/// Termes PID fournis à `step_response` (`PidTerms` de Python, ou `None`).
pub enum LoopInput<'a> {
    /// `terms=None` : méthode approchée, note « termes PID non fournis ».
    Absent,
    /// `terms.note` non vide : boucle ouverte non mesurable, la raison devient une note.
    Unmeasurable(&'a str),
    Terms(Terms<'a>),
}

/// `StepResult`.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub t_ms: Vec<f64>,
    /// `n × len(t_ms)`, rangé ligne par ligne.
    pub responses: Vec<f64>,
    pub mean: Vec<f64>,
    pub std: Vec<f64>,
    pub n: usize,
    pub metrics: StepMetrics,
    pub loop_: LoopMetrics,
    pub max_inputs: Vec<f64>,
}

/// Paramètres de `step_response` autres que les signaux.
pub struct StepOptions {
    pub seg_s: f64,
    pub min_input: f64,
    pub max_input: Option<f64>,
    pub resp_ms: f64,
    pub hop_ms: f64,
    pub reg: f64,
    pub smooth: i64,
    pub y_correction: bool,
}

impl Default for StepOptions {
    fn default() -> Self {
        StepOptions {
            seg_s: 2.0,
            min_input: 20.0,
            max_input: None,
            resp_ms: 500.0,
            hop_ms: 62.5,
            reg: 1e-4,
            smooth: 0,
            y_correction: false,
        }
    }
}

/// `step_response` : réponse indicielle d'un axe. `terms` : boucle ouverte mesurée ; sans eux, méthode approchée. `breaks` : raccords du log.
pub fn step_response(
    sp: &[f64],
    gy: &[f64],
    fs: f64,
    o: &StepOptions,
    terms: &LoopInput,
    breaks: Option<&[i64]>,
) -> Result<StepResult, String> {
    if o.smooth > 1 {
        return Err("step_response : smooth > 1 non porté".into());
    }
    let n = sp.len().min(gy.len());
    let l = py_round(o.seg_s * fs)?;
    let lr = py_round(o.resp_ms * fs / 1000.0)? + 1;
    if lr < 0 {
        return Err("negative dimensions are not allowed".into()); // np.zeros((0, Lr)) de Python
    }
    let t_ms: Vec<f64> = (0..lr).map(|i| i as f64 * 1000.0 / fs).collect();
    let empty = |t_ms: &Vec<f64>| StepResult {
        t_ms: t_ms.clone(),
        responses: Vec::new(),
        mean: vec![f64::NAN; t_ms.len()],
        std: vec![f64::NAN; t_ms.len()],
        n: 0,
        metrics: StepMetrics::default(),
        loop_: LoopMetrics::new(&[], &[], &[]),
        max_inputs: Vec::new(),
    };
    if (n as i64) < l {
        return Ok(empty(&t_ms));
    }
    let starts = window_starts(&sp[..n], fs, l, o.hop_ms, o.min_input, o.max_input, breaks)?;
    let w = hann(l, true);
    let nfft = l as usize + 2 * PAD;
    // plage du régime établi (200–500 ms) où l'on contrôle la qualité
    let steady: Vec<usize> = (0..t_ms.len())
        .filter(|&i| t_ms[i] >= 200.0 && t_ms[i] <= 500.0)
        .collect();
    let (lo, hi) = if steady.is_empty() {
        (0, 0)
    } else {
        (steady[0], steady[steady.len() - 1] + 1)
    };
    let p = otb_dsp::step::StepParams {
        w: &w,
        l: l as usize,
        lr: lr as usize,
        pad: PAD,
        reg: o.reg,
        y_correction: o.y_correction,
        steady: (lo, hi),
    };
    let out = otb_dsp::step::step_windows(sp, gy, &starts, &p)?;
    let k = out.maxin.len();
    if k == 0 {
        return Ok(empty(&t_ms));
    }
    let len = lr as usize;
    let mean = mean_rows(&out.rows, k, len);
    // R.std(axis=0) : écart quadratique moyen à la moyenne, somme des lignes dans l'ordre
    let mut dev = Vec::with_capacity(k * len);
    for row in out.rows.chunks(len) {
        for (v, m) in row.iter().zip(&mean) {
            let d = v - m;
            dev.push(d * d);
        }
    }
    let std: Vec<f64> = sum_rows(&dev, k, len)
        .into_iter()
        .map(|v| (v / k as f64).sqrt())
        .collect();
    let freqs = rfftfreq(nfft, fs);
    let nf = freqs.len();
    let t: Vec<open_loop::C> = (0..nf)
        .map(|j| cdiv((out.sum_gh_re[j], out.sum_gh_im[j]), (out.sum_hh[j], 0.0)))
        .collect();
    let coh: Vec<f64> = (0..nf)
        .map(|j| {
            let a = out.sum_gh_re[j].hypot(out.sum_gh_im[j]);
            a * a / (out.sum_gg[j] * out.sum_hh[j])
        })
        .collect();
    let mut lp = open_loop::loop_from_t(&freqs, &t, &coh, 150.0, 0.5)?;
    lp = match terms {
        LoopInput::Absent => {
            lp.notes.push("termes PID non fournis".into());
            lp
        }
        LoopInput::Unmeasurable(note) if !note.is_empty() => {
            lp.notes.push((*note).to_string());
            lp
        }
        LoopInput::Unmeasurable(_) => lp,
        LoopInput::Terms(tm) => {
            open_loop::loop_from_terms(sp, gy, tm, fs, lp, o.seg_s, o.min_input, o.max_input, breaks)?
        }
    };
    let mut metrics = step_metrics(&t_ms, &mean)?;
    shape_metrics(&t_ms, &out.rows, k, &mut metrics)?;
    Ok(StepResult {
        t_ms,
        responses: out.rows,
        mean,
        std,
        n: k,
        metrics,
        loop_: lp,
        max_inputs: out.maxin,
    })
}
