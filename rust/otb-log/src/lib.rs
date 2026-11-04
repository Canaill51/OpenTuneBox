//! Ouverture d'un log (D-031, phase 3a) : port des deux passes numpy qui dominaient `FlightLog.from_session`
//! (`_drop_implausible` et `axes()` / `col()`). La matrice décodée est rangée ligne par ligne : un parcours par colonne
//! (numpy) touche une ligne de cache par valeur, une seule passe sur les lignes les lit toutes une fois.
//!
//! Les conversions reproduisent celles de numpy (modulo mathématique, addition entière qui déborde sans erreur,
//! conversion entier → float32 arrondie au plus proche, division float32), pas celles « naturelles » de Rust.

pub mod debug_tables;
pub mod flightlog;
pub mod modes;
pub mod settings;

/// Colonne ignorée (`loopIteration`, `time`) : ni ramenée aux 32 bits ni contrôlée.
pub const KIND_SKIP: u8 = 0;
/// Gyro ou consigne : trame rejetée si |valeur| > 4000 · hr.
pub const KIND_GYRO: u8 = 1;
/// Terme PID : trame rejetée si |valeur| > 65535.
pub const KIND_PID: u8 = 2;
/// Autre colonne : ramenée aux 32 bits si besoin, jamais rejetée.
pub const KIND_WRAP: u8 = 3;

const I32_MIN: i64 = -(1 << 31);
const TWO_31: i64 = 1 << 31;
const TWO_32: i64 = 1 << 32;
const PID_LIMIT: i64 = 65535;

fn shape(len: usize, nrows: usize, ncols: usize) -> Result<(), String> {
    match nrows.checked_mul(ncols) {
        Some(n) if n == len => Ok(()),
        _ => Err(format!("tableau de {len} valeurs pour {nrows} × {ncols}")),
    }
}

/// Écarte les trames aux valeurs impossibles : renvoie `bad` (une case par ligne, vrai = à rejeter) et ramène `main`
/// aux entiers 32 bits **sur place**, colonne par colonne et seulement si elle sort de la plage
/// (`((c + 2³¹) mod 2³²) − 2³¹`, comme le firmware). Le filtrage des lignes reste à l'appelant.
pub fn drop_implausible(
    main: &mut [i64],
    nrows: usize,
    ncols: usize,
    kinds: &[u8],
    hr: f64,
) -> Result<Vec<bool>, String> {
    shape(main.len(), nrows, ncols)?;
    if kinds.len() != ncols {
        return Err(format!("kinds : {} valeurs pour {ncols} colonnes", kinds.len()));
    }
    if kinds.iter().any(|&k| k > KIND_WRAP) {
        return Err("kinds : valeur inconnue (0 à 3 attendu)".into());
    }
    let mut bad = vec![false; nrows];
    if nrows == 0 || ncols == 0 {
        return Ok(bad);
    }
    // 1. min et max de chaque colonne, en une passe sur les lignes
    let (mut lo, mut hi) = (vec![i64::MAX; ncols], vec![i64::MIN; ncols]);
    for row in main.chunks_exact(ncols) {
        for c in 0..ncols {
            lo[c] = lo[c].min(row[c]);
            hi[c] = hi[c].max(row[c]);
        }
    }
    let wrap: Vec<usize> = (0..ncols)
        .filter(|&c| kinds[c] != KIND_SKIP && (lo[c] < I32_MIN || hi[c] >= TWO_31))
        .collect();
    let gyro: Vec<usize> = (0..ncols).filter(|&c| kinds[c] == KIND_GYRO).collect();
    let pid: Vec<usize> = (0..ncols).filter(|&c| kinds[c] == KIND_PID).collect();
    let gyro_limit = 4000.0 * hr;
    // 2. ramener les colonnes hors plage, puis marquer les trames impossibles
    for (row, b) in main.chunks_exact_mut(ncols).zip(bad.iter_mut()) {
        for &c in &wrap {
            row[c] = row[c].wrapping_add(TWO_31).rem_euclid(TWO_32) - TWO_31;
        }
        let mut is_bad = false;
        for &c in &gyro {
            is_bad |= (row[c].unsigned_abs() as f64) > gyro_limit;
        }
        for &c in &pid {
            is_bad |= row[c].unsigned_abs() > PID_LIMIT as u64;
        }
        *b = is_bad;
    }
    Ok(bad)
}

