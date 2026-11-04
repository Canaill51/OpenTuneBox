//! Raie fixe et notch dynamique de `advisor/rules.py` en Rust (phase 5a-3c) sur des journaux fabriqués : cas écrits par
//! `scripts/rules_notch_cases.py` (régénérer `tests/data/rules_notch.json` avec ce script si `rules.py` change).
//!
//! Appels directs de `frame_resonance`, `line_follows_motors`, `_frame_resonance`, `dyn_notch_view` et `_dyn_notch_q_n_rules`, qui couvrent
//! toutes les lignes et toutes les branches de ces fonctions côté Python (`coverage run --branch`, 0 manquante). `filter_rules`, qui les
//! assemble, est prouvée par la couche corpus (`otb-parity check-advisor`, 630 sessions). Exigé : textes identiques au caractère près,
//! mêmes constats dans le même ordre, mêmes erreurs ; fréquences de la grille et décisions (raie moteur ou non) identiques ; nombres
//! issus de FFT à une tolérance mesurée (affichée avec `--nocapture`).

use otb_advisor::profiles::profile;
use otb_advisor::rules::{self, DynNotch, Finding};
use otb_log::flightlog::FlightLog;
use otb_log::settings::parse_settings;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;

/// Tolérance des nombres issus de spectres `realfft` / pocketfft (L-048), écart absolu rapporté à max(|valeur|, 1). **Mesurée** : 1,8e-12 au
/// pire sur ces cas (ligne « écart max » avec `--nocapture`), marge ×57.
const FFT_RTOL: f64 = 1e-10;

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn check_finding(what: &str, got: &Finding, want: &Value) {
    let s = |k: &str| want[k].as_str().unwrap().to_string();
    assert_eq!(got.key, s("key"), "{what} : clé");
    assert_eq!(got.category, s("category"), "{what} : catégorie");
    assert_eq!(got.severity, s("severity"), "{what} : gravité");
    assert_eq!(got.title, s("title"), "{what} : titre");
    assert_eq!(got.detail, s("detail"), "{what} : détail");
    assert_eq!(got.actions, strings(&want["actions"]), "{what} : actions");
    assert_eq!(got.axis, want["axis"].as_str().map(str::to_string), "{what} : axe");
    assert_eq!(got.settings, strings(&want["settings"]), "{what} : réglages déclarés");
}

fn check_findings(what: &str, got: &[Finding], want: &Value) {
    let w = want.as_array().unwrap();
    assert_eq!(
        got.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        w.iter().map(|f| f["key"].as_str().unwrap()).collect::<Vec<_>>(),
        "{what} : liste des constats"
    );
    for (g, x) in got.iter().zip(w) {
        check_finding(what, g, x);
    }
}

/// Journal fabriqué : courbes et temps du fichier, réglages relus depuis l'en-tête brut.
fn log_of(c: &Value, v: &Value) -> FlightLog {
    let mut lg = flight_log(c, v);
    let raw: BTreeMap<String, String> = v["raw"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, x)| (k.clone(), x.as_str().unwrap().to_string()))
        .collect();
    lg.settings = parse_settings(&raw);
    lg.pid_columns = strings(&v["columns"]).into_iter().collect::<BTreeSet<_>>();
    lg
}

fn hex_opt(v: &Value) -> Option<f64> {
    (!v.is_null()).then(|| f1(v))
}

/// Compare deux nombres (NaN égaux, infinis égaux) : écart absolu rapporté à max(|Python|, 1) sous `rtol` ; renvoie cet écart.
fn close(what: &str, got: f64, want: f64, rtol: f64) -> f64 {
    if got.is_nan() && want.is_nan() {
        return 0.0;
    }
    assert!(
        got.is_finite() == want.is_finite() && (got.is_nan() == want.is_nan()),
        "{what} : Rust {got:e} / Python {want:e}"
    );
    if !got.is_finite() {
        assert_eq!(got, want, "{what} : infini");
        return 0.0;
    }
    let d = (got - want).abs() / want.abs().max(1.0);
    assert!(d <= rtol, "{what} : Rust {got:e} / Python {want:e} (écart {d:e})");
    d
}

