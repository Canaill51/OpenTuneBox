//! `analysis/loop.py` (étape 4c-B(a) : `margins`, `loop_from_T`, `dterm_model`) porté en Rust : cas écrits par
//! `scripts/loop_cases.py` (régénérer `tests/data/loop.json` avec ce script si `loop.py` change).
//!
//! Exigé **identique** : les erreurs (Python lève ⇔ Rust lève), les NaN, les indicateurs (`sensitivity_peak_at_edge`, masque `valid`,
//! méthode), les valeurs lues dans la grille de fréquences (`sensitivity_peak_hz`, bande, bande passante). À une tolérance mesurée
//! (L-048, L-051) : les valeurs qui passent par `log`, `atan2`, `exp`, `cos` et les produits complexes. Un saut d'indice (croisement,
//! passage par −180°, pic) donnerait un écart de l'ordre d'un pas de grille, très au-dessus de la tolérance.

use otb_analysis::open_loop::{self, C, LoopMetrics, Margins};
use serde_json::Value;

mod common;
use common::*;

/// Tolérance sur les valeurs continues (marges, fonctions de transfert). Mesuré le 2026-10-30 : 2,0e-15 au plus (`log`, `atan2`, `exp`) ;
/// `L` et `dterm_model` identiques au bit près. Marge ×5 000 pour une autre libm (Windows : L-048).
const RTOL: f64 = 1e-11;

fn cx(c: &Value, v: &Value) -> Vec<C> {
    let (re, im) = (input(c, &v["re"]), input(c, &v["im"]));
    re.into_iter().zip(im).collect()
}

/// Écart relatif de chaque élément complexe : `|g − w| / |w|` (plancher : `CELL_FLOOR` fois le plus grand module de la référence).
fn assert_close_cx(cat: &'static str, got: &[C], want: &[C], rtol: f64, what: &str) {
    assert_eq!(got.len(), want.len(), "{what} : longueur");
    let flat = |z: &[C]| -> Vec<f64> { z.iter().flat_map(|c| [c.0, c.1]).collect() };
    // NaN et infinis aux mêmes endroits (partie réelle et imaginaire)
    let (g, w) = (flat(got), flat(want));
    let top = want
        .iter()
        .filter(|c| c.0.is_finite() && c.1.is_finite())
        .fold(0.0f64, |m, c| m.max(c.0.hypot(c.1)));
    let mut worst = 0.0f64;
    for i in 0..got.len() {
        let (a, b) = (got[i], want[i]);
        if b.0.is_finite() && b.1.is_finite() && a.0.is_finite() && a.1.is_finite() {
            let d = (a.0 - b.0).hypot(a.1 - b.1);
            worst = worst.max(d / (b.0.hypot(b.1) + CELL_FLOOR * top).max(1e-300));
        } else {
            for k in [2 * i, 2 * i + 1] {
                assert!(same(g[k], w[k]), "{what} : élément {i} : Rust {a:?} / Python {b:?}");
            }
        }
    }
    assert_close(cat, &[worst, 1.0], &[0.0, 1.0], rtol, what);
}

fn margins_of(k: &Value) -> Margins {
    let o = &k["out"];
    Margins {
        crossover_hz: f1(&o["crossover"]),
        phase_margin_deg: f1(&o["pm"]),
        gain_margin_db: f1(&o["gm"]),
        phase_crossover_hz: f1(&o["pcross"]),
        sensitivity_peak_db: f1(&o["ms_db"]),
        sensitivity_peak_hz: f1(&o["ms_hz"]),
        sensitivity_peak_at_edge: o["edge"].as_bool().unwrap(),
    }
}

/// Compare les marges ; `sentinel` ajoute une échelle absolue aux angles et aux dB (un résultat proche de zéro n'a pas d'écart relatif).
fn check_margins(got: &Margins, want: &Margins, what: &str) {
    assert_eq!(
        got.sensitivity_peak_at_edge, want.sensitivity_peak_at_edge,
        "{what} : pic au bord"
    );
    assert_bits(
        &[got.sensitivity_peak_hz],
        &[want.sensitivity_peak_hz],
        &format!("{what} : fréquence du pic"),
    );
    assert_close("croisement (Hz)", &[got.crossover_hz], &[want.crossover_hz], RTOL, what);
    assert_close(
        "croisement phase (Hz)",
        &[got.phase_crossover_hz],
        &[want.phase_crossover_hz],
        RTOL,
        what,
    );
    assert_close(
        "marge phase (°)",
        &[got.phase_margin_deg, 180.0],
        &[want.phase_margin_deg, 180.0],
        RTOL,
        what,
    );
    assert_close(
        "marge gain (dB)",
        &[got.gain_margin_db, 1.0],
        &[want.gain_margin_db, 1.0],
        RTOL,
        what,
    );
    assert_close(
        "pic sensibilité (dB)",
        &[got.sensitivity_peak_db, 1.0],
        &[want.sensitivity_peak_db, 1.0],
        RTOL,
        what,
    );
}

