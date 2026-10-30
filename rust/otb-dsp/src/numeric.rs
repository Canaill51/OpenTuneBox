//! Primitives numériques de `analysis/_numeric.py` (D-035, phase 4b-1) : fenêtre de Hann, moyenne glissante, flou
//! gaussien, intercorrélation par FFT.
//!
//! Même ordre des opérations que numpy pour rendre les mêmes bits (`hann`, `uniform_filter1d`, `gaussian_filter` :
//! identiques au bit près, vérifié sur `tests/data/numeric.json`) ; `xcorr_full` passe par une FFT et s'écarte de numpy
//! de ≈ 1e-15 du plus grand élément (mesuré en 4b-0). Les tableaux sont à plat, rangés ligne par ligne (ordre C), avec
//! leur forme. Une entrée float32 est convertie en float64 avant (exact) et le résultat arrondi en float32 après, par
//! l'appelant (`as f32`, arrondi au plus proche comme `astype`).

use crate::pairwise_sum;
use realfft::RealFftPlanner;
use realfft::num_complex::Complex;
use std::f64::consts::PI;

/// Fenêtre de Hann (`hann`) : symétrique, ou périodique avec `sym = false`. Longueur < 1 : vide ; 1 : `[1.0]`.
pub fn hann(length: i64, sym: bool) -> Vec<f64> {
    if length < 1 {
        return Vec::new();
    }
    if length == 1 {
        return vec![1.0];
    }
    let m = if sym { length } else { length + 1 };
    let mut w: Vec<f64> = (0..m)
        .map(|k| 0.5 - 0.5 * (2.0 * PI * k as f64 / (m - 1) as f64).cos())
        .collect();
    if !sym {
        w.pop();
    }
    w
}

/// Découpe `shape` autour de `axis` : (nombre de blocs avant, longueur de l'axe, pas entre deux éléments d'une ligne).
fn split(shape: &[usize], axis: i64) -> Result<(usize, usize, usize), String> {
    let nd = shape.len() as i64;
    let ax = if axis < 0 { axis + nd } else { axis };
    if ax < 0 || ax >= nd {
        return Err(format!("axe {axis} hors de la forme {shape:?}"));
    }
    let ax = ax as usize;
    Ok((
        shape[..ax].iter().product(),
        shape[ax],
        shape[ax + 1..].iter().product(),
    ))
}

/// Applique `f` à chaque ligne de l'axe : `f` lit la ligne (copiée à plat) et écrit la ligne de sortie.
fn each_line(a: &[f64], shape: &[usize], axis: i64, mut f: impl FnMut(&[f64], &mut [f64])) -> Result<Vec<f64>, String> {
    let (outer, n, inner) = split(shape, axis)?;
    if a.len() != shape.iter().product::<usize>() {
        return Err(format!("{} valeurs pour la forme {shape:?}", a.len()));
    }
    let mut out = vec![0.0; a.len()];
    let (mut line, mut res) = (vec![0.0; n], vec![0.0; n]);
    for o in 0..outer {
        for i in 0..inner {
            let at = |k: usize| (o * n + k) * inner + i;
            for (k, v) in line.iter_mut().enumerate() {
                *v = a[at(k)];
            }
            f(&line, &mut res);
            for (k, v) in res.iter().enumerate() {
                out[at(k)] = *v;
            }
        }
    }
    Ok(out)
}

/// Moyenne glissante de largeur `size`, bord répété (`uniform_filter1d`, `scipy.ndimage` en mode `nearest`).
///
/// Largeur paire : la fenêtre de la sortie i couvre [i − size/2, i + size/2 − 1]. Calcul par somme cumulée, comme Python
/// (pas de moyenne recalculée) : un NaN ou un infini se propage jusqu'à la fin de la ligne. Erreur si `size < 1`.
pub fn uniform_filter1d(a: &[f64], shape: &[usize], size: i64, axis: i64) -> Result<Vec<f64>, String> {
    if size < 1 {
        return Err("size doit être au moins 1".into());
    }
    let size = size as usize;
    each_line(a, shape, axis, |x, out| {
        let n = x.len();
        if size == 1 || n == 0 {
            out.copy_from_slice(x);
            return;
        }
        let left = size / 2;
        let right = size - 1 - left;
        // somme cumulée du tableau complété ; csum[0] = 0 (zéro ajouté devant), csum[k+1] = somme des k+1 premiers
        let total = left + n + right;
        let mut csum = Vec::with_capacity(total + 1);
        csum.push(0.0);
        let mut run = 0.0;
        for k in 0..total {
            let v = x[k.saturating_sub(left).min(n - 1)];
            // np.cumsum recopie le premier élément tel quel (garde un −0.0)
            run = if k == 0 { v } else { run + v };
            csum.push(run);
        }
        for (i, o) in out.iter_mut().enumerate() {
            *o = (csum[size + i] - csum[i]) / size as f64;
        }
    })
}

