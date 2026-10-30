//! `analysis/step.py` (phase 4c-B(d)) porté en Rust : cas écrits par `scripts/step_cases.py` (régénérer `tests/data/step.json` avec ce script si
//! `step.py` change). `np.gradient`, `step_metrics`, `shape_metrics` (sommes dans l'ordre de numpy, pas de libm) : **identiques au bit près** ;
//! `window_starts` : identique ; `step_response` : réponses, moyenne, écart-type et métriques au bit près (le noyau de déconvolution est le même
//! des deux côtés : L-049, seule la colle est jugée ici), boucle ouverte à la tolérance mesurée (`log`, `atan2`, FFT : L-048).

use otb_analysis::open_loop::{self, LoopMetrics};
use otb_analysis::step::{self, LoopInput, StepMetrics, StepOptions};
use serde_json::Value;

mod common;
use common::*;

/// Boucle ouverte de `step_response` (`log`, `atan2`, FFT : L-048). Mesuré le 2026-10-30 : 1,9e-13 au pire (cohérence), 9,5e-14 sur `L` et `T`,
/// 4,9e-14 sur les écarts-types. Marge ×5 000.
const LOOP_RTOL: f64 = 1e-9;

fn metrics_of(v: &Value) -> StepMetrics {
    let g = |k: &str| f1(&v[k]);
    StepMetrics {
        peak: g("peak"),
        peak_ms: g("peak_ms"),
        latency_ms: g("latency_ms"),
        rise_ms: g("rise_ms"),
        settling_ms: g("settling_ms"),
        overshoot_pct: g("overshoot_pct"),
        ripple: g("ripple"),
        ripple_std: g("ripple_std"),
        ripple_ms: g("ripple_ms"),
        waves: g("waves"),
        waves_ratio: g("waves_ratio"),
    }
}

fn arr(m: &StepMetrics) -> [f64; 11] {
    [
        m.peak,
        m.peak_ms,
        m.latency_ms,
        m.rise_ms,
        m.settling_ms,
        m.overshoot_pct,
        m.ripple,
        m.ripple_std,
        m.ripple_ms,
        m.waves,
        m.waves_ratio,
    ]
}

/// Réponses, moyenne et écart-type après FFT : écart relatif au plus grand élément. Mesuré : 0 sous Linux, 1,8e-12 sous Windows (CI du 2026-10-30,
/// erreur de la première case seulement). Marge ×500.
const RESPONSE_RTOL: f64 = 1e-9;
/// Métriques tirées de ces réponses : écart relatif à max(|valeur|, 1) (ms, %, rapports). Les NaN doivent coïncider.
const METRICS_RTOL: f64 = 1e-6;

fn assert_metrics_close(got: &StepMetrics, want: &StepMetrics, what: &str) {
    let names = [
        "pic",
        "pic (ms)",
        "latence",
        "montée",
        "établissement",
        "dépassement",
        "cassure",
        "cassure (écart-type)",
        "cassure (ms)",
        "vagues",
        "vagues (rapport)",
    ];
    for ((n, g), w) in names.iter().zip(arr(got)).zip(arr(want)) {
        assert_close(
            "métrique de step_response",
            &[g, 1.0],
            &[w, 1.0],
            METRICS_RTOL,
            &format!("{what} : {n}"),
        );
    }
}

fn assert_metrics(got: &StepMetrics, want: &StepMetrics, what: &str) {
    assert_bits(&arr(got), &arr(want), what);
}

#[test]
fn gradient_conforme() {
    let c = cas("step");
    let (mut errors, mut done) = (0, 0);
    for k in c["gradient"].as_array().unwrap() {
        let what = format!("gradient {}", k["label"]);
        let got = step::gradient(&input(&c, &k["f"]), &input(&c, &k["x"]));
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
            continue;
        }
        assert_bits(
            &got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})")),
            &input(&c, &k["out"]),
            &what,
        );
        done += 1;
    }
    assert!(errors >= 2 && done >= 12, "{errors} {done}");
}

#[test]
fn step_metrics_conforme() {
    let c = cas("step");
    let (mut errors, mut done, mut with_peak, mut settled) = (0, 0, 0, 0);
    for k in c["metrics"].as_array().unwrap() {
        let what = format!("step_metrics {}", k["label"]);
        let got = step::step_metrics(&input(&c, &k["t"]), &input(&c, &k["r"]));
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let want = metrics_of(&k["out"]);
        assert_metrics(&got, &want, &what);
        done += 1;
        with_peak += want.peak.is_finite() as usize;
        settled += (want.settling_ms.is_finite() && want.settling_ms > 0.0) as usize;
    }
    assert!(
        errors <= 3 && done >= 45 && with_peak >= 30 && settled >= 10,
        "{errors} {done} {with_peak} {settled}"
    );
}

