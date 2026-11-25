//! Outils de la phase 6-4 (un seul fichier d'intégration pour toute la phase : L-069) : l'Assistant et la Configuration, sur un
//! vrai log de 5 s (`tests/data/jeno5_5s.bbl`).

mod support;
use support::Cliquer;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use otb_advisor::rules::{self as regles, Finding};
use otb_analysis::flight_gap::{BandGap, FlightGap, GapCause};
use otb_gui::advice::{self, Comparison};
use otb_gui::app::{App, Screen};
use otb_gui::state::AppState;
use otb_gui::theme::{CLAIR, SOMBRE, Theme};
use otb_gui::tool::Tool;
use otb_gui::tools::advisor::{
    AdvisorOutput, AdvisorTool, FOOTER, HELP, NO_COMPARE, NO_COMPARE_META, NO_HISTORY, ONE_CHANGE, Options,
    REASON_QUESTION, STEP_TITLE, card_id, gone_text, oks_line, open_by_default, pills, split_cards, step_text,
};
use otb_gui::tools::common::safe_text;
use otb_gui::tools::filter_sim::{
    Applied, COPIED_STATUS, FilterSimTool, MULT_WARNING, NO_UNFILT_TITLE, Options as SimOptions, STALE_STATUS,
    result_is_for_other_log,
};
use otb_gui::tools::filter_sim_calc::{
    self as fsc, DELAY_FREQS, Editor, LPF_TYPES, SPECTRA_KEYS, bode, cli_values, delays, filter_cli_lines,
    simulate_filters,
};
use otb_gui::tools::setup::{
    self, Group, LogSetup, Row, SetupTool, align, sections_from, shown_rows, toggle_group, visible,
};
use otb_log::settings::{Settings, parse_settings};
use otb_store::Library;
use otb_store::model::{Advice, Drone, Fragment, LogEntry};

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
    AdvisorTool::compute(Options { profile, compare: None }, st.inputs(&[0])).unwrap()
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
    // roll : aucune fenêtre exploitable sur ces 5 s ; le texte dit pourquoi (et non « n=0 »)
    assert_eq!(r.steps[0].result.n, 0);
    assert_eq!(step_text(&r.steps[0].result), "données insuffisantes");
    // pitch : 9 fenêtres ; le texte du prototype, « pic 1,04 · 13,8 ms », virgule française
    assert_eq!(r.steps[1].result.n, 9);
    let t = step_text(&r.steps[1].result);
    assert!(t.starts_with("pic ") && t.ends_with(" ms") && t.contains(','), "{t}");
}

fn constat(key: &str, severite: &str, titre: &str) -> Finding {
    Finding::new(key, "Réponse", severite, titre, "d", Vec::new(), None)
}

#[test]
fn les_cartes_suivent_la_gravite_et_les_ok_sont_a_part() {
    let st = etat();
    let mut rep = (*calcule(None, &st).report).clone();
    // dans le désordre : une info, un OK, une attention, un conseil, une seconde info
    rep.findings = vec![
        constat("i1", regles::INFO, "Info 1"),
        constat("ok1", regles::OK, "Ok 1"),
        constat("a1", regles::ATTENTION, "Attention 1"),
        constat("c1", regles::CONSEIL, "Conseil 1"),
        constat("i2", regles::INFO, "Info 2"),
    ];
    let (cartes, oks) = split_cards(&rep);
    let cles: Vec<&str> = cartes.iter().map(|f| f.key.as_str()).collect();
    // par gravité, et à gravité égale l'ordre des règles (i1 avant i2)
    assert_eq!(cles, ["a1", "c1", "i1", "i2"]);
    assert_eq!(oks.len(), 1);
    assert_eq!(oks[0].key, "ok1");
}

#[test]
fn les_pastilles_comptent_chaque_gravite() {
    let mut v = vec![
        constat("a", regles::ATTENTION, "A"),
        constat("c1", regles::CONSEIL, "C"),
        constat("c2", regles::CONSEIL, "C"),
        constat("i", regles::INFO, "I"),
        constat("o1", regles::OK, "O"),
        constat("o2", regles::OK, "O"),
    ];
    let textes = |v: &[Finding]| pills(v).into_iter().map(|(_, t)| t).collect::<Vec<_>>();
    // pas d'alerte : pas de pastille d'alerte ; un seul conseil s'écrit au singulier
    assert_eq!(textes(&v), ["1 attention", "2 conseils", "1 info", "2 OK"]);
    v.remove(2);
    v.push(constat("x", regles::ALERTE, "X"));
    assert_eq!(textes(&v), ["1 alerte", "1 attention", "1 conseil", "1 info", "2 OK"]);
    assert_eq!(textes(&[]), ["0 attention", "0 conseil", "0 info", "0 OK"]);
}

#[test]
fn la_ligne_des_ok_compte_et_liste_les_titres_en_minuscules() {
    let oks = [
        constat("a", regles::OK, "Retard des filtres gyro"),
        constat("b", regles::OK, "RPM bien filtré"),
    ];
    let (tete, liste) = oks_line(&oks);
    assert_eq!(tete, "2 vérifications OK");
    // un sigle garde ses capitales
    assert_eq!(liste, "retard des filtres gyro, RPM bien filtré");
    assert_eq!(oks_line(&oks[..1]).0, "1 vérification OK");
}

#[test]
fn une_info_est_repliee_au_depart_et_une_carte_se_reconnait_par_sa_cle() {
    assert!(!open_by_default(&constat("i", regles::INFO, "I")));
    for s in [regles::ALERTE, regles::ATTENTION, regles::CONSEIL] {
        assert!(open_by_default(&constat("x", s, "X")), "{s}");
    }
    // même clé, autre titre : même carte (D-028) ; même clé, autre axe : autre carte
    let mut a = constat("step_slow", regles::ATTENTION, "Réponse lente");
    let b = constat("step_slow", regles::ATTENTION, "Réponse très lente");
    assert_eq!(card_id(&a), card_id(&b));
    a.axis = Some("Roll".into());
    assert_ne!(card_id(&a), card_id(&b));
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
            compare: None,
        },
        st.inputs(&[0]),
    );
    assert!(r.is_err());
    assert!(
        AdvisorTool::compute(
            Options {
                profile: None,
                compare: None
            },
            Vec::new()
        )
        .is_err()
    );
}

// ------------------------------------------------------------------ Assistant : dans la fenêtre

fn banc(theme: Theme) -> Harness<'static, App> {
    banc_page(theme, "Assistant")
}

fn banc_page(theme: Theme, titre: &str) -> Harness<'static, App> {
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
        .position(|p| p.title() == titre)
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
        h.get_by_label_contains("période 0,0 – 5,1 s");
        h.get_by_label("Taille du drone");
        h.get_by_label(FOOTER);
        h.get_by_label(HELP);
        h.get_by_label(ONE_CHANGE);
        h.get_by_label(STEP_TITLE);
        let out = calcule(None, &h.state().state);
        for (_, t) in pills(&out.report.findings) {
            h.get_by_label(&t);
        }
        for ax in ["Roll", "Pitch", "Yaw"] {
            h.get_by_label(ax);
        }
        assert!(
            h.query_all_by_label("données insuffisantes").count() >= 1,
            "roll sans fenêtre"
        );
        // une carte par constat qui n'est pas OK, avec son bouton pour la replier ou la déplier
        let (cartes, oks) = split_cards(&out.report);
        assert!(!cartes.is_empty() && !oks.is_empty(), "le log de 5 s a des deux");
        for f in &cartes {
            let verbe = if open_by_default(f) { "Replier" } else { "Déplier" };
            h.get_by_label(&format!("{verbe} « {} »", safe_text(&f.title)));
        }
        h.get_by_label(&oks_line(&oks).0);
    }
}

