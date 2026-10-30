//! `loop_from_terms` (boucle mesurée, repli, `d_ratio`, jackknife) de `analysis/loop.py` (étape 4c-B(c)) en Rust : cas écrits par
//! `scripts/loop_terms_cases.py` (boucles fermées simulées ; régénérer `tests/data/loop_terms.json` avec ce script si `loop.py` change).
//!
//! Exigé **identique** : les erreurs, la méthode retenue (mesurée ou repli), les notes, la bande (lue dans la grille), le masque `valid`,
//! la présence et le NaN de chaque écart-type. À une tolérance mesurée : les fonctions de transfert (FFT : L-048), les marges, `d_ratio`
//! et les écarts-types, qui en dépendent. Un saut d'indice du croisement, du pic ou d'un point valide dans l'un des 8 sous-ensembles du
//! jackknife change l'écart-type de plusieurs ordres de grandeur : la tolérance le refuse.

use otb_analysis::open_loop::{self, LoopMetrics, Terms};
use serde_json::Value;

mod common;
use common::*;

/// Fonctions de transfert, marges, `d_ratio` (spectres par FFT puis estimation). Mesuré le 2026-10-30 : 4,0e-13 au pire (cohérence),
/// 2,7e-13 sur `L` et `T`, 2e-15 sur les marges. Marge ×2 500.
const RTOL: f64 = 1e-9;
/// Écarts-types jackknife (différences entre 8 sous-ensembles : amplifient l'écart des marges). Mesuré : 4,4e-14. Marge ×200 000.
const STD_RTOL: f64 = 1e-8;

fn approx_of(k: &Value) -> LoopMetrics {
    if k["approx"].is_null() {
        return LoopMetrics::new(&[], &[], &[]);
    }
    // `loop_from_T` sur la grille fabriquée par le script : bande passante et pic de suivi réels, note déjà présente
    let fr: Vec<f64> = (0..61).map(|i| 2.0 * i as f64).collect();
    let t: Vec<(f64, f64)> = fr
        .iter()
        .map(|&f| {
            let d = 1.0 + (f / 35.0) * (f / 35.0);
            (1.0 / d, -(f / 35.0) / d)
        })
        .collect();
    let mut lm = open_loop::loop_from_t(&fr, &t, &vec![0.9; 61], 150.0, 0.5).unwrap();
    lm.notes.push("termes PID non fournis".into());
    lm
}

#[test]
fn loop_from_terms_conforme() {
    let c = cas("loop_terms");
    let (mut errors, mut measured, mut fallback, mut crossing, mut stds, mut dr) = (0, 0, 0, 0, 0, 0);
    for k in c["cases"].as_array().unwrap() {
        let what = format!("loop_from_terms {}", k["label"]);
        let (r, y) = (input(&c, &k["r"]), input(&c, &k["y"]));
        let (p, i, d, f) = (
            input(&c, &k["p"]),
            input(&c, &k["i"]),
            input(&c, &k["d"]),
            input(&c, &k["f"]),
        );
        let z = k["z"].as_str().map(|_| input(&c, &k["z"]));
        let breaks: Option<Vec<i64>> = k["breaks"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_i64().unwrap()).collect());
        let max_input = k["max_input"].as_str().map(|_| f1(&k["max_input"]));
        let terms = Terms {
            p: &p,
            i: &i,
            d: &d,
            f: &f,
            instrument: z.as_deref(),
        };
        let got = open_loop::loop_from_terms(
            &r,
            &y,
            &terms,
            f1(&k["fs"]),
            approx_of(k),
            f1(&k["seg_s"]),
            f1(&k["min_input"]),
            max_input,
            breaks.as_deref(),
        );
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend un résultat");
            errors += 1;
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let o = &k["out"];
        assert_eq!(got.method, o["method"].as_str().unwrap(), "{what} : méthode");
        let notes: Vec<&str> = o["notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            got.notes.iter().map(String::as_str).collect::<Vec<_>>(),
            notes,
            "{what} : notes"
        );
        assert_bits(
            &[got.band_hz.0, got.band_hz.1, got.bandwidth_hz],
            &[f1(&o["band"][0]), f1(&o["band"][1]), f1(&o["bandwidth"])],
            &format!("{what} : bande et bande passante"),
        );
        assert_close(
            "pic de suivi",
            &[got.tracking_peak_db, 1.0],
            &[f1(&o["tracking"]), 1.0],
            1e-11,
            &what,
        );
        let valid: Vec<bool> = o["valid"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_bool().unwrap())
            .collect();
        assert_eq!(got.valid, valid, "{what} : masque valid");
        assert_eq!(
            got.sensitivity_peak_at_edge,
            o["edge"].as_bool().unwrap(),
            "{what} : bord"
        );
        assert_bits(
            &[got.sensitivity_peak_hz],
            &[f1(&o["ms_hz"])],
            &format!("{what} : fréquence du pic"),
        );
        if got.method == open_loop::APPROX {
            fallback += 1;
            continue;
        }
        measured += 1;
        assert_bits(&got.freqs, &input(&c, &o["freqs"]), &format!("{what} : fréquences"));
        assert_close_cx("T (mesurée)", &got.t, &cx(&c, &o["T"]), RTOL, &what);
        assert_close_cx("L (mesurée)", &got.l, &cx(&c, &o["L"]), RTOL, &what);
        assert_close_rel("cohérence zy", &got.coherence, &input(&c, &o["coh"]), RTOL, &what);
        assert_close(
            "croisement (Hz)",
            &[got.crossover_hz],
            &[f1(&o["crossover"])],
            RTOL,
            &what,
        );
        assert_close(
            "marge phase (°)",
            &[got.phase_margin_deg, 180.0],
            &[f1(&o["pm"]), 180.0],
            RTOL,
            &what,
        );
        assert_close(
            "marge gain (dB)",
            &[got.gain_margin_db, 1.0],
            &[f1(&o["gm"]), 1.0],
            RTOL,
            &what,
        );
        assert_close(
            "pic sensibilité (dB)",
            &[got.sensitivity_peak_db, 1.0],
            &[f1(&o["ms_db"]), 1.0],
            RTOL,
            &what,
        );
        assert_close("d_ratio", &[got.d_ratio, 1.0], &[f1(&o["d_ratio"]), 1.0], RTOL, &what);
        crossing += got.crossover_hz.is_finite() as usize;
        dr += got.d_ratio.is_finite() as usize;
        // jackknife : mêmes clés, dans l'ordre, NaN aux mêmes endroits
        let want = o["std"].as_object().unwrap();
        assert_eq!(got.std.len(), want.len(), "{what} : nombre d'écarts-types");
        for (key, v) in &got.std {
            let w = f1(&want[*key]);
            assert_close(
                "écart-type jackknife",
                &[*v, 1.0],
                &[w, 1.0],
                STD_RTOL,
                &format!("{what} : {key}"),
            );
            stds += v.is_finite() as usize;
        }
    }
    assert!(
        errors >= 1 && measured >= 40 && fallback >= 5 && crossing >= 25 && stds >= 80 && dr >= 25,
        "{errors} {measured} {fallback} {crossing} {stds} {dr}"
    );
    report();
}
