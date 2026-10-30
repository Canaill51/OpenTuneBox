//! `bf_filters` (chaînes et réponses) rend ce que `analysis/bf_filters.py` rend : cas écrits par
//! `scripts/bf_filters_cases.py` (régénérer `tests/data/bf_filters.json` avec ce script si `bf_filters.py` change).
//!
//! Exigé **identique au bit près** : chaînes de filtres (listes de filtres et cadences), `dyn_lpf_cutoff`, `scale_lowpass`,
//! `lowpass_off`, `rpm_notch_freq_weight`, notch dynamique : arithmétique pure, arrondis et entiers, sans bibliothèque
//! mathématique. À une tolérance mesurée (L-048) : `coefficients` (sin/cos de la libm, `np.convolve` par BLAS), la réponse
//! (sin/cos) et le retard (en plus `atan2`, dont l'écart est amplifié par la différence de phase sur 0,02 Hz).

use otb_analysis::bf_filters::*;
use serde_json::Value;

mod common;
use common::*;

/// Mesuré : 0 (mêmes bits que Python sur ce poste). La marge couvre `sin` / `cos` d'une autre libm (≈ 1 ulp) et un `ddot`
/// de BLAS fusionné en FMA (pas la même somme, ≈ 1 ulp).
const COEF_RTOL: f64 = 1e-14;
/// Mesuré : 0 (Python appelle le même noyau Rust). Une autre libm change `sin` / `cos` d'≈ 1 ulp par étage, sur une
/// trentaine d'étages au plus.
const RESP_RTOL: f64 = 1e-12;
/// Mesuré : 2,2e-13. L'écart de `atan2` (≈ 1 ulp d'un angle jusqu'à π, 4e-16) est divisé par `2π · 2 · df` (≈ 0,13 pour
/// 10 Hz) : ≈ 3e-15 en valeur relative pour un retard de 1 ms ; marge large, un vrai défaut (signe, saut de phase oublié)
/// fait de l'ordre de 1.
const DELAY_RTOL: f64 = 1e-10;

fn spec(v: &Value) -> FilterSpec {
    FilterSpec::with(
        Kind::parse(v["k"].as_str().unwrap()).unwrap(),
        f1(&v["f"]),
        f1(&v["q"]),
        f1(&v["w"]),
    )
}

fn chain(v: &Value) -> Chain {
    v.as_array().unwrap().iter().map(|e| (spec(&e[0]), f1(&e[1]))).collect()
}

fn assert_chain(got: &Chain, want: &Value, what: &str) {
    let want = chain(want);
    assert_eq!(got.len(), want.len(), "{what} : nombre de filtres");
    for (i, ((g, gr), (w, wr))) in got.iter().zip(&want).enumerate() {
        assert_eq!(g.kind, w.kind, "{what} : filtre {i} : type");
        assert_bits(
            &[g.f_hz, g.q, g.weight, *gr],
            &[w.f_hz, w.q, w.weight, *wr],
            &format!("{what} : filtre {i} (f, q, poids, cadence)"),
        );
    }
}

#[test]
fn coefficients_dans_la_tolerance() {
    let c = cas("bf_filters");
    let cases = c["coefficients"].as_array().unwrap();
    assert!(cases.len() > 700);
    for k in cases {
        let (sp, fs) = (spec(&k["spec"]), f1(&k["fs"]));
        let what = format!(
            "coefficients {:?} f={} q={} w={} fs={fs}",
            sp.kind, sp.f_hz, sp.q, sp.weight
        );
        let (b, a) = coefficients(&sp, fs);
        assert!(!is_error(k));
        assert_close("coefficients b", &b, &f64s(k["b"].as_str().unwrap()), COEF_RTOL, &what);
        assert_close("coefficients a", &a, &f64s(k["a"].as_str().unwrap()), COEF_RTOL, &what);
    }
    report();
}

#[test]
fn chain_response_dans_la_tolerance() {
    let c = cas("bf_filters");
    let cases = c["chain_response"].as_array().unwrap();
    assert!(cases.len() > 700);
    for k in cases {
        let (ch, fr) = (chain(&k["chain"]), input(&c, &k["freqs"]));
        let what = format!("chain_response {} filtres, {} fréquences", ch.len(), fr.len());
        let (re, im) = chain_response(&ch, &fr).unwrap_or_else(|e| panic!("{what} : {e}"));
        let inter: Vec<f64> = re.iter().zip(&im).flat_map(|(r, i)| [*r, *i]).collect();
        assert_close(
            "chain_response",
            &inter,
            &f64s(k["out"].as_str().unwrap()),
            RESP_RTOL,
            &what,
        );
    }
    report();
}

#[test]
fn chain_delay_ms_dans_la_tolerance() {
    let c = cas("bf_filters");
    let cases = c["chain_delay_ms"].as_array().unwrap();
    assert!(cases.len() > 200);
    for k in cases {
        if k.get("chain_no_rate").is_some() {
            // un FilterSpec sans cadence : `as_chain` lève l'erreur de Python
            let items = k["chain_no_rate"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| ChainItem::Spec(spec(s)))
                .collect();
            assert_eq!(is_error(k), as_chain(items, None).is_err());
            continue;
        }
        let (ch, f_hz) = (chain(&k["chain"]), f1(&k["f_hz"]));
        let what = format!("chain_delay_ms {} filtres, f={f_hz}", ch.len());
        let got = chain_delay_ms(&ch, f_hz);
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève une erreur");
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : {e}"));
        let want = hexes(&k["out"]);
        assert_close(
            "chain_delay_ms",
            &[got.group_ms, got.phase_ms],
            &want,
            DELAY_RTOL,
            &what,
        );
    }
    report();
}