#[test]
fn une_info_se_deplie_et_les_ok_s_affichent_au_clic() {
    let mut h = banc(CLAIR);
    let out = calcule(None, &h.state().state);
    let (cartes, oks) = split_cards(&out.report);
    // une info : titre seul, son détail n'apparaît qu'une fois dépliée
    let info = cartes
        .iter()
        .find(|f| f.severity == regles::INFO)
        .expect("une info sur ce log");
    let detail = safe_text(&info.detail);
    assert!(h.query_by_label(&detail).is_none(), "info repliée au départ");
    h.cliquer_label(&format!("Déplier « {} »", safe_text(&info.title)));
    h.run_steps(3);
    h.get_by_label(&detail);
    h.get_by_label(&format!("Replier « {} »", safe_text(&info.title)));
    // les vérifications OK : leur ligne seulement, puis leurs cartes une fois dépliées
    let ok = &oks[0];
    assert!(
        h.query_by_label(&format!("Replier « {} »", safe_text(&ok.title)))
            .is_none()
    );
    h.cliquer_label("Afficher les vérifications OK");
    h.run_steps(3);
    h.get_by_label(&format!("Replier « {} »", safe_text(&ok.title)));
    h.get_by_label("Masquer les vérifications OK");
}

#[test]
fn choisir_un_profil_change_le_drone_affiche() {
    let mut h = banc(CLAIR);
    assert!(
        h.query_by_label_contains("choisi par l'utilisateur").is_none(),
        "détection automatique au départ"
    );
    h.cliquer_label("Taille du drone");
    h.run_steps(3);
    let whoop = safe_text(otb_advisor::profiles::profile("whoop").unwrap().label);
    h.cliquer_label(&whoop);
    attendre(&mut h);
    h.get_by_label_contains("choisi par l'utilisateur");
    // la valeur affichée a changé : l'en-tête du fragment porte le profil choisi
    let Screen::Tool(i) = h.state().screen else {
        panic!("page d'outil")
    };
    assert!(!h.state().pages[i].busy());
    h.get_by_label_contains(&format!("{whoop} (choisi par l'utilisateur)"));
}

// ------------------------------------------------------------------ Assistant : vol comparé et choix (lot 31d)

fn vol(id: &str, log_id: &str, date: &str, start: u64) -> Fragment {
    Fragment {
        id: id.into(),
        log_id: log_id.into(),
        session: 1,
        start,
        end: start + 100,
        name: id.to_uppercase(),
        role: String::new(),
        date: date.into(),
        chosen: true,
    }
}

/// Deux drones : « d1 » a trois vols (deux le 20, un le 22), « d2 » un seul.
fn bibliotheque_de_vols() -> Library {
    let mut lib = Library::in_memory();
    lib.put_drone(Drone::new("d1", "JeNo 5"));
    lib.put_drone(Drone::new("d2", "Whoop"));
    for (id, drone) in [("l1", "d1"), ("l2", "d1"), ("lw", "d2")] {
        lib.add_log(LogEntry {
            id: id.into(),
            drone_id: drone.into(),
            name: id.into(),
            path: format!("/logs/{id}.bbl"),
            copied: false,
            bytes: 1,
            added: "2026-11-20".into(),
        });
    }
    lib.put_fragment(vol("v3", "l2", "2026-11-22", 0));
    lib.put_fragment(vol("v1", "l1", "2026-11-20", 0));
    lib.put_fragment(vol("v2", "l1", "2026-11-20", 500));
    lib.put_fragment(vol("w1", "lw", "2026-11-21", 0));
    lib.add_to_analysis("w1");
    lib
}

#[test]
fn le_vol_compare_est_le_precedent_du_drone_sauf_choix_du_pilote() {
    let lib = bibliotheque_de_vols();
    assert_eq!(advice::previous_flight(&lib, "v2").map(|f| f.id.as_str()), Some("v1"));
    assert!(advice::previous_flight(&lib, "v1").is_none(), "premier vol du drone");
    let plus_recents: Vec<&str> = advice::newer_flights(&lib, "v1")
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(plus_recents, ["v2", "v3"]);
    assert!(advice::newer_flights(&lib, "v3").is_empty());
    assert_eq!(
        advice::newer_text(&advice::newer_flights(&lib, "v1")),
        "· 2 vols plus récents : V2, V3"
    );
    // au choix : les autres vols du drone, du plus récent au plus ancien, puis la liste d'analyse (un autre drone)
    let choix: Vec<&str> = advice::candidates(&lib, "v2").iter().map(|f| f.id.as_str()).collect();
    assert_eq!(choix, ["v3", "v1", "w1"]);

    let auto = advice::compare_target(&lib, "v3", None).expect("vol précédent");
    assert_eq!(auto.fragment.id, "v2");
    assert!(auto.automatic);
    assert_eq!(auto.meta, "vol précédent · 2026-11-20");
    assert_eq!(auto.path, PathBuf::from("/logs/l1.bbl"));
    let choisi = advice::compare_target(&lib, "v3", Some("w1")).unwrap();
    assert_eq!((choisi.fragment.id.as_str(), choisi.automatic), ("w1", false));
    assert_eq!(choisi.meta, "choisi · Whoop · 2026-11-21");
    // un vol choisi puis supprimé, ou le fragment lui-même : retour au vol précédent
    assert_eq!(
        advice::compare_target(&lib, "v3", Some("parti")).unwrap().fragment.id,
        "v2"
    );
    assert_eq!(
        advice::compare_target(&lib, "v3", Some("v3")).unwrap().fragment.id,
        "v2"
    );
    assert!(
        advice::compare_target(&lib, "v1", None).is_none(),
        "« aucun vol à comparer »"
    );
    assert!(advice::compare_target(&lib, "w1", None).is_none());
}

#[test]
fn la_comparaison_marque_les_cartes_nouvelles_disparues_et_le_repere_des_jauges() {
    let st = etat();
    let rapport = calcule(None, &st).report;
    let (cartes, _) = split_cards(&rapport);
    let avec_jauge = cartes
        .iter()
        .find(|f| f.gauge.is_some())
        .expect("une carte à jauge sur ce log");
    let sans_jauge = cartes.iter().find(|f| f.gauge.is_none()).expect("une carte sans jauge");
    let mut avant = (*rapport).clone();
    // le vol comparé : la carte sans jauge n'y était pas, la carte à jauge y valait 99, une carte a disparu depuis
    avant.findings.retain(|f| card_id(f) != card_id(sans_jauge));
    for f in avant.findings.iter_mut().filter(|f| card_id(f) == card_id(avec_jauge)) {
        f.gauge.as_mut().unwrap().value = 99.0;
    }
    let mut partie = sans_jauge.clone();
    partie.key = "carte_disparue".into();
    partie.title = "Ancien souci".into();
    avant.findings.push(partie);
    let c = advice::compare_cards("FINAL.02", &rapport, &avant);
    assert_eq!(c.name, "FINAL.02");
    assert!(c.new_cards.contains(&card_id(sans_jauge)));
    assert!(!c.new_cards.contains(&card_id(avec_jauge)));
    assert_eq!(c.values.get(&card_id(avec_jauge)), Some(&99.0));
    assert_eq!(c.gone, ["Ancien souci"]);
    assert_eq!(gone_text(&c), "Plus signalé depuis FINAL.02 : Ancien souci");
    // une jauge d'une autre unité ne pose pas de repère (il serait faux)
    for f in avant.findings.iter_mut().filter(|f| card_id(f) == card_id(avec_jauge)) {
        f.gauge.as_mut().unwrap().unit = "unité inconnue";
    }
    assert!(
        !advice::compare_cards("X", &rapport, &avant)
            .values
            .contains_key(&card_id(avec_jauge))
    );
    // le même vol des deux côtés : rien de nouveau, rien de parti
    let meme = advice::compare_cards("X", &rapport, &rapport);
    assert!(meme.new_cards.is_empty() && meme.gone.is_empty());
    assert_eq!(meme.values.len(), cartes.iter().filter(|f| f.gauge.is_some()).count());
    let _ = Comparison::default();
}

