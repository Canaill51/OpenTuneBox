//! Fiche d'un drone (lot 30d, D-068) : traduction pour l'assistant, règles d'enregistrement, textes de la liste des
//! configurations, puis l'écran ouvert depuis le tableau de bord, la suppression et le rattachement des logs.
//!
//! **Contre-preuves** (L-071) : chaque protection a été cassée à la main et le test visé a rougi — c'est noté sous
//! le test concerné.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_advisor::drone_card::DroneCard;
use otb_gui::app::{App, Screen};
use otb_gui::drone_sheet::*;
use otb_store::Library;
use otb_store::model::{Drone, LogEntry};

fn banc(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1300.0, 800.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app)
}

fn donnees() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")
}

fn nom_du_log() -> String {
    otb_log::flightlog::load(&donnees()).unwrap()[0]
        .settings
        .craft_name
        .trim()
        .to_string()
}

fn attendre_logs(h: &mut Harness<'static, App>, combien: usize) {
    let debut = Instant::now();
    while h.state().state.logs.len() < combien {
        h.step();
        assert!(debut.elapsed() < Duration::from_secs(30), "log pas ouvert");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(3);
}

fn avec_fiche(id: &str, nom: &str) -> Drone {
    let mut d = Drone::new(id, nom);
    d.sheet = Some(new_sheet());
    d
}

// ---------------------------------------------------------------- sans écran

/// Une fiche du Guide passe par la bibliothèque et revient la même, pour chacun des cinq profils ; la version
/// 2026.6 devient la famille 2025.12 que l'assistant connaît.
#[test]
fn la_fiche_se_traduit_pour_l_assistant_et_revient() {
    for profil in ["whoop", "micro", "five", "seven", "large"] {
        let mut c = DroneCard::new("A");
        c.size = profil.into();
        c.pid_khz = 3.2;
        c.esc = "Bluejay".into();
        c.bf = "4.4".into();
        c.done = vec![("recording".into(), true)];
        let mut d = Drone::new("A", "A");
        d.sheet = Some(sheet_of_card(&c));
        assert_eq!(card_of(&d), Some(c), "profil {profil}");
    }
    let mut d = Drone::new("B", "B");
    assert_eq!(card_of(&d), None, "pas de fiche, pas de fiche du Guide");
    d.sheet = Some(new_sheet());
    let c = card_of(&d).unwrap();
    assert_eq!((c.bf.as_str(), c.size.as_str(), c.pid_khz), ("2025.12", "five", 8.0));
    // chaque taille proposée choisit un profil qui existe
    for (taille, _) in SIZES {
        assert!(
            otb_advisor::profiles::profile(profile_of_size(taille)).is_some(),
            "{taille}"
        );
    }
    assert_eq!(profile_of_size("inconnue"), "five");
}

/// ⚠️ Deux drones ne partagent jamais un nom (les logs iraient au premier) ; un drone connu par ses seuls logs
/// **reçoit** la fiche d'un nouveau drone du même nom au lieu d'en faire un second.
///
/// Contre-preuve vue en échec (2026-11-23) : sans le refus du nom déjà pris en modification, la troisième
/// assertion rougit (deux drones « Cine »).
#[test]
fn enregistrer_refuse_un_nom_pris_et_rattache_un_drone_connu_par_ses_logs() {
    let mut lib = Library::in_memory();
    lib.put_drone(Drone::new("JeNo", "JeNo")); // vu dans un log, sans fiche
    lib.put_drone(avec_fiche("Cine", "Cine"));

    // nouveau drone du nom d'un drone sans fiche : c'est lui qui la reçoit
    let id = save_form(&mut lib, &SheetForm::new("  JeNo ", new_sheet())).unwrap();
    assert_eq!((id.as_str(), lib.drones().len()), ("JeNo", 2));
    assert!(lib.drone("JeNo").unwrap().sheet.is_some());
    // nouveau drone du nom d'un drone qui a sa fiche : refusé
    assert!(save_form(&mut lib, &SheetForm::new("Cine", new_sheet())).is_err());
    // renommer un drone du nom d'un autre : refusé
    let mut f = SheetForm::edit(lib.drone("JeNo").unwrap());
    f.name = "Cine".into();
    assert!(save_form(&mut lib, &f).is_err());
    assert_eq!(lib.drones().iter().filter(|d| d.name == "Cine").count(), 1);
    // renommer : même drone, même identifiant ; un nouveau drone qui prend l'ancien nom reçoit un autre identifiant
    f.name = "JeNo Ced".into();
    assert_eq!(save_form(&mut lib, &f).unwrap(), "JeNo");
    let id = save_form(&mut lib, &SheetForm::new("JeNo", new_sheet())).unwrap();
    assert_eq!(id, "JeNo (2)");
    assert_eq!(lib.drone_by_name("JeNo Ced").unwrap().id, "JeNo");
}

#[test]
fn les_dates_et_les_versions_se_lisent() {
    assert_eq!(date_fr("2026-11-13T18:15:02"), "13 nov. 2026, 18:15");
    assert_eq!(date_fr("2026-02-01"), "1 févr. 2026");
    assert_eq!(date_fr("n'importe quoi"), "n'importe quoi");
    assert_eq!(date_fr("2026-13-01"), "2026-13-01");
    assert_eq!(
        version_of_firmware("Betaflight 2026.6.0-rc3 (f625a9996) STM32F405"),
        Some("2026.6")
    );
    assert_eq!(
        version_of_firmware("Betaflight 4.5.1 (77d01ba3b) STM32F7X2"),
        Some("4.5")
    );
    assert_eq!(version_of_firmware("Betaflight 4.3.2"), None);
    assert_eq!(version_of_firmware(""), None);
}

/// La ligne d'une configuration : ses premiers changements, ou « configuration initiale », puis le premier log
/// rangé ce jour-là ou après.
#[test]
fn le_resume_d_une_configuration_dit_ce_qui_change_et_avant_quel_vol() {
    let mut lib = Library::in_memory();
    lib.put_drone(Drone::new("d", "D"));
    lib.add_log(LogEntry {
        id: "l1".into(),
        drone_id: "d".into(),
        name: "BTFL_014.BBL".into(),
        path: String::new(),
        copied: false,
        bytes: 1,
        added: "2026-11-02".into(),
    });
    lib.record_config("d", "2026-11-02T11:15:00", "set gyro_lpf1_static_hz = 150\n");
    lib.record_config("d", "2026-11-09T15:50:00", "set gyro_lpf1_static_hz = 165\n");
    lib.record_config(
        "d",
        "2026-11-10T09:00:00",
        "set gyro_lpf1_static_hz = 170\nset dyn_notch_count = 2\nset rpm_filter_q = 600\n",
    );
    let configs = lib.configs_of_drone("d");
    assert_eq!(
        config_summary(&lib, &configs, 0),
        "configuration initiale · avant BTFL_014"
    );
    assert_eq!(config_summary(&lib, &configs, 1), "gyro_lpf1_static_hz 150 → 165");
    assert_eq!(
        config_summary(&lib, &configs, 2),
        "gyro_lpf1_static_hz 165 → 170, dyn_notch_count défaut → 2, + 1"
    );
}

// ---------------------------------------------------------------- l'écran

/// Le crayon d'une carte ouvre la fiche **de ce drone** ; « Annuler » ne change rien, « Enregistrer » range la
/// fiche dans la bibliothèque et le Guide la voit aussitôt.
///
/// Contre-preuve vue en échec (2026-11-23) : le crayon renvoyé vers le Guide (l'ancien geste), l'écran attendu
/// n'est pas `DroneSheet` et la première assertion rougit.
#[test]
fn le_crayon_d_une_carte_ouvre_la_fiche_du_drone() {
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library.put_drone(Drone::new("uid-1", "JeNo Ced"));
    app.screen = Screen::Dashboard;
    let mut h = banc(app);
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Modifier le drone").click();
    h.run_steps(3);
    assert_eq!(h.state().screen, Screen::DroneSheet);
    h.get_by_label("Tableau de bord · fiche du drone");
    h.get_by_label("Nom du drone").set_text_value_accesskit("Autre nom");
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Annuler").click();
    h.run_steps(3);
    assert_eq!(h.state().screen, Screen::Dashboard);
    let d = h.state().library.drone("uid-1").unwrap().clone();
    assert_eq!(
        (d.name.as_str(), d.sheet.is_none()),
        ("JeNo Ced", true),
        "« Annuler » ne change rien"
    );

    h.get_by_role_and_label(Role::Button, "Modifier le drone").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Enregistrer").click();
    h.run_steps(3);
    assert_eq!(h.state().screen, Screen::Dashboard);
    assert_eq!(h.state().library.drone("uid-1").unwrap().sheet, Some(new_sheet()));
    assert_eq!(h.state().guide.cards.list.len(), 1, "le Guide voit la fiche");
    assert_eq!(h.state().guide.card().unwrap().name, "JeNo Ced");
}

/// « Nouveau drone » du tableau de bord crée un drone et sa carte, choisie au retour.
#[test]
fn nouveau_drone_cree_un_drone_et_sa_carte() {
    let mut app = App::new(otb_gui::tools::all_pages());
    app.screen = Screen::Dashboard;
    let mut h = banc(app);
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Nouveau drone").click();
    h.run_steps(3);
    assert_eq!(h.state().screen, Screen::DroneSheet);
    h.get_by_label("Aucune configuration : elle sera enregistrée au premier branchement.");
    assert!(
        h.query_by_label("Supprimer…").is_none(),
        "un drone pas encore créé ne se supprime pas"
    );
    h.get_by_label("Nom du drone").set_text_value_accesskit("Cine 3");
    h.run_steps(2);
    h.get_by_label("Nouveau drone : Cine 3");
    h.get_by_role_and_label(Role::Button, "Créer le drone").click();
    h.run_steps(3);
    let lib = &h.state().library;
    assert_eq!(lib.drones().len(), 1);
    assert_eq!(lib.drones()[0].name, "Cine 3");
    assert_eq!(h.state().dash.selected.as_deref(), Some("Cine 3"));
    assert_eq!(h.state().screen, Screen::Dashboard);
    // sa carte, et son panneau ouvert à droite (le drone est choisi)
    assert!(h.query_all_by_label("Cine 3").count() >= 2);
}

/// Supprimer un drone retire ses fragments de la bibliothèque **et de l'écran** : un log ouvert de ce drone se
/// ferme, sinon sa pastille montrerait un fragment qui n'existe plus.
///
/// Contre-preuve vue en échec (2026-11-23) : sans la boucle qui ferme les logs dans `App::apply_sheet`, le log
/// reste ouvert et la dernière assertion rougit.
#[test]
fn supprimer_un_drone_ferme_ses_logs_ouverts() {
    let mut h = banc(App::new(otb_gui::tools::all_pages()));
    h.run_steps(2);
    h.state_mut().open(vec![donnees()]);
    attendre_logs(&mut h, 1);
    let id = h.state().library.drones()[0].id.clone();
    let form = SheetForm::edit(h.state().library.drone(&id).unwrap());
    h.state_mut().open_sheet(form);
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Supprimer…").scroll_to_me();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Supprimer…").click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Supprimer définitivement")
        .click();
    h.run_steps(3);
    assert!(h.state().library.drones().is_empty() && h.state().library.fragments().is_empty());
    assert_eq!(h.state().screen, Screen::Dashboard);
    assert!(h.state().state.logs.is_empty(), "le log du drone supprimé est fermé");
}

