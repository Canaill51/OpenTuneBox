//! Outils communs des tests de cas écrits par un script Python (`scripts/*_cases.py`) : flottants en hexadécimal, entrées
//! partagées, comparaison au bit près ou à une tolérance (avec l'écart maximal observé), décodage des réglages.
#![allow(dead_code)]

use otb_log::settings::{Lowpass, Settings};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Écart maximal observé par catégorie (affiché avec `--nocapture`, pour fixer les tolérances sur une mesure).
static WORST: Mutex<BTreeMap<&'static str, f64>> = Mutex::new(BTreeMap::new());

pub fn cas(name: &str) -> Value {
    let p = format!("{}/tests/data/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

pub fn f64s(h: &str) -> Vec<f64> {
    (0..h.len() / 16)
        .map(|i| f64::from_bits(u64::from_str_radix(&h[16 * i..16 * i + 16], 16).unwrap().swap_bytes()))
        .collect()
}

pub fn f1(h: &Value) -> f64 {
    f64s(h.as_str().unwrap())[0]
}

/// Entrée d'un cas : les signaux sont écrits une fois dans la table `inputs`, les cas y renvoient par identifiant.
pub fn input(c: &Value, id: &Value) -> Vec<f64> {
    f64s(c["inputs"][id.as_str().unwrap()].as_str().unwrap())
}

pub fn hexes(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(f1).collect()
}

pub fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

pub fn is_error(k: &Value) -> bool {
    k["error"].as_bool().unwrap_or(false)
}

pub fn assert_bits(got: &[f64], want: &[f64], what: &str) {
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

/// Écart maximal rapporté au plus grand élément fini de la référence ; NaN et infinis aux mêmes endroits.
pub fn assert_close(cat: &'static str, got: &[f64], want: &[f64], rtol: f64, what: &str) {
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

pub fn report() {
    for (k, v) in WORST.lock().unwrap().iter() {
        eprintln!("écart max {k:<22} {v:.3e}");
    }
}

pub fn lowpass(v: &Value) -> Lowpass {
    Lowpass {
        kind: match v["type"].as_str().unwrap() {
            "PT1" => "PT1",
            "BIQUAD" => "BIQUAD",
            "PT2" => "PT2",
            "PT3" => "PT3",
            other => panic!("type de passe-bas {other}"),
        },
        static_hz: v["static_hz"].as_i64().unwrap(),
        dyn_min_hz: v["dyn_min_hz"].as_i64().unwrap(),
        dyn_max_hz: v["dyn_max_hz"].as_i64().unwrap(),
        expo: v["expo"].as_i64().unwrap(),
    }
}

pub fn pair(v: &Value) -> (i64, i64) {
    (v[0].as_i64().unwrap(), v[1].as_i64().unwrap())
}

/// Réglages lus par les chaînes de filtres ; `version`, `debug_mode` et `rpm_lpf_hz` seulement s'ils sont écrits (4b-4).
pub fn settings(v: &Value) -> Settings {
    let mut s = Settings {
        looptime_us: v["looptime_us"].as_i64().unwrap(),
        pid_denom: v["pid_denom"].as_i64().unwrap(),
        gyro_lpf1: lowpass(&v["gyro_lpf1"]),
        gyro_lpf2: lowpass(&v["gyro_lpf2"]),
        dterm_lpf1: lowpass(&v["dterm_lpf1"]),
        dterm_lpf2: lowpass(&v["dterm_lpf2"]),
        gyro_notches: v["gyro_notches"].as_array().unwrap().iter().map(pair).collect(),
        dterm_notch: pair(&v["dterm_notch"]),
        dyn_notch_count: v["dyn_notch_count"].as_i64().unwrap(),
        dyn_notch_q: v["dyn_notch_q"].as_i64().unwrap(),
        dyn_notch_min_hz: v["dyn_notch_min_hz"].as_i64().unwrap(),
        dyn_notch_max_hz: v["dyn_notch_max_hz"].as_i64().unwrap(),
        rpm_harmonics: v["rpm_harmonics"].as_i64().unwrap(),
        rpm_weights: v["rpm_weights"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_i64().unwrap())
            .collect(),
        rpm_q: v["rpm_q"].as_i64().unwrap(),
        rpm_min_hz: v["rpm_min_hz"].as_i64().unwrap(),
        rpm_fade_hz: v["rpm_fade_hz"].as_i64().unwrap(),
        ..Settings::default()
    };
    if let Some(ver) = v.get("version") {
        s.version = ver.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
        s.debug_mode = v["debug_mode"].as_str().unwrap().to_string();
        s.rpm_lpf_hz = v["rpm_lpf_hz"].as_i64().unwrap();
    }
    s
}

pub fn all_settings(c: &Value) -> Vec<Settings> {
    c["settings"].as_array().unwrap().iter().map(settings).collect()
}
