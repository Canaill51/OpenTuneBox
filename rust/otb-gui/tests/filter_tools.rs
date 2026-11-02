//! Outils de la phase 6-4 (un seul fichier d'intégration pour toute la phase : L-069). Pour l'instant, l'Assistant, sur un vrai log de 5 s
//! (`tests/data/jeno5_5s.bbl`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_gui::app::{App, Screen};
use otb_gui::state::AppState;
use otb_gui::theme::{CLAIR, SOMBRE, Theme};
use otb_gui::tool::Tool;
use otb_gui::tools::advisor::{AdvisorOutput, AdvisorTool, FOOTER, HELP, Options, categories, step_text};
use otb_gui::tools::common::safe_text;

fn log5s() -> otb_log::flightlog::FlightLog {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    otb_log::flightlog::load(&p).unwrap().remove(0)
}

fn etat() -> AppState {
    let mut st = AppState::new();
    st.add(log5s());
    st
}

fn calcule(profile: Option<&'static str>, st: &AppState) -> AdvisorOutput {
    AdvisorTool::compute(Options { profile }, st.inputs(&[0])).unwrap()
}

// ------------------------------------------------------------------ Assistant : calcul

#[test]
fn l_assistant_rend_un_rapport_avec_profil_periode_et_trois_axes() {
    let st = etat();
    let out = calcule(None, &st);
    assert_eq!(out.label, "jeno5_5s");
    let r = &out.report;
    assert!(!r.profile.key.is_empty());
    assert_ne!(r.profile_reason, "choisi par l'utilisateur");
    assert!(r.period.1 > r.period.0, "{:?}", r.period);
    assert_eq!(r.steps.len(), 3);
    assert!(!r.findings.is_empty());
    assert_eq!(out.firmware, st.logs[0].log.settings.firmware);
    // profil choisi : la raison le dit
    let w = calcule(Some("whoop"), &st);
    assert_eq!(w.report.profile.key, "whoop");
    assert_eq!(w.report.profile_reason, "choisi par l'utilisateur");
    assert!(step_text(&r.steps[0].result) == "données insuffisantes" || r.steps[0].result.n > 0);
    assert!(
        step_text(&r.steps[1].result).contains("n=9"),
        "{}",
        step_text(&r.steps[1].result)
    );
}

#[test]
fn les_categories_suivent_l_ordre_fixe_puis_les_autres_triees() {
    let st = etat();
    let mut rep = (*calcule(None, &st).report).clone();
    let cats = categories(&rep);
    assert_eq!(&cats[..3], ["Enregistrement", "Vol", "Réponse"]);
    rep.findings[0].category = "Zèbre".into();
    rep.findings.push(rep.findings[0].clone());
    rep.findings[1].category = "Autre".into();
    let cats = categories(&rep);
    assert_eq!(&cats[8..], ["Autre", "Zèbre"]);
}

#[test]
fn le_cache_sert_au_second_calcul_mais_ni_apres_un_autre_profil_ni_une_autre_periode() {
    let mut st = etat();
    let a = calcule(None, &st);
    let b = calcule(None, &st);
    assert!(
        Arc::ptr_eq(&a.report, &b.report),
        "même profil et même période : repris"
    );
    let c = calcule(Some("whoop"), &st);
    assert!(!Arc::ptr_eq(&a.report, &c.report), "un autre profil");
    let d = calcule(Some("whoop"), &st);
    assert!(Arc::ptr_eq(&c.report, &d.report));
    let (t0, t1) = st.logs[0].period;
    // `set_period` vide déjà le cache : on change la période à la main pour vérifier que la CLÉ la contient
    st.logs[0].period = (t0 + 0.5, t1 - 0.5);
    let e = calcule(None, &st);
    assert!(!Arc::ptr_eq(&a.report, &e.report), "une autre période");
    assert!(e.report.period.0 > a.report.period.0 - 1e-9);
}

#[test]
fn un_profil_inconnu_est_une_erreur() {
    let st = etat();
    let r = AdvisorTool::compute(
        Options {
            profile: Some("zeppelin"),
        },
        st.inputs(&[0]),
    );
    assert!(r.is_err());
    assert!(AdvisorTool::compute(Options { profile: None }, Vec::new()).is_err());
}

// ------------------------------------------------------------------ Assistant : dans la fenêtre

fn banc(theme: Theme) -> Harness<'static, App> {
    let mut app = App::new(otb_gui::tools::all_pages());
    app.theme = theme;
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 950.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app);
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
        .position(|p| p.title() == "Assistant")
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
fn la_page_de_l_assistant_s_affiche_dans_les_deux_themes() {
    for theme in [CLAIR, SOMBRE] {
        let mut h = banc(theme);
        h.run(); // contrôle d'accessibilité de kittest (L-068)
        assert!(
            h.query_all_by_label("jeno5_5s").count() >= 2,
            "choix du fichier et titre du rapport"
        );
        h.get_by_label("Drone :");
        h.get_by_label("Période analysée :");
        h.get_by_label("Firmware :");
        h.get_by_label("Taille du drone");
        h.get_by_label(FOOTER);
        h.get_by_label(HELP);
        for ax in ["Roll", "Pitch", "Yaw"] {
            h.get_by_label(&format!("{ax} — réponse indicielle"));
        }
    }
}

#[test]
fn choisir_un_profil_change_le_drone_affiche() {
    let mut h = banc(CLAIR);
    assert!(
        h.query_by_label_contains("choisi par l'utilisateur").is_none(),
        "détection automatique au départ"
    );
    h.get_by_label("Taille du drone").click();
    h.run_steps(3);
    let whoop = safe_text(otb_advisor::profiles::profile("whoop").unwrap().label);
    h.get_by_label(&whoop).click();
    attendre(&mut h);
    h.get_by_label_contains("choisi par l'utilisateur");
    // la valeur affichée a changé : le titre « Drone : » porte le profil choisi
    let Screen::Tool(i) = h.state().screen else {
        panic!("page d'outil")
    };
    assert!(!h.state().pages[i].busy());
    assert!(
        h.query_all_by_label(&whoop).count() >= 1,
        "la ligne « Drone : » porte le profil choisi"
    );
}
