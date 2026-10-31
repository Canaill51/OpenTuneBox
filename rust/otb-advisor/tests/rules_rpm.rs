//! Règles de passe-bas, de filtre RPM et de raies du vol stationnaire de `advisor/rules.py` en Rust (phase 5a-3b) sur des journaux fabriqués :
//! cas écrits par `scripts/rules_rpm_cases.py` (régénérer `tests/data/rules_rpm.json` avec ce script si `rules.py` change).
//!
//! **Appels directs** : ces règles ne sont appelées que par `filter_rules` (porté en 5a-3c, comparé sur le corpus par `otb-parity
//! check-advisor`) ; les appels directs atteignent les branches que le corpus n'atteint pas. Exigé : textes identiques au caractère près, mêmes
//! constats dans le même ordre, mêmes erreurs ; nombres issus de FFT (niveaux d'harmoniques, raies, forme d'une raie) à une tolérance
//! mesurée ; nombres de pure arithmétique (`dyn_notch_q_need`) identiques au bit près.

use otb_advisor::rules::{self, Finding, Measures};
use otb_analysis::motors::SpectrumLine;
use otb_log::flightlog::FlightLog;
use otb_log::settings::parse_settings;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;

/// Tolérance absolue (dB, Hz) des nombres issus de spectres `realfft` / pocketfft (L-048). **Mesurée** : voir la ligne « écart max » affichée
/// avec `--nocapture`.
const FFT_ATOL: f64 = 1e-9;

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

/// Compare deux nombres (NaN égaux) à la tolérance `atol` ; renvoie l'écart.
fn close(what: &str, got: f64, want: f64, atol: f64) -> f64 {
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
    let d = (got - want).abs();
    assert!(d <= atol, "{what} : Rust {got:e} / Python {want:e} (écart {d:e})");
    d
}

fn cut_of(v: &Value) -> (i64, bool) {
    let a = v.as_array().unwrap();
    (a[0].as_i64().unwrap(), a[1].as_bool().unwrap())
}

