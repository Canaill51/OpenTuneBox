//! `spectral` rend ce que `analysis/spectral.py` rend : cas écrits par `scripts/spectral_cases.py` (régénérer
//! `tests/data/spectral.json` avec ce script si `spectral.py` change).
//!
//! Exigé identique : erreurs, formes, entiers (comptes, bits de masque), fréquences, centres de tranches, temps, et tout ce
//! qui est de l'arithmétique pure (découpage, sommes, lissage de `uniform_filter1d`). Sorties qui passent par la FFT, `log10`,
//! `hypot` ou `exp` : écart permis rapporté au plus grand élément fini de la référence (`*_RTOL`, mesures dans
//! docs/migration-rust.md ; L-048). Les NaN et les infinis doivent tomber aux mêmes endroits.

use otb_analysis::spectral::{self, Mode};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Écart maximal observé par catégorie (affiché avec `--nocapture`, pour fixer les tolérances sur une mesure).
static WORST: Mutex<BTreeMap<&'static str, f64>> = Mutex::new(BTreeMap::new());

/// `log10` (mesuré 7,4e-18) : 1 ulp d'une valeur en dB, rapporté au plus grand.
const DB_RTOL: f64 = 1e-14;
/// Spectre d'amplitude ou de puissance après FFT, rapporté au plus grand élément (mesuré 5,0e-16, marge ×20).
const FFT_RTOL: f64 = 1e-14;
/// Spectre en dB après FFT (mesuré 1,5e-10 : la dynamique du spectre, jusqu'à 1e12 en puissance dans ces cas, amplifie
/// l'écart de la FFT sur les petits éléments ; marge ×7, et ≤ 1e-10 sur le corpus).
const FFT_DB_RTOL: f64 = 1e-9;
/// Carte lissée par `gaussian_filter` (`exp`, mesuré 4,3e-16) : voir `numeric.rs`.
const SMOOTH_RTOL: f64 = 1e-14;

fn cas() -> Value {
    let p = format!("{}/tests/data/spectral.json", env!("CARGO_MANIFEST_DIR"));
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

fn f1(h: &Value) -> f64 {
    f64s(h.as_str().unwrap())[0]
}

fn ints(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect()
}

fn opt_ints(v: &Value) -> Option<Vec<i64>> {
    (!v.is_null()).then(|| ints(v))
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn assert_bits(got: &[f64], want: &[f64], what: &str) {
    assert_eq!(got.len(), want.len(), "{what} : longueur");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert!(same(*g, *w), "{what} : élément {i} : Rust {g:e} / Python {w:e}");
    }
}

/// Écart maximal rapporté au plus grand élément fini de la référence ; NaN et infinis aux mêmes endroits.
fn assert_close(cat: &'static str, got: &[f64], want: &[f64], rtol: f64, what: &str) {
    assert_eq!(got.len(), want.len(), "{what} : longueur");
    let top = want
        .iter()
        .filter(|v| v.is_finite())
        .fold(0.0f64, |m, v| m.max(v.abs()));
    let mut worst = 0.0f64;
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        if w.is_finite() && g.is_finite() {
            worst = worst.max((g - w).abs() / top.max(1e-300));
        } else {
            assert!(same(*g, *w), "{what} : élément {i} : Rust {g:e} / Python {w:e}");
        }
    }
    let mut m = WORST.lock().unwrap();
    let e = m.entry(cat).or_insert(0.0);
    *e = e.max(worst);
    drop(m);
    assert!(
        worst <= rtol,
        "{what} : écart {worst:e} du plus grand élément > {rtol:e}"
    );
}

fn report() {
    for (k, v) in WORST.lock().unwrap().iter() {
        eprintln!("écart max {k:<22} {v:.3e}");
    }
}

fn is_error(out: &Value) -> bool {
    out.get("error").and_then(Value::as_bool).unwrap_or(false)
}

