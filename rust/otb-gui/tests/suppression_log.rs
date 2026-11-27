//! Supprimer un log depuis le tableau de bord (D-089) : la corbeille d'un fragment, la confirmation en haut du
//! panneau, le log entier qui quitte la bibliothèque et l'écran, et le fichier copié qui ne part que case cochée.
//!
//! **Contre-preuves** (L-071) : chaque protection a été cassée à la main et le test visé a rougi — c'est noté sous
//! le test concerné.

mod support;
use support::Cliquer;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_gui::app::{App, Screen};
use otb_gui::dashboard::{copy_text, delete_log_text};
use otb_store::Library;
use otb_store::model::{Drone, Fragment, LogEntry};

fn banc(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1300.0, 800.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app)
}

fn donnees() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")
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

/// Un dossier d'essai propre, hors du dépôt. Le nom porte le processus : deux exécutions ne se gênent pas.
fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-gui-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dossier d'essai");
    d
}

fn fragment(id: &str, log_id: &str, date: &str) -> Fragment {
    Fragment {
        id: id.to_string(),
        log_id: log_id.to_string(),
        session: 1,
        start: 0,
        end: 10,
        name: id.to_string(),
        role: "filters".to_string(),
        date: date.to_string(),
        chosen: true,
    }
}

/// Un log copié par USB (fichier écrit dans le dossier des copies) et son fragment.
fn copie(lib: &mut Library, id: &str, date: &str) -> PathBuf {
    let path = lib.copy_path("JeNo", id).unwrap().unwrap();
    std::fs::write(&path, b"vol").unwrap();
    lib.add_log(LogEntry {
        id: id.into(),
        drone_id: "uid-1".into(),
        name: format!("{id}.bbl"),
        path: path.display().to_string(),
        copied: true,
        bytes: 3,
        added: "2026-11-27".into(),
    });
    lib.put_fragment(fragment(&format!("fr-{id}"), id, date));
    path
}

// ---------------------------------------------------------------- sans écran

#[test]
fn la_confirmation_dit_ce_qui_part_et_ce_que_devient_le_fichier() {
    let mut lib = Library::in_memory();
    let mut log = LogEntry {
        id: "l1".into(),
        drone_id: "d1".into(),
        name: "LOG1".into(),
        path: "/x/LOG1.bbl".into(),
        copied: false,
        bytes: 3_355_443,
        added: "2026-11-27".into(),
    };
    lib.add_log(log.clone());
    assert!(delete_log_text(&lib, &log).starts_with("Aucun fragment"));
    lib.put_fragment(fragment("Vol 1", "l1", "2026-11-27"));
    let t = delete_log_text(&lib, &log);
    assert!(t.starts_with("Son fragment « Vol 1 » quitte"), "{t}");
    assert!(t.contains("reste dans votre dossier"), "{t}");
    lib.put_fragment(fragment("Vol 2", "l1", "2026-11-27"));
    log.copied = true;
    let t = delete_log_text(&lib, &log);
    assert!(t.starts_with("Ses 2 fragments (« Vol 1 », « Vol 2 ») quittent"), "{t}");
    assert!(t.contains("Sans la case, le fichier copié reste"), "{t}");
    assert_eq!(
        copy_text(3_355_443),
        "Supprimer aussi le fichier copié par USB (3,2 Mo)"
    );
}

// ---------------------------------------------------------------- à l'écran

