//! Outil « Fréquence × gaz » (port de `test_gui_spectral.py::test_freq_throttle_window`) sur le vrai log de 5 s.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_gui::app::{App, Screen};
use otb_gui::state::AppState;
use otb_gui::tool::Tool;
use otb_gui::tools::common::percentile_linear;
use otb_gui::tools::freq_throttle::{
    FreqThrottleOutput, FreqThrottleTool, Map, NBINS, PRESETS, XAxis, preset_columns, x_values,
};

fn log5s() -> otb_log::flightlog::FlightLog {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    otb_log::flightlog::load(&p).unwrap().remove(0)
}

/// Un log ouvert deux fois : fichiers « jeno5_5s » et « jeno5_5s (2) ».
fn etat(n: usize) -> AppState {
    let mut st = AppState::new();
    for _ in 0..n {
        st.add(log5s());
    }
    st
}

fn calcule(t: &FreqThrottleTool, st: &AppState) -> FreqThrottleOutput {
    let tous: Vec<usize> = (0..st.logs.len()).collect();
    FreqThrottleTool::compute(t.options(), st.inputs(&tous)).unwrap()
}

fn tool_sur(st: &AppState) -> FreqThrottleTool {
    let mut t = FreqThrottleTool::new();
    let mut c = t.columns();
    for col in &mut c {
        col.1 = Some(st.current.unwrap_or(0));
    }
    t.set_columns(c);
    t
}

#[test]
fn le_calcul_par_defaut_donne_quatre_colonnes_de_cartes() {
    let st = etat(1);
    let out = calcule(&tool_sur(&st), &st);
    assert!(!out.psd);
    assert_eq!(out.xaxis, XAxis::Throttle);
    assert_eq!(out.columns.len(), 4);
    let cles: Vec<&str> = out.columns.iter().map(|c| c.as_ref().unwrap().key).collect();
    assert_eq!(cles, ["gyro_prefilt", "gyro", "dterm_prefilt", "dterm"]);
    let axes = |c: usize| {
        out.columns[c]
            .as_ref()
            .unwrap()
            .maps
            .keys()
            .copied()
            .collect::<Vec<_>>()
    };
    assert_eq!(axes(0), [0, 1, 2]);
    assert_eq!(axes(3), [0, 1], "pas de terme D sur le yaw");
    let col = out.columns[0].as_ref().unwrap();
    assert_eq!(col.x_range, (0.0, 100.0));
    assert_eq!(col.label, "jeno5_5s");
    let m = &col.maps[&0];
    assert_eq!(m.z.len(), m.freqs.len() * NBINS);
    assert_eq!(m.counts.len(), NBINS);
    let (mean, peak) = m.band_stats(100.0, 1000.0);
    assert!(mean.is_finite() && peak > mean && mean > 0.0, "moy {mean}, pic {peak}");
}

#[test]
fn la_moyenne_et_le_pic_d_une_bande_se_lisent_sur_les_tranches_non_vides() {
    // valeurs lues sur `FreqThrottle.band_stats` de Python
    let z = [
        1.0,
        2.0,
        f64::NAN,
        4.0, //
        5.0,
        6.0,
        7.0,
        8.0, //
        9.0,
        f64::NAN,
        11.0,
        12.0, //
        13.0,
        14.0,
        15.0,
        16.0, //
        17.0,
        18.0,
        19.0,
        20.0,
    ];
    let m = Map {
        freqs: vec![0.0, 50.0, 100.0, 150.0, 200.0],
        bins: vec![1.0, 2.0, 3.0, 4.0],
        z: z.to_vec(),
        counts: vec![3, 2, 0, 5],
    };
    assert_eq!(m.band_stats(50.0, 150.0), (10.375, 16.0));
    let (mean, peak) = m.band_stats(0.0, 0.0);
    assert!((mean - 2.3333333333333335).abs() < 1e-15 && peak == 4.0);
    assert_eq!(m.band_stats(100.0, 100.0), (10.5, 12.0));
    // bande sans fréquence, ou tout en NaN : (NaN, NaN)
    let (a, b) = m.band_stats(300.0, 400.0);
    assert!(a.is_nan() && b.is_nan());
    let vide = Map {
        z: vec![f64::NAN; 20],
        counts: vec![1; 4],
        ..m
    };
    let (a, b) = vide.band_stats(0.0, 200.0);
    assert!(a.is_nan() && b.is_nan());
}

#[test]
fn le_centile_lineaire_est_celui_de_numpy() {
    let a = [1.0, 1.5, 2.6, 3.0, 4.0, 5.0, 9.0];
    for (q, attendu) in [
        (0.0, 1.0),
        (50.0, 3.0),
        (99.0, 8.759999999999998),
        (100.0, 9.0),
        (25.5, 2.083),
    ] {
        assert!((percentile_linear(&a, q) - attendu).abs() < 1e-12, "q = {q}");
    }
}

