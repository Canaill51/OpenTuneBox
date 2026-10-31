//! Comparaison d'un rapport complet de `rules::analyze` avec celui de Python (`encode_out` de `scripts/rules_state_cases.py`) : partagée
//! par `rules_state.rs` et `rules_delay.rs`. **Tous** les constats sont comparés (clés, catégories, gravités, textes, actions, axes,
//! réglages déclarés, ordre, ordre trié), puis toutes les mesures du rapport (depuis 5a-3c, `analyze` est complet).
#![allow(dead_code)]

use super::common::*;
use otb_advisor::rules::{self, Finding, Report};
use otb_log::flightlog::FlightLog;
use otb_log::settings::parse_settings;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Tolérance des retards (FFT, `atan2`, produits complexes : L-048), relative à une période d'échantillonnage : même valeur que `delay.rs`
/// (phase). **Mesurée sur ces cas** : voir la ligne « écart max retards » affichée avec `--nocapture`.
pub const DELAY_RTOL: f64 = 1e-10;
/// Tolérance des niveaux de bruit en dB (spectres de `realfft` contre pocketfft : L-048), relative au plus grand élément de la liste.
pub const NOISE_RTOL: f64 = 1e-11;
/// Tolérance des mesures de `filter_rules` et du vol stationnaire (niveaux et largeurs de raies : L-048), relative au plus grand élément.
pub const FILTER_RTOL: f64 = 1e-11;

pub fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

