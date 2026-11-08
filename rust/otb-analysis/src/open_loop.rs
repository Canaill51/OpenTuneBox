//! Boucle ouverte (port de `analysis/loop.py`, phase 4c-B) : marges de stabilité, méthode approchée, modèle du terme D.
//!
//! Étape (a) : `margins`, `loop_from_t` (T = consigne → gyro, L = T / (1 − T)) et `dterm_model`. Les nombres complexes sont des
//! couples `(re, im)` ; la division reproduit celle de numpy (Smith, `npstat::cdiv`), le module est `hypot` (L-042).
//!
//! Décisions discrètes (indice du croisement, de la phase à −180°, du pic de sensibilité, bord de bande, nombre de points
//! valides) : à exiger **identiques** à Python ; valeurs continues (logs, atan2, exp) à tolérance mesurée (L-048, L-051).

use crate::npstat::{argmax_f64, cdiv, conj_mul_fma, py_min, unwrap};
use crate::spectral::{clear_of_breaks, py_round, rfftfreq, row_mean};
use otb_dsp::numeric::{hann, rfft_rows};
use otb_log::settings::Settings;
use std::f64::consts::PI;

pub type C = (f64, f64);

pub const MEASURED: &str = "mesurée";
pub const APPROX: &str = "approchée";
/// Bande de la mesure (Hz), plafonnée à 0,4 × fréquence du log.
pub const F_MIN: f64 = 2.0;
/// Lissage des spectres (points), méthode approchée.
pub const SMOOTH: i64 = 5;
/// Bande de la mesure (Hz), plafonnée à 0,4 × fréquence du log.
pub const F_MAX: f64 = 200.0;
/// Erreur aléatoire relative de Ĝ tolérée.
pub const MAX_ERR_G: f64 = 0.2;
/// Cohérences erreur → P+I et gyro → D minimales.
pub const MIN_COH_PID: f64 = 0.8;
/// Lissage de la mesure : largeur = 20 % de la fréquence (au moins `SMOOTH` points).
pub const SMOOTH_REL: f64 = 0.2;
/// Blocs du jackknife.
pub const BLOCKS: i64 = 8;
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

/// ⚠️ Les fonctions de cette page supposent des entrées que `loop_spectra` produit : grille de fréquences strictement croissante, `valid` de la
/// longueur de la grille (W-027).
///
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
    /// Écarts-types jackknife des marges (mesurée, au moins 3 blocs) : clé et valeur, dans l'ordre de [`STD_KEYS`].
    pub std: Vec<(&'static str, f64)>,
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
            std: Vec::new(),
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

// ---------------------------------------------------------------- spectres croisés et fonctions de transfert (étape (b))

/// Produit complexe de numpy sur ce poste (opérations fusionnées : L-051).
fn cmul(a: C, b: C) -> C {
    (a.0.mul_add(b.0, -(a.1 * b.1)), a.0.mul_add(b.1, a.1 * b.0))
}

fn cadd(a: C, b: C) -> C {
    (a.0 + b.0, a.1 + b.1)
}

/// Paires de spectres croisés S(x, y) = Σ conj(X)·Y : r consigne, y gyro, p = P + I, u = P + I + D + F, e = r − y, z instrument.
pub const PAIRS: [&str; 16] = [
    "rr", "yy", "ee", "uu", "pp", "dd", "zz", "ry", "rf", "zy", "zu", "zp", "zd", "ze", "ep", "yd",
];

fn pair_index(name: &str) -> usize {
    PAIRS.iter().position(|p| *p == name).expect("paire connue")
}

/// `LoopSpectra` : spectres croisés sommés par bloc de fenêtres consécutives (pour le jackknife).
#[derive(Debug, Clone)]
pub struct LoopSpectra {
    pub freqs: Vec<f64>,
    pub fs: f64,
    /// `blocks[paire][bloc][fréquence]`, paires dans l'ordre de [`PAIRS`].
    pub blocks: Vec<Vec<Vec<C>>>,
    pub n_windows: usize,
    pub has_d: bool,
}

impl LoopSpectra {
    pub fn n_blocks(&self) -> usize {
        self.blocks.first().map_or(0, |b| b.len())
    }

    /// `total(leave_out)` : somme des blocs conservés, dans l'ordre (somme de numpy sur l'axe 0 : séquentielle).
    pub fn total(&self, leave_out: Option<usize>) -> Vec<Vec<C>> {
        let nf = self.freqs.len();
        self.blocks
            .iter()
            .map(|pair| {
                let kept = pair
                    .iter()
                    .enumerate()
                    .filter(|(b, _)| Some(*b) != leave_out)
                    .map(|(_, v)| v);
                // `v[keep].sum(axis=0)` part de +0,0 (un −0,0 de la partie imaginaire des cases DC et Nyquist devient +0,0)
                let mut acc = vec![(0.0, 0.0); nf];
                for v in kept {
                    for (a, x) in acc.iter_mut().zip(v) {
                        *a = cadd(*a, *x);
                    }
                }
                acc
            })
            .collect()
    }
}

/// Entrées de `loop_spectra` autres que la consigne et le gyro : les termes PID (en `float64`, comme Python les convertit).
pub struct Terms<'a> {
    pub p: &'a [f64],
    pub i: &'a [f64],
    pub d: &'a [f64],
    pub f: &'a [f64],
    /// Signal extérieur à la boucle pour estimer G ; `None` : la consigne.
    pub instrument: Option<&'a [f64]>,
}

