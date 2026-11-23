//! Mesures suivies dans le temps (lot 31), sur `tests/data/jeno5_5s.bbl` : chaque grandeur est celle du rapport de
//! l'assistant (jamais un second calcul), et **une mesure rangée dans la bibliothèque se recalcule à l'identique** —
//! le critère d'arrêt du lot 31 dans `docs/plan.md`.

use otb_advisor::rules::analyze;
use otb_advisor::tracked::{
    self, ALL_AXES, AXIS_KEYS, CALC_VERSION, DTERM_NOISE_DB, GYRO_FILTER_DELAY_MS, GYRO_NOISE_DB, LATENCY_MS,
    PHASE_MARGIN_DEG, STEP_WINDOWS, Tracked, WHOLE_KEYS,
};
use otb_log::flightlog::{self, FlightLog};
use otb_store::Library;
use otb_store::model::{Fragment, LogEntry, Measure};
use std::ops::Range;

fn journal() -> FlightLog {
    let p = format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"));
    flightlog::load(std::path::Path::new(&p)).unwrap().remove(0)
}

/// Des bornes qui ne sont ni le log entier ni ses limites automatiques (L-110). Sur ces 5 s, seul le tangage a assez
/// de mouvements pour une réponse indicielle (9 fenêtres) ; roll et lacet restent NaN.
fn bornes(log: &FlightLog) -> Range<usize> {
    let n = log.n();
    n / 20..n - n / 20
}

fn valeur(v: &[Tracked], axis: u64, key: &str) -> f64 {
    v.iter()
        .find(|t| t.axis == axis && t.key == key)
        .unwrap_or_else(|| panic!("{key} (axe {axis}) absent"))
        .value
}

/// Même nombre, NaN compris (une grandeur non mesurée reste non mesurée).
fn identiques(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

#[test]
fn toujours_les_memes_lignes_et_les_grandeurs_du_rapport() {
    let log = journal();
    let sel = bornes(&log);
    let (reglages, v) = tracked::measure(&log, sel.clone()).expect("mesure");
    assert!(reglages.starts_with("profil="), "{reglages}");
    assert_eq!(v.len(), 3 * AXIS_KEYS.len() + WHOLE_KEYS.len());
    for axis in 0..3 {
        for key in AXIS_KEYS {
            valeur(&v, axis, key);
        }
    }
    for key in WHOLE_KEYS {
        valeur(&v, ALL_AXES, key);
    }

    // les mêmes variables que les règles : lues dans le rapport, au bit près
    let rep = analyze(&log, Some(sel), None).expect("rapport");
    for (axis, st) in rep.steps.iter().enumerate() {
        let axis = axis as u64;
        assert!(identiques(valeur(&v, axis, LATENCY_MS), st.result.metrics.latency_ms));
        assert!(identiques(
            valeur(&v, axis, PHASE_MARGIN_DEG),
            st.result.loop_.phase_margin_deg
        ));
        assert_eq!(valeur(&v, axis, STEP_WINDOWS), st.result.n as f64);
    }
    let g = rep.measures.gyro_noise_db.as_ref().unwrap();
    assert!(identiques(valeur(&v, ALL_AXES, GYRO_NOISE_DB), g[0].max(g[1])));
    assert!(
        valeur(&v, 1, LATENCY_MS).is_finite(),
        "latence pitch mesurée sur ce log"
    );
    assert!(
        valeur(&v, 1, PHASE_MARGIN_DEG).is_finite(),
        "marge de phase pitch mesurée sur ce log"
    );
    assert!(valeur(&v, ALL_AXES, DTERM_NOISE_DB).is_finite(), "ce log a un terme D");
    let d = rep.measures.delays.as_ref().unwrap();
    if d.moving() {
        assert!(identiques(valeur(&v, ALL_AXES, GYRO_FILTER_DELAY_MS), d.gyro_ms));
    } else {
        assert!(
            valeur(&v, ALL_AXES, GYRO_FILTER_DELAY_MS).is_nan(),
            "vol sans mouvements : pas de retard suivi"
        );
    }
}

/// ⚠️ **Critère d'arrêt du lot 31** : une mesure rangée dans la bibliothèque, relue après fermeture, est exactement
/// celle que le calcul rend de nouveau. Sans cela, l'historique d'un drone mêlerait des chiffres qu'on ne sait plus
/// reproduire.
///
/// Contre-preuve vue en échec (2026-11-23) : une valeur arrondie à l'écriture (`(value * 1e6).round() / 1e6` dans
/// `Measure::to_value`) fait rougir la comparaison au bit près.
#[test]
fn une_mesure_rangee_se_recalcule_a_lidentique() {
    let log = journal();
    let sel = bornes(&log);
    let dir = std::env::temp_dir().join(format!("otb-advisor-suivi-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    let fragment = Fragment {
        id: "emp-1-1".into(),
        log_id: "emp-1".into(),
        session: 1,
        start: sel.start as u64,
        end: sel.end as u64,
        name: "jeno".into(),
        role: String::new(),
        date: "2026-11-23".into(),
        chosen: true,
    };
    let (reglages, v) = tracked::measure(&log, sel.clone()).expect("mesure");
    {
        let mut lib = Library::open(dir.clone());
        lib.load_measures();
        lib.add_log(LogEntry {
            id: "emp-1".into(),
            drone_id: "uid-1".into(),
            name: "jeno".into(),
            path: "/logs/jeno.bbl".into(),
            copied: false,
            bytes: 1,
            added: "2026-11-23T10:00:00".into(),
        });
        lib.put_fragment(fragment.clone());
        for t in &v {
            lib.set_measure(Measure {
                fragment_id: fragment.id.clone(),
                axis: t.axis,
                key: t.key.to_string(),
                value: t.value,
                start: fragment.start,
                end: fragment.end,
                settings: reglages.clone(),
                calc_version: CALC_VERSION,
            });
        }
        lib.save().expect("écriture");
    }

    let mut relue = Library::open(dir.clone());
    relue.load_measures();
    assert!(relue.problems().is_empty(), "{:?}", relue.problems());
    assert!(relue.is_measured(relue.fragment("emp-1-1").unwrap(), CALC_VERSION));
    let (reglages2, v2) = tracked::measure(&log, sel).expect("seconde mesure");
    assert_eq!(reglages2, reglages);
    let mut nan = 0;
    for t in &v2 {
        let m = relue
            .measure("emp-1-1", t.axis, t.key)
            .unwrap_or_else(|| panic!("{} (axe {}) non relue", t.key, t.axis));
        assert!(
            identiques(m.value, t.value),
            "{} (axe {}) : rangée {:e}, recalculée {:e}",
            t.key,
            t.axis,
            m.value,
            t.value
        );
        assert_eq!(m.settings, reglages);
        nan += usize::from(t.value.is_nan());
    }
    // roll et lacet sans réponse indicielle sur ces 5 s (5 grandeurs chacun) : tout le reste doit être un nombre
    assert_eq!(
        nan, 10,
        "grandeurs non mesurées : l'essai ne prouverait pas grand-chose"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
