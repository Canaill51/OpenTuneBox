//! Textes du rapport « Diagnostic du bruit » (valeurs lues sur `spectral_window._db` / `_levels_table`) et caractères
//! absents des polices d'egui.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_advisor::noise::AxisNoise;
use otb_advisor::rules::Finding;
use otb_gui::tools::common::safe_text;
use otb_gui::tools::report_ui::{db_text, finding_ui, flight_text, level_row, levels_note};

#[test]
fn les_niveaux_s_ecrivent_comme_en_python() {
    let cas = [
        (-12.4, "−12 dB"),
        (-0.4, "−0 dB"),
        (0.5, "0 dB"),
        (1.5, "2 dB"),
        (2.5, "2 dB"),
        (12.0, "12 dB"),
        (f64::NAN, "–"),
        (f64::INFINITY, "–"),
    ];
    for (v, s) in cas {
        assert_eq!(db_text(v), s, "db_text({v})");
    }
}

#[test]
fn une_ligne_du_tableau_des_niveaux() {
    let r = AxisNoise {
        axis: "Roll".into(),
        before_db: -12.4,
        after_db: -30.6,
        peak_hz: 123.4,
        reduction_db: -1.55,
    };
    // la baisse garde le « - » ASCII et un point décimal, comme `f'{x:.1f} dB'`
    assert_eq!(level_row(&r), ["Roll", "−12 dB", "123 Hz", "−31 dB", "-1.6 dB"]);
    let vide = AxisNoise {
        axis: "Pitch".into(),
        before_db: 5.0,
        after_db: f64::NAN,
        peak_hz: f64::NAN,
        reduction_db: f64::NAN,
    };
    assert_eq!(level_row(&vide), ["Pitch", "5 dB", "–", "–", "–"]);
}

#[test]
fn la_legende_cite_la_bande_et_le_repere() {
    let t = levels_note((80.0, 500.0), "gyro");
    assert!(
        t.starts_with("Sommet du spectre lissé entre 80 et 500 Hz (échelle de PIDtoolbox)"),
        "{t}"
    );
    assert!(t.contains("Repère après filtres"));
    assert!(flight_text(true).starts_with("vol avec mouvements"));
    assert!(flight_text(false).starts_with("vol surtout stationnaire"));
}

#[test]
fn la_fleche_est_remplacee_car_les_polices_d_egui_ne_l_ont_pas() {
    assert_eq!(safe_text("12 → 8 dB"), "12 -> 8 dB");
    assert_eq!(safe_text("sans flèche é à × ≈"), "sans flèche é à × ≈");
    // un constat qui contient « → » s'affiche sans panique (le banc de test panique sur un caractère absent)
    let f = Finding::new(
        "essai",
        "filtres",
        "conseil",
        "Titre → suite",
        "Détail : 12 → 8 dB",
        vec!["Action → faire".into()],
        None,
    );
    let mut h = Harness::new_ui(move |ui| finding_ui(ui, &f));
    h.run();
    h.get_by_label("Titre -> suite");
    h.get_by_label("Détail : 12 -> 8 dB");
    h.get_by_label("  •  Action -> faire");
}
