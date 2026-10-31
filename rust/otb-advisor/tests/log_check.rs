//! `advisor/log_check.py` porté en Rust (phase 5b-2) sur des journaux fabriqués : cas écrits par `scripts/log_check_cases.py` (régénérer
//! `tests/data/log_check.json` avec ce script si `log_check.py` ou ce qu'il appelle change).
//!
//! - `check_log` : **tout** est comparé (points : clé, titre, état, texte, corrections ; usages ; parties du vol ; fenêtres ; curseurs ;
//!   rôle ; retards ; vues `items_for` / `usages_for` / `cli_commands` des deux rôles) ; textes au caractère près ;
//! - `check_logs` : groupes de logs (séries, vols identiques, hors série) ;
//! - `filter_targets`, `FilterTargets::checks` / `reached` : repères de la phase Filtres ;
//! - `log_rates`, `axes_mixed_share`, `log_rates_fix`, tables.

use otb_advisor::log_check::{self, CheckItem, FilterTargets, LogCheck, Usage};
use serde_json::Value;
use std::collections::BTreeMap;

#[path = "../../otb-analysis/tests/common/mod.rs"]
mod common;
use common::*;
#[path = "support/mod.rs"]
mod support;
use support::*;

/// Retards (FFT, `atan2` : L-048), relatifs à la plus grande valeur de la liste ; voir `DELAY_RTOL` de `support`.
const DELAY: f64 = DELAY_RTOL;

fn data() -> Value {
    cas("log_check")
}

fn opt_pair(v: &Value) -> Option<(f64, f64)> {
    (!v.is_null()).then(|| {
        let a = hexes(v);
        (a[0], a[1])
    })
}

fn same_pair(got: Option<(f64, f64)>, want: Option<(f64, f64)>, what: &str) {
    match (got, want) {
        (None, None) => {}
        (Some(g), Some(w)) => assert!(same(g.0, w.0) && same(g.1, w.1), "{what} : Rust {g:?} / Python {w:?}"),
        (g, w) => panic!("{what} : Rust {g:?} / Python {w:?}"),
    }
}

fn check_items(what: &str, got: &[CheckItem], want: &Value) {
    let want = want.as_array().unwrap();
    assert_eq!(
        got.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(),
        want.iter().map(|w| w["key"].as_str().unwrap()).collect::<Vec<_>>(),
        "{what} : liste des points"
    );
    for (g, w) in got.iter().zip(want) {
        let key = &g.key;
        assert_eq!(g.title, w["title"].as_str().unwrap(), "{what} : {key} titre");
        assert_eq!(g.status, w["status"].as_str().unwrap(), "{what} : {key} état");
        assert_eq!(g.detail, w["detail"].as_str().unwrap(), "{what} : {key} texte");
        let fixes = w["fixes"].as_array().unwrap();
        assert_eq!(g.fixes.len(), fixes.len(), "{what} : {key} nombre de corrections");
        for (gf, wf) in g.fixes.iter().zip(fixes) {
            assert_eq!(gf.where_, wf[0].as_str().unwrap(), "{what} : {key} où");
            assert_eq!(gf.cli, wf[1].as_str().unwrap(), "{what} : {key} commandes");
        }
    }
}

fn check_usages(what: &str, got: &[Usage], want: &Value) {
    let want = want.as_array().unwrap();
    assert_eq!(got.len(), want.len(), "{what} : nombre d'usages");
    for (g, w) in got.iter().zip(want) {
        let key = &g.key;
        assert_eq!(*key, w["key"].as_str().unwrap(), "{what} : clé d'usage");
        assert_eq!(g.label, w["label"].as_str().unwrap(), "{what} : {key} libellé");
        assert_eq!(g.ok, w["ok"].as_bool().unwrap(), "{what} : {key} ok");
        assert_eq!(g.reason, w["reason"].as_str().unwrap(), "{what} : {key} raison");
        assert_eq!(g.shown, w["shown"].as_bool().unwrap(), "{what} : {key} affiché");
    }
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect()
}

