//! Boucles par fenêtre de `analysis/motors.py` (D-031, phase 3b). Rust pur : le pont numpy est dans `otb-py`.
//!
//! Chaque noyau reproduit **au bit près** le code numpy d'origine (`tests/ref_motors.py`) : mêmes expressions dans le
//! même ordre (pas de réécriture algébrique, Rust ne fusionne pas en FMA), mêmes sommes (voir [`numpy_mean`] et
//! [`nan_mean_rows`]). La FFT et le découpage en fenêtres restent côté Python (numpy, déjà en C).

pub mod bf_filters;
pub mod datasets;
pub mod npstat;
pub mod period;
pub mod sliders;
pub mod spectral;
pub mod stats;

use otb_dsp::{K_NOTCH, apply_stage, biquad, pairwise_sum, unit_delays};
use rayon::prelude::*;
use std::ops::{Add, Div, Mul, Rem, Sub};

/// Précision d'un tableau de fréquences moteur (`f32` ou `f64`).
/// ⚠️ Sur de vrais logs `motor_hz` est en float32 (D-029) : avec NumPy 2 (NEP 50) l'ancien code calcule alors
/// `h * lo_m[k]`, `marge * lo_m[k]`, `fs - b`, `1,3 * fc`… **dans cette précision** (un scalaire Python prend celle
/// du tableau, comparaisons comprises), puis compare au tableau `f` en float64. Un noyau tout float64 décalerait
/// les bandes de quelques bits (L-041). `of` reproduit cette conversion d'un scalaire Python.
pub trait Real:
    Copy
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Rem<Output = Self>
{
    fn of(x: f64) -> Self;
    fn wide(self) -> f64;
    fn floor(self) -> Self;
    fn is_finite(self) -> bool;
}

macro_rules! real {
    ($t:ty) => {
        impl Real for $t {
            fn of(x: f64) -> Self {
                x as $t
            }
            fn wide(self) -> f64 {
                self as f64
            }
            fn floor(self) -> Self {
                <$t>::floor(self)
            }
            fn is_finite(self) -> bool {
                <$t>::is_finite(self)
            }
        }
    };
}
real!(f32);
real!(f64);

/// Plafond du nombre d'harmoniques : au-delà, l'allocation `n_harm × fenêtres` pourrait terminer le processus
/// (un échec d'allocation n'est pas une panique rattrapable). L'application en utilise 8 au plus.
const MAX_HARMONICS: usize = 1000;

/// Plafond du nombre d'images repliées par bande : au-delà, l'étalon ne finirait pratiquement jamais.
const MAX_ALIASES: usize = 1_000_000;

/// `np.mean` d'un tableau 1D contigu de float64 : +0,0 (identité de l'addition) plus la somme par paires de **tous** les
/// éléments (NumPy 2.5 ne traite pas le premier à part), divisés par leur nombre. Vide → NaN. Le +0,0 donne +0,0 pour
/// une somme de zéros négatifs, comme numpy. Vérifié bit à bit sur les longueurs 0 à 2000 (L-042).
pub fn numpy_mean(a: &[f64]) -> f64 {
    if a.is_empty() {
        return f64::NAN;
    }
    (0.0 + pairwise_sum(a)) / (a.len() as f64)
}

/// `np.nanmean(P, axis=0)` d'une matrice (lignes de `nf` valeurs) : somme **séquentielle** des lignes, NaN
/// remplacés par 0, divisée par le nombre de valeurs non NaN (0/0 = NaN). `row(k, buf)` remplit la ligne `k`.
fn nan_mean_rows(nw: usize, nf: usize, mut row: impl FnMut(usize, &mut [f64])) -> Vec<f64> {
    let (mut tot, mut cnt) = (vec![0.0_f64; nf], vec![0.0_f64; nf]);
    let mut buf = vec![0.0_f64; nf];
    for k in 0..nw {
        row(k, &mut buf);
        for i in 0..nf {
            if buf[i].is_nan() {
                tot[i] += 0.0;
            } else {
                tot[i] += buf[i];
                cnt[i] += 1.0;
            }
        }
    }
    tot.iter().zip(&cnt).map(|(&t, &c)| t / c).collect()
}

