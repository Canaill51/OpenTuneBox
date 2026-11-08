//! `analysis/delay.py` porté en Rust : cas écrits par `scripts/delay_cases.py` (régénérer `tests/data/delay.json` avec ce script si
//! `delay.py` change).
//!
//! Exigé **identique** : les erreurs (Python lève ⇔ Rust lève), les NaN, le centile de la consigne (`float32` pur), l'indicateur
//! `moving`. À une tolérance mesurée (L-048, L-051) : les retards, qui passent par une FFT (`xcorr_full`, `rfft`), `hypot` et `atan2`.
//! Un écart de plus d'un dernier bit sur le pic d'intercorrélation peut faire changer son indice : le retard saute alors d'un
//! échantillon, c'est ce que la tolérance doit refuser.

use otb_analysis::delay::{self, Delays};
use serde_json::Value;

mod common;
use common::*;

/// Intercorrélation (FFT de `realfft`, rapport des trois points du pic) : écart en périodes d'échantillonnage. Mesuré le 2026-10-30 :
/// 4,4e-15. Marge ×2 000 pour une autre libm ou un autre FFT (L-048).
const XCORR_RTOL: f64 = 1e-11;
/// Retard par la phase (`hypot`, `atan2`, produits complexes en FMA : L-051), écart relatif. Mesuré : 6,3e-16 (phase seule),
/// 1,3e-15 (`compute_delays`). Marge pour un processeur sans FMA, où les spectres croisés diffèrent jusqu'à 6e-14 (banc 4c-0).
const PHASE_RTOL: f64 = 1e-10;

/// Écart relatif à la référence (NaN et infinis aux mêmes endroits).
fn close1(cat: &'static str, got: f64, want: f64, rtol: f64, what: &str) {
    assert_close(cat, &[got], &[want], rtol, what);
}

/// Écart d'un retard d'intercorrélation **en périodes d'échantillonnage** (`1000 / fs` ms) : pour un vrai retard nul, Python et Rust
/// rendent chacun du bruit d'arrondi (≈ 1e-19 ms) dont l'écart relatif ne veut rien dire. Le `1.0` ajouté aux deux tableaux fixe le
/// plus grand élément de la référence à au moins une période : l'écart rapporté est alors absolu, en périodes.
fn close_samples(cat: &'static str, got: f64, want: f64, fs: f64, rtol: f64, what: &str) {
    let u = fs / 1000.0;
    assert_close(cat, &[got * u, 1.0], &[want * u, 1.0], rtol, what);
}

#[test]
fn xcorr_delay_ms_conforme() {
    let c = cas("delay");
    let (mut errors, mut nan, mut finite, mut unported) = (0, 0, 0, 0);
    for k in c["xcorr"].as_array().unwrap() {
        let what = format!("xcorr {}", k["label"]);
        let (x, y) = (input(&c, &k["x"]), input(&c, &k["y"]));
        // Le lissage n'existe plus (W-024, 2026-11-08) : les cas de référence qui le demandaient sont hors de portée
        // de la fonction. Ils restent comptés, pour qu'un fichier de cas appauvri ne passe pas inaperçu.
        if k["smooth"].as_i64().unwrap() > 1 {
            assert!(
                !k["ported"].as_bool().unwrap(),
                "{what} : un cas lissé ne peut pas être porté"
            );
            unported += 1;
            continue;
        }
        let got = delay::xcorr_delay_ms(&x, &y, f1(&k["fs"]), f1(&k["max_ms"]));
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
            continue;
        }
        let g = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let w = f1(&k["out"]);
        if w.is_nan() {
            nan += 1;
        } else {
            finite += 1;
        }
        close_samples("xcorr (périodes)", g, w, f1(&k["fs"]), XCORR_RTOL, &what);
    }
    assert!(
        errors >= 2 && nan >= 5 && finite >= 40 && unported >= 4,
        "{errors} {nan} {finite} {unported}"
    );
    report();
}

