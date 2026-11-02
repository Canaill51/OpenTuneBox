//! Outils de la phase 6-4 (un seul fichier d'intégration pour toute la phase : L-069) : l'Assistant et la Configuration, sur un
//! vrai log de 5 s (`tests/data/jeno5_5s.bbl`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use otb_gui::app::{App, Screen};
use otb_gui::state::AppState;
use otb_gui::theme::{CLAIR, SOMBRE, Theme};
use otb_gui::tool::Tool;
use otb_gui::tools::advisor::{AdvisorOutput, AdvisorTool, FOOTER, HELP, Options, categories, step_text};
use otb_gui::tools::common::safe_text;
use otb_gui::tools::filter_sim_calc::{
    self as fsc, DELAY_FREQS, Editor, LPF_TYPES, SPECTRA_KEYS, bode, cli_values, delays, filter_cli_lines,
    simulate_filters,
};
use otb_gui::tools::setup::{self, Group, LogSetup, Row, SetupTool, align, sections_from, visible};
use otb_log::settings::{Settings, parse_settings};

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

// ------------------------------------------------------------------ Configuration : calcul pur

fn reglages(pairs: &[(&str, &str)]) -> Settings {
    parse_settings(&pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}

fn valeur(secs: &[setup::Section], groupe: &str, cle: &str) -> String {
    let (_, rows) = secs
        .iter()
        .find(|(t, _)| *t == groupe)
        .unwrap_or_else(|| panic!("groupe {groupe}"));
    rows.iter()
        .find(|(k, _)| k == cle)
        .unwrap_or_else(|| panic!("ligne {cle}"))
        .1
        .clone()
}

#[test]
fn les_sections_du_vrai_log_ont_les_textes_de_python() {
    // valeurs relevées avec `_sections` de Python sur ce même fichier (sortie identique, ligne à ligne)
    let l = log5s();
    let s = setup::sections(&l.settings, &l);
    let titres: Vec<&str> = s.iter().map(|(t, _)| *t).collect();
    assert_eq!(
        titres,
        [
            "Général",
            "PID",
            "Curseurs simplifiés",
            "Filtres gyro",
            "Filtres D",
            "Feedforward et lissage RC",
            "Terme I, TPA, anti-gravité",
            "Rates",
            "Moteurs"
        ]
    );
    assert_eq!(
        valeur(&s, "Général", "Firmware"),
        "Betaflight 2026.6.0-rc3 (f625a9996) STM32F405"
    );
    assert_eq!(valeur(&s, "Général", "Carte"), "STM32F405");
    assert_eq!(valeur(&s, "Général", "Fréquence gyro / PID"), "3205 Hz / 3205 Hz");
    assert_eq!(valeur(&s, "Général", "Fréquence d'enregistrement"), "1618 Hz");
    assert_eq!(valeur(&s, "Général", "Durée du log"), "5.1 s");
    assert_eq!(valeur(&s, "PID", "Roll P, I, D, Dmax, F"), "58,72,39,39,156");
    assert_eq!(valeur(&s, "PID", "D max : gain / avance"), "0 / 35");
    assert_eq!(valeur(&s, "Curseurs simplifiés", "master"), "130");
    assert_eq!(valeur(&s, "Filtres gyro", "LPF1"), "désactivé");
    assert_eq!(valeur(&s, "Filtres gyro", "LPF2"), "PT1 750 Hz");
    assert_eq!(valeur(&s, "Filtres gyro", "Notch dynamique"), "2 × Q4.0, 100–600 Hz");
    assert_eq!(
        valeur(&s, "Filtres gyro", "Filtre RPM"),
        "3 harmoniques, Q7.0, min 100 Hz, poids 100,50,70 %, fondu 50 Hz"
    );
    assert_eq!(valeur(&s, "Filtres D", "LPF1"), "PT1 dynamique 105–210 Hz (expo 5)");
    assert_eq!(valeur(&s, "Feedforward et lissage RC", "rc_smoothing_mode"), "—");
    assert_eq!(valeur(&s, "Rates", "Type"), "ACTUAL");
    assert_eq!(valeur(&s, "Moteurs", "Protocole / DShot bidirectionnel"), "6 / oui");
    assert_eq!(valeur(&s, "Moteurs", "KV moteur"), "1960");
    assert_eq!(valeur(&s, "Moteurs", "Limite moteur / gaz"), "100 % / 100 %");
}

#[test]
fn les_sections_d_un_journal_fabrique_prennent_les_replis_de_python() {
    // aucun réglage : « — », « désactivé », « aucun », « absents », type de rates inconnu « ? »
    let mut s = Settings::default();
    let v = sections_from(&s, 1000.0, 2.25);
    assert_eq!(valeur(&v, "Général", "Nom du drone"), "—");
    assert_eq!(valeur(&v, "Général", "Fréquence gyro / PID"), "8000 Hz / 8000 Hz");
    assert_eq!(valeur(&v, "Général", "Fréquence d'enregistrement"), "1000 Hz");
    assert_eq!(
        valeur(&v, "Général", "Durée du log"),
        "2.2 s",
        "arrondi pair de Python : 2.25 -> 2.2"
    );
    assert_eq!(valeur(&v, "Curseurs simplifiés", "—"), "absents");
    assert_eq!(valeur(&v, "Filtres gyro", "Notchs statiques"), "aucun");
    assert_eq!(valeur(&v, "Filtres gyro", "Notch dynamique"), "désactivé");
    assert_eq!(valeur(&v, "Filtres gyro", "Filtre RPM"), "désactivé");
    assert_eq!(valeur(&v, "Filtres D", "Notch D"), "aucun");
    assert_eq!(valeur(&v, "Filtres D", "Passe-bas P yaw"), "—");
    assert_eq!(valeur(&v, "PID", "Level (angle)"), "—");
    assert_eq!(valeur(&v, "Rates", "Type"), "?");
    assert_eq!(valeur(&v, "Moteurs", "KV moteur"), "—");
    assert_eq!(valeur(&v, "Moteurs", "Ralenti"), "—");
    assert_eq!(valeur(&v, "Moteurs", "Protocole / DShot bidirectionnel"), "— / non");
    // réglages renseignés : notchs, passe-bas statique et dynamique, ralenti de repli, type de rates connu
    s = reglages(&[
        ("gyro_notch_hz", "300,0,450"),
        ("gyro_notch_cutoff", "250,0,400"),
        ("gyro_lpf1_static_hz", "200"),
        ("gyro_lpf1_type", "2"),
        ("gyro_lpf2_dyn_hz", "100,300"),
        ("dterm_notch_hz", "180"),
        ("dterm_notch_cutoff", "120"),
        ("yaw_lowpass_hz", "90"),
        ("dyn_notch_count", "3"),
        ("dyn_notch_q", "350"),
        ("dyn_notch_min_hz", "90"),
        ("dyn_notch_max_hz", "700"),
        ("rates_type", "4"),
        ("dshot_idle_value", "600"),
        ("levelPID", ""),
        ("motor_kv", "2400"),
    ]);
    let v = sections_from(&s, 1600.0, 10.0);
    assert_eq!(valeur(&v, "Filtres gyro", "LPF1"), "PT2 200 Hz");
    assert_eq!(valeur(&v, "Filtres gyro", "LPF2"), "PT1 dynamique 100–300 Hz (expo 5)");
    assert_eq!(
        valeur(&v, "Filtres gyro", "Notchs statiques"),
        "300 Hz (coupure 250), 450 Hz (coupure 400)"
    );
    assert_eq!(valeur(&v, "Filtres gyro", "Notch dynamique"), "3 × Q3.5, 90–700 Hz");
    assert_eq!(valeur(&v, "Filtres D", "Notch D"), "180 Hz (coupure 120)");
    assert_eq!(valeur(&v, "Filtres D", "Passe-bas P yaw"), "90 Hz");
    assert_eq!(valeur(&v, "Rates", "Type"), "QUICK");
    assert_eq!(valeur(&v, "Moteurs", "Ralenti"), "600");
    assert_eq!(
        valeur(&v, "PID", "Level (angle)"),
        "—",
        "valeur vide : « — » (`or \"—\"`)"
    );
    assert_eq!(valeur(&v, "Moteurs", "KV moteur"), "2400");
    assert_eq!(valeur(&v, "Général", "Durée du log"), "10.0 s");
}

fn config(label: &str, pairs: &[(&str, &str)]) -> LogSetup {
    let s = reglages(pairs);
    LogSetup::new(label.into(), egui::Color32::WHITE, sections_from(&s, 1000.0, 1.0), &s)
}

fn ligne<'a>(g: &'a [Group], titre: &str, cle: &str) -> Option<&'a Row> {
    g.iter().find(|x| x.title == titre)?.rows.iter().find(|r| r.key == cle)
}

