//! Règles de retard de `advisor/rules.py` en Rust (phase 5a-3a) sur des journaux fabriqués : cas écrits par `scripts/rules_delay_cases.py`
//! (régénérer `tests/data/rules_delay.json` avec ce script si `rules.py` change).
//!
//! Deux familles : `cases` (`analyze` complet : tous les constats et toutes les mesures, `support::check_report`) et `direct` (appel direct de
//! `predict_filter_change`, `_gyro_lpf1_off`, `_dterm_lift`, `_dterm_lift_noise`, `filter_strategy`). Exigé : textes identiques au caractère
//! près, mêmes constats dans le même ordre, mêmes erreurs. Les prévisions du simulateur (FFT) sont comparées avec une tolérance mesurée.

use otb_advisor::profiles::{CraftProfile, profile};
use otb_advisor::rules::{self, Finding};
use otb_analysis::bf_filters::{lowpass_off, scale_lowpass};
use otb_log::flightlog::FlightLog;
use otb_log::settings::Settings;
use serde_json::Value;
use std::collections::BTreeMap;

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;
#[path = "support/mod.rs"]
mod support;
use support::*;

/// Tolérance absolue des prévisions (dB, ms) : les niveaux viennent de spectres `realfft` / pocketfft (L-048). **Mesurée** : voir la ligne
/// « écart max prévisions » affichée avec `--nocapture`.
const PRED_ATOL: f64 = 1e-9;

fn check_opt_finding(what: &str, got: Option<&Finding>, want: &Value) {
    match (got, want.is_null()) {
        (None, true) => {}
        (Some(g), false) => check_finding(what, g, want),
        (Some(g), true) => panic!("{what} : Rust rend « {} », Python rien", g.key),
        (None, false) => panic!("{what} : Python rend « {} », Rust rien", want["key"].as_str().unwrap()),
    }
}

fn profile_of(p: &Value) -> CraftProfile {
    let mut out = profile(p["base"].as_str().unwrap()).unwrap().clone();
    for (k, v) in p["over"].as_object().unwrap() {
        let x = f1(v);
        match k.as_str() {
            "gyro_delay_max_ms" => out.gyro_delay_max_ms = x,
            "dterm_delay_max_ms" => out.dterm_delay_max_ms = x,
            "gyro_noise_high_db" => out.gyro_noise_high_db = x,
            "dterm_noise_high_db" => out.dterm_noise_high_db = x,
            other => panic!("seuil {other} non géré par le test"),
        }
    }
    out
}

fn sel_of(case: &Value, n: usize) -> std::ops::Range<usize> {
    case["sel"].as_array().map_or(0..n, |s| {
        s[0].as_u64().unwrap() as usize..s[1].as_u64().unwrap() as usize
    })
}

#[test]
fn analyze_rend_les_constats_de_retard_de_python() {
    let c = cas("rules_delay");
    let keys = strings(&c["keys"]);
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for case in c["cases"].as_array().unwrap() {
        let label = case["label"].as_str().unwrap();
        let lg = log_of(&c, &case["log"]);
        let prof = profile_of(&case["profile"]);
        let sel = case["sel"]
            .as_array()
            .map(|s| s[0].as_u64().unwrap() as usize..s[1].as_u64().unwrap() as usize);
        let got = rules::analyze(&lg, sel, Some(&prof));
        if is_error(case) {
            assert!(got.is_err(), "{label} : Python lève, Rust rend un rapport");
            continue;
        }
        let rep = got.unwrap_or_else(|e| panic!("{label} : Rust lève ({e}), Python rend un rapport"));
        check_report(label, &lg, &rep, &case["out"], &mut seen);
    }
    report();
    for k in &keys {
        assert!(seen.contains_key(k), "le constat {k} n'est produit par aucun cas");
    }
    eprintln!("constats comparés : {seen:?}");
}

fn mod_settings(lg: &FlightLog, args: &Value) -> Settings {
    let mut m = lg.settings.clone();
    match args["mod"].as_str().unwrap() {
        "gyro_lpf1_off" => m.gyro_lpf1 = lowpass_off(&lg.settings.gyro_lpf1),
        "dterm" => {
            let f = f1(&args["factor"]);
            m.dterm_lpf1 = scale_lowpass(&lg.settings.dterm_lpf1, f).unwrap();
            m.dterm_lpf2 = scale_lowpass(&lg.settings.dterm_lpf2, f).unwrap();
        }
        other => panic!("modification {other} inconnue"),
    }
    m
}

fn finding_of(v: &Value) -> Finding {
    let mut f = Finding::new(
        v["key"].as_str().unwrap(),
        "Filtres",
        v["severity"].as_str().unwrap(),
        v["title"].as_str().unwrap(),
        "d",
        strings(&v["actions"]),
        None,
    );
    f.settings = Vec::new();
    f
}

