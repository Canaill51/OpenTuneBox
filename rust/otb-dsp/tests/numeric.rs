//! `numeric` rend ce que `analysis/_numeric.py` rend : cas écrits par `scripts/numeric_cases.py` (régénérer
//! `tests/data/numeric.json` avec ce script si `_numeric.py` change).
//!
//! - `uniform_filter1d` (arithmétique pure) : **bit pour bit**.
//! - `hann` (`cos`) et `gaussian_filter` (`exp`) : à `FN_RTOL` du plus grand élément. Rust appelle la bibliothèque
//!   mathématique du système (glibc, CRT de Windows…) et numpy a son propre `exp` AVX-512 : ils diffèrent parfois du
//!   dernier bit (mesuré : `np.exp` ≠ `math.exp` pour 4,5 % des arguments sur ce poste ; avec `NPY_DISABLE_CPU_FEATURES=
//!   "X86_V4 AVX512_ICL"` numpy redonne exactement les bits de Rust). Un test bit pour bit serait faux sous Windows.
//! - `xcorr_full` (FFT) : à `XCORR_RTOL` du plus grand élément (écart mesuré 9e-16 en 4b-0).

use otb_dsp::numeric::{gaussian_filter, hann, uniform_filter1d, xcorr_full};
use serde_json::Value;

const XCORR_RTOL: f64 = 1e-12;
/// 1 à 2 ulp d'un `exp` ou d'un `cos`, amplifiés par les sommes du noyau : ≈ 50 ulp du plus grand élément de marge.
const FN_RTOL: f64 = 1e-14;
/// Même tolérance pour une sortie arrondie en float32 : un arrondi de plus peut tomber de l'autre côté (1 ulp f32).
const FN_RTOL_F32: f64 = 2e-7;

fn cas() -> Value {
    let p = format!("{}/tests/data/numeric.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn f64s(h: &str) -> Vec<f64> {
    (0..h.len() / 16)
        .map(|i| f64::from_bits(u64::from_str_radix(&h[16 * i..16 * i + 16], 16).unwrap().swap_bytes()))
        .collect()
}

/// Entrée d'un cas : les signaux sont écrits une fois dans la table `inputs`, les cas y renvoient par identifiant.
fn input(c: &Value, id: &Value) -> Vec<f64> {
    f64s(c["inputs"][id.as_str().unwrap()].as_str().unwrap())
}

fn f32s(h: &str) -> Vec<f32> {
    (0..h.len() / 8)
        .map(|i| f32::from_bits(u32::from_str_radix(&h[8 * i..8 * i + 8], 16).unwrap().swap_bytes()))
        .collect()
}

fn shape(v: &Value) -> Vec<usize> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_u64().unwrap() as usize)
        .collect()
}

/// Même flottant : mêmes bits, ou deux NaN.
fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn assert_bits(got: &[f64], want: &[f64], what: &str) {
    assert_eq!(got.len(), want.len(), "{what} : longueur");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert!(
            same(*g, *w),
            "{what} : élément {i} : Rust {g:e} ({:016x}) / Python {w:e} ({:016x})",
            g.to_bits(),
            w.to_bits()
        );
    }
}

/// `rtol = None` : bit pour bit ; sinon écart permis `rtol` fois le plus grand élément fini de la référence (les NaN et les
/// infinis doivent tomber aux mêmes endroits).
fn assert_close(got: &[f64], want: &[f64], rtol: Option<f64>, what: &str) {
    assert_eq!(got.len(), want.len(), "{what} : longueur");
    let Some(rtol) = rtol else {
        return assert_bits(got, want, what);
    };
    let top = want
        .iter()
        .filter(|v| v.is_finite())
        .fold(0.0f64, |m, v| m.max(v.abs()));
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        if w.is_finite() {
            assert!(
                (g - w).abs() <= rtol * top,
                "{what} : élément {i} : Rust {g:e} / Python {w:e} (plus grand {top:e})"
            );
        } else {
            assert!(same(*g, *w), "{what} : élément {i} : Rust {g:e} / Python {w:e}");
        }
    }
}

