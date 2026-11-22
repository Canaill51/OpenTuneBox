//! `otb-dev corpus predict` (lot 22f, W-060) sans vrais logs : les réglages prédits prennent **tous** les filtres du vol
//! APRÈS et rien d'autre. Un filtre oublié dans `with_filters_of` ferait passer son changement pour une variation d'un vol à
//! l'autre (prédiction nulle) : ce test casse dès qu'un champ manque.

use otb_dev::corpus::predict::{other_changes, with_filters_of};
use otb_log::settings::{Lowpass, Settings};

fn lowpass(hz: i64) -> Lowpass {
    Lowpass {
        kind: "PT2",
        static_hz: hz,
        dyn_min_hz: hz,
        dyn_max_hz: 2 * hz,
        expo: 7,
    }
}

/// `a` avec chaque réglage de filtre changé.
fn all_filters_changed(a: &Settings) -> Settings {
    let mut b = a.clone();
    b.gyro_lpf1 = lowpass(210);
    b.gyro_lpf2 = lowpass(420);
    b.gyro_notches = vec![(300, 250)];
    b.dterm_lpf1 = lowpass(80);
    b.dterm_lpf2 = lowpass(160);
    b.dterm_notch = (260, 160);
    b.yaw_lowpass_hz = 90;
    b.dyn_notch_count = 2;
    b.dyn_notch_q = 350;
    b.dyn_notch_min_hz = 120;
    b.dyn_notch_max_hz = 650;
    b.rpm_harmonics = 2;
    b.rpm_weights = vec![100, 80];
    b.rpm_q = 600;
    b.rpm_min_hz = 150;
    b.rpm_fade_hz = 30;
    b.rpm_lpf_hz = 120;
    b
}

#[test]
fn les_reglages_predits_prennent_tous_les_filtres_du_vol_apres() {
    let a = Settings::default();
    let b = all_filters_changed(&a);
    assert_eq!(with_filters_of(&a, &b), b);
    assert!(other_changes(&a, &b).is_empty());
}

#[test]
fn les_reglages_predits_gardent_le_reste_du_vol_avant() {
    let a = Settings::default();
    let mut b = all_filters_changed(&a);
    b.pids[0].d += 5;
    b.high_resolution = !a.high_resolution;
    let m = with_filters_of(&a, &b);
    assert_eq!(m.pids, a.pids);
    assert_eq!(m.high_resolution, a.high_resolution);
    assert_eq!(other_changes(&a, &b), ["gains PID", "haute résolution du log"]);
}