#[test]
fn dyn_lpf_cutoff_identique() {
    let c = cas("bf_filters");
    for k in c["dyn_lpf_cutoff"].as_array().unwrap() {
        let (mn, mx, ex) = (
            k["min"].as_f64().unwrap(),
            k["max"].as_f64().unwrap(),
            k["expo"].as_f64().unwrap(),
        );
        let what = format!("dyn_lpf_cutoff {mn} {mx} {ex}");
        if k["arr"].as_bool().unwrap() {
            let t = input(&c, &k["t"]);
            let got: Vec<f64> = t.iter().map(|&x| dyn_lpf_cutoff(mn, mx, ex, x)).collect();
            assert_bits(&got, &f64s(k["out"].as_str().unwrap()), &what);
        } else {
            let t = f1(&k["t"]);
            assert_bits(
                &[dyn_lpf_cutoff(mn, mx, ex, t)],
                &[f1(&k["out"])],
                &format!("{what} gaz {t}"),
            );
        }
    }
}

fn lp_tuple(l: &otb_log::settings::Lowpass) -> (&'static str, i64, i64, i64, i64) {
    (l.kind, l.static_hz, l.dyn_min_hz, l.dyn_max_hz, l.expo)
}

#[test]
fn scale_lowpass_et_lowpass_off_identiques() {
    let c = cas("bf_filters");
    let cases = c["scale_lowpass"].as_array().unwrap();
    assert!(cases.len() > 150);
    for k in cases {
        let (lp, factor) = (lowpass(&k["lp"]), f1(&k["factor"]));
        let got = scale_lowpass(&lp, factor);
        if is_error(k) {
            assert!(got.is_err(), "scale_lowpass {lp:?} × {factor} : Python lève une erreur");
        } else {
            assert_eq!(
                lp_tuple(&got.unwrap()),
                lp_tuple(&lowpass(&k["out"])),
                "scale_lowpass {lp:?} × {factor}"
            );
        }
    }
    for k in c["lowpass_off"].as_array().unwrap() {
        assert_eq!(
            lp_tuple(&lowpass_off(&lowpass(&k["lp"]))),
            lp_tuple(&lowpass(&k["out"]))
        );
    }
}

#[test]
fn rpm_notch_freq_weight_identique() {
    let c = cas("bf_filters");
    let ss = all_settings(&c);
    let cases = c["rpm_notch_freq_weight"].as_array().unwrap();
    assert!(cases.len() > 500);
    for k in cases {
        let (m, h, i, rate) = (
            input(&c, &k["motor"]),
            k["harmonic"].as_i64().unwrap(),
            k["s"].as_u64().unwrap() as usize,
            f1(&k["rate"]),
        );
        let what = format!("rpm_notch_freq_weight harmonique {h} réglage {i} cadence {rate}");
        let got = rpm_notch_freq_weight(&m, h, &ss[i], rate);
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève une erreur");
            continue;
        }
        let (f, w) = got.unwrap_or_else(|e| panic!("{what} : {e}"));
        assert_bits(&f, &f64s(k["f"].as_str().unwrap()), &format!("{what} : fréquences"));
        assert_bits(&w, &f64s(k["w"].as_str().unwrap()), &format!("{what} : poids"));
    }
}

#[test]
fn notch_dynamique_identique() {
    let c = cas("bf_filters");
    let ss = all_settings(&c);
    for (k, s) in c["dyn_notch_enabled"].as_array().unwrap().iter().zip(&ss) {
        assert_eq!(
            dyn_notch_enabled(s),
            k["out"].as_bool().unwrap(),
            "dyn_notch_enabled {s:?}"
        );
    }
    for (k, s) in c["dyn_notch_initial_centres"].as_array().unwrap().iter().zip(&ss) {
        assert_bits(
            &dyn_notch_initial_centres(s),
            &hexes(&k["out"]),
            &format!("centres initiaux {s:?}"),
        );
    }
}

#[test]
fn gyro_chain_identique() {
    let c = cas("bf_filters");
    let ss = all_settings(&c);
    let cases = c["gyro_chain"].as_array().unwrap();
    assert!(cases.len() > 1500);
    let mut errors = 0;
    for k in cases {
        let (i, t) = (k["s"].as_u64().unwrap() as usize, f1(&k["t"]));
        let motor = (!k["motor"].is_null()).then(|| hexes(&k["motor"]));
        let dynh = (!k["dyn"].is_null()).then(|| hexes(&k["dyn"]));
        let what = format!("gyro_chain réglage {i} gaz {t} moteurs {motor:?} notchs dynamiques {dynh:?}");
        let got = gyro_chain(&ss[i], t, motor.as_deref(), dynh.as_deref());
        if is_error(k) {
            errors += 1;
            assert!(got.is_err(), "{what} : Python lève une erreur");
        } else {
            assert_chain(&got.unwrap_or_else(|e| panic!("{what} : {e}")), &k["out"], &what);
        }
    }
    assert!(errors > 50, "les réglages à division par zéro doivent être couverts");
}

#[test]
fn dterm_chain_identique() {
    let c = cas("bf_filters");
    let ss = all_settings(&c);
    for k in c["dterm_chain"].as_array().unwrap() {
        let (i, t) = (k["s"].as_u64().unwrap() as usize, f1(&k["t"]));
        assert_chain(
            &dterm_chain(&ss[i], t),
            &k["out"],
            &format!("dterm_chain réglage {i} gaz {t}"),
        );
    }
}
