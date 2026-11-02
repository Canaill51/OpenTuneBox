//! Guide intégré (6-5), port de `gui/guide.py` et de `tests/test_gui_guide.py` : fichier des fiches, liens, avancement, outils cités,
//! puis la page. Un seul fichier de test pour toute la sous-étape (L-069).

use eframe::Storage as _;
mod support;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use otb_advisor::drone_card::{BF_FAMILIES, DroneCard, ESCS, PID_RATES_KHZ};
use otb_advisor::log_check::{FILTERS, PID};
use otb_advisor::profiles::PROFILES;
use otb_advisor::tuning_path::phases;
use otb_gui::app::{App, Screen};
use otb_gui::guide::*;
use otb_gui::guide_page::GuidePage;
use otb_gui::tools::all_pages;
use serde_json::Value;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// Un dossier à soi, jamais le vrai `drones.json`.
fn dossier(nom: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!(
        "otb-guide-{}-{}-{nom}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn cas() -> Value {
    serde_json::from_str(include_str!("data/guide_cards.json")).unwrap()
}

// ------------------------------------------------------------------ fichier des fiches
#[test]
fn le_fichier_est_reecrit_octet_pour_octet_comme_python() {
    let c = cas();
    let list = c["roundtrip"].as_array().unwrap();
    assert_eq!(list.len(), 27);
    for w in list {
        let (input, want) = (w["input"].as_str().unwrap(), w["output"].as_str().unwrap());
        let cards = cards_from_json(input).unwrap_or_else(|e| panic!("{e} : {input}"));
        assert_eq!(cards.len() as u64, w["n"].as_u64().unwrap(), "{input}");
        assert_eq!(cards_to_json(&cards), want, "entrée : {input}");
    }
}

#[test]
fn un_fichier_que_python_ne_lit_pas_est_signale() {
    let c = cas();
    let list = c["unreadable"].as_array().unwrap();
    assert_eq!(list.len(), 7);
    let d = dossier("illisible-signale");
    for t in list {
        let p = d.join("drones.json");
        std::fs::write(&p, t.as_str().unwrap()).unwrap();
        let cards = Cards::load(p);
        assert!(cards.list.is_empty() && cards.problem().is_some(), "{t}");
    }
}

#[test]
fn un_fichier_absent_n_est_pas_un_probleme() {
    let c = Cards::load(dossier("absent").join("drones.json"));
    assert!(c.list.is_empty() && c.problem().is_none());
}

#[test]
fn un_fichier_illisible_est_mis_de_cote_avant_d_ecrire() {
    let d = dossier("mis-de-cote");
    let p = d.join("drones.json");
    std::fs::write(&p, "[{\"bf\": \"4.4\"}] précieux").unwrap();
    let mut c = Cards::load(p.clone());
    assert!(c.problem().is_some());
    c.add(DroneCard::new("A")).unwrap();
    assert!(c.problem().is_none());
    assert_eq!(
        std::fs::read_to_string(d.join("drones.json.illisible")).unwrap(),
        "[{\"bf\": \"4.4\"}] précieux"
    );
    assert_eq!(Cards::load(p.clone()).list.len(), 1);
    // un second fichier abîmé n'écrase pas le premier
    std::fs::write(&p, "pas du json").unwrap();
    let mut c = Cards::load(p);
    c.add(DroneCard::new("B")).unwrap();
    assert_eq!(
        std::fs::read_to_string(d.join("drones.json.illisible")).unwrap(),
        "[{\"bf\": \"4.4\"}] précieux"
    );
    assert_eq!(
        std::fs::read_to_string(d.join("drones.json.illisible.1")).unwrap(),
        "pas du json"
    );
    // et aucun fichier temporaire ne reste
    let mut noms: Vec<String> = std::fs::read_dir(&d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    noms.sort();
    assert_eq!(
        noms,
        ["drones.json", "drones.json.illisible", "drones.json.illisible.1"]
    );
}

#[test]
fn un_fichier_lisible_n_est_jamais_mis_de_cote() {
    let d = dossier("lisible");
    let mut c = Cards::load(d.join("drones.json"));
    c.add(DroneCard::new("A")).unwrap();
    c.add(DroneCard::new("B")).unwrap();
    assert!(!d.join("drones.json.illisible").exists());
}

#[test]
fn les_operations_sur_les_fiches_sont_celles_de_python() {
    let d = dossier("operations");
    let p = d.join("sous").join("dossier").join("drones.json");
    let mut c = Cards::load(p.clone());
    let a = c.add(DroneCard::new("A")).unwrap();
    let b = c.add(DroneCard::new("B")).unwrap();
    assert_eq!((a, b), (0, 1));
    // une fiche de même nom en remplace une autre et passe à la fin
    let mut a2 = DroneCard::new("A");
    a2.bf = "4.4".into();
    assert_eq!(c.add(a2).unwrap(), 1);
    assert_eq!(c.list.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["B", "A"]);
    // cocher puis décocher : la clé disparaît, elle n'est pas écrite à false
    c.set_done(1, "recording", true).unwrap();
    c.set_done(1, "rpm", true).unwrap();
    c.set_done(1, "recording", true).unwrap();
    assert_eq!(
        c.list[1].done,
        [("recording".to_string(), true), ("rpm".to_string(), true)]
    );
    c.set_done(1, "recording", false).unwrap();
    assert_eq!(c.list[1].done, [("rpm".to_string(), true)]);
    assert!(!std::fs::read_to_string(&p).unwrap().contains("recording"));
    // le fichier relu est identique à la mémoire
    assert_eq!(Cards::load(p.clone()).list, c.list);
    c.reset(1).unwrap();
    assert!(c.list[1].done.is_empty());
    c.delete(0).unwrap();
    assert_eq!(
        Cards::load(p).list.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
        ["A"]
    );
    // un numéro hors liste ne fait rien (et ne plante pas)
    c.set_done(9, "x", true).unwrap();
    c.delete(9).unwrap();
    c.reset(9).unwrap();
    assert_eq!(c.list.len(), 1);
}

#[test]
fn la_fiche_modifiee_garde_son_avancement_et_sa_place() {
    let d = dossier("remplacer");
    let mut c = Cards::load(d.join("drones.json"));
    c.add(DroneCard::new("A")).unwrap();
    c.add(DroneCard::new("B")).unwrap();
    c.set_done(0, "recording", true).unwrap();
    let mut neuve = DroneCard::new("A2");
    neuve.done = c.list[0].done.clone();
    c.replace(0, neuve).unwrap();
    let relu = Cards::load(d.join("drones.json")).list;
    assert_eq!((relu[0].name.as_str(), relu[1].name.as_str()), ("A2", "B"));
    assert_eq!(relu[0].done, [("recording".to_string(), true)]);
}

// ------------------------------------------------------------------ emplacement du fichier
fn chemin(os: &str, env: &[(&str, &str)]) -> String {
    let var = |k: &str| env.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v));
    cards_path_in(os, &var).to_string_lossy().replace('\\', "/")
}

