//! État partagé : logs, couleurs, libellés, période, révision (port de `gui/state.py`).

mod support;

use otb_gui::state::{AppState, MAX_LOGS};
use otb_gui::theme::file_color;
use support::synthetic_log;

#[test]
fn un_log_ajoute_devient_le_courant_avec_la_premiere_couleur_libre() {
    let mut s = AppState::new();
    assert!(s.current_log().is_none());
    assert!(s.add(synthetic_log("a", 1000, 1000.0)));
    assert!(s.add(synthetic_log("b", 1000, 1000.0)));
    assert_eq!(s.current, Some(1));
    assert_eq!((s.logs[0].color, s.logs[1].color), (file_color(0), file_color(1)));
    // une couleur libérée est reprise avant les suivantes
    s.remove(0);
    s.add(synthetic_log("c", 1000, 1000.0));
    assert_eq!(s.logs[1].color, file_color(0));
}

#[test]
fn deux_logs_du_meme_nom_ont_des_libelles_distincts() {
    let mut s = AppState::new();
    for _ in 0..3 {
        s.add(synthetic_log("vol", 100, 1000.0));
    }
    let labels: Vec<_> = s.logs.iter().map(|l| l.label.as_str()).collect();
    assert_eq!(labels, ["vol", "vol (2)", "vol (3)"]);
}

#[test]
fn pas_plus_de_dix_logs() {
    let mut s = AppState::new();
    for i in 0..MAX_LOGS {
        assert!(s.add(synthetic_log(&format!("l{i}"), 100, 1000.0)));
    }
    let rev = s.revision();
    assert!(!s.add(synthetic_log("de_trop", 100, 1000.0)));
    assert_eq!((s.logs.len(), s.revision()), (MAX_LOGS, rev));
}

#[test]
fn retirer_garde_un_courant_valide() {
    let mut s = AppState::new();
    for n in ["a", "b", "c"] {
        s.add(synthetic_log(n, 100, 1000.0));
    }
    s.remove(2);
    assert_eq!(s.current, Some(1));
    s.remove(7); // hors limites : rien
    assert_eq!(s.logs.len(), 2);
    s.remove(0);
    s.remove(0);
    assert_eq!((s.logs.len(), s.current), (0, None));
    s.add(synthetic_log("d", 100, 1000.0));
    s.clear();
    assert_eq!((s.logs.len(), s.current), (0, None));
}

#[test]
fn la_periode_est_bornee_ordonnee_et_vide_les_caches() {
    let mut s = AppState::new();
    s.add(synthetic_log("a", 10_000, 1000.0)); // 0 à 9,999 s
    let rev = s.revision();
    s.logs[0].extras.insert("spectre", std::sync::Arc::new(1u8));
    s.set_period(0, 8.0, 2.0);
    assert_eq!(s.logs[0].period, (2.0, 8.0));
    assert!(s.logs[0].extras.is_empty());
    assert_eq!(s.revision(), rev + 1);
    s.set_period(0, -5.0, 50.0);
    assert_eq!(s.logs[0].period, (0.0, 9.999));
    // même période : ni révision ni cache vidé
    let rev = s.revision();
    s.logs[0].extras.insert("spectre", std::sync::Arc::new(1u8));
    s.set_period(0, 0.0, 9.999);
    assert_eq!(s.revision(), rev);
    assert!(!s.logs[0].extras.is_empty());
    s.set_period(3, 0.0, 1.0); // log inexistant : rien
    s.reset_period(0);
    assert_eq!(s.logs[0].period, (0.0, 9.999));
}

#[test]
fn le_curseur_et_le_log_courant_ne_changent_pas_la_revision() {
    let mut s = AppState::new();
    s.add(synthetic_log("a", 100, 1000.0));
    s.add(synthetic_log("b", 100, 1000.0));
    let rev = s.revision();
    s.set_cursor(0.05);
    s.set_current(Some(0));
    assert_eq!((s.cursor, s.current, s.revision()), (0.05, Some(0), rev));
    s.set_current(Some(9)); // hors limites : aucun
    assert_eq!(s.current, None);
}