#[test]
fn phase_delay_ms_conforme() {
    let c = cas("delay");
    let (mut errors, mut nan, mut finite) = (0, 0, 0);
    for k in c["phase"].as_array().unwrap() {
        let what = format!("phase {}", k["label"]);
        let (x, y) = (input(&c, &k["x"]), input(&c, &k["y"]));
        let breaks: Option<Vec<i64>> = k["breaks"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_i64().unwrap()).collect());
        let band = (f1(&k["band"][0]), f1(&k["band"][1]));
        let got = delay::phase_delay_ms(&x, &y, f1(&k["fs"]), band, f1(&k["coh"]), breaks.as_deref());
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
            continue;
        }
        let g = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let w = f1(&k["out"]);
        if w.is_nan() {
            nan += 1;
        } else {
            finite += 1;
        }
        close1("phase", g, w, PHASE_RTOL, &what);
    }
    assert!(errors >= 4 && nan >= 8 && finite >= 30, "{errors} {nan} {finite}");
    report();
}

#[test]
fn stick_p95_dps_identique() {
    let c = cas("delay");
    let logs: Vec<_> = c["logs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| flight_log(&c, v))
        .collect();
    let (mut errors, mut ok) = (0, 0);
    for k in c["stick"].as_array().unwrap() {
        let lg = &logs[k["log"].as_u64().unwrap() as usize];
        let what = format!("stick {} {}", c["names"][k["log"].as_u64().unwrap() as usize], k["sel"]);
        let sel = match k["sel"].as_array() {
            None => 0..lg.n(),
            Some(s) => s[0].as_u64().unwrap() as usize..s[1].as_u64().unwrap() as usize,
        };
        let got = delay::stick_p95_dps(lg, sel);
        if is_error(k) {
            assert!(got.is_err(), "{what}");
            errors += 1;
        } else {
            assert!(same(got.unwrap(), f1(&k["out"])), "{what}");
            ok += 1;
        }
    }
    assert!(errors > 10 && ok > 30, "{errors} {ok}");
}

fn delays_eq(got: &Delays, want: &Value, fs: f64, what: &str) {
    close1(
        "compute gyro",
        got.gyro_ms,
        f1(&want["gyro"]),
        PHASE_RTOL,
        &format!("{what} gyro"),
    );
    close1(
        "compute dterm",
        got.dterm_ms,
        f1(&want["dterm"]),
        PHASE_RTOL,
        &format!("{what} terme D"),
    );
    for a in 0..3 {
        close_samples(
            "compute sp (périodes)",
            got.sp_gyro_ms[a],
            f1(&want["sp"][a]),
            fs,
            XCORR_RTOL,
            &format!("{what} consigne→gyro {a}"),
        );
    }
    assert!(same(got.stick_p95_dps, f1(&want["stick"])), "{what} p95");
    assert_eq!(got.moving(), want["moving"].as_bool().unwrap(), "{what} moving");
}

#[test]
fn compute_delays_conforme() {
    let c = cas("delay");
    let logs: Vec<_> = c["logs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| flight_log(&c, v))
        .collect();
    let (mut errors, mut ok, mut gyro_nan, mut dterm_nan, mut sp_nan) = (0, 0, 0, 0, 0);
    for k in c["compute"].as_array().unwrap() {
        let lg = &logs[k["log"].as_u64().unwrap() as usize];
        let what = format!(
            "compute {} {}",
            c["names"][k["log"].as_u64().unwrap() as usize],
            k["sel"]
        );
        let sel = match k["sel"].as_array() {
            None => 0..lg.n(),
            Some(s) => s[0].as_u64().unwrap() as usize..s[1].as_u64().unwrap() as usize,
        };
        let got = delay::compute_delays(lg, sel);
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
            continue;
        }
        let g = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        delays_eq(&g, &k["out"], lg.fs, &what);
        ok += 1;
        gyro_nan += usize::from(g.gyro_ms.is_nan());
        dterm_nan += usize::from(g.dterm_ms.is_nan());
        sp_nan += g.sp_gyro_ms.iter().filter(|v| v.is_nan()).count();
    }
    assert!(errors > 10 && ok > 30, "{errors} {ok}");
    assert!(
        gyro_nan > 3 && dterm_nan > 3 && sp_nan > 3,
        "{gyro_nan} {dterm_nan} {sp_nan}"
    );
    report();
}

#[test]
fn unwrap_identique() {
    let c = cas("delay");
    let mut n = 0;
    for k in c["unwrap"].as_array().unwrap() {
        let what = format!("unwrap {}", k["label"]);
        assert_bits(
            &otb_analysis::npstat::unwrap(&input(&c, &k["in"])),
            &input(&c, &k["out"]),
            &what,
        );
        n += 1;
    }
    assert!(n >= 15);
}