/// `np.array_split(starts, k)` : les `n % k` premiers morceaux ont un élément de plus.
fn array_split(n: usize, k: usize) -> Vec<(usize, usize)> {
    let (q, r) = (n / k, n % k);
    let mut out = Vec::with_capacity(k);
    let mut at = 0;
    for g in 0..k {
        let len = q + usize::from(g < r);
        out.push((at, at + len));
        at += len;
    }
    out
}

/// `loop_spectra` : spectres croisés sur les fenêtres où la consigne dépasse `min_input` (et reste sous `max_input`).
/// `None` quand il n'y a pas assez de fenêtres (Python rend `None`).
// Signature calquée sur celle de l'étalon : `otb-parity` rejoue ces fonctions par leur nom avec leurs arguments
// dans le même ordre, et les regrouper en structure casserait cette correspondance.
#[allow(clippy::too_many_arguments)]
pub fn loop_spectra(
    r: &[f64],
    y: &[f64],
    terms: &Terms,
    fs: f64,
    seg_s: f64,
    min_input: f64,
    max_input: Option<f64>,
    blocks: i64,
    breaks: Option<&[i64]>,
) -> Result<Option<LoopSpectra>, String> {
    let z = terms.instrument.unwrap_or(r);
    // `p + i` de numpy : tailles égales, ou l'une vaut 1 (diffusée) ; sinon Python lève une erreur de diffusion
    let (np_, ni) = (terms.p.len(), terms.i.len());
    if np_ != ni && np_ != 1 && ni != 1 {
        return Err(format!(
            "operands could not be broadcast together with shapes ({np_},) ({ni},)"
        ));
    }
    let len = if np_ == ni || ni == 1 { np_ } else { ni };
    let pi: Vec<f64> = (0..len)
        .map(|k| terms.p[if np_ == 1 { 0 } else { k }] + terms.i[if ni == 1 { 0 } else { k }])
        .collect();
    let n = [r.len(), y.len(), pi.len(), terms.d.len(), terms.f.len(), z.len()]
        .into_iter()
        .min()
        .unwrap();
    let l = py_round(seg_s * fs)?;
    if (n as i64) < l || l < 8 {
        return Ok(None);
    }
    let l = l as usize;
    let hop = l / 2;
    let all: Vec<i64> = (0..=n - l).step_by(hop).map(|s| s as i64).collect();
    let keep = clear_of_breaks(&all, l as i64, breaks);
    let starts: Vec<usize> = all
        .iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(s, _)| *s as usize)
        .collect();
    if starts.len() < 2 {
        return Ok(None);
    }
    // crête de la consigne par fenêtre (np.max d'un |r| : un NaN la rend NaN, la comparaison est alors fausse)
    let peak = |s: usize| {
        r[s..s + l].iter().map(|v| v.abs()).fold(
            0.0f64,
            |m, v| if m.is_nan() || v.is_nan() { f64::NAN } else { m.max(v) },
        )
    };
    let starts: Vec<usize> = starts
        .into_iter()
        .filter(|&s| {
            let pk = peak(s);
            pk >= min_input && max_input.is_none_or(|m| pk < m)
        })
        .collect();
    if starts.len() < 2 {
        return Ok(None);
    }
    if blocks < 1 {
        return Err("array_split : le nombre de morceaux doit être positif".into());
    }
    let w = hann(l as i64, true);
    let freqs = rfftfreq(l, fs);
    let nf = freqs.len();
    let groups = array_split(starts.len(), (blocks as usize).min(starts.len()));
    let mut out: Vec<Vec<Vec<C>>> = vec![Vec::with_capacity(groups.len()); PAIRS.len()];
    for &(g0, g1) in &groups {
        let st = &starts[g0..g1];
        let nw = st.len();
        let spec = |x: &[f64]| -> Vec<(f64, f64)> {
            let mut data = Vec::with_capacity(nw * l);
            for &s in st {
                let seg = &x[s..s + l];
                let m = row_mean(seg);
                data.extend(seg.iter().zip(&w).map(|(v, wv)| (v - m) * wv));
            }
            rfft_rows(&data, nw, l).into_iter().map(|c| (c.re, c.im)).collect()
        };
        let (sr, sy, sp, sd, sf, sz) = (spec(r), spec(y), spec(&pi), spec(terms.d), spec(terms.f), spec(z));
        let se: Vec<C> = sr.iter().zip(&sy).map(|(a, b)| (a.0 - b.0, a.1 - b.1)).collect();
        let su: Vec<C> = sp
            .iter()
            .zip(&sd)
            .zip(&sf)
            .map(|((a, b), c)| cadd(cadd(*a, *b), *c))
            .collect();
        let sig = |c: char| -> &Vec<C> {
            match c {
                'r' => &sr,
                'y' => &sy,
                'p' => &sp,
                'd' => &sd,
                'f' => &sf,
                'z' => &sz,
                'e' => &se,
                _ => &su,
            }
        };
        for (k, name) in PAIRS.iter().enumerate() {
            let (a, b) = (sig(name.as_bytes()[0] as char), sig(name.as_bytes()[1] as char));
            // np.sum(conj(X) · Y, axis=0) : produit fusionné par case, puis somme séquentielle sur les fenêtres
            let mut acc: Vec<C> = vec![(0.0, 0.0); nf]; // somme de numpy : part de +0,0
            for wi in 0..nw {
                for j in 0..nf {
                    acc[j] = cadd(acc[j], conj_mul_fma(a[wi * nf + j], b[wi * nf + j]));
                }
            }
            out[k].push(acc);
        }
    }
    let has_d = terms.d.iter().any(|&v| v != 0.0);
    Ok(Some(LoopSpectra {
        freqs,
        fs,
        blocks: out,
        n_windows: starts.len(),
        has_d,
    }))
}

