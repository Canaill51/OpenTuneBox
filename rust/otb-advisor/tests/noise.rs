//! `noise::*` de `advisor/noise.py` en Rust (phase 5a-4) sur des journaux fabriqués : cas écrits par `scripts/noise_cases.py` (régénérer
//! `tests/data/noise.json` avec ce script si `noise.py` ou `rules.py` change).
//!
//! - appels directs : `section_of` pour chaque clé de chaque liste et chaque catégorie, `lowpass_text`, `level_note` ;
//! - `noise_report` sur des vols fabriqués (filtre RPM absent ou coupé, notch coupé / actif / hors plage, passe-bas, bruit, trous,
//!   période explicite, profil imposé) : **tout** le diagnostic est comparé (bande, niveaux de bruit, harmoniques, constats rangés par
//!   partie, blocs avec leurs observations, réglages et verdicts, stratégie), textes au caractère près ; les niveaux issus d'une FFT à une
//!   tolérance mesurée ; les erreurs de Python sont rejouées ;
//! - invariants, vérifiés sur chaque cas.

use otb_advisor::noise::{self, FilterBlock, NoiseReport};
use otb_advisor::profiles::profile;
use otb_advisor::rules::{self, Finding};
use serde_json::Value;
use std::collections::BTreeMap;

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;
#[path = "support/mod.rs"]
mod support;
use support::*;

/// Tolérance des niveaux de bruit en dB (spectres de `realfft` contre pocketfft : L-048), relative au plus grand élément de la liste.
/// **Mesurée sur ces cas** : voir la ligne « écart max bruit (dB) » affichée avec `--nocapture`.
const LEVEL_RTOL: f64 = 1e-11;

fn data() -> Value {
    cas("noise")
}

fn finding(key: &str, category: &str) -> Finding {
    Finding::new(key, category, rules::INFO, "t", "d", Vec::new(), None)
}

#[test]
fn section_of_range_chaque_cle_et_chaque_categorie_comme_python() {
    let c = data();
    let rows = c["sections"].as_array().unwrap();
    let mut by_section: BTreeMap<String, usize> = BTreeMap::new();
    for r in rows {
        let (key, cat) = (r[0].as_str().unwrap(), r[1].as_str().unwrap());
        let want = r[2].as_str();
        let got = noise::section_of(&finding(key, cat));
        assert_eq!(got, want, "section_of({key}, {cat})");
        *by_section.entry(want.unwrap_or("None").to_string()).or_insert(0) += 1;
    }
    // chaque partie reçoit des constats, y compris « hors sujet »
    for s in ["conditions", "dterm", "gyro", "strategy", "None"] {
        assert!(by_section.contains_key(s), "aucune clé ne va dans « {s} »");
    }
    // les listes de clés elles-mêmes (le test précédent ne les voit qu'à travers section_of)
    let k = &c["keys"];
    let to_vec = |v: Vec<&str>| v.into_iter().map(String::from).collect::<Vec<_>>();
    assert_eq!(to_vec(noise::rpm_keys()), strings(&k["rpm"]), "clés du filtre RPM");
    assert_eq!(
        to_vec(noise::notch_keys()),
        strings(&k["notch"]),
        "clés du notch dynamique"
    );
    assert_eq!(
        to_vec(noise::gyro_lowpass_keys()),
        strings(&k["gyro_lowpass"]),
        "clés du passe-bas du gyro"
    );
}

#[test]
fn lowpass_text_et_level_note_comme_python() {
    let c = data();
    let mut seen_cut = false;
    let mut seen_dyn = false;
    let mut seen_static = false;
    for r in c["lowpass"].as_array().unwrap() {
        let lp = lowpass(&r[0]);
        let want = r[1].as_str().unwrap();
        assert_eq!(noise::lowpass_text(&lp), want);
        seen_cut |= want == "coupé";
        seen_dyn |= want.contains("dynamique");
        seen_static |= !want.contains("dynamique") && want != "coupé";
    }
    assert!(
        seen_cut && seen_dyn && seen_static,
        "les trois formes du texte doivent être couvertes"
    );
    for r in c["levels"].as_array().unwrap() {
        assert_eq!(noise::level_note(r[0].as_str().unwrap()), r[1].as_str().unwrap());
    }
}