#[test]
fn l_historique_dit_quand_et_par_quel_vol_un_conseil_a_ete_pris_en_compte() {
    let lib = bibliotheque_de_vols();
    let choix = |applique: bool, quand: &str| Advice {
        fragment_id: "v1".into(),
        card: "step_latency|Roll".into(),
        title: "Réponse lente".into(),
        applied: applique,
        reason: if applique {
            String::new()
        } else {
            "déjà essayé".into()
        },
        when: quand.into(),
    };
    // appliqué le 21 : V2 (le 20) a volé avant, V3 (le 22) l'a pris en compte
    let h = advice::history_line(&lib, &choix(true, "2026-11-21T09:00:00Z"));
    assert!(h.applied);
    assert_eq!(h.status, "Appliqué le 21/11/2026");
    assert_eq!(h.detail, "Pris en compte par V3");
    assert_eq!(h.title, "Réponse lente");
    let h = advice::history_line(&lib, &choix(true, "2026-11-23T09:00:00Z"));
    assert_eq!(h.detail, "En attente du prochain vol");
    let h = advice::history_line(&lib, &choix(false, "2026-11-21T09:00:00Z"));
    assert_eq!(
        (h.status.as_str(), h.detail.as_str()),
        ("Écarté le 21/11/2026", "Raison : « déjà essayé »")
    );
    assert_eq!(advice::day_text("2026-11-03T23:59:59Z"), "03/11/2026");
    assert_eq!(advice::day_text("hier"), "hier");
    assert_eq!(advice::iso_of_secs(0), "1970-01-01T00:00:00Z");
    assert_eq!(advice::iso_of_secs(1_763_918_765), "2025-11-23T17:26:05Z");
    assert!(advice::now_iso().ends_with('Z'));
}

#[test]
fn seuls_les_conseils_qui_proposent_un_geste_se_votent() {
    let st = etat();
    let rapport = calcule(None, &st).report;
    let (cartes, oks) = split_cards(&rapport);
    assert!(oks.iter().all(|f| !advice::can_vote(f)));
    for f in &cartes {
        assert_eq!(advice::can_vote(f), !f.actions.is_empty(), "{}", f.title);
    }
    assert!(
        cartes.iter().any(advice::can_vote),
        "au moins un conseil à voter sur ce log"
    );
}

/// Le fragment analysé par la page (celui du log ouvert) et la bibliothèque de la fenêtre.
fn fragment_ouvert(h: &Harness<'static, App>) -> Fragment {
    let id = h.state().state.logs[0]
        .fragment
        .clone()
        .expect("fragment rangé à l'ouverture");
    h.state().library.fragment(&id).cloned().unwrap()
}

/// Ajoute au drone du log ouvert un vol de la même session, daté `date`, sur la seconde moitié du log.
fn ajoute_un_vol(h: &mut Harness<'static, App>, id: &str, date: &str) {
    let mut f = fragment_ouvert(h);
    let milieu = (f.start + f.end) / 2;
    f.id = id.into();
    f.name = id.into();
    f.date = date.into();
    f.start = milieu;
    h.state_mut().library.put_fragment(f);
    attendre(h);
}

/// Le premier conseil à voter, dans l'ordre des cartes (celui du premier bouton « pouce » trouvé).
fn premier_conseil(h: &Harness<'static, App>) -> Finding {
    let out = calcule(None, &h.state().state);
    split_cards(&out.report)
        .0
        .into_iter()
        .find(advice::can_vote)
        .expect("un conseil à voter")
}

const UP: &str = "Conseil appliqué : je l'ai réglé sur le drone";
const DOWN: &str = "Conseil écarté : indiquer la raison";

#[test]
fn sans_vol_precedent_l_en_tete_le_dit() {
    let mut h = banc(CLAIR);
    h.run();
    h.get_by_label("comparé à");
    h.get_by_label("Vol comparé");
    h.get_by_label(NO_COMPARE_META);
    assert_eq!(h.get_by_label("Vol comparé").value().as_deref(), Some(NO_COMPARE));
    assert!(
        h.query_by_label("Fragment déjà traité").is_none(),
        "le drone n'a pas volé depuis"
    );
}

#[test]
fn le_vol_precedent_du_drone_est_compare_sans_etre_ouvert() {
    for theme in [CLAIR, SOMBRE] {
        let mut h = banc(theme);
        ajoute_un_vol(&mut h, "AVANT", "2000-01-01");
        h.run();
        // il n'est pas dans la liste d'analyse : le fil de calcul a lu son fichier
        assert_eq!(h.state().state.logs.len(), 1);
        h.get_by_label("vol précédent · 2000-01-01");
        assert!(h.query_all_by_label_contains("AVANT").count() >= 1);
        // au moins une jauge porte le repère du vol comparé
        assert!(
            h.query_all_by_label_contains("AVANT : ").count() >= 1,
            "repère du vol comparé sous une jauge"
        );
        assert!(h.query_by_label_contains("analyse impossible").is_none());
    }
}

#[test]
fn un_pouce_leve_s_inscrit_tout_de_suite_et_un_second_clic_l_annule() {
    let mut h = banc(CLAIR);
    let frag = fragment_ouvert(&h).id;
    let conseil = premier_conseil(&h);
    h.cliquer(|h| h.get_all_by_label(UP).next().unwrap().accesskit_node().locate());
    h.run_steps(3);
    let lib = &h.state().library;
    let a = lib.advice(&frag, &card_id(&conseil)).expect("choix inscrit au clic");
    assert!(a.applied);
    assert_eq!(a.title, conseil.title);
    let jour = advice::day_text(&a.when);
    h.get_by_label(&format!("Appliqué le {jour}"));
    h.get_by_label("· en attente du prochain vol");
    h.cliquer(|h| h.get_all_by_label(UP).next().unwrap().accesskit_node().locate());
    h.run_steps(3);
    assert!(
        h.state().library.advice_of(&frag).is_empty(),
        "second clic : choix annulé"
    );
}

