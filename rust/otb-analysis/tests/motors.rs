//! `analysis/motors.py` (la partie Python : colle des boucles de la phase 3b) porté en Rust : cas écrits par
//! `scripts/motors_cases.py` (régénérer `tests/data/motors.json` avec ce script si `motors.py` change).
//!
//! Exigé **identique** : les vitesses moteur estimées, les centres des notchs, `hover_motor_hz`, `distribution`, les fréquences des
//! spectres, les fréquences et le nombre des pics, les erreurs. À une tolérance mesurée (L-048, L-051) : tout ce qui passe par une
//! FFT, `hypot`, `log10` ou `pow`.

use otb_analysis::motors::{self, NotchTargets};
use otb_log::flightlog::Rows;
use otb_log::settings::Settings;
use serde_json::Value;

mod common;
use common::*;

/// Spectres, bruit moteur, `rpm_weight_needed` : FFT, `log10`, `pow`. Mesuré le 2026-10-30 : 7,1e-12 au pire (écart-type en dB), le reste
/// ≤ 1e-12. Marge ×140 (autre libm, autre FFT : L-048).
const SPEC_RTOL: f64 = 1e-9;
/// Module des notchs RPM : `hypot` de Rust contre le module SIMD de numpy, à un dernier bit près. Mesuré : 2,2e-16.
const ATT_RTOL: f64 = 1e-12;
/// Hauteurs de pics en dB (`log10` d'un rapport de moyennes de spectres). Mesuré : 8,5e-13 (`harmonic_prominence`).
const DB_RTOL: f64 = 1e-9;

fn rows_of(c: &Value, o: &Value, n: usize) -> Rows {
    rows32(c, o, n)
}

fn to_f64(r: &Rows) -> Vec<f64> {
    r.data.iter().map(|&v| v as f64).collect()
}

fn breaks(k: &Value) -> Option<Vec<i64>> {
    k["breaks"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_i64().unwrap()).collect())
}

struct Scene {
    fs: f64,
    x: Vec<f64>,
    mhz: Rows,
}

fn scenes(c: &Value) -> Vec<Scene> {
    c["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let n = s["n"].as_u64().unwrap() as usize;
            Scene {
                fs: f1(&s["fs"]),
                x: inp32(c, &s["x"]).iter().map(|&v| v as f64).collect(),
                mhz: rows_of(c, &s["mhz"], n),
            }
        })
        .collect()
}

fn rpm_settings(c: &Value, key: &Value) -> Option<Settings> {
    key.as_str().map(|k| log_settings(&c["settings"][k]))
}

/// Durée de fenêtre d'un enregistrement (300 ms par défaut).
fn seg(k: &Value) -> f64 {
    k.get("seg_ms").map_or(300.0, f1)
}

fn str_list(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect()
}