/// Colonnes `cols` de `src` (`nrows × ncols`, rangée ligne par ligne) en float32, une ligne de sortie par colonne,
/// chacune avec **son** échelle : `out[i][j] = float32(src[j][cols[i]])` puis, si `scales[i] != 1`, divisé par
/// `float32(scales[i])`. Une colonne absente (`-1`) donne des zéros. Résultat rangé `k × nrows`.
/// Toutes les colonnes d'une ligne sont lues ensemble : une seule passe sur la matrice, au lieu d'une par groupe.
pub fn rows_f32_scaled(
    src: &[i64],
    nrows: usize,
    ncols: usize,
    cols: &[i64],
    scales: &[f64],
) -> Result<Vec<f32>, String> {
    shape(src.len(), nrows, ncols)?;
    if scales.len() != cols.len() {
        return Err(format!(
            "scales : {} valeurs pour {} colonnes",
            scales.len(),
            cols.len()
        ));
    }
    let mut idx = Vec::with_capacity(cols.len());
    for &c in cols {
        if c < -1 || (c >= 0 && c as usize >= ncols) {
            return Err(format!("colonne {c} hors du tableau ({ncols} colonnes)"));
        }
        idx.push(if c < 0 { None } else { Some(c as usize) });
    }
    let size = cols
        .len()
        .checked_mul(nrows)
        .ok_or("sortie trop grande : colonnes × lignes déborde")?;
    let mut out = vec![0.0f32; size];
    if nrows == 0 {
        return Ok(out);
    }
    for (j, row) in src.chunks_exact(ncols.max(1)).take(nrows).enumerate() {
        for (i, c) in idx.iter().enumerate() {
            if let Some(c) = c {
                out[i * nrows + j] = row[*c] as f32;
            }
        }
    }
    for (i, &scale) in scales.iter().enumerate() {
        if scale != 1.0 {
            let s = scale as f32;
            out[i * nrows..(i + 1) * nrows].iter_mut().for_each(|v| *v /= s);
        }
    }
    Ok(out)
}