/// `LoopEstimate` : fonctions de transfert estimées (toutes sur `freqs`) et masque des points valides.
#[derive(Debug, Clone)]
pub struct LoopEstimate {
    pub freqs: Vec<f64>,
    pub ce: Vec<C>,
    pub cd: Vec<C>,
    pub cf: Vec<C>,
    pub g: Vec<C>,
    pub t: Vec<C>,
    pub l: Vec<C>,
    /// Cohérences « zy », « zu » (instrument → gyro, → u), « ep », « yd ».
    pub coh_zy: Vec<f64>,
    pub coh_zu: Vec<f64>,
    pub coh_ep: Vec<f64>,
    pub coh_yd: Vec<f64>,
    pub valid: Vec<bool>,
    /// Contrôle, variante A : S = 1 − T·(Ce + Cd)/(Ce + Cf).
    pub s_a: Vec<C>,
    /// Contrôle, variante C : Ce et Cd estimés eux aussi par l'instrument.
    pub l_c: Vec<C>,
    pub err_g: Vec<f64>,
}

/// `_smooth_variable` : moyenne glissante de largeur variable (impaire, centrée), tronquée aux bords (somme cumulée).
fn smooth_variable(z: &[C], widths: &[i64]) -> Vec<C> {
    let n = z.len();
    let mut c: Vec<C> = Vec::with_capacity(n + 1);
    c.push((0.0, 0.0));
    let mut run = (0.0, 0.0);
    for (k, &v) in z.iter().enumerate() {
        run = if k == 0 { v } else { cadd(run, v) }; // np.cumsum recopie le premier élément
        c.push(run);
    }
    (0..n)
        .map(|i| {
            let half = widths[i] / 2;
            let lo = (i as i64 - half).clamp(0, n as i64) as usize;
            let hi = (i as i64 + half + 1).clamp(0, n as i64) as usize;
            cdiv((c[hi].0 - c[lo].0, c[hi].1 - c[lo].1), ((hi - lo) as f64, 0.0))
        })
        .collect()
}

