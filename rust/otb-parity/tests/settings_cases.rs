//! `parse_settings` rend les mêmes réglages que Python, sur les cas de `scripts/settings_cases.py`
//! (régénérer `tests/data/settings_cases.json` avec ce script si `log/settings.py` change).

use otb_log::settings::parse_settings;
use otb_parity::settings_json::{diff, settings_json};
use serde_json::Value;
use std::collections::BTreeMap;

#[test]
fn parse_settings_suit_python() {
    let p = format!("{}/tests/data/settings_cases.json", env!("CARGO_MANIFEST_DIR"));
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    let (mut compared, mut skipped) = (0, 0);
    for (n, c) in cases.iter().enumerate() {
        if c.get("error").is_some() {
            skipped += 1; // Python plante (int(float("inf"))) : le Rust rend la valeur par défaut, non comparé
            continue;
        }
        let raw: BTreeMap<String, String> = c["raw"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
            .collect();
        let d = diff(&c["settings"], &settings_json(&parse_settings(&raw)));
        assert!(d.is_empty(), "cas {n} {:?} : {}", c["raw"], d.join(" ; "));
        compared += 1;
    }
    assert!(
        compared > 1000 && skipped <= 20,
        "{compared} comparés, {skipped} ignorés"
    );
}
