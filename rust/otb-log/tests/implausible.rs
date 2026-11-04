//! Un en-tête aux réglages absurdes est signalé, jamais affiché comme vrai (D-044).

use otb_log::settings::{Settings, parse_settings};
use std::collections::BTreeMap;

fn avec(looptime: &str) -> Settings {
    let raw: BTreeMap<String, String> = [("looptime".to_string(), looptime.to_string())].into();
    parse_settings(&raw)
}

#[test]
fn looptime_zero_est_signale() {
    let s = avec("0");
    let m = s.implausible().expect("looptime 0 doit être signalé");
    assert!(m.contains("invraisemblables"), "{m}");
}

#[test]
fn les_vraies_cadences_ne_sont_pas_signalees() {
    for lt in ["31", "32", "125", "250", "500"] {
        assert_eq!(avec(lt).implausible(), None, "looptime {lt}");
    }
}
