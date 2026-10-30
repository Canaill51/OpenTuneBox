//! `loop_spectra` et `estimate` de `analysis/loop.py` (étape 4c-B(b)) en Rust : cas écrits par `scripts/loop_spectra_cases.py`
//! (régénérer `tests/data/loop_spectra.json` avec ce script si `loop.py` change).
//!
//! Exigé **identique** : les erreurs, les « pas assez de fenêtres » (`None`), le nombre de fenêtres retenues, `has_d`, les fréquences,
//! le masque `valid`. À une tolérance mesurée : les spectres croisés (FFT de `realfft` contre pocketfft de numpy : L-048) et les fonctions
//! de transfert qui en découlent (divisions de Smith, produits en FMA : L-051). `estimate` est jugé sur les spectres **de Python**
//! (relus tels quels), pour ne pas cumuler l'écart des FFT et celui de l'estimation.

use otb_analysis::open_loop::{self, C, LoopSpectra, PAIRS, Terms};
use serde_json::Value;

mod common;
use common::*;

/// Spectres croisés, écart par case relatif (plancher `CELL_FLOOR`) : FFT de `realfft` contre pocketfft. Mesuré le 2026-10-30 :
/// 8,3e-11 au pire (cases proches d'une annulation). Marge ×12 ; une mauvaise fenêtre ou un mauvais bloc dépasse 1e-3.
const SPECTRA_RTOL: f64 = 1e-9;
/// Fonctions de transfert estimées depuis les spectres de Python (divisions de Smith, produits en FMA : L-051). Mesuré : 2,2e-15 au
/// pire, `Ce` identique au bit près. Marge ×4 000.
const ESTIMATE_RTOL: f64 = 1e-11;

fn spec_of(c: &Value, v: &Value) -> LoopSpectra {
    let nb = v["nb"].as_u64().unwrap() as usize;
    let freqs = input(c, &v["freqs"]);
    let nf = freqs.len();
    let blocks = PAIRS
        .iter()
        .map(|p| {
            let z = cx(c, &v["blocks"][*p]);
            (0..nb)
                .map(|b| z[b * nf..(b + 1) * nf].to_vec())
                .collect::<Vec<Vec<C>>>()
        })
        .collect();
    LoopSpectra {
        freqs,
        fs: f1(&v["fs"]),
        blocks,
        n_windows: v["n_windows"].as_u64().unwrap() as usize,
        has_d: v["has_d"].as_bool().unwrap(),
    }
}

#[test]
fn loop_spectra_conforme() {
    let c = cas("loop_spectra");
    let (mut errors, mut nones, mut done, mut blocks_seen) = (0, 0, 0, 0);
    for k in c["spectra"].as_array().unwrap() {
        let what = format!("loop_spectra {}", k["label"]);
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
        let got = open_loop::loop_spectra(
            &r,
            &y,
            &terms,
            f1(&k["fs"]),
            f1(&k["seg_s"]),
            f1(&k["min_input"]),
            max_input,
            k["blocks"].as_i64().unwrap(),
            breaks.as_deref(),
        );
        if is_error(k) {
            assert!(got.is_err(), "{what} : Python lève, Rust rend {got:?}");
            errors += 1;
            continue;
        }
        let got = got.unwrap_or_else(|e| panic!("{what} : Rust lève ({e})"));
        if k["none"].as_bool().unwrap() {
            assert!(got.is_none(), "{what} : Python rend None");
            nones += 1;
            continue;
        }
        let got = got.unwrap_or_else(|| panic!("{what} : Rust rend None, Python un résultat"));
        let want = spec_of(&c, &k["out"]);
        assert_eq!(got.n_windows, want.n_windows, "{what} : fenêtres");
        assert_eq!(got.has_d, want.has_d, "{what} : has_d");
        assert_eq!(got.n_blocks(), want.n_blocks(), "{what} : blocs");
        assert_bits(&got.freqs, &want.freqs, &format!("{what} : fréquences"));
        for (pi, name) in PAIRS.iter().enumerate() {
            for b in 0..want.n_blocks() {
                assert_close_cx(
                    "spectres croisés",
                    &got.blocks[pi][b],
                    &want.blocks[pi][b],
                    SPECTRA_RTOL,
                    &format!("{what} : {name} bloc {b}"),
                );
            }
        }
        blocks_seen += want.n_blocks();
        done += 1;
    }
    assert!(
        errors >= 3 && nones >= 8 && done >= 40 && blocks_seen >= 150,
        "{errors} {nones} {done} {blocks_seen}"
    );
    report();
}

fn check_estimate(got: &open_loop::LoopEstimate, o: &Value, c: &Value, what: &str) {
    let r = ESTIMATE_RTOL;
    assert_close_cx("Ce", &got.ce, &cx(c, &o["Ce"]), r, what);
    assert_close_cx("Cd", &got.cd, &cx(c, &o["Cd"]), r, what);
    assert_close_cx("Cf", &got.cf, &cx(c, &o["Cf"]), r, what);
    assert_close_cx("G", &got.g, &cx(c, &o["G"]), r, what);
    assert_close_cx("T", &got.t, &cx(c, &o["T"]), r, what);
    assert_close_cx("L", &got.l, &cx(c, &o["L"]), r, what);
    assert_close_cx("S_a", &got.s_a, &cx(c, &o["S_a"]), r, what);
    assert_close_cx("L_c", &got.l_c, &cx(c, &o["L_c"]), r, what);
    assert_close_rel("erreur de G", &got.err_g, &input(c, &o["err_g"]), r, what);
    assert_close_rel("cohérence zy", &got.coh_zy, &input(c, &o["coh"]["zy"]), r, what);
    assert_close_rel("cohérence zu", &got.coh_zu, &input(c, &o["coh"]["zu"]), r, what);
    assert_close_rel("cohérence ep", &got.coh_ep, &input(c, &o["coh"]["ep"]), r, what);
    assert_close_rel("cohérence yd", &got.coh_yd, &input(c, &o["coh"]["yd"]), r, what);
    let valid: Vec<bool> = o["valid"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_bool().unwrap())
        .collect();
    assert_eq!(got.valid, valid, "{what} : masque valid");
}

#[test]
fn estimate_conforme() {
    let c = cas("loop_spectra");
    let (mut done, mut with_valid, mut leave, mut no_d) = (0, 0, 0, 0);
    for k in c["estimate"].as_array().unwrap() {
        let what = format!(
            "estimate {} (spectre {}, retiré {})",
            k["label"], k["spec"], k["leave_out"]
        );
        let idx = k["spec"].as_u64().unwrap() as usize;
        let spec = if idx >= 1000 {
            spec_of(&c, &c["random_specs"][idx - 1000])
        } else {
            spec_of(&c, &c["spectra"][idx]["out"])
        };
        let lo = k["leave_out"].as_u64().map(|v| v as usize);
        let got = open_loop::estimate(&spec, lo);
        if is_error(k) {
            panic!("{what} : Python lève, Rust rend un résultat");
        }
        check_estimate(&got, &k["out"], &c, &what);
        done += 1;
        with_valid += got.valid.iter().any(|&v| v) as usize;
        leave += lo.is_some() as usize;
        no_d += (!spec.has_d) as usize;
    }
    assert!(
        done >= 150 && with_valid >= 40 && leave >= 40 && no_d >= 3,
        "{done} {with_valid} {leave} {no_d}"
    );
    report();
}
