//! Outil « Analyse spectrale » (port de `test_gui_spectral.py::test_spectral_window` et `…_noise_diagnosis`), sur un vrai
//! log de 5 s (`tests/data/jeno5_5s.bbl` : eRPM, notchs FFT_FREQ, gyro non filtré, filtre RPM Q700).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_analysis::spectral::{Mode, spectrum, to_ptb_db};
use otb_gui::app::{App, Screen};
use otb_gui::state::AppState;
use otb_gui::tool::Tool;
use otb_gui::tools::spectral::{FileSpectra, SpectralOutput, SpectralTool, region_alpha, rpm_regions, s_curve};

fn log5s() -> otb_log::flightlog::FlightLog {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    otb_log::flightlog::load(&p).unwrap().remove(0)
}

fn etat() -> AppState {
    let mut st = AppState::new();
    st.add(log5s());
    st
}

fn calcule(t: &SpectralTool, st: &AppState) -> SpectralOutput {
    SpectralTool::compute(t.options(), st.inputs(&[0])).unwrap()
}

#[test]
fn le_calcul_par_defaut_donne_les_spectres_et_les_mesures() {
    let st = etat();
    let out = calcule(&SpectralTool::new(), &st);
    assert!(out.psd);
    assert_eq!(out.keys, ["gyro", "gyro_prefilt"]);
    assert_eq!(out.files.len(), 1);
    let f = &out.files[0];
    assert_eq!(f.label, "jeno5_5s");
    assert_eq!(f.spectra.len(), 6); // 2 jeux × 3 axes
    let (freqs, v) = &f.spectra[&("gyro", 0)];
    assert!((freqs.last().unwrap() / f.nyquist - 1.0).abs() < 0.01);
    assert!(v.iter().all(|x| x.is_finite()));
    // échelle de PIDtoolbox : densité physique − 6,5 dB, fenêtres de 2 s (0,5 Hz) par défaut
    let log = &st.logs[0].log;
    let sel = st.logs[0].sel();
    let x: Vec<f64> = log.gyro.row(0)[sel].iter().map(|&v| f64::from(v)).collect();
    let ref_ = spectrum(&x, log.fs, Mode::Psd, 0.5, 0, Some(&[])).unwrap();
    let attendu = to_ptb_db(&ref_.values);
    assert_eq!(v.len(), attendu.len());
    for (a, b) in v.iter().zip(&attendu) {
        assert!((a - b).abs() < 1e-9, "{a} contre {b}");
    }
    assert!(f.gyro_ms.is_finite() && f.gyro_ms > 0.0 && f.gyro_ms < 10.0);
    assert_eq!(f.motor_sorted.len(), 4);
    assert!(f.motor_sorted[0].len() > 100);
    assert!(!f.notch_sorted.is_empty());
    let (mean, std) = &f.noise[&("gyro_prefilt", 0)];
    assert_eq!((mean.len(), std.len()), (3, 3));
    assert!(mean.iter().all(|x| x.is_finite()));
    assert!(f.rpm_text.ends_with("Q700"), "{}", f.rpm_text);
    assert!(f.diagnosis.is_none(), "le diagnostic n'est calculé que sur demande");
}

#[test]
fn le_mode_amplitude_garde_l_echelle_physique_et_le_yaw_sans_terme_d_est_absent() {
    let st = etat();
    let mut t = SpectralTool::new();
    t.set_psd(false);
    for k in ["gyro", "gyro_prefilt"] {
        t.set_dataset(k, false);
    }
    t.set_dataset("dterm", true);
    t.set_dataset("dterm_prefilt", true);
    let out = calcule(&t, &st);
    assert!(!out.psd);
    let f = &out.files[0];
    assert!(f.spectra.contains_key(&("dterm", 0)));
    assert!(!f.spectra.contains_key(&("dterm", 2)), "pas de terme D sur le yaw");
    assert!(!f.dterm_axes[2] && f.dterm_axes[0] && f.dterm_axes[1]);
    // l'amplitude n'est pas décalée de 6,5 dB
    let log = &st.logs[0].log;
    let sel = st.logs[0].sel();
    let x: Vec<f64> = log.d_term.row(0)[sel].iter().map(|&v| f64::from(v)).collect();
    let ref_ = spectrum(&x, log.fs, Mode::Amp, 0.5, 0, Some(&[])).unwrap();
    assert_eq!(f.spectra[&("dterm", 0)].1, ref_.values);
}

#[test]
fn passer_en_amplitude_change_l_echelle_y() {
    let mut t = SpectralTool::new();
    t.set_y_range(-10.0, 5.0);
    t.set_psd(false);
    assert!(!t.options().psd);
    t.set_psd(true);
    // retour en PSD : échelle par défaut de la PSD, comme `_on_mode_toggled`
    let st = etat();
    assert!(calcule(&t, &st).psd);
}

#[test]
fn le_multiplicateur_est_sans_effet_avec_l_erpm() {
    let st = etat();
    let a = calcule(&SpectralTool::new(), &st);
    let mut o = SpectralTool::new().options();
    o.multiplier = 7.0;
    let b = SpectralTool::compute(o, st.inputs(&[0])).unwrap();
    assert_eq!(a.files[0].motor_sorted, b.files[0].motor_sorted);
}