#[test]
fn clear_of_breaks_identique() {
    for k in cas()["clear_of_breaks"].as_array().unwrap() {
        let starts = ints(&k["starts"]);
        let breaks = opt_ints(&k["breaks"]);
        let got = spectral::clear_of_breaks(&starts, k["length"].as_i64().unwrap(), breaks.as_deref());
        let want: Vec<bool> = k["out"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_bool().unwrap())
            .collect();
        assert_eq!(got, want, "starts {starts:?} breaks {breaks:?} length {}", k["length"]);
    }
}

#[test]
fn ptb_subsample_identique() {
    for k in cas()["ptb_subsample"].as_array().unwrap() {
        let d = f1(&k["d"]);
        assert_eq!(
            spectral::ptb_subsample(d, 5, 3, 1),
            k["out"].as_i64().unwrap(),
            "durée {d}"
        );
    }
}

#[test]
fn to_db_dans_la_tolerance() {
    let c = cas();
    for k in c["to_db"].as_array().unwrap() {
        let p = input(&c, &k["p"]);
        let db = spectral::to_db(&p);
        assert_close("to_db", &db, &f64s(k["db"].as_str().unwrap()), DB_RTOL, "to_db");
        // l'échelle PIDtoolbox est une soustraction : identique bit pour bit à partir des dB de Python
        let want_db = f64s(k["db"].as_str().unwrap());
        assert_bits(
            &spectral::to_ptb_db(&want_db),
            &f64s(k["ptb"].as_str().unwrap()),
            "to_ptb_db",
        );
    }
    report();
}

#[test]
fn smooth_freq_identique() {
    let c = cas();
    for k in c["smooth_freq"].as_array().unwrap() {
        let shape: Vec<usize> = ints(&k["shape"]).iter().map(|&v| v as usize).collect();
        let (y, level) = (input(&c, &k["y"]), k["level"].as_i64().unwrap());
        let what = format!("smooth_freq forme {shape:?} niveau {level}");
        assert_bits(
            &spectral::smooth_freq(&y, &shape, level),
            &f64s(k["out"].as_str().unwrap()),
            &what,
        );
    }
}