/// Sortie comparée à celle de Python, en float32 arrondi quand l'entrée l'était (`f32_rtol` remplace alors `rtol`).
fn check_out(got: &[f64], c: &Value, rtol: Option<f64>, what: &str) {
    let out = c["out"].as_str().unwrap();
    if c["f32"].as_bool().unwrap() {
        let want = f32s(out);
        assert_eq!(got.len(), want.len(), "{what} : longueur");
        let want: Vec<f64> = want.iter().map(|v| *v as f64).collect();
        let got: Vec<f64> = got.iter().map(|v| *v as f32 as f64).collect();
        assert_close(&got, &want, rtol.map(|_| FN_RTOL_F32), what);
    } else {
        assert_close(got, &f64s(out), rtol, what);
    }
}

#[test]
fn hann_dans_la_tolerance() {
    let c = cas();
    let cases = c["hann"].as_array().unwrap();
    assert!(cases.len() >= 30);
    for k in cases {
        let (len, sym) = (k["length"].as_i64().unwrap(), k["sym"].as_bool().unwrap());
        assert_close(
            &hann(len, sym),
            &f64s(k["out"].as_str().unwrap()),
            Some(FN_RTOL),
            &format!("hann({len}, {sym})"),
        );
    }
}

#[test]
fn uniform_filter1d_identique() {
    let c = cas();
    let cases = c["uniform"].as_array().unwrap();
    assert!(cases.len() > 800);
    for k in cases {
        let (sh, axis, size) = (
            shape(&k["shape"]),
            k["axis"].as_i64().unwrap(),
            k["size"].as_i64().unwrap(),
        );
        let x = input(&c, &k["x"]);
        let what = format!("uniform_filter1d forme {sh:?} axe {axis} largeur {size}");
        let got = uniform_filter1d(&x, &sh, size, axis);
        if k["out"].is_null() {
            assert!(got.is_err(), "{what} : Python lève une erreur");
        } else {
            check_out(&got.expect(&what), k, None, &what);
        }
    }
}

#[test]
fn gaussian_filter_dans_la_tolerance() {
    let c = cas();
    let cases = c["gaussian"].as_array().unwrap();
    assert!(cases.len() > 700);
    for k in cases {
        let sh = shape(&k["shape"]);
        let sigma: Vec<f64> = k["sigma"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let x = input(&c, &k["x"]);
        let what = format!("gaussian_filter forme {sh:?} sigma {sigma:?}");
        check_out(&gaussian_filter(&x, &sh, &sigma).unwrap(), k, Some(FN_RTOL), &what);
    }
}

#[test]
fn xcorr_full_dans_la_tolerance() {
    let c = cas();
    let cases = c["xcorr"].as_array().unwrap();
    assert!(cases.len() >= 18);
    for k in cases {
        let (y, x) = (input(&c, &k["y"]), input(&c, &k["x"]));
        let what = format!("xcorr_full ({}, {})", y.len(), x.len());
        let got = xcorr_full(&y, &x);
        if k["out"].is_null() {
            assert!(got.is_err(), "{what} : Python lève une erreur");
            continue;
        }
        let (corr, lags) = got.expect(&what);
        let want = f64s(k["out"].as_str().unwrap());
        assert_eq!(corr.len(), want.len(), "{what} : longueur");
        let lg = k["lags"].as_array().unwrap();
        assert_eq!(
            (lags[0], *lags.last().unwrap(), lags.len() as i64),
            (
                lg[0].as_i64().unwrap(),
                lg[1].as_i64().unwrap(),
                lg[2].as_i64().unwrap()
            ),
            "{what} : décalages"
        );
        let top = want.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let worst = corr.iter().zip(&want).fold(0.0f64, |m, (g, w)| m.max((g - w).abs()));
        assert!(
            worst <= XCORR_RTOL * top,
            "{what} : écart {:e} du plus grand élément {top:e}",
            worst / top.max(1e-300)
        );
    }
}