#[test]
fn appels_directs_identiques_a_python() {
    let c = cas("rules_delay");
    let mut worst = 0.0f64;
    let mut n_find = 0;
    for case in c["direct"].as_array().unwrap() {
        let label = case["label"].as_str().unwrap();
        let lg = log_of(&c, &case["log"]);
        let prof = profile_of(&case["profile"]);
        let sel = sel_of(case, lg.n());
        let a = &case["args"];
        let list = |k: &str| hexes(&a[k]);
        let call = case["call"].as_str().unwrap();
        let want = &case["out"];
        let err = is_error(case);
        let check = |res: Result<Option<Finding>, String>| {
            if err {
                assert!(res.is_err(), "{label} : Python lève, Rust rend {res:?}");
                return;
            }
            let got = res.unwrap_or_else(|e| panic!("{label} : Rust lève ({e}), Python rend un résultat"));
            check_opt_finding(label, got.as_ref(), &want["finding"]);
        };
        match call {
            "predict" => {
                let hi = f1(&a["hi"]);
                let got = rules::predict_filter_change(&lg, &sel, &mod_settings(&lg, a), hi);
                if err {
                    assert!(got.is_err(), "{label} : Python lève, Rust rend une prévision");
                    continue;
                }
                let p = got.unwrap_or_else(|e| panic!("{label} : Rust lève ({e})"));
                let w = &want["pred"];
                for (name, g) in [
                    ("gyro_mean", &p.gyro_mean),
                    ("gyro_peak", &p.gyro_peak),
                    ("d_mean", &p.d_mean),
                    ("d_peak", &p.d_peak),
                ] {
                    let wv = hexes(&w[name]);
                    assert_eq!(g.len(), wv.len(), "{label} : longueur de {name}");
                    for (x, y) in g.iter().zip(&wv) {
                        worst = worst.max((x - y).abs());
                        assert!((x - y).abs() <= PRED_ATOL, "{label} : {name} Rust {x:e} / Python {y:e}");
                    }
                }
                for (name, g) in [
                    ("gyro_delay_gain_ms", p.gyro_delay_gain_ms),
                    ("d_delay_gain_ms", p.d_delay_gain_ms),
                ] {
                    let y = f1(&w[name]);
                    worst = worst.max((g - y).abs());
                    assert!((g - y).abs() <= PRED_ATOL, "{label} : {name} Rust {g:e} / Python {y:e}");
                }
            }
            "gyro_lpf1_off" => check(rules::gyro_lpf1_off(
                &lg,
                &sel,
                f1(&a["hi"]),
                &prof,
                &list("g_db"),
                &list("d_db"),
                f1(&a["d_ptb"]),
                a["hover"].as_bool().unwrap(),
            )),
            "dterm_lift" => check(
                rules::dterm_lift(
                    &lg,
                    &sel,
                    f1(&a["hi"]),
                    &prof,
                    f1(&a["delay_ms"]),
                    &list("d_db"),
                    f1(&a["d_ptb"]),
                    a["d_goal"].as_str().unwrap(),
                )
                .map(Some),
            ),
            "dterm_lift_noise" => check(rules::dterm_lift_noise(
                &lg,
                &sel,
                f1(&a["hi"]),
                &prof,
                &list("d_db"),
                f1(&a["d_ptb"]),
                a["hover"].as_bool().unwrap(),
                f1(&a["delay_ms"]),
            )),
            "strategy" => {
                let fs: Vec<Finding> = a["findings"].as_array().unwrap().iter().map(finding_of).collect();
                check(rules::filter_strategy(&fs, a["moving"].as_bool().unwrap()));
            }
            other => panic!("appel {other} inconnu"),
        }
        n_find += 1;
    }
    eprintln!("écart max prévisions {worst:.3e} ; {n_find} appels comparés");
}

#[test]
fn main_action_prend_l_action_qui_change_un_reglage() {
    let mk = |actions: &[&str]| {
        Finding::new(
            "k",
            "Filtres",
            "conseil",
            "t",
            "d",
            actions.iter().map(|s| s.to_string()).collect(),
            None,
        )
    };
    assert_eq!(rules::main_action(&mk(&["texte", "a = b"])).unwrap(), "a = b");
    assert_eq!(rules::main_action(&mk(&["texte", "x → y", "a = b"])).unwrap(), "x → y");
    assert_eq!(
        rules::main_action(&mk(&["texte", "Curseur « D »"])).unwrap(),
        "Curseur « D »"
    );
    assert_eq!(rules::main_action(&mk(&["premier", "second"])).unwrap(), "premier");
    assert!(rules::main_action(&mk(&[])).is_err());
}