#[test]
fn le_chemin_est_celui_de_qt_sous_linux_et_sur_ce_poste() {
    // mesuré : ~/.local/share/OpenTuneBox/OpenTuneBox/drones.json
    assert_eq!(
        chemin("linux", &[("HOME", "/home/u")]),
        "/home/u/.local/share/OpenTuneBox/OpenTuneBox/drones.json"
    );
    assert_eq!(
        chemin("linux", &[("HOME", "/home/u"), ("XDG_DATA_HOME", "/data")]),
        "/data/OpenTuneBox/OpenTuneBox/drones.json"
    );
    // XDG_DATA_HOME relatif ou vide : ignoré
    for x in ["donnees", ""] {
        assert_eq!(
            chemin("linux", &[("HOME", "/home/u"), ("XDG_DATA_HOME", x)]),
            "/home/u/.local/share/OpenTuneBox/OpenTuneBox/drones.json"
        );
    }
}

#[test]
fn le_chemin_suit_la_documentation_de_qt_sous_windows_et_macos() {
    assert_eq!(
        chemin(
            "windows",
            &[
                ("APPDATA", "C:\\Users\\u\\AppData\\Roaming"),
                ("USERPROFILE", "C:\\Users\\u")
            ]
        ),
        "C:/Users/u/AppData/Roaming/OpenTuneBox/OpenTuneBox/drones.json"
    );
    assert_eq!(
        chemin("macos", &[("HOME", "/Users/u")]),
        "/Users/u/Library/Application Support/OpenTuneBox/OpenTuneBox/drones.json"
    );
}

#[test]
fn sans_dossier_de_donnees_le_repli_est_celui_de_python() {
    assert_eq!(
        chemin("windows", &[("USERPROFILE", "C:\\Users\\u")]),
        "C:/Users/u/.opentunebox/drones.json"
    );
    assert_eq!(chemin("linux", &[]), ".opentunebox/drones.json");
}

// ------------------------------------------------------------------ liens
#[test]
fn les_liens_internes_sont_lus() {
    assert_eq!(parse_link("otb:phase/pid"), Some(GuideLink::Phase("pid")));
    assert_eq!(parse_link("otb:phase/home"), Some(GuideLink::Phase(HOME)));
    assert_eq!(
        parse_link("otb:tool/step"),
        Some(GuideLink::Tool(ToolTarget::Page("Réponse indicielle")))
    );
    assert_eq!(parse_link("otb:tool/viewer"), Some(GuideLink::Tool(ToolTarget::Viewer)));
    assert_eq!(parse_link("otb:open/pid"), Some(GuideLink::Open(PID)));
    assert_eq!(parse_link("otb:open/filters"), Some(GuideLink::Open(FILTERS)));
    assert_eq!(parse_link("otb:role/0/pid"), Some(GuideLink::Role(0, PID)));
    assert_eq!(parse_link("otb:role/12/filters"), Some(GuideLink::Role(12, FILTERS)));
}

#[test]
fn les_liens_invalides_ou_externes_ne_sont_pas_internes() {
    for l in [
        "https://exemple.org",
        "otb:",
        "otb:phase",
        "otb:phase/inconnue",
        "otb:tool/inconnu",
        "otb:open/rien",
        "otb:role/pid",
        "otb:role/x/pid",
        "otb:role/-1/pid",
        "otb:role/+1/pid",
        "otb:role//pid",
        "otb:role/1/rien",
        "otb:role/99999999999999999999/pid",
        "otb:autre/pid",
        "phase/pid",
    ] {
        assert_eq!(parse_link(l), None, "{l}");
    }
}

#[test]
fn chaque_lien_de_l_accueil_est_valide_et_la_source_est_unique() {
    let mut n = 0;
    for part in HOME_TEXT.split("](").skip(1) {
        let dest = part.split(')').next().unwrap();
        if dest.starts_with("otb:") {
            assert!(parse_link(dest).is_some(), "lien cassé : {dest}");
            n += 1;
        }
    }
    assert!(n >= 5, "l'accueil devrait contenir au moins 5 liens internes ({n})");
    // même fichier que Python
    let python = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../src/opentunebox/gui/assets/guide/accueil.md"
    ))
    .unwrap();
    assert_eq!(HOME_TEXT, python);
}