#[test]
fn les_lignes_sont_alignees_par_nom_et_non_par_position() {
    // A : master, i_gain, d_gain ; B : master, pi_gain, d_gain (autre version) -> par position, « i_gain » de A
    // serait face à « pi_gain » de B
    let a = config(
        "A",
        &[
            ("simplified_master_multiplier", "100"),
            ("simplified_i_gain", "70"),
            ("simplified_d_gain", "110"),
        ],
    );
    let b = config(
        "B",
        &[
            ("simplified_master_multiplier", "100"),
            ("simplified_pi_gain", "90"),
            ("simplified_d_gain", "110"),
        ],
    );
    let g = align(&[a, b], false);
    let cles: Vec<&str> = g
        .iter()
        .find(|x| x.title == "Curseurs simplifiés")
        .unwrap()
        .rows
        .iter()
        .map(|r| r.key.as_str())
        .collect();
    assert_eq!(
        cles,
        ["master", "i_gain", "d_gain", "pi_gain"],
        "ordre du 1er log, puis les noms propres à B"
    );
    let c = "Curseurs simplifiés";
    assert_eq!(ligne(&g, c, "i_gain").unwrap().values, ["70", "—"]);
    assert_eq!(ligne(&g, c, "pi_gain").unwrap().values, ["—", "90"]);
    assert_eq!(ligne(&g, c, "d_gain").unwrap().values, ["110", "110"]);
    assert!(!ligne(&g, c, "d_gain").unwrap().differs());
    assert!(ligne(&g, c, "master").is_some_and(|r| !r.differs()));
    assert!(ligne(&g, c, "i_gain").unwrap().differs());
}

#[test]
fn un_log_sans_curseurs_ne_cree_pas_de_ligne_fantome() {
    let a = config("A", &[]);
    let b = config("B", &[("simplified_master_multiplier", "100")]);
    let g = align(&[a, b], false);
    let c = &g.iter().find(|x| x.title == "Curseurs simplifiés").unwrap().rows;
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].values, ["—", "100"]);
    // seuls, ils gardent le repère « absents »
    let seul = align(&[config("A", &[])], false);
    assert_eq!(ligne(&seul, "Curseurs simplifiés", "—").unwrap().values, ["absents"]);
}