#[test]
fn un_pouce_baisse_n_inscrit_rien_sans_raison() {
    let mut h = banc(CLAIR);
    let frag = fragment_ouvert(&h).id;
    let conseil = premier_conseil(&h);
    h.cliquer(|h| h.get_all_by_label(DOWN).next().unwrap().accesskit_node().locate());
    h.run_steps(3);
    assert!(
        h.state().library.advice_of(&frag).is_empty(),
        "rien avant « Enregistrer »"
    );
    assert!(
        h.get_by_label("Enregistrer").accesskit_node().is_disabled(),
        "raison vide"
    );
    // un clic passe le champ en saisie, puis le texte
    let champ = || egui::accesskit::Role::MultilineTextInput;
    h.cliquer_role(champ(), REASON_QUESTION);
    h.run_steps(2);
    h.get_by_role_and_label(champ(), REASON_QUESTION)
        .type_text("hélice abîmée");
    h.run_steps(3);
    h.cliquer_label("Enregistrer");
    h.run_steps(3);
    let a = h
        .state()
        .library
        .advice(&frag, &card_id(&conseil))
        .expect("choix inscrit")
        .clone();
    assert!(!a.applied);
    assert_eq!(a.reason, "hélice abîmée");
    h.get_by_label("Raison : « hélice abîmée »");
}

#[test]
fn un_fragment_deja_traite_montre_l_historique_de_ses_choix() {
    let mut h = banc(CLAIR);
    ajoute_un_vol(&mut h, "APRES", "2099-01-01");
    h.run();
    h.get_by_label("Fragment déjà traité");
    h.get_by_label("· 1 vol plus récent : APRES");
    h.get_by_label(NO_HISTORY);
    h.cliquer(|h| h.get_all_by_label(UP).next().unwrap().accesskit_node().locate());
    h.run_steps(3);
    assert!(h.query_by_label(NO_HISTORY).is_none());
    // le vol daté après le jour du choix l'a pris en compte
    h.get_by_label("Pris en compte par APRES");
    h.get_by_label("· pris en compte par APRES");
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

/// La Configuration a son entrée dans le rail, comme dans le prototype (D-060) : elle était cachée sous
/// « Courbes », qui n'a plus d'entrée depuis que le visualiseur est l'écran d'extraction (lot 29b).
#[test]
fn la_page_configuration_a_son_entree_dans_le_rail() {
    let app = App::new(otb_gui::tools::all_pages());
    let p = app.pages.iter().find(|p| p.title() == "Configuration").unwrap();
    assert!(!p.hidden());
    assert_eq!(p.tab(), "Configuration");
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
        // les deux fragments de la barre sont actifs : deux colonnes (D-066)
        assert!(h.query_by_label("Firmware").is_some());
        assert!(h.query_by_label("Nom du drone").is_some());
        h.get_by_label("Général");
        assert!(h.query_by_label("Autre drone").is_some(), "colonne du second fragment");
    }
}

#[test]
fn deux_logs_cote_a_cote_alignes_par_nom_avec_differences_et_recherche() {
    let mut h = banc_config(CLAIR, true);
    h.run();
    h.get_by_label("Autre drone");
    h.get_by_label("JeNo Ced");
    // le curseur propre au second log a sa ligne (« — » pour le premier), au bout de son groupe
    h.get_by_label("pi_gain");
    let Screen::Tool(i) = h.state().screen else { panic!() };
    assert!(!h.state().pages[i].busy());
    // seulement les différences : le nom du drone et le curseur restent, la carte (identique) disparaît
    h.cliquer_label("Seulement les différences");
    h.run_steps(3);
    assert!(h.query_by_label("Nom du drone").is_some());
    assert!(h.query_by_label("pi_gain").is_some());
    assert!(h.query_by_label("Carte").is_none(), "ligne identique masquée");
    assert!(h.query_by_label("Rates").is_none(), "groupe sans ligne visible masqué");
    h.cliquer_label("Seulement les différences");
    h.run_steps(3);
    assert!(h.query_by_label("Carte").is_some());
    // paramètres bruts : la clé propre au second log apparaît
    assert!(h.query_by_label("zz_propre_au_second").is_none());
    h.cliquer_label("Tous les paramètres bruts");
    h.run_steps(3);
    h.get_by_label("zz_propre_au_second");
    h.get_by_label(setup::RAW_TITLE);
    // recherche insensible à la casse, dans les valeurs aussi
    h.cliquer_label("Rechercher un paramètre");
    h.get_by_label("Rechercher un paramètre").type_text("AUTRE DRONE");
    h.run_steps(3);
    h.get_by_label("Nom du drone");
    assert!(h.query_by_label("Carte").is_none());
    assert!(h.query_by_label("pi_gain").is_none());
}

/// L'entrée « Configuration » du rail est mise en valeur (état « coché » d'accesskit).
fn configuration_en_valeur(h: &Harness<'static, App>) -> bool {
    h.get_by_label("Configuration").accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
}

/// L'entrée du rail ouvre la page et s'allume ; sur un autre outil, elle s'éteint.
#[test]
fn l_entree_configuration_du_rail_ouvre_la_page_et_s_allume() {
    let mut h = banc(CLAIR);
    h.state_mut().screen = Screen::Dashboard;
    h.run_steps(4);
    assert!(!configuration_en_valeur(&h));
    h.cliquer_label("Configuration");
    attendre(&mut h);
    h.run_steps(2);
    let Screen::Tool(i) = h.state().screen else {
        panic!(
            "la page de la Configuration doit s'ouvrir, écran : {:?}",
            h.state().screen
        )
    };
    assert_eq!(h.state().pages[i].title(), "Configuration");
    assert!(configuration_en_valeur(&h));
    h.get_by_label("Firmware");
    let j = h.state().pages.iter().position(|p| p.title() == "Assistant").unwrap();
    h.state_mut().screen = Screen::Tool(j);
    attendre(&mut h);
    assert!(!configuration_en_valeur(&h));
}

/// Sans log, l'entrée ouvre quand même la page, qui invite à ouvrir un log (comme toutes les entrées du rail).
#[test]
fn l_entree_configuration_sans_log_invite_a_ouvrir_un_log() {
    let app = App::new(otb_gui::tools::all_pages());
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 950.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app);
    h.run_steps(4);
    h.cliquer_label("Configuration");
    h.run_steps(4);
    assert!(matches!(h.state().screen, Screen::Tool(_)));
    h.get_by_label("Ouvrez un log pour commencer.");
}

