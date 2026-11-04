//! Éléments sans état de `advisor/rules.py` portés en Rust (phase 5a-2a) : cas écrits par `scripts/rules_cases.py` (régénérer
//! `tests/data/rules.json` avec ce script si `rules.py` change). Tous les textes (titres, détails, actions) sont exigés
//! **identiques au caractère près**, ainsi que les décisions (quelle règle se déclenche, dans quel ordre).

use otb_advisor::profiles::profile;
use otb_advisor::rules::{self, AxisStep, Finding};
use otb_analysis::open_loop::{APPROX, LoopMetrics, MEASURED};
use otb_analysis::step::{StepMetrics, StepResult};
use serde_json::Value;

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn lp_of(v: &Value) -> LoopMetrics {
    let mut lp = LoopMetrics::new(&[], &[], &[]);
    lp.method = if v["measured"].as_bool().unwrap() {
        MEASURED
    } else {
        APPROX
    };
    lp.notes = strings(&v["notes"]);
    lp.sensitivity_peak_db = f1(&v["sens"]);
    lp.sensitivity_peak_hz = f1(&v["hz"]);
    lp.sensitivity_peak_at_edge = v["edge"].as_bool().unwrap();
    lp.phase_margin_deg = f1(&v["pm"]);
    lp.crossover_hz = f1(&v["xo"]);
    lp.d_ratio = f1(&v["dr"]);
    lp
}

fn step_of(c: &Value) -> AxisStep {
    let metrics = StepMetrics {
        peak: f1(&c["pk"]),
        latency_ms: f1(&c["lat"]),
        ..StepMetrics::default()
    };
    AxisStep {
        axis: c["axis"].as_str().unwrap().to_string(),
        result: StepResult {
            t_ms: vec![],
            responses: vec![],
            mean: vec![],
            std: vec![],
            n: c["n"].as_u64().unwrap() as usize,
            metrics,
            loop_: lp_of(&c["lp"]),
            max_inputs: vec![],
        },
    }
}

/// Compare un constat à celui de Python champ par champ (le message donne le cas, l'attendu et l'obtenu).
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
    let want = want.as_array().unwrap();
    assert_eq!(
        got.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        want.iter().map(|f| f["key"].as_str().unwrap()).collect::<Vec<_>>(),
        "{what} : liste des constats"
    );
    for (g, w) in got.iter().zip(want) {
        check_finding(what, g, w);
    }
}