#[test]
fn shape_metrics_conforme() {
    let c = cas("step");
    let (mut errors, mut done, mut std_seen, mut waves_seen) = (0, 0, 0, 0);
    for k in c["shape"].as_array().unwrap() {
        let what = format!("shape_metrics {}", k["label"]);
        let t = input(&c, &k["t"]);
        let rows = k["rows"].as_u64().unwrap() as usize;
        let r = input(&c, &k["R"]);
        let mut m = metrics_of(&k["before"]);
        let got = step::shape_metrics(&t, &r, rows, &mut m);
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
            continue;
        }
        got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let want = metrics_of(&k["out"]);
        assert_metrics(&m, &want, &what);
        done += 1;
        std_seen += want.ripple_std.is_finite() as usize;
        waves_seen += want.waves_ratio.is_finite() as usize;
    }
    assert!(
        errors <= 3 && done >= 25 && std_seen >= 10 && waves_seen >= 10,
        "{errors} {done} {std_seen} {waves_seen}"
    );
}

#[test]
fn window_starts_conforme() {
    let c = cas("step");
    let (mut errors, mut nonempty) = (0, 0);
    for k in c["starts"].as_array().unwrap() {
        let what = format!("window_starts {}", k["label"]);
        let sp = input(&c, &k["sp"]);
        let (fs, seg, hop, mi) = (f1(&k["fs"]), f1(&k["seg_s"]), f1(&k["hop_ms"]), f1(&k["min_input"]));
        let ma = k["max_input"].as_str().map(|_| f1(&k["max_input"]));
        let breaks: Option<Vec<i64>> = k["breaks"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_i64().unwrap()).collect());
        let l = otb_analysis::spectral::py_round(seg * fs);
        let got = l.and_then(|l| step::window_starts(&sp, fs, l, hop, mi, ma, breaks.as_deref()));
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
        } else {
            let want: Vec<i64> = k["out"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect();
            assert_eq!(
                got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})")),
                want,
                "{what}"
            );
            nonempty += !want.is_empty() as usize;
        }
        let iw = step::input_windows(&sp, fs, seg, mi, hop, breaks.as_deref());
        if k["error_iw"].as_bool().unwrap() {
            assert!(iw.is_err(), "{what} : input_windows : Python lève, Rust rend {iw:?}");
        } else {
            assert_eq!(
                iw.unwrap_or_else(|e| panic!("{what} : input_windows : Rust lève ({e})")) as i64,
                k["iw"].as_i64().unwrap(),
                "{what} : input_windows"
            );
        }
    }
    assert!(errors >= 2 && nonempty >= 20, "{errors} {nonempty}");
}

fn check_loop(got: &LoopMetrics, o: &Value, c: &Value, what: &str) {
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
        &[got.band_hz.0, got.band_hz.1, got.bandwidth_hz, got.sensitivity_peak_hz],
        &[
            f1(&o["band"][0]),
            f1(&o["band"][1]),
            f1(&o["bandwidth"]),
            f1(&o["ms_hz"]),
        ],
        &format!("{what} : bande, bande passante, fréquence du pic"),
    );
    assert_eq!(
        got.sensitivity_peak_at_edge,
        o["edge"].as_bool().unwrap(),
        "{what} : bord"
    );
    let valid: Vec<bool> = o["valid"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_bool().unwrap())
        .collect();
    assert_eq!(got.valid, valid, "{what} : valid");
    assert_bits(&got.freqs, &input(c, &o["freqs"]), &format!("{what} : fréquences"));
    assert_close_cx("T (step)", &got.t, &cx(c, &o["T"]), LOOP_RTOL, what);
    assert_close_cx("L (step)", &got.l, &cx(c, &o["L"]), LOOP_RTOL, what);
    assert_close_rel(
        "cohérence (step)",
        &got.coherence,
        &input(c, &o["coh"]),
        LOOP_RTOL,
        what,
    );
    for (name, g, w) in [
        ("croisement (Hz)", got.crossover_hz, f1(&o["crossover"])),
        ("bande passante", got.bandwidth_hz, f1(&o["bandwidth"])),
    ] {
        assert_close(name, &[g], &[w], LOOP_RTOL, what);
    }
    for (name, g, w) in [
        ("marge phase (°)", got.phase_margin_deg, f1(&o["pm"])),
        ("marge gain (dB)", got.gain_margin_db, f1(&o["gm"])),
        ("pic sensibilité (dB)", got.sensitivity_peak_db, f1(&o["ms_db"])),
        ("pic de suivi (dB)", got.tracking_peak_db, f1(&o["tracking"])),
        ("d_ratio", got.d_ratio, f1(&o["d_ratio"])),
    ] {
        assert_close(name, &[g, 1.0], &[w, 1.0], LOOP_RTOL, what);
    }
    let want = o["std"].as_object().unwrap();
    assert_eq!(got.std.len(), want.len(), "{what} : écarts-types");
    for (key, v) in &got.std {
        assert_close(
            "écart-type jackknife (step)",
            &[*v, 1.0],
            &[f1(&want[*key]), 1.0],
            1e-8,
            what,
        );
    }
}

