//! Banc 4b-0 (jetable) : écarts de `realfft` et des fonctions mathématiques de Rust contre numpy.
use realfft::RealFftPlanner;
use std::f64::consts::PI;

fn read(dir: &str, name: &str) -> Vec<f64> {
    let b = std::fs::read(format!("{dir}/{name}.bin")).unwrap();
    b.as_chunks::<8>().0.iter().map(|c| f64::from_le_bytes(*c)).collect()
}

fn cmp(label: &str, a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len(), "{label}");
    let mx = b.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let (mut ident, mut worst) = (0usize, 0.0f64);
    for (x, y) in a.iter().zip(b) {
        if x.to_bits() == y.to_bits() {
            ident += 1;
        }
        worst = worst.max((x - y).abs());
    }
    println!(
        "{label:<14} n={:<6} identiques={:<6} écart max / max|ref| = {:.3e}",
        a.len(),
        ident,
        worst / mx.max(1e-300)
    );
}

fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let mut planner = RealFftPlanner::<f64>::new();
    for l in [600usize, 640, 997, 1000, 1024, 2048, 4000, 4096] {
        let mut input = read(&dir, &format!("fft_in_{l}"));
        let want = read(&dir, &format!("fft_out_{l}"));
        let r2c = planner.plan_fft_forward(l);
        let mut spec = r2c.make_output_vec();
        r2c.process(&mut input, &mut spec).unwrap();
        let got: Vec<f64> = spec.iter().flat_map(|c| [c.re, c.im]).collect();
        cmp(&format!("rfft {l}"), &got, &want);
        let c2r = planner.plan_fft_inverse(l);
        let mut y = c2r.make_output_vec();
        let mut s2 = spec.clone();
        s2[0].im = 0.0;
        if l % 2 == 0 {
            let last = s2.len() - 1;
            s2[last].im = 0.0;
        }
        c2r.process(&mut s2, &mut y).unwrap();
        let y: Vec<f64> = y.iter().map(|v| v / l as f64).collect();
        cmp(&format!("irfft {l}"), &y, &read(&dir, &format!("ifft_out_{l}")));
    }
    for m in [64usize, 595, 992, 1500] {
        let got: Vec<f64> = (0..m)
            .map(|k| 0.5 - 0.5 * (2.0 * PI * k as f64 / (m - 1) as f64).cos())
            .collect();
        cmp(&format!("hann {m}"), &got, &read(&dir, &format!("cos_{m}")));
    }
    let v = read(&dir, "pos_in");
    let got: Vec<f64> = v.iter().map(|x| 10.0 * x.max(1e-12).log10()).collect();
    cmp("log10", &got, &read(&dir, "log10_out"));
    let got: Vec<f64> = v.iter().map(|x| (x / 3.0).ln()).collect();
    cmp("ln", &got, &read(&dir, "log_out"));
    let got: Vec<f64> = (-12..=12).map(|x: i32| (-0.5 * (x * x) as f64 / 9.0).exp()).collect();
    cmp("exp", &got, &read(&dir, "exp_out"));
    let c = read(&dir, "c_in");
    let ang: Vec<f64> = c.chunks(2).map(|z| z[1].atan2(z[0])).collect();
    cmp("atan2", &ang, &read(&dir, "angle_out"));
    let ab: Vec<f64> = c.chunks(2).map(|z| z[0].hypot(z[1])).collect();
    cmp("hypot", &ab, &read(&dir, "abs_out"));
}
