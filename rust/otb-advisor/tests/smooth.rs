//! `smooth5` et `smooth15` de `rules.rs` (lissages `np.convolve(v, np.ones(K) / K, "same")` de `advisor/rules.py`) contre les sorties de numpy
//! écrites par `scripts/smooth_cases.py` (régénérer `tests/data/smooth.json` avec ce script).
//!
//! Deux exigences :
//! - **tolérance** (toujours) : écart d'au plus 1e-15 fois la somme des valeurs absolues de la fenêtre (de l'ordre de quatre unités du dernier
//!   bit) ; l'écart maximal est affiché en bits (`--nocapture`). Un autre processeur ou une autre bibliothèque de produits scalaires (CI Windows,
//!   L-059) peut différer d'un bit de numpy sans que Rust soit en faute ;
//! - **bit près** (seulement avec `OTB_BITEXACT=1`) : attendu sur ce poste, où numpy somme dans l'ordre croissant avec des FMA (L-061). Sans la
//!   variable, ce test rend la main sans rien exiger.

use otb_advisor::rules::{smooth5, smooth15};
use serde_json::Value;

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;

/// Tolérance relative à la somme des valeurs absolues de la fenêtre (en unités de `kern`).
const WINDOW_RTOL: f64 = 1e-15;

/// Distance en bits (unités de dernière place) entre deux flottants finis de même signe ; les autres cas rendent `u64::MAX`.
fn ulps(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        0
    } else if a.is_finite() && b.is_finite() && a.is_sign_negative() == b.is_sign_negative() {
        a.to_bits().abs_diff(b.to_bits())
    } else if a == b {
        0 // +0 et −0
    } else {
        u64::MAX
    }
}

/// Chaque entrée de `name` : (entrée, sortie de Rust, sortie de numpy).
fn run(name: &str, k: usize, f: &dyn Fn(&[f64]) -> Vec<f64>) -> Vec<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let c = cas("smooth");
    c[name]
        .as_array()
        .unwrap()
        .iter()
        .map(|row: &Value| {
            let x = f64s(row["x"].as_str().unwrap());
            let want = f64s(row["y"].as_str().unwrap());
            let got = f(&x);
            assert_eq!(
                got.len(),
                want.len(),
                "{name} (K={k}) : longueur pour {} valeurs",
                x.len()
            );
            (x, got, want)
        })
        .collect()
}

type Smoother = Box<dyn Fn(&[f64]) -> Vec<f64>>;

fn sources() -> Vec<(&'static str, usize, Smoother)> {
    vec![
        ("smooth5", 5, Box::new(smooth5)),
        ("smooth15", 15, Box::new(|v: &[f64]| smooth15(v).unwrap())),
    ]
}

#[test]
fn smooth_est_proche_de_numpy_a_1e_15() {
    for (name, k, f) in sources() {
        let mut worst_ulps = 0u64;
        let mut worst_rel = 0.0f64;
        let mut n_values = 0usize;
        for (x, got, want) in run(name, k, &f) {
            let n = x.len();
            for i in 0..got.len() {
                // fenêtre de la sortie `i` : n ≥ K ⇒ centrée sur `i` ; sinon numpy échange les tableaux (fenêtre = tout le signal)
                let (lo, hi) = if n >= k {
                    (i.saturating_sub(k / 2), (i + k / 2 + 1).min(n))
                } else {
                    (0, n)
                };
                let scale: f64 = x[lo..hi].iter().map(|v| v.abs()).sum::<f64>() / k as f64;
                let diff = (got[i] - want[i]).abs();
                worst_rel = worst_rel.max(if scale > 0.0 { diff / scale } else { diff });
                assert!(
                    diff <= WINDOW_RTOL * scale,
                    "{name} : {n} valeurs, sortie {i} : Rust {:e} / numpy {:e} (écart {diff:e}, fenêtre {scale:e})",
                    got[i],
                    want[i]
                );
                worst_ulps = worst_ulps.max(ulps(got[i], want[i]));
                n_values += 1;
            }
        }
        eprintln!("{name} : {n_values} valeurs, écart maximal {worst_ulps} bit(s) ({worst_rel:.2e} de la fenêtre)");
    }
}

#[test]
fn smooth_est_identique_a_numpy_au_bit_pres_si_demande() {
    if std::env::var("OTB_BITEXACT").as_deref() != Ok("1") {
        println!(
            "[OTB-SAUT] OTB_BITEXACT absent : comparaison au bit près ignorée (la tolérance est vérifiée par l'autre test)"
        );
        return;
    }
    for (name, k, f) in sources() {
        let mut n_values = 0usize;
        for (x, got, want) in run(name, k, &f) {
            if x.len() < k {
                continue; // numpy échange les tableaux : somme dans un autre ordre, pas de variante au bit près (voir `smooth5`)
            }
            assert_bits(&got, &want, &format!("{name} : {} valeurs", x.len()));
            n_values += got.len();
        }
        eprintln!("{name} : {n_values} valeurs identiques au bit près");
    }
}