#[test]
fn la_selection_de_la_configuration_est_multiple_et_limitee_a_dix() {
    let mut st = AppState::new();
    for _ in 0..3 {
        st.add(log5s());
    }
    let t = SetupTool::new();
    assert_eq!(
        t.selected(&st),
        vec![0, 1, 2],
        "les fragments actifs de la barre (D-066)"
    );
    st.set_active(1, false);
    assert_eq!(t.selected(&st), vec![0, 2], "une pastille désactivée perd sa colonne");
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

/// Moyenne de `a − b` (dB) sur les fréquences de [`lo`, `hi`[.
fn band_db(f: &[f64], a: &[f64], b: &[f64], lo: f64, hi: f64) -> f64 {
    let v: Vec<f64> = f
        .iter()
        .zip(a.iter().zip(b))
        .filter(|(x, _)| **x >= lo && **x < hi)
        .map(|(_, (p, q))| p - q)
        .collect();
    v.iter().sum::<f64>() / v.len() as f64
}

/// Spectres enregistrés : ceux de Python. Spectres simulés : depuis le lot 22e (D-058), la chaîne de filtres est rejouée et
/// non plus déduite du rapport des réponses |H|² ; leurs valeurs ne sont plus celles de Python (changement voulu), seule leur
/// présence l'est. On vérifie à la place ce que la méthode garantit.
#[test]
fn les_spectres_simules_sont_ceux_de_python() {
    let st = etat();
    let c = &cas()["sim"];
    let log = &*st.logs[0].log;
    let base = &log.settings;
    assert_eq!(filters_of(base), c["base"], "les réglages du log");
    let sel = sim_sel(c, &st);
    let (mut nones, mut nans, mut nb, mut simulated) = (0, 0, 0, 0);
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
                    (Some(g), false) if !key.contains("_sim") => {
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
                    (Some(g), false) => {
                        assert!(g.iter().all(|v| v.is_finite()), "{what} {key}[{ax}]");
                        nb += 1;
                        simulated += 1;
                    }
                    _ => nones += 1,
                }
            }
        }
        for ax in 0..2 {
            for (logged, sim, cur) in [("gyro", "gyro_sim", "gyro_sim_cur"), ("d", "d_sim", "d_sim_cur")] {
                let (l, m) = (res.spectrum(logged, ax).unwrap(), res.spectrum(sim, ax).unwrap());
                if name == "same" {
                    // réglages inchangés : exactement le log
                    assert_eq!(l, m, "{what} {sim}[{ax}]");
                }
                if name == "off" {
                    // aucun filtre : jamais moins de bruit que le log
                    for (lo, hi) in [(20.0, 100.0), (100.0, 250.0), (250.0, 500.0)] {
                        let e = band_db(&res.freqs, m, l, lo, hi);
                        assert!(e > -0.01, "{what} {sim}[{ax}] {lo}–{hi} Hz : {e:+.3} dB");
                    }
                }
                if let Some(c) = res.spectrum(cur, ax) {
                    // le rejeu des réglages actuels juge le modèle : proche du log sans le recopier
                    let e = band_db(&res.freqs, c, l, 20.0, 250.0);
                    assert!(e.abs() < 1.0 && c != l, "{what} {cur}[{ax}] : {e:+.3} dB");
                }
            }
        }
    }
    assert!(
        nones == 115 && nb == 245 && nans == 15 && simulated == 95,
        "{nones} {nb} {nans} {simulated}"
    );
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
    // rien de rejoué : aucun écart au vol à montrer
    assert!(res.flight_gap.is_none());
}

#[test]
fn l_ecart_au_vol_donne_la_pire_bande_de_chaque_signal_et_sa_cause() {
    let b = |lo, hi, gyro_db, d_db| BandGap {
        lo_hz: lo,
        nominal_hi_hz: hi,
        hi_hz: hi.min(721.0),
        gyro_db,
        d_db,
    };
    let g = FlightGap {
        bands: vec![
            b(20.0, 100.0, 0.08, -0.4),
            b(100.0, 250.0, -0.36, 1.13),
            b(250.0, 500.0, 0.2, f64::NAN),
        ],
        causes: vec![
            GapCause::Slowed {
                loops_per_sample: 4,
                log_hz: 999.6,
            },
            GapCause::OldFirmware,
        ],
    };
    assert_eq!(
        fsc::flight_gap_line(&g).unwrap(),
        "Écart de la simulation au vol : gyro 0,4 dB (100 à 250 Hz), terme D 1,1 dB (100 à 250 Hz)"
    );
    let detail = fsc::flight_gap_detail(&g);
    assert!(detail.starts_with(fsc::FLIGHT_GAP_HINT), "{detail}");
    assert!(detail.ends_with("\n20 à 100 Hz : gyro +0,08 dB, terme D -0,40 dB\n100 à 250 Hz : gyro -0,36 dB, terme D +1,13 dB\n250 à 500 Hz : gyro +0,20 dB, terme D —"), "{detail}");
    assert!(fsc::gap_cause_text(&g.causes[0]).starts_with("Ce log ne garde qu'une boucle PID sur 4 (log à 1000 Hz) :"));
    assert!(fsc::gap_cause_text(&g.causes[1]).starts_with("Firmware d'avant la 4.3"));
    // sans aucune bande mesurée, pas de ligne
    assert!(
        fsc::flight_gap_line(&FlightGap {
            bands: vec![],
            causes: vec![]
        })
        .is_none()
    );
}

#[test]
fn le_simulateur_mesure_l_ecart_au_vol_du_log_de_5_s() {
    let st = etat();
    let log = &*st.logs[0].log;
    let base = &log.settings;
    let res = simulate_filters(log, st.logs[0].sel(), base, base, false).unwrap();
    let g = res.flight_gap.as_ref().expect("gyro non filtré : écart mesuré");
    assert!(g.worst_gyro().is_some() && g.worst_d().is_some(), "{g:?}");
    assert!(
        matches!(
            g.causes[..],
            [GapCause::Slowed {
                loops_per_sample: 2,
                ..
            }]
        ),
        "{g:?}"
    );
    // l'écart ne dépend que des réglages du log, pas de ceux qu'on essaie
    let mut ed = Editor::from_settings(base);
    ed.g2_hz = 250;
    let res2 = simulate_filters(log, st.logs[0].sel(), base, &ed.modified_settings(base), false).unwrap();
    assert_eq!(res2.flight_gap.as_ref(), Some(g));
}

// ------------------------------------------------------------------ Simulateur de filtres : interface

const SIM: &str = "Simulateur de filtres";

fn sim_busy(h: &Harness<'static, App>) -> bool {
    let Screen::Tool(i) = h.state().screen else {
        panic!("pas d'outil affiché")
    };
    h.state().pages[i].busy()
}

/// Texte du cadre CLI (zone de texte en lecture seule).
fn cli_text(h: &Harness<'static, App>) -> String {
    h.get_by_label("CLI des filtres modifiés")
        .accesskit_node()
        .value()
        .expect("le cadre CLI a un texte")
}

/// Met `valeur` dans le champ nommé `nom` : un clic le passe en saisie, puis le texte et Entrée.
fn saisir(h: &mut Harness<'static, App>, nom: &str, valeur: &str) {
    h.cliquer_label(nom);
    h.run_steps(2);
    h.get_by_label(nom).type_text(valeur);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
}

/// Coupe le curseur de filtre gyro du log de 5 s (mis à 1,50, fréquences concordantes) : ses fréquences redeviennent
/// saisissables (D-080).
fn couper_curseur_gyro(h: &mut Harness<'static, App>) {
    assert!(h.get_by_label("Gyro LPF2 coupure").accesskit_node().is_disabled());
    h.cliquer_label("Multiplicateur du filtre gyro");
    h.run_steps(3);
    assert!(!h.get_by_label("Gyro LPF2 coupure").accesskit_node().is_disabled());
}

#[test]
fn le_simulateur_se_calcule_et_s_affiche_dans_les_deux_themes() {
    for theme in [CLAIR, SOMBRE] {
        let mut h = banc_page(theme, SIM);
        h.run(); // contrôle d'accessibilité de kittest (L-068)
        for t in [
            "Filtres Gyro — gain",
            "Filtres Gyro — phase",
            "Filtres D — gain",
            "Filtres D — phase",
        ] {
            h.get_by_label(t);
        }
        h.get_by_label("Spectre gyro (R+P+Y)");
        h.get_by_label("Spectre D (R+P+Y)");
        h.get_by_label_contains("Retard des filtres (ms)");
        h.get_by_label("Gyro + D");
        h.get_by_label("CLI des filtres modifiés :");
        h.get_by_label("Copier");
        h.get_by_label("Appliquer");
        h.get_by_label("Réinitialiser");
        for nom in [
            "Gyro LPF1 type",
            "Gyro LPF1 mode",
            "Gyro LPF2 coupure",
            "Gaz (Bode)",
            "Moteur (Bode)",
        ] {
            h.get_by_label(nom);
        }
        assert!(h.query_by_label(STALE_STATUS).is_none(), "rien n'a changé");
        assert!(h.query_by_label(NO_UNFILT_TITLE).is_none());
        // écart au vol et sa cause : le log de 5 s garde une boucle PID sur 2
        h.get_by_label_contains("Écart de la simulation au vol : gyro ");
        h.get_by_label_contains("Ce log ne garde qu'une boucle PID sur 2 (log à 1603 Hz)");
    }
}