/// `b // fs` de numpy (dans la précision de `b`).
/// ⚠️ Division entière de numpy (`npy_divmod`) : reste par `fmod`, quotient corrigé, puis arrondi ; pas `(b/fs).floor()`.
fn floor_div<T: Real>(b: T, fs: T) -> T {
    let zero = T::of(0.0);
    let m = b % fs;
    let mut div = (b - m) / fs;
    if m != zero && ((fs < zero) != (m < zero)) {
        div = div - T::of(1.0);
    }
    if div == zero {
        return if (b / fs).wide().is_sign_negative() {
            T::of(-0.0)
        } else {
            zero
        };
    }
    let fl = div.floor();
    if div - fl > T::of(0.5) { fl + T::of(1.0) } else { fl }
}

/// Nombre d'images repliées à parcourir pour la borne haute `b` : l'étalon boucle sur
/// `range(1, int(b // fs) + 2)`. Erreur si `b` ou `fs` rendent l'étalon incalculable (`int(nan)` lève).
fn alias_count<T: Real>(b: T, fs: T) -> Result<usize, String> {
    if !b.is_finite() || !fs.is_finite() || fs <= T::of(0.0) {
        return Err("fs ou borne de bande non finis (fs > 0 attendu)".into());
    }
    let q = floor_div(b, fs).wide();
    if q >= MAX_ALIASES as f64 {
        return Err("trop d'images repliées (fs trop petit devant la bande)".into());
    }
    // int() tronque vers zéro ; range(1, n) est vide pour n ≤ 1
    let n = q as i64 + 2;
    Ok(if n > 1 { (n - 1) as usize } else { 0 })
}

fn check_harmonics(n_harm: usize) -> Result<(), String> {
    if n_harm > MAX_HARMONICS {
        return Err(format!("n_harm : au plus {MAX_HARMONICS} harmoniques"));
    }
    Ok(())
}

fn check_matrix(f: &[f64], p: &[f64], nw: usize, vecs: &[(&str, usize)]) -> Result<(), String> {
    if p.len() != nw * f.len() {
        return Err("P : taille différente de fenêtres × len(f)".into());
    }
    for (name, len) in vecs {
        if *len != nw {
            return Err(format!("{name} : une valeur par fenêtre attendue"));
        }
    }
    Ok(())
}

/// Met à NaN les cases de `row` dont la fréquence est dans [lo, hi] (comparaisons fausses si NaN, comme numpy).
fn blank(row: &mut [f64], f: &[f64], lo: f64, hi: f64) {
    for (v, &fi) in row.iter_mut().zip(f) {
        if fi >= lo && fi <= hi {
            *v = f64::NAN;
        }
    }
}

/// Réponse complexe (parties réelle et imaginaire, `nw` lignes de `f.len()` valeurs) des notchs RPM du firmware.
/// `fk`, `wk` : `ns` étages par fenêtre (moteur puis harmonique) ; un poids ≤ 0 (ou NaN) omet l'étage. Fenêtres en
/// parallèle, chacune exacte. Le module est pris par l'appelant : ⚠️ `np.abs` d'un tableau complexe n'est pas le
/// `hypot` de la libm (numpy utilise une version SIMD, avec FMA selon le processeur) : le refaire ici donnerait des
/// écarts d'un bit qui dépendent de la machine (L-042).
pub fn rpm_response(
    f: &[f64],
    fk: &[f64],
    wk: &[f64],
    nw: usize,
    ns: usize,
    q: f64,
    rate: f64,
) -> Result<(Vec<f64>, Vec<f64>), String> {
    if fk.len() != nw * ns || wk.len() != nw * ns {
        return Err("fk, wk : forme (fenêtres, étages) attendue".into());
    }
    let nf = f.len();
    // ⚠️ une seule cadence : z⁻¹ calculé une fois pour toutes les fenêtres (l'ancien code le refaisait à chaque fenêtre)
    let (zr, zi) = unit_delays(f, rate);
    let (mut re, mut im) = (vec![1.0_f64; nw * nf], vec![0.0_f64; nw * nf]);
    if nf == 0 || ns == 0 {
        return Ok((re, im));
    }
    re.par_chunks_mut(nf)
        .zip(im.par_chunks_mut(nf))
        .enumerate()
        .for_each(|(k, (hr, hi))| {
            for s in 0..ns {
                let (fc, w) = (fk[k * ns + s], wk[k * ns + s]);
                if w > 0.0 {
                    apply_stage(hr, hi, &zr, &zi, biquad(K_NOTCH, fc, q, w, rate));
                }
            }
        });
    Ok((re, im))
}