/// `_rand_err` : erreur aléatoire relative du module d'une fonction de transfert estimée (cohérence γ², n moyennes).
fn rand_err(coh: f64, n_avg: f64) -> f64 {
    let nan_to_num = |v: f64| {
        if v.is_nan() {
            0.0
        } else if v == f64::INFINITY {
            f64::MAX
        } else if v == f64::NEG_INFINITY {
            f64::MIN
        } else {
            v
        }
    };
    let c = nan_to_num(coh).clamp(1e-12, 1.0);
    ((1.0 - c) / (2.0 * n_avg * c)).sqrt()
}

/// `_coh` : |S(x, y)|² / (Re S(x, x) · Re S(y, y)).
fn coh_of(xy: &[C], xx: &[C], yy: &[C]) -> Vec<f64> {
    (0..xy.len())
        .map(|i| {
            let a = abs(xy[i]);
            a * a / (xx[i].0 * yy[i].0)
        })
        .collect()
}

/// `_first_run` : points valides contigus à partir du premier point valide.
fn first_run(ok: &[bool]) -> Vec<bool> {
    let mut out = vec![false; ok.len()];
    if let Some(i) = ok.iter().position(|&v| v) {
        let stop = ok[i..].iter().position(|&v| !v).unwrap_or(ok.len() - i);
        for v in &mut out[i..i + stop] {
            *v = true;
        }
    }
    out
}