fn check_views(what: &str, ck: &LogCheck, w: &Value) {
    for (name, role) in [
        ("null", None),
        (log_check::FILTERS, Some(log_check::FILTERS)),
        (log_check::PID, Some(log_check::PID)),
    ] {
        let items: Vec<String> = ck.items_for(role).iter().map(|i| i.key.clone()).collect();
        assert_eq!(items, strs(&w["items_for"][name]), "{what} : items_for({name})");
        let usages: Vec<String> = ck.usages_for(role).iter().map(|u| u.key.clone()).collect();
        assert_eq!(usages, strs(&w["usages_for"][name]), "{what} : usages_for({name})");
    }
    assert_eq!(ck.cli_commands(), strs(&w["cli"]), "{what} : commandes CLI");
}

fn compare(what: &str, ck: &LogCheck, w: &Value) {
    assert_eq!(ck.label, w["label"].as_str().unwrap(), "{what} : libellé");
    assert_eq!(ck.moving, w["moving"].as_bool().unwrap(), "{what} : mouvements");
    same_pair(
        ck.hover,
        opt_pair(&w["hover"]),
        &format!("{what} : partie stationnaire"),
    );
    same_pair(
        ck.moving_part,
        opt_pair(&w["moving_part"]),
        &format!("{what} : partie avec mouvements"),
    );
    check_items(what, &ck.items, &w["items"]);
    check_usages(what, &ck.usages, &w["usages"]);
    let win: Vec<i64> = w["windows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap())
        .collect();
    assert_eq!(
        ck.windows.iter().map(|&x| x as i64).collect::<Vec<_>>(),
        win,
        "{what} : fenêtres"
    );
    assert_eq!(ck.exact, w["exact"].as_bool().unwrap(), "{what} : exact");
    let sl = w["sliders"].as_array().unwrap();
    assert_eq!(ck.sliders.len(), sl.len(), "{what} : curseurs");
    for ((k, v), s) in ck.sliders.iter().zip(sl) {
        assert_eq!(*k, s[0].as_str().unwrap(), "{what} : clé de curseur");
        assert!(
            same(*v, f1(&s[1])),
            "{what} : curseur {k} : Rust {v} / Python {}",
            f1(&s[1])
        );
    }
    assert_eq!(ck.role, w["role"].as_str().unwrap(), "{what} : rôle");
    assert_close(
        "délai consigne",
        &ck.sp_delay_ms,
        &hexes(&w["sp_delay"]),
        DELAY,
        &format!("{what} : retard consigne → gyro"),
    );
    check_views(what, ck, w);
}

#[test]
fn check_log_rend_les_points_de_python() {
    let c = data();
    let cases = c["cases"].as_array().unwrap();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for k in cases {
        let label = k["label"].as_str().unwrap();
        let lg = log_of(&c, &k["log"]);
        let sel = (!k["sel"].is_null()).then(|| {
            let s = k["sel"].as_array().unwrap();
            s[0].as_u64().unwrap() as usize..s[1].as_u64().unwrap() as usize
        });
        let got = log_check::check_log(&lg, sel, label, k["exact"].as_bool().unwrap());
        if is_error(k) {
            assert!(got.is_err(), "{label} : Python lève, Rust rend un résultat");
            continue;
        }
        let ck = got.unwrap_or_else(|e| panic!("{label} : Rust échoue ({e}) alors que Python réussit"));
        compare(label, &ck, &k["out"]);
        for it in &ck.items {
            *seen.entry(format!("{}:{}", it.key, it.status)).or_insert(0) += 1;
        }
        for u in &ck.usages {
            *seen.entry(format!("usage {}:{}", u.key, u.ok)).or_insert(0) += 1;
        }
    }
    report();
    // chaque branche des points est atteinte par au moins un cas
    for key in [
        "version:ok",
        "version:bloquant",
        "duration:ok",
        "duration:à revoir",
        "log_rate:ok",
        "log_rate:bloquant",
        "log_rate:à revoir",
        "pid_log_rate:ok",
        "pid_log_rate:à revoir",
        "pid_log_rate:bloquant",
        "gyro_unfilt:ok",
        "gyro_unfilt:bloquant",
        "debug_mode:à revoir",
        "rpm:ok",
        "rpm:à revoir",
        "parts:info",
        "windows:ok",
        "windows:à revoir",
        "windows:bloquant",
        "rates:ok",
        "rates:à revoir",
        "angle_limit:ok",
        "angle_limit:à revoir",
        "angle_limit:info",
        "flight_mode:ok",
        "flight_mode:info",
        "flight_mode:à revoir",
        "axes_mixed:ok",
        "axes_mixed:à revoir",
        "i_slider:à revoir",
        "dyn_damping:à revoir",
        "simplified:à revoir",
        "gaps:info",
        "usage series:true",
        "usage series:false",
        "usage noise:true",
        "usage noise:false",
        "usage delay:true",
        "usage delay:false",
        "usage ff_series:true",
        "usage ff_series:false",
    ] {
        assert!(seen.contains_key(key), "branche jamais atteinte par les cas : {key}");
    }
}

#[test]
fn check_logs_rend_les_groupes_de_python() {
    let c = data();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for g in c["groups"].as_array().unwrap() {
        let label = g["label"].as_str().unwrap();
        let logs: Vec<(String, otb_log::flightlog::FlightLog)> = g["logs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| (l[0].as_str().unwrap().to_string(), log_of(&c, &l[1])))
            .collect();
        let refs: Vec<(String, &otb_log::flightlog::FlightLog)> = logs.iter().map(|(n, l)| (n.clone(), l)).collect();
        let delays: BTreeMap<String, Vec<f64>> = g["delays"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), hexes(v)))
            .collect();
        let got = log_check::check_logs(&refs, &delays).unwrap_or_else(|e| panic!("{label} : {e}"));
        let want = g["out"].as_array().unwrap();
        assert_eq!(got.len(), want.len(), "{label} : nombre de groupes");
        for (gg, w) in got.iter().zip(want) {
            assert_eq!(gg.craft, w["craft"].as_str().unwrap(), "{label} : drone");
            assert_eq!(gg.labels, strs(&w["labels"]), "{label} : libellés");
            assert_eq!(gg.status, w["status"].as_str().unwrap(), "{label} : état");
            assert_eq!(gg.detail, w["detail"].as_str().unwrap(), "{label} : texte");
            *kinds
                .entry(format!("{}:{}", gg.status, gg.detail.split(' ').next().unwrap()))
                .or_insert(0) += 1;
        }
    }
    for key in ["ok:Série", "info:Mêmes", "info:Hors"] {
        assert!(kinds.contains_key(key), "groupe jamais produit : {key}");
    }
}