#[test]
fn les_parametres_bruts_sont_l_union_triee_sans_field_ni_h() {
    let a = config(
        "A",
        &[("zeta", "1"), ("alpha", "2"), ("Field I name", "x"), ("H Product", "y")],
    );
    let b = config("B", &[("alpha", "3"), ("beta", "4")]);
    let sans = align(&[a.clone(), b.clone()], false);
    assert!(sans.iter().all(|g| g.title != setup::RAW_TITLE));
    let g = align(&[a, b], true);
    let raw = g.last().unwrap();
    assert_eq!(raw.title, setup::RAW_TITLE);
    let cles: Vec<&str> = raw.rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(cles, ["alpha", "beta", "zeta"]);
    assert_eq!(raw.rows[0].values, ["2", "3"]);
    assert_eq!(raw.rows[1].values, ["—", "4"]);
    assert_eq!(raw.rows[2].values, ["1", "—"]);
}

#[test]
fn les_filtres_d_affichage_gardent_les_differences_et_la_recherche() {
    let a = config("A", &[("rates_type", "3"), ("rc_expo", "17,17,17"), ("tpa_rate", "65")]);
    let b = config("B", &[("rates_type", "3"), ("rc_expo", "20,20,20"), ("tpa_rate", "65")]);
    let g = align(&[a, b], false);
    let n = |g: &[Group]| g.iter().map(|x| x.rows.len()).sum::<usize>();
    // seulement les différences : le groupe « Rates » ne garde que rc_expo, les groupes sans différence disparaissent
    let d = visible(&g, true, "");
    assert!(d.iter().all(|x| x.rows.iter().all(Row::differs)));
    assert_eq!(ligne(&d, "Rates", "rc_expo").unwrap().values, ["17,17,17", "20,20,20"]);
    assert!(ligne(&d, "Rates", "Type").is_none());
    assert!(
        d.iter().all(|x| x.title != "Moteurs"),
        "groupe sans ligne visible masqué"
    );
    assert!(n(&d) < n(&g));
    // recherche : dans le nom (casse ignorée, espaces coupés) ou dans les valeurs
    let r = visible(&g, false, "  RC_EXPO ");
    assert_eq!((r.len(), n(&r)), (1, 1));
    let r = visible(&g, false, "20,20");
    assert_eq!(ligne(&r, "Rates", "rc_expo").unwrap().key, "rc_expo");
    assert_eq!(n(&r), 1);
    assert!(visible(&g, false, "introuvable-xyz").is_empty());
    assert_eq!(n(&visible(&g, false, "")), n(&g));
    // les deux ensemble
    assert!(
        visible(&g, true, "tpa").is_empty(),
        "tpa_rate est identique : écarté par « différences »"
    );
}

// ------------------------------------------------------------------ Configuration : dans la fenêtre

/// Ouvre un second log (réglages modifiés) à côté du premier, premier log courant, et affiche la Configuration.
fn banc_config(theme: Theme, deux: bool) -> Harness<'static, App> {
    let mut h = banc(theme);
    if deux {
        let mut l = log5s();
        let mut raw = l.settings.raw.clone();
        raw.insert("Craft name".into(), "Autre drone".into());
        raw.insert("simplified_pi_gain".into(), "90".into());
        raw.insert("zz_propre_au_second".into(), "42".into());
        l.settings = parse_settings(&raw);
        h.state_mut().state.add(l);
        h.state_mut().state.current = Some(0);
    }
    ouvre_config(&mut h);
    h
}

fn ouvre_config(h: &mut Harness<'static, App>) {
    let i = h
        .state()
        .pages
        .iter()
        .position(|p| p.title() == "Configuration")
        .expect("page enregistrée");
    h.state_mut().screen = Screen::Tool(i);
    attendre(h);
}

#[test]
fn la_page_configuration_est_cachee_et_s_ouvre_depuis_les_courbes() {
    let app = App::new(otb_gui::tools::all_pages());
    let p = app.pages.iter().find(|p| p.title() == "Configuration").unwrap();
    assert!(p.hidden());
    assert_eq!(p.tab(), "Courbes");
}

#[test]
fn la_page_configuration_s_affiche_dans_les_deux_themes() {
    for theme in [CLAIR, SOMBRE] {
        let mut h = banc_config(theme, true);
        h.run(); // contrôle d'accessibilité de kittest (L-068)
        h.get_by_label("Paramètre");
        h.get_by_label("Seulement les différences");
        h.get_by_label("Tous les paramètres bruts");
        h.get_by_label("Rechercher un paramètre");
        // seul le 1er log est coché au départ : une colonne
        assert!(h.query_by_label("Firmware").is_some());
        assert!(h.query_by_label("Nom du drone").is_some());
        h.get_by_label("Général");
        assert!(h.query_by_label("jeno5_5s (2)").is_some(), "choix du second fichier");
    }
}

#[test]
fn deux_logs_cote_a_cote_alignes_par_nom_avec_differences_et_recherche() {
    let mut h = banc_config(CLAIR, true);
    h.get_by_label("jeno5_5s (2)").click();
    attendre(&mut h);
    h.run();
    h.get_by_label("Autre drone");
    h.get_by_label("JeNo Ced");
    // le curseur propre au second log a sa ligne (« — » pour le premier), au bout de son groupe
    h.get_by_label("pi_gain");
    let Screen::Tool(i) = h.state().screen else { panic!() };
    assert!(!h.state().pages[i].busy());
    // seulement les différences : le nom du drone et le curseur restent, la carte (identique) disparaît
    h.get_by_label("Seulement les différences").click();
    h.run_steps(3);
    assert!(h.query_by_label("Nom du drone").is_some());
    assert!(h.query_by_label("pi_gain").is_some());
    assert!(h.query_by_label("Carte").is_none(), "ligne identique masquée");
    assert!(h.query_by_label("Rates").is_none(), "groupe sans ligne visible masqué");
    h.get_by_label("Seulement les différences").click();
    h.run_steps(3);
    assert!(h.query_by_label("Carte").is_some());
    // paramètres bruts : la clé propre au second log apparaît
    assert!(h.query_by_label("zz_propre_au_second").is_none());
    h.get_by_label("Tous les paramètres bruts").click();
    h.run_steps(3);
    h.get_by_label("zz_propre_au_second");
    h.get_by_label(setup::RAW_TITLE);
    // recherche insensible à la casse, dans les valeurs aussi
    h.get_by_label("Rechercher un paramètre").click();
    h.get_by_label("Rechercher un paramètre").type_text("AUTRE DRONE");
    h.run_steps(3);
    h.get_by_label("Nom du drone");
    assert!(h.query_by_label("Carte").is_none());
    assert!(h.query_by_label("pi_gain").is_none());
}

