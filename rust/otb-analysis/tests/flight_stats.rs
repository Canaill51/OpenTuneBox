//! `datasets`, `period` et `stats` rendent ce que rend Python : cas écrits par `scripts/flight_stats_cases.py` sur de petits
//! journaux fabriqués (régénérer `tests/data/flight_stats.json` avec ce script si l'un de ces trois modules change).
//!
//! Tout est exigé **identique au bit près** : les courbes sont en `float32` et numpy y calcule sommes par paires, moyennes,
//! écarts-types, médianes et centiles en `float32` (`npstat.rs`) ; aucune FFT ni fonction de libm n'intervient ici.

use otb_analysis::datasets::{self, DATASETS};
use otb_analysis::period::{self, FlightParts};
use otb_analysis::stats::{self, MeanSd};
use otb_log::flightlog::FlightLog;
use serde_json::Value;
use std::ops::Range;

mod common;
use common::*;

fn logs(c: &Value) -> Vec<FlightLog> {
    c["logs"].as_array().unwrap().iter().map(|v| flight_log(c, v)).collect()
}

fn sel_of(k: &Value, n: usize) -> Range<usize> {
    match k["sel"].as_array() {
        None => 0..n,
        Some(s) => s[0].as_u64().unwrap() as usize..s[1].as_u64().unwrap() as usize,
    }
}

fn name(c: &Value, k: &Value) -> String {
    format!("{} sel {}", c["names"][k["log"].as_u64().unwrap() as usize], k["sel"])
}

fn bits(got: f64, want: &Value, what: &str) {
    assert!(same(got, f1(want)), "{what} : Rust {got:e} / Python {:e}", f1(want));
}

fn ms(got: &MeanSd, want: &Value, what: &str) {
    bits(got.mean, &want[0], &format!("{what} moyenne"));
    bits(got.sd, &want[1], &format!("{what} écart-type"));
    bits(got.max, &want[2], &format!("{what} maximum"));
}

fn opt_ms(got: &Option<MeanSd>, want: &Value, what: &str) {
    match (got, want.is_null()) {
        (None, true) => {}
        (Some(g), false) => ms(g, want, what),
        _ => panic!("{what} : présence différente"),
    }
}

fn opt_f(got: Option<f64>, want: &Value, what: &str) {
    match (got, want.is_null()) {
        (None, true) => {}
        (Some(g), false) => bits(g, want, what),
        _ => panic!("{what} : présence différente"),
    }
}

fn arr(c: &Value, id: &Value, got: &[f64], what: &str) {
    assert_bits(got, &input(c, id), what);
}

