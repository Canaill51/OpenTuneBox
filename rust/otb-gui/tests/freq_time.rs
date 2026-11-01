//! Outil « Fréquence × temps » (port de `test_gui_spectral.py::test_freq_time_window`) sur le vrai log de 5 s.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::Pos2;
use egui_kittest::Harness;
use otb_gui::state::AppState;
use otb_gui::theme::CLAIR;
use otb_gui::tool::{Page, Tool, ToolPage};
use otb_gui::tools::freq_time::{FreqTimeOutput, FreqTimeTool, MAX_TRACE_POINTS, finite_runs, reduce_columns};

fn log5s() -> otb_log::flightlog::FlightLog {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    otb_log::flightlog::load(&p).unwrap().remove(0)
}

fn etat() -> AppState {
    let mut st = AppState::new();
    st.add(log5s());
    st
}

fn calcule(t: &FreqTimeTool, st: &AppState) -> FreqTimeOutput {
    FreqTimeTool::compute(t.options(), st.inputs(&[0])).unwrap()
}

#[test]
fn le_calcul_par_defaut_donne_les_spectrogrammes_et_les_traces() {
    let st = etat();
    let out = calcule(&FreqTimeTool::new(), &st);
    assert_eq!((out.label.as_str(), out.key), ("jeno5_5s", "gyro_prefilt"));
    assert_eq!(out.maps.keys().copied().collect::<Vec<_>>(), [0, 1, 2]);
    let (t0, t1) = out.period;
    for sg in out.maps.values() {
        assert_eq!(sg.z.len(), sg.freqs.len() * sg.times.len());
        assert!(sg.times.iter().all(|t| *t >= t0 - 0.5 && *t <= t1 + 0.5));
        assert!((sg.freqs.last().unwrap() / out.nyquist - 1.0).abs() < 0.01);
    }
    assert_eq!(out.motor_hz.len(), 4);
    let mt = out.motor_t.as_ref().unwrap();
    assert!(mt.len() <= MAX_TRACE_POINTS && mt.len() > 1000, "{}", mt.len());
    assert!(out.motor_hz.iter().all(|m| m.len() == mt.len()));
    assert!(!out.notches.is_empty());
    assert!(
        out.notches
            .iter()
            .all(|(t, h)| t.len() == h.len() && t.len() <= MAX_TRACE_POINTS)
    );
}

#[test]
fn le_terme_d_n_a_pas_de_yaw_et_le_pas_temporel_change_le_nombre_de_colonnes() {
    let st = etat();
    let mut t = FreqTimeTool::new();
    t.set_dataset("dterm");
    t.set_hop_div(10); // 30 ms
    let fin = calcule(&t, &st);
    assert_eq!(fin.maps.keys().copied().collect::<Vec<_>>(), [0, 1]);
    t.set_hop_div(1); // 300 ms
    let gros = calcule(&t, &st);
    assert!(fin.maps[&0].times.len() > 5 * gros.maps[&0].times.len());
}

#[test]
fn les_tronçons_finis_coupent_la_ligne_sur_un_nan() {
    let x = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let y = [1.0, 2.0, f64::NAN, 4.0, 5.0, f64::INFINITY, 7.0];
    assert_eq!(
        finite_runs(&x, &y),
        vec![
            vec![[0.0, 1.0], [1.0, 2.0]],
            vec![[3.0, 4.0], [4.0, 5.0]],
            vec![[6.0, 7.0]]
        ]
    );
    assert!(finite_runs(&[], &[]).is_empty());
    assert!(finite_runs(&[0.0, 1.0], &[f64::NAN, f64::NAN]).is_empty());
}