#[test]
fn les_constantes_sont_celles_de_python() {
    let c = &cas("rules")["constants"];
    let pair = |k: &str| {
        let a = hexes(&c[k]);
        (a[0], a[1])
    };
    assert_eq!(rules::STEP_PEAK_HIGH, pair("step_peak_high"));
    assert_eq!(rules::STEP_PEAK_HIGH_FF, pair("step_peak_high_ff"));
    for (name, got) in [
        ("step_peak_flat", rules::STEP_PEAK_FLAT),
        ("step_peak_soft", rules::STEP_PEAK_SOFT),
        ("sensitivity_high_db", rules::SENSITIVITY_HIGH_DB),
        ("phase_margin_low_deg", rules::PHASE_MARGIN_LOW_DEG),
        ("d_dominant_ratio", rules::D_DOMINANT_RATIO),
        ("log_rate_min_hz", rules::LOG_RATE_MIN_HZ),
        ("pid_log_rate_min_hz", rules::PID_LOG_RATE_MIN_HZ),
        ("pid_log_rate_floor_hz", rules::PID_LOG_RATE_FLOOR_HZ),
    ] {
        assert!(same(got, f1(&c[name])), "{name} : Rust {got} / Python {}", f1(&c[name]));
    }
    assert_eq!(rules::STEP_MIN_WINDOWS as u64, c["step_min_windows"].as_u64().unwrap());
    assert_eq!(
        rules::STEP_SURE_WINDOWS as u64,
        c["step_sure_windows"].as_u64().unwrap()
    );
    assert_eq!(rules::FF_SERIES_ADVICE, c["ff_series_advice"].as_str().unwrap());
    assert_eq!(
        [rules::INFO, rules::OK, rules::CONSEIL, rules::ATTENTION, rules::ALERTE].to_vec(),
        strings(&c["severities"])
    );
    let order = c["severity_order"].as_object().unwrap();
    assert_eq!(order.len(), rules::SEVERITY_ORDER.len());
    for (k, v) in order {
        assert_eq!(rules::severity_order(k), v.as_i64().unwrap(), "ordre de {k}");
    }
    assert_eq!(rules::severity_order("inconnue"), 9);
    assert_eq!(rules::TRACKED_SETTINGS.to_vec(), strings(&c["tracked_settings"]));
    assert_eq!(rules::LOOP_MARGIN_KEYS.to_vec(), strings(&c["loop_margin_keys"]));
    let fr: Vec<i64> = c["log_rate_fractions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(rules::LOG_RATE_FRACTIONS.to_vec(), fr);
}

#[test]
fn gaps_text_est_celui_de_python() {
    for c in cas("rules")["gaps"].as_array().unwrap() {
        let got = rules::gaps_text(c["n"].as_i64().unwrap(), f1(&c["lost"]), f1(&c["span"]));
        assert_eq!(got, c["out"].as_str().unwrap(), "gaps_text {c}");
    }
}

#[test]
fn fr_arrondit_et_met_la_virgule_comme_python() {
    for c in cas("rules")["fr"].as_array().unwrap() {
        assert_eq!(rules::fr(f1(&c["x"])), c["out"].as_str().unwrap(), "_fr {c}");
    }
}

#[test]
fn les_frequences_d_enregistrement_sont_celles_de_python() {
    for c in cas("rules")["rates"].as_array().unwrap() {
        let pid = f1(&c["pid"]);
        let motor = c["motor"].as_str().map(|_| f1(&c["motor"]));
        let need = rules::log_rate_needed(pid, motor);
        assert!(same(need, f1(&c["need"])), "log_rate_needed {c} : Rust {need}");
        let pn = rules::pid_log_rate_needed(pid);
        assert!(same(pn, f1(&c["pidneed"])), "pid_log_rate_needed {c} : Rust {pn}");
    }
}

#[test]
fn sample_rate_cli_choisit_la_meme_fraction() {
    for c in cas("rules")["cli"].as_array().unwrap() {
        let got = rules::sample_rate_cli(f1(&c["pid"]), f1(&c["rate"]));
        assert_eq!(got, c["out"].as_str().unwrap(), "sample_rate_cli {c}");
    }
}

#[test]
fn les_textes_de_la_boucle_ouverte_sont_ceux_de_python() {
    for c in cas("rules")["loop_texts"].as_array().unwrap() {
        if let Some(key) = c["profile"].as_str() {
            let p = profile(key).unwrap();
            assert_eq!(
                rules::whoop_margin_note(p),
                c["whoop"].as_str().unwrap(),
                "_whoop_margin_note {key}"
            );
            continue;
        }
        let lp = lp_of(&c["lp"]);
        assert_eq!(
            rules::loop_caveat(&lp),
            c["caveat"].as_str().unwrap(),
            "_loop_caveat {c}"
        );
        assert_eq!(rules::edge_actions(&lp), strings(&c["edge"]), "_edge_actions {c}");
        assert_eq!(rules::margin_actions(&lp), strings(&c["margin"]), "_margin_actions {c}");
    }
}

#[test]
fn le_constat_de_desequilibre_est_celui_de_python() {
    for c in cas("rules")["imbalance"].as_array().unwrap() {
        let m = hexes(&c["m"]);
        let got = rules::imbalance_finding(&m, f1(&c["imb"]));
        if is_error(c) {
            assert!(got.is_err(), "_imbalance_finding {c} : Python lève, Rust rend {got:?}");
        } else {
            let f = got.unwrap_or_else(|e| panic!("_imbalance_finding {c} : Rust lève ({e})"));
            check_finding(&format!("_imbalance_finding {c}"), &f, &c["out"]);
        }
    }
}

#[test]
fn le_tri_des_constats_est_stable_comme_celui_de_python() {
    for c in cas("rules")["sorted"].as_array().unwrap() {
        let fs: Vec<Finding> = c["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| {
                Finding::new(
                    i[0].as_str().unwrap(),
                    "Cat",
                    i[1].as_str().unwrap(),
                    "t",
                    "d",
                    vec![],
                    None,
                )
            })
            .collect();
        let got: Vec<String> = rules::sorted(&fs).into_iter().map(|f| f.key).collect();
        assert_eq!(got, strings(&c["out"]), "sorted {c}");
    }
}

fn run_step(
    c: &Value,
    f: fn(&mut Vec<Finding>, &AxisStep, &otb_advisor::profiles::CraftProfile, i64, bool),
    name: &str,
) {
    let st = step_of(c);
    let p = profile(c["profile"].as_str().unwrap()).unwrap();
    let mut got = Vec::new();
    f(&mut got, &st, p, c["ff"].as_i64().unwrap(), c["cut"].as_bool().unwrap());
    check_findings(&format!("{name} {c}"), &got, &c["out"]);
}

#[test]
fn les_regles_de_la_reponse_indicielle_sont_celles_de_python() {
    for c in cas("rules")["step"].as_array().unwrap() {
        run_step(c, rules::step_rules, "_step_rules");
    }
}

#[test]
fn les_constats_de_la_reponse_indicielle_sont_ceux_de_python() {
    for c in cas("rules")["step_findings"].as_array().unwrap() {
        run_step(c, rules::step_findings, "_step_findings");
    }
}