fn check_list(what: &str, got: &[Finding], want: &Value) {
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

fn check_block(what: &str, got: &FilterBlock, want: &Value) {
    assert_eq!(got.title, want["title"].as_str().unwrap(), "{what} : titre");
    assert_eq!(got.settings, want["settings"].as_str().unwrap(), "{what} : réglages");
    assert_eq!(
        got.observations,
        strings(&want["observations"]),
        "{what} : observations"
    );
    check_list(&format!("{what} (constats)"), &got.findings, &want["findings"]);
    match (&got.verdict, want["verdict"].is_null()) {
        (None, true) => {}
        (Some(g), false) => check_finding(&format!("{what} (verdict)"), g, &want["verdict"]),
        (g, w) => panic!("{what} : verdict : Rust {}, Python None={w}", g.is_some()),
    }
}

fn check_axes(what: &str, got: &[noise::AxisNoise], want: &Value) {
    let want = want.as_array().unwrap();
    assert_eq!(got.len(), want.len(), "{what} : nombre d'axes");
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.axis, w["axis"].as_str().unwrap(), "{what} : axe");
        assert!(same(g.peak_hz, f1(&w["hz"])), "{what} {} : fréquence du sommet", g.axis);
        for (name, v, k) in [
            ("avant", g.before_db, "before"),
            ("après", g.after_db, "after"),
            ("baisse", g.reduction_db, "drop"),
        ] {
            close1(
                "bruit (dB)",
                v,
                f1(&w[k]),
                LEVEL_RTOL,
                &format!("{what} {} : niveau {name}", g.axis),
            );
        }
    }
}