/// Le bouton « Courbes » de la barre est mis en valeur (état « coché » d'accesskit).
fn courbes_en_valeur(h: &Harness<'static, App>) -> bool {
    h.get_by_label("Courbes").accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
}

#[test]
fn le_bouton_configuration_du_visualiseur_ouvre_la_page_et_met_courbes_en_valeur() {
    let mut h = banc(CLAIR);
    h.state_mut().screen = Screen::Viewer;
    h.run_steps(4);
    assert!(courbes_en_valeur(&h));
    h.get_by_label("Configuration").click();
    attendre(&mut h);
    h.run_steps(2);
    let Screen::Tool(i) = h.state().screen else {
        panic!(
            "la page de la Configuration doit s'ouvrir, écran : {:?}",
            h.state().screen
        )
    };
    assert_eq!(h.state().pages[i].title(), "Configuration");
    assert!(courbes_en_valeur(&h), "« Courbes » reste mis en valeur");
    assert!(h.query_by_label("Configuration").is_none() || h.state().pages[i].hidden());
    h.get_by_label("Firmware");
    // sur un autre outil, « Courbes » n'est plus mis en valeur
    let j = h.state().pages.iter().position(|p| p.title() == "Assistant").unwrap();
    h.state_mut().screen = Screen::Tool(j);
    attendre(&mut h);
    assert!(!courbes_en_valeur(&h));
}

#[test]
fn le_bouton_configuration_sans_log_demande_d_ouvrir_un_log() {
    let app = App::new(otb_gui::tools::all_pages());
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 950.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app);
    h.state_mut().screen = Screen::Viewer;
    h.run_steps(4);
    h.get_by_label("Configuration").click();
    h.run_steps(4);
    assert_eq!(h.state().screen, Screen::Viewer);
    h.get_by_label("Ouvrez d'abord un log Blackbox.");
}

#[test]
fn la_selection_de_la_configuration_est_multiple_et_limitee_a_dix() {
    let mut st = AppState::new();
    for _ in 0..3 {
        st.add(log5s());
    }
    let t = SetupTool::new();
    assert_eq!(t.selected(&st).len(), 1, "par défaut le log courant");
    let out = SetupTool::compute((), st.inputs(&[0, 1, 2])).unwrap();
    assert_eq!(out.logs.len(), 3);
    assert_eq!(out.logs[1].label, "jeno5_5s (2)");
    assert!(SetupTool::compute((), Vec::new()).is_err());
}

// ------------------------------------------------------------------ Simulateur de filtres : calcul pur

