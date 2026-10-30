//! Les curseurs de `analysis/sliders.py` rendent ce que rend Python : cas écrits par `scripts/sliders_cases.py` (régénérer
//! `tests/data/sliders.json` avec ce script si `sliders.py` change).
//!
//! Exigé **identique** : les PID calculés par les curseurs (arithmétique `f32` dans l'ordre du C), les fréquences de filtres, les
//! commandes CLI, les erreurs. À une tolérance mesurée : l'estimation des curseurs depuis les PID (`sliders_from_pids`, SVD de
//! Jacobi à la place de LAPACK, `ln` et `exp`), et les valeurs de `slider_view` qui en dépendent (les booléens, eux, sont exigés
//! identiques : aucune estimation arrondie ne doit basculer).

use otb_analysis::sliders::*;
use otb_log::settings::{AxisPid, parse_settings};
use serde_json::Value;
use std::collections::BTreeMap;

mod common;
use common::*;

/// Estimation des curseurs depuis les PID : écart relatif au plus grand curseur (en %, ≈ 100). Mesuré : voir `report()`.
const FROM_PIDS_RTOL: f64 = 1e-11;

/// Un curseur du fichier : `null` (absent), un entier, `"nan"` / `"inf"` / `"-inf"` ou 16 caractères hexadécimaux.
fn slider(v: &Value) -> Option<f64> {
    match v {
        Value::Null => None,
        Value::Number(n) => Some(n.as_f64().unwrap()),
        Value::String(s) => Some(match s.as_str() {
            "nan" => f64::NAN,
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            h => f1(&Value::String(h.to_string())),
        }),
        other => panic!("curseur : {other:?}"),
    }
}

/// Paires (clé, valeur) sans les curseurs absents (`None` de Python).
fn sliders(v: &Value) -> Vec<(String, f64)> {
    v.as_array()
        .unwrap()
        .iter()
        .filter_map(|kv| slider(&kv[1]).map(|x| (kv[0].as_str().unwrap().to_string(), x)))
        .collect()
}

fn borrowed(v: &[(String, f64)]) -> Vec<(&str, f64)> {
    v.iter().map(|(k, x)| (k.as_str(), *x)).collect()
}

fn pid(v: &Value) -> AxisPid {
    let n: Vec<i64> = v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
    AxisPid {
        p: n[0],
        i: n[1],
        d: n[2],
        dmax: n[3],
        f: n[4],
    }
}

fn pids(v: &Value) -> Vec<AxisPid> {
    v.as_array().unwrap().iter().map(pid).collect()
}

#[test]
fn default_pids_identiques() {
    let c = cas("sliders");
    for k in c["default"].as_array().unwrap() {
        assert_eq!(default_pids(k["fam"].as_str().unwrap()), pids(&k["out"]));
    }
}

#[test]
fn pids_from_sliders_identiques() {
    let c = cas("sliders");
    let (mut ok, mut errors) = (0, 0);
    for (n, k) in c["pids"].as_array().unwrap().iter().enumerate() {
        let s = sliders(&k["s"]);
        let base = (!k["base"].is_null()).then(|| pids(&k["base"]));
        let got = pids_from_sliders(&borrowed(&s), k["fam"].as_str().unwrap(), base.as_deref());
        if is_error(k) {
            assert!(got.is_err(), "cas {n} {s:?} : Python lève, Rust rend {got:?}");
            errors += 1;
        } else {
            assert_eq!(
                got.as_ref(),
                Ok(&pids(&k["out"])),
                "cas {n} {} {s:?} base {:?}",
                k["fam"],
                k["base"]
            );
            ok += 1;
        }
    }
    eprintln!("pids_from_sliders : {ok} cas rendus, {errors} erreurs identiques");
    assert!(ok > 5000 && errors > 100);
}

