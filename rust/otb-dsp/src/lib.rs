//! Noyaux de calcul d'OpenTuneBox (D-031, phase 2a) : étages biquad des filtres Betaflight.
//!
//! Port des noyaux numba de `analysis/bf_filters.py` (`_biquad`, `_sections_response`, `_run_section`,
//! `_pt1_track`). Rust pur, sans dépendance : le pont numpy est dans `otb-py`.

use std::f64::consts::PI;

pub mod step;

/// Codes des types d'étage (les mêmes que `_K_PT1`, `_K_LPF`, `_K_NOTCH` de `bf_filters.py`).
pub const K_PT1: i64 = 0;
pub const K_LPF: i64 = 1;
pub const K_NOTCH: i64 = 2;

/// Gain plancher d'un étage (|H|² ≥ 1e-30).
/// ⚠️ Au centre exact d'un notch, un vrai zéro rendrait nul le rapport |H_mod|²/|H_actuel|² du simulateur
/// de filtres (L-024) : même plancher dans les deux chaînes comparées.
pub const MIN_STAGE_GAIN: f64 = 1e-15;

/// Coefficients (b0, b1, b2, a1, a2) d'un étage ; `f ≤ 0` → étage transparent.
/// Un type inconnu est traité comme un notch, comme dans la version numba.
pub fn biquad(kind: i64, f: f64, q: f64, w: f64, fs: f64) -> [f64; 5] {
    if f <= 0.0 {
        return [1.0, 0.0, 0.0, 0.0, 0.0];
    }
    let omega = 2.0 * PI * f / fs;
    if kind == K_PT1 {
        let k = omega / (omega + 1.0);
        return [k, 0.0, 0.0, -(1.0 - k), 0.0];
    }
    let (sn, cs) = omega.sin_cos();
    let alpha = sn / (2.0 * q);
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cs / a0;
    let a2 = (1.0 - alpha) / a0;
    if kind == K_LPF {
        let b1 = (1.0 - cs) / a0;
        return [b1 * 0.5, b1, b1 * 0.5, a1, a2];
    }
    // notch pondéré : b = w·b_notch + (1−w)·a
    let b0 = 1.0 / a0;
    let b1 = -2.0 * cs / a0;
    let b2 = 1.0 / a0;
    [
        w * b0 + (1.0 - w),
        w * b1 + (1.0 - w) * a1,
        w * b2 + (1.0 - w) * a2,
        a1,
        a2,
    ]
}

/// Étages d'une chaîne : un élément par étage dans chaque tableau.
pub struct Sections<'a> {
    pub kinds: &'a [i64],
    pub fcs: &'a [f64],
    pub qs: &'a [f64],
    pub ws: &'a [f64],
    pub rates: &'a [f64],
}

/// Réponse complexe (parties réelle et imaginaire) du produit des étages, aux fréquences `freqs` (Hz).
/// Chaque étage est évalué à sa propre cadence ; son gain n'est jamais sous [`MIN_STAGE_GAIN`].
pub fn sections_response(s: &Sections, freqs: &[f64]) -> Result<(Vec<f64>, Vec<f64>), String> {
    let n_st = s.kinds.len();
    if [s.fcs.len(), s.qs.len(), s.ws.len(), s.rates.len()]
        .iter()
        .any(|&l| l != n_st)
    {
        return Err("étages : tableaux de longueurs différentes".into());
    }
    let n = freqs.len();
    let (mut hr, mut hi) = (vec![1.0; n], vec![0.0; n]);
    // z⁻¹ = e^(−jω), recalculé seulement quand la cadence change
    let (mut zr, mut zi) = (vec![0.0; n], vec![0.0; n]);
    let mut rate = -1.0;
    for st in 0..n_st {
        if s.rates[st] != rate {
            rate = s.rates[st];
            for i in 0..n {
                (zi[i], zr[i]) = (-2.0 * PI * freqs[i] / rate).sin_cos();
            }
        }
        let [b0, b1, b2, a1, a2] = biquad(s.kinds[st], s.fcs[st], s.qs[st], s.ws[st], rate);
        for i in 0..n {
            let (z_re, z_im) = (zr[i], zi[i]);
            // num = b0 + z·(b1 + z·b2) ; den = 1 + z·(a1 + z·a2)
            let (t_re, t_im) = (b1 + (z_re * b2), z_im * b2);
            let (num_re, num_im) = (b0 + (z_re * t_re - z_im * t_im), z_re * t_im + z_im * t_re);
            let (u_re, u_im) = (a1 + (z_re * a2), z_im * a2);
            let (den_re, den_im) = (1.0 + (z_re * u_re - z_im * u_im), z_re * u_im + z_im * u_re);
            let d = den_re * den_re + den_im * den_im;
            let (mut g_re, mut g_im) = (
                (num_re * den_re + num_im * den_im) / d,
                (num_im * den_re - num_re * den_im) / d,
            );
            if g_re.hypot(g_im) < MIN_STAGE_GAIN {
                (g_re, g_im) = (MIN_STAGE_GAIN, 0.0);
            }
            let (r, im) = (hr[i], hi[i]);
            hr[i] = r * g_re - im * g_im;
            hi[i] = r * g_im + im * g_re;
        }
    }
    Ok((hr, hi))
}

