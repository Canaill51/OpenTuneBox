//! Le décodeur Rust de bout en bout (`decode_bytes`) rend les mêmes empreintes que le décodeur numba d'origine sur les
//! fichiers de `tests/data/decode/` (écrits par `scripts/decoder_fixtures.py`, avec leurs empreintes dans `expected.json`).

use otb_decoder::file::decode_bytes;
use otb_parity::decoded::diff;
use serde_json::Value;
use std::path::PathBuf;

#[test]
fn les_fichiers_fabriques_et_abimes_se_decodent_comme_en_python() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/decode");
    let expected: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    let (mut files, mut sessions) = (0, 0);
    for (name, want) in expected.as_object().unwrap() {
        let path = match name.strip_prefix('@') {
            Some(rel) => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel),
            None => dir.join(format!("{name}.bbl")),
        };
        let got = decode_bytes(&std::fs::read(&path).unwrap(), None);
        let want = want.as_array().unwrap();
        assert_eq!(got.len(), want.len(), "{name} : nombre de sessions décodées");
        for (g, w) in got.iter().zip(want) {
            let d = diff(w, g);
            assert!(d.is_empty(), "{name} session {} : {}", g.index, d.join(" ; "));
            sessions += 1;
        }
        files += 1;
    }
    assert!(files >= 30 && sessions >= 30, "{files} fichiers, {sessions} sessions");
}
