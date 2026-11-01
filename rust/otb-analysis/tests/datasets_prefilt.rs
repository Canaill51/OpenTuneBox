//! `PREFILT_OF` : jeu filtré → jeu préfiltre (port de `datasets.PREFILT_OF`, utilisé par les outils spectraux).

use otb_analysis::datasets::{DATASETS, PREFILT_OF, is_prefilt};

#[test]
fn prefilt_of_comme_python() {
    // Python : {"gyro": "gyro_prefilt", "dterm": "dterm_prefilt"}
    assert_eq!(PREFILT_OF, [("gyro", "gyro_prefilt"), ("dterm", "dterm_prefilt")]);
}

#[test]
fn les_cles_existent_et_is_prefilt_les_reconnait() {
    let cles: Vec<&str> = DATASETS.iter().map(|(k, _)| *k).collect();
    for (a, b) in PREFILT_OF {
        assert!(cles.contains(&a) && cles.contains(&b));
        assert!(is_prefilt(b));
        assert!(!is_prefilt(a));
    }
    assert!(!is_prefilt("setpoint"));
}