/// Applique un étage biquad (forme directe I) à `x` (`n_ax` lignes de `n` valeurs, rangées à la suite),
/// coefficients variables. `freq` et `weight` : `n` valeurs ; une fréquence ≤ 0 ou ≥ `f_limit` rend l'étage
/// transparent. L'état initial est l'état stationnaire pour la première valeur (gain statique 1).
#[allow(clippy::too_many_arguments)]
pub fn run_section(
    x: &[f64],
    n_ax: usize,
    kind: i64,
    freq: &[f64],
    q: f64,
    weight: &[f64],
    fs: f64,
    f_limit: f64,
) -> Result<Vec<f64>, String> {
    let n = freq.len();
    if weight.len() != n {
        return Err("freq et weight : longueurs différentes".into());
    }
    if x.len() != n_ax * n {
        return Err(format!("x : {} valeurs, {n_ax} × {n} attendues", x.len()));
    }
    let mut y = vec![0.0; x.len()];
    if n == 0 {
        return Ok(y);
    }
    for ax in 0..n_ax {
        let row = &x[ax * n..(ax + 1) * n];
        let (mut x1, mut x2, mut y1, mut y2) = (row[0], row[0], row[0], row[0]);
        let (mut pf, mut pw) = (-1.0, -1.0);
        let mut c = [1.0, 0.0, 0.0, 0.0, 0.0];
        for i in 0..n {
            let (f, w) = (freq[i], weight[i]);
            if f != pf || w != pw {
                let fe = if f < f_limit { f } else { 0.0 };
                c = biquad(kind, fe, q, w, fs);
                (pf, pw) = (f, w);
            }
            let xi = row[i];
            let yi = c[0] * xi + c[1] * x1 + c[2] * x2 - c[3] * y1 - c[4] * y2;
            (x2, x1, y2, y1) = (x1, xi, y1, yi);
            y[ax * n + i] = yi;
        }
    }
    Ok(y)
}