mod simu {
    use super::*;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};

    /// Références écrites par `scripts/filter_sim_cases.py` (`tests/data/filter_sim.json`).
    pub fn cas() -> &'static Value {
        static C: OnceLock<Value> = OnceLock::new();
        C.get_or_init(|| {
            let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/filter_sim.json");
            serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
        })
    }

    /// Flottant écrit en hexadécimal (`struct.pack("<d").hex()`).
    pub fn hx(v: &Value) -> f64 {
        let h = v.as_str().unwrap();
        let mut b = [0u8; 8];
        for (i, byte) in b.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap();
        }
        f64::from_le_bytes(b)
    }

    /// Tableau float64 de la table `inputs`.
    pub fn input(id: &Value) -> Vec<f64> {
        let h = cas()["inputs"][id.as_str().unwrap()].as_str().unwrap();
        (0..h.len() / 16)
            .map(|i| hx(&Value::String(h[16 * i..16 * i + 16].to_string())))
            .collect()
    }

    static ERRS: Mutex<BTreeMap<&'static str, f64>> = Mutex::new(BTreeMap::new());

    /// Plus grand écart relevé par catégorie (affiché par `report`, `--nocapture`).
    pub fn note(cat: &'static str, e: f64) {
        let mut m = ERRS.lock().unwrap();
        let v = m.entry(cat).or_insert(0.0);
        if e > *v {
            *v = e;
        }
    }

    pub fn report() {
        for (k, v) in ERRS.lock().unwrap().iter() {
            eprintln!("écart max {k} : {v:e}");
        }
    }

    /// NaN et infinis aux mêmes endroits, puis écart absolu (ou relatif si `rel`) à `tol`.
    pub fn close(cat: &'static str, got: &[f64], want: &[f64], tol: f64, rel: bool, what: &str) {
        assert_eq!(got.len(), want.len(), "{what} : longueurs");
        let mut worst = 0.0f64;
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            if w.is_nan() || g.is_nan() {
                assert!(w.is_nan() && g.is_nan(), "{what}[{i}] : NaN {g} / {w}");
                continue;
            }
            if w.is_infinite() || g.is_infinite() {
                assert_eq!(g, w, "{what}[{i}]");
                continue;
            }
            let e = (g - w).abs() / if rel { w.abs().max(1e-300) } else { 1.0 };
            worst = worst.max(e);
            assert!(e <= tol, "{what}[{i}] : {g} contre {w} (écart {e:e} > {tol:e})");
        }
        note(cat, worst);
    }

    pub fn raw_settings(v: &Value) -> Settings {
        let m: BTreeMap<String, String> = v
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
            .collect();
        parse_settings(&m)
    }

    fn i(v: &Value) -> i64 {
        v.as_i64().unwrap()
    }

    fn lowpass(lp: &mut otb_log::settings::Lowpass, v: &Value) {
        lp.kind = LPF_TYPES.iter().find(|t| **t == v[0].as_str().unwrap()).unwrap();
        (lp.static_hz, lp.dyn_min_hz, lp.dyn_max_hz, lp.expo) = (i(&v[1]), i(&v[2]), i(&v[3]), i(&v[4]));
    }

    /// Réglages de `base` dont les filtres sont remplacés par ceux de `enc` (`enc_filters` de Python).
    pub fn apply_filters(base: &Settings, enc: &Value) -> Settings {
        let mut s = base.clone();
        lowpass(&mut s.gyro_lpf1, &enc["gyro_lpf1"]);
        lowpass(&mut s.gyro_lpf2, &enc["gyro_lpf2"]);
        lowpass(&mut s.dterm_lpf1, &enc["dterm_lpf1"]);
        lowpass(&mut s.dterm_lpf2, &enc["dterm_lpf2"]);
        s.gyro_notches = enc["gyro_notches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| (i(&n[0]), i(&n[1])))
            .collect();
        s.dterm_notch = (i(&enc["dterm_notch"][0]), i(&enc["dterm_notch"][1]));
        let d = &enc["dyn"];
        (s.dyn_notch_count, s.dyn_notch_q, s.dyn_notch_min_hz, s.dyn_notch_max_hz) =
            (i(&d[0]), i(&d[1]), i(&d[2]), i(&d[3]));
        let r = &enc["rpm"];
        (s.rpm_harmonics, s.rpm_q, s.rpm_min_hz) = (i(&r[0]), i(&r[1]), i(&r[2]));
        s.rpm_weights = enc["rpm_weights"].as_array().unwrap().iter().map(i).collect();
        s
    }

    /// Même forme que `enc_filters` de Python.
    pub fn filters_of(s: &Settings) -> Value {
        let lp = |l: &otb_log::settings::Lowpass| json!([l.kind, l.static_hz, l.dyn_min_hz, l.dyn_max_hz, l.expo]);
        json!({
            "gyro_lpf1": lp(&s.gyro_lpf1), "gyro_lpf2": lp(&s.gyro_lpf2), "dterm_lpf1": lp(&s.dterm_lpf1),
            "dterm_lpf2": lp(&s.dterm_lpf2),
            "gyro_notches": s.gyro_notches.iter().map(|n| json!([n.0, n.1])).collect::<Vec<_>>(),
            "dterm_notch": [s.dterm_notch.0, s.dterm_notch.1],
            "dyn": [s.dyn_notch_count, s.dyn_notch_q, s.dyn_notch_min_hz, s.dyn_notch_max_hz],
            "rpm": [s.rpm_harmonics, s.rpm_q, s.rpm_min_hz], "rpm_weights": s.rpm_weights,
        })
    }

    /// Lecture d'un champ de l'éditeur par son nom de Python (`g1_type`, `rpm_w0`…).
    pub fn get(e: &Editor, k: &str) -> i64 {
        let (p, name) = k.split_at(if k.starts_with("g1_") || k.starts_with("d1_") {
            3
        } else {
            0
        });
        let l = match p {
            "g1_" => Some(&e.g1),
            "d1_" => Some(&e.d1),
            _ => None,
        };
        if let Some(l) = l {
            return match name {
                "type" => l.kind as i64,
                "mode" => l.mode as i64,
                "static" => l.static_hz,
                "min" => l.min_hz,
                "max" => l.max_hz,
                "expo" => l.expo,
                _ => panic!("{k}"),
            };
        }
        match k {
            "g2_type" => e.g2_type as i64,
            "g2_hz" => e.g2_hz,
            "n1_hz" => e.n1_hz,
            "n1_cut" => e.n1_cut,
            "n2_hz" => e.n2_hz,
            "n2_cut" => e.n2_cut,
            "d2_type" => e.d2_type as i64,
            "d2_hz" => e.d2_hz,
            "dn_hz" => e.dn_hz,
            "dn_cut" => e.dn_cut,
            "dyn_count" => e.dyn_count,
            "dyn_q" => e.dyn_q,
            "dyn_min" => e.dyn_min,
            "dyn_max" => e.dyn_max,
            "rpm_harm" => e.rpm_harm,
            "rpm_q" => e.rpm_q,
            "rpm_min" => e.rpm_min,
            "rpm_w0" => e.rpm_w[0],
            "rpm_w1" => e.rpm_w[1],
            "rpm_w2" => e.rpm_w[2],
            _ => panic!("{k}"),
        }
    }

    pub fn set(e: &mut Editor, k: &str, v: i64) {
        let slot: &mut i64 = match k {
            "g1_type" => return e.g1.kind = v as usize,
            "g1_mode" => return e.g1.mode = v as usize,
            "d1_type" => return e.d1.kind = v as usize,
            "d1_mode" => return e.d1.mode = v as usize,
            "g2_type" => return e.g2_type = v as usize,
            "d2_type" => return e.d2_type = v as usize,
            "g1_static" => &mut e.g1.static_hz,
            "g1_min" => &mut e.g1.min_hz,
            "g1_max" => &mut e.g1.max_hz,
            "g1_expo" => &mut e.g1.expo,
            "d1_static" => &mut e.d1.static_hz,
            "d1_min" => &mut e.d1.min_hz,
            "d1_max" => &mut e.d1.max_hz,
            "d1_expo" => &mut e.d1.expo,
            "g2_hz" => &mut e.g2_hz,
            "n1_hz" => &mut e.n1_hz,
            "n1_cut" => &mut e.n1_cut,
            "n2_hz" => &mut e.n2_hz,
            "n2_cut" => &mut e.n2_cut,
            "d2_hz" => &mut e.d2_hz,
            "dn_hz" => &mut e.dn_hz,
            "dn_cut" => &mut e.dn_cut,
            "dyn_count" => &mut e.dyn_count,
            "dyn_q" => &mut e.dyn_q,
            "dyn_min" => &mut e.dyn_min,
            "dyn_max" => &mut e.dyn_max,
            "rpm_harm" => &mut e.rpm_harm,
            "rpm_q" => &mut e.rpm_q,
            "rpm_min" => &mut e.rpm_min,
            "rpm_w0" => &mut e.rpm_w[0],
            "rpm_w1" => &mut e.rpm_w[1],
            "rpm_w2" => &mut e.rpm_w[2],
            _ => panic!("{k}"),
        };
        *slot = v;
    }

    pub fn lines(v: &Value) -> Vec<String> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap().to_string())
            .collect()
    }

    pub fn pairs(v: &Value) -> Vec<(String, String)> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|kv| (kv[0].as_str().unwrap().to_string(), kv[1].as_str().unwrap().to_string()))
            .collect()
    }
}
use simu::*;