/// `estimate` : fonctions de transfert de la boucle depuis les spectres croisés (`leave_out` : bloc retiré, jackknife).
pub fn estimate(spec: &LoopSpectra, leave_out: Option<usize>) -> LoopEstimate {
    let f = &spec.freqs;
    let df = if f.len() > 1 { f[1] - f[0] } else { 1.0 };
    let widths: Vec<i64> = f
        .iter()
        .map(|&v| {
            let w = (SMOOTH as f64).max(2.0 * (SMOOTH_REL * v / df / 2.0).round_ties_even() + 1.0);
            w as i64
        })
        .collect();
    let s: Vec<Vec<C>> = spec
        .total(leave_out)
        .iter()
        .map(|v| smooth_variable(v, &widths))
        .collect();
    let sp = |name: &str| &s[pair_index(name)];
    let n = f.len();
    let div = |a: &[C], b: &[C]| -> Vec<C> { (0..n).map(|i| cdiv(a[i], b[i])).collect() };
    let neg = |a: &[C]| -> Vec<C> { a.iter().map(|z| (-z.0, -z.1)).collect() };
    let zeros = vec![(0.0, 0.0); n];
    let ce = div(sp("ep"), sp("ee"));
    let cd = if spec.has_d {
        div(&neg(sp("yd")), sp("yy"))
    } else {
        zeros.clone()
    };
    let g = div(sp("zy"), sp("zu"));
    let t = div(sp("ry"), sp("rr"));
    let cf = div(sp("rf"), sp("rr"));
    let l: Vec<C> = (0..n).map(|i| cmul(g[i], cadd(ce[i], cd[i]))).collect();
    let coh_zy = coh_of(sp("zy"), sp("zz"), sp("yy"));
    let coh_zu = coh_of(sp("zu"), sp("zz"), sp("uu"));
    let coh_ep = coh_of(sp("ep"), sp("ee"), sp("pp"));
    let coh_yd = coh_of(sp("yd"), sp("yy"), sp("dd"));
    let s_a: Vec<C> = (0..n)
        .map(|i| {
            let q = cdiv(cmul(t[i], cadd(ce[i], cd[i])), cadd(ce[i], cf[i]));
            (1.0 - q.0, 0.0 - q.1)
        })
        .collect();
    let cd_c = if spec.has_d {
        div(&neg(sp("zd")), sp("zy"))
    } else {
        zeros
    };
    let zp_ze = div(sp("zp"), sp("ze"));
    let l_c: Vec<C> = (0..n).map(|i| cmul(g[i], cadd(zp_ze[i], cd_c[i]))).collect();
    let err_g: Vec<f64> = (0..n)
        .map(|i| {
            let n_avg = (spec.n_windows as f64 * widths[i] as f64) / 2.0;
            let (a, b) = (rand_err(coh_zy[i], n_avg), rand_err(coh_zu[i], n_avg));
            (a * a + b * b).sqrt()
        })
        .collect();
    let f_top = py_min(F_MAX, 0.4 * spec.fs);
    let ok: Vec<bool> = (0..n)
        .map(|i| {
            f[i] >= F_MIN
                && f[i] <= f_top
                && l[i].0.is_finite()
                && l[i].1.is_finite()
                && err_g[i] <= MAX_ERR_G
                && coh_ep[i] >= MIN_COH_PID
                && (!spec.has_d || coh_yd[i] >= MIN_COH_PID)
        })
        .collect();
    LoopEstimate {
        freqs: f.clone(),
        ce,
        cd,
        cf,
        g,
        t,
        l,
        coh_zy,
        coh_zu,
        coh_ep,
        coh_yd,
        valid: first_run(&ok),
        s_a,
        l_c,
        err_g,
    }
}

// ---------------------------------------------------------------- boucle mesurée et jackknife (étape (c))

/// Marges dont on mesure l'écart-type par jackknife.
pub const STD_KEYS: [&str; 4] = [
    "sensitivity_peak_db",
    "crossover_hz",
    "phase_margin_deg",
    "gain_margin_db",
];

fn margin_of(m: &Margins, key: &str) -> f64 {
    match key {
        "sensitivity_peak_db" => m.sensitivity_peak_db,
        "crossover_hz" => m.crossover_hz,
        "phase_margin_deg" => m.phase_margin_deg,
        _ => m.gain_margin_db,
    }
}

/// `_jackknife` : écart-type de chaque marge, un bloc de fenêtres retiré à chaque fois.
fn jackknife(spec: &LoopSpectra, lm: &LoopMetrics) -> Result<Vec<(&'static str, f64)>, String> {
    let nb = spec.n_blocks();
    let mut vals: Vec<Vec<f64>> = vec![Vec::with_capacity(nb); STD_KEYS.len()];
    for b in 0..nb {
        let est = estimate(spec, Some(b));
        let mg = margins(&spec.freqs, &est.l, Some(&est.valid))?;
        for (k, key) in STD_KEYS.iter().enumerate() {
            vals[k].push(margin_of(&mg, key));
        }
    }
    let own = Margins {
        crossover_hz: lm.crossover_hz,
        phase_margin_deg: lm.phase_margin_deg,
        gain_margin_db: lm.gain_margin_db,
        phase_crossover_hz: f64::NAN,
        sensitivity_peak_db: lm.sensitivity_peak_db,
        sensitivity_peak_hz: lm.sensitivity_peak_hz,
        sensitivity_peak_at_edge: lm.sensitivity_peak_at_edge,
    };
    Ok(STD_KEYS
        .iter()
        .zip(vals)
        .map(|(key, v)| {
            let out = if margin_of(&own, key).is_finite() && v.iter().all(|x| x.is_finite()) {
                let n = v.len() as f64;
                let mean = (0.0 + otb_dsp::pairwise_sum(&v)) / n;
                let sq: Vec<f64> = v.iter().map(|x| (x - mean) * (x - mean)).collect();
                // `np.sqrt((nb - 1) / nb * np.sum(...))`
                ((nb as f64 - 1.0) / nb as f64 * (0.0 + otb_dsp::pairwise_sum(&sq))).sqrt()
            } else {
                f64::NAN // la mesure n'existe pas dans tous les sous-ensembles
            };
            (*key, out)
        })
        .collect())
}

