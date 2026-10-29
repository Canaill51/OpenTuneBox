//! Noyau de `analysis/step.py:step_response` (D-031, phase 2b, pilote) : réponses indicielles par déconvolution de
//! Wiener, fenêtre par fenêtre. Mêmes opérations, dans le même ordre, que la boucle numpy d'origine
//! (`tests/ref_step.py`, gardée comme étalon).
//!
//! Les fenêtres se calculent en parallèle (rayon) mais les sommes se font **dans l'ordre des fenêtres** : le
//! résultat est identique quel que soit le nombre de fils (`RAYON_NUM_THREADS`).

use rayon::prelude::*;
use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::sync::Arc;

/// Fenêtres traitées ensemble : borne la mémoire, comme les paquets de 256 de la version numpy.
const BATCH: usize = 256;

/// Paramètres du calcul (tout ce qui n'est ni un signal ni la liste des fenêtres).
pub struct StepParams<'a> {
    /// Fenêtre de pondération (Hann symétrique, passée depuis Python), `l` valeurs.
    pub w: &'a [f64],
    /// Longueur d'une fenêtre d'analyse.
    pub l: usize,
    /// Longueur de la réponse gardée.
    pub lr: usize,
    /// Zéros ajoutés de chaque côté (`PAD`).
    pub pad: usize,
    /// Régularisation absolue de la déconvolution.
    pub reg: f64,
    pub y_correction: bool,
    /// Plage d'indices de la réponse où l'on contrôle la qualité ; vide = toutes les fenêtres passent.
    pub steady: (usize, usize),
}

/// Résultat : `k` fenêtres retenues, dans l'ordre de `starts`.
pub struct StepOut {
    /// `k × lr`, rangé ligne par ligne.
    pub rows: Vec<f64>,
    pub maxin: Vec<f64>,
    pub sum_gh_re: Vec<f64>,
    pub sum_gh_im: Vec<f64>,
    pub sum_hh: Vec<f64>,
    pub sum_gg: Vec<f64>,
}

/// Contribution d'une fenêtre retenue.
struct Win {
    row: Vec<f64>,
    maxin: f64,
    gh: Vec<Complex<f64>>,
    hh: Vec<f64>,
    gg: Vec<f64>,
}

/// Tampons d'un fil, réutilisés d'une fenêtre à l'autre.
struct Work {
    x: Vec<f64>,
    y: Vec<f64>,
    hr: Vec<Complex<f64>>,
    gr: Vec<Complex<f64>>,
    spec: Vec<Complex<f64>>,
    out: Vec<f64>,
    scratch_f: Vec<Complex<f64>>,
    scratch_i: Vec<Complex<f64>>,
}

/// Minimum et maximum qui propagent les NaN comme `numpy.min` / `numpy.max` (`f64::min` les ignorerait).
fn min_max(v: &[f64]) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &x in v {
        if x.is_nan() {
            return (f64::NAN, f64::NAN);
        }
        lo = lo.min(x);
        hi = hi.max(x);
    }
    (lo, hi)
}