/// Tolérances (mesurées le 2026-11-02 sur le poste de référence, FFT et `pow`/`atan2` ne sont jamais identiques d'une plateforme à
/// l'autre : L-048, L-059). Spectres et gains (dB) : écart absolu mesuré 2,8e-13 et 1,4e-12 ; phases (°) : 5,7e-12 ; retards (ms) :
/// 4,4e-16 ; fréquences : écart relatif 2,2e-16 ; coupures dynamiques : 0. Marges de plusieurs milliers.
const DB_TOL: f64 = 1e-9;
const DEG_TOL: f64 = 1e-8;
const MS_TOL: f64 = 1e-10;
const FREQ_RTOL: f64 = 1e-12;

#[test]
fn les_commandes_cli_sont_celles_de_python_au_caractere_pres() {
    let c = &cas()["cli"];
    let bases: Vec<Settings> = c["bases"].as_array().unwrap().iter().map(raw_settings).collect();
    for (i, b) in bases.iter().enumerate() {
        assert_eq!(cli_values(b), pairs(&c["base_values"][i]), "valeurs de la base {i}");
    }
    let (mut avec_changement, mut sans_changement, mut longs) = (0, 0, 0);
    for k in c["cases"].as_array().unwrap() {
        let base = &bases[k["base"].as_u64().unwrap() as usize];
        let what = format!("base {} {}", k["base"], k["name"]);
        let modf = apply_filters(base, &k["mod"]);
        assert_eq!(filters_of(&modf), k["mod"], "{what} : réglages relus");
        assert_eq!(cli_values(&modf), pairs(&k["values"]), "{what} : valeurs");
        let want = lines(&k["lines"]);
        assert_eq!(filter_cli_lines(base, &modf), want, "{what}");
        assert_eq!(fsc::filter_cli(base, &modf), want.join("\n"), "{what}");
        if want.len() == 2 {
            sans_changement += 1;
        } else {
            avec_changement += 1;
        }
        if want.iter().any(|l| l.contains("attention")) {
            longs += 1;
        }
    }
    assert!(
        avec_changement > 400 && sans_changement > 40 && longs >= 6,
        "{avec_changement} {sans_changement} {longs}"
    );
}

#[test]
fn le_cli_ne_garde_que_les_changements_et_suit_la_version() {
    // port de `test_filter_cli_only_changes_and_versions`
    let settings = |version: &str| {
        reglages(&[
            ("Firmware revision", &format!("Betaflight {version} (abc) STM32F7X2")),
            ("looptime", "125"),
            ("pid_process_denom", "1"),
            ("gyro_lpf2_static_hz", "500"),
            ("rpm_filter_harmonics", "3"),
            ("rpm_filter_weights", "100,100,100"),
            ("simplified_gyro_filter", "1"),
        ])
    };
    let base = settings("4.5.5");
    assert!(
        filter_cli_lines(&base, &base.clone())
            .last()
            .unwrap()
            .starts_with("# aucun changement")
    );
    let mut modf = base.clone();
    modf.gyro_lpf2.static_hz = 300;
    modf.rpm_weights = vec![100, 50, 0];
    let l = filter_cli_lines(&base, &modf);
    assert!(l.contains(&"set gyro_lpf2_static_hz = 300".to_string()));
    assert!(l.contains(&"set simplified_gyro_filter = OFF".to_string()));
    assert!(l.contains(&"set rpm_filter_weights = 100,50,0".to_string()));
    assert_eq!(l.last().unwrap(), "save");
    let b44 = settings("4.4.3");
    let mut m44 = b44.clone();
    m44.rpm_weights = vec![100, 50, 0];
    assert!(
        !filter_cli_lines(&b44, &m44)
            .iter()
            .any(|l| l.starts_with("set rpm_filter_weights"))
    );
}

