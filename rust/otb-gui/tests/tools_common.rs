//! Briques communes des outils (port de `_spectral_common.py` et de `FileSelector`). Les valeurs attendues viennent des
//! fonctions Python elles-mêmes (étalon).

mod support;

use otb_gui::state::AppState;
use otb_gui::tools::common::{
    AxisToggles, FileSelection, MotorToggles, cdf, decimate, fmt_ms, fmt3, motor_freqs, nice_ceil, rpm_filter_text,
    sel_rows, sorted_positive,
};
use support::synthetic_log;

#[test]
fn les_retards_s_affichent_comme_en_python() {
    let cas = [
        (1.25, "1,25 ms"),
        (1.0, "1 ms"),
        (0.0, "0 ms"),
        (0.004, "0 ms"),
        (0.005, "0,01 ms"),
        (10.0, "10 ms"),
        (12.349, "12,35 ms"),
        (-0.5, "-0,5 ms"),
        (f64::INFINITY, "—"),
        (f64::NAN, "—"),
    ];
    for (v, s) in cas {
        assert_eq!(fmt_ms(v), s, "fmt_ms({v})");
    }
}

#[test]
fn trois_chiffres_significatifs_a_virgule() {
    let cas = [
        (0.07243, "0,0724"),
        (12.0, "12"),
        (1234.5, "1,23e+03"),
        (0.0001, "0,0001"),
        (1e-5, "1e-05"),
        (99.95, "100"),
        (-2.5, "-2,5"),
        (f64::NAN, "—"),
    ];
    for (v, s) in cas {
        assert_eq!(fmt3(v), s, "fmt3({v})");
    }
}

#[test]
fn l_arrondi_rond_de_l_echelle_de_couleurs() {
    let cas = [
        (0.0, 1.0),
        (-1.0, 1.0),
        (f64::NAN, 1.0),
        (0.3, 0.5),
        (1.0, 1.0),
        (1.0000000001, 1.0),
        (2.2, 2.5),
        (2.6, 5.0),
        (4.9, 5.0),
        (7.0, 10.0),
        (10.0, 10.0),
        (99.0, 100.0),
        (101.0, 200.0),
        (0.0123, 0.02),
    ];
    for (v, s) in cas {
        assert_eq!(nice_ceil(v), s, "nice_ceil({v})");
    }
}

#[test]
fn la_reduction_des_courbes_moyenne_par_blocs() {
    let t: Vec<f64> = (0..10).map(f64::from).collect();
    let y: Vec<f64> = (1..=10).map(f64::from).collect();
    // (points max, temps, moyennes) lus sur `sc.decimate`
    let (a, b) = decimate(&t, &y, 3);
    assert_eq!((a, b), (vec![0.0, 4.0], vec![2.5, 6.5]));
    let (a, b) = decimate(&t, &y, 4);
    assert_eq!((a, b), (vec![0.0, 3.0, 6.0], vec![2.0, 5.0, 8.0]));
    // assez court : rendu tel quel
    for max in [10, 20] {
        assert_eq!(decimate(&t, &y, max), (t.clone(), y.clone()));
    }
    // 10 000 points → au plus 1 000 (test_colormaps_and_helpers)
    let t: Vec<f64> = (0..10_000).map(|i| f64::from(i) / 1000.0).collect();
    let (tt, yy) = decimate(&t, &vec![1.0; 10_000], 1000);
    assert_eq!(tt.len(), yy.len());
    assert!(tt.len() <= 1000);
}

#[test]
fn la_courbe_en_s_compte_les_valeurs_au_plus_egales() {
    let f = [0.0, 1.0, 1.5, 2.0, 4.0, 5.0, 6.0];
    assert_eq!(
        cdf(&[1.0, 2.0, 2.0, 5.0], &f),
        vec![0.0, 0.25, 0.25, 0.75, 0.75, 1.0, 1.0]
    );
    assert_eq!(cdf(&[], &f), vec![0.0; 7]);
}

#[test]
fn les_valeurs_utiles_sont_triees_finies_et_positives() {
    assert_eq!(
        sorted_positive(&[3.0, f64::NAN, -1.0, 0.0, 1.0, f64::INFINITY, 2.0]),
        vec![1.0, 2.0, 3.0]
    );
}

#[test]
fn les_colonnes_choisies_de_chaque_canal() {
    let mut r = otb_log::flightlog::Rows::zeros(2, 5);
    for k in 0..2 {
        for (i, v) in r.row_mut(k).iter_mut().enumerate() {
            *v = (10 * k + i) as f32;
        }
    }
    let s = sel_rows(&r, &(1..4));
    assert_eq!((s.k, s.n), (2, 3));
    assert_eq!(s.row(1), &[11.0, 12.0, 13.0]);
    // plage hors du tableau : bornée, jamais de panique
    assert_eq!(sel_rows(&r, &(3..99)).n, 2);
    assert_eq!(sel_rows(&r, &(9..99)).n, 0);
}