#[test]
fn spectrum_dans_la_tolerance() {
    let c = cas();
    let cases = c["spectrum"].as_array().unwrap();
    assert!(cases.len() >= 80);
    for k in cases {
        let (x, fs) = (input(&c, &k["x"]), f1(&k["fs"]));
        let mode = k["mode"].as_str().unwrap();
        let breaks = opt_ints(&k["breaks"]);
        let what = format!(
            "spectrum n={} fs={fs} mode={mode} res={} lissage={} breaks={breaks:?}",
            x.len(),
            k["res_hz"],
            k["smoothing"]
        );
        let got = spectral::spectrum(
            &x,
            fs,
            Mode::parse(mode),
            k["res_hz"].as_f64().unwrap(),
            k["smoothing"].as_i64().unwrap(),
            breaks.as_deref(),
        );
        let out = &k["out"];
        if is_error(out) {
            assert!(got.is_err(), "{what} : Python lève une erreur");
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : {e}"));
        assert_bits(
            &got.freqs,
            &f64s(out["freqs"].as_str().unwrap()),
            &format!("{what} : fréquences"),
        );
        assert!(same(got.resolution_hz, f1(&out["res"])), "{what} : résolution");
        let tol = match mode {
            "psd" => FFT_DB_RTOL,
            _ => FFT_RTOL,
        };
        let cat = if mode == "psd" {
            "spectrum psd (dB)"
        } else {
            "spectrum amp/autre"
        };
        assert_close(cat, &got.values, &f64s(out["values"].as_str().unwrap()), tol, &what);
    }
    report();
}

#[test]
fn freq_throttle_dans_la_tolerance() {
    let c = cas();
    let cases = c["freq_throttle"].as_array().unwrap();
    assert!(cases.len() >= 40);
    for k in cases {
        let (x, xa, fs) = (input(&c, &k["x"]), input(&c, &k["xaxis"]), f1(&k["fs"]));
        let mode = k["mode"].as_str().unwrap();
        let breaks = opt_ints(&k["breaks"]);
        let r = k["range"].as_array().unwrap();
        let what = format!(
            "freq_throttle n={} mode={mode} nbins={} seg={} plage={r:?} breaks={breaks:?}",
            x.len(),
            k["nbins"],
            k["seg_ms"]
        );
        let got = spectral::freq_throttle(
            &x,
            &xa,
            fs,
            Mode::parse(mode),
            k["seg_ms"].as_f64().unwrap(),
            k["nbins"].as_u64().unwrap() as usize,
            (r[0].as_f64().unwrap(), r[1].as_f64().unwrap()),
            breaks.as_deref(),
        );
        let out = &k["out"];
        if is_error(out) {
            assert!(got.is_err(), "{what} : Python lève une erreur");
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : {e}"));
        assert_bits(
            &got.freqs,
            &f64s(out["freqs"].as_str().unwrap()),
            &format!("{what} : fréquences"),
        );
        assert_bits(
            &got.bins,
            &f64s(out["bins"].as_str().unwrap()),
            &format!("{what} : centres"),
        );
        assert_eq!(
            got.counts,
            ints(&out["counts"]),
            "{what} : nombre de fenêtres par tranche"
        );
        assert_eq!(
            vec![got.freqs.len(), got.bins.len()],
            ints(&out["shape"]).iter().map(|&v| v as usize).collect::<Vec<_>>(),
            "{what} : forme"
        );
        let (tol, cat) = if mode == "psd" {
            (FFT_DB_RTOL, "freq_throttle psd (dB)")
        } else {
            (FFT_RTOL, "freq_throttle amp")
        };
        assert_close(cat, &got.z, &f64s(out["z"].as_str().unwrap()), tol, &what);
    }
    report();
}

#[test]
fn smooth_map_dans_la_tolerance() {
    let c = cas();
    for k in c["smooth_map"].as_array().unwrap() {
        let shape: Vec<usize> = ints(&k["shape"]).iter().map(|&v| v as usize).collect();
        let (z, level) = (input(&c, &k["z"]), k["level"].as_i64().unwrap());
        let what = format!("smooth_map forme {shape:?} niveau {level}");
        assert_close(
            "smooth_map",
            &spectral::smooth_map(&z, &shape, level),
            &f64s(k["out"].as_str().unwrap()),
            SMOOTH_RTOL,
            &what,
        );
    }
    report();
}

#[test]
fn freq_time_dans_la_tolerance() {
    let c = cas();
    let cases = c["freq_time"].as_array().unwrap();
    assert!(cases.len() >= 25);
    for k in cases {
        let (x, t, fs) = (input(&c, &k["x"]), input(&c, &k["t"]), f1(&k["fs"]));
        let breaks = opt_ints(&k["breaks"]);
        let what = format!(
            "freq_time n={} fenêtre={} hop_div={} lissage={} breaks={breaks:?}",
            x.len(),
            k["win_ms"],
            k["hop_div"],
            k["smoothing"]
        );
        let got = spectral::freq_time(
            &x,
            &t,
            fs,
            k["win_ms"].as_f64().unwrap(),
            k["hop_div"].as_i64().unwrap(),
            k["smoothing"].as_i64().unwrap(),
            breaks.as_deref(),
        );
        let out = &k["out"];
        if is_error(out) {
            assert!(got.is_err(), "{what} : Python lève une erreur");
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : {e}"));
        assert_bits(
            &got.times,
            &f64s(out["times"].as_str().unwrap()),
            &format!("{what} : temps"),
        );
        assert_bits(
            &got.freqs,
            &f64s(out["freqs"].as_str().unwrap()),
            &format!("{what} : fréquences"),
        );
        assert_eq!(
            vec![got.nf, got.nt],
            ints(&out["shape"]).iter().map(|&v| v as usize).collect::<Vec<_>>(),
            "{what} : forme"
        );
        assert_close(
            "freq_time (dB)",
            &got.z,
            &f64s(out["z"].as_str().unwrap()),
            FFT_DB_RTOL,
            &what,
        );
    }
    report();
}
