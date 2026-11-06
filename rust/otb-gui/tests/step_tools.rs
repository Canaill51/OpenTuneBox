//! Outil « Réponse indicielle » (port de `test_gui_step.py::test_step_window` et `…test_segment_metrics_matches_definition`), sur un vrai
//! log de 5 s (`tests/data/jeno5_5s.bbl`) : le roll n'a aucune fenêtre exploitable (0 réponse), le pitch en a 9, la boucle y est approchée.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_analysis::open_loop;
use otb_gui::app::{App, Screen};
use otb_gui::state::AppState;
use otb_gui::theme::{CLAIR, file_color};
use otb_gui::tool::Tool;
use otb_gui::tools::step::{
    Marker, Options, StepOutput, StepTool, interp, loop_marker, metric_value, metric_y_range, raw_segments, spread,
    text_lines,
};
use otb_gui::tools::step_common::{FileData, Split, StepOpts, mode_text, pid_text};

fn log5s() -> otb_log::flightlog::FlightLog {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    otb_log::flightlog::load(&p).unwrap().remove(0)
}

fn etat() -> AppState {
    let mut st = AppState::new();
    st.add(log5s());
    st
}

fn calcule(opts: StepOpts, st: &AppState) -> StepOutput {
    StepTool::compute(Options { calc: opts }, st.inputs(&[0])).unwrap()
}

// ------------------------------------------------------------------ calcul

#[test]
fn le_calcul_par_defaut_donne_les_reponses_des_trois_axes_et_le_diagnostic() {
    let st = etat();
    let out = calcule(StepOpts::default(), &st);
    assert_eq!(out.files.len(), 1);
    let f = &out.files[0];
    assert_eq!(f.label, "jeno5_5s");
    let d = &f.data;
    assert_eq!(d.axes.len(), 3);
    assert_eq!((d.axes[0].res.n, d.axes[1].res.n, d.axes[2].res.n), (0, 9, 0));
    let pitch = &d.axes[1];
    assert_eq!((pitch.seg_peak.len(), pitch.seg_latency.len()), (9, 9));
    assert!(pitch.res.metrics.peak > 1.0 && pitch.res.metrics.latency_ms > 5.0 && pitch.res.metrics.latency_ms < 60.0);
    assert!(pitch.sp_delay_ms.is_finite());
    // consigne trop peu mobile (écart-type < 5) : pas de retard mesuré ; le pitch en a un
    assert!(d.axes[0].sp_delay_ms.is_nan() && d.axes[2].sp_delay_ms.is_nan());
    assert_eq!(d.mode, 2, "réglage simplifié en RPY");
    assert_eq!(mode_text(d.mode), "RPY");
    assert_eq!(pid_text(&d.pids[0]), "58,72,39,39,156");
    // un seul vol : le diagnostic ne reconnaît aucune série, mais juge les conditions de vol
    assert!(out.report.blocks.is_empty());
    assert!(!out.report.conditions.is_empty());
    // le nom du vol du diagnostic est celui du fichier affiché (il apparaît dans les constats), jamais celui rangé dans le cache
    assert!(d.flight.label.is_empty(), "le libellé n'est pas rangé dans le cache");
    assert!(
        format!("{:?}", out.report.conditions).contains("jeno5_5s"),
        "{:?}",
        out.report.conditions
    );
}

#[test]
fn le_lissage_du_gyro_change_les_reponses_et_la_separation_les_fenetres() {
    let st = etat();
    let base = calcule(StepOpts::default(), &st);
    let lisse = calcule(
        StepOpts {
            smooth: 40,
            ..StepOpts::default()
        },
        &st,
    );
    let (a, b) = (&base.files[0].data.axes[1].res, &lisse.files[0].data.axes[1].res);
    // le choix des fenêtres vient de la consigne, mais le contrôle de qualité (sur le gyro lissé) en retient d'autres
    assert!(
        a.n != b.n || a.mean.iter().zip(&b.mean).any(|(x, y)| (x - y).abs() > 1e-3),
        "le lissage doit changer le résultat"
    );
    assert!(b.n > 0 && b.mean.iter().all(|v| v.is_finite()));
    // consigne max 500 °/s : les fenêtres plus vives disparaissent
    let petits = calcule(
        StepOpts {
            split: Split::Small,
            ..StepOpts::default()
        },
        &st,
    );
    let grands = calcule(
        StepOpts {
            split: Split::Large,
            ..StepOpts::default()
        },
        &st,
    );
    // fenêtre de 1 s : le nombre de fenêtres change ; correction du palier : la moyenne change
    let court = calcule(
        StepOpts {
            seg_s: 1.0,
            ..StepOpts::default()
        },
        &st,
    );
    assert_ne!(
        court.files[0].data.axes[1].res.n, base.files[0].data.axes[1].res.n,
        "la fenêtre est transmise au calcul"
    );
    let corr = calcule(
        StepOpts {
            y_corr: true,
            ..StepOpts::default()
        },
        &st,
    );
    assert!(
        base.files[0].data.axes[1]
            .res
            .mean
            .iter()
            .zip(&corr.files[0].data.axes[1].res.mean)
            .any(|(x, y)| x != y),
        "la correction du palier est transmise au calcul"
    );
    let n = |o: &StepOutput| o.files[0].data.axes[1].res.n;
    assert!(n(&petits) <= n(&base) && n(&grands) <= n(&base));
    assert_eq!(
        n(&petits) + n(&grands),
        n(&base),
        "petits + grands = tous (pas de recouvrement)"
    );
}