#[test]
fn le_diagnostic_du_bruit_est_calcule_sur_demande() {
    let st = etat();
    let mut t = SpectralTool::new();
    t.set_diagnosis(true);
    let out = calcule(&t, &st);
    let nr = out.files[0].diagnosis.as_ref().expect("diagnostic demandé");
    let axes: Vec<&str> = nr.gyro.iter().map(|a| a.axis.as_str()).collect();
    assert_eq!(axes, ["Roll", "Pitch", "Yaw"]);
    assert_eq!(nr.dterm.len(), 2);
    assert!(
        nr.gyro.iter().all(|a| a.reduction_db > 0.0),
        "les filtres baissent le bruit"
    );
    assert!(!nr.strategy.is_empty());
}

#[test]
fn les_courbes_en_s_et_les_plages_du_filtre_rpm() {
    // part des valeurs ≤ f : 0 avant la première, 1 après la dernière, × rang de l'harmonique
    let f = [0.0, 50.0, 100.0, 150.0, 250.0];
    let s = s_curve(&[100.0, 200.0], 1.0, &f, -50.0, 40.0);
    let y: Vec<f64> = s.iter().map(|p| p[1]).collect();
    assert_eq!(y, [-50.0, -50.0, -30.0, -30.0, -10.0]);
    let s2 = s_curve(&[100.0, 200.0], 2.0, &f, 0.0, 1.0);
    assert_eq!(s2.iter().map(|p| p[1]).collect::<Vec<_>>(), [0.0, 0.0, 0.0, 0.0, 0.5]);
    assert_eq!(s2[2][0], 100.0);

    assert_eq!(region_alpha(100), 35);
    assert_eq!(region_alpha(25), 16);
    assert_eq!(region_alpha(0), 10);

    let st = etat();
    let mut fs = calcule(&SpectralTool::new(), &st).files.remove(0);
    let r = rpm_regions(&fs);
    assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), [1, 2, 3]); // poids 100, 50, 70 : tous actifs
    assert!(
        r.iter()
            .all(|(h, a, b, _)| b > a && *a >= 100.0 * (*h == 1) as i64 as f64)
    );
    assert_eq!(r[1].3, 50);
    // un poids nul retire l'harmonique ; filtre RPM inactif : aucune plage
    fs.rpm_weights = vec![100, 0, 70];
    assert_eq!(rpm_regions(&fs).iter().map(|x| x.0).collect::<Vec<_>>(), [1, 3]);
    fs.rpm_harmonics = 0;
    assert!(rpm_regions(&fs).is_empty());
    let vide = FileSpectra {
        motor_sorted: vec![],
        ..fs
    };
    assert!(rpm_regions(&vide).is_empty());
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
    h.state_mut().screen = Screen::Tool(0);
    attendre(&mut h);
    h
}

/// Laisse finir le calcul de la page affichée, puis dessine.
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
fn la_page_affiche_les_deux_colonnes_et_bascule_vers_le_bruit_moteur() {
    let mut h = banc();
    h.get_by_label("Spectre complet");
    h.get_by_label("Sous 100 Hz");
    // l'outil est un onglet de la barre, mis en valeur
    h.get_by_label("Analyse spectrale");
    // « Bruit moteur » : choix dans la liste déroulante
    h.get_by_value("< 100 Hz").click();
    h.run_steps(2);
    h.get_by_label("Bruit moteur").click();
    h.run_steps(4);
    h.get_by_label("Bruit moteur");
    assert!(h.query_by_label("Sous 100 Hz").is_none());
}

#[test]
fn le_diagnostic_apparait_a_cote_des_courbes_et_disparait_sans_recalcul() {
    let mut h = banc();
    assert!(h.query_by_label("1. Conditions du log").is_none());
    h.get_by_label("Diagnostic du bruit").click();
    attendre(&mut h);
    for t in ["1. Conditions du log", "2. Gyro : bruit avant et après filtrage"] {
        h.get_by_label(t);
    }
    assert!(h.query_all_by_label_contains("Stratégie de filtrage").count() >= 1);
    h.get_by_label("Diagnostic du bruit").click();
    attendre(&mut h);
    assert!(h.query_by_label("1. Conditions du log").is_none());
}

#[test]
fn tous_les_choix_d_affichage_se_dessinent_sans_panique() {
    let mut h = banc();
    // liste des harmoniques : la valeur affichée est « 1re, 2e, 3e » au départ ; on clique le bouton qui porte la valeur courante
    let mut courant = "1re, 2e, 3e";
    for choix in [
        "Plage du filtre RPM",
        "1re harmonique",
        "Harmoniques : aucune",
        "1re, 2e, 3e",
    ] {
        h.get_by_value(courant).click();
        h.run_steps(2);
        h.get_by_label(choix).click();
        h.run_steps(4);
        courant = choix;
    }
    // notchs et moteurs
    h.get_by_label("Notchs dynamiques").click();
    h.get_by_label("M1/5").click();
    h.run_steps(4);
}
