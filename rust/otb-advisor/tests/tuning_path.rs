//! `advisor/tuning_path.py` porté en Rust (phase 5b-3) : cas écrits par `scripts/tuning_path_cases.py` (régénérer
//! `tests/data/tuning_path.json` avec ce script si `tuning_path.py` change).
//!
//! - `phases(card)` : toutes les cases du parcours pour chaque version × taille × boucle PID × ESC du menu, **titre et aide au caractère
//!   près** (résolus pour la fiche), plus tout ce qui pilote le guide (vérifications, usages, vol de série, outil, partie, rôle) ;
//! - `auto_status` : l'état de chaque case pour des listes de `LogCheck` fabriquées (points et usages dans tous leurs états, rôles,
//!   curseurs au voisinage des valeurs de série), avec et sans repères de la phase Filtres.

use otb_advisor::drone_card::DroneCard;
use otb_advisor::log_check::{CheckItem, FilterTargets, LogCheck, Usage};
use otb_advisor::tuning_path::{self, Phase, Step};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;

fn data() -> Value {
    cas("tuning_path")
}

fn card_of(k: &Value) -> DroneCard {
    let mut d = k.clone();
    d["name"] = json!("Test");
    d["pid_khz"] = json!(f1(&d["pid_khz"]));
    DroneCard::from_value(&d).unwrap()
}

fn text<'a>(c: &'a Value, i: &Value) -> &'a str {
    c["strings"][i.as_u64().unwrap() as usize].as_str().unwrap()
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn check_step(what: &str, c: &Value, got: &Step, w: &Value) {
    let key = &got.key;
    assert_eq!(*key, w[0].as_str().unwrap(), "{what} : clé de case");
    assert_eq!(got.title, text(c, &w[1]), "{what} : {key} titre");
    assert_eq!(got.help, text(c, &w[2]), "{what} : {key} aide");
    assert_eq!(got.checks, strs(&w[3]), "{what} : {key} vérifications");
    assert_eq!(got.usages, strs(&w[4]), "{what} : {key} usages");
    assert_eq!(got.any_log, w[5].as_bool().unwrap(), "{what} : {key} any_log");
    match (&got.flight, &w[6]) {
        (None, Value::Null) => {}
        (Some((cat, v)), f) => {
            assert_eq!(*cat, f[0].as_str().unwrap(), "{what} : {key} curseur");
            assert!(
                same(*v, f1(&f[1])),
                "{what} : {key} valeur : Rust {v} / Python {}",
                f1(&f[1])
            );
        }
        (g, f) => panic!("{what} : {key} vol de série : Rust {g:?} / Python {f}"),
    }
    assert_eq!(got.flight_usage, w[7].as_str().unwrap(), "{what} : {key} usage de vol");
    assert_eq!(got.tool, w[8].as_str(), "{what} : {key} outil");
    assert_eq!(got.part, w[9].as_str(), "{what} : {key} partie");
    assert_eq!(got.targets, w[10].as_bool().unwrap(), "{what} : {key} repères");
    assert_eq!(got.logs, w[11].as_str().unwrap(), "{what} : {key} rôle");
    assert_eq!(got.open_logs, w[12].as_bool().unwrap(), "{what} : {key} bouton");
}

fn check_phases(what: &str, c: &Value, got: &[Phase], w: &Value) {
    let w = w.as_array().unwrap();
    assert_eq!(got.len(), w.len(), "{what} : nombre de phases");
    for (g, p) in got.iter().zip(w) {
        assert_eq!(g.key, p[0].as_str().unwrap(), "{what} : clé de phase");
        assert_eq!(g.title, text(c, &p[1]), "{what} : {} titre", g.key);
        assert_eq!(g.intro, text(c, &p[2]), "{what} : {} introduction", g.key);
        let steps = p[3].as_array().unwrap();
        assert_eq!(
            g.steps.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(),
            steps.iter().map(|s| s[0].as_str().unwrap()).collect::<Vec<_>>(),
            "{what} : cases de {}",
            g.key
        );
        for (gs, ws) in g.steps.iter().zip(steps) {
            check_step(&format!("{what} / {}", g.key), c, gs, ws);
        }
    }
}