#[test]
fn les_colonnes_en_trop_sont_moyennees_par_blocs() {
    // 2 fréquences × 7 colonnes, au plus 3 : blocs de 3 → 3 colonnes (3 + 3 + 1)
    let z: Vec<f32> = vec![
        1.0,
        2.0,
        3.0,
        4.0,
        5.0,
        6.0,
        7.0, //
        f32::NAN,
        f32::NAN,
        f32::NAN,
        1.0,
        f32::NAN,
        3.0,
        9.0,
    ];
    let (r, nt) = reduce_columns(&z, 2, 7, 3);
    assert_eq!(nt, 3);
    assert_eq!(r.len(), 6);
    assert_eq!(&r[..3], &[2.0, 5.0, 7.0]);
    assert!(r[3].is_nan(), "bloc sans valeur : NaN");
    assert_eq!((r[4], r[5]), (2.0, 9.0), "les NaN d'un bloc sont ignorés");
    // assez étroit : rendu tel quel
    let (r, nt) = reduce_columns(&z, 2, 7, 7);
    assert_eq!(nt, 7);
    assert_eq!(
        r.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        z.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
}

// ------------------------------------------------------------------ dans la fenêtre

type Banc = Harness<'static, (ToolPage<FreqTimeTool>, AppState)>;

fn banc(tool: FreqTimeTool) -> Banc {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
        |ui, (p, s): &mut (ToolPage<FreqTimeTool>, AppState)| p.ui(ui, s, &CLAIR),
        (ToolPage::new(tool), etat()),
    );
    attendre(&mut h);
    h
}

fn attendre(h: &mut Banc) {
    let debut = Instant::now();
    loop {
        h.run_steps(2);
        if !h.state().0.busy() {
            break;
        }
        assert!(debut.elapsed() < Duration::from_secs(60), "calcul jamais terminé");
        std::thread::sleep(Duration::from_millis(10));
    }
    h.run_steps(4);
}

fn px(h: &Banc, ax: usize, t: f64) -> f32 {
    h.state().0.tool.transforms[ax]
        .expect("graphique dessiné")
        .position_from_point_x(t)
}

fn centre_y(h: &Banc, ax: usize) -> f32 {
    h.state().0.tool.transforms[ax].unwrap().frame().center().y
}

#[test]
fn le_curseur_se_deplace_en_glissant_son_trait_et_sur_un_clic() {
    let mut h = banc(FreqTimeTool::new());
    let (t0, t1) = h.state().1.logs[0].period;
    h.state_mut().1.set_cursor(t0 + (t1 - t0) * 0.25);
    h.run_steps(2);
    for ax in 0..3 {
        let depart = t0 + (t1 - t0) * 0.25;
        let cible = t0 + (t1 - t0) * 0.7;
        h.state_mut().1.set_cursor(depart);
        h.run_steps(2);
        let y = centre_y(&h, ax);
        h.hover_at(Pos2::new(px(&h, ax, depart), y));
        h.run();
        h.drag_at(Pos2::new(px(&h, ax, depart), y));
        h.run();
        h.hover_at(Pos2::new(px(&h, ax, cible), y));
        h.run();
        h.drop_at(Pos2::new(px(&h, ax, cible), y));
        h.run_steps(3);
        let c = h.state().1.cursor;
        assert!((c - cible).abs() < 0.1, "graphique {ax} : curseur à {c}, cible {cible}");
    }
    // un clic loin du trait place le curseur
    let cible = t0 + (t1 - t0) * 0.4;
    let y = centre_y(&h, 1);
    let p = Pos2::new(px(&h, 1, cible), y + 20.0);
    h.hover_at(p);
    h.run();
    h.drag_at(p);
    h.run();
    h.drop_at(p);
    h.run_steps(3);
    assert!(
        (h.state().1.cursor - cible).abs() < 0.1,
        "curseur à {}",
        h.state().1.cursor
    );
    // aucun recalcul : le curseur n'est pas un réglage de calcul
    assert!(!h.state().0.busy());
}

#[test]
fn moins_de_100_hz_ramene_le_haut_de_la_vue_a_100() {
    let mut t = FreqTimeTool::new();
    t.set_sub100(true);
    let h = banc(t);
    for ax in 0..3 {
        let b = h.state().0.tool.transforms[ax].unwrap().bounds().max()[1];
        assert_eq!(b, 100.0, "graphique {ax}");
    }
    // sans cette case : jusqu'à Nyquist
    let h = banc(FreqTimeTool::new());
    let nyq = log5s().fs / 2.0;
    assert!((h.state().0.tool.transforms[0].unwrap().bounds().max()[1] - nyq).abs() < 1e-6);
}