fn close_list(what: &str, got: &[f64], want: &[f64], worst: &mut f64) {
    assert_eq!(got.len(), want.len(), "{what} : longueur");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        *worst = worst.max(close(&format!("{what}[{i}]"), *g, *w, FFT_RTOL));
    }
}

fn dn_of(a: &Value) -> DynNotch {
    DynNotch {
        count: a["count"].as_i64().unwrap(),
        min_hz: a["min_hz"].as_i64().unwrap(),
        max_hz: a["max_hz"].as_i64().unwrap(),
        peaks: hexes(&a["peaks"]),
        widths: hexes(&a["widths"]),
        heights: hexes(&a["heights"]),
        left: hexes(&a["left"]),
        q_need: hex_opt(&a["q_need"]),
    }
}

#[test]
fn appels_directs_identiques_a_python() {
    let c = cas("rules_notch");
    let mut worst = 0.0f64;
    let mut by_call: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_key: BTreeMap<String, usize> = BTreeMap::new();
    let mut n_err = 0;
    for case in c["cases"].as_array().unwrap() {
        let label = case["label"].as_str().unwrap();
        let lg = log_of(&c, &case["log"]);
        let s = &lg.settings;
        let n = lg.n();
        let sel_opt = case["sel"]
            .as_array()
            .map(|x| x[0].as_u64().unwrap() as usize..x[1].as_u64().unwrap() as usize);
        let sel = sel_opt.clone().unwrap_or(0..n);
        let a = &case["args"];
        let call = case["call"].as_str().unwrap();
        let want = &case["out"];
        let err = is_error(case);
        *by_call.entry(call.to_string()).or_insert(0) += 1;
        // Résultat en erreur : Python lève, Rust doit renvoyer `Err`
        macro_rules! ok {
            ($res:expr) => {{
                let r = $res;
                if err {
                    assert!(r.is_err(), "{label} : Python lève, Rust rend {:?}", r.ok());
                    n_err += 1;
                    continue;
                }
                r.unwrap_or_else(|e| panic!("{label} : Rust lève ({e}), Python rend un résultat"))
            }};
        }
        match call {
            "frame" => {
                let got = ok!(rules::frame_resonance(&lg, &sel, f1(&a["lo_hz"])));
                match (got, want["frame"].as_array()) {
                    (None, None) => {}
                    (Some(g), Some(w)) => {
                        assert!(
                            same(g.freq_hz, f1(&w[0])),
                            "{label} : fréquence {} / {}",
                            g.freq_hz,
                            f1(&w[0])
                        );
                        worst = worst.max(close(&format!("{label} : excès"), g.excess_db, f1(&w[1]), FFT_RTOL));
                        assert_eq!(g.near_motor, w[2].as_bool().unwrap(), "{label} : raie moteur");
                    }
                    (g, w) => panic!("{label} : Rust {g:?} / Python {w:?}"),
                }
            }
            "follows" => {
                let z = input(&c, &a["z"]);
                let f = input(&c, &a["f"]);
                let ok_cols: Vec<bool> = a["ok"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_bool().unwrap())
                    .collect();
                let got = ok!(rules::line_follows_motors(&lg, &sel, f1(&a["f0"]), &z, &f, &ok_cols));
                worst = worst.max(close(
                    &format!("{label} : score"),
                    got,
                    hex_opt(&want["num"]).unwrap(),
                    FFT_RTOL,
                ));
            }
            "fres" => {
                let res = (!a["res"].is_null()).then(|| {
                    let r = a["res"].as_array().unwrap();
                    rules::FrameResonance {
                        freq_hz: f1(&r[0]),
                        excess_db: f1(&r[1]),
                        near_motor: r[2].as_bool().unwrap(),
                    }
                });
                let prof = profile(a["profile"].as_str().unwrap()).unwrap();
                let got = ok!(rules::frame_resonance_finding(
                    &lg,
                    res.as_ref(),
                    prof,
                    sel_opt.as_ref()
                ));
                match (&got, want["finding"].is_null()) {
                    (None, true) => {}
                    (Some(g), false) => {
                        check_finding(label, g, &want["finding"]);
                        *by_key.entry(g.key.clone()).or_insert(0) += 1;
                    }
                    (g, _) => panic!(
                        "{label} : Rust {:?} / Python {}",
                        g.as_ref().map(|f| &f.key),
                        want["finding"]
                    ),
                }
            }
            "dnview" => {
                let got = ok!(rules::dyn_notch_view(&lg, &sel, hex_opt(&a["fixed_hz"])));
                match (got, want["dn"].is_object()) {
                    (None, false) => {}
                    (Some(g), true) => {
                        let w = &want["dn"];
                        assert_eq!(g.count, w["count"].as_i64().unwrap(), "{label} : count");
                        assert_eq!(g.min_hz, w["min_hz"].as_i64().unwrap(), "{label} : min_hz");
                        assert_eq!(g.max_hz, w["max_hz"].as_i64().unwrap(), "{label} : max_hz");
                        let wp = hexes(&w["peaks"]);
                        assert_eq!(g.peaks.len(), wp.len(), "{label} : nombre de pics");
                        for (x, y) in g.peaks.iter().zip(&wp) {
                            assert!(same(*x, *y), "{label} : pic {x} / {y}");
                        }
                        close_list(
                            &format!("{label} : largeurs"),
                            &g.widths,
                            &hexes(&w["widths"]),
                            &mut worst,
                        );
                        close_list(
                            &format!("{label} : hauteurs"),
                            &g.heights,
                            &hexes(&w["heights"]),
                            &mut worst,
                        );
                        close_list(
                            &format!("{label} : hauteurs filtrées"),
                            &g.left,
                            &hexes(&w["left"]),
                            &mut worst,
                        );
                        match (g.q_need, hex_opt(&w["q_need"])) {
                            (None, None) => {}
                            (Some(x), Some(y)) => {
                                worst = worst.max(close(&format!("{label} : q_need"), x, y, FFT_RTOL))
                            }
                            (x, y) => panic!("{label} : q_need Rust {x:?} / Python {y:?}"),
                        }
                    }
                    (g, _) => panic!("{label} : Rust {:?} / Python {}", g.map(|d| d.peaks), want["dn"]),
                }
            }
            "qn" => {
                let present: Vec<Finding> = a["present"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| {
                        let mut f = Finding::new(
                            p[0].as_str().unwrap(),
                            "Filtres",
                            "conseil",
                            "t",
                            "d",
                            vec!["a".to_string()],
                            None,
                        );
                        f.settings = strings(&p[1]);
                        f
                    })
                    .collect();
                let got = ok!(rules::dyn_notch_q_n_rules(
                    s,
                    &dn_of(&a["dn"]),
                    a["lines_pass"].as_bool().unwrap(),
                    a["gyro_noisy"].as_bool().unwrap(),
                    a["dterm_noisy"].as_bool().unwrap(),
                    &present,
                    a["hover"].as_bool().unwrap()
                ));
                check_findings(label, &got, &want["findings"]);
                got.iter().for_each(|f| *by_key.entry(f.key.clone()).or_insert(0) += 1);
            }
            other => panic!("appel {other} inconnu"),
        }
    }
    eprintln!("appels par fonction : {by_call:?}");
    eprintln!("constats comparés : {by_key:?}");
    eprintln!("erreurs de Python reproduites : {n_err} ; écart max {worst:.3e}");
    for k in [
        "frame_line_fixed",
        "dyn_notch_q_lower",
        "dyn_notch_q_raise",
        "dyn_notch_more",
    ] {
        assert!(by_key.contains_key(k), "le constat {k} n'est produit par aucun cas");
    }
}
