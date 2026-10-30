//! Banc 4c-0 (jetable) : primitives complexes et `np.gradient` de numpy contre Rust (produit, division, module, angle,
//! cumsum, dérivée à coordonnées). Entrées et attendus écrits par un script numpy ; chaque ligne dit combien de valeurs
//! sont identiques au bit près et l'écart maximal rapporté au plus grand élément.
fn read(dir: &str, name: &str) -> Vec<f64> {
    let b = std::fs::read(format!("{dir}/{name}.bin")).unwrap();
    b.as_chunks::<8>().0.iter().map(|c| f64::from_le_bytes(*c)).collect()
}

fn cmp(label: &str, a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len(), "{label}");
    let mx = b.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let (mut ident, mut worst, mut rel) = (0usize, 0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b) {
        if x.to_bits() == y.to_bits() {
            ident += 1;
        }
        worst = worst.max((x - y).abs());
        if *y != 0.0 {
            rel = rel.max(((x - y) / y).abs());
        }
    }
    println!(
        "{label:<22} n={:<6} identiques={:<6} écart/max|ref|={:.3e}  écart relatif max={:.3e}",
        a.len(),
        ident,
        worst / mx.max(1e-300),
        rel
    );
}

type C = (f64, f64);

/// Division complexe de numpy (algorithme de Smith).
fn cdiv(a: C, b: C) -> C {
    let (ar, ai, br, bi) = (a.0, a.1, b.0, b.1);
    let (abs_br, abs_bi) = (br.abs(), bi.abs());
    if abs_br >= abs_bi {
        if abs_br == 0.0 && abs_bi == 0.0 {
            return (ar / abs_br, ai / abs_bi);
        }
        let rat = bi / br;
        let scl = 1.0 / (br + bi * rat);
        ((ar + ai * rat) * scl, (ai - ar * rat) * scl)
    } else {
        let rat = br / bi;
        let scl = 1.0 / (bi + br * rat);
        ((ar * rat + ai) * scl, (ai * rat - ar) * scl)
    }
}

fn pairs(v: &[f64]) -> Vec<C> {
    v.chunks(2).map(|c| (c[0], c[1])).collect()
}

fn flat(v: &[C]) -> Vec<f64> {
    v.iter().flat_map(|c| [c.0, c.1]).collect()
}

/// `np.gradient(f, x)` (coordonnées quelconques, ordre 1 aux bords).
fn gradient(f: &[f64], x: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut g = vec![0.0; n];
    g[0] = (f[1] - f[0]) / (x[1] - x[0]);
    g[n - 1] = (f[n - 1] - f[n - 2]) / (x[n - 1] - x[n - 2]);
    for i in 1..n - 1 {
        let (dx1, dx2) = (x[i] - x[i - 1], x[i + 1] - x[i]);
        let a = -dx2 / (dx1 * (dx1 + dx2));
        let b = (dx2 - dx1) / (dx1 * dx2);
        let c = dx1 / (dx2 * (dx1 + dx2));
        g[i] = a * f[i - 1] + b * f[i] + c * f[i + 1];
    }
    g
}

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let (x, y) = (pairs(&read(&dir, "X")), pairs(&read(&dir, "Y")));
    let m = 1001;
    let k = x.len() / m;
    // S = Σ conj(X)·Y sur les lignes, dans l'ordre (somme séquentielle de l'axe 0)
    let mut s = vec![(0.0f64, 0.0f64); m];
    let mut s2 = vec![(0.0f64, 0.0f64); m];
    let mut p = vec![0.0f64; m];
    for r in 0..k {
        for j in 0..m {
            let (a, b) = (x[r * m + j], y[r * m + j]);
            s[j].0 += a.0 * b.0 + a.1 * b.1;
            s[j].1 += a.0 * b.1 - a.1 * b.0;
            s2[j].0 += a.0.mul_add(b.0, a.1 * b.1);
            s2[j].1 += a.0.mul_add(b.1, -(a.1 * b.0));
            let h = a.0.hypot(a.1);
            p[j] += h * h;
        }
    }
    cmp("Σ conj(X)·Y", &flat(&s), &read(&dir, "cs_conj_sum"));
    cmp("Σ conj(X)·Y (fma)", &flat(&s2), &read(&dir, "cs_conj_sum"));
    cmp("Σ |X|² (hypot²)", &p, &read(&dir, "cs_abs2_sum"));
    let (a, b) = (pairs(&read(&dir, "A")), pairs(&read(&dir, "B")));
    let q: Vec<C> = a.iter().zip(&b).map(|(u, v)| cdiv(*u, *v)).collect();
    cmp("division complexe", &flat(&q), &read(&dir, "div"));
    let bc: Vec<C> = b
        .iter()
        .map(|v| (v.0 * 0.3 - v.1 * 0.7, v.0 * 0.7 + v.1 * 0.3))
        .collect();
    let qc: Vec<C> = a.iter().zip(&bc).map(|(u, v)| cdiv(*u, *v)).collect();
    cmp("division (B complexe)", &flat(&qc), &read(&dir, "div_c"));
    let ab: Vec<f64> = a.iter().map(|v| v.0.hypot(v.1)).collect();
    cmp("|z| (hypot)", &ab, &read(&dir, "abs"));
    cmp(
        "|z|² (hypot)²",
        &ab.iter().map(|v| v * v).collect::<Vec<_>>(),
        &read(&dir, "abs2"),
    );
    cmp(
        "angle (atan2)",
        &a.iter().map(|v| v.1.atan2(v.0)).collect::<Vec<_>>(),
        &read(&dir, "angle"),
    );
    let mut cs = vec![(0.0, 0.0)];
    for v in &a {
        let l = *cs.last().unwrap();
        cs.push((l.0 + v.0, l.1 + v.1));
    }
    cmp("cumsum complexe", &flat(&cs), &read(&dir, "cumsum_c"));
    let (r, tm, tm2) = (read(&dir, "r"), read(&dir, "tm"), read(&dir, "tm2"));
    cmp("gradient (pas 0,5 ms)", &gradient(&r, &tm), &read(&dir, "grad"));
    cmp("gradient (pas 1984 Hz)", &gradient(&r, &tm2), &read(&dir, "grad2"));
}