#[test]
fn le_texte_du_filtre_rpm() {
    let mut log = synthetic_log("a", 10, 1000.0);
    assert_eq!(rpm_filter_text(&log), "filtre RPM inactif");
    log.settings.rpm_harmonics = 3;
    log.settings.rpm_weights = vec![100, 25, 100];
    log.settings.rpm_q = 500;
    assert_eq!(rpm_filter_text(&log), "100,25,100 % | Q500");
    // moins de poids que d'harmoniques : pas de panique
    log.settings.rpm_harmonics = 5;
    assert_eq!(rpm_filter_text(&log), "100,25,100 % | Q500");
}

#[test]
fn le_multiplicateur_ne_sert_que_sans_erpm() {
    let mut log = synthetic_log("a", 4, 1000.0);
    log.motors.row_mut(0).copy_from_slice(&[10.0, 20.0, 30.0, 40.0]);
    let mhz = motor_freqs(&log, 2.0).unwrap();
    assert_eq!(mhz.row(0), &[80.0, 160.0, 240.0, 320.0]); // 2 × % × 4
    // avec l'eRPM mesuré : rendu tel quel, quel que soit le multiplicateur
    let mut mesure = otb_log::flightlog::Rows::zeros(4, 4);
    mesure.row_mut(0).copy_from_slice(&[1.0, 2.0, 3.0, 4.0]);
    log.motor_hz = Some(mesure);
    assert_eq!(motor_freqs(&log, 9.0).unwrap().row(0), &[1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn la_selection_des_fichiers_suit_les_libelles_et_retombe_sur_le_log_courant() {
    let mut state = AppState::new();
    let sel = FileSelection::new(true);
    assert!(sel.indices(&state).is_empty());
    for nom in ["a", "b", "c"] {
        state.add(synthetic_log(nom, 10, 1000.0));
    }
    // rien de choisi : le log courant (le dernier ouvert)
    assert_eq!(sel.indices(&state), vec![2]);
    state.set_current(Some(0));
    assert_eq!(sel.indices(&state), vec![0]);
    // un seul fichier à la fois en mode simple, même si deux libellés sont retenus
    let simple = FileSelection::new(false);
    assert_eq!(simple.indices(&state).len(), 1);
    // sans choix, après la fermeture du log courant : le log courant qui reste (« b » passe à l'indice 0)
    state.remove(0);
    assert_eq!(sel.indices(&state), vec![0]);
}

#[test]
fn un_clic_choisit_un_fichier_et_le_choix_survit_a_la_fermeture_d_un_autre() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use std::cell::RefCell;
    use std::rc::Rc;

    let mut state = AppState::new();
    for nom in ["a", "b", "c"] {
        state.add(synthetic_log(nom, 10, 1000.0));
    }
    let sel = Rc::new(RefCell::new(FileSelection::new(true)));
    let s2 = sel.clone();
    let mut h = Harness::new_ui_state(
        move |ui, st: &mut AppState| {
            s2.borrow_mut().ui(ui, st);
        },
        state,
    );
    h.run();
    h.get_by_label("a").click(); // « c » (courant) est déjà pris : « a » s'y ajoute
    h.run();
    assert_eq!(sel.borrow().indices(h.state()), vec![0, 2]);
    h.get_by_label("c").click(); // un second clic retire
    h.run();
    assert_eq!(sel.borrow().indices(h.state()), vec![0]);
    // fermer « b » : « a » reste choisi, par son libellé
    h.state_mut().remove(1);
    h.run();
    assert_eq!(sel.borrow().indices(h.state()), vec![0]);
    // fermer « a » : plus rien de choisi → le log courant
    h.state_mut().remove(0);
    h.run();
    assert_eq!(sel.borrow().indices(h.state()), vec![0]);
    assert_eq!(h.state().logs[0].label, "c");

    // mode simple : un clic remplace
    let mut st = AppState::new();
    for nom in ["x", "y"] {
        st.add(synthetic_log(nom, 10, 1000.0));
    }
    let one = Rc::new(RefCell::new(FileSelection::new(false)));
    let o2 = one.clone();
    let mut h = Harness::new_ui_state(move |ui, st: &mut AppState| o2.borrow_mut().ui(ui, st), st);
    h.run();
    h.get_by_label("x").click();
    h.run();
    assert_eq!(one.borrow().indices(h.state()), vec![0]);
}

#[test]
fn les_cases_d_axes_et_de_moteurs() {
    let mut a = AxisToggles::default();
    assert_eq!(a.enabled(), vec![0, 1, 2]);
    a.0[1] = false;
    assert_eq!(a.enabled(), vec![0, 2]);
    let mut m = MotorToggles::default();
    m.0[0] = false;
    m.0[3] = false;
    assert_eq!(m.enabled(), vec![1, 2]);
}