#[test]
fn motor_hz_identique() {
    let c = cas("motors");
    let logs: Vec<_> = c["logs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| flight_log(&c, v))
        .collect();
    let (mut some, mut none, mut est) = (0, 0, 0);
    for k in c["motor_hz"].as_array().unwrap() {
        let lg = &logs[k["log"].as_u64().unwrap() as usize];
        let what = format!(
            "motor_hz {} × {}",
            c["log_names"][k["log"].as_u64().unwrap() as usize],
            k["mult"]
        );
        assert!(!is_error(k), "{what}");
        let mult = (!k["mult"].is_null()).then(|| f1(&k["mult"]));
        let (got, e) = motors::motor_hz(lg, mult);
        assert_eq!(e, k["out"]["est"].as_bool().unwrap(), "{what} : indicateur");
        est += usize::from(e);
        match (&k["out"]["hz"], got) {
            (Value::Null, None) => none += 1,
            (o, Some(g)) if !o.is_null() => {
                some += 1;
                assert_eq!(g.k as u64, o["k"].as_u64().unwrap(), "{what} : forme");
                let want: Vec<f64> = inp32(&c, &o["x"]).iter().map(|&v| v as f64).collect();
                assert_bits(&to_f64(&g), &want, &what);
            }
            (o, g) => panic!("{what} : Python {o}, Rust {}", g.is_some()),
        }
    }
    assert!(some > 40 && none > 3 && est > 30, "{some} {none} {est}");
}

#[test]
fn dyn_notch_centres_identique() {
    let c = cas("motors");
    let logs: Vec<_> = c["logs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| flight_log(&c, v))
        .collect();
    let (mut some, mut none) = (0, 0);
    for k in c["dyn_centres"].as_array().unwrap() {
        let lg = &logs[k["log"].as_u64().unwrap() as usize];
        let what = format!(
            "dyn_notch_centres {}",
            c["log_names"][k["log"].as_u64().unwrap() as usize]
        );
        assert!(!is_error(k), "{what}");
        match (&k["out"], motors::dyn_notch_centres(lg)) {
            (Value::Null, None) => none += 1,
            (o, Some(g)) if !o.is_null() => {
                some += 1;
                assert_eq!(g.len() as u64, o["k"].as_u64().unwrap(), "{what} : forme");
                let flat: Vec<f64> = g.concat();
                assert_bits(&flat, &input(&c, &o["x"]), &what);
            }
            (o, g) => panic!("{what} : Python {o}, Rust {g:?}"),
        }
    }
    assert!(some > 60 && none > 60, "{some} {none}");
}

#[test]
fn hover_motor_hz_identique() {
    let c = cas("motors");
    let logs: Vec<_> = c["logs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| flight_log(&c, v))
        .collect();
    let (mut some, mut none) = (0, 0);
    for k in c["hover"].as_array().unwrap() {
        let lg = &logs[k["log"].as_u64().unwrap() as usize];
        let what = format!("hover {}", c["log_names"][k["log"].as_u64().unwrap() as usize]);
        match (k["none"].as_bool().unwrap(), motors::hover_motor_hz(lg)) {
            (true, None) => none += 1,
            (false, Some(g)) => {
                some += 1;
                assert!(
                    same(g, f1(&k["out"])),
                    "{what} : Rust {g:e} / Python {:e}",
                    f1(&k["out"])
                );
            }
            (p, g) => panic!("{what} : Python None = {p}, Rust {g:?}"),
        }
    }
    assert!(some >= 2 && none >= 4, "{some} {none}");
}

#[test]
fn distribution_et_poids_identiques() {
    let c = cas("motors");
    for k in c["distribution"].as_array().unwrap() {
        let (v, f) = (inp32(&c, &k["values"]), input(&c, &k["freqs"]));
        assert!(!is_error(k));
        assert_bits(&motors::distribution(&v, &f), &input(&c, &k["out"]), "distribution");
    }
    for k in c["weight"].as_array().unwrap() {
        let g = motors::rpm_weight_needed(f1(&k["in"]));
        assert_close(
            "rpm_weight_needed",
            &[g],
            &[f1(&k["out"])],
            SPEC_RTOL,
            &format!("poids pour {}", f1(&k["in"])),
        );
    }
    report();
}

#[test]
fn bruit_moteur_et_proeminence_conformes() {
    let c = cas("motors");
    let sc = scenes(&c);
    let (mut n_noise, mut n_prom, mut errs) = (0, 0, 0);
    for k in c["noise"].as_array().unwrap() {
        let s = &sc[k["scene"].as_u64().unwrap() as usize];
        let what = format!(
            "motor_noise scène {} h={} {} {} {}",
            k["scene"], k["n_harm"], k["seg_ms"], k["min_hz"], k["breaks"]
        );
        let b = breaks(k);
        let got = motors::motor_noise(
            &s.x,
            &s.mhz,
            s.fs,
            k["n_harm"].as_i64().unwrap(),
            f1(&k["seg_ms"]),
            f1(&k["min_hz"]),
            b.as_deref(),
        );
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève");
            errs += 1;
            continue;
        }
        let g = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        assert_eq!(g.harmonics, str_list(&k["out"]["h"]), "{what} : harmoniques");
        assert_close(
            "motor_noise moyenne",
            &g.mean_db,
            &input(&c, &k["out"]["mean"]),
            SPEC_RTOL,
            &what,
        );
        assert_close(
            "motor_noise écart-type",
            &g.std_db,
            &input(&c, &k["out"]["std"]),
            SPEC_RTOL,
            &what,
        );
        n_noise += 1;
    }
    for k in c["prominence"].as_array().unwrap() {
        let s = &sc[k["scene"].as_u64().unwrap() as usize];
        let what = format!(
            "harmonic_prominence scène {} h={} {} {} {}",
            k["scene"], k["n_harm"], k["seg_ms"], k["min_hz"], k["breaks"]
        );
        let b = breaks(k);
        let got = motors::harmonic_prominence(
            &s.x,
            &s.mhz,
            s.fs,
            k["n_harm"].as_i64().unwrap(),
            f1(&k["seg_ms"]),
            f1(&k["min_hz"]),
            b.as_deref(),
        );
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève");
            errs += 1;
            continue;
        }
        let g = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        assert_close("harmonic_prominence", &g, &input(&c, &k["out"]), DB_RTOL, &what);
        n_prom += 1;
    }
    assert!(n_noise > 100 && n_prom > 100 && errs >= 1, "{n_noise} {n_prom} {errs}");
    report();
}

#[test]
fn rpm_attenuation_conforme() {
    let c = cas("motors");
    let (mut ok, mut errs) = (0, 0);
    for k in c["attenuation"].as_array().unwrap() {
        let s = log_settings(&c["settings"][k["settings"].as_str().unwrap()]);
        let what = format!("rpm_attenuation {} k={} nwin={}", k["settings"], k["k"], k["nwin"]);
        let (f, m) = (input(&c, &k["f"]), input(&c, &k["mhz"]));
        let got = motors::rpm_attenuation(
            &f,
            &m,
            k["k"].as_u64().unwrap() as usize,
            k["nwin"].as_u64().unwrap() as usize,
            &s,
        );
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève");
            errs += 1;
            continue;
        }
        let g = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        assert_close_rel(
            "rpm_attenuation (case par case)",
            &g,
            &input(&c, &k["out"]),
            ATT_RTOL,
            &what,
        );
        ok += 1;
    }
    assert!(ok > 15 && errs == 3, "{ok} {errs}");
    report();
}

