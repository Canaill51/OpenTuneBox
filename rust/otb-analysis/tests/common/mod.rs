//! Outils communs des tests de cas écrits par un script Python (`scripts/*_cases.py`) : flottants en hexadécimal, entrées
//! partagées, comparaison au bit près ou à une tolérance (avec l'écart maximal observé), décodage des réglages.
#![allow(dead_code)]

use otb_analysis::open_loop::C;
use otb_decoder::file::{Matrix, Stats};
use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::{Lowpass, Settings};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

/// Écart maximal observé par catégorie (affiché avec `--nocapture`, pour fixer les tolérances sur une mesure).
static WORST: Mutex<BTreeMap<&'static str, f64>> = Mutex::new(BTreeMap::new());

pub fn cas(name: &str) -> Value {
    let p = format!("{}/tests/data/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let c: Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    // une liste de cas vide ferait passer sa boucle de test sans rien vérifier
    for (k, v) in c.as_object().unwrap() {
        if let Some(a) = v.as_array() {
            assert!(!a.is_empty(), "{name}.json : la liste « {k} » est vide");
        }
    }
    c
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

/// Plancher des comparaisons case par case, en part du plus grand élément de la référence : une case plus petite est le résidu d'arrondi
/// d'un zéro exact (centre d'un notch RPM, |H| ≈ 1e-15 de part et d'autre selon la libm : L-056), son écart relatif ne veut rien dire.
pub const CELL_FLOOR: f64 = 1e-12;

/// Écart **case par case**, relatif à la valeur de référence : `|g − w| / (|w| + CELL_FLOOR × max|w|)`. Pour des spectres dont les petites
/// cases comptent (creux d'un notch RPM, hautes fréquences), une tolérance rapportée au plus grand élément les laisserait libres
/// (relecture 4c-A) ; sous le plancher, l'écart est jugé relativement au plancher. NaN et infinis aux mêmes endroits.
pub fn assert_close_rel(cat: &'static str, got: &[f64], want: &[f64], rtol: f64, what: &str) {
    assert_eq!(got.len(), want.len(), "{what} : longueur");
    let top = want
        .iter()
        .filter(|v| v.is_finite())
        .fold(0.0f64, |m, v| m.max(v.abs()));
    let mut worst = 0.0f64;
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        if w.is_finite() && g.is_finite() {
            worst = worst.max((g - w).abs() / (w.abs() + CELL_FLOOR * top).max(1e-300));
        } else {
            assert!(same(*g, *w), "{what} : élément {i} : Rust {g:e} / Python {w:e}");
        }
    }
    let mut m = WORST.lock().unwrap();
    let e = m.entry(cat).or_insert(0.0);
    *e = e.max(worst);
    drop(m);
    assert!(worst <= rtol, "{what} : écart case par case {worst:e} > {rtol:e}");
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

// ---------------------------------------------------------------- journaux de vol fabriqués (flight_stats_cases.py, delay_cases.py)

pub fn f32s(h: &str) -> Vec<f32> {
    (0..h.len() / 8)
        .map(|i| f32::from_bits(u32::from_str_radix(&h[8 * i..8 * i + 8], 16).unwrap().swap_bytes()))
        .collect()
}

/// Entrée d'un cas : « s… » = float32 (8 caractères par valeur), « i… » = float64.
pub fn inp32(c: &Value, id: &Value) -> Vec<f32> {
    f32s(c["inputs"][id.as_str().unwrap()].as_str().unwrap())
}

pub fn rows32(c: &Value, o: &Value, n: usize) -> Rows {
    Rows {
        k: o["k"].as_u64().unwrap() as usize,
        n,
        data: inp32(c, &o["x"]),
    }
}

pub fn flight_log(c: &Value, v: &Value) -> FlightLog {
    let n = v["n"].as_u64().unwrap() as usize;
    let opt32 = |k: &str| (!v[k].is_null()).then(|| inp32(c, &v[k]));
    FlightLog {
        path: Default::default(),
        session: 1,
        settings: if v.get("settings").is_some_and(|x| !x.is_null()) {
            log_settings(&v["settings"])
        } else {
            Settings::default()
        },
        t: input(c, &v["t"]),
        fs: f1(&v["fs"]),
        gyro: rows32(c, &v["gyro"], n),
        gyro_unfilt: (!v["gyro_unfilt"].is_null()).then(|| rows32(c, &v["gyro_unfilt"], n)),
        setpoint: rows32(c, &v["setpoint"], n),
        rc_command: rows32(c, &v["rc_command"], n),
        p_term: rows32(c, &v["p_term"], n),
        i_term: rows32(c, &v["i_term"], n),
        d_term: rows32(c, &v["d_term"], n),
        f_term: rows32(c, &v["f_term"], n),
        throttle: inp32(c, &v["throttle"]),
        motors: rows32(c, &v["motors"], n),
        motor_hz: (!v["motor_hz"].is_null()).then(|| rows32(c, &v["motor_hz"], n)),
        debug: rows32(c, &v["debug"], n),
        vbat: opt32("vbat"),
        current: opt32("current"),
        acc: None,
        mode_flags: (!v["mode_flags"].is_null())
            .then(|| input(c, &v["mode_flags"]).iter().map(|&x| x as i32).collect()),
        events: Matrix::default(),
        stats: Stats::default(),
        implausible_frames: 0,
        pid_columns: BTreeSet::new(),
    }
}

/// Réglages d'un journal fabriqué : ceux des chaînes de filtres (`settings`) plus le KV moteur.
pub fn log_settings(v: &Value) -> Settings {
    let mut s = settings(v);
    if let Some(kv) = v.get("motor_kv") {
        s.motor_kv = kv.as_i64().unwrap();
    }
    s
}

// ---------------------------------------------------------------- nombres complexes (boucle ouverte, 4c-B)

pub fn cx(c: &Value, v: &Value) -> Vec<C> {
    let (re, im) = (input(c, &v["re"]), input(c, &v["im"]));
    re.into_iter().zip(im).collect()
}

/// Écart relatif de chaque élément complexe : `|g − w| / |w|` (plancher : `CELL_FLOOR` fois le plus grand module de la référence).
pub fn assert_close_cx(cat: &'static str, got: &[C], want: &[C], rtol: f64, what: &str) {
    assert_eq!(got.len(), want.len(), "{what} : longueur");
    let flat = |z: &[C]| -> Vec<f64> { z.iter().flat_map(|c| [c.0, c.1]).collect() };
    // NaN et infinis aux mêmes endroits (partie réelle et imaginaire)
    let (g, w) = (flat(got), flat(want));
    let top = want
        .iter()
        .filter(|c| c.0.is_finite() && c.1.is_finite())
        .fold(0.0f64, |m, c| m.max(c.0.hypot(c.1)));
    let mut worst = 0.0f64;
    for i in 0..got.len() {
        let (a, b) = (got[i], want[i]);
        if b.0.is_finite() && b.1.is_finite() && a.0.is_finite() && a.1.is_finite() {
            let d = (a.0 - b.0).hypot(a.1 - b.1);
            worst = worst.max(d / (b.0.hypot(b.1) + CELL_FLOOR * top).max(1e-300));
        } else {
            for k in [2 * i, 2 * i + 1] {
                assert!(same(g[k], w[k]), "{what} : élément {i} : Rust {a:?} / Python {b:?}");
            }
        }
    }
    assert_close(cat, &[worst, 1.0], &[0.0, 1.0], rtol, what);
}