#[test]
fn le_parcours_a_les_memes_cases_et_les_memes_textes() {
    let c = data();
    let cards = c["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 300);
    let mut steps = 0;
    for k in cards {
        let card = card_of(&k["card"]);
        let got = tuning_path::phases(&card);
        steps = got.iter().map(|p| p.steps.len()).sum();
        check_phases(&format!("{:?}", k["card"]), &c, &got, &k["phases"]);
    }
    assert_eq!(steps as i64, c["n_steps"].as_i64().unwrap());
}

fn check_of(v: &Value) -> LogCheck {
    LogCheck {
        label: v["label"].as_str().unwrap().to_string(),
        moving: true,
        hover: None,
        moving_part: None,
        items: v["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| CheckItem {
                key: i[0].as_str().unwrap().to_string(),
                title: i[1].as_str().unwrap().to_string(),
                status: match i[2].as_str().unwrap() {
                    "ok" => otb_advisor::log_check::OK,
                    "à revoir" => otb_advisor::log_check::WARN,
                    "bloquant" => otb_advisor::log_check::BLOCK,
                    "info" => otb_advisor::log_check::INFO,
                    s => panic!("état {s}"),
                },
                detail: i[3].as_str().unwrap().to_string(),
                fixes: Vec::new(),
            })
            .collect(),
        usages: v["usages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| Usage {
                key: u[0].as_str().unwrap().to_string(),
                label: u[1].as_str().unwrap().to_string(),
                ok: u[2].as_bool().unwrap(),
                reason: u[3].as_str().unwrap().to_string(),
                shown: u[4].as_bool().unwrap(),
            })
            .collect(),
        windows: Vec::new(),
        exact: false,
        sliders: v["sliders"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                let k = s[0].as_str().unwrap();
                let key = ["Damping", "Master", "Stick Resp."]
                    .into_iter()
                    .find(|x| *x == k)
                    .unwrap();
                (key, f1(&s[1]))
            })
            .collect(),
        role: if v["role"].as_str().unwrap() == "filters" {
            otb_advisor::log_check::FILTERS
        } else {
            otb_advisor::log_check::PID
        },
        sp_delay_ms: Vec::new(),
    }
}

fn targets_of(v: &Value) -> Option<FilterTargets> {
    (!v.is_null()).then(|| {
        let a = hexes(&v["vals"]);
        let goal = (!v["goal"].is_null()).then(|| {
            let g = hexes(&v["goal"]);
            (g[0], g[1])
        });
        FilterTargets {
            gyro_ptb_db: a[0],
            dterm_ptb_db: a[1],
            gyro_delay_ms: a[2],
            dterm_delay_ms: a[3],
            delay_goal_ms: goal,
        }
    })
}

#[test]
fn auto_status_rend_l_etat_de_python() {
    let c = data();
    let pool: Vec<LogCheck> = c["pool"].as_array().unwrap().iter().map(check_of).collect();
    let targets: Vec<Option<FilterTargets>> = c["targets"].as_array().unwrap().iter().map(targets_of).collect();
    let phases = tuning_path::phases(&DroneCard::new("Test"));
    let steps: Vec<&Step> = phases.iter().flat_map(|p| p.steps.iter()).collect();
    assert_eq!(
        steps.iter().map(|s| s.key.clone()).collect::<Vec<_>>(),
        strs(&c["steps"])
    );
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for e in c["status"].as_array().unwrap() {
        let checks: Vec<LogCheck> = e["checks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| pool[i.as_u64().unwrap() as usize].clone())
            .collect();
        let t = targets[e["targets"].as_u64().unwrap() as usize].as_ref();
        let out = e["out"].as_array().unwrap();
        assert_eq!(out.len(), steps.len());
        for (s, w) in steps.iter().zip(out) {
            let (st, txt) = tuning_path::auto_status(s, &checks, t).unwrap();
            let what = format!("liste {:?}, repères {}, case {}", e["checks"], e["targets"], s.key);
            assert_eq!(st, w[0].as_str(), "{what} : état");
            assert_eq!(txt, w[1].as_str().unwrap(), "{what} : texte");
            *seen.entry(format!("{st:?}")).or_insert(0) += 1;
        }
    }
    for k in ["None", "Some(\"ok\")", "Some(\"bad\")"] {
        assert!(seen.contains_key(k), "état jamais produit : {k}");
    }
    assert_eq!(c["auto"], json!([tuning_path::AUTO_OK, tuning_path::AUTO_BAD]));
}

#[test]
fn auto_status_echoue_si_l_usage_manque() {
    // Python : `u = hits[-1].usage(...)` vaut None, `u.ok` lève AttributeError
    let c = data();
    let mut ck = check_of(
        &c["pool"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| !p["sliders"].as_array().unwrap().is_empty())
            .unwrap()
            .clone(),
    );
    ck.usages.clear();
    let phases = tuning_path::phases(&DroneCard::new("Test"));
    let step = phases
        .iter()
        .flat_map(|p| p.steps.iter())
        .find(|s| s.flight.is_some_and(|(cat, _)| cat == ck.sliders[0].0))
        .unwrap();
    let mut ck2 = ck.clone();
    ck2.sliders = vec![(step.flight.unwrap().0, step.flight.unwrap().1)];
    ck2.role = otb_advisor::log_check::PID;
    assert!(tuning_path::auto_status(step, &[ck2], None).is_err());
}
