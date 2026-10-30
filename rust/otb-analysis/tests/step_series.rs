//! `find_series` et `Series::steps` de `analysis/step_series.py` (phase 4c-B(e)) en Rust : cas écrits par `scripts/step_series_cases.py`
//! (régénérer `tests/data/step_series.json` avec ce script). Identité exigée : indices, valeurs et pas sont des arrondis à 2 ou 3 décimales.
//! `flight_step`, `flight_conditions` et `filter_signature` sont jugés sur le corpus (`otb-parity check-analysis`).

use otb_analysis::sliders::PTB_CATEGORIES;
use otb_analysis::step_series::{self, FlightConditions, FlightStep, Sig};
use serde_json::Value;

mod common;
use common::*;

fn sig_of(v: &Value) -> Sig {
    match v {
        Value::Array(a) => Sig::T(a.iter().map(sig_of).collect()),
        other => Sig::I(other.as_i64().unwrap()),
    }
}

fn flight(sliders: &[f64], filters: Sig) -> FlightStep {
    FlightStep {
        label: String::new(),
        sliders: PTB_CATEGORIES.iter().zip(sliders).map(|((c, _), v)| (*c, *v)).collect(),
        filters,
        conditions: FlightConditions {
            ff: (100, 100),
            angle_share: 0.0,
            i_slider: 1.0,
        },
        metrics: Vec::new(),
        n: Vec::new(),
        sp_delay_ms: Vec::new(),
        loops: Vec::new(),
    }
}

#[test]
fn find_series_conforme() {
    let c = cas("step_series");
    let (mut with_series, mut multi, mut total) = (0, 0, 0);
    for k in c["cases"].as_array().unwrap() {
        let what = format!("find_series {}", k["label"]);
        let flights: Vec<FlightStep> = k["flights"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| flight(&hexes(&f["sliders"]), sig_of(&f["filters"])))
            .collect();
        let got = step_series::find_series(&flights).unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let want = k["out"].as_array().unwrap();
        assert_eq!(got.len(), want.len(), "{what} : nombre de séries");
        for (g, w) in got.iter().zip(want) {
            assert_eq!(g.key, w["key"].as_str().unwrap(), "{what} : curseur");
            let idx: Vec<usize> = w["flights"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(g.flights, idx, "{what} : vols");
            assert_bits(&g.values, &hexes(&w["values"]), &format!("{what} : valeurs"));
            assert_bits(&g.steps(), &hexes(&w["steps"]), &format!("{what} : pas"));
        }
        total += 1;
        with_series += !want.is_empty() as usize;
        multi += (want.len() >= 2) as usize;
    }
    assert!(
        total >= 80 && with_series >= 25 && multi >= 1,
        "{total} {with_series} {multi}"
    );
}

/// Python trie une liste à NaN selon l'algorithme de `sorted` : non reproduit, refusé.
#[test]
fn find_series_refuse_un_curseur_nan() {
    let mut a = vec![1.0; 8];
    let mut b = vec![1.0; 8];
    a[0] = f64::NAN;
    b[0] = 1.1;
    let sig = Sig::T(vec![]);
    let flights = vec![flight(&a, sig.clone()), flight(&b, sig)];
    assert!(step_series::find_series(&flights).is_err());
}

#[test]
fn conditions_du_protocole_conformes() {
    let c = cas("step_series");
    let (mut angle, mut active) = (0, 0);
    for k in c["conditions"].as_array().unwrap() {
        let ff = (k["ff"][0].as_i64().unwrap(), k["ff"][1].as_i64().unwrap());
        let cond = FlightConditions {
            ff,
            angle_share: f1(&k["share"]),
            i_slider: 1.0,
        };
        let what = format!("conditions ff {ff:?} part {}", f1(&k["share"]));
        assert_eq!(cond.angle(), k["angle"].as_bool().unwrap(), "{what} : angle");
        assert_eq!(
            cond.ff_active(),
            k["ff_active"].as_bool().unwrap(),
            "{what} : ff_active"
        );
        assert_eq!(cond.ff_gain(0), k["ff_gain"][0].as_i64().unwrap(), "{what} : gain roll");
        assert_eq!(
            cond.ff_gain(1),
            k["ff_gain"][1].as_i64().unwrap(),
            "{what} : gain pitch"
        );
        angle += cond.angle() as usize;
        active += cond.ff_active() as usize;
    }
    assert!(angle >= 8 && active >= 8, "{angle} {active}");
}
