//! Page du visualiseur sur un vrai log de 5 s (`tests/data/jeno5_5s.bbl`) : période déplaçable depuis n'importe
//! quel graphique et appliquée une seule fois, curseur, bascule vers les courbes (port de `test_gui_viewer.py`).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::Pos2;
use egui_kittest::Harness;
use otb_gui::app::{App, Screen};

fn banc() -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1400.0, 800.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), App::new(Vec::new()));
    let log = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    h.state_mut().open(vec![log]);
    let debut = Instant::now();
    while h.state().state.logs.is_empty() {
        h.run();
        assert!(debut.elapsed() < Duration::from_secs(30), "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(5));
    }
    for _ in 0..4 {
        h.run();
    }
    h
}

fn px(h: &Harness<'static, App>, ax: usize, t: f64) -> f32 {
    h.state().viewer.transforms[ax]
        .expect("graphique dessiné")
        .position_from_point_x(t)
}

fn centre_y(h: &Harness<'static, App>, ax: usize) -> f32 {
    h.state().viewer.transforms[ax].unwrap().frame().center().y
}

#[test]
fn le_premier_log_ouvert_fait_passer_aux_courbes() {
    let h = banc();
    assert_eq!(h.state().screen, Screen::Viewer);
    assert!(
        h.state().viewer.transforms.iter().all(Option::is_some),
        "les quatre graphiques sont dessinés"
    );
}

#[test]
fn la_periode_se_deplace_depuis_n_importe_quel_graphique_et_s_applique_une_fois() {
    for ax in [0usize, 1, 2, 3] {
        let mut h = banc();
        let (p0, p1) = h.state().state.logs[0].period;
        // le curseur (prioritaire) est écarté du bord de période, qui est ici le début du log
        h.state_mut().state.set_cursor(p0 + (p1 - p0) * 0.8);
        h.run();
        let rev = h.state().state.revision();
        let y = centre_y(&h, ax);
        let x_depart = px(&h, ax, p0);
        let cible = p0 + (p1 - p0) * 0.3;
        h.hover_at(Pos2::new(x_depart, y));
        h.run();
        h.drag_at(Pos2::new(x_depart, y));
        h.run();
        h.hover_at(Pos2::new(px(&h, ax, cible), y));
        h.run();
        // en cours de glissement : la période est modifiée à l'écran, pas dans l'état (les calculs ne repartent pas)
        let (e0, _) = h.state().viewer.period_edit.expect("glissement en cours");
        assert!((e0 - cible).abs() < 0.2, "graphique {ax} : bord à {e0}, cible {cible}");
        assert_eq!(
            h.state().state.logs[0].period,
            (p0, p1),
            "graphique {ax} : période appliquée trop tôt"
        );
        assert_eq!(h.state().state.revision(), rev);
        h.drop_at(Pos2::new(px(&h, ax, cible), y));
        h.run();
        h.run();
        let (n0, n1) = h.state().state.logs[0].period;
        assert!(
            (n0 - cible).abs() < 0.2 && n1 == p1,
            "graphique {ax} : période {n0}–{n1}"
        );
        assert_eq!(
            h.state().state.revision(),
            rev + 1,
            "graphique {ax} : appliquée en une seule fois"
        );
        assert!(h.state().viewer.period_edit.is_none());
    }
}

#[test]
fn le_curseur_se_deplace_sans_changer_la_periode() {
    let mut h = banc();
    let log = h.state().state.logs[0].log.clone();
    let c0 = h.state().state.cursor;
    assert_eq!(c0, log.t[0]);
    let rev = h.state().state.revision();
    let y = centre_y(&h, 1);
    let depart = px(&h, 1, c0);
    let cible = log.t[0] + 2.5;
    h.hover_at(Pos2::new(depart, y));
    h.run();
    h.drag_at(Pos2::new(depart, y));
    h.run();
    h.hover_at(Pos2::new(px(&h, 1, cible), y));
    h.run();
    h.drop_at(Pos2::new(px(&h, 1, cible), y));
    h.run();
    h.run();
    assert!(
        (h.state().state.cursor - cible).abs() < 0.2,
        "curseur à {}",
        h.state().state.cursor
    );
    assert_eq!(h.state().state.revision(), rev, "le curseur ne relance aucun calcul");
}

#[test]
fn un_seul_axe_en_vue_focalisee() {
    let mut h = banc();
    h.state_mut().viewer.focus = 2;
    h.state_mut().viewer.transforms = [None; 4]; // on ne garde que ce qui est dessiné après le changement
    h.run();
    h.run();
    h.run();
    // Pitch occupe toute la hauteur au-dessus des moteurs ; les deux autres axes ne sont plus dessinés
    let (pitch, moteurs) = (
        h.state().viewer.transforms[1].unwrap(),
        h.state().viewer.transforms[3].unwrap(),
    );
    assert!(pitch.frame().height() > 2.0 * moteurs.frame().height());
    let t = &h.state().viewer.transforms;
    assert!(t[0].is_none() && t[2].is_none(), "Roll et Yaw ne sont plus dessinés");
}

#[test]
fn changer_de_log_abandonne_le_glissement_de_periode_en_cours() {
    let mut h = banc();
    let log = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    h.state_mut().open(vec![log]);
    let debut = Instant::now();
    while h.state().state.logs.len() < 2 {
        h.run();
        assert!(debut.elapsed() < Duration::from_secs(30), "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run();
    let avant: Vec<_> = h.state().state.logs.iter().map(|l| l.period).collect();
    h.state_mut().viewer.period_edit = Some((1.0, 2.0));
    h.state_mut().state.set_current(Some(0));
    h.run();
    h.run();
    assert!(
        h.state().viewer.period_edit.is_none(),
        "le glissement d'un autre log ne doit pas survivre"
    );
    let apres: Vec<_> = h.state().state.logs.iter().map(|l| l.period).collect();
    assert_eq!(avant, apres, "et ne doit être appliqué à aucun log");
}