/// Moyennes de la raie et du bruit voisin de chaque harmonique, par fenêtre (pour `harmonic_prominence`).
/// Sorties `(n_harm, nw)` rangées à plat : moyenne de la raie, moyenne du bruit, fenêtre retenue ou non.
pub struct Levels {
    pub line: Vec<f64>,
    pub floor: Vec<f64>,
    pub used: Vec<bool>,
}

#[allow(clippy::too_many_arguments)]
pub fn harmonic_levels<T: Real>(
    f: &[f64],
    p: &[f64],
    lo_m: &[T],
    hi_m: &[T],
    mean_m: &[T],
    n_harm: usize,
    min_hz: f64,
    fs: f64,
) -> Result<Levels, String> {
    let nw = lo_m.len();
    check_harmonics(n_harm)?;
    check_matrix(f, p, nw, &[("hi_m", hi_m.len()), ("mean_m", mean_m.len())])?;
    let nf = f.len();
    let mut out = Levels {
        line: vec![f64::NAN; n_harm * nw],
        floor: vec![f64::NAN; n_harm * nw],
        used: vec![false; n_harm * nw],
    };
    if nw == 0 || n_harm == 0 {
        return Ok(out);
    }
    if nf < 2 {
        return Err("f : au moins deux fréquences attendues".into());
    }
    let df = f[1] - f[0];
    let (min_t, nyq_t) = (T::of(min_hz), T::of(0.95 * fs / 2.0));
    let (c13, c07, c085, c115) = (T::of(1.3), T::of(0.7), T::of(0.85), T::of(1.15));
    let (mut line, mut floor) = (Vec::with_capacity(nf), Vec::with_capacity(nf));
    for h in 1..=n_harm {
        let ht = T::of(h as f64);
        for k in 0..nw {
            let fc = ht * mean_m[k];
            if fc < min_t || c13 * fc > nyq_t {
                continue;
            }
            let row = &p[k * nf..(k + 1) * nf];
            line.clear();
            floor.clear();
            // h·lo est calculé dans la précision du tableau, puis df (float64) l'élargit
            let (a, b) = ((ht * lo_m[k]).wide() - df, (ht * hi_m[k]).wide() + df);
            let (l1, l2, r1, r2) = (
                (c07 * fc).wide(),
                (c085 * fc).wide(),
                (c115 * fc).wide(),
                (c13 * fc).wide(),
            );
            for (i, &fi) in f.iter().enumerate() {
                if fi >= a && fi <= b {
                    line.push(row[i]);
                }
                if (fi >= l1 && fi <= l2) || (fi >= r1 && fi <= r2) {
                    floor.push(row[i]);
                }
            }
            if !line.is_empty() && floor.len() >= 2 {
                let at = (h - 1) * nw + k;
                out.line[at] = numpy_mean(&line);
                out.floor[at] = numpy_mean(&floor);
                out.used[at] = true;
            }
        }
    }
    Ok(out)
}

