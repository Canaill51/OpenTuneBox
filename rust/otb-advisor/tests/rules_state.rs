//! `rules::analyze` de `advisor/rules.py` en Rust (phase 5a-2b) sur des journaux fabriqués : cas écrits par `scripts/rules_state_cases.py`
//! (régénérer `tests/data/rules_state.json` avec ce script si `rules.py` change). Les branches que le corpus n'atteint pas ou mal :
//! enregistrement (fréquences, champs absents, trous, crash), réponse indicielle (peu de fenêtres, écart roll / pitch), niveaux de bruit,
//! moteurs, curseurs, profil imposé ou détecté.
//!
//! Exigé : textes identiques au caractère près, mêmes constats dans le même ordre, mêmes gravités après le tri stable, mesures identiques au
//! bit près (écart mesuré et affiché pour les retards et les niveaux de bruit, qui passent par des FFT).
//! **Seuls les constats de `KEYS`** (parties portées en 5a-2b) sont comparés : `delay_rules`, `filter_rules` et `filter_strategy` (5a-3)
//! n'existent pas encore côté Rust.

use otb_advisor::profiles::{CraftProfile, profile};
use otb_advisor::rules::{self, Finding};
use otb_log::flightlog::FlightLog;
use otb_log::settings::parse_settings;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;

/// Tolérance des retards (FFT, `atan2`, produits complexes : L-048), relative à une période d'échantillonnage : même valeur que `delay.rs`
/// (phase). **Mesurée sur ces cas** : 4,7e-12 au pire (`report()`), marge ×20.
const DELAY_RTOL: f64 = 1e-10;
/// Tolérance des niveaux de bruit en dB (spectres de `realfft` contre pocketfft : L-048), relative au plus grand élément de la liste.
/// **Mesurée sur ces cas** : 2,4e-13 au pire, marge ×40 (sur le corpus : 2,2e-14 en absolu, voir `advisor_layer::NOISE_TOL`).
const NOISE_RTOL: f64 = 1e-11;

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

/// Le journal fabriqué : courbes et temps du fichier, réglages relus depuis l'en-tête brut (comme `parse_settings` de Python).
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

/// Profil imposé : celui de la base, avec les seuils remplacés par ceux du cas.
fn profile_of(p: &Value) -> Option<CraftProfile> {
    if p.is_null() {
        return None;
    }
    let mut out = profile(p["base"].as_str().unwrap()).unwrap().clone();
    for (k, v) in p["over"].as_object().unwrap() {
        let x = f1(v);
        match k.as_str() {
            "roll_pitch_gap_high" => out.roll_pitch_gap_high = x,
            "gyro_noise_high_db" => out.gyro_noise_high_db = x,
            "dterm_noise_high_db" => out.dterm_noise_high_db = x,
            "saturation_high_pct" => out.saturation_high_pct = x,
            "imbalance_high_pct" => out.imbalance_high_pct = x,
            "iterm_offset_high" => out.iterm_offset_high = x,
            other => panic!("seuil {other} non géré par le test"),
        }
    }
    Some(out)
}

fn close1(cat: &'static str, got: f64, want: f64, rtol: f64, what: &str) {
    assert_close(cat, &[got], &[want], rtol, what);
}

#[test]
fn analyze_rend_les_constats_et_les_mesures_de_python() {
    let c = cas("rules_state");
    let keys = strings(&c["keys"]);
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut errors = 0;
    for case in c["cases"].as_array().unwrap() {
        let label = case["label"].as_str().unwrap();
        let lg = log_of(&c, &case["log"]);
        let sel = case["sel"]
            .as_array()
            .map(|s| s[0].as_u64().unwrap() as usize..s[1].as_u64().unwrap() as usize);
        let prof = profile_of(&case["profile"]);
        let got = rules::analyze(&lg, sel, prof.as_ref());
        if is_error(case) {
            errors += 1;
            assert!(got.is_err(), "{label} : Python lève, Rust rend un rapport");
            continue;
        }
        let rep = got.unwrap_or_else(|e| panic!("{label} : Rust lève ({e}), Python rend un rapport"));
        let want = &case["out"];
        assert_eq!(rep.profile.key, want["profile"].as_str().unwrap(), "{label} : profil");
        assert_eq!(rep.profile_reason, want["reason"].as_str().unwrap(), "{label} : raison");
        assert_bits(
            &[rep.period.0, rep.period.1],
            &hexes(&want["period"]),
            &format!("{label} : période"),
        );

        // constats : ceux des parties portées, dans l'ordre de Python
        let kept: Vec<Finding> = rep.findings.iter().filter(|f| keys.contains(&f.key)).cloned().collect();
        let want_f = want["findings"].as_array().unwrap();
        assert_eq!(
            kept.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
            want_f.iter().map(|f| f["key"].as_str().unwrap()).collect::<Vec<_>>(),
            "{label} : liste des constats"
        );
        for (g, w) in kept.iter().zip(want_f) {
            check_finding(label, g, w);
            *seen.entry(g.key.clone()).or_insert(0) += 1;
        }
        // tri stable par gravité : `want["sorted"]` = rang de chaque constat dans `findings`
        let sorted = rules::sorted(&kept);
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
            strings(&want["steps"])
        );

        // mesures
        let m = &rep.measures;
        let wm = &want["measures"];
        let hz = wm["hover_motor_hz"].as_str().map(|_| f1(&wm["hover_motor_hz"]));
        assert_eq!(
            m.hover_motor_hz.map(f64::to_bits),
            hz.map(f64::to_bits),
            "{label} : hover_motor_hz"
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
            assert_bits(&got, &hexes(&ws[axis]), &format!("{label} : steps {axis}"));
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
    }
    report();
    // chaque constat des parties portées est produit au moins une fois, et les erreurs de Python sont bien rejouées
    for k in &keys {
        let optional = [
            "step_ff_overshoot",
            "step_overshoot_strong",
            "step_overshoot_light",
            "step_soft",
            "loop_phase_margin_low",
        ];
        assert!(
            seen.contains_key(k) || optional.contains(&k.as_str()),
            "le constat {k} n'est produit par aucun cas"
        );
    }
    assert!(errors >= 2, "les cas où Python lève sont absents");
    eprintln!("constats comparés : {seen:?}");
}
