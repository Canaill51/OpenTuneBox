//! L'écran « Bibliothèque » (lot 34, D-081) : ouvert du tableau de bord, déplacement par ses boutons, bibliothèque
//! trouvée ailleurs, dossier introuvable au lancement, déplacement refusé pendant une récupération USB, avertissement
//! des deux postes. Le travail sur les fichiers est prouvé dans `rust/otb-store/tests/emplacement.rs`.
//!
//! ⚠️ Tous les dossiers sont temporaires et `LibraryPage::default_dir` y pointe : aucun test n'écrit le pointeur ni
//! la bibliothèque du poste (L-075).
//!
//! **Contre-preuves** (L-071) : notées sous chaque test.

mod support;
use support::Cliquer;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_gui::app::{App, Screen};
use otb_gui::library_page::{LibraryPage, USB_BUSY};
use otb_store::Library;
use otb_store::model::{Drone, LogEntry};
use otb_store::place::{self, Place};

fn banc(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1300.0, 800.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app)
}

fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-gui-emplacement-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dossier d'essai");
    d
}

/// Une bibliothèque écrite dans `dir`, avec un drone de ce nom et un log copié.
fn bibliotheque(dir: &Path, drone: &str) -> Library {
    let mut lib = Library::open(dir.to_path_buf());
    lib.put_drone(Drone::new(drone, drone));
    let path = lib
        .copy_path(drone, "2026-11-25_10-00-00")
        .expect("dossier")
        .expect("créé");
    std::fs::write(&path, vec![7u8; 50_000]).expect("log copié");
    lib.add_log(LogEntry {
        id: format!("log-{drone}"),
        drone_id: drone.into(),
        name: "2026-11-25_10-00-00.bbl".into(),
        path: path.to_string_lossy().into_owned(),
        copied: true,
        bytes: 50_000,
        added: "2026-11-25T10:00:00".into(),
    });
    lib.save().expect("écrite");
    lib
}

/// L'application sur la bibliothèque `dir`, l'écran de la bibliothèque branché sur ces dossiers d'essai ; la boîte
/// « choisir un dossier » rend `choisi`.
fn app(defaut: &Path, lib: Library, choisi: Option<PathBuf>) -> App {
    let mut a = App::new(otb_gui::tools::all_pages());
    a.library = lib;
    a.libpage.default_dir = Some(defaut.to_path_buf());
    a.libpage.pick_folder = Box::new(move || choisi.clone());
    a.screen = Screen::Dashboard;
    a
}

fn attendre(h: &mut Harness<'static, App>, quoi: &str, fini: impl Fn(&App) -> bool) {
    let debut = Instant::now();
    while !fini(h.state()) {
        h.step();
        assert!(debut.elapsed() < Duration::from_secs(30), "{quoi} : trop long");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(2);
}

#[test]
fn le_tableau_de_bord_ouvre_l_ecran_et_y_revient() {
    let base = dossier("ouvrir");
    let a = base.join("bibliotheque");
    let mut h = banc(app(&a, bibliotheque(&a, "JeNo"), None));
    h.run_steps(2);
    h.cliquer_label("Bibliothèque");
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Library);
    assert!(h.query_by_label(&a.to_string_lossy()).is_some(), "chemin affiché");
    assert!(h.query_by_label("Emplacement par défaut").is_some());
    assert!(h.query_by_label_contains("1 drone, 1 log dont 1 copié").is_some());
    h.cliquer_label("Retour");
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Dashboard);
}

#[test]
fn deplacer_par_l_ecran_garde_les_logs_copies_et_efface_l_ancien_dossier() {
    let base = dossier("deplacer");
    let a = base.join("donnees").join("bibliotheque");
    let b = base.join("disque");
    std::fs::create_dir_all(&b).expect("dossier choisi, vide");
    let mut h = banc(app(&a, bibliotheque(&a, "JeNo"), Some(b.clone())));
    h.state_mut().open_library_screen();
    h.run_steps(2);
    h.cliquer_label("Changer d'emplacement…");
    h.run_steps(2);
    assert!(
        h.query_by_label_contains("Vers ").is_some(),
        "confirmation demandée avant de copier"
    );
    assert!(a.exists(), "rien ne bouge avant la confirmation");
    h.cliquer_label("Déplacer");
    attendre(&mut h, "déplacement", |app| !app.libpage.moving() && !a.exists());
    attendre(&mut h, "message de fin", |app| {
        app.libpage
            .message
            .as_ref()
            .is_some_and(|(_, m)| m.contains("Ancien dossier effacé"))
    });
    let lib = &h.state().library;
    assert_eq!(lib.location(), Some(b.as_path()));
    let log = lib.log("log-JeNo").expect("log connu");
    assert!(
        Path::new(&log.path).starts_with(&b) && Path::new(&log.path).is_file(),
        "{}",
        log.path
    );
    assert_eq!(
        place::locate(&a),
        Place::Ready {
            dir: b.clone(),
            chosen: true
        }
    );
    assert_eq!(Library::open(b.clone()).drones().len(), 1);
    // Contre-preuve : `switch_to_copy` sans `relocate` → la bibliothèque reste annoncée à l'ancien dossier, effacé :
    // la première assertion après l'attente échoue.
}