/// ⚠️ C'est le **nom** qui relie un log à son drone : un drone créé à la main (identifiant de sa création) reçoit
/// les logs qui portent son nom, au lieu qu'un second drone du même nom apparaisse.
///
/// Contre-preuve vue en échec (2026-11-23) : en cherchant le drone par identifiant comme avant, un second drone
/// est créé et la première assertion rougit.
#[test]
fn un_log_rejoint_le_drone_qui_porte_son_nom() {
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library.put_drone(avec_fiche("cree-a-la-main", &nom_du_log()));
    let mut h = banc(app);
    h.run_steps(2);
    h.state_mut().open(vec![donnees()]);
    attendre_logs(&mut h, 1);
    let lib = &h.state().library;
    assert_eq!(lib.drones().len(), 1, "pas de second drone du même nom");
    assert_eq!(lib.logs()[0].drone_id, "cree-a-la-main");
    assert!(lib.drones()[0].sheet.is_some(), "la fiche est gardée");
}

// ---------------------------------------------------------------- fiche cible (lot 31b, D-072)

/// Tant que le pilote n'a rien changé, les cases montrent les seuils de l'assistant pour la taille **choisie** :
/// changer la taille change les valeurs. Une cible enregistrée, elle, ne bouge plus.
#[test]
fn la_fiche_cible_suit_la_taille_tant_qu_elle_n_est_pas_changee() {
    let mut s = new_sheet();
    assert_eq!(
        target_texts(&s),
        ["22", "", "1,20", "", "35", "", "-28,3", "-13,1", "1,90", "2,70"]
    );
    assert_eq!(target_note(&s), "Valeurs proposées pour la taille 5″, modifiables.");
    s.size = "7″".into();
    assert_eq!(target_texts(&s)[0], "24", "latence du profil 6 à 7 pouces");
    assert_eq!(target_texts(&s)[6], "-31,4");
    s.size = "65 mm".into();
    assert_eq!(
        target_note(&s),
        "Valeurs proposées pour la taille 65 mm (seuils encore provisoires), modifiables."
    );

    s.target = Some(vec![
        ("latency_ms".into(), "18".into()),
        ("rise_ms".into(), "12".into()),
    ]);
    let t = target_texts(&s);
    assert_eq!((t[0].as_str(), t[2].as_str(), t[3].as_str()), ("18", "", "12"));
    assert!(target_note(&s).starts_with("Valeurs choisies pour ce drone"));
}

