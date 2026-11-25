//! Outil « Réponse indicielle » (port de `test_gui_step.py::test_step_window` et `…test_segment_metrics_matches_definition`), sur un vrai
//! log de 5 s (`tests/data/jeno5_5s.bbl`) : le roll n'a aucune fenêtre exploitable (0 réponse), le pitch en a 9, la boucle y est approchée.

mod support;
use support::Cliquer;

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

/// Les réglages « Tri des fenêtres » de la page arrivent jusqu'au calcul (D-082) : une bande plus large garde plus de
/// fenêtres, une bande plus étroite moins ; les candidates ne changent pas.
#[test]
fn les_reglages_du_tri_changent_les_fenetres_gardees() {
    let st = etat();
    let n = |o: StepOpts| {
        let out = calcule(o, &st);
        let r = &out.files[0].data.axes[1].res;
        (r.n, r.windows)
    };
    let base = n(StepOpts::default());
    let wide = n(StepOpts {
        band: 2.0,
        k: f64::INFINITY,
        ..StepOpts::default()
    });
    let narrow = n(StepOpts {
        band: 0.6,
        ..StepOpts::default()
    });
    assert!(
        wide.0 > base.0 && narrow.0 < base.0,
        "large {wide:?}, défaut {base:?}, étroite {narrow:?}"
    );
    assert!(
        wide.1 == base.1 && narrow.1 == base.1,
        "candidates : {wide:?} {base:?} {narrow:?}"
    );
}

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

/// D-066 (remplace D-049) : la couleur d'une courbe est celle de sa pastille, la même sur toutes les pages ; l'ordre
/// des courbes est celui de la barre.
#[test]
fn le_libelle_est_celui_du_fichier_et_la_couleur_celle_de_la_pastille() {
    let mut st = etat();
    let a = calcule(StepOpts::default(), &st);
    st.add(log5s());
    st.remove(0);
    let b = calcule(StepOpts::default(), &st);
    assert_eq!(a.files[0].data.axes[1].res.n, b.files[0].data.axes[1].res.n);
    assert_eq!(b.files[0].label, st.logs[0].label);
    assert_eq!(b.files[0].color, st.logs[0].color);
    assert_eq!(
        b.files[0].color,
        file_color(1),
        "la couleur reçue à l'ouverture, pas le rang"
    );
    st.add(log5s());
    st.move_log(1, 0);
    let deux = StepTool::compute(
        Options {
            calc: StepOpts::default(),
        },
        st.inputs(&StepTool::new().selected(&st)),
    )
    .unwrap();
    assert_eq!(deux.files[0].label, st.logs[0].label, "l'ordre de la barre");
    assert_eq!(deux.files[0].color, st.logs[0].color);
    assert_eq!(deux.files[1].color, st.logs[1].color);
    assert_ne!(deux.files[0].color, deux.files[1].color);
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
    let texts: Vec<&str> = l.iter().map(|(t, _)| t.as_str()).collect();
    // deux lignes par fichier : sur une seule, le bloc débordait sur la montée de la courbe
    assert_eq!(
        texts,
        [
            "P, I, D, Dm, F (fenêtres gardées)",
            "pic · latence · cassure",
            "1) 67,84,48,48,178 (n=9 sur 26)", // gains du pitch ; 26 fenêtres candidates
            "     1,17 · 22,9 ms · +0,297",
        ]
    );
    assert!(texts.iter().all(|t| !t.contains('.')), "virgule décimale : {texts:?}");
    assert_eq!((l[2].1, l[3].1), (out.files[0].color, out.files[0].color));
    assert_eq!(text_lines(&out.files, 0, &CLAIR)[2].0, "1) données insuffisantes");
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
        h.cliquer_label(col);
        h.run_steps(2);
        if h.query_by_label(titre).is_none() {
            let n_sel = h.query_all_by_label_contains("Pic").count();
            let n_lat = h.query_all_by_label_contains("Latence").count();
            let n_mar = h.query_all_by_label_contains("Marge").count();
            panic!("{titre} absent (i={i}) ; Pic {n_sel} Latence {n_lat} Marge {n_mar}");
        }
        h.cliquer_label(titre);
        attendre(&mut h);
        if h.query_by_label(titre).is_none() {
            let v1 = h.get_by_label("Colonne 1").value();
            let v2 = h.get_by_label("Colonne 2").value();
            panic!("après le clic : {titre} introuvable (i={i}) ; colonne 1 = {v1:?}, colonne 2 = {v2:?}");
        }
    }
    h.cliquer_label("Brutes (toutes les fenêtres)");
    h.cliquer_label("± écart-type");
    attendre(&mut h);
    h.cliquer_value("non");
    h.run_steps(2);
    h.cliquer_label("Pitch");
    attendre(&mut h);
    h.cliquer_value("250 ms");
    h.run_steps(2);
    h.cliquer_label("150 ms");
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