fn check_noise(label: &str, nr: &NoiseReport, want: &Value) {
    assert_bits(
        &[nr.band.0, nr.band.1],
        &hexes(&want["band"]),
        &format!("{label} : bande"),
    );
    assert_eq!(nr.moving, want["moving"].as_bool().unwrap(), "{label} : mouvements");
    assert_eq!(
        nr.has_unfiltered,
        want["has_unfiltered"].as_bool().unwrap(),
        "{label} : gyro non filtré"
    );
    check_axes(&format!("{label} : gyro"), &nr.gyro, &want["gyro"]);
    check_axes(&format!("{label} : D"), &nr.dterm, &want["dterm"]);
    match (&nr.harmonics, want["harmonics"].is_null()) {
        (None, true) => {}
        (Some((raw, left)), false) => {
            assert_close(
                "bruit (dB)",
                raw,
                &hexes(&want["harmonics"][0]),
                LEVEL_RTOL,
                &format!("{label} : harmoniques avant filtres"),
            );
            assert_close(
                "bruit (dB)",
                left,
                &hexes(&want["harmonics"][1]),
                LEVEL_RTOL,
                &format!("{label} : harmoniques après filtres"),
            );
        }
        (g, w) => panic!("{label} : harmoniques : Rust {}, Python None={w}", g.is_some()),
    }
    check_list(&format!("{label} : conditions"), &nr.conditions, &want["conditions"]);
    check_list(&format!("{label} : gyro"), &nr.gyro_findings, &want["gyro_findings"]);
    check_list(&format!("{label} : D"), &nr.dterm_findings, &want["dterm_findings"]);
    check_list(&format!("{label} : stratégie"), &nr.strategy, &want["strategy"]);
    check_list(
        &format!("{label} : gyro hors blocs"),
        &nr.gyro_other,
        &want["gyro_other"],
    );
    let wb = want["gyro_blocks"].as_array().unwrap();
    assert_eq!(nr.gyro_blocks.len(), wb.len(), "{label} : nombre de blocs");
    for (g, w) in nr.gyro_blocks.iter().zip(wb) {
        check_block(&format!("{label} : bloc {}", g.title), g, w);
    }
    check_block(
        &format!("{label} : bloc du D"),
        nr.dterm_block.as_ref().expect("bloc du D absent"),
        &want["dterm_block"],
    );
    assert_eq!(
        nr.report.findings.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        want["report_findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap())
            .collect::<Vec<_>>(),
        "{label} : constats du rapport"
    );
}

/// Invariants indépendants de Python : trois blocs, bloc du D, stratégie jamais vide, constats hors blocs disjoints des blocs, chaque
/// constat du rapport rangé une fois.
fn check_invariants(label: &str, nr: &NoiseReport) {
    assert_eq!(nr.gyro_blocks.len(), 3, "{label} : trois blocs de filtres");
    assert!(nr.dterm_block.is_some(), "{label} : bloc du D");
    assert!(!nr.strategy.is_empty(), "{label} : stratégie vide");
    for f in &nr.gyro_other {
        assert!(
            nr.gyro_blocks.iter().all(|b| !b.findings.contains(f)),
            "{label} : {} est à la fois hors blocs et dans un bloc",
            f.key
        );
        assert!(
            nr.gyro_findings.contains(f),
            "{label} : {} hors blocs n'est pas un constat du gyro",
            f.key
        );
    }
    for f in &nr.gyro_findings {
        let in_blocks = nr.gyro_blocks.iter().any(|b| b.findings.contains(f));
        let other = nr.gyro_other.contains(f);
        assert!(
            in_blocks != other,
            "{label} : {} doit être dans les blocs ou hors blocs, pas les deux",
            f.key
        );
    }
    // la stratégie de repli (aucune stratégie dans le rapport) s'ajoute aux constats du rapport
    let in_report = |sec: &str| {
        nr.report
            .findings
            .iter()
            .filter(|f| noise::section_of(f) == Some(sec))
            .count()
    };
    assert_eq!(
        nr.conditions.len(),
        in_report("conditions"),
        "{label} : conditions rangées une fois"
    );
    assert_eq!(
        nr.gyro_findings.len(),
        in_report("gyro"),
        "{label} : gyro rangés une fois"
    );
    assert_eq!(
        nr.dterm_findings.len(),
        in_report("dterm"),
        "{label} : D rangés une fois"
    );
    assert_eq!(
        nr.strategy.len(),
        in_report("strategy").max(1),
        "{label} : stratégie rangée une fois"
    );
}

#[test]
fn noise_report_rend_le_diagnostic_de_python() {
    let c = data();
    let mut verdicts: BTreeMap<String, usize> = BTreeMap::new();
    let mut errors = 0;
    let mut texts = [
        ("rpm_filter_weights n'existe qu'à partir de Betaflight 4.5", false),
        ("non traitée par le filtre RPM", false),
        ("aucun poids ne peut baisser", false),
    ];
    for case in c["cases"].as_array().unwrap() {
        let label = case["label"].as_str().unwrap();
        let lg = log_of(&c, &case["log"]);
        let sel = case["sel"]
            .as_array()
            .map(|s| s[0].as_u64().unwrap() as usize..s[1].as_u64().unwrap() as usize);
        let prof = case["profile"].as_str().map(|k| profile(k).unwrap().clone());
        let got = noise::noise_report(&lg, sel, prof.as_ref());
        if is_error(case) {
            errors += 1;
            assert!(got.is_err(), "{label} : Python lève, Rust rend un diagnostic");
            continue;
        }
        let nr = got.unwrap_or_else(|e| panic!("{label} : Rust lève ({e}), Python rend un diagnostic"));
        check_noise(label, &nr, &case["out"]);
        check_invariants(label, &nr);
        for b in nr.gyro_blocks.iter().chain(nr.dterm_block.iter()) {
            if let Some(v) = &b.verdict {
                *verdicts.entry(v.key.clone()).or_insert(0) += 1;
            }
            for (t, seen) in texts.iter_mut() {
                *seen |= b.observations.iter().any(|o| o.contains(*t))
                    || b.verdict.as_ref().is_some_and(|v| v.detail.contains(*t));
            }
        }
        for f in &nr.strategy {
            *verdicts.entry(f.key.clone()).or_insert(0) += 1;
        }
    }
    report();
    assert!(errors >= 2, "les cas où Python lève sont absents");
    // chaque verdict et chaque stratégie de repli sont produits au moins une fois
    for k in [
        "rpm_block_no_rpm",
        "rpm_block_off",
        "rpm_block_unmeasurable",
        "rpm_block_ok",
        "notch_block_ok",
        "notch_block_both_off",
        "notch_block_unjudged",
        "notch_block_peak_outside",
        "lowpass_block_ok",
        "filter_no_change",
        "filter_single_step",
        "filter_strategy",
    ] {
        assert!(
            verdicts.contains_key(k),
            "le verdict ou la stratégie {k} n'est produit par aucun cas"
        );
    }
    for (t, seen) in texts {
        assert!(seen, "le texte « {t} » n'est produit par aucun cas");
    }
    eprintln!("verdicts comparés : {verdicts:?}");
}
