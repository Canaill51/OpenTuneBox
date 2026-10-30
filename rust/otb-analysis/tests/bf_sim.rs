//! La simulation de filtres de `analysis/bf_filters.py` (`fft_freq_centres`, `simulate_gyro`, `simulate_dterm`,
//! `whatif_spectrum`) rend ce que rend Python : cas écrits par `scripts/bf_sim_cases.py` sur de petits journaux fabriqués
//! (régénérer `tests/data/bf_sim.json` avec ce script si `bf_filters.py` change).
//!
//! Exigé **identique** : les centres lus dans le debug (copie), les fréquences, les erreurs, et « réglages inchangés ⇒ spectre
//! du log » (bit pour bit, comme Python). À une tolérance mesurée (L-048) : le gyro et le terme D simulés (filtres
//! récursifs dont les coefficients passent par `sin` / `cos`) et les spectres « et si » (FFT, `log10`, `hypot`).

use otb_analysis::bf_filters::*;
use otb_decoder::file::{Matrix, Stats};
use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::Settings;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

mod common;
use common::*;

/// Gyro et terme D simulés : filtres récursifs (état conservé sur tout le signal), coefficients de `sin` / `cos`. Mesuré : 0
/// (mêmes bits que Python, qui appelle le même noyau). Une autre libm change les coefficients d'≈ 1 ulp ; un filtre stable
/// ne l'amplifie guère, le Q d'un notch (≤ 10 ici) un peu : marge de six chiffres.
const SIM_RTOL: f64 = 1e-12;
/// Spectres « et si » en dB : FFT (4b-2) puis réponses de filtres (`sin` / `cos`). Mesuré : 1,7e-15 sur ces signaux à faible
/// dynamique ; sur un spectre très dynamique l'écart de la FFT monte à 1e-10 (L-048) : même tolérance qu'en 4b-2.
const WHATIF_RTOL: f64 = 1e-11;

fn rows(c: &Value, o: &Value, n: usize) -> Option<Rows> {
    (!o.is_null()).then(|| Rows {
        k: o["k"].as_u64().unwrap() as usize,
        n,
        data: input(c, &o["x"]).iter().map(|&x| x as f32).collect(),
    })
}

/// Journal fabriqué : seuls les champs que lisent les filtres sont remplis.
fn log(c: &Value, v: &Value, ss: &[Settings]) -> FlightLog {
    let n = v["n"].as_u64().unwrap() as usize;
    FlightLog {
        path: Default::default(),
        session: 1,
        settings: ss[v["settings"].as_u64().unwrap() as usize].clone(),
        t: input(c, &v["t"]),
        fs: f1(&v["fs"]),
        gyro: rows(c, &v["gyro"], n).unwrap(),
        gyro_unfilt: rows(c, &v["gyro_unfilt"], n),
        setpoint: Rows::default(),
        rc_command: Rows::default(),
        p_term: Rows::default(),
        i_term: Rows::default(),
        d_term: Rows::default(),
        f_term: Rows::default(),
        throttle: input(c, &v["throttle"]).iter().map(|&x| x as f32).collect(),
        motors: Rows::default(),
        motor_hz: rows(c, &v["motor_hz"], n),
        debug: rows(c, &v["debug"], n).unwrap(),
        vbat: None,
        current: None,
        acc: None,
        mode_flags: None,
        events: Matrix::default(),
        stats: Stats::default(),
        implausible_frames: 0,
        pid_columns: BTreeSet::new(),
    }
}

fn logs(c: &Value, ss: &[Settings]) -> Vec<FlightLog> {
    c["logs"].as_array().unwrap().iter().map(|v| log(c, v, ss)).collect()
}

fn opt_settings<'a>(k: &Value, ss: &'a [Settings]) -> Option<&'a Settings> {
    k["s"].as_u64().map(|i| &ss[i as usize])
}

#[test]
fn fft_freq_centres_identique() {
    let c = cas("bf_sim");
    let ss = all_settings(&c);
    let ls = logs(&c, &ss);
    let cases = c["fft_freq_centres"].as_array().unwrap();
    assert!(cases.len() > 40);
    let (mut none, mut some) = (0, 0);
    for k in cases {
        let lg = &ls[k["log"].as_u64().unwrap() as usize];
        let what = format!("fft_freq_centres journal {} réglage {}", k["log"], k["s"]);
        let got = fft_freq_centres(lg, opt_settings(k, &ss));
        assert!(!is_error(k), "{what}");
        match (&k["out"], got) {
            (Value::Null, None) => none += 1,
            (Value::Null, Some(m)) => panic!("{what} : Python rend None, Rust {}×{}", m.k, m.n),
            (_, None) => panic!("{what} : Python rend un tableau, Rust None"),
            (o, Some(m)) => {
                some += 1;
                let shape: Vec<usize> = o["shape"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap() as usize)
                    .collect();
                assert_eq!(vec![m.k, m.n], shape, "{what} : forme");
                assert_bits(&m.data, &f64s(o["x"].as_str().unwrap()), &what);
            }
        }
    }
    assert!(none > 5 && some > 20, "None : {none}, tableaux : {some}");
}

fn as_matrix(c: &Value, id: &Value, k: usize) -> Matrix64 {
    let data = input(c, id);
    Matrix64 {
        k,
        n: data.len() / k,
        data,
    }
}

