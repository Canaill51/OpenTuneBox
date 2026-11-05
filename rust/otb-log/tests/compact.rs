//! `implausible_rows` et `compact_f32_scaled` (rangement compact, L-081) rendent exactement ce que rendaient
//! `drop_implausible` et `rows_f32_scaled` sur la matrice d'entiers 64 bits.

use otb_decoder::CompactTable;
use otb_log::{
    KIND_GYRO, KIND_PID, KIND_SKIP, KIND_WRAP, compact_f32_scaled, drop_implausible, implausible_rows, rows_f32_scaled,
};

/// Matrice pseudo-aléatoire (graine fixe) : valeurs ordinaires, hors plage 32 bits, gyro > 4000, PID > 65535.
fn matrix(rows: usize, cols: usize) -> Vec<i64> {
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    (0..rows * cols)
        .map(|i| {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let r = (x >> 33) as i64;
            match i % 7 {
                0 => r % 100,
                1 => r % 5000 - 2500,
                2 => r * 40_000,        // hors plage 32 bits
                3 => -(r * 31_000) - 1, // hors plage, négatif
                4 => r % 70_000,
                _ => r % 3000,
            }
        })
        .collect()
}

#[test]
fn memes_resultats_que_les_fonctions_sur_i64() {
    let (rows, cols) = (500usize, 9usize);
    let kinds = [
        KIND_SKIP, KIND_SKIP, KIND_GYRO, KIND_GYRO, KIND_PID, KIND_PID, KIND_WRAP, KIND_WRAP, KIND_GYRO,
    ];
    let raw = matrix(rows, cols);

    let mut t = CompactTable::new(cols, &[0, 1], rows);
    for r in 0..rows {
        t.push(&raw[r * cols..(r + 1) * cols]).unwrap();
    }

    let mut old = raw.clone();
    for hr in [1.0, 10.0] {
        old.copy_from_slice(&raw);
        let bad_old = drop_implausible(&mut old, rows, cols, &kinds, hr).unwrap();
        assert_eq!(
            implausible_rows(&t, &kinds, hr),
            bad_old,
            "trames impossibles, hr = {hr}"
        );
    }
    // valeurs : mêmes que la matrice ramenée sur place
    let bad_old_wrapped = {
        let mut m = raw.clone();
        drop_implausible(&mut m, rows, cols, &kinds, 1.0).unwrap();
        m
    };
    for r in 0..rows {
        for c in 0..cols {
            assert_eq!(t.get(r, c), bad_old_wrapped[r * cols + c], "valeur {r},{c}");
        }
    }
    // conversion en f32 : mêmes bits, colonnes larges et étroites mêlées, échelles différentes
    let cols_sel = [2usize, 5, 1, 8, 0];
    let scales = [1.0, 10.0, 1.0, 2048.0, 1.0];
    let attendu = rows_f32_scaled(&bad_old_wrapped, rows, cols, &cols_sel.map(|c| c as i64), &scales).unwrap();
    let obtenu = compact_f32_scaled(&t, &cols_sel, &scales);
    assert_eq!(attendu.len(), obtenu.len());
    assert!(attendu.iter().zip(&obtenu).all(|(a, b)| a.to_bits() == b.to_bits()));
}