#[test]
fn filters_from_sliders_identiques() {
    let c = cas("sliders");
    let settings = all_settings(&c);
    for k in c["filters"].as_array().unwrap() {
        let s = k["set"].as_u64().map(|i| &settings[i as usize]);
        let got = filters_from_sliders(k["g"].as_i64().unwrap(), k["d"].as_i64().unwrap(), s);
        let want: Vec<(String, i64)> = k["out"]
            .as_array()
            .unwrap()
            .iter()
            .map(|kv| (kv[0].as_str().unwrap().to_string(), kv[1].as_i64().unwrap()))
            .collect();
        let got: Vec<(String, i64)> = got.into_iter().map(|(a, b)| (a.to_string(), b)).collect();
        assert_eq!(got, want, "{k}");
    }
}

/// Valeurs d'une estimation : clés dans l'ordre de Python, flottants en hexadécimal.
fn estimate(v: &Value) -> (Vec<String>, Vec<f64>) {
    let kv = v.as_array().unwrap();
    (
        kv.iter().map(|x| x[0].as_str().unwrap().to_string()).collect(),
        kv.iter().map(|x| f1(&x[1])).collect(),
    )
}

#[test]
fn sliders_from_pids_a_la_tolerance() {
    let c = cas("sliders");
    for (n, k) in c["from_pids"].as_array().unwrap().iter().enumerate() {
        let got = sliders_from_pids(&pids(&k["pids"]), k["fam"].as_str().unwrap());
        let (keys, want) = estimate(&k["out"]);
        assert_eq!(got.iter().map(|(k, _)| k.to_string()).collect::<Vec<_>>(), keys);
        let got: Vec<f64> = got.iter().map(|(_, v)| *v).collect();
        assert_close(
            "sliders_from_pids",
            &got,
            &want,
            FROM_PIDS_RTOL,
            &format!("cas {n} {k}"),
        );
    }
    report();
}

#[test]
fn slider_view_booleens_identiques() {
    let c = cas("sliders");
    let (mut header, mut consistent) = (0, 0);
    for (n, k) in c["view"].as_array().unwrap().iter().enumerate() {
        let raw: BTreeMap<String, String> = k["raw"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(a, b)| (a.clone(), b.as_str().unwrap().to_string()))
            .collect();
        let got = slider_view(&parse_settings(&raw));
        if is_error(k) {
            assert!(got.is_err(), "cas {n} : Python lève");
            continue;
        }
        let got = got.unwrap();
        assert_eq!(got.from_header, k["from_header"].as_bool().unwrap(), "cas {n} {raw:?}");
        assert_eq!(got.consistent, k["consistent"].as_bool().unwrap(), "cas {n} {raw:?}");
        let (keys, want) = estimate(&k["values"]);
        assert_eq!(got.values.iter().map(|(k, _)| k.to_string()).collect::<Vec<_>>(), keys);
        let vals: Vec<f64> = got.values.iter().map(|(_, v)| *v).collect();
        assert_close(
            "slider_view",
            &vals,
            &want,
            FROM_PIDS_RTOL / 10.0,
            &format!("cas {n} {raw:?}"),
        );
        header += got.from_header as usize;
        consistent += got.consistent as usize;
    }
    eprintln!("slider_view : {header} depuis l'en-tête, {consistent} cohérents");
    assert!(header > 300 && consistent > 300);
    report();
}

#[test]
fn cli_commands_identiques() {
    let c = cas("sliders");
    let (mut ok, mut errors) = (0, 0);
    for (n, k) in c["cli"].as_array().unwrap().iter().enumerate() {
        let s = sliders(&k["s"]);
        let got = cli_commands(&borrowed(&s), k["fam"].as_str().unwrap());
        if is_error(k) {
            assert!(got.is_err(), "cas {n} {s:?} : Python lève, Rust rend {got:?}");
            errors += 1;
        } else {
            let want: Vec<String> = k["out"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap().to_string())
                .collect();
            assert_eq!(got.unwrap(), want, "cas {n} {s:?}");
            ok += 1;
        }
    }
    eprintln!("cli_commands : {ok} cas rendus, {errors} erreurs identiques");
    assert!(ok > 500 && errors > 20);
}