#[test]
fn le_cache_sert_au_second_calcul_mais_ni_apres_un_changement_de_periode_ni_de_reglage() {
    let mut st = etat();
    let data = |o: &StepOutput| o.files[0].data.clone();
    let a = calcule(StepOpts::default(), &st);
    let b = calcule(StepOpts::default(), &st);
    assert!(
        Arc::ptr_eq(&data(&a), &data(&b)),
        "même réglage et même période : le résultat est repris"
    );
    assert_ne!(a.stamp, b.stamp, "mais chaque calcul est distinct pour l'affichage");
    let c = calcule(
        StepOpts {
            y_corr: true,
            ..StepOpts::default()
        },
        &st,
    );
    assert!(!Arc::ptr_eq(&data(&a), &data(&c)), "un autre réglage de calcul");
    // période raccourcie : la clé change (et le cache est vidé)
    let (t0, t1) = st.logs[0].period;
    st.set_period(0, t0 + 0.5, t1 - 0.5);
    let d = calcule(StepOpts::default(), &st);
    assert!(!Arc::ptr_eq(&data(&a), &data(&d)));
    // un calcul lancé avant le changement de période (entrées copiées avant) ne sert pas après : sa clé contient l'ancienne période
    let avant = st.inputs(&[0]);
    st.set_period(0, t0, t1);
    let e = StepTool::compute(
        Options {
            calc: StepOpts::default(),
        },
        avant,
    )
    .unwrap();
    let f = calcule(StepOpts::default(), &st);
    assert!(!Arc::ptr_eq(&data(&e), &data(&f)));
}

#[test]
fn le_libelle_est_celui_du_fichier_et_la_couleur_le_rang_dans_la_selection() {
    let mut st = etat();
    let a = calcule(StepOpts::default(), &st);
    st.add(log5s());
    st.remove(0);
    let b = calcule(StepOpts::default(), &st);
    assert_eq!(a.files[0].data.axes[1].res.n, b.files[0].data.axes[1].res.n);
    assert_eq!(b.files[0].label, st.logs[0].label);
    // la couleur d'une courbe est sa place dans la sélection, pas la couleur du log (2026-11-06)
    assert_eq!(b.files[0].color, file_color(0));
    st.add(log5s());
    let deux = StepTool::compute(
        Options {
            calc: StepOpts::default(),
        },
        st.inputs(&[1, 0]),
    )
    .unwrap();
    assert_eq!(
        deux.files[0].label, st.logs[1].label,
        "le log coché en premier est tracé en premier"
    );
    assert_eq!(deux.files[0].color, file_color(0));
    assert_eq!(deux.files[1].color, file_color(1));
}

// ------------------------------------------------------------------ fonctions pures

fn donnees() -> FileData {
    (*calcule(StepOpts::default(), &etat()).files[0].data).clone()
}

#[test]
fn la_dispersion_est_l_ecart_type_des_valeurs_finies() {
    assert_eq!(spread(&[]), 0.0);
    assert_eq!(spread(&[3.0]), 0.0);
    assert_eq!(spread(&[f64::NAN, 3.0]), 0.0, "une seule valeur finie");
    assert!(
        (spread(&[1.0, 3.0]) - 1.0).abs() < 1e-15,
        "écart-type de population (ddof = 0)"
    );
    assert!((spread(&[1.0, f64::NAN, 3.0, f64::INFINITY]) - 1.0).abs() < 1e-15);
}

#[test]
fn l_interpolation_est_celle_de_numpy_et_bornee() {
    let (x, y) = ([0.0, 2.0, 4.0], [1.0, 3.0, 2.0]);
    assert_eq!(interp(&x, &y, -1.0), 1.0);
    assert_eq!(interp(&x, &y, 9.0), 2.0);
    assert_eq!(interp(&x, &y, 1.0), 2.0);
    assert_eq!(interp(&x, &y, 2.0), 3.0);
    assert_eq!(interp(&x, &y, 3.0), 2.5);
    assert!(interp(&[], &[], 1.0).is_nan());
}

