//! Banc de la décimation min/max (`otb_dsp::minmax`, D-048, W-049) : temps d'un appel, parcours contre index.
//!
//! `cargo run --release -p otb-dsp --example banc_minmax -- [N] [POINTS]`
//!
//! Compare, sur une même courbe de `N` échantillons et à nombre de points rendus constant (`POINTS`, 3840 par défaut,
//! soit une fenêtre de 1920 pixels) :
//! - la **vue entière** : tous les échantillons sont visibles ;
//! - des **vues zoomées** : 1/8 et 1/64 du log, donc 8 et 64 fois moins d'échantillons visibles pour le même nombre
//!   de points rendus. Un calcul en O(points rendus) y mettrait le même temps qu'en vue entière.
//!
//! Isole le calcul du reste d'un cadre d'interface (tracé des points, egui_plot, accessibilité) : à comparer au banc
//! `otb-gui --example banc_cadre`, qui mesure le cadre complet. Ne jamais lancer sous profileur pour décider.

use std::time::Instant;

use otb_dsp::minmax::{MinMaxPyramid, window_points, window_points_scan};

/// Essais chronométrés par cas (médiane retenue), après une chauffe.
const ESSAIS: usize = 30;

/// Courbe d'essai : un sinus bruité, qui ne laisse aucun paquet constant.
fn courbe(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut graine = 0x5EEDu64;
    let mut bruit = || {
        graine = graine
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (graine >> 11) as f64 / (1u64 << 53) as f64
    };
    let t = (0..n).map(|i| i as f64 / 1000.0).collect();
    let y = (0..n).map(|i| (i as f64 * 0.05).sin() * 500.0 + bruit()).collect();
    (t, y)
}

fn median_ms(mut ms: Vec<f64>) -> f64 {
    ms.sort_by(|a, b| a.total_cmp(b));
    ms[ms.len() / 2]
}

fn mesure(mut f: impl FnMut() -> usize) -> (f64, usize) {
    for _ in 0..3 {
        f();
    }
    let mut points = 0;
    let ms = (0..ESSAIS)
        .map(|_| {
            let t0 = Instant::now();
            points = f();
            t0.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    (median_ms(ms), points)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let n: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(620_079);
    let points: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(3840);

    let (t, y) = courbe(n);
    let t0 = Instant::now();
    let p = MinMaxPyramid::build(&y);
    let construction = t0.elapsed().as_secs_f64() * 1e3;
    println!(
        "{n} échantillons, {points} points rendus. Index : {:.1} ms à construire, {} Ko ({:.0} % d'une courbe f64)",
        construction,
        p.bytes() / 1024,
        100.0 * p.bytes() as f64 / (8 * n) as f64
    );
    println!(
        "{:<18} {:>12} {:>11} {:>11} {:>7}",
        "Fenêtre", "échant. vus", "parcours", "index", "rapport"
    );

    for fraction in [1usize, 8, 64] {
        let large = t[n - 1] / fraction as f64;
        let x0 = (t[n - 1] - large) / 2.0;
        let x1 = x0 + large;
        let vus = t.partition_point(|&v| v <= x1) - t.partition_point(|&v| v < x0);
        let (ms_scan, pts) = mesure(|| window_points_scan(&t, &y, x0, x1, points).len());
        let (ms_pyr, pts2) = mesure(|| window_points(&t, &y, &p, x0, x1, points).len());
        assert_eq!(pts, pts2, "les deux calculs doivent rendre le même nombre de points");
        println!(
            "{:<18} {vus:>12} {ms_scan:>9.3} ms {ms_pyr:>9.3} ms {:>6.1}×  ({pts} points)",
            format!("1/{fraction}"),
            ms_scan / ms_pyr
        );
    }
}