#[allow(clippy::too_many_arguments)]
fn window(
    sp: &[f64],
    gy: &[f64],
    start: usize,
    p: &StepParams,
    fwd: &Arc<dyn RealToComplex<f64>>,
    inv: &Arc<dyn ComplexToReal<f64>>,
    wk: &mut Work,
) -> Result<Option<Win>, String> {
    let nfft = p.l + 2 * p.pad;
    let nf = nfft / 2 + 1;
    wk.x.fill(0.0);
    wk.y.fill(0.0);
    for i in 0..p.l {
        wk.x[p.pad + i] = sp[start + i] * p.w[i];
        wk.y[p.pad + i] = gy[start + i] * p.w[i];
    }
    fwd.process_with_scratch(&mut wk.x, &mut wk.hr, &mut wk.scratch_f)
        .map_err(|e| e.to_string())?;
    fwd.process_with_scratch(&mut wk.y, &mut wk.gr, &mut wk.scratch_f)
        .map_err(|e| e.to_string())?;

    // imp = irfft(G·conj(H) / (H·conj(H) + reg)) avec H = Hr/nfft, G = Gr/nfft
    let n = nfft as f64;
    for j in 0..nf {
        let (h, g) = (wk.hr[j] / n, wk.gr[j] / n);
        let num = Complex::new(g.re * h.re + g.im * h.im, -g.re * h.im + g.im * h.re);
        let den = h.re * h.re + h.im * h.im + p.reg;
        wk.spec[j] = Complex::new(num.re / den, num.im / den);
    }
    // l'inverse d'un signal réel ignore la partie imaginaire du continu et de Nyquist (comme pocketfft)
    wk.spec[0].im = 0.0;
    if nfft.is_multiple_of(2) {
        wk.spec[nf - 1].im = 0.0;
    }
    inv.process_with_scratch(&mut wk.spec, &mut wk.out, &mut wk.scratch_i)
        .map_err(|e| e.to_string())?;

    let scale = 1.0 / n;
    let mut row = Vec::with_capacity(p.lr);
    let mut acc = 0.0;
    for &v in &wk.out[..p.lr] {
        acc += v * scale;
        row.push(acc);
    }
    let (lo, hi) = p.steady;
    if hi > lo {
        let (mn, mx) = min_max(&row[lo..hi]);
        // comparaisons strictes ; un NaN rejette la fenêtre, comme `NaN > 0.5` en numpy
        if !(mn > 0.5 && mx < 3.0) {
            return Ok(None);
        }
        if p.y_correction {
            let mean = row[lo..hi].iter().sum::<f64>() / (hi - lo) as f64;
            let f = 2.0 - mean;
            row.iter_mut().for_each(|v| *v *= f);
        }
    }
    let mut maxin = 0.0f64;
    for &v in &sp[start..start + p.l] {
        if v.is_nan() {
            maxin = f64::NAN;
            break;
        }
        maxin = maxin.max(v.abs());
    }
    let gh = (0..nf)
        .map(|j| {
            let (h, g) = (wk.hr[j], wk.gr[j]);
            Complex::new(g.re * h.re + g.im * h.im, -g.re * h.im + g.im * h.re)
        })
        .collect();
    let hh = wk.hr.iter().map(|h| h.re * h.re + h.im * h.im).collect();
    let gg = wk.gr.iter().map(|g| g.re * g.re + g.im * g.im).collect();
    Ok(Some(Win { row, maxin, gh, hh, gg }))
}

