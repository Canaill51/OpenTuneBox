//! `rules::analyze` de `advisor/rules.py` en Rust (phase 5a-2b, complété en 5a-3c) sur des journaux fabriqués : cas écrits par
//! `scripts/rules_state_cases.py` (régénérer `tests/data/rules_state.json` avec ce script si `rules.py` change). Les branches que le corpus
//! n'atteint pas ou mal : enregistrement (fréquences, champs absents, trous, crash), réponse indicielle (peu de fenêtres, écart roll / pitch),
//! niveaux de bruit, moteurs, curseurs, profil imposé ou détecté, et un vol « combiné » (bruit moteur, oscillations 20–80 Hz, notch dynamique,
//! stratégie de filtrage : l'insertion de `filter_strategy` en tête).
//!
//! Exigé : **tous** les constats du rapport (`support::check_report`) : textes identiques au caractère près, mêmes constats dans le même
//! ordre, mêmes gravités après le tri stable, toutes les mesures (au bit près, ou avec une tolérance mesurée pour celles qui passent par des
//! FFT).

use otb_advisor::profiles::{CraftProfile, profile};
use otb_advisor::rules;
use serde_json::Value;
use std::collections::BTreeMap;

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;
#[path = "support/mod.rs"]
mod support;
use support::*;

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
        check_report(label, &lg, &rep, want, &mut seen);
    }
    report();
    // chaque constat attendu est produit au moins une fois, et les erreurs de Python sont bien rejouées
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
    // le cas combiné produit ensemble les constats de filter_rules et la stratégie, en tête
    let comb = c["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["label"].as_str().unwrap().starts_with("combiné : "))
        .expect("cas combiné absent");
    let ck: Vec<&str> = comb["out"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["key"].as_str().unwrap())
        .collect();
    assert_eq!(ck[0], "filter_strategy", "la stratégie est insérée en tête");
    for k in ["motor_noise_left", "low_band_oscillation", "dyn_notch_fewer"] {
        assert!(ck.contains(&k), "le cas combiné ne produit pas {k}");
    }
    eprintln!("constats comparés : {seen:?}");
}