fn compare_targets(what: &str, ft: &FilterTargets, w: &Value) {
    let v = hexes(&w["vals"]);
    assert_close(
        "repère bruit",
        &[ft.gyro_ptb_db, ft.dterm_ptb_db],
        &v[..2],
        1e-11,
        &format!("{what} : bruit"),
    );
    assert_close(
        "repère retard",
        &[ft.gyro_delay_ms, ft.dterm_delay_ms],
        &v[2..],
        DELAY,
        &format!("{what} : retards"),
    );
    same_pair(ft.delay_goal_ms, opt_pair(&w["goal"]), &format!("{what} : objectif"));
}

#[test]
fn filter_targets_rend_les_reperes_de_python() {
    let c = data();
    let (mut some, mut none) = (0, 0);
    for t in c["targets"].as_array().unwrap() {
        let label = t["label"].as_str().unwrap();
        let lg = log_of(&c, &t["log"]);
        let inch = (!t["inch"].is_null()).then(|| f1(&t["inch"]));
        let got = log_check::filter_targets(&lg, opt_pair(&t["hover"]), opt_pair(&t["moving"]), inch);
        if is_error(t) {
            assert!(got.is_err(), "{label} : Python lève, Rust rend un résultat");
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{label} : Rust échoue ({e}) alors que Python réussit"));
        match (&got, t["out"].is_null()) {
            (None, true) => none += 1,
            (Some(ft), false) => {
                some += 1;
                compare_targets(label, ft, &t["out"]);
                // textes et verdicts : calculés depuis les valeurs de Rust, donc comparés sur les valeurs de Python juste après
                let w = &t["out"];
                let from_py = FilterTargets {
                    gyro_ptb_db: hexes(&w["vals"])[0],
                    dterm_ptb_db: hexes(&w["vals"])[1],
                    gyro_delay_ms: hexes(&w["vals"])[2],
                    dterm_delay_ms: hexes(&w["vals"])[3],
                    delay_goal_ms: opt_pair(&w["goal"]),
                };
                compare_checks(label, &from_py, w);
            }
            _ => panic!("{label} : Rust {got:?} / Python {}", t["out"]),
        }
    }
    report();
    assert!(
        some >= 10 && none >= 3,
        "trop peu de cas : {some} repères, {none} absents"
    );
}

fn compare_checks(what: &str, ft: &FilterTargets, w: &Value) {
    let checks = ft.checks();
    let want = w["checks"].as_array().unwrap();
    assert_eq!(checks.len(), want.len(), "{what} : nombre de repères");
    for ((name, ok, text), x) in checks.iter().zip(want) {
        assert_eq!(*name, x[0].as_str().unwrap(), "{what} : nom");
        assert_eq!(*ok, x[1].as_bool(), "{what} : {name} atteint");
        assert_eq!(*text, x[2].as_str().unwrap(), "{what} : {name} texte");
    }
    assert_eq!(ft.reached(), w["reached"].as_bool(), "{what} : reached");
}

#[test]
fn les_reperes_ont_les_memes_textes_et_verdicts() {
    let c = data();
    let list = c["target_checks"].as_array().unwrap();
    let mut verdicts: BTreeMap<String, usize> = BTreeMap::new();
    for (i, w) in list.iter().enumerate() {
        let v = hexes(&w["vals"]);
        let ft = FilterTargets {
            gyro_ptb_db: v[0],
            dterm_ptb_db: v[1],
            gyro_delay_ms: v[2],
            dterm_delay_ms: v[3],
            delay_goal_ms: opt_pair(&w["goal"]),
        };
        compare_checks(&format!("repères {i}"), &ft, w);
        *verdicts.entry(format!("{:?}", ft.reached())).or_insert(0) += 1;
    }
    for k in ["Some(true)", "Some(false)", "None"] {
        assert!(verdicts.contains_key(k), "verdict jamais produit : {k}");
    }
}

#[test]
fn log_rates_et_axes_mixed_share_comme_python() {
    let c = data();
    for r in c["rates"].as_array().unwrap() {
        let raw: std::collections::BTreeMap<String, String> = r["raw"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, x)| (k.clone(), x.as_str().unwrap().to_string()))
            .collect();
        let s = otb_log::settings::parse_settings(&raw);
        let got = log_check::log_rates(&s);
        let want = (!r["out"].is_null()).then(|| {
            let a = r["out"].as_array().unwrap();
            let v = |i: usize| {
                a[i].as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_i64().unwrap())
                    .collect::<Vec<_>>()
            };
            (v(0), v(1), v(2))
        });
        assert_eq!(got, want, "log_rates({raw:?})");
    }
    let m = &c["mixed"];
    let lg = log_of(&c, &m["log"]);
    for k in m["cases"].as_array().unwrap() {
        let s = k["sel"].as_array().unwrap();
        let (a, b) = (s[0].as_u64().unwrap() as usize, s[1].as_u64().unwrap() as usize);
        let got = log_check::axes_mixed_share(&lg, a..b, f1(&k["thr"]));
        assert!(
            same(got, f1(&k["out"])),
            "axes_mixed_share({a}..{b}, {}) : Rust {got} / Python {}",
            f1(&k["thr"]),
            f1(&k["out"])
        );
    }
    let fix = log_check::log_rates_fix();
    assert_eq!(fix.where_, c["rates_fix"][0].as_str().unwrap());
    assert_eq!(fix.cli, c["rates_fix"][1].as_str().unwrap());
}

#[test]
fn les_tables_sont_celles_de_python() {
    let c = &data()["constants"];
    let want: Vec<(String, String)> = c["configurator"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
        .collect();
    assert_eq!(want.len(), log_check::CONFIGURATOR_SLIDERS.len());
    for (k, v) in &want {
        assert_eq!(log_check::configurator_slider(k), v);
    }
    let roles = c["role_names"].as_object().unwrap();
    for (k, v) in log_check::ROLE_NAMES {
        assert_eq!(roles[k].as_str().unwrap(), v);
    }
}