/// Une case qui n'est pas un nombre refuse l'enregistrement, et la bibliothèque n'est pas touchée.
#[test]
fn une_cible_illisible_refuse_l_enregistrement() {
    let mut lib = Library::in_memory();
    let mut form = SheetForm::new("Cine 3", new_sheet());
    form.sheet.target = Some(vec![("peak_high".into(), "1,1x".into())]);
    let e = save_form(&mut lib, &form).unwrap_err();
    assert_eq!(e, "Fiche cible : « Pic, haut de la zone » n'est pas un nombre (1,1x).");
    assert!(lib.drones().is_empty());
    form.sheet.target = Some(vec![
        ("peak_high".into(), " 1.1 ".into()),
        ("rise_ms".into(), "".into()),
    ]);
    assert!(save_form(&mut lib, &form).is_ok(), "point décimal et case vide permis");
}

/// À l'écran : une case changée fige les dix et s'enregistre ; « Valeurs proposées » les remet à suivre la taille ;
/// un texte illisible laisse la fiche ouverte avec le message.
#[test]
fn la_fiche_cible_se_change_a_l_ecran() {
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library.put_drone(avec_fiche("uid-1", "JeNo Ced"));
    app.screen = Screen::Dashboard;
    let mut h = banc(app);
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Modifier le drone").click();
    h.run_steps(3);
    h.get_by_label("Fiche cible");
    h.get_by_label("Valeurs proposées pour la taille 5″, modifiables.");
    h.get_by_label("Latence (50 %)").set_text_value_accesskit("18");
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Enregistrer").click();
    h.run_steps(3);
    assert_eq!(h.state().screen, Screen::Dashboard);
    let cible = h
        .state()
        .library
        .drone("uid-1")
        .unwrap()
        .sheet
        .clone()
        .unwrap()
        .target
        .unwrap();
    let attendu: Vec<(String, String)> = otb_advisor::target::ROWS
        .iter()
        .zip(["18", "", "1,20", "", "35", "", "-28,3", "-13,1", "1,90", "2,70"])
        .map(|(r, v)| (r.key.to_string(), v.to_string()))
        .collect();
    assert_eq!(
        cible, attendu,
        "une case changée, les neuf autres gardées telles qu'affichées"
    );

    h.get_by_role_and_label(Role::Button, "Modifier le drone").click();
    h.run_steps(3);
    h.get_by_label("Pic, haut de la zone").set_text_value_accesskit("abc");
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Enregistrer").click();
    h.run_steps(3);
    assert_eq!(h.state().screen, Screen::DroneSheet, "refusé : la fiche reste ouverte");
    h.get_by_label("Fiche cible : « Pic, haut de la zone » n'est pas un nombre (abc).");

    // sous le bas du banc de 800 points : le faire défiler d'abord (L-114)
    h.get_by_role_and_label(Role::Button, "Valeurs proposées")
        .scroll_to_me();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Valeurs proposées").click();
    h.run_steps(2);
    h.get_by_role_and_label(Role::Button, "Enregistrer").click();
    h.run_steps(3);
    assert_eq!(h.state().screen, Screen::Dashboard);
    assert_eq!(h.state().library.drone("uid-1").unwrap().sheet, Some(new_sheet()));
}