/// PT1 simple sur un signal 1D (lissage de la fréquence moteur) ; état initial = première valeur.
pub fn pt1_track(x: &[f64], k: f64) -> Vec<f64> {
    let mut s = x.first().copied().unwrap_or(0.0);
    x.iter()
        .map(|&v| {
            s += k * (v - s);
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * (1.0 + b.abs())
    }

    #[test]
    fn transparent_stage_when_frequency_is_not_positive() {
        assert_eq!(biquad(K_NOTCH, 0.0, 1.0, 1.0, 8000.0), [1.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(biquad(K_LPF, -5.0, 1.0, 1.0, 8000.0), [1.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn pt1_gain_matches_formula() {
        let omega = 2.0 * PI * 100.0 / 8000.0;
        let k = omega / (omega + 1.0);
        assert_eq!(biquad(K_PT1, 100.0, 0.0, 1.0, 8000.0), [k, 0.0, 0.0, -(1.0 - k), 0.0]);
    }

    #[test]
    fn lowpass_and_notch_have_expected_dc_and_centre_gain() {
        let fs = 8000.0;
        let lp = biquad(K_LPF, 200.0, std::f64::consts::FRAC_1_SQRT_2, 1.0, fs);
        // gain en continu = (b0+b1+b2)/(1+a1+a2) = 1
        assert!(close((lp[0] + lp[1] + lp[2]) / (1.0 + lp[3] + lp[4]), 1.0));
        let notch = [K_NOTCH; 1];
        let st = Sections {
            kinds: &notch,
            fcs: &[500.0],
            qs: &[3.0],
            ws: &[1.0],
            rates: &[fs],
        };
        let (re, im) = sections_response(&st, &[500.0, 0.0]).unwrap();
        // creux exact au centre : plancher, pas zéro ; gain 1 en continu
        assert!(re[0].hypot(im[0]) < 1e-9 && re[0].hypot(im[0]) >= MIN_STAGE_GAIN);
        assert!(close(re[1], 1.0) && im[1].abs() < 1e-12);
    }

    #[test]
    fn weighted_notch_with_zero_weight_is_transparent() {
        let c = biquad(K_NOTCH, 300.0, 4.0, 0.0, 4000.0);
        // b = a → H = 1
        assert!(close(c[0], 1.0) && close(c[1], c[3]) && close(c[2], c[4]));
    }

    #[test]
    fn mixed_rates_multiply_stages() {
        let st = Sections {
            kinds: &[K_PT1, K_PT1],
            fcs: &[100.0, 100.0],
            qs: &[0.0, 0.0],
            ws: &[1.0, 1.0],
            rates: &[8000.0, 4000.0],
        };
        let one = |rate: f64| {
            let s = Sections {
                kinds: &[K_PT1],
                fcs: &[100.0],
                qs: &[0.0],
                ws: &[1.0],
                rates: &[rate],
            };
            sections_response(&s, &[123.0]).unwrap()
        };
        let (a, b) = (one(8000.0), one(4000.0));
        let (re, im) = sections_response(&st, &[123.0]).unwrap();
        assert!(close(re[0], a.0[0] * b.0[0] - a.1[0] * b.1[0]));
        assert!(close(im[0], a.0[0] * b.1[0] + a.1[0] * b.0[0]));
    }

    #[test]
    fn mismatched_lengths_are_errors() {
        let st = Sections {
            kinds: &[0],
            fcs: &[1.0, 2.0],
            qs: &[0.0],
            ws: &[1.0],
            rates: &[8000.0],
        };
        assert!(sections_response(&st, &[1.0]).is_err());
        assert!(
            run_section(
                &[1.0, 2.0, 3.0],
                1,
                0,
                &[1.0, 1.0],
                1.0,
                &[1.0, 1.0],
                8000.0,
                f64::INFINITY
            )
            .is_err()
        );
        assert!(run_section(&[1.0, 2.0], 1, 0, &[1.0, 1.0], 1.0, &[1.0], 8000.0, f64::INFINITY).is_err());
    }

    #[test]
    fn run_section_keeps_a_constant_signal_constant() {
        let x = vec![5.0; 64];
        let f = vec![150.0; 32];
        let w = vec![1.0; 32];
        let y = run_section(
            &x,
            2,
            K_LPF,
            &f,
            std::f64::consts::FRAC_1_SQRT_2,
            &w,
            8000.0,
            f64::INFINITY,
        )
        .unwrap();
        assert!(y.iter().all(|&v| close(v, 5.0)));
    }

    #[test]
    fn run_section_is_transparent_above_the_limit() {
        let x = [1.0, -2.0, 3.0, 0.5];
        let y = run_section(&x, 1, K_PT1, &[1000.0; 4], 0.0, &[1.0; 4], 2000.0, 960.0).unwrap();
        assert_eq!(y, x);
        assert!(
            run_section(&[], 3, K_PT1, &[], 0.0, &[], 2000.0, 960.0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn pt1_track_starts_at_first_value_and_converges() {
        let y = pt1_track(&[10.0, 10.0, 20.0, 20.0, 20.0], 0.5);
        assert_eq!(y[..2], [10.0, 10.0]);
        assert!(close(y[2], 15.0) && close(y[3], 17.5) && close(y[4], 18.75));
        assert!(pt1_track(&[], 0.5).is_empty());
    }
}