#[test]
fn les_metriques_se_lisent_au_bon_endroit_avec_leur_barre_d_erreur() {
    let mut d = donnees();
    let a = d.axes[1].clone();
    let m = a.res.metrics;
    assert_eq!(metric_value(&d, 1, "peak"), (m.peak, spread(&a.seg_peak)));
    assert_eq!(metric_value(&d, 1, "latency"), (m.latency_ms, spread(&a.seg_latency)));
    assert_eq!(metric_value(&d, 1, "sp_delay"), (a.sp_delay_ms, 0.0));
    assert_eq!(metric_value(&d, 1, "rise"), (m.rise_ms, 0.0));
    // axe sans réponse, ou inexistant : NaN sans barre
    for ax in [0, 5] {
        let (v, e) = metric_value(&d, ax, "peak");
        assert!(v.is_nan() && e == 0.0);
    }
    // marges de la boucle : valeur et écart-type jackknife (0 s'il n'est pas fini)
    d.axes[1].res.loop_.phase_margin_deg = 42.0;
    d.axes[1].res.loop_.gain_margin_db = 9.5;
    d.axes[1].res.loop_.crossover_hz = 77.0;
    d.axes[1].res.loop_.sensitivity_peak_db = 3.0;
    d.axes[1].res.loop_.sensitivity_peak_hz = 55.0;
    d.axes[1].res.loop_.bandwidth_hz = 120.0;
    d.axes[1].res.loop_.std = vec![("phase_margin_deg", 2.5), ("gain_margin_db", f64::NAN)];
    assert_eq!(metric_value(&d, 1, "phase_margin"), (42.0, 2.5));
    assert_eq!(
        metric_value(&d, 1, "gain_margin"),
        (9.5, 0.0),
        "écart-type NaN -> pas de barre"
    );
    assert_eq!(
        metric_value(&d, 1, "crossover"),
        (77.0, 0.0),
        "pas d'écart-type calculé"
    );
    assert_eq!(metric_value(&d, 1, "sens_peak"), (3.0, 0.0));
    assert_eq!(metric_value(&d, 1, "sens_hz"), (55.0, 0.0));
    assert_eq!(metric_value(&d, 1, "bandwidth"), (120.0, 0.0));
    d.axes[1].res.metrics.ripple = 0.1;
    d.axes[1].res.metrics.ripple_std = f64::NAN;
    assert_eq!(metric_value(&d, 1, "ripple"), (0.1, 0.0));
    d.axes[1].res.metrics.ripple_std = 0.02;
    assert_eq!(metric_value(&d, 1, "ripple"), (0.1, 0.02));
}

#[test]
fn la_forme_du_point_dit_si_la_marge_est_mesuree_approchee_ou_un_minorant() {
    let mut d = donnees();
    assert_eq!(d.axes[1].res.loop_.method, open_loop::APPROX);
    assert_eq!(loop_marker(&d, 1, "phase_margin"), Marker::Approx);
    assert_eq!(
        loop_marker(&d, 1, "latency"),
        Marker::Measured,
        "pas une marge : toujours plein"
    );
    assert_eq!(
        loop_marker(&d, 1, "bandwidth"),
        Marker::Measured,
        "comme Python : la bande passante n'est pas une marge"
    );
    assert_eq!(loop_marker(&d, 0, "phase_margin"), Marker::Measured, "axe sans réponse");
    d.axes[1].res.loop_.method = open_loop::MEASURED;
    assert_eq!(loop_marker(&d, 1, "phase_margin"), Marker::Measured);
    d.axes[1].res.loop_.sensitivity_peak_at_edge = true;
    assert_eq!(loop_marker(&d, 1, "sens_peak"), Marker::Lower);
    assert_eq!(
        loop_marker(&d, 1, "sens_hz"),
        Marker::Measured,
        "seul le pic de sensibilité est un minorant"
    );
}

#[test]
fn les_bornes_y_des_metriques_gardent_un_quart_de_marge_et_le_pic_de_sensibilite_voit_0_et_7() {
    let n = f64::NAN;
    assert_eq!(metric_y_range(&[n, n], &[0.0, 0.0], "peak"), None);
    // une valeur : étendue nulle, repli 10 % de |hi| puis 0,02
    let (lo, hi) = metric_y_range(&[10.0], &[0.0], "peak").unwrap();
    assert!(
        (lo - (10.0 - 0.25)).abs() < 1e-12 && (hi - (10.0 + 0.25)).abs() < 1e-12,
        "{lo} {hi}"
    );
    let (lo, hi) = metric_y_range(&[0.0], &[0.0], "peak").unwrap();
    assert!((lo + 0.005).abs() < 1e-12 && (hi - 0.005).abs() < 1e-12, "{lo} {hi}");
    // barres d'erreur comprises ; une valeur NaN est ignorée
    let (lo, hi) = metric_y_range(&[1.0, n, 3.0], &[0.5, 9.0, 1.0], "latency").unwrap();
    assert!((lo - (0.5 - 0.25 * 3.5)).abs() < 1e-12 && (hi - (4.0 + 0.25 * 3.5)).abs() < 1e-12);
    // pic de sensibilité : toujours 0 à 7 au moins
    let (lo, hi) = metric_y_range(&[3.0], &[0.0], "sens_peak").unwrap();
    assert!((lo - (0.0 - 0.25 * 7.0)).abs() < 1e-12 && (hi - (7.0 + 0.25 * 7.0)).abs() < 1e-12);
}