// ------------------------------------------------------------------ outils cités par les cases
#[test]
fn chaque_outil_cite_par_une_case_existe_dans_l_interface() {
    let titres: Vec<&str> = all_pages().iter().map(|p| p.title()).collect();
    for (cle, _, cible) in TOOLS {
        if let ToolTarget::Page(t) = cible {
            assert!(
                titres.contains(&t),
                "la clé « {cle} » mène à « {t} », qui n'est pas une page : {titres:?}"
            );
        }
    }
    let (mut vus, mut parts) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    for (_, bf) in BF_FAMILIES.iter().map(|(v, l)| (l, v)) {
        for p in &PROFILES {
            for (khz, _) in PID_RATES_KHZ {
                for (esc, _) in ESCS {
                    let mut card = DroneCard::new("A");
                    (card.bf, card.size, card.pid_khz, card.esc) =
                        (bf.to_string(), p.key.to_string(), khz, esc.to_string());
                    for ph in phases(&card) {
                        for s in &ph.steps {
                            if let Some(t) = s.tool {
                                assert!(tool_target(t).is_some(), "outil inconnu : {t} (case {})", s.key);
                                vus.insert(t);
                            }
                            if let Some(part) = s.part {
                                assert!(PART_NAMES.iter().any(|(k, _)| *k == part), "partie inconnue : {part}");
                                parts.insert(part);
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(vus.len() >= 3 && !parts.is_empty(), "{vus:?} {parts:?}");
}

#[test]
fn les_libelles_des_boutons_sont_ceux_de_python() {
    assert_eq!(
        tool_label("freq_throttle", Some("hover")),
        "Fréq. × gaz (partie stationnaire)"
    );
    assert_eq!(
        tool_label("spectral", Some("moving")),
        "Spectre (partie avec mouvements)"
    );
    assert_eq!(tool_label("step", None), "Réponse indicielle");
    assert_eq!(tool_label("inconnu", None), "inconnu");
    assert_eq!(open_logs_label(FILTERS), "Ouvrir les logs des filtres…");
    assert_eq!(open_logs_label(PID), "Ouvrir les logs de la série…");
    assert_eq!((role_name(FILTERS), role_name(PID)), ("filtres", "séries PID"));
    assert_eq!((other_role(FILTERS), other_role(PID)), (PID, FILTERS));
}

// ------------------------------------------------------------------ avancement
fn prep_cles(card: &DroneCard) -> Vec<String> {
    phases(card)[0].steps.iter().map(|s| s.key.clone()).collect()
}

#[test]
fn l_avancement_compte_les_cases_cochees_a_la_main() {
    let mut card = DroneCard::new("A");
    card.size = "seven".into();
    card.pid_khz = 3.2;
    let prep = |c: &DroneCard| progress(c, &[], None).into_iter().find(|p| p.0 == "prep").unwrap();
    let (_, n0, total) = prep(&card);
    assert_eq!((n0, total), (0, 11));
    card.done.push(("recording".into(), true));
    let autre = prep_cles(&card)[1].clone();
    card.done.push((autre, false)); // décochée dans le fichier : ne compte pas
    assert_eq!(prep(&card).1, 1);
    let phases_noms: Vec<String> = progress(&card, &[], None).into_iter().map(|p| p.0).collect();
    assert_eq!(phases_noms, PHASE_KEYS);
}

// ------------------------------------------------------------------ la page
fn banc(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1300.0, 800.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app)
}

fn app_sur(path: &std::path::Path) -> App {
    App::with_guide(all_pages(), GuidePage::new(Cards::load(path.to_path_buf())))
}

/// Un drone de 7", boucle PID 3,2 kHz (la fiche du test de Python).
fn sept_pouces() -> DroneCard {
    let mut c = DroneCard::new("Sept");
    c.size = "seven".into();
    c.pid_khz = 3.2;
    c
}

fn donnees() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")
}

fn attendre(h: &mut Harness<'static, App>, fin: impl Fn(&App) -> bool) {
    let debut = std::time::Instant::now();
    while !fin(h.state()) {
        h.run_steps(2);
        assert!(
            debut.elapsed() < std::time::Duration::from_secs(60),
            "attente trop longue"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.run_steps(3);
}

#[test]
fn l_accueil_est_affiche_et_une_phase_sans_fiche_invite_a_en_creer_une() {
    let d = dossier("accueil");
    let mut h = banc(app_sur(&d.join("drones.json")));
    h.run();
    h.get_by_label("Régler son drone avec OpenTuneBox");
    assert!(h.state().guide.at_home());
    h.state_mut().guide.phase = "prep";
    h.run();
    h.get_by_label_contains("Créez d'abord la fiche de votre drone");
    assert!(
        h.query_by_role_and_label(Role::CheckBox, "Thème sombre").is_some(),
        "seule la case du thème reste"
    );
    assert_eq!(
        h.query_all_by_role(Role::CheckBox).count(),
        1,
        "aucune case du parcours sans fiche"
    );
}

#[test]
fn les_cases_se_cochent_s_enregistrent_et_survivent_a_un_rechargement() {
    let d = dossier("cases");
    let p = d.join("drones.json");
    let mut c = Cards::load(p.clone());
    c.add(sept_pouces()).unwrap();
    let mut h = banc(app_sur(&p));
    h.state_mut().guide.phase = "prep";
    h.run_steps(3);
    let prep = &phases(&sept_pouces())[0];
    assert_eq!(prep.steps.len(), 11);
    for step in &prep.steps {
        h.get_by_role_and_label(Role::CheckBox, &step.title); // une case par étape
    }
    assert_eq!(
        h.query_all_by_role(Role::CheckBox).count(),
        11 + 1,
        "11 cases et celle du thème"
    );
    assert_eq!(
        h.query_all_by_label("1. Préparer le drone (0/11)").count(),
        2,
        "frise et liste"
    );
    // cocher la case « recording » : enregistrée dans le fichier, comptée dans la phase
    let titre = prep.steps.iter().find(|s| s.key == "recording").unwrap().title.clone();
    h.get_by_label(&titre).click();
    h.run_steps(3);
    assert_eq!(
        h.query_all_by_label("1. Préparer le drone (1/11)").count(),
        2,
        "frise et liste"
    );
    assert_eq!(Cards::load(p.clone()).list[0].done, [("recording".to_string(), true)]);
    // décocher : la clé disparaît du fichier
    h.get_by_label(&titre).click();
    h.run_steps(3);
    assert_eq!(
        h.query_all_by_label("1. Préparer le drone (0/11)").count(),
        2,
        "frise et liste"
    );
    assert!(Cards::load(p.clone()).list[0].done.is_empty());
    h.get_by_label(&titre).click();
    h.run_steps(3);
    // un nouveau lancement retrouve l'avancement
    let mut h2 = banc(app_sur(&p));
    h2.state_mut().guide.phase = "prep";
    h2.run_steps(3);
    assert_eq!(
        h2.query_all_by_label("1. Préparer le drone (1/11)").count(),
        2,
        "frise et liste"
    );
}

#[test]
fn le_texte_de_la_case_suit_la_fiche_et_le_bouton_point_d_interrogation_affiche_l_aide() {
    let d = dossier("aide");
    let mut app = app_sur(&d.join("drones.json"));
    app.guide.cards.add(sept_pouces()).unwrap();
    app.guide.select_by_name("Sept");
    let mut h = banc(app);
    h.state_mut().guide.phase = "prep";
    h.run_steps(3);
    let prep = &phases(&sept_pouces())[0];
    let titre = prep.steps.iter().find(|s| s.key == "recording").unwrap().title.clone();
    assert!(
        titre.contains("1,6 kHz"),
        "la fiche 7\" / 3,2 kHz donne 1,6 kHz : {titre}"
    );
    h.get_by_label(&titre);
    assert_eq!(
        h.query_all_by_role_and_label(Role::Button, "?").count(),
        11,
        "un « ? » par case"
    );
    // l'aide d'une case est repliée au départ, puis affichée par son « ? » (plus de textes à l'écran)
    let avant = h.query_all_by_role(Role::Label).count();
    h.get_all_by_role_and_label(Role::Button, "?").next().unwrap().click();
    h.run_steps(3);
    let apres = h.query_all_by_role(Role::Label).count();
    assert!(
        apres > avant,
        "l'aide de « {} » n'apparaît pas ({avant} → {apres} textes)",
        prep.steps[0].key
    );
    h.get_all_by_role_and_label(Role::Button, "?").next().unwrap().click();
    h.run_steps(3);
    assert_eq!(
        h.query_all_by_role(Role::Label).count(),
        avant,
        "un second clic replie l'aide"
    );
}

#[test]
fn un_lien_de_l_accueil_mene_a_la_phase() {
    let d = dossier("lien");
    let mut app = app_sur(&d.join("drones.json"));
    app.guide.cards.add(sept_pouces()).unwrap();
    app.guide.select_by_name("Sept");
    let mut h = banc(app);
    h.run_steps(3);
    assert!(h.state().guide.at_home());
    // le lien du tableau de l'accueil, pas le bouton de la liste de gauche
    let liens: Vec<_> = h.query_all_by_role(Role::Link).collect();
    let pid = liens
        .iter()
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.contains("PID par séries")))
        .expect("lien « PID par séries »");
    pid.click();
    h.run_steps(3);
    assert_eq!(h.state().guide.phase, "pid");
    // la phase PID : Damping 1.0 en 3e position et le bouton d'ouverture des logs de série
    let cases = &phases(&sept_pouces())[2].steps;
    assert_eq!(cases[2].key, "Damping_1.0");
    h.get_by_role_and_label(Role::Button, "Ouvrir les logs de la série…");
}

#[test]
fn creer_modifier_recommencer_et_supprimer_une_fiche() {
    let d = dossier("fiches");
    let p = d.join("drones.json");
    let mut h = banc(app_sur(&p));
    h.run_steps(3);
    // nouvelle fiche : le nom saisi, les choix de la fenêtre
    h.get_by_role_and_label(Role::Button, "Nouveau drone").click();
    h.run_steps(3);
    // le champ est prérempli (« Mon drone ») : on remplace le texte comme le ferait un lecteur d'écran
    h.get_by_label("Nom (comme dans Betaflight)")
        .set_text_value_accesskit("Mon Cinq");
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Enregistrer").click();
    h.run_steps(4);
    let relu = Cards::load(p.clone()).list;
    assert_eq!(relu.len(), 1);
    assert_eq!(relu[0].name, "Mon Cinq");
    assert_eq!(
        (relu[0].bf.as_str(), relu[0].size.as_str(), relu[0].esc.as_str()),
        ("4.5", "five", "BLHeli_32")
    );
    assert!(
        !relu[0].pid_khz_int,
        "la fenêtre de saisie écrit un flottant, comme Python"
    );
    assert_eq!(h.state().guide.card().unwrap().name, "Mon Cinq");
    // un nom vide devient « Mon drone »
    h.get_by_role_and_label(Role::Button, "Nouveau drone").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Annuler").click();
    h.run_steps(3);
    assert_eq!(Cards::load(p.clone()).list.len(), 1, "annuler n'enregistre rien");
    // cocher une case, puis Recommencer (confirmation « Non » puis « Oui »)
    h.state_mut().guide.phase = "prep";
    h.run_steps(3);
    let titre = phases(&relu[0])[0].steps[0].title.clone();
    h.get_by_label(&titre).click();
    h.run_steps(3);
    assert_eq!(Cards::load(p.clone()).list[0].done.len(), 1);
    h.get_by_role_and_label(Role::Button, "Recommencer").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Non").click();
    h.run_steps(3);
    assert_eq!(Cards::load(p.clone()).list[0].done.len(), 1, "« Non » ne change rien");
    h.get_by_role_and_label(Role::Button, "Recommencer").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Oui").click();
    h.run_steps(3);
    assert!(Cards::load(p.clone()).list[0].done.is_empty());
    // Supprimer
    h.get_by_role_and_label(Role::Button, "Supprimer").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Oui").click();
    h.run_steps(3);
    assert!(Cards::load(p).list.is_empty());
    assert!(h.state().guide.card().is_none());
}

#[test]
fn un_fichier_de_fiches_illisible_est_annonce_puis_mis_de_cote() {
    let d = dossier("abime");
    let p = d.join("drones.json");
    std::fs::write(&p, "[{\"bf\": \"4.4\"}] précieux").unwrap();
    let mut h = banc(app_sur(&p));
    h.run_steps(3);
    h.get_by_label_contains("est illisible");
    assert!(
        std::fs::read_to_string(&p).unwrap().contains("précieux"),
        "rien n'est écrit tant qu'on n'enregistre pas"
    );
    h.get_by_role_and_label(Role::Button, "Nouveau drone").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Enregistrer").click();
    h.run_steps(4);
    assert_eq!(
        std::fs::read_to_string(d.join("drones.json.illisible")).unwrap(),
        "[{\"bf\": \"4.4\"}] précieux"
    );
    assert!(h.query_by_label_contains("est illisible").is_none());
}

struct Memoire(std::collections::HashMap<String, String>);
impl eframe::Storage for Memoire {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }
    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }
    fn flush(&mut self) {}
}

#[test]
fn la_phase_la_fiche_et_les_roles_sont_retrouves_au_lancement_suivant() {
    let mut a = GuidePage::in_memory();
    a.cards.list.push(DroneCard::new("A"));
    a.cards.list.push(DroneCard::new("B"));
    a.select_by_name("B");
    a.phase = "filters";
    a.roles.insert("/x/vol1.bbl".into(), PID.into());
    let mut m = Memoire(Default::default());
    a.save(&mut m);
    let mut b = GuidePage::in_memory();
    b.cards.list.push(DroneCard::new("A"));
    b.cards.list.push(DroneCard::new("B"));
    b.restore(&m);
    assert_eq!((b.phase, b.card().unwrap().name.as_str()), ("filters", "B"));
    assert_eq!(b.roles.get("/x/vol1.bbl").map(String::as_str), Some(PID));
    // une phase ou un rôle inconnus (fichier d'une autre version) sont ignorés
    let mut m2 = Memoire(Default::default());
    m2.set_string("guide.phase", "\"inconnue\"".into());
    m2.set_string("guide.roles", "{\"/y.bbl\": \"nimporte\"}".into());
    let mut c = GuidePage::in_memory();
    c.restore(&m2);
    assert!(c.at_home() && c.roles.is_empty());
    // une fiche disparue : on retombe sur la première
    let mut d = GuidePage::in_memory();
    d.cards.list.push(DroneCard::new("A"));
    d.restore(&m);
    assert_eq!(d.card().unwrap().name, "A");
}

#[test]
fn les_signes_du_guide_s_affichent_sans_police_manquante() {
    // « → ▲ ● ○ ↑ ↓ ← » viennent de Hack (police de secours) ; le banc refuse toute police manquante
    let mut h = Harness::builder()
        .with_size(egui::vec2(400.0, 100.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui(|ui| {
            ui.label("→ ▲ ● ○ ↑ ↓ ← ◇ ■ « ≥ ≤ ° · ± × » – — … • ≈ ÷ Δ Ω µ");
        });
    h.run();
}

#[test]
fn le_texte_du_guide_n_a_plus_de_coche_absente_des_polices() {
    assert_eq!(
        display_text("« ✔ Vérifié » ou « ▲ À revoir »"),
        "« Vérifié » ou « ▲ À revoir »"
    );
    assert_eq!(display_text("sans signe"), "sans signe");
    for ph in phases(&sept_pouces()) {
        for t in
            std::iter::once(ph.intro.to_string()).chain(ph.steps.iter().flat_map(|s| [s.title.clone(), s.help.clone()]))
        {
            let t = display_text(&t);
            for c in ['✔', '✖', 'ℹ', '⚠', '✓'] {
                assert!(!t.contains(c), "« {c} » absent des polices dans : {t}");
            }
        }
    }
}

#[test]
fn les_textes_de_progression_sont_ceux_de_python() {
    assert_eq!(phase_text(0, 3, 11), "1. Préparer le drone (3/11)");
    assert_eq!(phase_text(1, 0, 0), "2. Filtres");
    assert_eq!(phase_text(3, 4, 4), "4. Finitions (4/4)");
    assert!(phase_finished(4, 4) && !phase_finished(3, 4) && !phase_finished(0, 0));
    assert_eq!(
        cli_text(&["set a = 1".into(), "set b = 2".into()]),
        "set a = 1\nset b = 2\nsave"
    );
    assert_eq!(cli_text(&[]), "save");
}

#[test]
fn le_premier_log_ouvert_depuis_l_accueil_passe_aux_courbes_mais_pas_depuis_une_phase() {
    let d = dossier("premier-log");
    let mut h = banc(app_sur(&d.join("drones.json")));
    h.run();
    h.state_mut().open(vec![donnees()]);
    attendre(&mut h, |a| !a.state.logs.is_empty());
    assert_eq!(h.state().screen, Screen::Viewer);
    // depuis une phase du guide : on reste sur le guide
    let mut h = banc(app_sur(&d.join("drones.json")));
    h.state_mut().guide.phase = "filters";
    h.run();
    h.state_mut().open(vec![donnees()]);
    attendre(&mut h, |a| !a.state.logs.is_empty());
    assert_eq!(h.state().screen, Screen::Guide);
}

#[test]
fn les_logs_du_drone_sont_verifies_classes_et_les_commandes_copiables() {
    let d = dossier("logs");
    let p = d.join("drones.json");
    // le nom du drone dans l'en-tête du log de test
    let nom = otb_log::flightlog::load(&donnees()).unwrap()[0]
        .settings
        .craft_name
        .clone();
    let mut app = app_sur(&p);
    let mut fiche = DroneCard::new(&nom);
    fiche.pid_khz = 3.2;
    app.guide.cards.add(fiche).unwrap();
    app.guide.select_by_name(&nom);
    app.guide.phase = "filters";
    let mut h = banc(app);
    h.run();
    h.state_mut().open(vec![donnees()]);
    attendre(&mut h, |a| !a.state.logs.is_empty());
    // la vérification complète finit en arrière-plan
    attendre(&mut h, |a| !a.guide.busy());
    h.get_by_label_contains("Logs de «");
    let role_initial = h.state().state.logs[0].log.path.to_string_lossy().into_owned();
    assert!(
        !h.state().guide.roles.contains_key(&role_initial),
        "aucun rôle choisi : le rôle est deviné"
    );
    // « classer dans les … » : le rôle choisi remplace le rôle deviné
    let lien = h
        .query_all_by_label_contains("→ classer dans les")
        .next()
        .expect("lien de reclassement");
    let texte = lien.accesskit_node().label().unwrap();
    let premier = if texte.contains(role_name(FILTERS)) {
        FILTERS
    } else {
        PID
    };
    lien.click();
    h.run_steps(4);
    assert_eq!(
        h.state().guide.roles.get(&role_initial).map(String::as_str),
        Some(premier),
        "clé = chemin du log"
    );
    // le lien propose maintenant le rôle d'origine : le cliquer fait revenir
    let retour = other_role(premier);
    h.get_by_label_contains(&format!("→ classer dans les {}", role_name(retour)))
        .click();
    h.run_steps(4);
    assert_eq!(
        h.state().guide.roles.get(&role_initial).map(String::as_str),
        Some(retour)
    );
    // les commandes CLI : le bouton copie les commandes du log suivies de « save »
    let cli = h.state().guide.cli_lines().to_vec();
    assert!(!cli.is_empty(), "ce log a des corrections à proposer (rôle {retour})");
    // le bouton est en bas du panneau, hors de la zone visible : on le déclenche comme un lecteur d'écran
    h.get_by_role_and_label(Role::Button, "Copier les commandes CLI")
        .click_accesskit();
    // le clic se joue sur deux cadres : on lit la sortie de chacun
    let mut copie = None;
    for _ in 0..4 {
        h.run_steps(1);
        copie = copie.or_else(|| {
            h.output().platform_output.commands.iter().find_map(|c| {
                if let egui::OutputCommand::CopyText(t) = c {
                    Some(t.clone())
                } else {
                    None
                }
            })
        });
    }
    let copie = copie.expect("le texte est copié");
    assert_eq!(copie, cli_text(&cli));
    assert!(copie.ends_with("\nsave") && copie.lines().count() == cli.len() + 1);
}

#[test]
fn ouvrir_un_outil_depuis_une_case_regle_la_periode_sur_la_partie_du_vol() {
    let d = dossier("outil");
    let p = d.join("drones.json");
    let nom = otb_log::flightlog::load(&donnees()).unwrap()[0]
        .settings
        .craft_name
        .clone();
    let mut app = app_sur(&p);
    app.guide.cards.add(DroneCard::new(&nom)).unwrap();
    app.guide.select_by_name(&nom);
    app.guide.phase = "filters";
    let mut h = banc(app);
    h.run();
    h.state_mut().open(vec![donnees()]);
    attendre(&mut h, |a| !a.state.logs.is_empty());
    attendre(&mut h, |a| !a.guide.busy());
    // une case « lire <outil> (partie stationnaire) » existe dans la phase des filtres
    let carte = h.state().guide.card().unwrap().clone();
    let (outil, partie) = phases(&carte)[1]
        .steps
        .iter()
        .find_map(|s| s.tool.zip(s.part))
        .expect("une case de la phase des filtres ouvre un outil sur une partie du vol");
    let bouton = format!("Ouvrir {}", tool_label(outil, Some(partie)));
    let (attendue, avant) = {
        let ll = &h.state().state.logs[0];
        let c = otb_advisor::log_check::check_log(&ll.log, Some(ll.sel()), &ll.label, false).unwrap();
        (if partie == "hover" { c.hover } else { c.moving_part }, ll.period)
    };
    h.get_by_role_and_label(Role::Button, &bouton).click();
    h.run_steps(4);
    // le bouton mène à la page de l'outil
    let ToolTarget::Page(titre) = tool_target(outil).unwrap() else {
        panic!("{outil}")
    };
    let attendu = h.state().pages.iter().position(|pg| pg.title() == titre).unwrap();
    assert_eq!(h.state().screen, Screen::Tool(attendu));
    // ce log de 5 s n'a pas de partie stationnaire : la période ne bouge pas ; sinon elle prend celle de la partie
    let ll = &h.state().state.logs[0];
    match attendue {
        None => assert_eq!(ll.period, avant),
        Some((t0, t1)) => assert_eq!(ll.period, (t0.max(ll.log.t[0]), t1.min(*ll.log.t.last().unwrap()))),
    }
}

#[test]
fn la_periode_d_une_partie_du_vol_vient_du_controle_du_log() {
    let mut c =
        otb_advisor::log_check::check_log(&otb_log::flightlog::load(&donnees()).unwrap()[0], None, "x", false).unwrap();
    c.hover = Some((1.0, 2.5));
    c.moving_part = Some((3.0, 4.0));
    assert_eq!(part_period(&c, "hover"), Some((1.0, 2.5)));
    assert_eq!(part_period(&c, "moving"), Some((3.0, 4.0)));
    assert_eq!(part_period(&c, "autre"), None);
    c.hover = None;
    assert_eq!(part_period(&c, "hover"), None);
}

#[test]
fn le_panneau_des_logs_garde_sa_hauteur_meme_s_il_commence_sans_fiche() {
    let d = dossier("hauteur-panneau");
    let mut h = banc(app_sur(&d.join("drones.json")));
    h.run_steps(3); // sans fiche : une seule phrase dans le panneau
    let mut app_card = DroneCard::new("Sept");
    app_card.size = "seven".into();
    h.state_mut().guide.cards.add(app_card).unwrap();
    h.state_mut().guide.select_by_name("Sept");
    h.run_steps(4);
    let haut = h.get_by_label("Logs de « Sept »").rect().top();
    // fenêtre de 800 : le panneau (250) et la barre d'état (≈ 25) laissent le titre vers 515 ; un panneau réduit à son contenu
    // le laisserait vers 650
    assert!(haut < 560.0, "panneau des logs trop petit : son titre est à {haut}");
}

#[test]
fn changer_la_periode_d_un_log_relance_sa_verification() {
    let d = dossier("periode");
    let nom = otb_log::flightlog::load(&donnees()).unwrap()[0]
        .settings
        .craft_name
        .clone();
    let mut app = app_sur(&d.join("drones.json"));
    app.guide.cards.add(DroneCard::new(&nom)).unwrap();
    app.guide.select_by_name(&nom);
    app.guide.phase = "filters";
    let mut h = banc(app);
    h.run();
    h.state_mut().open(vec![donnees()]);
    attendre(&mut h, |a| !a.state.logs.is_empty());
    attendre(&mut h, |a| !a.guide.busy());
    let texte = |h: &Harness<'static, App>| {
        let n = h
            .query_all_by_label_contains("s de vol dans la période d'analyse")
            .next()
            .expect("durée analysée");
        n.accesskit_node().label().or_else(|| n.value()).unwrap_or_default()
    };
    let avant = texte(&h);
    // la période passe à la première moitié du vol : les résultats gardés pour l'ancienne ne servent plus
    let (t0, t1) = h.state().state.logs[0].period;
    // `period` directement : `set_period` vide déjà le cache, il ne prouverait pas la clé (L-071)
    h.state_mut().state.logs[0].period = (t0, t0 + (t1 - t0) / 2.0);
    attendre(&mut h, |a| !a.guide.busy());
    let apres = texte(&h);
    assert_ne!(avant, apres, "la durée analysée doit suivre la période");
    // et revenir à l'ancienne période redonne le premier résultat
    h.state_mut().state.logs[0].period = (t0, t1);
    attendre(&mut h, |a| !a.guide.busy());
    assert_eq!(texte(&h), avant);
}

#[test]
fn un_nouveau_log_fait_passer_le_guide_sur_la_fiche_du_drone() {
    let d = dossier("suivre-fiche");
    let nom = otb_log::flightlog::load(&donnees()).unwrap()[0]
        .settings
        .craft_name
        .clone();
    let mut app = app_sur(&d.join("drones.json"));
    app.guide
        .cards
        .add(DroneCard::new(&format!("{nom} (autre drone)")))
        .unwrap();
    app.guide
        .cards
        .add(DroneCard::new(&format!("  {}  ", nom.to_uppercase())))
        .unwrap(); // casse et espaces ignorés
    app.guide.select_by_name(&format!("{nom} (autre drone)"));
    app.guide.phase = "filters";
    let mut h = banc(app);
    h.run();
    assert_eq!(h.state().guide.card().unwrap().name, format!("{nom} (autre drone)"));
    h.state_mut().open(vec![donnees()]);
    attendre(&mut h, |a| !a.state.logs.is_empty());
    assert_eq!(
        h.state().guide.card().unwrap().name.trim(),
        nom.to_uppercase(),
        "la fiche du drone du log est choisie"
    );
    // un choix de l'utilisateur ensuite n'est pas défait à chaque cadre : seul un nouveau log fait changer la fiche
    h.state_mut().guide.selected = Some(0);
    h.run_steps(4);
    assert_eq!(h.state().guide.card().unwrap().name, format!("{nom} (autre drone)"));
    // un log d'un drone sans fiche ne change pas la fiche choisie
    let mut app = app_sur(&d.join("autre.json"));
    app.guide.cards.add(DroneCard::new("Sans rapport")).unwrap();
    app.guide.select_by_name("Sans rapport");
    app.guide.phase = "filters";
    let mut h = banc(app);
    h.run();
    h.state_mut().open(vec![donnees()]);
    attendre(&mut h, |a| !a.state.logs.is_empty());
    assert_eq!(h.state().guide.card().unwrap().name, "Sans rapport");
}

fn controle(
    role: &'static str,
    hover: Option<(f64, f64)>,
    moving: Option<(f64, f64)>,
) -> otb_advisor::log_check::LogCheck {
    let log = &otb_log::flightlog::load(&donnees()).unwrap()[0];
    let mut c = otb_advisor::log_check::check_log(log, None, "x", false).unwrap();
    (c.role, c.hover, c.moving_part) = (role, hover, moving);
    c
}

#[test]
fn le_dernier_log_d_un_role_est_celui_qui_compte() {
    assert_eq!(last_index_with_role(&[FILTERS, PID, FILTERS, PID], FILTERS), Some(2));
    assert_eq!(last_index_with_role(&[FILTERS, PID, FILTERS, PID], PID), Some(3));
    assert_eq!(last_index_with_role(&[PID], FILTERS), None);
    assert_eq!(last_index_with_role(&[], PID), None);
}

#[test]
fn la_partie_du_vol_vient_du_dernier_log_des_filtres() {
    let (a, b, c) = (
        controle(FILTERS, Some((1.0, 2.0)), Some((3.0, 4.0))),
        controle(FILTERS, Some((5.0, 6.0)), None),
        controle(PID, Some((7.0, 8.0)), Some((9.0, 10.0))),
    );
    // le dernier log des filtres est « b » (les logs de série ne comptent pas)
    assert_eq!(part_target(&[&a, &b, &c], "hover"), Some((1, (5.0, 6.0))));
    // « b » n'a pas de partie avec mouvements : pas de repli sur un log plus ancien
    assert_eq!(part_target(&[&a, &b, &c], "moving"), None);
    assert_eq!(part_target(&[&a, &c], "moving"), Some((0, (3.0, 4.0))));
    assert_eq!(part_target(&[&c], "hover"), None, "aucun log des filtres");
    assert_eq!(part_target(&[], "hover"), None);
}

#[test]
fn un_nom_vide_devient_mon_drone() {
    let d = dossier("nom-vide");
    let p = d.join("drones.json");
    let mut h = banc(app_sur(&p));
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Nouveau drone").click();
    h.run_steps(3);
    h.get_by_label("Nom (comme dans Betaflight)")
        .set_text_value_accesskit("   ");
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Enregistrer").click();
    h.run_steps(4);
    assert_eq!(
        Cards::load(p).list.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["Mon drone"]
    );
}

fn trois_fiches(nom: &str) -> (PathBuf, Harness<'static, App>) {
    let d = dossier(nom);
    let p = d.join("drones.json");
    let mut app = app_sur(&p);
    for n in ["A", "B", "C"] {
        let mut c = DroneCard::new(n);
        c.done.push(("recording".into(), true));
        app.guide.cards.add(c).unwrap();
    }
    app.guide.select_by_name("B"); // la fiche du clic n'est ni la première ni la dernière
    app.guide.phase = "prep";
    let mut h = banc(app);
    h.run_steps(3);
    (p, h)
}

#[test]
fn la_confirmation_vise_la_fiche_du_clic_meme_si_la_fiche_choisie_change_ensuite() {
    // Supprimer : B est la fiche du clic, la fiche choisie passe ensuite sur C (un log de ce drone vient d'arriver)
    let (p, mut h) = trois_fiches("confirmation-supprimer");
    h.get_by_role_and_label(Role::Button, "Supprimer").click();
    h.run_steps(3);
    h.state_mut().guide.select_by_name("C");
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Oui").click();
    h.run_steps(4);
    assert_eq!(
        Cards::load(p).list.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["A", "C"],
        "B était la fiche du clic"
    );
    assert_eq!(
        h.state().guide.card().unwrap().name,
        "C",
        "la fiche choisie reste C (Python : la première seulement si on supprime la choisie)"
    );
    // Recommencer : seule B est remise à zéro
    let (p, mut h) = trois_fiches("confirmation-recommencer");
    h.get_by_role_and_label(Role::Button, "Recommencer").click();
    h.run_steps(3);
    h.state_mut().guide.select_by_name("C");
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Oui").click();
    h.run_steps(4);
    let relu = Cards::load(p).list;
    let vide = |n: &str| relu.iter().find(|c| c.name == n).unwrap().done.is_empty();
    assert!(!vide("A") && vide("B") && !vide("C"), "seule B est remise à zéro");
    // supprimer la fiche choisie : le choix retombe sur la première, comme Python
    let (_p, mut h) = trois_fiches("confirmation-choisie");
    h.get_by_role_and_label(Role::Button, "Supprimer").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Oui").click();
    h.run_steps(4);
    assert_eq!(h.state().guide.card().unwrap().name, "A");
}

#[test]
fn la_confirmation_ne_touche_a_rien_si_la_liste_des_fiches_a_change() {
    let (p, mut h) = trois_fiches("confirmation-liste-changee");
    h.get_by_role_and_label(Role::Button, "Supprimer").click();
    h.run_steps(3);
    // pendant la confirmation, B disparaît : le numéro 1 désigne maintenant C
    h.state_mut().guide.cards.delete(1).unwrap();
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Oui").click();
    h.run_steps(4);
    assert_eq!(
        Cards::load(p).list.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["A", "C"],
        "C n'est pas supprimée"
    );
}

#[test]
fn un_log_dont_la_verification_echoue_reste_au_drone() {
    // `check_log` ne rend jamais `Err` sur un journal fabriqué (essayé : 2 à 40 points, 1 et 8 kHz) : la règle est une fonction pure
    assert_eq!(other_drone_logs(5, &[0, 2], &[3]), [1, 4]);
    assert_eq!(other_drone_logs(3, &[], &[]), [0, 1, 2]);
    assert_eq!(other_drone_logs(2, &[0], &[1]), Vec::<usize>::new());
    assert_eq!(other_drone_logs(0, &[], &[]), Vec::<usize>::new());
}

#[test]
fn le_bouton_modifier_garde_l_avancement_et_la_place_de_la_fiche() {
    let d = dossier("modifier");
    let p = d.join("drones.json");
    let mut app = app_sur(&p);
    for n in ["A", "B", "C"] {
        let mut c = DroneCard::new(n);
        c.done.push(("recording".into(), true));
        app.guide.cards.add(c).unwrap();
    }
    app.guide.select_by_name("B");
    let mut h = banc(app);
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Modifier").click();
    h.run_steps(3);
    h.get_by_label("Nom (comme dans Betaflight)")
        .set_text_value_accesskit("B2");
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Enregistrer").click();
    h.run_steps(4);
    let relu = Cards::load(p).list;
    assert_eq!(
        relu.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["A", "B2", "C"],
        "même place"
    );
    assert_eq!(relu[1].done, [("recording".to_string(), true)], "avancement gardé");
    assert_eq!(
        h.state().guide.card().unwrap().name,
        "B2",
        "la fiche modifiée reste la fiche choisie"
    );
}
