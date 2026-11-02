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