#[test]
fn l_editeur_a_les_champs_les_bornes_et_les_reglages_de_python() {
    let c = &cas()["editor"];
    let keys: Vec<&str> = c["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect();
    let (mut coupes, mut edits) = (0, 0);
    for (ci, k) in c["cases"].as_array().unwrap().iter().enumerate() {
        let base = raw_settings(&k["raw"]);
        let e0 = Editor::from_settings(&base);
        for key in &keys {
            assert_eq!(
                get(&e0, key),
                k["init"][key].as_i64().unwrap(),
                "cas {ci} champ initial {key}"
            );
        }
        let m0 = e0.modified_settings(&base);
        assert_eq!(filters_of(&m0), k["init_mod"], "cas {ci} réglages initiaux");
        assert_eq!(
            filter_cli_lines(&base, &m0),
            lines(&k["init_lines"]),
            "cas {ci} CLI initial"
        );
        for (ei, ed) in k["edits"].as_array().unwrap().iter().enumerate() {
            let what = format!("cas {ci} modification {ei} {}", ed["set"]);
            let mut e = e0.clone();
            for (key, v) in ed["set"].as_object().unwrap() {
                set(&mut e, key, v.as_i64().unwrap());
            }
            e.clamp(base.family());
            for key in &keys {
                let w = ed["after"][key].as_i64().unwrap();
                assert_eq!(get(&e, key), w, "{what} : champ {key}");
                if ed["set"].get(key).is_some_and(|s| s.as_i64().unwrap() != w) {
                    coupes += 1;
                }
            }
            let m = e.modified_settings(&base);
            assert_eq!(filters_of(&m), ed["mod"], "{what}");
            assert_eq!(filter_cli_lines(&base, &m), lines(&ed["lines"]), "{what} : CLI");
            edits += 1;
        }
    }
    // des saisies hors bornes ont bien été ramenées dans les bornes
    assert!(edits == 7 * 21 && coupes >= 40, "{edits} {coupes}");
}

#[test]
fn l_editeur_inchange_redonne_les_filtres_du_log_sauf_les_deux_couples_de_notchs() {
    let st = etat();
    let base = &st.logs[0].log.settings;
    assert!(base.gyro_notches.is_empty(), "le log d'essai n'a pas de notch gyro");
    let m = Editor::from_settings(base).modified_settings(base);
    assert_eq!(m.gyro_notches, vec![(0, 0), (0, 0)]);
    let mut attendu = base.clone();
    attendu.gyro_notches = vec![(0, 0), (0, 0)];
    assert_eq!(m, attendu);
    // un seul notch dans l'en-tête : le second couple est à zéro
    let one = reglages(&[
        ("Firmware revision", "Betaflight 4.5.2 (abc) STM32F7X2"),
        ("gyro_notch_hz", "260"),
        ("gyro_notch_cutoff", "200"),
    ]);
    assert_eq!(
        Editor::from_settings(&one).modified_settings(&one).gyro_notches,
        vec![(260, 200), (0, 0)]
    );
    // et le CLI d'un éditeur intact ne signale aucun changement
    assert!(fsc::filter_cli(base, &m).ends_with("# aucun changement par rapport au log"));
}

fn sim_sel(c: &serde_json::Value, st: &AppState) -> std::ops::Range<usize> {
    let sel = st.logs[0].sel();
    assert_eq!(
        [sel.start as u64, sel.end as u64],
        [c["sel"][0].as_u64().unwrap(), c["sel"][1].as_u64().unwrap()]
    );
    sel
}

#[test]
fn les_spectres_simules_sont_ceux_de_python() {
    let st = etat();
    let c = &cas()["sim"];
    let log = &*st.logs[0].log;
    let base = &log.settings;
    assert_eq!(filters_of(base), c["base"], "les réglages du log");
    let sel = sim_sel(c, &st);
    let (mut nones, mut nans, mut nb) = (0, 0, 0);
    for r in c["results"].as_array().unwrap() {
        let name = r["mod"].as_str().unwrap();
        let what = format!("{name} (actuel : {})", r["with_current"]);
        let modf = apply_filters(base, &c["mods"][name]);
        let with_cur = r["with_current"].as_bool().unwrap();
        let res = simulate_filters(log, sel.clone(), base, &modf, with_cur).unwrap();
        assert_eq!(res.fs, hx(&r["fs"]), "{what} : fs");
        assert_eq!(res.has_unfilt, r["has_unfilt"].as_bool().unwrap(), "{what}");
        assert_eq!(&res.modf, &modf);
        close(
            "fréquences (relatif)",
            &res.freqs,
            &input(&r["freqs"]),
            FREQ_RTOL,
            true,
            &what,
        );
        let ds: Vec<f64> = r["d_scale"].as_array().unwrap().iter().map(hx).collect();
        close("d_scale", &res.d_scale, &ds, 0.0, false, &what);
        nans += ds.iter().filter(|v| v.is_nan()).count(); // le D du lacet est nul
        assert_eq!(res.spectra.len(), SPECTRA_KEYS.len());
        for key in SPECTRA_KEYS {
            for ax in 0..3 {
                let w = &r["spectra"][key][ax];
                let g = res.spectrum(key, ax);
                assert_eq!(g.is_some(), !w.is_null(), "{what} : {key}[{ax}] présent");
                match (g, w.is_null()) {
                    (Some(g), false) => {
                        close(
                            "spectres (dB)",
                            g,
                            &input(w),
                            DB_TOL,
                            false,
                            &format!("{what} {key}[{ax}]"),
                        );
                        nb += 1;
                    }
                    _ => nones += 1,
                }
            }
        }
    }
    assert!(nones == 25 && nb == 95 && nans == 5, "{nones} {nb} {nans}");
    report();
}

#[test]
fn bode_et_retards_sont_ceux_de_python() {
    let st = etat();
    let c = &cas()["sim"];
    let base = &st.logs[0].log.settings;
    let (mut vides, mut dynamiques) = (0, 0);
    for b in c["bode"].as_array().unwrap() {
        let name = b["mod"].as_str().unwrap();
        let (thr, mhz, dynamic) = (
            b["thr"].as_i64().unwrap(),
            b["motor_hz"].as_f64().unwrap(),
            b["dyn"].as_bool().unwrap(),
        );
        let what = format!("{name} gaz {thr} moteur {mhz} dyn {dynamic}");
        let modf = apply_filters(base, &c["mods"][name]);
        let f = input(&b["freqs"]);
        for (key, s) in [("cur", base), ("mod", &modf)] {
            let (g, d) = bode(s, thr, mhz, dynamic).unwrap();
            for (nm, got) in [("gyro", g), ("d", d)] {
                let w = &b["curves"][format!("{key}_{nm}")];
                close("Bode fréquences (relatif)", &got.freqs, &f, FREQ_RTOL, true, &what);
                close("Bode gain (dB)", &got.gain_db, &input(&w[0]), DB_TOL, false, &what);
                close("Bode phase (°)", &got.phase_deg, &input(&w[1]), DEG_TOL, false, &what);
                if got.gain_db.iter().all(|&v| v == 0.0) {
                    vides += 1;
                }
            }
        }
        let got = delays(base, &modf, thr, mhz, dynamic).unwrap();
        for (row, w) in got.rows.iter().zip(b["delays"].as_array().unwrap()) {
            assert_eq!(row.name, w["name"].as_str().unwrap());
            for (cell, wc) in row.cells.iter().zip(w["cells"].as_array().unwrap()) {
                close(
                    "retards (ms)",
                    &[cell.0, cell.1],
                    &[hx(&wc[0]), hx(&wc[1])],
                    MS_TOL,
                    false,
                    &what,
                );
            }
        }
        assert_eq!(got.rows.len(), 3);
        let want: Vec<(String, f64)> = b["dyn_cutoffs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| (d[0].as_str().unwrap().to_string(), hx(&d[1])))
            .collect();
        assert_eq!(got.dyn_cutoffs.len(), want.len(), "{what} : coupures dynamiques");
        for ((n, v), (wn, wv)) in got.dyn_cutoffs.iter().zip(&want) {
            assert_eq!(n, wn);
            close("coupure dynamique (Hz)", &[*v], &[*wv], MS_TOL, false, &what);
            dynamiques += 1;
        }
    }
    assert!(vides >= 6 && dynamiques >= 3, "{vides} {dynamiques}");
    assert_eq!(DELAY_FREQS, [20.0, 50.0, 100.0]);
    report();
}

#[test]
fn le_simulateur_baisse_le_spectre_au_dessus_de_300_hz_quand_le_lpf2_descend() {
    // port de `test_filter_sim_window`, sur le log de 5 s
    let st = etat();
    let log = &*st.logs[0].log;
    let base = &log.settings;
    let sel = st.logs[0].sel();
    let res = simulate_filters(log, sel.clone(), base, &base.clone(), true).unwrap();
    let g = res.spectrum("gyro", 0).unwrap();
    assert!(!g.is_empty() && g.iter().all(|v| v.is_finite()));
    assert!(res.spectrum("gyro_sim", 0).is_some());
    assert!(res.d_scale[0].is_finite() && res.d_scale[0] > 0.0);
    // réglages inchangés : le spectre simulé retombe sur celui du log
    for (a, b) in g.iter().zip(res.spectrum("gyro_sim", 0).unwrap()) {
        assert!((a - b).abs() < 1e-9, "{a} {b}");
    }
    let mut ed = Editor::from_settings(base);
    ed.g2_hz = 250;
    let modf = ed.modified_settings(base);
    assert!(fsc::filter_cli(base, &modf).contains("set gyro_lpf2_static_hz = 250"));
    let res2 = simulate_filters(log, sel, base, &modf, true).unwrap();
    let moyenne = |r: &fsc::FilterSimResult| {
        let v: Vec<f64> = r
            .freqs
            .iter()
            .zip(r.spectrum("gyro_sim", 0).unwrap())
            .filter(|(f, _)| **f > 300.0)
            .map(|(_, v)| *v)
            .collect();
        v.iter().sum::<f64>() / v.len() as f64
    };
    assert!(moyenne(&res2) < moyenne(&res));
    let d = delays(base, &modf, 50, 200.0, false).unwrap();
    assert_eq!(d.rows.len(), 3);
    assert!(
        d.rows[0].cells.iter().all(|c| c.0.is_finite() && c.1 > c.0 - 1e-9),
        "{:?}",
        d.rows[0]
    );
}

#[test]
fn un_log_sans_gyro_non_filtre_n_a_ni_spectre_avant_filtres_ni_simulation_du_d_nul() {
    let st = etat();
    let mut log = (*st.logs[0].log).clone();
    log.gyro_unfilt = None;
    for v in log.d_term.row_mut(2).iter_mut() {
        *v = 0.0;
    }
    let base = log.settings.clone();
    let res = simulate_filters(&log, st.logs[0].sel(), &base, &base, false).unwrap();
    assert!(!res.has_unfilt);
    assert!((0..3).all(|a| res.spectrum("gyro_pf", a).is_none()));
    assert!(res.d_scale[2].is_nan() && res.d_scale[0] == 1.0);
    assert!(res.spectrum("d", 2).is_none() && res.spectrum("d_pf", 2).is_none() && res.spectrum("d", 0).is_some());
    assert!((0..3).all(|a| res.spectrum("gyro_sim_cur", a).is_none() && res.spectrum("d_sim_cur", a).is_none()));
}