#[test]
fn spectre_apres_filtre_rpm_conforme() {
    let c = cas("motors");
    let sc = scenes(&c);
    let (mut ok, mut empty) = (0, 0);
    for k in c["filtered"].as_array().unwrap() {
        let s = &sc[k["scene"].as_u64().unwrap() as usize];
        let what = format!(
            "rpm_filtered_spectrum scène {} {} m={} {}",
            k["scene"], k["rpm"], k["use_m"], k["breaks"]
        );
        let (rpm, b) = (rpm_settings(&c, &k["rpm"]), breaks(k));
        let mhz = k["use_m"].as_bool().unwrap().then_some(&s.mhz);
        let got = motors::rpm_filtered_spectrum(&s.x, mhz, s.fs, rpm.as_ref(), seg(k), b.as_deref());
        assert!(!is_error(k), "{what}");
        let (f, p) = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        assert_bits(&f, &input(&c, &k["out"]["f"]), &format!("{what} fréquences"));
        assert_close_rel(
            "rpm_filtered_spectrum (case par case)",
            &p,
            &input(&c, &k["out"]["s"]),
            SPEC_RTOL,
            &what,
        );
        ok += 1;
        empty += usize::from(f.is_empty());
    }
    assert!(ok > 50 && empty >= 3, "{ok} {empty}");
    report();
}

fn targets_eq(g: &NotchTargets, want: &Value, c: &Value, what: &str) {
    assert_bits(&g.freqs, &input(c, &want["f"]), &format!("{what} fréquences des pics"));
    assert_close(
        "dyn_notch_targets dB",
        &g.ratios_db,
        &input(c, &want["db"]),
        DB_RTOL,
        what,
    );
    assert!(
        same(g.band.0, f1(&want["band"][0])) && same(g.band.1, f1(&want["band"][1])),
        "{what} bande"
    );
}