#[test]
fn les_prereglages_remplissent_les_colonnes_comme_en_python() {
    assert_eq!(PRESETS.len(), 9);
    // préréglage 1 (le défaut) : un seul fichier, quatre jeux
    assert_eq!(
        preset_columns(0, &[2], 0),
        [
            (Some("gyro_prefilt"), Some(2)),
            (Some("gyro"), Some(2)),
            (Some("dterm_prefilt"), Some(2)),
            (Some("dterm"), Some(2))
        ]
    );
    // préréglage 4 : A|A|B|B
    let c = preset_columns(3, &[0, 1], 0);
    assert_eq!(c.map(|x| x.1), [Some(0), Some(0), Some(1), Some(1)]);
    // préréglage 6 : A|B|C|D avec deux fichiers seulement → colonnes 3 et 4 vides
    let c = preset_columns(5, &[0, 1], 0);
    assert_eq!(c[2], (None, None));
    assert_eq!(c[3], (None, None));
    assert_eq!(c[1], (Some("gyro_prefilt"), Some(1)));
    // aucun fichier choisi : le log courant
    assert_eq!(preset_columns(6, &[], 3)[0], (Some("gyro"), Some(3)));
}

#[test]
fn le_mode_psd_change_les_echelles_et_l_amplitude_les_remet() {
    let mut t = FreqThrottleTool::new();
    assert_eq!((t.zmin(), t.scale()), (0.0, [0.5; 4]));
    t.set_psd(true);
    assert_eq!((t.zmin(), t.scale()), (-40.0, [10.0; 4]));
    t.set_psd(false);
    assert_eq!((t.zmin(), t.scale()), (0.0, [0.5; 4]));
}

#[test]
fn l_axe_hz_moteur_donne_un_intervalle_arrondi_a_50_hz() {
    let st = etat(1);
    let mut t = tool_sur(&st);
    t.set_xaxis(XAxis::MotorHz);
    t.set_psd(true);
    let out = calcule(&t, &st);
    assert!(out.psd);
    assert_eq!(out.xaxis, XAxis::MotorHz);
    let col = out.columns[0].as_ref().unwrap();
    assert!(
        col.x_range.1 > 100.0 && col.x_range.1 % 50.0 == 0.0,
        "{:?}",
        col.x_range
    );
    // % moteur moyen : 0..100
    t.set_xaxis(XAxis::MotorPct);
    assert_eq!(calcule(&t, &st).columns[0].as_ref().unwrap().x_range, (0.0, 100.0));
}

#[test]
fn sans_moteurs_seuls_les_gaz_donnent_des_cartes() {
    let mut log = log5s();
    log.motors = otb_log::flightlog::Rows::zeros(0, log.n());
    log.motor_hz = None;
    let sel = 0..log.n();
    assert!(x_values(&log, &sel, XAxis::Throttle, 2.1).is_some());
    assert!(x_values(&log, &sel, XAxis::MotorPct, 2.1).is_none());
    assert!(x_values(&log, &sel, XAxis::MotorHz, 2.1).is_none());
}

#[test]
fn l_echelle_auto_se_cale_sur_les_donnees() {
    let st = etat(1);
    let mut t = tool_sur(&st);
    t.set_psd(true); // échelles à 10
    let out = calcule(&t, &st);
    t.auto_scale(&out);
    assert!(t.scale().iter().all(|s| *s != 10.0), "{:?}", t.scale());
    // PSD : multiple de 5 au-dessus du maximum de la bande 100–1000 Hz
    assert!(t.scale().iter().all(|s| (s / 5.0).fract() == 0.0));
    // amplitude : valeur ronde (1, 2, 2,5, 5 × 10^n)
    let mut t = tool_sur(&st);
    let out = calcule(&t, &st);
    t.auto_scale(&out);
    for s in t.scale() {
        let e = 10f64.powf(s.log10().floor());
        assert!(
            [1.0, 2.0, 2.5, 5.0, 10.0].iter().any(|m| (s - m * e).abs() < 1e-12 * e),
            "{s}"
        );
    }
}

#[test]
fn une_colonne_sans_fichier_ou_sans_donnees_reste_vide() {
    let st = etat(1);
    let mut t = tool_sur(&st);
    let mut c = t.columns();
    c[1] = (None, Some(0)); // (vide)
    c[2] = (Some("gyro"), Some(7)); // fichier inexistant
    t.set_columns(c);
    let out = calcule(&t, &st);
    assert!(out.columns[0].is_some() && out.columns[1].is_none() && out.columns[2].is_none());
}