#[test]
fn step_response_conforme() {
    let c = cas("step");
    let (mut errors, mut empty, mut full, mut measured, mut approx) = (0, 0, 0, 0, 0);
    for k in c["response"].as_array().unwrap() {
        let what = format!("step_response {}", k["label"]);
        let (r, y) = (input(&c, &k["r"]), input(&c, &k["y"]));
        let (p, i, d, f) = (
            input(&c, &k["p"]),
            input(&c, &k["i"]),
            input(&c, &k["d"]),
            input(&c, &k["f"]),
        );
        let z = k["z"].as_str().map(|_| input(&c, &k["z"]));
        let note = k["note"].as_str().unwrap().to_string();
        let terms = match k["terms"].as_str().unwrap() {
            "none" => LoopInput::Absent,
            "note" => LoopInput::Unmeasurable(&note),
            _ => LoopInput::Terms(open_loop::Terms {
                p: &p,
                i: &i,
                d: &d,
                f: &f,
                instrument: z.as_deref(),
            }),
        };
        let o = StepOptions {
            seg_s: f1(&k["seg_s"]),
            min_input: f1(&k["min_input"]),
            max_input: k["max_input"].as_str().map(|_| f1(&k["max_input"])),
            resp_ms: f1(&k["resp_ms"]),
            hop_ms: f1(&k["hop_ms"]),
            reg: f1(&k["reg"]),
            smooth: k["smooth"].as_i64().unwrap(),
            y_correction: k["y_correction"].as_bool().unwrap(),
        };
        let breaks: Option<Vec<i64>> = k["breaks"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_i64().unwrap()).collect());
        let got = step::step_response(&r, &y, f1(&k["fs"]), &o, &terms, breaks.as_deref());
        if is_error(k) || o.smooth > 1 {
            assert!(
                got.is_err(),
                "{what} : Python lève (ou non porté), Rust rend un résultat"
            );
            errors += 1;
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        let w = &k["out"];
        assert_eq!(got.n as i64, w["n"].as_i64().unwrap(), "{what} : nombre de fenêtres");
        assert_bits(&got.t_ms, &input(&c, &w["t"]), &format!("{what} : t_ms"));
        // ⚠️ les réponses sortent d'une FFT (`realfft`) : identiques à Python sur ce poste, pas d'une plateforme à l'autre (CI Windows : 1,8e-12 ;
        // L-048, L-059). La consigne max (valeur absolue, sans FFT) reste exacte.
        assert_close(
            "réponses",
            &got.responses,
            &input(&c, &w["R"]),
            RESPONSE_RTOL,
            &format!("{what} : réponses"),
        );
        assert_close(
            "moyenne",
            &got.mean,
            &input(&c, &w["mean"]),
            RESPONSE_RTOL,
            &format!("{what} : moyenne"),
        );
        assert_close(
            "écart-type",
            &got.std,
            &input(&c, &w["std"]),
            RESPONSE_RTOL,
            &format!("{what} : écart-type"),
        );
        assert_bits(
            &got.max_inputs,
            &input(&c, &w["max_inputs"]),
            &format!("{what} : consignes max"),
        );
        assert_metrics_close(&got.metrics, &metrics_of(&w["metrics"]), &format!("{what} : métriques"));
        check_loop(&got.loop_, &w["lm"], &c, &what);
        if got.n == 0 {
            empty += 1;
        } else {
            full += 1;
            measured += (got.loop_.method == open_loop::MEASURED) as usize;
            approx += (got.loop_.method == open_loop::APPROX) as usize;
        }
    }
    assert!(
        errors >= 3 && empty >= 2 && full >= 20 && measured >= 10 && approx >= 5,
        "{errors} {empty} {full} {measured} {approx}"
    );
    report();
}