#[test]
fn le_bloc_de_texte_donne_gains_pic_latence_en_virgule_decimale_ou_donnees_insuffisantes() {
    let out = calcule(StepOpts::default(), &etat());
    let l = text_lines(&out.files, 1, &CLAIR);
    assert_eq!(l.len(), 2);
    assert_eq!(l[0].0, "P, I, D, Dm, F · pic · latence · cassure");
    assert!(
        l[1].0.starts_with("1) 67,84,48,48,178 (n=9) · 1,17 · 22,9 ms · +0,297"),
        "{}",
        l[1].0
    ); // gains du pitch
    assert!(!l[1].0.contains('.'), "virgule décimale : {}", l[1].0);
    assert_eq!(l[1].1, out.files[0].color);
    assert_eq!(text_lines(&out.files, 0, &CLAIR)[1].0, "1) données insuffisantes");
}

// ------------------------------------------------------------------ dans la fenêtre

fn banc() -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 950.0))
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
    let i = h
        .state()
        .pages
        .iter()
        .position(|p| p.title() == "Réponse indicielle")
        .expect("page enregistrée");
    h.state_mut().screen = Screen::Tool(i);
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
fn la_page_affiche_les_graphiques_le_diagnostic_et_les_curseurs() {
    let mut h = banc();
    h.run(); // contrôle d'accessibilité de kittest (L-068)
    assert!(
        h.query_all_by_label("Réponse indicielle").count() >= 2,
        "onglet et titre de colonne"
    );
    h.get_by_label("Diagnostic du réglage");
    h.get_by_label("Conditions des vols");
    h.get_by_label("Curseurs PID Betaflight");
    h.get_by_label("MODE");
    // les deux colonnes de métriques par défaut : latence et retard consigne -> gyro
    assert!(h.query_by_label("Latence (50 %)").is_some() && h.query_by_label("Retard consigne->gyro").is_some());
    // pas de série reconnue avec un seul vol
    h.get_by_label_contains("Aucune série reconnue");
}

#[test]
fn chaque_colonne_et_chaque_option_d_affichage_se_dessine() {
    let mut h = banc();
    for i in 0..otb_gui::tools::step::METRICS.len() {
        let titre = otb_gui::tools::step::metric_title(i);
        let titre = titre.as_str();
        let col = if i % 2 == 0 { "Colonne 1" } else { "Colonne 2" };
        h.get_by_label(col).click();
        h.run_steps(2);
        if h.query_by_label(titre).is_none() {
            let n_sel = h.query_all_by_label_contains("Pic").count();
            let n_lat = h.query_all_by_label_contains("Latence").count();
            let n_mar = h.query_all_by_label_contains("Marge").count();
            panic!("{titre} absent (i={i}) ; Pic {n_sel} Latence {n_lat} Marge {n_mar}");
        }
        h.get_by_label(titre).click();
        attendre(&mut h);
        if h.query_by_label(titre).is_none() {
            let v1 = h.get_by_label("Colonne 1").value();
            let v2 = h.get_by_label("Colonne 2").value();
            panic!("après le clic : {titre} introuvable (i={i}) ; colonne 1 = {v1:?}, colonne 2 = {v2:?}");
        }
    }
    h.get_by_label("Brutes (toutes les fenêtres)").click();
    h.get_by_label("± écart-type").click();
    attendre(&mut h);
    h.get_by_value("non").click();
    h.run_steps(2);
    h.get_by_label("Pitch").click();
    attendre(&mut h);
    h.get_by_value("250 ms").click();
    h.run_steps(2);
    h.get_by_label("150 ms").click();
    attendre(&mut h);
    h.run();
}