#[test]
fn margins_conforme() {
    let c = cas("loop");
    let (mut errors, mut crossing, mut gain, mut edge, mut nan) = (0, 0, 0, 0, 0);
    for k in c["margins"].as_array().unwrap() {
        let what = format!("margins {}", k["label"]);
        let f = input(&c, &k["f"]);
        let l = cx(&c, &k["L"]);
        let valid: Option<Vec<bool>> = k["valid"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_bool().unwrap()).collect());
        let got = open_loop::margins(&f, &l, valid.as_deref());
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let want = margins_of(k);
        check_margins(&got, &want, &what);
        crossing += want.crossover_hz.is_finite() as usize;
        gain += want.gain_margin_db.is_finite() as usize;
        edge += want.sensitivity_peak_at_edge as usize;
        nan += want.sensitivity_peak_db.is_nan() as usize;
    }
    assert!(
        errors <= 3 && crossing >= 40 && gain >= 30 && edge >= 5 && nan >= 4,
        "{errors} {crossing} {gain} {edge} {nan}"
    );
    report();
}

fn lm_check(got: &LoopMetrics, c: &Value, k: &Value, what: &str) {
    let o = &k["out"];
    assert_eq!(got.method, o["method"].as_str().unwrap(), "{what} : méthode");
    assert_eq!(got.notes.len(), o["notes"].as_array().unwrap().len(), "{what} : notes");
    assert_bits(
        &[got.band_hz.0, got.band_hz.1, got.bandwidth_hz],
        &[f1(&o["band"][0]), f1(&o["band"][1]), f1(&o["bandwidth"])],
        &format!("{what} : bande et bande passante (lues dans la grille)"),
    );
    check_margins(
        &Margins {
            crossover_hz: got.crossover_hz,
            phase_margin_deg: got.phase_margin_deg,
            gain_margin_db: got.gain_margin_db,
            phase_crossover_hz: f64::NAN,
            sensitivity_peak_db: got.sensitivity_peak_db,
            sensitivity_peak_hz: got.sensitivity_peak_hz,
            sensitivity_peak_at_edge: got.sensitivity_peak_at_edge,
        },
        &Margins {
            crossover_hz: f1(&o["crossover"]),
            phase_margin_deg: f1(&o["pm"]),
            gain_margin_db: f1(&o["gm"]),
            phase_crossover_hz: f64::NAN,
            sensitivity_peak_db: f1(&o["ms_db"]),
            sensitivity_peak_hz: f1(&o["ms_hz"]),
            sensitivity_peak_at_edge: o["edge"].as_bool().unwrap(),
        },
        what,
    );
    assert_bits(
        &[got.tracking_peak_db],
        &[got.sensitivity_peak_db],
        &format!("{what} : pic de suivi = pic de sensibilité"),
    );
    assert_close(
        "pic de suivi (dB)",
        &[got.tracking_peak_db, 1.0],
        &[f1(&o["tracking"]), 1.0],
        RTOL,
        what,
    );
    let valid: Vec<bool> = o["valid"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_bool().unwrap())
        .collect();
    assert_eq!(got.valid, valid, "{what} : masque valid");
    assert_close_cx("L (module relatif)", &got.l, &cx(c, &o["L"]), RTOL, what);
}

#[test]
fn loop_from_t_conforme() {
    let c = cas("loop");
    let (mut errors, mut full, mut empty, mut bw) = (0, 0, 0, 0);
    for k in c["from_t"].as_array().unwrap() {
        let what = format!("loop_from_T {}", k["label"]);
        let (f, t, coh) = (input(&c, &k["f"]), cx(&c, &k["T"]), input(&c, &k["coh"]));
        let got = open_loop::loop_from_t(&f, &t, &coh, f1(&k["f_max"]), f1(&k["min_coh"]));
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let want_valid = k["out"]["valid"].as_array().unwrap().len();
        if want_valid == 0 {
            empty += 1;
            assert!(got.valid.is_empty(), "{what} : valid doit rester vide");
            assert!(got.sensitivity_peak_db.is_nan(), "{what}");
        } else {
            lm_check(&got, &c, k, &what);
            full += 1;
            bw += got.bandwidth_hz.is_finite() as usize;
        }
    }
    assert!(
        errors <= 3 && full >= 40 && empty >= 8 && bw >= 30,
        "{errors} {full} {empty} {bw}"
    );
    report();
}

#[test]
fn dterm_model_conforme() {
    let c = cas("loop");
    let settings: Vec<_> = c["settings"].as_array().unwrap().iter().map(settings).collect();
    let (mut errors, mut done) = (0, 0);
    for k in c["dterm"].as_array().unwrap() {
        let what = format!("dterm_model réglage {} gain {}", k["s"], f1(&k["d"]));
        let f = input(&c, &k["f"]);
        let got = open_loop::dterm_model(
            &settings[k["s"].as_u64().unwrap() as usize],
            &f,
            f1(&k["d"]),
            f1(&k["thr"]),
        );
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        assert_close_cx("dterm_model", &got, &cx(&c, &k["out"]), RTOL, &what);
        done += 1;
    }
    assert!(errors <= 40 && done >= 300, "{errors} {done}");
    report();
}