#[test]
fn les_trois_onglets_ont_tous_leurs_champs_nommes() {
    let mut h = banc_page(CLAIR, SIM);
    // le terme D s'atteint par le choix « Gyro | Terme D » de l'espace (D-069), les notchs dynamiques par la page de
    // la chaîne du gyro : on revient au gyro avant de l'ouvrir
    for (nom, clics) in [
        ("D", &["Terme D"][..]),
        ("Notch dyn / RPM", &["Gyro", "Notch dyn / RPM"][..]),
    ] {
        for c in clics {
            h.cliquer_role(egui::accesskit::Role::Button, c);
            h.run();
        }
        let champs: &[&str] = if nom == "D" {
            &[
                "D LPF1 type",
                "D LPF1 statique",
                "D LPF2 coupure",
                "D notch centre",
                "D notch coupure",
            ]
        } else {
            &[
                "Notch dynamique nombre",
                "Notch dynamique Q",
                "Filtre RPM harmoniques",
                "Filtre RPM poids 3",
            ]
        };
        for c in champs {
            h.get_by_label(c);
        }
    }
}

#[test]
fn modifier_un_champ_ne_lance_aucun_calcul_et_appliquer_simule() {
    let mut h = banc_page(CLAIR, SIM);
    couper_curseur_gyro(&mut h);
    assert!(cli_text(&h).contains("aucun changement"), "{}", cli_text(&h));
    saisir(&mut h, "Gyro LPF2 coupure", "250");
    assert!(!sim_busy(&h), "les champs ne relancent rien");
    h.get_by_label(STALE_STATUS);
    let cli = cli_text(&h);
    assert!(cli.contains("set gyro_lpf2_static_hz = 250"), "{cli}");
    assert!(!cli.contains("aucun changement"));
    h.cliquer_label("Appliquer");
    h.run_steps(2);
    assert!(sim_busy(&h), "« Appliquer » lance le calcul");
    attendre(&mut h);
    assert!(
        h.query_by_label(STALE_STATUS).is_none(),
        "le calcul correspond à l'éditeur"
    );
    assert!(cli_text(&h).contains("set gyro_lpf2_static_hz = 250"));
    // « Réinitialiser » recharge l'éditeur : plus de changement, mais le dernier calcul n'est plus celui de l'éditeur
    h.cliquer_label("Réinitialiser");
    h.run_steps(3);
    assert!(cli_text(&h).contains("aucun changement"));
    h.get_by_label(STALE_STATUS);
    assert!(!sim_busy(&h));
}

#[test]
fn le_bouton_copier_affiche_le_statut_de_copie() {
    let mut h = banc_page(SOMBRE, SIM);
    assert!(h.query_by_label(COPIED_STATUS).is_none());
    h.cliquer_label("Copier");
    h.run_steps(3);
    h.get_by_label(COPIED_STATUS);
}