// ------------------------------------------------------------------ dans la fenêtre

fn banc() -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1500.0, 900.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), App::new(otb_gui::tools::all_pages()));
    let log = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    h.state_mut().open(vec![log]);
    let debut = Instant::now();
    while h.state().state.logs.is_empty() {
        h.run();
        assert!(debut.elapsed() < Duration::from_secs(30), "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(5));
    }
    h
}

fn attendre(h: &mut Harness<'static, App>) {
    let debut = Instant::now();
    loop {
        h.run_steps(2);
        let Screen::Tool(i) = h.state().screen else { return };
        if !h.state().pages[i].busy() {
            break;
        }
        assert!(debut.elapsed() < Duration::from_secs(60), "calcul jamais terminé");
        std::thread::sleep(Duration::from_millis(10));
    }
    h.run_steps(4);
}

#[test]
fn le_bouton_du_spectre_ouvre_l_outil_sans_bouton_propre_dans_la_barre() {
    let mut h = banc();
    h.state_mut().screen = Screen::Tool(0);
    attendre(&mut h);
    assert!(
        h.query_by_label("Fréquence × gaz").is_none(),
        "pas de bouton dans la barre"
    );
    h.get_by_label("Fréq. × gaz").click();
    attendre(&mut h);
    assert_eq!(h.state().screen, Screen::Tool(1));
    // les cartes sont là, et la barre garde « Analyse spectrale » en valeur
    // chaque champ de la page a un nom accessible (le banc le vérifie à la fin de `run`)
    h.run();
    h.get_by_label_contains("Gyro préfiltre | jeno5_5s");
    h.get_by_label("Analyse spectrale");
    h.get_by_label("Échelle auto").click();
    attendre(&mut h);
}

#[test]
fn sans_log_le_bouton_demande_d_en_ouvrir_un_sans_empiler_le_message() {
    let mut h = Harness::builder()
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), App::new(otb_gui::tools::all_pages()));
    for _ in 0..3 {
        h.state_mut().open_tool("Fréquence × gaz");
        h.run_steps(2);
    }
    assert_eq!(h.state().screen, Screen::Guide);
    assert_eq!(h.state().status, "Ouvrez d'abord un log Blackbox.");
}

#[test]
fn un_jeu_de_donnees_absent_du_log_laisse_la_colonne_vide() {
    // sans gyro non filtré, « Gyro préfiltre » n'existe pas : colonne vide (et non une colonne sans carte)
    let mut log = log5s();
    log.gyro_unfilt = None;
    let mut st = AppState::new();
    st.add(log);
    let out = calcule(&tool_sur(&st), &st);
    assert!(out.columns[0].is_none(), "gyro_prefilt absent");
    assert!(out.columns[1].is_some() && out.columns[3].is_some());
}

#[test]
fn en_psd_les_cartes_sont_a_l_echelle_de_pidtoolbox() {
    use otb_analysis::spectral::{Mode, freq_throttle, to_ptb_db};
    let st = etat(1);
    let mut t = tool_sur(&st);
    t.set_psd(true);
    let out = calcule(&t, &st);
    let m = &out.columns[1].as_ref().unwrap().maps[&0]; // gyro, roll
    // même carte, calculée directement : PSD en dB − 6,5 dB
    let log = &st.logs[0].log;
    let sel = st.logs[0].sel();
    let x: Vec<f64> = log.gyro.row(0)[sel.clone()].iter().map(|&v| f64::from(v)).collect();
    let thr: Vec<f64> = log.throttle[sel.clone()].iter().map(|&v| f64::from(v)).collect();
    let br: Vec<i64> = log.breaks(sel, 4.0).iter().map(|&b| b as i64).collect();
    let direct = freq_throttle(&x, &thr, log.fs, Mode::Psd, 300.0, NBINS, (0.0, 100.0), Some(&br)).unwrap();
    let attendu = to_ptb_db(&direct.z);
    assert_eq!(m.z.len(), attendu.len());
    let mut compares = 0;
    for (a, b) in m.z.iter().zip(&attendu) {
        assert!(a.is_nan() == b.is_nan(), "NaN différents");
        if a.is_finite() {
            assert!((a - b).abs() < 1e-9, "{a} contre {b}");
            compares += 1;
        }
    }
    assert!(compares > 1000);
    // et ce n'est pas l'amplitude
    let amp = calcule(&tool_sur(&st), &st).columns[1].as_ref().unwrap().maps[&0]
        .z
        .clone();
    assert_ne!(
        amp.iter().position(|v| v.is_finite()).map(|i| amp[i]),
        m.z.iter().position(|v| v.is_finite()).map(|i| m.z[i])
    );
}