/// `loop_from_terms` : boucle ouverte mesurée. `approx` (méthode approchée, même période) fournit la bande passante et le pic de suivi,
/// et sert de repli, avec une note, si la mesure est impossible.
// Signature calquée sur celle de l'étalon : `otb-parity` rejoue ces fonctions par leur nom avec leurs arguments
// dans le même ordre, et les regrouper en structure casserait cette correspondance.
#[allow(clippy::too_many_arguments)]
pub fn loop_from_terms(
    r: &[f64],
    y: &[f64],
    terms: &Terms,
    fs: f64,
    approx: LoopMetrics,
    seg_s: f64,
    min_input: f64,
    max_input: Option<f64>,
    breaks: Option<&[i64]>,
) -> Result<LoopMetrics, String> {
    let mut approx = approx;
    let Some(spec) = loop_spectra(r, y, terms, fs, seg_s, min_input, max_input, BLOCKS, breaks)? else {
        approx
            .notes
            .push("pas assez de fenêtres pour mesurer la boucle ouverte".into());
        return Ok(approx);
    };
    let est = estimate(&spec, None);
    if est.valid.iter().filter(|&&v| v).count() < 5 {
        approx
            .notes
            .push("cohérence insuffisante pour mesurer la boucle ouverte".into());
        return Ok(approx);
    }
    let mut lm = LoopMetrics::new(&spec.freqs, &est.t, &est.coh_zy);
    lm.method = MEASURED;
    lm.bandwidth_hz = approx.bandwidth_hz;
    lm.tracking_peak_db = approx.tracking_peak_db;
    lm.l = est.l.clone();
    lm.valid = est.valid.clone();
    let fv: Vec<f64> = spec
        .freqs
        .iter()
        .zip(&est.valid)
        .filter(|(_, v)| **v)
        .map(|(f, _)| *f)
        .collect();
    lm.band_hz = (fv[0], fv[fv.len() - 1]);
    lm.fill(&margins(&spec.freqs, &est.l, Some(&est.valid))?);
    if lm.crossover_hz.is_finite() {
        // np.argmin(np.abs(freqs − crossover)) : premier minimum ; un NaN l'emporterait mais les fréquences sont finies
        let k = (0..spec.freqs.len()).fold(0, |best, i| {
            if (spec.freqs[i] - lm.crossover_hz).abs() < (spec.freqs[best] - lm.crossover_hz).abs() {
                i
            } else {
                best
            }
        });
        lm.d_ratio = abs(cdiv(est.cd[k], est.ce[k]));
    }
    if spec.n_blocks() >= 3 {
        lm.std = jackknife(&spec, &lm)?;
    }
    Ok(lm)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Python : `LoopSpectra.total()` de blocs valant tous −0,0 rend +0,0 (la somme de numpy part de +0,0) ; mesuré avec numpy 2.
    #[test]
    fn la_somme_des_blocs_part_de_plus_zero() {
        let block = vec![(0.0, -0.0); 4];
        let spec = LoopSpectra {
            freqs: vec![0.0, 1.0, 2.0, 3.0],
            fs: 1000.0,
            blocks: vec![vec![block.clone(), block.clone(), block]],
            n_windows: 5,
            has_d: true,
        };
        for leave_out in [None, Some(1)] {
            for z in spec.total(leave_out).concat() {
                assert!(z.0.is_sign_positive() && z.1.is_sign_positive(), "{z:?}");
            }
        }
    }
}