#[test]
fn un_log_sans_gyro_non_filtre_affiche_le_titre_d_alerte() {
    let mut log = log5s();
    log.gyro_unfilt = None;
    let mut app = App::new(otb_gui::tools::all_pages());
    app.theme = CLAIR;
    let mut h = Harness::builder()
        .with_size(egui::vec2(1600.0, 950.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app);
    h.state_mut().state.add(log);
    let i = h.state().pages.iter().position(|p| p.title() == SIM).unwrap();
    h.state_mut().screen = Screen::Tool(i);
    attendre(&mut h);
    h.run();
    h.get_by_label(NO_UNFILT_TITLE);
    h.get_by_label("Spectre D (R+P+Y)");
}

#[test]
fn le_simulateur_sert_le_cache_sauf_si_les_reglages_ou_la_periode_changent() {
    let mut st = etat();
    let base = st.logs[0].log.settings.clone();
    let opts = |applied: Option<Applied>, with_current| SimOptions { applied, with_current };
    let a = FilterSimTool::compute(opts(None, true), st.inputs(&[0])).unwrap();
    let b = FilterSimTool::compute(opts(None, true), st.inputs(&[0])).unwrap();
    assert!(Arc::ptr_eq(&a, &b), "mêmes réglages et même période : repris");
    // l'éditeur tel que chargé et ses réglages appliqués tels quels donnent le même calcul
    let id = st.logs[0].uid;
    let same = Applied {
        log_id: id,
        settings: Editor::from_settings(&base).modified_settings(&base),
    };
    assert!(Arc::ptr_eq(
        &a,
        &FilterSimTool::compute(opts(Some(same), true), st.inputs(&[0])).unwrap()
    ));
    let mut changed = Editor::from_settings(&base);
    changed.g2_hz = 250;
    let ap = Applied {
        log_id: id,
        settings: changed.modified_settings(&base),
    };
    let c = FilterSimTool::compute(opts(Some(ap.clone()), true), st.inputs(&[0])).unwrap();
    assert!(!Arc::ptr_eq(&a, &c), "d'autres réglages");
    assert!(Arc::ptr_eq(
        &c,
        &FilterSimTool::compute(opts(Some(ap.clone()), true), st.inputs(&[0])).unwrap()
    ));
    let d = FilterSimTool::compute(opts(Some(ap.clone()), false), st.inputs(&[0])).unwrap();
    assert!(!Arc::ptr_eq(&c, &d), "la case « actuel » change le résultat");
    // des réglages appliqués à un autre log ne servent pas : retour à l'éditeur chargé
    let other = Applied { log_id: id + 1, ..ap };
    assert!(Arc::ptr_eq(
        &a,
        &FilterSimTool::compute(opts(Some(other), true), st.inputs(&[0])).unwrap()
    ));
    let (t0, t1) = st.logs[0].period;
    st.logs[0].period = (t0 + 0.5, t1 - 0.5);
    let e = FilterSimTool::compute(opts(None, true), st.inputs(&[0])).unwrap();
    assert!(!Arc::ptr_eq(&a, &e), "une autre période");
    assert!(FilterSimTool::compute(opts(None, true), Vec::new()).is_err());
}

// ------------------------------------------------------------------ Simulateur : règles d'affichage

#[test]
fn la_moyenne_des_axes_se_fait_en_lineaire_puis_revient_en_db() {
    use otb_gui::tools::filter_sim::mean_db;
    // 0 dB et 10 dB : en linéaire (1 + 10) / 2 = 5,5, soit 7,40 dB (et non 5 dB, la moyenne des dB)
    let m = mean_db(&[&[0.0], &[10.0]]);
    assert!((m[0] - 10.0 * 5.5f64.log10()).abs() < 1e-12, "{m:?}");
    // plancher à 1e-12 (-120 dB) : un spectre nul ne donne pas -inf
    assert!((mean_db(&[&[-300.0]])[0] + 120.0).abs() < 1e-9);
    // longueur = la plus courte des courbes
    assert_eq!(mean_db(&[&[1.0, 2.0, 3.0], &[1.0, 2.0]]).len(), 2);
}

#[test]
fn un_retard_est_vert_rouge_ou_inchange_a_5_microsecondes_pres() {
    use otb_gui::tools::filter_sim::{Trend, delay_trend};
    assert_eq!(delay_trend(1.0, 0.99), Trend::Shorter);
    assert_eq!(delay_trend(1.0, 1.01), Trend::Longer);
    assert_eq!(delay_trend(1.0, 1.004), Trend::Same);
    assert_eq!(delay_trend(1.0, 0.996), Trend::Same);
    assert_eq!(delay_trend(1.0, 1.0), Trend::Same);
}

/// 2026-11-06 : les spectres du Simulateur étaient calés sur le sommet de leurs courbes (sommet −70 … sommet +5),
/// donc jamais sur la même échelle que le Spectre ni d'une ligne à l'autre. Ils prennent l'échelle de PIDtoolbox.
#[test]
fn l_echelle_du_spectre_simule_est_celle_de_pidtoolbox() {
    use otb_gui::tools::common::PTB_DB_RANGE;
    use otb_gui::tools::filter_sim::y_bounds;
    use otb_gui::tools::spectral::SpectralTool;
    assert_eq!(y_bounds(), (-50.0, 20.0));
    assert_eq!(y_bounds(), PTB_DB_RANGE);
    // le Spectre part de la même constante : les deux pages se lisent l'une contre l'autre
    let spectre = SpectralTool::new();
    assert_eq!(spectre.y_range(), PTB_DB_RANGE);
}

#[test]
fn un_identifiant_de_chargement_n_est_jamais_reutilise() {
    // l'adresse mémoire d'un log fermé peut servir à un autre (L-072) : l'identifiant, non
    let mut st = etat();
    let premier = st.logs[0].uid;
    let log = st.logs[0].log.as_ref().clone();
    st.remove(0);
    assert!(st.add(log.clone()));
    let second = st.logs[0].uid;
    assert_ne!(premier, second);
    assert!(st.add(log));
    let uids: Vec<u64> = st.inputs(&[0, 1]).iter().map(|l| l.uid).collect();
    assert_eq!(uids, vec![second, st.logs[1].uid]);
    assert_ne!(uids[0], uids[1]);
}

#[test]
fn changer_de_log_recharge_l_editeur_et_oublie_les_reglages_appliques() {
    let mut h = banc_page(CLAIR, SIM);
    couper_curseur_gyro(&mut h);
    saisir(&mut h, "Gyro LPF2 coupure", "250");
    h.cliquer_label("Appliquer");
    h.run_steps(2);
    attendre(&mut h);
    assert!(cli_text(&h).contains("set gyro_lpf2_static_hz = 250"));
    // on ferme le log puis on rouvre le même fichier : un nouveau chargement, donc un éditeur neuf
    let Screen::Tool(page) = h.state().screen else {
        panic!("page de l'outil")
    };
    // la page de l'outil n'est pas dessinée pendant ce temps (on est dans « Courbes ») : rien ne la remet à zéro
    h.state_mut().screen = Screen::Viewer;
    h.run_steps(2);
    h.state_mut().state.remove(0);
    h.run_steps(2);
    let log = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    h.state_mut().open(vec![log]);
    let debut = Instant::now();
    while h.state().state.logs.is_empty() {
        h.run();
        assert!(debut.elapsed() < Duration::from_secs(30), "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.state_mut().screen = Screen::Tool(page);
    attendre(&mut h);
    assert!(cli_text(&h).contains("aucun changement"), "{}", cli_text(&h));
    assert!(h.query_by_label(STALE_STATUS).is_none());
}

#[test]
fn les_bode_laissent_une_marge_sous_la_borne_basse() {
    use otb_gui::tools::filter_sim::bode_y_bounds;
    // borne basse nominale : -40 dB pour le gain (sa graduation « -40 » est étiquetée : vue sur capture), -270 ° pour la phase
    // (sa dernière graduation étiquetée n'est pas vérifiée : la marge y est seulement cohérente). Le test ne prouve que les
    // bornes ; l'absence de chevauchement des étiquettes se juge sur les captures.
    for (col, basse, haut) in [(0, -40.0, 3.0), (1, -270.0, 10.0)] {
        let (lo, hi) = bode_y_bounds(col);
        assert_eq!(hi, haut, "colonne {col} : borne haute inchangée");
        assert!(
            basse - lo >= 0.05 * (hi - lo),
            "colonne {col} : marge {} pour une hauteur de {}",
            basse - lo,
            hi - lo
        );
        assert!(basse - lo <= 0.1 * (hi - lo), "colonne {col} : marge raisonnable");
    }
}

#[test]
fn un_groupe_se_replie_et_se_deplie_d_un_clic_sur_son_titre() {
    let mut h = banc_config(CLAIR, true);
    h.run();
    assert!(h.query_by_label("Firmware").is_some() && h.query_by_label("Carte").is_some());
    h.cliquer_label("Général");
    h.run_steps(3);
    assert!(
        h.query_by_label("Firmware").is_none(),
        "les lignes du groupe replié disparaissent"
    );
    assert!(h.query_by_label("Carte").is_none());
    assert!(h.query_by_label("Général").is_some(), "son titre reste");
    assert!(
        h.query_by_label("Nom du drone").is_none(),
        "il fait partie de « Général »"
    );
    assert!(
        h.query_by_label("Roll P, I, D, Dmax, F").is_some(),
        "les autres groupes restent dépliés"
    );
    h.cliquer_label("Général");
    h.run_steps(3);
    assert!(h.query_by_label("Firmware").is_some(), "dépliable de nouveau");
}

#[test]
fn les_lignes_montrees_d_un_groupe_replie_sont_vides() {
    use std::collections::HashSet;
    let g = Group {
        title: "Général".into(),
        rows: vec![Row {
            key: "a".into(),
            values: vec!["1".into()],
        }],
    };
    let mut repliés: HashSet<String> = HashSet::new();
    assert_eq!(shown_rows(&g, &repliés).len(), 1);
    toggle_group(&mut repliés, "Général");
    assert!(shown_rows(&g, &repliés).is_empty());
    toggle_group(&mut repliés, "Autre");
    assert!(
        shown_rows(&g, &repliés).is_empty(),
        "un autre groupe ne déplie pas celui-ci"
    );
    toggle_group(&mut repliés, "Général");
    assert_eq!(shown_rows(&g, &repliés).len(), 1);
}

#[test]
fn la_configuration_n_a_pas_de_bouton_calculer_mais_a_l_export() {
    let mut h = banc_config(CLAIR, true);
    h.run();
    assert!(
        h.query_by_label("Calculer").is_none(),
        "elle se met à jour sans calcul, comme en Python"
    );
    h.get_by_label("Exporter image");
    assert!(h.query_by_label("Tout voir").is_none(), "pas de graphique à zoomer");
}

// ------------------------------------------------------------------ Simulateur : résultat d'un autre log (W-036 (8))

#[test]
fn le_resultat_du_simulateur_porte_l_identifiant_de_son_log() {
    let st = etat();
    let out = FilterSimTool::compute(
        SimOptions {
            applied: None,
            with_current: true,
        },
        st.inputs(&[0]),
    )
    .unwrap();
    assert_eq!(out.log_id, st.logs[0].uid);
}

#[test]
fn un_resultat_d_un_autre_log_n_est_pas_affiche() {
    assert!(!result_is_for_other_log(7, Some(7)), "même log : affiché");
    assert!(
        result_is_for_other_log(7, Some(8)),
        "autre log : masqué pendant le calcul"
    );
    assert!(
        !result_is_for_other_log(7, None),
        "pas de log choisi : rien à comparer, la page gère l'absence de log"
    );
}

// ---------------------------------------------------------------- curseurs de filtres (D-080)

#[test]
fn les_curseurs_de_filtres_recalculent_les_frequences_comme_le_firmware() {
    let base = log5s().settings;
    let mut e = Editor::from_settings(&base);
    assert!(
        e.g_slider && e.d_slider,
        "le log a ses deux curseurs mis, aux fréquences qu'ils donnent"
    );
    assert_eq!((e.g_mult, e.d_mult), (150, 140));
    let g1 = e.g1;
    e.g_mult = 120;
    e.d_mult = 80;
    e.follow_sliders();
    // gyro : LPF1 désactivé, il le reste ; LPF2 500 × 1,2 (`config/simplified_tuning.c`)
    assert_eq!(e.g1, g1);
    assert_eq!(e.g2_hz, 600);
    // D dynamique : 75 et 150 × 0,8 ; LPF2 150 × 0,8
    assert_eq!((e.d1.min_hz, e.d1.max_hz, e.d2_hz), (60, 120, 120));
    // gyro statique : 250 × 1,2 ; dynamique : 250 et 500 × 1,2, la coupure statique vide le reste
    e.g1.mode = 1;
    e.g1.static_hz = 0;
    e.follow_sliders();
    assert_eq!(e.g1.static_hz, 300);
    e.g1.mode = 2;
    e.g1.static_hz = 0;
    e.follow_sliders();
    assert_eq!((e.g1.min_hz, e.g1.max_hz, e.g1.static_hz), (300, 600, 0));
    // arithmétique entière du firmware : 75 × 0,55 = 41,25 → 41
    e.d_mult = 55;
    e.follow_sliders();
    assert_eq!(e.d1.min_hz, 41);
    // bornes du firmware : 1000 Hz au plus
    e.g_mult = 200;
    e.follow_sliders();
    assert_eq!((e.g1.max_hz, e.g2_hz), (1000, 1000));
    // un LPF2 désactivé le reste
    e.g2_hz = 0;
    e.follow_sliders();
    assert_eq!(e.g2_hz, 0);
    // interrupteur coupé : plus rien ne bouge
    e.g_slider = false;
    e.g_mult = 50;
    e.follow_sliders();
    assert_eq!(e.g1.max_hz, 1000);
}

#[test]
fn l_interrupteur_suit_le_log_seulement_si_ses_frequences_sont_celles_du_curseur() {
    let base = log5s().settings;
    // une coupure changée après coup par `set` : le configurateur coupe le curseur, la position reste connue
    let mut apres_coup = base.clone();
    apres_coup.gyro_lpf2.static_hz = 700;
    let e = Editor::from_settings(&apres_coup);
    assert!(!e.g_slider && e.d_slider);
    assert_eq!(e.g_mult, 150);
    let mut coupe = base.clone();
    coupe.raw.insert("simplified_dterm_filter".into(), "0".into());
    assert!(!Editor::from_settings(&coupe).d_slider);
    // log sans curseurs (avant 4.3) : interrupteurs coupés, position par défaut
    let e0 = Editor::from_settings(&Settings::default());
    assert!(!e0.g_slider && !e0.d_slider);
    assert_eq!((e0.g_mult, e0.d_mult), (100, 100));
}

#[test]
fn le_cli_ecrit_le_curseur_mis_et_les_frequences_qu_il_donne() {
    let base = log5s().settings;
    let cli = |b: &Settings, e: &Editor| fsc::filter_cli_lines_with(b, &e.modified_settings(b), e.filter_sliders());
    let has = |l: &[String], w: &str| l.iter().any(|x| x == w);
    let e0 = Editor::from_settings(&base);
    assert!(cli(&base, &e0).last().unwrap().starts_with("# aucun changement"));
    // curseur déplacé : son état, puis les fréquences (le CLI ne les recalcule pas sur un `set`)
    let mut e = e0.clone();
    e.g_mult = 120;
    e.follow_sliders();
    let l = cli(&base, &e);
    for w in [
        "set simplified_gyro_filter = ON",
        "set simplified_gyro_filter_multiplier = 120",
        "set gyro_lpf2_static_hz = 600",
        "save",
    ] {
        assert!(has(&l, w), "{w} absent : {l:#?}");
    }
    assert!(
        !l.iter().any(|x| x.contains("OFF") || x.contains("simplified_dterm")),
        "{l:#?}"
    );
    // type changé sous le curseur mis : le curseur reste mis (avant D-080, le passe-bas changé le coupait)
    let mut t = e0.clone();
    t.g2_type = 1;
    let l = cli(&base, &t);
    assert!(has(&l, "set gyro_lpf2_type = BIQUAD"), "{l:#?}");
    assert!(!l.iter().any(|x| x.contains("simplified_gyro_filter")), "{l:#?}");
    // interrupteur coupé puis coupure saisie : le curseur du log est coupé
    let mut m = e0.clone();
    m.g_slider = false;
    m.g2_hz = 250;
    assert!(has(&cli(&base, &m), "set simplified_gyro_filter = OFF"));
    // curseur mis sur un log qui l'avait coupé, fréquences déjà les siennes : seul son état s'écrit
    let mut coupe = base.clone();
    coupe.raw.insert("simplified_gyro_filter".into(), "0".into());
    let mut e = Editor::from_settings(&coupe);
    assert!(!e.g_slider);
    e.g_slider = true;
    e.follow_sliders();
    let l = cli(&coupe, &e);
    assert!(has(&l, "set simplified_gyro_filter = ON") && has(&l, "save"), "{l:#?}");
    assert!(!l.iter().any(|x| x.starts_with("set gyro_lpf")), "{l:#?}");
}

#[test]
fn le_curseur_de_filtre_grise_les_frequences_et_les_recalcule_a_l_ecran() {
    let mut h = banc_page(CLAIR, SIM);
    let champ = |h: &Harness<'static, App>, nom: &str| h.get_by_label(nom).accesskit_node().is_disabled();
    assert!(champ(&h, "Gyro LPF2 coupure"), "curseur du log mis : coupure calculée");
    assert!(!champ(&h, "Gyro LPF2 type"), "le type reste au pilote");
    h.get_by_label("Position du multiplicateur du filtre gyro")
        .set_value_accesskit(120.0);
    h.run_steps(4);
    let cli = cli_text(&h);
    assert!(cli.contains("set simplified_gyro_filter_multiplier = 120"), "{cli}");
    assert!(cli.contains("set gyro_lpf2_static_hz = 600"), "{cli}");
    h.get_by_label(STALE_STATUS);
    assert!(h.query_by_label(MULT_WARNING).is_none());
    // aux extrémités, l'avertissement du configurateur
    h.get_by_label("Position du multiplicateur du filtre gyro")
        .set_value_accesskit(200.0);
    h.run_steps(4);
    h.get_by_label(MULT_WARNING);
    // le terme D a le même curseur
    h.cliquer_role(egui::accesskit::Role::Button, "Terme D");
    h.run_steps(3);
    assert!(champ(&h, "D LPF2 coupure"));
    assert!(champ(&h, "D LPF1 dynamique min"));
    h.cliquer_label("Multiplicateur du filtre D");
    h.run_steps(3);
    assert!(!champ(&h, "D LPF2 coupure"), "interrupteur coupé : saisie libre");
}
