//! `advisor/step_tuning.py` en Rust (phase 5a-5) sur des listes de `FlightStep` fabriquées : cas écrits par `scripts/step_tuning_cases.py`
//! (régénérer `tests/data/step_tuning.json` avec ce script si `step_tuning.py` ou `step_series.py` change).
//!
//! - formats : `fmt`, `n_text`, `gap_action` pour des valeurs aux frontières (arrondi au pair, NaN, infinis, -0,0) ;
//! - `step_tuning` sur des vols fabriqués (grilles de pics aux seuils, tirages libres de séries, conditions du protocole, ex æquo, erreurs) ;
//! - appels directs de chaque bloc avec des séries écrites à la main ;
//! - textes, gravités, clés, indices du vol conseillé, nombre de blocs et lignes comparés **au caractère près** (aucune tolérance : le
//!   diagnostic ne lit que des mesures déjà calculées) ; les erreurs de Python (`Err` ici) sont rejouées ;
//! - chaque constat produit par `step_tuning.py` apparaît au moins une fois dans les cas (liste dans `ALL_KEYS`).

use otb_advisor::step_tuning::{self, TuningBlock, TuningReport};
use otb_analysis::sliders::PTB_CATEGORIES;
use otb_analysis::step::StepMetrics;
use otb_analysis::step_series::{FlightConditions, FlightStep, Series, Sig};
use serde_json::Value;
use std::collections::BTreeMap;

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;
#[path = "support/mod.rs"]
mod support;
use support::*;

/// Toutes les clés de constat de `step_tuning.py` (relevées dans le source Python : chaque `Finding("…"`).
const ALL_KEYS: [&str; 17] = [
    "ff_active",
    "ff_series_angle",
    "i_slider_out",
    "few_windows",
    "few_windows_unsure",
    "series_incomplete",
    "series_more_overshoot",
    "series_already_flat",
    "series_choice",
    "master_near_optimum",
    "master_no_sign",
    "master_choice",
    "pitch_series_incomplete",
    "pitch_balance_ok",
    "pitch_balance_near",
    "pitch_balance_slower",
    "ff_series_info",
];

fn data() -> Value {
    cas("step_tuning")
}

fn flights_of(v: &Value) -> Vec<FlightStep> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let metrics = f["metrics"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| StepMetrics {
                    peak: f1(&m["peak"]),
                    latency_ms: f1(&m["latency_ms"]),
                    ripple: f1(&m["ripple"]),
                    ripple_std: f1(&m["ripple_std"]),
                    waves_ratio: f1(&m["waves_ratio"]),
                    ..StepMetrics::default()
                })
                .collect();
            let ff = f["ff"].as_array().unwrap();
            FlightStep {
                label: f["label"].as_str().unwrap().to_string(),
                sliders: PTB_CATEGORIES
                    .iter()
                    .zip(hexes(&f["sliders"]))
                    .map(|((c, _), v)| (*c, v))
                    .collect(),
                filters: Sig::T(vec![Sig::I(f["filters"].as_i64().unwrap())]),
                conditions: FlightConditions {
                    ff: (ff[0].as_i64().unwrap(), ff[1].as_i64().unwrap()),
                    angle_share: f1(&f["angle_share"]),
                    i_slider: f1(&f["i_slider"]),
                },
                metrics,
                n: f["n"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap() as usize)
                    .collect(),
                sp_delay_ms: vec![16.0, 16.5],
                loops: Vec::new(),
            }
        })
        .collect()
}

fn series_of(v: &Value) -> Series {
    let key = v["key"].as_str().unwrap();
    Series {
        // la clé est un `&'static str` de `PTB_CATEGORIES` ; une clé inconnue (cas KeyError) reste inconnue de `slider_name`
        key: PTB_CATEGORIES
            .iter()
            .map(|(c, _)| *c)
            .find(|c| *c == key)
            .unwrap_or_else(|| Box::leak(key.to_string().into_boxed_str())),
        flights: v["flights"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap() as usize)
            .collect(),
        values: hexes(&v["values"]),
    }
}