/// La corbeille d'un fragment demande confirmation ; « Garder » ne change rien ; « Supprimer définitivement »
/// retire le log de la bibliothèque **et ferme sa pastille**. Le drone reste ; le fichier, ouvert depuis un dossier
/// de l'utilisateur, n'est jamais touché.
///
/// Contre-preuve vue en échec (2026-11-27) : sans la boucle qui ferme les logs dans `App::delete_log`, la pastille
/// restée ouverte remet son fragment dans la liste d'analyse, et `lib.analysis().items.is_empty()` rougit.
#[test]
fn la_corbeille_supprime_le_log_et_ferme_sa_pastille() {
    let mut h = banc(App::new(otb_gui::tools::all_pages()));
    h.run_steps(2);
    h.state_mut().open(vec![donnees()]);
    attendre_logs(&mut h, 1);
    let drone = h.state().library.drones()[0].id.clone();
    let nom = h.state().library.logs()[0].name.clone();
    h.state_mut().dash.selected = Some(drone.clone());
    h.state_mut().screen = Screen::Dashboard;
    h.run_steps(3);

    let corbeille = format!("Supprimer le log {nom}");
    let question = format!("Supprimer le log « {nom} » ?");
    assert!(
        h.query_by_label(&question).is_none(),
        "pas de confirmation avant la corbeille"
    );
    h.cliquer_role(Role::Button, &corbeille);
    h.run_steps(3);
    h.get_by_label(&question);
    assert!(
        h.query_by_label_contains("Supprimer aussi le fichier copié").is_none(),
        "un log de l'utilisateur n'a pas de case"
    );
    h.cliquer_role(Role::Button, "Garder");
    h.run_steps(3);
    assert!(h.query_by_label(&question).is_none());
    assert_eq!(h.state().library.logs().len(), 1, "« Garder » ne change rien");

    h.cliquer_role(Role::Button, &corbeille);
    h.run_steps(3);
    h.cliquer_role(Role::Button, "Supprimer définitivement");
    h.run_steps(3);
    let lib = &h.state().library;
    assert!(lib.logs().is_empty() && lib.fragments().is_empty());
    assert!(lib.analysis().items.is_empty());
    assert!(lib.drone(&drone).is_some(), "le drone reste");
    assert!(
        h.state().state.logs.is_empty(),
        "la pastille du log supprimé est fermée"
    );
    assert!(donnees().exists(), "le fichier de l'utilisateur reste");
    assert_eq!(h.state().screen, Screen::Dashboard);
}

/// Un log copié par USB : sans la case, son fichier reste ; case cochée, il part. La case est décochée à chaque
/// corbeille cliquée — même en passant d'une confirmation à l'autre sans « Garder » — et seul le log choisi est
/// touché.
///
/// Contre-preuve vue en échec (2026-11-27) : sans la remise à `false` de la case au clic sur la corbeille, la case
/// cochée pour le log `a` l'était encore pour le log `c` et l'assertion « case décochée » rougissait. Et avec
/// `Align::Center` sur la ligne des boutons de `confirm_delete`, le cadre prenait toute la hauteur du panneau : la
/// corbeille de `c` passait hors de la zone cliquable et la même assertion rougissait.
#[test]
fn le_fichier_copie_ne_part_que_case_cochee() {
    let dir = dossier("suppr-log-ecran");
    let mut lib = Library::open(dir.clone());
    lib.put_drone(Drone::new("uid-1", "JeNo"));
    let garde = copie(&mut lib, "a", "2026-11-25");
    let efface = copie(&mut lib, "b", "2026-11-26");
    let autre = copie(&mut lib, "c", "2026-11-27");
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library = lib;
    app.dash.selected = Some("uid-1".into());
    app.screen = Screen::Dashboard;
    let mut h = banc(app);
    h.run_steps(3);
    let case = copy_text(3);

    // case cochée : le fichier part
    h.cliquer_role(Role::Button, "Supprimer le log b.bbl");
    h.run_steps(3);
    h.cliquer_label(&case);
    h.run_steps(2);
    assert!(h.state().dash.delete_copy);
    h.cliquer_role(Role::Button, "Supprimer définitivement");
    h.run_steps(3);
    assert!(h.state().library.log("b").is_none());
    assert!(!efface.exists(), "case cochée : le fichier copié est effacé");

    // case cochée pour `a`, puis corbeille de `c` sans « Garder » : la case de `c` est décochée
    h.cliquer_role(Role::Button, "Supprimer le log a.bbl");
    h.run_steps(3);
    h.cliquer_label(&case);
    h.run_steps(2);
    h.cliquer_role(Role::Button, "Supprimer le log c.bbl");
    h.run_steps(3);
    assert!(!h.state().dash.delete_copy, "case décochée");
    h.get_by_label("Supprimer le log « c.bbl » ?");
    h.cliquer_role(Role::Button, "Supprimer définitivement");
    h.run_steps(3);
    assert!(h.state().library.log("c").is_none());
    assert!(autre.exists(), "case décochée : le fichier copié reste");

    // `a` est intact
    assert!(h.state().library.log("a").is_some() && garde.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