pub fn gaussian_kernel(sigma: f64) -> Vec<f64> {
    let radius = (4.0 * sigma + 0.5) as i64;
    let k: Vec<f64> = (-radius..=radius)
        .map(|x| {
            let x = x as f64;
            (-0.5 * x * x / (sigma * sigma)).exp()
        })
        .collect();
    let sum = pairwise_sum(&k);
    k.iter().map(|v| v / sum).collect()
}

/// Flou gaussien séparable, bord répété (`gaussian_filter`, `truncate = 4`) : un sigma par axe, un sigma ≤ 1e-15 laisse
/// l'axe intact. Chaque ligne cumule `noyau[j] × valeur` dans l'ordre des j, en partant de 0.0, comme Python.
pub fn gaussian_filter(a: &[f64], shape: &[usize], sigma: &[f64]) -> Vec<f64> {
    assert_eq!(sigma.len(), shape.len(), "un sigma par axe");
    let mut cur = a.to_vec();
    for (axis, &s) in sigma.iter().enumerate() {
        if s > 1e-15 {
            let kernel = gaussian_kernel(s);
            let radius = kernel.len() / 2;
            cur = each_line(&cur, shape, axis as i64, |x, out| {
                let n = x.len();
                if n == 0 {
                    return;
                }
                let at = |k: usize| x[k.saturating_sub(radius).min(n - 1)];
                for (i, o) in out.iter_mut().enumerate() {
                    let mut acc = 0.0;
                    for (j, w) in kernel.iter().enumerate() {
                        acc += w * at(i + j);
                    }
                    *o = acc;
                }
            })
            .expect("forme cohérente");
        }
    }
    cur
}

/// Spectre d'un signal réel complété de zéros jusqu'à `nfft`.
fn rfft_padded(planner: &mut RealFftPlanner<f64>, x: &[f64], nfft: usize) -> Vec<Complex<f64>> {
    let mut buf = vec![0.0; nfft];
    buf[..x.len()].copy_from_slice(x);
    let plan = planner.plan_fft_forward(nfft);
    let mut spec = plan.make_output_vec();
    plan.process(&mut buf, &mut spec).expect("longueurs cohérentes");
    spec
}

/// Intercorrélation complète par FFT et ses décalages (`xcorr_full`) ; le pic est au décalage positif quand `y` retarde
/// sur `x`. Erreur si un signal est vide.
pub fn xcorr_full(y: &[f64], x: &[f64]) -> Result<(Vec<f64>, Vec<i64>), String> {
    if x.is_empty() || y.is_empty() {
        return Err("xcorr_full : signal vide".into());
    }
    let n = y.len() + x.len() - 1;
    let nfft = 1usize << (usize::BITS - (n - 1).leading_zeros());
    let mut planner = RealFftPlanner::<f64>::new();
    let (sy, sx) = (rfft_padded(&mut planner, y, nfft), rfft_padded(&mut planner, x, nfft));
    let mut prod: Vec<Complex<f64>> = sy.iter().zip(&sx).map(|(a, b)| a * b.conj()).collect();
    // irfft de numpy ignore la partie imaginaire du continu et de Nyquist
    prod[0].im = 0.0;
    if nfft.is_multiple_of(2) {
        let last = prod.len() - 1;
        prod[last].im = 0.0;
    }
    let inv = planner.plan_fft_inverse(nfft);
    let mut corr = inv.make_output_vec();
    inv.process(&mut prod, &mut corr).map_err(|e| e.to_string())?;
    for v in corr.iter_mut() {
        *v /= nfft as f64;
    }
    // la corrélation est circulaire : les décalages négatifs sont en fin de tableau
    let mut out = Vec::with_capacity(n);
    if x.len() > 1 {
        out.extend_from_slice(&corr[nfft - (x.len() - 1)..]);
    }
    out.extend_from_slice(&corr[..y.len()]);
    let lags = (-(x.len() as i64 - 1)..y.len() as i64).collect();
    Ok((out, lags))
}

pub use realfft::num_complex::Complex as Complex64;

/// `np.fft.rfft(a, axis=1)` d'une matrice `rows × len` rangée ligne par ligne : `rows × (len / 2 + 1)` valeurs complexes.
/// `len` doit être au moins 1 (numpy lève une erreur pour 0).
pub fn rfft_rows(data: &[f64], rows: usize, len: usize) -> Vec<Complex64<f64>> {
    assert!(len >= 1 && data.len() == rows * len, "matrice {rows} × {len}");
    let plan = RealFftPlanner::<f64>::new().plan_fft_forward(len);
    let nf = len / 2 + 1;
    let mut out = vec![Complex64::new(0.0, 0.0); rows * nf];
    let mut buf = vec![0.0; len];
    for r in 0..rows {
        buf.copy_from_slice(&data[r * len..(r + 1) * len]);
        plan.process(&mut buf, &mut out[r * nf..(r + 1) * nf])
            .expect("longueurs cohérentes");
    }
    out
}