fn check_series(what: &str, got: &Option<Series>, want: &Value) {
    match (got, want.is_null()) {
        (None, true) => {}
        (Some(g), false) => {
            assert_eq!(g.key, want["key"].as_str().unwrap(), "{what} : clé de la série");
            let idx: Vec<usize> = want["flights"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(g.flights, idx, "{what} : vols de la série");
            assert_bits(
                &g.values,
                &hexes(&want["values"]),
                &format!("{what} : valeurs de la série"),
            );
        }
        (g, w) => panic!("{what} : série : Rust {}, Python None={w}", g.is_some()),
    }
}

fn check_block(what: &str, got: &TuningBlock, want: &Value) {
    assert_eq!(got.title, want["title"].as_str().unwrap(), "{what} : titre");
    check_series(what, &got.series, &want["series"]);
    assert_eq!(got.lines, strings(&want["lines"]), "{what} : lignes ({})", got.title);
    assert_eq!(
        got.best,
        want["best"].as_u64().map(|b| b as usize),
        "{what} : vol conseillé ({})",
        got.title
    );
    match (&got.finding, want["finding"].is_null()) {
        (None, true) => {}
        (Some(g), false) => check_finding(&format!("{what} : bloc {}", got.title), g, &want["finding"]),
        (g, w) => panic!("{what} : constat : Rust {}, Python None={w}", g.is_some()),
    }
}

fn check_report(what: &str, got: &TuningReport, want: &Value) {
    let wc = want["conditions"].as_array().unwrap();
    assert_eq!(
        got.conditions.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        wc.iter().map(|f| f["key"].as_str().unwrap()).collect::<Vec<_>>(),
        "{what} : conditions"
    );
    for (g, w) in got.conditions.iter().zip(wc) {
        check_finding(&format!("{what} : condition"), g, w);
    }
    let wb = want["blocks"].as_array().unwrap();
    assert_eq!(got.blocks.len(), wb.len(), "{what} : nombre de blocs");
    for (g, w) in got.blocks.iter().zip(wb) {
        check_block(what, g, w);
    }
}

#[test]
fn formats_comme_python() {
    let c = data();
    for r in c["fmt"].as_array().unwrap() {
        let x = f1(&r[0]);
        assert_eq!(step_tuning::fmt(x), r[1].as_str().unwrap(), "fmt({x:e})");
    }
    let mut signed = 0;
    for r in c["n"].as_array().unwrap() {
        let (x, d, sg) = (f1(&r[0]), r[1].as_u64().unwrap() as usize, r[2].as_bool().unwrap());
        assert_eq!(
            step_tuning::n_text(x, d, sg),
            r[3].as_str().unwrap(),
            "n({x:e}, {d}, {sg})"
        );
        signed += sg as usize;
    }
    assert!(signed > 50);
    for r in c["gap_action"].as_array().unwrap() {
        let g = f1(&r[0]);
        assert_eq!(step_tuning::gap_action(g), r[1].as_str().unwrap(), "gap_action({g:e})");
    }
}

#[test]
fn step_tuning_et_blocs_rendent_le_diagnostic_de_python() {
    let c = data();
    let mut keys: BTreeMap<String, usize> = BTreeMap::new();
    let (mut errors, mut tuning, mut direct) = (0, 0, 0);
    for case in c["cases"].as_array().unwrap() {
        let label = case["label"].as_str().unwrap();
        let kind = case["kind"].as_str().unwrap();
        let fl = flights_of(&case["flights"]);
        let series = (!case["series"].is_null()).then(|| series_of(&case["series"]));
        let blocks: Result<Vec<TuningBlock>, String>;
        let mut report: Option<TuningReport> = None;
        match kind {
            "tuning" => {
                tuning += 1;
                match step_tuning::step_tuning(&fl) {
                    Ok(r) => {
                        blocks = Ok(Vec::new());
                        report = Some(r);
                    }
                    Err(e) => blocks = Err(e),
                }
            }
            _ => {
                direct += 1;
                let s = series.as_ref();
                let ref_ = case["ref"].as_u64().map(|r| r as usize);
                blocks = match kind {
                    "damping" => step_tuning::damping_block(&fl, s.unwrap()).map(|b| vec![b]),
                    "master" => step_tuning::master_block(&fl, s.unwrap()).map(|b| vec![b]),
                    "ff" => step_tuning::ff_block(&fl, s.unwrap()).map(|b| vec![b]),
                    "roll_pitch" | "roll_pitch_ref" => {
                        step_tuning::roll_pitch_block(&fl, s, ref_).map(|b| b.into_iter().collect())
                    }
                    other => panic!("{label} : genre de cas inconnu {other}"),
                };
            }
        }
        if is_error(case) {
            errors += 1;
            assert!(blocks.is_err(), "{label} : Python lève, Rust rend un diagnostic");
            continue;
        }
        let blocks = blocks.unwrap_or_else(|e| panic!("{label} : Rust lève ({e}), Python rend un diagnostic"));
        match report {
            Some(r) => {
                check_report(label, &r, &case["out"]);
                for f in r
                    .conditions
                    .iter()
                    .chain(r.blocks.iter().filter_map(|b| b.finding.as_ref()))
                {
                    *keys.entry(f.key.clone()).or_insert(0) += 1;
                }
            }
            None => {
                // `roll_pitch_block` rend `None` quand rien ne se juge : Python rend alors `None`
                match (blocks.first(), case["out"].is_null()) {
                    (None, true) => {}
                    (Some(b), false) => check_block(label, b, &case["out"]),
                    (b, w) => panic!("{label} : bloc : Rust {}, Python None={w}", b.is_some()),
                }
                for f in blocks.iter().filter_map(|b| b.finding.as_ref()) {
                    *keys.entry(f.key.clone()).or_insert(0) += 1;
                }
            }
        }
    }
    assert!(
        errors >= 15 && tuning >= 2000 && direct >= 40,
        "{errors} erreurs, {tuning} step_tuning, {direct} appels directs"
    );
    for k in ALL_KEYS {
        assert!(keys.contains_key(k), "le constat {k} n'est produit par aucun cas");
    }
    eprintln!(
        "constats comparés : {keys:?} ({tuning} listes de vols, {direct} appels directs, {errors} erreurs rejouées)"
    );
}