/// Comme [`rows_f32_scaled`] avec la même échelle pour toutes les colonnes.
pub fn rows_f32(src: &[i64], nrows: usize, ncols: usize, cols: &[i64], scale: f64) -> Result<Vec<f32>, String> {
    rows_f32_scaled(src, nrows, ncols, cols, &vec![scale; cols.len()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_inside_the_32_bit_range_are_left_alone() {
        let mut m = vec![1, 2, 3, -4, 5, 6];
        let before = m.clone();
        let bad = drop_implausible(&mut m, 2, 3, &[KIND_SKIP, KIND_WRAP, KIND_WRAP], 1.0).unwrap();
        assert_eq!(m, before);
        assert_eq!(bad, [false, false]);
    }

    #[test]
    fn out_of_range_column_is_wrapped_as_a_whole_like_the_firmware() {
        // 2³¹ → −2³¹, 2³¹ + 5 → −2³¹ + 5, −2³¹ − 1 → 2³¹ − 1 ; une valeur dans la plage de la même colonne ne bouge pas
        let mut m = vec![TWO_31, 7, TWO_31 + 5, 7, -TWO_31 - 1, 7, 3, 7];
        drop_implausible(&mut m, 4, 2, &[KIND_WRAP, KIND_WRAP], 1.0).unwrap();
        assert_eq!(m, [-TWO_31, 7, -TWO_31 + 5, 7, TWO_31 - 1, 7, 3, 7]);
    }

    #[test]
    fn wrapping_addition_overflows_silently_like_numpy() {
        let mut m = vec![i64::MAX - 3, i64::MIN + 3];
        drop_implausible(&mut m, 2, 1, &[KIND_WRAP], 1.0).unwrap();
        // (i64::MAX − 3 + 2³¹) déborde ; le résultat suit l'arithmétique modulo 2⁶⁴ puis le modulo 2³²
        let want = |v: i64| v.wrapping_add(TWO_31).rem_euclid(TWO_32) - TWO_31;
        assert_eq!(m, [want(i64::MAX - 3), want(i64::MIN + 3)]);
    }

    #[test]
    fn skipped_columns_are_neither_wrapped_nor_checked() {
        let mut m = vec![1 << 40, 5];
        let bad = drop_implausible(&mut m, 1, 2, &[KIND_SKIP, KIND_GYRO], 1.0).unwrap();
        assert_eq!(m, [1 << 40, 5]);
        assert_eq!(bad, [false]);
    }

    #[test]
    fn rejection_thresholds_are_strict_and_scaled_by_hr() {
        // gyro : |c| > 4000·hr ; PID : |c| > 65535
        for (hr, edge) in [(1.0, 4000i64), (10.0, 40000)] {
            let mut m = vec![edge, 0, -edge, 0, edge + 1, 0, 0, 0, -(edge + 1), 0];
            let bad = drop_implausible(&mut m, 5, 2, &[KIND_GYRO, KIND_PID], hr).unwrap();
            assert_eq!(bad, [false, false, true, false, true], "hr = {hr}");
        }
        let mut m = vec![0, 65535, 0, -65535, 0, 65536, 0, -65536];
        let bad = drop_implausible(&mut m, 4, 2, &[KIND_GYRO, KIND_PID], 1.0).unwrap();
        assert_eq!(bad, [false, false, true, true]);
    }

    #[test]
    fn the_wrapped_value_is_the_one_that_is_checked() {
        // 2³² + 10 revient à 10 : plausible ; 2³¹ revient à −2³¹ : |c| = 2³¹ dépasse 4000
        let mut m = vec![TWO_32 + 10, TWO_31];
        let bad = drop_implausible(&mut m, 2, 1, &[KIND_GYRO], 1.0).unwrap();
        assert_eq!(m, [10, -TWO_31]);
        assert_eq!(bad, [false, true]);
    }

    #[test]
    fn bad_shapes_and_kinds_are_errors() {
        let mut m = vec![0i64; 6];
        assert!(drop_implausible(&mut m, 2, 3, &[0, 1], 1.0).is_err());
        assert!(drop_implausible(&mut m, 2, 3, &[0, 1, 4], 1.0).is_err());
        assert!(drop_implausible(&mut m, 4, 3, &[0, 1, 2], 1.0).is_err());
        assert!(drop_implausible(&mut m, usize::MAX, 2, &[0, 1], 1.0).is_err());
        assert_eq!(drop_implausible(&mut [], 0, 0, &[], 1.0).unwrap(), Vec::<bool>::new());
    }

    #[test]
    fn rows_are_converted_scaled_and_absent_columns_are_zero() {
        let src = vec![10, 20, 30, 40, 50, 60]; // 3 lignes × 2 colonnes
        let out = rows_f32(&src, 3, 2, &[1, -1, 0], 10.0).unwrap();
        assert_eq!(out, [2.0, 4.0, 6.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0]);
        let raw = rows_f32(&src, 3, 2, &[0], 1.0).unwrap();
        assert_eq!(raw, [10.0, 30.0, 50.0]);
    }

    #[test]
    fn integer_to_float32_rounds_to_nearest_even() {
        // 2²⁴ + 1 n'est pas représentable : arrondi au pair (2²⁴), comme `astype(float32)`
        let out = rows_f32(&[(1 << 24) + 1, (1 << 24) + 3], 2, 1, &[0], 1.0).unwrap();
        assert_eq!(out, [16777216.0, 16777220.0]);
    }

    #[test]
    fn scale_is_applied_in_float32() {
        // float32(1.5) exact ; float32(0.1) ≠ 0.1 : la division se fait par la valeur arrondie en float32
        let out = rows_f32(&[3], 1, 1, &[0], 1.5).unwrap();
        assert_eq!(out, [2.0]);
        let out = rows_f32(&[1], 1, 1, &[0], 0.1).unwrap();
        assert_eq!(out[0], 1.0f32 / 0.1f32);
    }

    #[test]
    fn each_column_has_its_own_scale_and_equals_separate_calls() {
        let src: Vec<i64> = (0..24).map(|v| v * 37 - 100).collect(); // 6 lignes × 4 colonnes
        let scales = [10.0, 1.0, 2048.0, 1.5];
        let cols = [3i64, 0, -1, 2];
        let all = rows_f32_scaled(&src, 6, 4, &cols, &scales).unwrap();
        for (i, (&c, &s)) in cols.iter().zip(&scales).enumerate() {
            let one = rows_f32(&src, 6, 4, &[c], s).unwrap();
            assert_eq!(&all[i * 6..(i + 1) * 6], &one[..], "colonne {c}");
        }
        assert!(rows_f32_scaled(&src, 6, 4, &cols, &scales[..3]).is_err());
    }

    #[test]
    fn output_size_overflow_is_an_error_not_a_short_buffer() {
        // 2 colonnes × (usize::MAX / 2 + 1) lignes déborde ; `src` vide de 0 colonne passe le contrôle de forme
        let r = rows_f32_scaled(&[], usize::MAX / 2 + 1, 0, &[-1, -1], &[1.0, 1.0]);
        assert!(r.is_err(), "{r:?}");
    }

    #[test]
    fn bad_columns_and_shapes_are_errors_and_empty_input_is_fine() {
        assert!(rows_f32(&[1, 2], 1, 2, &[2], 1.0).is_err());
        assert!(rows_f32(&[1, 2], 1, 2, &[-2], 1.0).is_err());
        assert!(rows_f32(&[1, 2, 3], 1, 2, &[0], 1.0).is_err());
        assert_eq!(rows_f32(&[], 0, 3, &[0, 1], 2.0).unwrap(), Vec::<f32>::new());
    }
}