pub fn check_finding(what: &str, got: &Finding, want: &Value) {
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

/// Le journal fabriqué : courbes et temps du fichier, réglages relus depuis l'en-tête brut (comme `parse_settings` de Python).
pub fn log_of(c: &Value, v: &Value) -> FlightLog {
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

pub fn close1(cat: &'static str, got: f64, want: f64, rtol: f64, what: &str) {
    assert_close(cat, &[got], &[want], rtol, what);
}

/// Compare le rapport de Rust à `want` (la sortie `out` d'un cas). `seen` compte les constats comparés, par clé.
pub fn check_report(label: &str, lg: &FlightLog, rep: &Report, want: &Value, seen: &mut BTreeMap<String, usize>) {
    assert_eq!(rep.profile.key, want["profile"].as_str().unwrap(), "{label} : profil");
    assert_eq!(rep.profile_reason, want["reason"].as_str().unwrap(), "{label} : raison");
    assert_bits(
        &[rep.period.0, rep.period.1],
        &hexes(&want["period"]),
        &format!("{label} : période"),
    );

    // constats : tous, dans l'ordre de Python (`filter_strategy` en tête s'il existe)
    let want_f = want["findings"].as_array().unwrap();
    assert_eq!(
        rep.findings.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        want_f.iter().map(|f| f["key"].as_str().unwrap()).collect::<Vec<_>>(),
        "{label} : liste des constats"
    );
    for (g, w) in rep.findings.iter().zip(want_f) {
        check_finding(label, g, w);
        *seen.entry(g.key.clone()).or_insert(0) += 1;
    }
    // tri stable par gravité : `want["sorted"]` = rang de chaque constat dans `findings`
    let sorted = rules::sorted(&rep.findings);
    let order: Vec<usize> = want["sorted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i.as_u64().unwrap() as usize)
        .collect();
    assert_eq!(sorted.len(), order.len(), "{label} : nombre de constats triés");
    for (g, &i) in sorted.iter().zip(&order) {
        check_finding(&format!("{label} (trié)"), g, &want_f[i]);
    }
    assert_eq!(
        rep.steps.iter().map(|s| s.axis.clone()).collect::<Vec<_>>(),
        strings(&want["steps"]),
        "{label} : axes de la réponse indicielle"
    );
    check_measures(label, lg, rep, &want["measures"]);
}

fn opt_bits(got: Option<f64>, want: &Value, what: &str) {
    let w = want.as_str().map(|_| f1(want));
    assert_eq!(got.map(f64::to_bits), w.map(f64::to_bits), "{what}");
}

fn check_measures(label: &str, lg: &FlightLog, rep: &Report, wm: &Value) {
    let m = &rep.measures;
    opt_bits(
        m.hover_motor_hz,
        &wm["hover_motor_hz"],
        &format!("{label} : hover_motor_hz"),
    );
    let (b, h) = m.log_rate_needed_hz.unwrap();
    assert_bits(
        &[b, h],
        &hexes(&wm["log_rate_needed_hz"]),
        &format!("{label} : log_rate_needed_hz"),
    );
    assert_eq!(m.aliased, wm["aliased"].as_bool(), "{label} : aliased");
    let steps = m.steps.as_ref().unwrap();
    let ws = wm["steps"].as_object().unwrap();
    assert_eq!(steps.len(), ws.len(), "{label} : axes des mesures");
    for (axis, sm) in steps {
        let got = [
            sm.peak,
            sm.peak_ms,
            sm.latency_ms,
            sm.rise_ms,
            sm.settling_ms,
            sm.overshoot_pct,
            sm.ripple,
            sm.ripple_std,
            sm.ripple_ms,
            sm.waves,
            sm.waves_ratio,
        ];
        // ⚠️ issues d'une FFT (`step_response`) : identiques au bit près sur le poste des références seulement, 4 ulp d'écart sous Windows (L-059)
        assert_close_rel(
            "steps",
            &got,
            &hexes(&ws[axis]),
            1e-9,
            &format!("{label} : steps {axis}"),
        );
    }
    let d = m.delays.as_ref().unwrap();
    let wd = &wm["delays"];
    let u = 1000.0 / lg.fs; // une période d'échantillonnage, en ms
    for (name, g, w) in [
        ("gyro", d.gyro_ms, f1(&wd["gyro"])),
        ("dterm", d.dterm_ms, f1(&wd["dterm"])),
    ] {
        close1("retards", g * u, w * u, DELAY_RTOL, &format!("{label} : delays.{name}"));
    }
    let wsp = hexes(&wd["sp"]);
    for (a, w) in wsp.iter().enumerate() {
        close1(
            "retards",
            d.sp_gyro_ms[a] * u,
            w * u,
            DELAY_RTOL,
            &format!("{label} : delays.sp[{a}]"),
        );
    }
    assert!(same(d.stick_p95_dps, f1(&wd["stick"])), "{label} : stick_p95_dps");
    for (name, g, w) in [
        (
            "gyro_noise_db",
            m.gyro_noise_db.as_ref().unwrap().clone(),
            hexes(&wm["gyro_noise_db"]),
        ),
        (
            "dterm_noise_db",
            m.dterm_noise_db.as_ref().unwrap().clone(),
            hexes(&wm["dterm_noise_db"]),
        ),
        (
            "low_band_db",
            m.low_band_db.as_ref().unwrap().clone(),
            hexes(&wm["low_band_db"]),
        ),
        (
            "peaks",
            vec![m.gyro_peak_ptb_db.unwrap(), m.dterm_peak_ptb_db.unwrap()],
            vec![f1(&wm["gyro_peak_ptb_db"]), f1(&wm["dterm_peak_ptb_db"])],
        ),
    ] {
        assert_close("bruit (dB)", &g, &w, NOISE_RTOL, &format!("{label} : {name}"));
    }
    let st = m.stats.as_ref().unwrap();
    let ws = &wm["stats"];
    assert!(same(st.duration_s, f1(&ws["duration_s"])), "{label} : stats.duration_s");
    assert!(
        same(st.motor_saturation_pct, f1(&ws["sat"])),
        "{label} : stats.saturation"
    );
    assert!(
        same(st.motor_imbalance_pct, f1(&ws["imb"])),
        "{label} : stats.imbalance"
    );
    let wmot = ws["motors"].as_array().unwrap();
    assert_eq!(st.motors.len(), wmot.len(), "{label} : nombre de moteurs");
    for (k, (g, w)) in st.motors.iter().zip(wmot).enumerate() {
        assert_bits(&[g.mean, g.sd, g.max], &hexes(w), &format!("{label} : moteur {k}"));
    }
    assert_bits(
        m.pid_error_sd.as_ref().unwrap(),
        &hexes(&wm["pid_error_sd"]),
        &format!("{label} : pid_error_sd"),
    );
    assert_bits(
        m.iterm_mean.as_ref().unwrap(),
        &hexes(&wm["iterm_mean"]),
        &format!("{label} : iterm_mean"),
    );
    let sv = m.sliders.as_ref().unwrap();
    let wsv = &wm["sliders"];
    assert_eq!(
        sv.from_header,
        wsv["from_header"].as_bool().unwrap(),
        "{label} : from_header"
    );
    assert_eq!(
        sv.consistent,
        wsv["consistent"].as_bool().unwrap(),
        "{label} : consistent"
    );
    let wvals = wsv["values"].as_array().unwrap();
    assert_eq!(sv.values.len(), wvals.len(), "{label} : curseurs");
    for ((k, v), w) in sv.values.iter().zip(wvals) {
        assert_eq!(*k, w[0].as_str().unwrap(), "{label} : nom de curseur");
        close1("curseurs", *v, f1(&w[1]), 1e-11, &format!("{label} : curseur {k}"));
    }
    check_filter_measures(label, m, wm);
}

/// Mesures de `filter_rules` (5a-3c) et du vol stationnaire (5a-3b) : `None` de Rust ⇔ clé absente (ou `None`) de Python.
fn check_filter_measures(label: &str, m: &rules::Measures, wm: &Value) {
    for (name, g) in [
        ("motor_harmonic_left_db", m.motor_harmonic_left_db),
        ("motor_harmonic_raw_db", m.motor_harmonic_raw_db),
    ] {
        match (g, wm[name].as_str()) {
            (None, None) => {}
            (Some(g), Some(_)) => close1(
                "filtres (dB)",
                g,
                f1(&wm[name]),
                FILTER_RTOL,
                &format!("{label} : {name}"),
            ),
            (g, w) => panic!("{label} : {name} : Rust {g:?}, Python {w:?}"),
        }
    }
    match (&m.hover_lines, wm["hover_lines"].as_array()) {
        (None, None) => {}
        (Some(g), Some(w)) => {
            assert_eq!(g.len(), w.len(), "{label} : nombre de raies du vol stationnaire");
            for (i, (ln, wl)) in g.iter().zip(w).enumerate() {
                let wl = hexes(wl);
                assert!(same(ln.freq_hz, wl[0]), "{label} : hover_lines[{i}].freq_hz");
                assert_close(
                    "filtres (dB)",
                    &[ln.raw_db, ln.left_db],
                    &wl[1..],
                    FILTER_RTOL,
                    &format!("{label} : hover_lines[{i}]"),
                );
            }
        }
        (g, w) => panic!("{label} : hover_lines : Rust {}, Python {}", g.is_some(), w.is_some()),
    }
    match (&m.frame_resonance, wm["frame_resonance"].is_null()) {
        (None, true) => {}
        (Some(g), false) => {
            let w = &wm["frame_resonance"];
            assert!(same(g.freq_hz, f1(&w["freq"])), "{label} : frame_resonance.freq");
            close1(
                "filtres (dB)",
                g.excess_db,
                f1(&w["excess"]),
                FILTER_RTOL,
                &format!("{label} : frame_resonance.excess"),
            );
            assert_eq!(
                g.near_motor,
                w["near"].as_bool().unwrap(),
                "{label} : frame_resonance.near"
            );
        }
        (g, w) => panic!("{label} : frame_resonance : Rust {}, Python None={w}", g.is_some()),
    }
    match (&m.dyn_notch, wm["dyn_notch"].is_null()) {
        (None, true) => {}
        (Some(g), false) => {
            let w = &wm["dyn_notch"];
            assert_eq!(g.count, w["count"].as_i64().unwrap(), "{label} : dyn_notch.count");
            assert_eq!(g.min_hz, w["min_hz"].as_i64().unwrap(), "{label} : dyn_notch.min_hz");
            assert_eq!(g.max_hz, w["max_hz"].as_i64().unwrap(), "{label} : dyn_notch.max_hz");
            assert_bits(&g.peaks, &hexes(&w["peaks"]), &format!("{label} : dyn_notch.peaks"));
            for (name, got) in [("widths", &g.widths), ("heights", &g.heights), ("left", &g.left)] {
                assert_close(
                    "filtres (dB)",
                    got,
                    &hexes(&w[name]),
                    FILTER_RTOL,
                    &format!("{label} : dyn_notch.{name}"),
                );
            }
            match (g.q_need, w["q_need"].as_str()) {
                (None, None) => {}
                (Some(q), Some(_)) => close1(
                    "filtres (dB)",
                    q,
                    f1(&w["q_need"]),
                    FILTER_RTOL,
                    &format!("{label} : dyn_notch.q_need"),
                ),
                (g, w) => panic!("{label} : dyn_notch.q_need : Rust {g:?}, Python {w:?}"),
            }
        }
        (g, w) => panic!("{label} : dyn_notch : Rust {}, Python None={w}", g.is_some()),
    }
}