/// Réponses indicielles des fenêtres `starts` de `sp` (consigne) et `gy` (gyro).
pub fn step_windows(sp: &[f64], gy: &[f64], starts: &[i64], p: &StepParams) -> Result<StepOut, String> {
    let nfft = p.l + 2 * p.pad;
    let nf = nfft / 2 + 1;
    if p.w.len() != p.l {
        return Err(format!("w : {} valeurs, {} attendues", p.w.len(), p.l));
    }
    if p.l == 0 || p.lr == 0 || p.lr > nfft {
        return Err("lr doit être entre 1 et l + 2·pad, et l non nul".into());
    }
    if p.steady.0 > p.steady.1 || p.steady.1 > p.lr {
        return Err("steady : plage hors de la réponse".into());
    }
    let n = sp.len().min(gy.len());
    let mut idx = Vec::with_capacity(starts.len());
    for &s in starts {
        if s < 0 || (s as usize).checked_add(p.l).is_none_or(|e| e > n) {
            return Err(format!("fenêtre {s} + {} hors du signal ({n} valeurs)", p.l));
        }
        idx.push(s as usize);
    }
    let mut planner = RealFftPlanner::<f64>::new();
    let fwd = planner.plan_fft_forward(nfft);
    let inv = planner.plan_fft_inverse(nfft);

    let mut out = StepOut {
        rows: Vec::new(),
        maxin: Vec::new(),
        sum_gh_re: vec![0.0; nf],
        sum_gh_im: vec![0.0; nf],
        sum_hh: vec![0.0; nf],
        sum_gg: vec![0.0; nf],
    };
    for batch in idx.chunks(BATCH) {
        let wins: Vec<Option<Win>> = batch
            .par_iter()
            .map_init(
                || Work {
                    x: fwd.make_input_vec(),
                    y: fwd.make_input_vec(),
                    hr: fwd.make_output_vec(),
                    gr: fwd.make_output_vec(),
                    spec: inv.make_input_vec(),
                    out: inv.make_output_vec(),
                    scratch_f: fwd.make_scratch_vec(),
                    scratch_i: inv.make_scratch_vec(),
                },
                |wk, &s| window(sp, gy, s, p, &fwd, &inv, wk),
            )
            .collect::<Result<_, _>>()?;
        // somme de ce paquet dans l'ordre des fenêtres, puis ajout au total (comme `np.sum(axis=0)` par paquet)
        let kept: Vec<&Win> = wins.iter().flatten().collect();
        if kept.is_empty() {
            continue;
        }
        let (mut bgh_re, mut bgh_im, mut bhh, mut bgg) = (vec![0.0; nf], vec![0.0; nf], vec![0.0; nf], vec![0.0; nf]);
        for w in &kept {
            for j in 0..nf {
                bgh_re[j] += w.gh[j].re;
                bgh_im[j] += w.gh[j].im;
                bhh[j] += w.hh[j];
                bgg[j] += w.gg[j];
            }
            out.rows.extend_from_slice(&w.row);
            out.maxin.push(w.maxin);
        }
        for j in 0..nf {
            out.sum_gh_re[j] += bgh_re[j];
            out.sum_gh_im[j] += bgh_im[j];
            out.sum_hh[j] += bhh[j];
            out.sum_gg[j] += bgg[j];
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// Suite pseudo-aléatoire reproductible (congruentiel linéaire), valeurs dans [-1, 1].
    fn noise(n: usize, mut s: u64) -> Vec<f64> {
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((s >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
            })
            .collect()
    }

    fn hann(l: usize) -> Vec<f64> {
        (0..l)
            .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / (l - 1) as f64).cos())
            .collect()
    }

    fn params(w: &[f64], l: usize, lr: usize, pad: usize) -> StepParams<'_> {
        StepParams {
            w,
            l,
            lr,
            pad,
            reg: 1e-4,
            y_correction: false,
            steady: (0, 0),
        }
    }

    /// DFT naïve de la fenêtre pondérée et complétée de zéros : |X_k|² pour k = 0..nfft/2.
    fn naive_power(x: &[f64], w: &[f64], pad: usize) -> Vec<f64> {
        let nfft = x.len() + 2 * pad;
        let mut z = vec![0.0; nfft];
        for i in 0..x.len() {
            z[pad + i] = x[i] * w[i];
        }
        (0..=nfft / 2)
            .map(|k| {
                let (mut re, mut im) = (0.0, 0.0);
                for (t, v) in z.iter().enumerate() {
                    let a = -2.0 * PI * (k * t) as f64 / nfft as f64;
                    re += v * a.cos();
                    im += v * a.sin();
                }
                re * re + im * im
            })
            .collect()
    }

    #[test]
    fn spectra_match_a_naive_dft_for_even_and_odd_sizes() {
        for (l, pad) in [(40, 5), (41, 5), (37, 0), (64, 10)] {
            let (sp, gy) = (noise(l, 1), noise(l, 2));
            let w = hann(l);
            let out = step_windows(&sp, &gy, &[0], &params(&w, l, l, pad)).unwrap();
            let (hh, gg) = (naive_power(&sp, &w, pad), naive_power(&gy, &w, pad));
            for k in 0..hh.len() {
                assert!(
                    (out.sum_hh[k] - hh[k]).abs() <= 1e-9 * (1.0 + hh[k]),
                    "hh l={l} pad={pad} k={k}"
                );
                assert!(
                    (out.sum_gg[k] - gg[k]).abs() <= 1e-9 * (1.0 + gg[k]),
                    "gg l={l} pad={pad} k={k}"
                );
            }
        }
    }

    #[test]
    fn identity_system_gives_a_unit_step_response() {
        let l = 200;
        let sp: Vec<f64> = noise(l, 3).iter().map(|v| 300.0 * v).collect();
        let w = hann(l);
        let mut p = params(&w, l, 120, 20);
        p.steady = (60, 120);
        let out = step_windows(&sp, &sp, &[0], &p).unwrap();
        assert_eq!(out.rows.len(), 120);
        // gyro = consigne : déconvolution ≈ impulsion, donc marche ≈ 1 sur le plateau
        for v in &out.rows[60..120] {
            assert!((v - 1.0).abs() < 0.05, "plateau {v}");
        }
    }

    #[test]
    fn quality_control_rejects_windows_outside_the_band_and_nan() {
        let l = 200;
        let sp: Vec<f64> = noise(l, 4).iter().map(|v| 300.0 * v).collect();
        let w = hann(l);
        let mut p = params(&w, l, 120, 20);
        p.steady = (60, 120);
        let half: Vec<f64> = sp.iter().map(|v| 0.4 * v).collect(); // plateau ≈ 0,4 < 0,5 : rejetée
        let big: Vec<f64> = sp.iter().map(|v| 3.5 * v).collect(); // plateau ≈ 3,5 > 3,0 : rejetée
        assert!(step_windows(&sp, &half, &[0], &p).unwrap().maxin.is_empty());
        assert!(step_windows(&sp, &big, &[0], &p).unwrap().maxin.is_empty());
        let mut bad = sp.clone();
        bad[10] = f64::NAN;
        assert!(step_windows(&sp, &bad, &[0], &p).unwrap().maxin.is_empty());
        assert_eq!(step_windows(&sp, &sp, &[0], &p).unwrap().maxin.len(), 1);
    }

    #[test]
    fn rows_keep_the_order_of_the_windows() {
        let l = 100;
        // l'amplitude de la consigne croît de fenêtre en fenêtre : maxin doit croître dans le même ordre
        let n = 5 * l;
        let sp: Vec<f64> = (0..n)
            .map(|i| (1 + i / l) as f64 * 50.0 * (i as f64 * 0.37).sin())
            .collect();
        let w = hann(l);
        let starts: Vec<i64> = (0..5).map(|k| (k * l) as i64).collect();
        let out = step_windows(&sp, &sp, &starts, &params(&w, l, l, 10)).unwrap();
        assert_eq!(out.maxin.len(), 5);
        assert!(out.maxin.windows(2).all(|m| m[0] < m[1]), "{:?}", out.maxin);
        assert_eq!(out.rows.len(), 5 * l);
    }

    #[test]
    fn windows_outside_the_signal_are_errors_not_panics() {
        let sp = noise(300, 5);
        let w = hann(100);
        let p = params(&w, 100, 50, 10);
        assert!(step_windows(&sp, &sp, &[201], &p).is_err());
        assert!(step_windows(&sp, &sp, &[-1], &p).is_err());
        assert!(step_windows(&sp, &sp, &[i64::MAX], &p).is_err());
        assert!(step_windows(&sp, &sp[..150], &[100], &p).is_err()); // le plus court des deux signaux fait loi
        assert!(step_windows(&sp, &sp, &[200], &p).is_ok());
        assert!(step_windows(&sp, &sp, &[], &p).unwrap().rows.is_empty());
    }

    #[test]
    fn bad_parameters_are_errors() {
        let sp = noise(300, 6);
        let w = hann(100);
        assert!(step_windows(&sp, &sp, &[0], &params(&w, 99, 50, 10)).is_err()); // w de mauvaise longueur
        assert!(step_windows(&sp, &sp, &[0], &params(&w, 100, 0, 10)).is_err()); // lr nul
        assert!(step_windows(&sp, &sp, &[0], &params(&w, 100, 121, 10)).is_err()); // lr > nfft
        let mut p = params(&w, 100, 50, 10);
        p.steady = (30, 20);
        assert!(step_windows(&sp, &sp, &[0], &p).is_err());
        p.steady = (10, 51);
        assert!(step_windows(&sp, &sp, &[0], &p).is_err());
    }
}