#[test]
fn une_bibliotheque_trouvee_ailleurs_remplace_l_actuelle_sans_la_toucher() {
    let base = dossier("existante");
    let a = base.join("bibliotheque");
    let c = base.join("autre-pc");
    bibliotheque(&c.join(place::SUBFOLDER), "Autre");
    let mut h = banc(app(&a, bibliotheque(&a, "JeNo"), Some(c.clone())));
    let avant = std::fs::read(a.join("drones.json")).expect("lu");
    h.state_mut().open_library_screen();
    h.run_steps(2);
    h.cliquer_label("Changer d'emplacement…");
    h.run_steps(2);
    assert!(
        h.query_by_label_contains("contient déjà une bibliothèque : 1 drone")
            .is_some()
    );
    h.cliquer_label("Utiliser cette bibliothèque");
    h.run_steps(2);
    let lib = &h.state().library;
    assert_eq!(lib.location(), Some(c.join(place::SUBFOLDER).as_path()));
    assert_eq!(lib.drones()[0].name, "Autre");
    assert_eq!(
        std::fs::read(a.join("drones.json")).expect("intacte"),
        avant,
        "l'actuelle a changé"
    );
    assert!(
        a.join("logs").join("JeNo").exists(),
        "les logs copiés de l'actuelle restent"
    );
    assert_eq!(
        place::locate(&a),
        Place::Ready {
            dir: c.join(place::SUBFOLDER),
            chosen: true
        }
    ); // Contre-preuve : confirmer une bibliothèque trouvée en lançant une copie par-dessus (`start_move`) → la
    // bibliothèque en service n'est pas celle trouvée.
}

#[test]
fn introuvable_au_lancement_rien_ne_s_ecrit_puis_reessayer_la_retrouve() {
    let base = dossier("introuvable");
    let a = base.join("donnees").join("bibliotheque");
    let b = base.join("disque").join("OpenTuneBox");
    bibliotheque(&b, "JeNo");
    place::set_pointer(&a, &b).expect("pointeur");
    std::fs::rename(base.join("disque"), base.join("debranche")).expect("débranché");
    let Place::Missing { why, .. } = place::locate(&a) else {
        panic!("dossier absent pris pour une bibliothèque");
    };
    let mut h = banc(app(&a, Library::in_memory(), None));
    h.state_mut().show_library_problem(Some(why), None);
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Library);
    assert!(h.query_by_label("Bibliothèque introuvable").is_some());
    // une bibliothèque introuvable n'écrit rien, nulle part
    h.state_mut().library.save().expect("rien à écrire");
    assert!(!a.exists() && !b.exists());
    // toujours débranché
    h.cliquer_label("Réessayer");
    h.run_steps(2);
    assert!(h.state().library.location().is_none());
    // rebranché
    std::fs::rename(base.join("debranche"), base.join("disque")).expect("rebranché");
    h.cliquer_label("Réessayer");
    h.run_steps(2);
    assert_eq!(h.state().library.location(), Some(b.as_path()));
    assert_eq!(h.state().library.drones().len(), 1);
    assert!(h.query_by_label("Bibliothèque introuvable").is_none());
}

#[test]
fn le_deplacement_attend_la_fin_d_une_recuperation_usb() {
    let base = dossier("usb");
    let a = base.join("bibliotheque");
    let b = base.join("disque");
    std::fs::create_dir_all(&b).expect("vide");
    let mut lib = bibliotheque(&a, "JeNo");
    let mut page = LibraryPage::default();
    page.default_dir = Some(a.clone());
    page.choose(&lib, &b);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(
            |ui, (page, lib): &mut (LibraryPage, Library)| {
                page.ui(ui, lib, &otb_gui::theme::CLAIR, Some(USB_BUSY));
            },
            (page, std::mem::replace(&mut lib, Library::in_memory())),
        );
    h.run_steps(2);
    assert!(h.query_by_label(USB_BUSY).is_some());
    h.cliquer_label("Déplacer");
    h.run_steps(2);
    assert!(!h.state().0.moving(), "déplacement lancé pendant une récupération USB");
    assert!(!b.join(place::MOVING).exists());
    // Contre-preuve : `add_enabled(true, bouton)` → « déplacement lancé pendant une récupération USB ».
}

#[test]
fn un_autre_poste_recent_est_signale_au_lancement() {
    let base = dossier("autre-poste");
    let a = base.join("bibliotheque");
    let mut h = banc(app(&a, bibliotheque(&a, "JeNo"), None));
    let message = "Cette bibliothèque est peut-être ouverte sur le poste « portable »";
    h.state_mut().show_library_problem(None, Some(message.into()));
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Library);
    assert!(h.query_by_label(message).is_some());
    h.cliquer_label("Compris");
    h.run_steps(2);
    assert!(h.query_by_label(message).is_none());
    h.cliquer_label("Retour");
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Dashboard);
}