#[test]
fn un_reglage_de_calcul_relance_le_calcul_et_un_reglage_d_affichage_non() {
    use otb_gui::tool::ToolPage;
    let ctx = egui::Context::default();
    let st = etat();
    let mut page = ToolPage::new(StepTool::new());
    let attendre = |p: &mut ToolPage<StepTool>| {
        let debut = Instant::now();
        loop {
            p.update(&ctx, &st);
            if !p.busy() {
                return;
            }
            assert!(debut.elapsed() < Duration::from_secs(60), "calcul jamais terminé");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    attendre(&mut page);
    let stamp = |p: &ToolPage<StepTool>| p.result().expect("résultat").stamp;
    let s0 = stamp(&page);
    // affichage : ni recalcul ni nouveau résultat
    page.tool.set_display(true, true);
    page.tool.set_ymax(2.0);
    page.tool.set_xmax(150);
    page.tool.set_expand(2);
    page.tool.set_metrics("peak", "rise");
    attendre(&mut page);
    assert_eq!(stamp(&page), s0, "un réglage d'affichage ne relance pas le calcul");
    // calcul : nouveau résultat
    page.tool.set_calc(StepOpts {
        smooth: 20,
        ..StepOpts::default()
    });
    attendre(&mut page);
    assert_ne!(stamp(&page), s0, "un réglage de calcul relance le calcul");
    assert_eq!(page.tool.shown_axes(), [1], "« Agrandir » = un seul axe");
}

#[test]
fn les_caracteres_des_textes_existent_dans_les_polices() {
    // le banc refuse un caractère absent des polices (L-068) : on les écrit tous une fois
    let mut h = Harness::builder()
        .with_context_setup(otb_gui::app::setup)
        .build_ui(|ui| {
            ui.label("≥ ≤ ° · ± × « » – — … • ≈ ÷ Δ");
        });
    h.run();
}

// ================================================================== Curseurs PID

use otb_gui::tool::Page;
use otb_gui::tools::pid_slider::{
    FileSliders, PidSliderTool, SLIDERS, cli_lines, filter_rows, info_lines, pid_cell, sim_extra, sim_from_ref,
    sim_pids, snap,
};

fn fichier() -> FileSliders {
    let st = etat();
    let out = PidSliderTool::compute(Default::default(), st.inputs(&[0])).unwrap();
    out.files.into_iter().next().unwrap()
}

fn pids_texte(p: &[otb_log::settings::AxisPid]) -> Vec<String> {
    p.iter().map(pid_text).collect()
}

#[test]
fn la_simulation_repart_des_curseurs_de_l_en_tete() {
    let f = fichier();
    assert!(f.view.from_header && f.view.consistent);
    assert_eq!((f.mode, f.family()), (2, "2026.6"));
    let sim = sim_from_ref(&f);
    // d_gain, pi_gain, ff_gain, dmax_gain, i_gain, pitch_d, pitch_pi, master (en-tête du log, mesuré avec Python)
    assert_eq!(sim.values, [100, 100, 100, 0, 70, 110, 110, 130]);
    assert_eq!(
        (sim.mode, sim.gyro_mult, sim.dterm_mult, sim.gyro_on, sim.dterm_on),
        (2, 150, 140, true, true)
    );
    assert_eq!(
        SLIDERS.map(|s| s.0),
        [
            "d_gain",
            "pi_gain",
            "ff_gain",
            "dmax_gain",
            "i_gain",
            "pitch_d",
            "pitch_pi",
            "master"
        ]
    );
}

#[test]
fn les_pid_resultants_sont_ceux_du_firmware_et_suivent_les_curseurs() {
    let f = fichier();
    let mut sim = sim_from_ref(&f);
    // tel quel : les PID du fichier (le log suit ses curseurs)
    assert_eq!(
        pids_texte(&sim_pids(&sim, &f).unwrap()),
        ["58,72,39,39,156", "67,84,48,48,178", "58,72,0,0,156"]
    );
    // Master 50 % : valeurs données par Python (`pids_from_sliders`)
    sim.values[7] = 50;
    assert_eq!(
        pids_texte(&sim_pids(&sim, &f).unwrap()),
        ["22,28,15,15,60", "25,32,18,18,68", "22,28,0,0,60"]
    );
    // mode OFF : les PID du fichier restent
    let mut off = sim_from_ref(&f);
    off.mode = 0;
    assert_eq!(
        pids_texte(&sim_pids(&off, &f).unwrap()),
        ["58,72,39,39,156", "67,84,48,48,178", "58,72,0,0,156"]
    );
}

#[test]
fn les_commandes_cli_sont_celles_de_python_avec_leur_en_tete() {
    let f = fichier();
    let mut sim = sim_from_ref(&f);
    let l = cli_lines(&sim, &f).unwrap();
    assert_eq!(l[0], "# OpenTuneBox — curseurs pour jeno5_5s (Betaflight 2026.6)");
    assert_eq!(
        l[1..],
        [
            "set simplified_pids_mode = RPY",
            "set simplified_master_multiplier = 130",
            "set simplified_i_gain = 70",
            "set simplified_d_gain = 100",
            "set simplified_pi_gain = 100",
            "set simplified_d_max_gain = 0",
            "set simplified_feedforward_gain = 100",
            "set simplified_pitch_d_gain = 110",
            "set simplified_pitch_pi_gain = 110",
            "set simplified_dterm_filter = ON",
            "set simplified_dterm_filter_multiplier = 140",
            "set simplified_gyro_filter = ON",
            "set simplified_gyro_filter_multiplier = 150",
            "simplified_tuning apply",
            "save",
        ]
    );
    sim.mode = 0;
    sim.dterm_mult = 200;
    let l = cli_lines(&sim, &f).unwrap();
    assert_eq!(l[1], "# mode OFF : les curseurs PID ne seront pas appliqués");
    assert_eq!(l[2], "set simplified_pids_mode = OFF");
    assert!(l.contains(&"set simplified_dterm_filter_multiplier = 200".to_string()));
}

fn texte(row: &[(String, bool)]) -> String {
    row.iter().map(|(t, _)| t.as_str()).collect()
}

#[test]
fn les_filtres_montrent_les_changements_en_gras_et_ignorent_ceux_qui_sont_inactifs() {
    let f = fichier();
    let mut sim = sim_from_ref(&f);
    // filtres actuels du log : gyro LPF2 750 Hz ; D LPF1 dynamique 105–210, LPF2 210 (Python : `filters_from_sliders`)
    let rows = filter_rows(&sim, f.settings());
    assert_eq!(texte(&rows[0]), "Gyro : LPF2 750 Hz");
    assert_eq!(texte(&rows[1]), "D : LPF1 dyn min 105, max 210, LPF2 210 Hz");
    assert!(
        rows[0][0].1 && !rows[0][1].1,
        "le nom du filtre est en gras, pas le reste"
    );
    // D à 200 % : 105 -> 150, 210 -> 300, LPF2 210 -> 300 ; seules les nouvelles valeurs sont en gras
    sim.dterm_mult = 200;
    let rows = filter_rows(&sim, f.settings());
    assert_eq!(
        texte(&rows[1]),
        "D : LPF1 dyn min 105->150, max 210->300, LPF2 210->300 Hz"
    );
    let gras: Vec<&str> = rows[1].iter().filter(|(_, g)| *g).map(|(t, _)| t.as_str()).collect();
    assert_eq!(gras, ["D", "150", "300", "300"]);
    // curseur inactif
    sim.gyro_on = false;
    let rows = filter_rows(&sim, f.settings());
    assert_eq!(texte(&rows[0]), "Gyro : curseur inactif -> filtres inchangés");
}

#[test]
fn une_case_du_tableau_donne_l_ecart_et_son_signe() {
    assert_eq!(pid_cell(17, 17), ("17".to_string(), 0));
    assert_eq!(pid_cell(30, 17), ("30  (+13)".to_string(), 1));
    assert_eq!(pid_cell(5, 17), ("5  (-12)".to_string(), -1));
}

#[test]
fn l_arrondi_au_cinq_pour_cent_se_fait_au_pair_et_est_borne() {
    // Python : 5 * round(v / 5), borné à 0–200
    assert_eq!(
        [snap(7.5), snap(2.5), snap(12.5), snap(17.4), snap(17.6)],
        [10, 0, 10, 15, 20]
    );
    assert_eq!([snap(-3.0), snap(999.0), snap(0.0), snap(200.0)], [0, 200, 0, 200]);
}

#[test]
fn le_texte_d_information_donne_origine_pid_et_positions() {
    let f = fichier();
    let l = info_lines(std::slice::from_ref(&f));
    assert_eq!(l.len(), 4);
    assert_eq!(l[0].0, "PID actuels (P, I, D, Dmax, F)");
    assert_eq!(l[1].0, "1) jeno5_5s  BF 2026.6, mode RPY, curseurs de l'en-tête");
    assert_eq!(l[2].0, "   R 58,72,39,39,156  |  P 67,84,48,48,178  |  Y 58,72,0,0,156");
    assert_eq!(
        l[3].0,
        "   Damping 1.00, Tracking 1.00, Stick Response 1.00, Dynamic Damping 0.00, Drift - Wobble 0.70, \
         Pitch Damping 1.10, Pitch Tracking 1.10, Master Multiplier 1.30"
    );
    assert_eq!(l[1].1, Some(f.color));
}

#[test]
fn la_simulation_ne_repart_que_si_le_fichier_de_reference_change() {
    let st = etat();
    let sortie = |st: &AppState| PidSliderTool::compute(Default::default(), st.inputs(&[0])).unwrap();
    let mut t = PidSliderTool::new();
    let out = sortie(&st);
    t.sync_reference(&out);
    t.sim_mut().unwrap().values[7] = 50;
    // même fichier, nouveau calcul (autre sortie, même log) : la simulation de l'utilisateur est gardée
    t.sync_reference(&sortie(&st));
    assert_eq!(t.sim().unwrap().values[7], 50);
    // autre log (même libellé mais autre objet) : on repart du fichier
    let mut autre = AppState::new();
    autre.add(log5s());
    t.sync_reference(&sortie(&autre));
    assert_eq!(t.sim().unwrap().values[7], 130);
}

// ------------------------------------------------------------------ dans la fenêtre

struct Cadre {
    page: otb_gui::tool::ToolPage<PidSliderTool>,
    st: AppState,
}

fn cadre() -> Harness<'static, Cadre> {
    let mut st = etat();
    st.add(log5s());
    st.remove(1);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 950.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(
            |ui, c: &mut Cadre| c.page.ui(ui, &mut c.st, &CLAIR),
            Cadre {
                page: otb_gui::tool::ToolPage::new(PidSliderTool::new()),
                st,
            },
        );
    let debut = Instant::now();
    loop {
        h.run_steps(2);
        if !h.state().page.busy() && h.state().page.result().is_some() {
            break;
        }
        assert!(debut.elapsed() < Duration::from_secs(30), "calcul jamais terminé");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(4);
    h
}

#[test]
fn la_page_affiche_le_graphique_le_tableau_et_les_commandes() {
    let mut h = cadre();
    h.run(); // contrôle d'accessibilité (L-068)
    h.get_by_label("Simulation « et si… »");
    h.get_by_label("Curseurs PID Betaflight  (losange = simulation)");
    h.get_by_label("PID résultants (écart avec le fichier) :");
    h.get_by_label("PID actuels (P, I, D, Dmax, F)");
    let cli = h.get_by_label("Commandes CLI").value().expect("texte");
    assert!(
        cli.contains("simplified_tuning apply") && cli.contains("set simplified_master_multiplier = 130"),
        "{cli}"
    );
    // tableau : les PID du fichier, sans écart
    assert!(h.query_all_by_label("58").count() >= 1 && h.query_by_label("58  (+0)").is_none());
}

#[test]
fn bouger_un_curseur_change_le_tableau_et_les_commandes() {
    let mut h = cadre();
    h.state_mut().page.tool.sim_mut().unwrap().values[7] = 50; // Master 50 %
    h.run_steps(3);
    let cli = h.get_by_label("Commandes CLI").value().expect("texte");
    assert!(cli.contains("set simplified_master_multiplier = 50"), "{cli}");
    // P : roll 58 -> 22 et yaw 58 -> 22 (écart -36), pitch 67 -> 25 (écart -42) ; I : roll et yaw 72 -> 28 (-44)
    assert_eq!(h.query_all_by_label("22  (-36)").count(), 2);
    h.get_by_label("25  (-42)");
    assert_eq!(h.query_all_by_label("28  (-44)").count(), 2);
    // « Repartir du fichier » remet la simulation à l'état du log
    h.get_by_label("Repartir du fichier").click();
    h.run_steps(3);
    assert_eq!(h.state().page.tool.sim().unwrap().values[7], 130);
    assert!(h.query_by_label("22  (-36)").is_none());
}

// ------------------------------------------------------------------ briques pures (contrats avec Python)

#[test]
fn les_constantes_et_formules_des_briques_sont_celles_de_python() {
    use otb_gui::tools::step_common::{file_offset, header_mode};
    // SPLITS de `_step_common.py`
    assert_eq!(Split::All.bounds(), (20.0, None));
    assert_eq!(Split::Small.bounds(), (20.0, Some(500.0)));
    assert_eq!(Split::Large.bounds(), (500.0, None));
    // `int(sl.get("mode", 0) or 0)` : absent = 0 ; une valeur illisible (-1) reste -1
    assert_eq!(header_mode(&[]), 0);
    assert_eq!(header_mode(&[("master", 100), ("mode", 1)]), 1);
    assert_eq!(header_mode(&[("mode", -1)]), -1);
    // décalage vertical des fichiers d'un même curseur : 0,36 × (0,5 − k / (n − 1))
    assert_eq!(file_offset(0, 1), 0.0);
    assert!((file_offset(0, 2) - 0.18).abs() < 1e-15 && (file_offset(1, 2) + 0.18).abs() < 1e-15);
    assert!(file_offset(1, 3).abs() < 1e-15);
    // AxisPID.text() : P,I,D,Dmax,F dans cet ordre
    let p = otb_log::settings::AxisPid {
        p: 1,
        i: 2,
        d: 3,
        dmax: 4,
        f: 5,
    };
    assert_eq!(pid_text(&p), "1,2,3,4,5");
}

#[test]
fn les_bornes_des_graphiques_ne_sont_remises_que_si_un_reglage_d_affichage_ou_le_calcul_change() {
    let mut t = StepTool::new();
    let k0 = t.view_key(1);
    assert_eq!(k0, t.view_key(1), "stable");
    assert_ne!(k0, t.view_key(2), "un nouveau calcul remet les bornes");
    t.set_ymax(2.0);
    assert_ne!(k0, t.view_key(1), "Y max");
    let mut t = StepTool::new();
    t.set_xmax(150);
    assert_ne!(k0, t.view_key(1), "temps affiché");
    let mut t = StepTool::new();
    t.set_expand(1);
    assert_ne!(k0, t.view_key(1), "agrandir");
    let mut t = StepTool::new();
    t.set_metrics("peak", "rise");
    assert_ne!(k0, t.view_key(1), "colonnes");
}

// ------------------------------------------------------------------ simulation : en-têtes incomplets

fn fichier_modifie(modif: impl FnOnce(&mut std::collections::BTreeMap<String, String>)) -> FileSliders {
    let mut log = log5s();
    modif(&mut log.settings.raw);
    let mut st = AppState::new();
    st.add(log);
    let out = PidSliderTool::compute(Default::default(), st.inputs(&[0])).unwrap();
    out.files.into_iter().next().unwrap()
}

#[test]
fn un_curseur_absent_de_l_en_tete_vaut_100_et_un_multiplicateur_a_zero_reste_zero() {
    let f = fichier_modifie(|r| {
        r.remove("simplified_d_gain");
        r.insert("simplified_gyro_filter_multiplier".into(), "0".into());
    });
    assert!(f.view.from_header);
    let sim = sim_from_ref(&f);
    assert_eq!(sim.values[0], 100, "d_gain absent");
    assert_eq!(sim.values[7], 130, "les autres curseurs restent ceux de l'en-tête");
    assert_eq!(sim.gyro_mult, 0, "0 est une valeur valide (>= 0), pas « absent »");
    // avec ce multiplicateur, le LPF2 du gyro (750 Hz) tombe à 0
    let rows = filter_rows(&sim, f.settings());
    assert_eq!(texte(&rows[0]), "Gyro : LPF2 750->0 Hz");
}

#[test]
fn sans_curseurs_dans_l_en_tete_la_simulation_part_de_leur_estimation_depuis_les_pid_en_mode_rpy() {
    use otb_analysis::sliders::sliders_from_pids;
    let f = fichier_modifie(|r| {
        r.insert("simplified_pids_mode".into(), "0".into());
    });
    assert!(!f.view.from_header);
    let sim = sim_from_ref(&f);
    assert_eq!(sim.mode, 2, "réglage simplifié désactivé : on simule en RPY");
    let est = sliders_from_pids(&f.settings().pids, f.family());
    for (i, (k, _, _)) in SLIDERS.iter().enumerate() {
        let v = est.iter().find(|(n, _)| n == k).unwrap().1;
        assert_eq!(sim.values[i], snap(v), "{k}");
    }
}

#[test]
fn les_positions_de_la_simulation_sont_des_fractions_dans_l_ordre_des_categories() {
    let f = fichier();
    let sim = sim_from_ref(&f);
    let x = sim_extra(&sim);
    // Damping, Tracking, Stick Resp., Dynamic D., Drift-Wobble, Pitch D., Pitch T., Master
    let attendu = [1.0, 1.0, 1.0, 0.0, 0.7, 1.1, 1.1, 1.3];
    for (a, b) in x.iter().zip(attendu) {
        assert!((a - b).abs() < 1e-12, "{x:?}");
    }
}

#[test]
fn le_curseur_de_filtre_gyro_et_celui_du_d_sont_independants() {
    let f = fichier();
    let mut sim = sim_from_ref(&f);
    sim.gyro_on = false;
    let cli = cli_lines(&sim, &f).unwrap();
    assert!(cli.contains(&"set simplified_gyro_filter = OFF".to_string()), "{cli:?}");
    assert!(cli.contains(&"set simplified_dterm_filter = ON".to_string()), "{cli:?}");
    sim.gyro_on = true;
    sim.dterm_on = false;
    let cli = cli_lines(&sim, &f).unwrap();
    assert!(
        cli.contains(&"set simplified_gyro_filter = ON".to_string())
            && cli.contains(&"set simplified_dterm_filter = OFF".to_string())
    );
}

#[test]
fn une_valeur_de_l_en_tete_qui_n_est_pas_un_multiple_de_5_reste_telle_quelle_jusqu_a_ce_qu_on_y_touche() {
    // Python : le QSpinBox garde 103 (le pas de 5 ne recale rien) ; egui recalait la valeur sur la grille de 5 dès l'affichage
    let mut h = cadre();
    h.state_mut().page.tool.sim_mut().unwrap().values[0] = 103;
    h.state_mut().page.tool.sim_mut().unwrap().gyro_mult = 107;
    h.run_steps(4);
    let sim = h.state().page.tool.sim().unwrap();
    assert_eq!((sim.values[0], sim.gyro_mult), (103, 107));
    let cli = h.get_by_label("Commandes CLI").value().expect("texte");
    assert!(cli.contains("set simplified_d_gain = 103"), "{cli}");
}

#[test]
fn les_reponses_brutes_sont_reduites_plafonnees_et_coupees_aux_valeurs_non_finies() {
    use otb_gui::tools::step::MAX_RAW;
    let out = calcule(StepOpts::default(), &etat());
    let f = &out.files[0];
    // pitch : 9 réponses de 1 001 points -> 9 courbes de 250 points au plus, rien pour le roll (aucune réponse)
    assert!(raw_segments(f, 0).is_empty());
    let seg = raw_segments(f, 1);
    assert_eq!(seg.len(), 9);
    assert!(
        seg.iter().all(|s| s.len() <= 250 && s.len() > 100),
        "{:?}",
        seg.iter().map(Vec::len).collect::<Vec<_>>()
    );
    assert!(raw_segments(f, 7).is_empty(), "axe inexistant");
    // une valeur non finie coupe la courbe en deux (comme `connect="finite"`), sans point NaN
    let mut g = f.clone();
    let d = Arc::make_mut(&mut g.data);
    d.axes[1].res.responses[500] = f64::NAN; // réponse 0, instant 500
    let seg = raw_segments(&g, 1);
    assert_eq!(seg.len(), 10, "la réponse 0 est coupée en deux");
    assert!(seg.iter().flatten().all(|p| p.x.is_finite() && p.y.is_finite()));
    // plus de MAX_RAW réponses : régulièrement espacées, MAX_RAW au plus
    let mut h = f.clone();
    let d = Arc::make_mut(&mut h.data);
    let (n, len) = (700usize, d.axes[1].res.t_ms.len());
    d.axes[1].res.n = n;
    d.axes[1].res.responses = vec![0.5; n * len];
    assert_eq!(raw_segments(&h, 1).len(), MAX_RAW);
}
