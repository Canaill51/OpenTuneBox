//! Le décodeur Rust de bout en bout (`decode_bytes`) puis `FlightLog` (`from_session`) rendent les mêmes empreintes que le
//! décodeur numba et le Python d'origine, sur les fichiers de `tests/data/decode/` (écrits par `scripts/decoder_fixtures.py`,
//! avec leurs empreintes dans `expected.json`).

use otb_decoder::file::decode_bytes;
use otb_log::flightlog::load;
use otb_parity::decoded::diff;
use otb_parity::flightlog_digest::{diff as fl_diff, flightlog_digest, prefilter_deviation};
use otb_parity::layers::PREFILTER_TOLERANCE;
use serde_json::Value;
use std::path::PathBuf;

fn fixtures() -> Vec<(String, PathBuf, Value)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("tests/data/decode");
    let expected: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    expected
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, want)| {
            let path = match name.strip_prefix('@') {
                Some(rel) => root.join("../..").join(rel),
                None => dir.join(format!("{name}.bbl")),
            };
            (name.clone(), path, want.clone())
        })
        .collect()
}

#[test]
fn les_fichiers_fabriques_et_abimes_se_decodent_comme_en_python() {
    let (mut files, mut sessions) = (0, 0);
    for (name, path, want) in fixtures() {
        let got = decode_bytes(&std::fs::read(&path).unwrap(), None);
        let want = want["sessions"].as_array().unwrap();
        assert_eq!(got.len(), want.len(), "{name} : nombre de sessions décodées");
        for (g, w) in got.iter().zip(want) {
            let d = diff(w, g);
            assert!(d.is_empty(), "{name} session {} : {}", g.index, d.join(" ; "));
            sessions += 1;
        }
        files += 1;
    }
    assert!(files >= 50 && sessions >= 50, "{files} fichiers, {sessions} sessions");
}

#[test]
fn les_journaux_de_vol_se_construisent_comme_en_python() {
    let (mut logs, mut absents) = (0, 0);
    for (name, path, want) in fixtures() {
        let got = load(&path).unwrap();
        let want = want["flightlog"].as_array().unwrap();
        assert_eq!(got.len(), want.len(), "{name} : nombre de FlightLog");
        absents += (want.is_empty()) as usize;
        for (g, w) in got.iter().zip(want) {
            let d = fl_diff(w, &flightlog_digest(g));
            assert!(d.is_empty(), "{name} session {} : {}", g.session, d.join(" ; "));
            let dev = prefilter_deviation(&w["d_term_prefilter"], g).unwrap_or_else(|e| panic!("{name} : {e}"));
            assert!(dev <= PREFILTER_TOLERANCE, "{name} : d_term_prefilter écart {dev:e}");
            logs += 1;
        }
    }
    assert!(
        logs >= 50 && absents >= 3,
        "{logs} FlightLog, {absents} fichiers sans FlightLog"
    );
}