#[test]
fn simulate_gyro_dans_la_tolerance() {
    let c = cas("bf_sim");
    let ss = all_settings(&c);
    let ls = logs(&c, &ss);
    let cases = c["simulate_gyro"].as_array().unwrap();
    assert!(cases.len() > 40);
    let mut errors = 0;
    for k in cases {
        let lg = &ls[k["log"].as_u64().unwrap() as usize];
        let what = format!("simulate_gyro journal {} (n={}) réglage {}", k["log"], lg.n(), k["s"]);
        let got = simulate_gyro(lg, opt_settings(k, &ss));
        if is_error(k) {
            errors += 1;
            assert!(got.is_err(), "{what} : Python lève une erreur");
        } else {
            let m = got.unwrap_or_else(|e| panic!("{what} : {e}"));
            assert_close(
                "simulate_gyro",
                &m.data,
                &f64s(k["out"].as_str().unwrap()),
                SIM_RTOL,
                &what,
            );
        }
    }
    assert!(
        errors >= 3,
        "gyro non filtré absent et réglages invalides doivent être couverts"
    );
    report();
}

#[test]
fn simulate_dterm_dans_la_tolerance() {
    let c = cas("bf_sim");
    let ss = all_settings(&c);
    let ls = logs(&c, &ss);
    let cases = c["simulate_dterm"].as_array().unwrap();
    assert!(cases.len() > 40);
    for k in cases {
        let lg = &ls[k["log"].as_u64().unwrap() as usize];
        let g = (!k["gyro"].is_null()).then(|| as_matrix(&c, &k["gyro"], 3));
        let what = format!(
            "simulate_dterm journal {} (n={}) réglage {} gyro fourni {}",
            k["log"],
            lg.n(),
            k["s"],
            g.is_some()
        );
        let got = simulate_dterm(lg, opt_settings(k, &ss), g.as_ref());
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève une erreur");
        } else {
            let m = got.unwrap_or_else(|e| panic!("{what} : {e}"));
            assert_close(
                "simulate_dterm",
                &m.data,
                &f64s(k["out"].as_str().unwrap()),
                SIM_RTOL,
                &what,
            );
        }
    }
    report();
}

#[test]
fn whatif_spectrum_dans_la_tolerance() {
    let c = cas("bf_sim");
    let ss = all_settings(&c);
    let ls = logs(&c, &ss);
    let cases = c["whatif_spectrum"].as_array().unwrap();
    assert!(cases.len() > 100);
    let mut caches: HashMap<i64, WhatIfCache> = HashMap::new();
    let (mut unchanged, mut hits) = (0, 0);
    for k in cases {
        let lg = &ls[k["log"].as_u64().unwrap() as usize];
        let (xl, xp) = (input(&c, &k["x"]), (!k["xp"].is_null()).then(|| input(&c, &k["xp"])));
        let sel = k["sel"][0].as_u64().unwrap() as usize..k["sel"][1].as_u64().unwrap() as usize;
        let (base, modf) = (
            &ss[k["base"].as_u64().unwrap() as usize],
            &ss[k["mod"].as_u64().unwrap() as usize],
        );
        let target = Target::parse(k["target"].as_str().unwrap());
        let what = format!(
            "whatif_spectrum journal {} sel {:?} {:?} fenêtre {} ms cache {} avant-filtres {}",
            k["log"],
            sel,
            target,
            k["seg_ms"],
            k["cache"],
            xp.is_some()
        );
        let group = k["cache"].as_i64();
        let mut local = WhatIfCache::default();
        let cache = match group {
            Some(g) => caches.entry(g).or_default(),
            None => &mut local,
        };
        let before = cache.gyro.len();
        let got = whatif_spectrum(
            &xl,
            xp.as_deref(),
            lg,
            sel,
            base,
            modf,
            target,
            k["seg_ms"].as_f64().unwrap(),
            cache,
        );
        if group.is_some() && cache.gyro.len() == before {
            hits += 1; // tout est venu du cache
        }
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève une erreur");
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : {e}"));
        let o = &k["out"];
        assert_eq!(
            got.freqs.len() as u64,
            o["n"].as_u64().unwrap(),
            "{what} : nombre de fréquences"
        );
        assert_bits(
            &got.freqs,
            &f64s(o["freqs"].as_str().unwrap()),
            &format!("{what} : fréquences"),
        );
        let (want_cur, want_mod) = (
            f64s(o["current_db"].as_str().unwrap()),
            f64s(o["modified_db"].as_str().unwrap()),
        );
        assert_close("whatif current (dB)", &got.current_db, &want_cur, WHATIF_RTOL, &what);
        assert_close("whatif modifié (dB)", &got.modified_db, &want_mod, WHATIF_RTOL, &what);
        // réglages inchangés : Python rend exactement le spectre du log (facteur 1) ; le port aussi
        if want_cur.len() == want_mod.len()
            && want_cur.iter().zip(&want_mod).all(|(a, b)| same(*a, *b))
            && !want_cur.is_empty()
        {
            unchanged += 1;
            assert_bits(
                &got.modified_db,
                &got.current_db,
                &format!("{what} : réglages inchangés"),
            );
        }
    }
    assert!(
        unchanged >= 3,
        "le cas « réglages inchangés » doit être couvert ({unchanged})"
    );
    assert!(
        hits >= 2,
        "les appels qui réutilisent un cache doivent être couverts ({hits})"
    );
    report();
}