#[test]
fn notch_dynamique_conforme() {
    let c = cas("motors");
    let sc = scenes(&c);
    let (mut with_peaks, mut none, mut target_errors) = (0, 0, 0);
    for k in c["targets"].as_array().unwrap() {
        let s = &sc[k["scene"].as_u64().unwrap() as usize];
        let what = format!(
            "dyn_notch_targets scène {} {} m={} {} bande {} pas {}",
            k["scene"], k["rpm"], k["use_m"], k["breaks"], k["band"], k["bin_hz"]
        );
        let (rpm, b) = (rpm_settings(&c, &k["rpm"]), breaks(k));
        let mhz = k["use_m"].as_bool().unwrap().then_some(&s.mhz);
        let band = (f1(&k["band"][0]), f1(&k["band"][1]));
        let got = motors::dyn_notch_targets(
            &s.x,
            mhz,
            s.fs,
            band,
            rpm.as_ref(),
            seg(k),
            f1(&k["bin_hz"]),
            b.as_deref(),
        );
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            target_errors += 1;
            continue;
        }
        let g = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        targets_eq(&g, &k["out"], &c, &what);
        if g.freqs.is_empty() { none += 1 } else { with_peaks += 1 }
    }
    assert!(
        with_peaks > 60 && none > 30 && target_errors >= 3,
        "{with_peaks} {none} {target_errors}"
    );
    report();
}

#[test]
fn raies_conformes() {
    let c = cas("motors");
    let sc = scenes(&c);
    let (mut shapes, mut shape_err, mut lines_n, mut lines_nonempty, mut heights, mut lines_err) = (0, 0, 0, 0, 0, 0);
    for k in c["shapes"].as_array().unwrap() {
        let s = &sc[k["scene"].as_u64().unwrap() as usize];
        let what = format!(
            "line_shapes scène {} h={} {} {}",
            k["scene"], k["n_harm"], k["rpm"], k["breaks"]
        );
        let (rpm, b, fr) = (rpm_settings(&c, &k["rpm"]), breaks(k), input(&c, &k["freqs"]));
        let got = motors::line_shapes(
            &s.x,
            &s.mhz,
            s.fs,
            &fr,
            k["n_harm"].as_i64().unwrap(),
            seg(k),
            b.as_deref(),
            rpm.as_ref(),
        );
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève");
            shape_err += 1;
            continue;
        }
        let g = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let flat: Vec<f64> = g.iter().flat_map(|(w, h)| [*w, *h]).collect();
        assert_close("line_shapes", &flat, &input(&c, &k["out"]), DB_RTOL, &what);
        shapes += 1;
    }
    for k in c["heights"].as_array().unwrap() {
        let s = &sc[k["scene"].as_u64().unwrap() as usize];
        let what = format!("line_heights scène {}", k["scene"]);
        let got = motors::line_heights(&s.x, s.fs, &input(&c, &k["freqs"]), seg(k), None).unwrap();
        assert_close("line_heights", &got, &input(&c, &k["out"]), DB_RTOL, &what);
        heights += 1;
    }
    for k in c["lines"].as_array().unwrap() {
        let s = &sc[k["scene"].as_u64().unwrap() as usize];
        let what = format!(
            "spectrum_lines scène {} h={} bande {} min {}",
            k["scene"], k["n_harm"], k["band"], k["min_db"]
        );
        let band = (f1(&k["band"][0]), f1(&k["band"][1]));
        let xf = f1(&k["xf"]);
        let filt: Vec<f64> = s.x.iter().map(|v| v * xf).collect();
        let got = motors::spectrum_lines(
            &s.x,
            &filt,
            &s.mhz,
            s.fs,
            band,
            f1(&k["min_db"]),
            k["n_harm"].as_i64().unwrap(),
            seg(k),
            None,
        );
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève");
            lines_err += 1;
            continue;
        }
        let g = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let want = k["out"].as_array().unwrap();
        assert_eq!(g.len(), want.len(), "{what} : nombre de raies");
        for (l, w) in g.iter().zip(want) {
            assert!(
                same(l.freq_hz, f1(&w[0])),
                "{what} : fréquence {} / {}",
                l.freq_hz,
                f1(&w[0])
            );
            assert_close("spectrum_lines hauteur", &[l.raw_db], &[f1(&w[1])], DB_RTOL, &what);
            assert_close(
                "spectrum_lines après filtres",
                &[l.left_db],
                &[f1(&w[2])],
                DB_RTOL,
                &what,
            );
        }
        lines_n += 1;
        lines_nonempty += usize::from(!g.is_empty());
    }
    assert!(
        shapes > 80 && shape_err >= 10 && heights >= 10,
        "{shapes} {shape_err} {heights}"
    );
    assert!(
        lines_n > 80 && lines_nonempty > 20 && lines_err >= 10,
        "{lines_n} {lines_nonempty} {lines_err}"
    );
    report();
}