#[test]
fn appels_directs_identiques_a_python() {
    let c = cas("rules_rpm");
    let mut worst = 0.0f64;
    let mut by_call: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_key: BTreeMap<String, usize> = BTreeMap::new();
    let mut n_err = 0;
    for case in c["cases"].as_array().unwrap() {
        let label = case["label"].as_str().unwrap();
        let lg = log_of(&c, &case["log"]);
        let s = &lg.settings;
        let n = lg.n();
        let sel = case["sel"].as_array().map_or(0..n, |x| {
            x[0].as_u64().unwrap() as usize..x[1].as_u64().unwrap() as usize
        });
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
            "lowpass" => {
                let got = rules::lowpass_setup_rules(s);
                check_findings(label, &got, &want["findings"]);
                got.iter().for_each(|f| *by_key.entry(f.key.clone()).or_insert(0) += 1);
            }
            "levels" => {
                let (raw, left) = ok!(rules::rpm_harmonic_levels(&lg, &sel));
                for (name, g) in [("raw", &raw), ("left", &left)] {
                    let w = hexes(&want[name]);
                    assert_eq!(g.len(), w.len(), "{label} : longueur de {name}");
                    for (x, y) in g.iter().zip(&w) {
                        worst = worst.max(close(&format!("{label} : {name}"), *x, *y, FFT_ATOL));
                    }
                }
            }
            "weights" => {
                let got = ok!(rules::rpm_weight_rules(
                    &lg,
                    &sel,
                    a["lighten"].as_bool().unwrap(),
                    a["h1"].as_bool().unwrap()
                ));
                check_findings(label, &got, &want["findings"]);
                got.iter().for_each(|f| *by_key.entry(f.key.clone()).or_insert(0) += 1);
            }
            "qraise" => {
                let present: Vec<Finding> = strings(&a["present"])
                    .iter()
                    .map(|k| Finding::new(k, "Filtres", "conseil", "t", "d", vec!["a".to_string()], None))
                    .collect();
                let got = ok!(rules::rpm_q_raise(&lg, &sel, &present, a["hover"].as_bool().unwrap()));
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
            "hover" => {
                let mut meas = Measures::default();
                let got = ok!(rules::hover_noise(
                    &lg,
                    &sel,
                    f1(&a["g_ptb"]),
                    f1(&a["d_ptb"]),
                    hex_opt(&a["fixed_hz"]),
                    &mut meas,
                    a["aliased"].as_bool().unwrap()
                ));
                check_findings(label, &got, &want["findings"]);
                got.iter().for_each(|f| *by_key.entry(f.key.clone()).or_insert(0) += 1);
                let lines = meas.hover_lines.expect("hover_lines écrit");
                let wl = want["lines"].as_array().unwrap();
                assert_eq!(lines.len(), wl.len(), "{label} : nombre de raies");
                for (l, w) in lines.iter().zip(wl) {
                    let w = hexes(w);
                    worst = worst.max(close(&format!("{label} : fréquence"), l.freq_hz, w[0], FFT_ATOL));
                    worst = worst.max(close(&format!("{label} : raw_db"), l.raw_db, w[1], FFT_ATOL));
                    worst = worst.max(close(&format!("{label} : left_db"), l.left_db, w[2], FFT_ATOL));
                }
            }
            "cutoff" => {
                let got = ok!(rules::static_notch_cutoff(f1(&a["f0"]), f1(&a["w"]), f1(&a["h"])));
                assert_eq!(got, cut_of(&want["cutoff"]), "{label}");
            }
            "qneed" => {
                let got = rules::dyn_notch_q_need(&hexes(&a["peaks"]), &hexes(&a["widths"]), &hexes(&a["heights"]));
                match (got, hex_opt(&want["num"])) {
                    (None, None) => {}
                    (Some(g), Some(w)) => assert!(same(g, w), "{label} : Rust {g:e} / Python {w:e}"),
                    (g, w) => panic!("{label} : Rust {g:?} / Python {w:?}"),
                }
            }
            "qround" => {
                let got = ok!(rules::q_round(f1(&a["q"]), a["low"].as_i64().unwrap()));
                assert_eq!(got, want["int"].as_i64().unwrap(), "{label}");
            }
            "ncase" => {
                let got = rules::static_notch_case(s, f1(&a["f0"]));
                assert_eq!(got.as_deref(), want["opt"].as_str(), "{label}");
            }
            "nactive" => assert_eq!(rules::dyn_notch_active(s), want["bool"].as_bool().unwrap(), "{label}"),
            "nactions" => {
                let (acts, sets) = ok!(rules::dyn_notch_line_actions(
                    s,
                    f1(&a["f0"]),
                    a["fixed"].as_bool().unwrap()
                ));
                assert_eq!(acts, strings(&want["strs"]), "{label} : actions");
                assert_eq!(sets, strings(&want["settings"]), "{label} : réglages");
            }
            "naction" => {
                let got =
                    rules::static_notch_action(s, f1(&a["f0"]), cut_of(&a["cutoff"]), a["lead"].as_str().unwrap());
                assert_eq!(got, strings(&want["strs"]), "{label}");
            }
            "ntext" => assert_eq!(
                rules::static_notch_text(f1(&a["f0"]), cut_of(&a["cutoff"])),
                want["str"].as_str().unwrap(),
                "{label}"
            ),
            "caveat" => assert_eq!(
                rules::low_line_caveat(f1(&a["f0"])),
                want["str"].as_str().unwrap(),
                "{label}"
            ),
            "residual" => {
                let l = hexes(&a["line"]);
                let ln = SpectrumLine {
                    freq_hz: l[0],
                    raw_db: l[1],
                    left_db: l[2],
                };
                let cut = (!a["cutoff"].is_null()).then(|| cut_of(&a["cutoff"]));
                let got = ok!(rules::residual_line(s, &ln, cut));
                check_finding(label, &got, &want["finding"]);
                *by_key.entry(got.key.clone()).or_insert(0) += 1;
            }
            "shape" => {
                let (w, h) = ok!(rules::line_shape(&lg, &sel, f1(&a["f0"])));
                let wv = hexes(&want["shape"]);
                worst = worst.max(close(&format!("{label} : largeur"), w, wv[0], FFT_ATOL));
                worst = worst.max(close(&format!("{label} : hauteur"), h, wv[1], FFT_ATOL));
            }
            other => panic!("appel {other} inconnu"),
        }
    }
    eprintln!("appels par fonction : {by_call:?}");
    eprintln!("constats comparés : {by_key:?}");
    eprintln!("erreurs de Python reproduites : {n_err} ; écart max {worst:.3e}");
    for k in [
        "gyro_lpf2_off",
        "dterm_filter_weak",
        "dterm_lpf1_static",
        "gyro_lpf_biquad",
        "rpm_weight_low",
        "rpm_harmonic_missing",
        "rpm_line_visible",
        "rpm_weight_lighten",
        "rpm_q_raise",
        "hover_noise_summary",
        "hover_line_residual",
    ] {
        assert!(by_key.contains_key(k), "le constat {k} n'est produit par aucun cas");
    }
}