#[test]
fn datasets_identiques() {
    let c = cas("flight_stats");
    let ls = logs(&c);
    let (mut skipped, mut checked) = (0, 0);
    for k in c["datasets"].as_array().unwrap() {
        let lg = &ls[k["log"].as_u64().unwrap() as usize];
        let what = format!("datasets {}", c["names"][k["log"].as_u64().unwrap() as usize]);
        // ⚠️ Python lève (np.gradient exige 2 points) pour un journal d'un point au terme D non nul ; Rust (4a) ne lève pas :
        // écart de robustesse assumé, travail reporté W-023
        if k["error"].as_bool().unwrap() {
            assert_eq!(
                c["names"][k["log"].as_u64().unwrap() as usize],
                "n1",
                "seul le journal d'un point lève"
            );
            skipped += 1;
            continue;
        }
        let want: Vec<&str> = k["available"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(datasets::available(lg), want, "{what} : jeux présents");
        for (i, key) in DATASETS.iter().map(|(k, _)| *k).chain(["inconnu"]).enumerate() {
            for ax in 0..4 {
                let want = &k["has_axis"][i][ax];
                if want == "E" {
                    skipped += 1;
                    continue;
                }
                let got = datasets::has_axis(lg, key, ax);
                if key == "inconnu" {
                    assert!(got.is_err(), "{what} : clé inconnue");
                } else {
                    assert_eq!(got.unwrap(), want.as_bool().unwrap(), "{what} {key} axe {ax}");
                }
                checked += 1;
            }
        }
    }
    assert!(skipped > 0, "le journal d'un point doit être rencontré");
    assert!(checked > 1000);
}

#[test]
fn flight_stats_identique() {
    let c = cas("flight_stats");
    let ls = logs(&c);
    let cases = c["flight_stats"].as_array().unwrap();
    assert!(cases.len() > 150);
    for k in cases {
        let lg = &ls[k["log"].as_u64().unwrap() as usize];
        let w = name(&c, k);
        assert!(!is_error(k), "{w}");
        let got = stats::flight_stats(lg, sel_of(k, lg.n()));
        let o = &k["out"];
        bits(got.duration_s, &o["duration_s"], &format!("{w} durée"));
        for (i, (nm, centres, hist)) in got.stick_hist.iter().enumerate() {
            assert_eq!(*nm, o["stick_hist"][i][0].as_str().unwrap(), "{w} nom d'histogramme");
            arr(&c, &o["stick_hist"][i][1], centres, &format!("{w} centres {nm}"));
            arr(&c, &o["stick_hist"][i][2], hist, &format!("{w} histogramme {nm}"));
        }
        assert_eq!(got.stick_hist.len(), 4);
        for (nm, v) in &got.terms {
            for (a, m) in v.iter().enumerate() {
                ms(m, &o["terms"][nm][a], &format!("{w} terme {nm} axe {a}"));
            }
        }
        assert_eq!(
            got.motors.len(),
            o["motors"].as_array().unwrap().len(),
            "{w} nombre de moteurs"
        );
        for (i, m) in got.motors.iter().enumerate() {
            ms(m, &o["motors"][i], &format!("{w} moteur {i}"));
        }
        bits(got.motor_saturation_pct, &o["sat"], &format!("{w} saturation"));
        bits(got.motor_imbalance_pct, &o["imb"], &format!("{w} déséquilibre"));
        ms(&got.throttle, &o["throttle"], &format!("{w} gaz"));
        opt_ms(&got.vbat, &o["vbat"], &format!("{w} tension"));
        opt_f(got.vbat_min, &o["vbat_min"], &format!("{w} tension min"));
        opt_ms(&got.current, &o["current"], &format!("{w} courant"));
        opt_f(got.mah, &o["mah"], &format!("{w} mAh"));
    }
}

#[test]
fn pid_error_stats_identique() {
    let c = cas("flight_stats");
    let ls = logs(&c);
    let cases = c["pid_error_stats"].as_array().unwrap();
    assert!(cases.len() > 150);
    for k in cases {
        let lg = &ls[k["log"].as_u64().unwrap() as usize];
        let w = name(&c, k);
        assert!(!is_error(k), "{w}");
        let (ms_, er) = match k.get("args") {
            Some(a) => (f1(&a[0]), f1(&a[1])),
            None => (100.0, 50.0),
        };
        let got = stats::pid_error_stats(lg, sel_of(k, lg.n()), ms_, er);
        let o = &k["out"];
        arr(&c, &o["edges"], &got.edges, &format!("{w} bornes"));
        arr(&c, &o["stick_bins"], &got.stick_bins, &format!("{w} centres du manche"));
        for a in 0..3 {
            arr(&c, &o["hist"][a], &got.hist[a], &format!("{w} histogramme axe {a}"));
            bits(got.sd[a], &o["sd"][a], &format!("{w} écart-type robuste axe {a}"));
            bits(
                got.mean_abs[a],
                &o["mean_abs"][a],
                &format!("{w} erreur moyenne axe {a}"),
            );
            arr(
                &c,
                &o["err_vs_stick"][a],
                &got.err_vs_stick[a],
                &format!("{w} erreur selon le manche axe {a}"),
            );
        }
    }
}

#[test]
fn auto_period_identique() {
    let c = cas("flight_stats");
    let ls = logs(&c);
    let (mut full, mut part) = (0, 0);
    for k in c["auto_period"].as_array().unwrap() {
        let lg = &ls[k["log"].as_u64().unwrap() as usize];
        let w = format!(
            "auto_period {} {}",
            c["names"][k["log"].as_u64().unwrap() as usize],
            k["args"]
        );
        assert!(!is_error(k), "{w}");
        let a: Vec<f64> = k["args"].as_array().unwrap().iter().map(f1).collect();
        let (take, margin) = (a.first().copied().unwrap_or(8.0), a.get(1).copied().unwrap_or(0.5));
        let (t0, t1) = period::auto_period(lg, take, margin).unwrap();
        bits(t0, &k["out"][0], &format!("{w} début"));
        bits(t1, &k["out"][1], &format!("{w} fin"));
        if t0 == lg.t[0] && t1 == lg.t[lg.n() - 1] {
            full += 1;
        } else {
            part += 1;
        }
    }
    // les deux branches (période entière / période restreinte) sont atteintes
    assert!(full > 20 && part > 20, "pleine {full}, restreinte {part}");
}

#[test]
fn crash_time_identique() {
    let c = cas("flight_stats");
    let ls = logs(&c);
    let (mut some, mut none) = (0, 0);
    for k in c["crash_time"].as_array().unwrap() {
        let lg = &ls[k["log"].as_u64().unwrap() as usize];
        let w = format!(
            "crash_time {} {}",
            c["names"][k["log"].as_u64().unwrap() as usize],
            k["args"]
        );
        let a: Vec<f64> = k["args"].as_array().unwrap().iter().map(f1).collect();
        let (e, m) = (a.first().copied().unwrap_or(300.0), a.get(1).copied().unwrap_or(60.0));
        let got = period::crash_time(lg, e, m);
        match (&k["out"], got) {
            (Value::Null, None) => none += 1,
            (o, Some(g)) if !o.is_null() => {
                some += 1;
                bits(g, o, &w);
            }
            (o, g) => panic!("{w} : Python {o}, Rust {g:?}"),
        }
    }
    assert!(some > 20 && none > 20, "avec crash {some}, sans {none}");
}

fn parts_eq(got: &FlightParts, want: &Value, w: &str) {
    for (key, g) in [("hover", &got.hover), ("moving", &got.moving)] {
        match (g, want[key].is_null()) {
            (None, true) => {}
            (Some((a, b)), false) => {
                bits(*a, &want[key][0], &format!("{w} {key} début"));
                bits(*b, &want[key][1], &format!("{w} {key} fin"));
            }
            _ => panic!("{w} : présence de {key} différente"),
        }
    }
}

#[test]
fn flight_parts_identique() {
    let c = cas("flight_stats");
    let ls = logs(&c);
    let (mut hover, mut moving, mut neither) = (0, 0, 0);
    for k in c["flight_parts"].as_array().unwrap() {
        let lg = &ls[k["log"].as_u64().unwrap() as usize];
        let w = format!(
            "flight_parts {} [{}, {}] {}",
            c["names"][k["log"].as_u64().unwrap() as usize],
            f1(&k["t0"]),
            f1(&k["t1"]),
            k["args"]
        );
        assert!(!is_error(k), "{w}");
        let a: Vec<f64> = k["args"].as_array().unwrap().iter().map(f1).collect();
        let (block, dps) = (
            a.first().copied().unwrap_or(1.0),
            a.get(1).copied().unwrap_or(period::MOVING_SETPOINT_DPS),
        );
        let got = period::flight_parts(lg, f1(&k["t0"]), f1(&k["t1"]), block, dps).unwrap();
        parts_eq(&got, &k["out"], &w);
        hover += usize::from(got.hover.is_some());
        moving += usize::from(got.moving.is_some());
        neither += usize::from(got.hover.is_none() && got.moving.is_none());
    }
    assert!(
        hover > 20 && moving > 20 && neither > 20,
        "stationnaire {hover}, mouvements {moving}, aucun {neither}"
    );
}

// ---------------------------------------------------------------- primitives float32 de numpy (npstat.rs)

use otb_analysis::npstat::*;

#[test]
fn moyenne_ecart_type_mediane_centile_identiques() {
    let c = cas("flight_stats");
    let p = &c["prim"];
    let lengths: Vec<usize> = p["lengths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();
    let mut checked = 0;
    for (nm, d) in p["signals"].as_object().unwrap() {
        let x = inp32(&c, &d["x"]);
        for (i, &n) in lengths.iter().enumerate() {
            let v = &x[..n];
            let w = format!("{nm} n={n}");
            bits(mean_f32(v) as f64, &d["mean"][i], &format!("{w} moyenne"));
            bits(std_f32(v) as f64, &d["std"][i], &format!("{w} écart-type"));
            bits(median_f32(v) as f64, &d["median"][i], &format!("{w} médiane"));
            if n > 0 {
                bits(percentile_f32(v, 95.0) as f64, &d["p95"][i], &format!("{w} centile 95"));
                bits(max_f32(v).unwrap() as f64, &d["max"][i], &format!("{w} maximum"));
                bits(min_f32(v).unwrap() as f64, &d["min"][i], &format!("{w} minimum"));
            } else {
                assert!(max_f32(v).is_none() && min_f32(v).is_none());
            }
            checked += 1;
        }
    }
    assert!(checked > 1000);
    let x = inp32(&c, &p["signals"]["normal"]["x"]);
    for q in p["quantiles"].as_array().unwrap() {
        let n = q["n"].as_u64().unwrap() as usize;
        bits(
            percentile_f32(&x[..n], f1(&q["q"])) as f64,
            &q["out"],
            &format!("centile {} n={n}", f1(&q["q"])),
        );
    }
}

#[test]
fn histogramme_digitize_linspace_clip_identiques() {
    let c = cas("flight_stats");
    let p = &c["prim"];
    let h = &p["histogram"];
    let got = histogram_counts(
        inp32(&c, &h["values"]).iter().map(|&v| v as f64),
        &input(&c, &h["edges"]),
    );
    let want: Vec<i64> = h["counts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(got, want, "histogramme");
    let d = &p["digitize"];
    let bins = input(&c, &d["bins"]);
    let want: Vec<usize> = d["out"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();
    let got: Vec<usize> = inp32(&c, &d["values"])
        .iter()
        .map(|&v| digitize(v as f64, &bins))
        .collect();
    assert_eq!(got, want, "digitize");
    for l in p["linspace"].as_array().unwrap() {
        let n = l["n"].as_u64().unwrap() as usize;
        assert_bits(
            &linspace(f1(&l["a"]), f1(&l["b"]), n),
            &input(&c, &l["out"]),
            &format!("linspace {l}"),
        );
    }
    let k = &p["clip"];
    let got: Vec<f64> = inp32(&c, &k["values"])
        .iter()
        .map(|&v| clip_f32(v, -100.0, 100.0) as f64)
        .collect();
    let want: Vec<f64> = inp32(&c, &k["out"]).iter().map(|&v| v as f64).collect();
    assert_bits(&got, &want, "clip");
}