/// DSP moyenne (`np.nanmean` axe 0) après avoir retiré les bandes des harmoniques moteur 1…`n_harm`
/// (`[h·lo − marge·lo ; h·hi + marge·hi]`) et leurs images repliées ; équivalent de la fin de `_masked_spectrum`.
pub fn masked_mean<T: Real>(
    f: &[f64],
    p: &[f64],
    lo_m: &[T],
    hi_m: &[T],
    n_harm: usize,
    margin: f64,
    fs: f64,
) -> Result<Vec<f64>, String> {
    let nw = lo_m.len();
    check_harmonics(n_harm)?;
    check_matrix(f, p, nw, &[("hi_m", hi_m.len())])?;
    let (mt, fs_t) = (T::of(margin), T::of(fs));
    // les nombres d'images sont vérifiés avant tout calcul : l'étalon lève dans int(b // fs)
    let mut counts = Vec::with_capacity(nw * n_harm);
    for &hi in hi_m {
        for h in 1..=n_harm {
            counts.push(alias_count(T::of(h as f64) * hi + mt * hi, fs_t)?);
        }
    }
    let nf = f.len();
    Ok(nan_mean_rows(nw, nf, |k, buf| {
        buf.copy_from_slice(&p[k * nf..(k + 1) * nf]);
        for h in 1..=n_harm {
            let ht = T::of(h as f64);
            let a = ht * lo_m[k] - mt * lo_m[k];
            let b = ht * hi_m[k] + mt * hi_m[k];
            blank(buf, f, a.wide(), b.wide());
            for j in 1..=counts[k * n_harm + h - 1] {
                // j·fs est un flottant Python (float64), qui prend ensuite la précision du tableau
                let jfs = T::of(j as f64 * fs);
                blank(buf, f, (jfs - b).wide(), (jfs - a).wide());
                blank(buf, f, (a - jfs).wide(), (b - jfs).wide());
            }
        }
    }))
}

/// Idem pour la fin de `rpm_filtered_spectrum` : seule l'image repliée `[fs − b ; fs − a]` de chaque harmonique est
/// retirée (`a = h·lo − lobe`, `b = h·hi + lobe`).
pub fn fold_mean<T: Real>(
    f: &[f64],
    p: &[f64],
    lo_m: &[T],
    hi_m: &[T],
    n_harm: usize,
    lobe: f64,
    fs: f64,
) -> Result<Vec<f64>, String> {
    let nw = lo_m.len();
    check_harmonics(n_harm)?;
    check_matrix(f, p, nw, &[("hi_m", hi_m.len())])?;
    let (lobe_t, fs_t) = (T::of(lobe), T::of(fs));
    let nf = f.len();
    Ok(nan_mean_rows(nw, nf, |k, buf| {
        buf.copy_from_slice(&p[k * nf..(k + 1) * nf]);
        for h in 1..=n_harm {
            let ht = T::of(h as f64);
            let (a, b) = (ht * lo_m[k] - lobe_t, ht * hi_m[k] + lobe_t);
            blank(buf, f, (fs_t - b).wide(), (fs_t - a).wide());
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_small() {
        assert!(numpy_mean(&[]).is_nan());
        assert_eq!(numpy_mean(&[2.0]), 2.0);
        assert_eq!(numpy_mean(&[1.0, 2.0, 6.0]), 3.0);
    }

    #[test]
    fn floor_div_like_python() {
        assert_eq!(floor_div(7.0_f64, 2.0), 3.0);
        assert_eq!(floor_div(-7.0_f64, 2.0), -4.0);
        assert_eq!(floor_div(0.3_f64, 0.1), 2.0); // 0.3 // 0.1 vaut 2.0 en Python
        assert_eq!(floor_div(0.3_f32, 0.1), 3.0); // np.float32(0.3) // np.float32(0.1) vaut 3.0
        assert_eq!(alias_count(1.0_f64, 4000.0), Ok(1));
        assert!(alias_count(f64::NAN, 4000.0).is_err());
        assert!(alias_count(10.0_f64, 0.0).is_err());
    }

    #[test]
    fn response_without_stage_is_one() {
        let (re, im) = rpm_response(&[10.0, 20.0], &[100.0], &[0.0], 1, 1, 5.0, 8000.0).unwrap();
        assert_eq!((re, im), (vec![1.0, 1.0], vec![0.0, 0.0]));
    }

    #[test]
    fn rejects_bad_shapes() {
        assert!(masked_mean(&[1.0, 2.0], &[1.0], &[1.0_f64], &[1.0], 1, 0.05, 4000.0).is_err());
        assert!(rpm_response(&[1.0], &[1.0, 2.0], &[1.0], 1, 1, 5.0, 8000.0).is_err());
    }
}
