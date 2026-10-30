//! `debug_tables` et `modes` rendent ce que Python rend : tables entières et grille de cas écrites par
//! `scripts/log_tables_cases.py` (régénérer `tests/data/tables.json` avec ce script si les tables Python changent).

use otb_log::{debug_tables, modes};
use serde_json::Value;

fn cas() -> Value {
    let p = format!("{}/tests/data/tables.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn version(v: &Value) -> Vec<i64> {
    v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect()
}

fn names(v: &Value) -> Vec<&str> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect()
}

#[test]
fn les_tables_de_debogage_sont_identiques() {
    let c = cas();
    for (fam, t) in c["debug_tables"].as_object().unwrap() {
        assert_eq!(debug_tables::table(fam), names(t), "famille {fam}");
    }
    assert_eq!(c["debug_tables"].as_object().unwrap().len(), 4);
}

#[test]
fn les_tables_de_modes_sont_identiques() {
    let c = cas();
    for (fam, t) in c["mode_tables"].as_object().unwrap() {
        assert_eq!(modes::table(fam), names(t), "famille {fam}");
    }
    let mut shown: Vec<&str> = modes::SHOWN.to_vec();
    shown.sort();
    assert_eq!(shown, names(&c["shown"]));
}

#[test]
fn family_suit_python_sur_la_grille() {
    for e in cas()["family"].as_array().unwrap() {
        let v = version(&e[0]);
        assert_eq!(debug_tables::family(&v), e[1].as_str().unwrap(), "version {v:?}");
    }
}

#[test]
fn debug_mode_name_suit_python() {
    for e in cas()["debug_mode_name"].as_array().unwrap() {
        let (i, v, explicit) = (e[0].as_i64(), version(&e[1]), e[2].as_str());
        assert_eq!(
            debug_tables::debug_mode_name(i, &v, explicit),
            e[3].as_str().unwrap(),
            "{e}"
        );
    }
}

#[test]
fn mode_names_suit_python() {
    for e in cas()["mode_names"].as_array().unwrap() {
        let (mask, v, shown) = (e[0].as_i64().unwrap(), version(&e[1]), e[2].as_bool().unwrap());
        assert_eq!(modes::mode_names(mask, &v, shown), names(&e[3]), "{e}");
    }
}

#[test]
fn mode_changes_suit_python() {
    for e in cas()["mode_changes"].as_array().unwrap() {
        let flags: Option<Vec<i32>> = e[0]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_i64().unwrap() as i32).collect());
        let t: Vec<f64> = e[1].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        let want: Vec<(f64, &str)> = e[3]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p[0].as_f64().unwrap(), p[1].as_str().unwrap()))
            .collect();
        let got = modes::mode_changes(flags.as_deref(), &t, &version(&e[2]));
        let got: Vec<(f64, &str)> = got.iter().map(|(x, s)| (*x, s.as_str())).collect();
        assert_eq!(got, want, "{e}");
    }
}
