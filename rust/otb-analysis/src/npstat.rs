//! Statistiques de numpy sur des tableaux `float32` (phase 4c-1) : somme par paires, moyenne, écart-type, médiane, centile
//! (interpolation linéaire), histogramme à bornes données, `digitize`, `linspace`.
//!
//! Les courbes d'un journal sont en `float32` (D-029). NumPy 2 (NEP 50) calcule alors **dans cette précision** les
//! sommes, les moyennes, les écarts-types et les interpolations (un nombre Python mélangé à un tableau `float32` est
//! converti en `float32` d'abord) ; seules les divisions par le nombre d'éléments passent par `float64` (le nombre est un
//! `np.intp`, donc fort) avant d'être ramenées en `float32`. Voir L-041 et L-042.

/// `FLOAT_pairwise_sum` de numpy : blocs de 8 accumulateurs jusqu'à 128 valeurs, coupure en deux moitiés (multiple de 8)
/// au-delà. Même algorithme que [`otb_dsp::pairwise_sum`], en simple précision.
pub fn pairwise_sum_f32(a: &[f32]) -> f32 {
    let n = a.len();
    if n < 8 {
        return a.iter().fold(-0.0, |s, &v| s + v);
    }
    if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let body = n - (n % 8);
        let mut i = 8;
        while i < body {
            for (j, rj) in r.iter_mut().enumerate() {
                *rj += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        for &v in &a[i..] {
            res += v;
        }
        return res;
    }
    let mut n2 = n / 2;
    n2 -= n2 % 8;
    pairwise_sum_f32(&a[..n2]) + pairwise_sum_f32(&a[n2..])
}

/// `np.mean` d'un tableau `float32` contigu : +0,0 (identité de l'addition) plus la somme par paires, divisés en `float64`
/// par le nombre d'éléments, ramenés en `float32`. Vide → NaN.
pub fn mean_f32(a: &[f32]) -> f32 {
    if a.is_empty() {
        return f32::NAN;
    }
    ((0.0f32 + pairwise_sum_f32(a)) as f64 / a.len() as f64) as f32
}

/// `np.std` (ddof = 0) d'un tableau `float32` : moyenne, écarts élevés au carré, somme par paires, division en `float64`,
/// racine en `float32`. Vide → NaN.
pub fn std_f32(a: &[f32]) -> f32 {
    if a.is_empty() {
        return f32::NAN;
    }
    let n = a.len() as f64;
    let m = ((0.0f32 + pairwise_sum_f32(a)) as f64 / n) as f32;
    let sq: Vec<f32> = a
        .iter()
        .map(|&v| {
            let d = v - m;
            d * d
        })
        .collect();
    (((0.0f32 + pairwise_sum_f32(&sq)) as f64 / n) as f32).sqrt()
}

/// `np.max` d'un tableau `float32` : un NaN le rend NaN (contrairement à `f32::max`). `None` si vide (numpy lève).
pub fn max_f32(a: &[f32]) -> Option<f32> {
    let mut it = a.iter();
    let mut m = *it.next()?;
    for &v in it {
        if v.is_nan() || m.is_nan() {
            m = f32::NAN;
        } else if v > m {
            m = v;
        }
    }
    Some(m)
}

/// `np.min`, même règle pour les NaN.
pub fn min_f32(a: &[f32]) -> Option<f32> {
    let mut it = a.iter();
    let mut m = *it.next()?;
    for &v in it {
        if v.is_nan() || m.is_nan() {
            m = f32::NAN;
        } else if v < m {
            m = v;
        }
    }
    Some(m)
}

fn sorted_f32(a: &[f32]) -> Vec<f32> {
    let mut s = a.to_vec();
    s.sort_by(f32::total_cmp);
    s
}

/// `np.median` d'un tableau `float32` : un NaN le rend NaN ; nombre pair → moyenne (en `float64` puis `float32`) des deux
/// éléments centraux. Vide → NaN.
pub fn median_f32(a: &[f32]) -> f32 {
    if a.is_empty() || a.iter().any(|v| v.is_nan()) {
        return f32::NAN;
    }
    let s = sorted_f32(a);
    let n = s.len();
    if n % 2 == 1 {
        mean_f32(&s[n / 2..n / 2 + 1])
    } else {
        mean_f32(&s[n / 2 - 1..n / 2 + 1])
    }
}

/// `np.percentile(a, q)` d'un tableau `float32` (méthode linéaire), `q` en pourcentage (nombre Python : `float32`). Un NaN
/// le rend NaN. `a` ne doit pas être vide.
pub fn percentile_f32(a: &[f32], q: f64) -> f32 {
    if a.iter().any(|v| v.is_nan()) {
        return f32::NAN;
    }
    let s = sorted_f32(a);
    let n = s.len();
    let vi = (n - 1) as f64 * (q / 100.0);
    if vi >= (n - 1) as f64 {
        return s[n - 1]; // indices ramenés au dernier : les deux bornes sont égales
    }
    let prev = vi.floor();
    let (lo, hi) = (s[prev as usize], s[prev as usize + 1]);
    let t = vi - prev;
    let diff = hi - lo;
    let mut r = lo + diff * (t as f32);
    if t >= 0.5 {
        r = hi - diff * ((1.0 - t) as f32);
    }
    r
}

/// `np.linspace(start, stop, num)` : `i × pas + début`, le dernier point vaut `stop`.
pub fn linspace(start: f64, stop: f64, num: usize) -> Vec<f64> {
    if num == 0 {
        return Vec::new();
    }
    if num == 1 {
        return vec![start];
    }
    let div = (num - 1) as f64;
    let delta = stop - start;
    let step = delta / div;
    let mut y: Vec<f64> = (0..num)
        .map(|i| {
            if step == 0.0 {
                (i as f64 / div) * delta + start
            } else {
                i as f64 * step + start
            }
        })
        .collect();
    y[num - 1] = stop;
    y
}

/// Effectifs de `np.histogram(valeurs, bins=bornes)` : `bornes[i] ≤ v < bornes[i+1]`, la dernière case contient aussi sa
/// borne droite ; NaN et valeurs hors des bornes ne comptent pas.
pub fn histogram_counts(values: impl Iterator<Item = f64>, edges: &[f64]) -> Vec<i64> {
    let nb = edges.len() - 1;
    let mut h = vec![0i64; nb];
    for v in values {
        if v.is_nan() || v < edges[0] || v > edges[nb] {
            continue;
        }
        let i = edges.partition_point(|e| *e <= v);
        h[(i - 1).min(nb - 1)] += 1;
    }
    h
}

/// `np.digitize(x, bins)` pour des bornes croissantes : nombre de bornes `≤ x` (un NaN les dépasse toutes).
pub fn digitize(x: f64, bins: &[f64]) -> usize {
    if x.is_nan() {
        bins.len()
    } else {
        bins.partition_point(|b| *b <= x)
    }
}

/// `np.clip(x, lo, hi)` en `float32` (bornes Python entières : converties en `float32`) ; un NaN reste NaN.
pub fn clip_f32(x: f32, lo: f32, hi: f32) -> f32 {
    if x.is_nan() {
        x
    } else if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}
