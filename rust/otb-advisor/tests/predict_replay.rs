//! Prévision du bruit après un changement de filtres par le rejeu (D-059), sur `tests/data/jeno5_5s.bbl` : exacte quand rien
//! ne change, et sans gyro non filtré, la même que le rapport des |H|² (la seule méthode possible alors).

use otb_advisor::rules::{FilterModel, Prediction, predict_filter_change, predict_filter_change_with};
use otb_log::flightlog::{self, FlightLog};

fn journal() -> FlightLog {
    let p = format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"));
    flightlog::load(std::path::Path::new(&p)).unwrap().remove(0)
}

fn noise(p: &Prediction) -> Vec<f64> {
    [&p.gyro_mean, &p.gyro_peak, &p.d_mean, &p.d_peak]
        .into_iter()
        .flatten()
        .copied()
        .collect()
}

#[test]
fn sans_changement_le_rejeu_ne_prevoit_aucune_variation() {
    let log = journal();
    assert!(log.gyro_unfilt.is_some(), "le test demande un gyro non filtré");
    let p = predict_filter_change(&log, &(0..log.n()), &log.settings, 500.0).unwrap();
    assert_eq!(p.gyro_mean.len(), 2);
    assert_eq!(p.d_mean.len(), 2);
    assert!(noise(&p).iter().all(|&x| x == 0.0), "{p:?}");
}

#[test]
fn un_changement_est_prevu_par_le_rejeu_et_non_par_le_rapport() {
    let log = journal();
    let sel = 0..log.n();
    let mut modf = log.settings.clone();
    modf.dyn_notch_count += 1;
    let default = predict_filter_change(&log, &sel, &modf, 500.0).unwrap();
    let replay = predict_filter_change_with(&log, &sel, &modf, 500.0, FilterModel::Replay).unwrap();
    let ratio = predict_filter_change_with(&log, &sel, &modf, 500.0, FilterModel::Ratio).unwrap();
    assert_eq!(default, replay);
    assert_ne!(
        noise(&replay),
        noise(&ratio),
        "les deux méthodes devraient différer ici"
    );
}

#[test]
fn sans_gyro_non_filtre_le_rejeu_laisse_place_au_rapport() {
    let mut log = journal();
    log.gyro_unfilt = None;
    let sel = 0..log.n();
    let mut modf = log.settings.clone();
    modf.dyn_notch_count += 1;
    let replay = predict_filter_change_with(&log, &sel, &modf, 500.0, FilterModel::Replay).unwrap();
    let ratio = predict_filter_change_with(&log, &sel, &modf, 500.0, FilterModel::Ratio).unwrap();
    assert_eq!(replay, ratio);
    assert!(!replay.d_mean.is_empty(), "le terme D se prévoit encore");
}
