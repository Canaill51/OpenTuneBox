//! Récupération par USB, côté interface (lot 32b, D-074) : le fil de travail sur une carte simulée, le rangement dans
//! la bibliothèque, le choix du drone, puis l'écran entier dans la fenêtre. Aucune carte en CI : la carte simulée
//! d'`otb-usb` répond comme Betaflight (voir `docs/spec-usb.md` pour les essais sur une vraie carte).
//!
//! ⚠️ Chaque refus d'effacer est un test : c'est la promesse du lot 32 (« un log perdu est un vol à refaire »).

mod support;
use support::Cliquer;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use otb_gui::app::{App, Screen};
use otb_gui::theme::CLAIR;
use otb_gui::usb::{
    self, AnyLink, Connection, DroneChoice, Event, Ports, Request, apply_choice, default_choice, register_copy,
};
use otb_gui::usb_page::{Phase, TITLE, UsbPage, before_copy_text};
use otb_store::Library;
use otb_store::model::Drone;
use otb_usb::Link;
use otb_usb::client::Command;
use otb_usb::erase::ErasePolling;
use otb_usb::sim::SimBoard;

fn jeno() -> Vec<u8> {
    std::fs::read(format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"))).expect("log d'essai")
}

/// Deux vols à la suite, comme la mémoire d'une carte après deux batteries.
fn deux_vols() -> Vec<u8> {
    let mut f = jeno();
    f.extend(jeno());
    f
}

fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-gui-usb-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn vite() -> ErasePolling {
    ErasePolling {
        interval: Duration::from_millis(1),
        limit: Some(Duration::from_secs(5)),
    }
}

/// Une carte simulée partagée : le test garde la main dessus pour compter ce qu'elle a reçu.
#[derive(Clone)]
struct Partagee(Arc<Mutex<SimBoard>>);

impl Link for Partagee {
    fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.0.lock().unwrap().send(bytes)
    }
    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> std::io::Result<usize> {
        self.0.lock().unwrap().recv(buf, timeout)
    }
}

fn carte(flash: Vec<u8>) -> Arc<Mutex<SimBoard>> {
    Arc::new(Mutex::new(SimBoard::new(flash)))
}

fn ports(board: &Arc<Mutex<SimBoard>>) -> Ports {
    let b = board.clone();
    Ports {
        list: Arc::new(|| vec![PathBuf::from("/dev/simule0")]),
        open: Arc::new(move |_: &Path| Ok(AnyLink(Box::new(Partagee(b.clone()))))),
    }
}

fn effacements(board: &Arc<Mutex<SimBoard>>) -> usize {
    board.lock().unwrap().count(Command::DataflashErase)
}

/// Attend la prochaine nouvelle du fil qui n'est pas une progression.
fn attendre(c: &Connection) -> Event {
    let debut = Instant::now();
    loop {
        for e in c.poll() {
            if !matches!(e, Event::Progress { .. } | Event::Erasing(_)) {
                return e;
            }
        }
        assert!(debut.elapsed() < Duration::from_secs(30), "le fil ne répond pas");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn connecte(board: &Arc<Mutex<SimBoard>>) -> Connection {
    let c = Connection::open(&ports(board), "/dev/simule0".into(), vite(), egui::Context::default());
    match attendre(&c) {
        Event::Connected(b) => assert_eq!(b.identity.name, "jeno5"),
        e => panic!("connexion : {e:?}"),
    }
    c
}

// ---------------------------------------------------------------- le fil de travail

#[test]
fn copier_ranger_puis_effacer() {
    let board = carte(deux_vols());
    let c = connecte(&board);
    let dir = dossier("parcours");
    let mut lib = Library::open(dir.clone());
    let path = lib.copy_path("jeno5", "2026-11-23_18-00-00").unwrap().unwrap();
    c.request(Request::Copy(path.clone()));
    let copie = match attendre(&c) {
        Event::Copied(copie) => copie,
        e => panic!("copie : {e:?}"),
    };
    assert_eq!(copie.report.path, path);
    assert_eq!(copie.sessions.len(), 2, "deux vols, deux fragments");
    assert_eq!(copie.report.bytes, deux_vols().len() as u64);
    // rangée sous le drone choisi, marquée copiée
    lib.put_drone(Drone::new("d1", "Mon drone"));
    assert!(register_copy(&mut lib, &copie, "d1", "2026-11-23"));
    assert!(
        !register_copy(&mut lib, &copie, "d1", "2026-11-23"),
        "une seconde fois : rien ne change"
    );
    let log = lib.log(&copie.fingerprint).expect("log rangé");
    assert!(log.copied);
    assert_eq!(log.drone_id, "d1");
    assert_eq!(lib.fragments_of_drone("d1").len(), 2);
    assert_eq!(lib.copies_of_drone("d1"), (1, copie.report.bytes));
    // puis l'effacement, demandé par le pilote
    assert_eq!(effacements(&board), 0, "rien d'effacé avant la demande");
    c.request(Request::Erase);
    assert_eq!(attendre(&c), Event::Erased(0));
    assert_eq!(effacements(&board), 1);
    assert!(path.exists(), "la copie reste sur le disque");
}

#[test]
fn sans_copie_verifiee_rien_n_est_efface() {
    let board = carte(deux_vols());
    let c = connecte(&board);
    c.request(Request::Erase);
    match attendre(&c) {
        Event::Failed(e) => assert!(e.contains("Aucune copie vérifiée"), "{e}"),
        e => panic!("{e:?}"),
    }
    assert_eq!(effacements(&board), 0);
    // Contre-preuve : sans le `let Some(v) = verified.take() else` de `usb::worker`, ce test ne compile plus — il n'y
    // a pas d'autre chemin vers un permis d'effacer (`VerifiedCopy::confirm`).
}

#[test]
fn une_copie_non_verifiee_ne_permet_pas_d_effacer() {
    // l'en-tête seul d'un log : copié, mais sans vol lisible
    let d = jeno();
    let fin = d.windows(2).position(|w| w == b"\nI").unwrap_or(d.len() / 10) + 1;
    let board = carte(d[..fin].to_vec());
    let c = connecte(&board);
    let path = dossier("non-verifiee").join("copie.bbl");
    c.request(Request::Copy(path));
    match attendre(&c) {
        Event::NotVerified(e) => assert!(e.contains("Effacement refusé"), "{e}"),
        e => panic!("{e:?}"),
    }
    c.request(Request::Erase);
    assert!(matches!(attendre(&c), Event::Failed(_)));
    assert_eq!(effacements(&board), 0);
}

#[test]
fn drone_arme_rien_n_est_lu() {
    let board = carte(deux_vols());
    board.lock().unwrap().armed = true;
    let c = connecte(&board);
    let lectures = board.lock().unwrap().count(Command::DataflashRead);
    c.request(Request::Copy(dossier("arme").join("copie.bbl")));
    assert_eq!(attendre(&c), Event::Failed(usb::ARMED.into()));
    assert_eq!(board.lock().unwrap().count(Command::DataflashRead), lectures);
}

#[test]
fn une_copie_arretee_ne_garde_rien_et_n_efface_rien() {
    let board = carte(deux_vols());
    let c = connecte(&board);
    let path = dossier("arretee").join("copie.bbl");
    c.request(Request::Copy(path.clone()));
    c.cancel();
    match attendre(&c) {
        Event::Failed(e) => assert!(e.contains("arrêtée"), "{e}"),
        e => panic!("{e:?}"),
    }
    assert!(!path.exists(), "rien n'est gardé d'une copie arrêtée");
    c.request(Request::Erase);
    assert!(matches!(attendre(&c), Event::Failed(_)));
    assert_eq!(effacements(&board), 0);
}

#[test]
fn la_configuration_se_lit_et_la_carte_redemarre() {
    let board = carte(deux_vols());
    let c = connecte(&board);
    c.request(Request::Config);
    match attendre(&c) {
        Event::Config(t) => assert!(t.contains("set gyro_lpf1_static_hz = 0"), "{t}"),
        e => panic!("{e:?}"),
    }
    assert!(board.lock().unwrap().rebooted);
}

// ---------------------------------------------------------------- le drone

#[test]
fn le_drone_se_reconnait_par_sa_carte_sinon_on_propose() {
    let board = carte(Vec::new());
    let c = connecte(&board);
    c.request(Request::Refresh);
    let Event::Connected(b) = attendre(&c) else { panic!() };
    let id = b.identity;
    let uid = usb::uid_of(&id);
    assert_eq!(uid.len(), 24, "96 bits en hexadécimal");

    let mut lib = Library::in_memory();
    // inconnu : un nouveau drone du nom de la carte
    assert_eq!(default_choice(&lib, &id), (DroneChoice::New("jeno5".into()), false));
    // un drone du même nom : proposé, pas imposé
    lib.put_drone(Drone::new("j", "jeno5"));
    assert_eq!(default_choice(&lib, &id), (DroneChoice::Existing("j".into()), false));
    // le nom pris : refusé pour un nouveau drone ; un nom vide aussi
    assert!(apply_choice(&mut lib, &DroneChoice::New("jeno5".into()), &id, "2026-11-23").is_err());
    assert!(apply_choice(&mut lib, &DroneChoice::New("  ".into()), &id, "2026-11-23").is_err());
    // choisi : l'identifiant et la carte se rangent sur le drone, qui est ensuite reconnu même renommé
    let r = apply_choice(&mut lib, &DroneChoice::Existing("j".into()), &id, "2026-11-23").unwrap();
    assert_eq!(r, "j");
    let mut d = lib.drone("j").unwrap().clone();
    assert_eq!(d.uid, uid);
    assert_eq!(d.board, "STM32F7X2 (S7X2)");
    assert_eq!(d.firmware, "Betaflight 2025.12.0");
    let mut ancien = id.clone();
    ancien.version = (4, 5, 1);
    assert_eq!(usb::firmware_text(&ancien), "Betaflight 4.5.1");
    d.name = "Renommé".into();
    lib.put_drone(d);
    assert_eq!(default_choice(&lib, &id), (DroneChoice::Existing("j".into()), true));
}

#[test]
fn un_identifiant_nul_ne_reconnait_aucun_drone() {
    let board = carte(Vec::new());
    board.lock().unwrap().chip_id = [0, 0, 0];
    let c = connecte(&board);
    c.request(Request::Refresh);
    let Event::Connected(b) = attendre(&c) else { panic!() };
    assert_eq!(usb::uid_of(&b.identity), "");
    let mut lib = Library::in_memory();
    let mut d = Drone::new("vide", "vide");
    d.uid = String::new();
    lib.put_drone(d);
    assert!(
        !default_choice(&lib, &b.identity).1,
        "un identifiant nul ne prouve rien"
    );
}

#[test]
fn les_textes_de_taille_et_de_duree() {
    assert_eq!(usb::size_text(8_187_904), "7,8 Mo");
    assert_eq!(usb::duration_text(8_187_904, usb::MSP_MO_PER_MIN), "≈ 3 min");
    assert_eq!(usb::duration_text(1, usb::MSP_MO_PER_MIN), "≈ 1 min");
    assert_eq!(
        before_copy_text(16 * 1_048_576),
        "16,0 Mo à copier, ≈ 5 min au débit de l'USB (MSP)."
    );
    assert!(usb::stamp_now().len() == 19 && !usb::stamp_now().contains(':'));
}

// ---------------------------------------------------------------- l'écran dans la fenêtre

fn banc(board: &Arc<Mutex<SimBoard>>, dir: &Path) -> Harness<'static, App> {
    let mut app = App::new(otb_gui::tools::all_pages());
    app.theme = CLAIR;
    app.library = Library::open(dir.to_path_buf());
    app.usb = UsbPage::new(ports(board));
    app.usb.polling = vite();
    app.screen = Screen::Dashboard;
    let mut h = Harness::builder()
        .with_size(egui::vec2(1400.0, 950.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app);
    h.run_steps(2);
    h
}

/// Dessine jusqu'à ce que `fini` soit vrai.
fn jusqu_a(h: &mut Harness<'static, App>, fini: impl Fn(&App) -> bool) {
    let debut = Instant::now();
    while !fini(h.state()) {
        h.run_steps(1);
        assert!(
            debut.elapsed() < Duration::from_secs(20),
            "jamais arrivé ; écran : {:?} / {:?}",
            h.state().usb.error,
            h.state().usb.message
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(3);
}

#[test]
fn l_ecran_copie_range_puis_efface_en_deux_temps() {
    let board = carte(deux_vols());
    let dir = dossier("ecran");
    let mut h = banc(&board, &dir);
    h.cliquer_label(TITLE);
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Usb);
    h.get_by_label("Port : /dev/simule0");
    h.cliquer_label("Se connecter");
    h.run_steps(2);
    jusqu_a(&mut h, |a| a.usb.board.is_some());
    h.get_by_label("jeno5");
    h.get_by_label_contains("Première connexion de cette carte");
    h.get_by_label(&before_copy_text(deux_vols().len() as u64));
    h.cliquer_label("Copier les vols");
    h.run_steps(2);
    jusqu_a(&mut h, |a| {
        matches!(
            &a.usb.phase,
            Phase::Copied {
                registered: Some(_),
                ..
            }
        )
    });
    // rangée sous un nouveau drone qui porte l'identifiant de la carte, et la bibliothèque écrite sur le disque
    let relue = Library::open(dir.clone());
    let d = relue.drone_by_name("jeno5").expect("drone créé");
    assert_eq!(d.uid.len(), 24);
    assert_eq!(relue.copies_of_drone(&d.id).0, 1);
    h.get_by_label_contains("2 vols copiés et vérifiés");
    h.get_by_label("Ouvrir les vols");
    // l'effacement : un premier clic ne fait que demander
    h.cliquer_label("Effacer la mémoire du drone…");
    h.run_steps(2);
    assert_eq!(effacements(&board), 0);
    h.get_by_label(otb_gui::usb_page::ERASE_WARNING);
    h.cliquer_label("Garder");
    h.run_steps(2);
    assert_eq!(effacements(&board), 0);
    h.cliquer_label("Effacer la mémoire du drone…");
    h.run_steps(2);
    h.cliquer_label("Effacer définitivement");
    h.run_steps(2);
    jusqu_a(&mut h, |a| matches!(a.usb.phase, Phase::Erased(_)));
    assert_eq!(effacements(&board), 1);
    h.get_by_label("Mémoire effacée ; place occupée relue : 0,0 Mo.");
    // la carte est maintenant reconnue : plus de question
    h.cliquer_label("Relire l'état");
    h.run_steps(2);
    jusqu_a(&mut h, |a| !a.usb.busy());
    h.get_by_label_contains("Reconnu par l'identifiant de sa carte : jeno5");
}

#[test]
fn l_ecran_refuse_tout_drone_arme() {
    let board = carte(deux_vols());
    board.lock().unwrap().armed = true;
    let dir = dossier("ecran-arme");
    let mut h = banc(&board, &dir);
    h.cliquer_label(TITLE);
    h.run_steps(2);
    h.cliquer_label("Se connecter");
    h.run_steps(2);
    jusqu_a(&mut h, |a| a.usb.board.is_some());
    h.get_by_label(usb::ARMED);
    assert!(h.get_by_label("Copier les vols").accesskit_node().is_disabled());
    assert!(
        h.get_by_label("Sauvegarder la configuration")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn la_configuration_se_garde_seulement_si_elle_change() {
    let board = carte(Vec::new());
    let dir = dossier("ecran-config");
    let mut h = banc(&board, &dir);
    h.cliquer_label(TITLE);
    h.run_steps(2);
    for attendu in ["Configuration sauvegardée", "Configuration identique"] {
        h.cliquer_label("Se connecter");
        h.run_steps(2);
        jusqu_a(&mut h, |a| a.usb.board.is_some());
        h.cliquer_label("Sauvegarder la configuration");
        h.run_steps(2);
        jusqu_a(&mut h, |a| a.usb.board.is_none());
        h.get_by_label_contains(attendu);
        // la carte a redémarré : une carte neuve, même identifiant (la simulée resterait en mode CLI)
        *board.lock().unwrap() = SimBoard::new(Vec::new());
    }
    let lib = &h.state().library;
    let d = lib.drone_by_name("jeno5").expect("drone créé au premier diff all");
    assert_eq!(
        lib.configs_of_drone(&d.id).len(),
        1,
        "la seconde, identique, n'est pas gardée"
    );
}

#[test]
fn un_log_copie_rouvert_reste_sous_son_drone() {
    let board = carte(deux_vols());
    let dir = dossier("rouvert");
    let mut lib = Library::open(dir.clone());
    let path = lib.copy_path("Autre", "x").unwrap().unwrap();
    let c = connecte(&board);
    c.request(Request::Copy(path.clone()));
    let Event::Copied(copie) = attendre(&c) else { panic!() };
    lib.put_drone(Drone::new("autre", "Autre"));
    register_copy(&mut lib, &copie, "autre", "2026-11-23");
    lib.save().unwrap();
    let mut h = banc(&board, &dir);
    h.state_mut().open(vec![path]);
    jusqu_a(&mut h, |a| !a.state.logs.is_empty());
    let lib = &h.state().library;
    assert_eq!(lib.log(&copie.fingerprint).unwrap().drone_id, "autre");
    assert!(
        lib.drone_by_name("JeNo Ced").is_none(),
        "le nom de machine du log ne crée pas d'autre drone"
    );
    // Contre-preuve : sans la recherche de l'empreinte dans `App::remember`, un drone « JeNo Ced » apparaît.
}

#[test]
fn supprimer_un_drone_garde_ses_copies_sauf_case_cochee() {
    for cocher in [false, true] {
        let board = carte(deux_vols());
        let dir = dossier(if cocher { "w075-oui" } else { "w075-non" });
        let mut lib = Library::open(dir.clone());
        let path = lib.copy_path("Autre", "x").unwrap().unwrap();
        let c = connecte(&board);
        c.request(Request::Copy(path.clone()));
        let Event::Copied(copie) = attendre(&c) else { panic!() };
        lib.put_drone(Drone::new("autre", "Autre"));
        register_copy(&mut lib, &copie, "autre", "2026-11-23");
        lib.save().unwrap();
        let mut h = banc(&board, &dir);
        let d = h.state().library.drone("autre").unwrap().clone();
        h.state_mut().open_sheet(otb_gui::drone_sheet::SheetForm::edit(&d));
        h.run_steps(3);
        h.cliquer_role(egui::accesskit::Role::Button, "Supprimer…");
        h.run_steps(2);
        let case = otb_gui::drone_sheet::copies_text(1, copie.report.bytes);
        if cocher {
            h.cliquer_label(&case);
            h.run_steps(2);
        } else {
            h.get_by_label(&case);
        }
        h.cliquer_label("Supprimer définitivement");
        h.run_steps(3);
        assert!(h.state().library.drone("autre").is_none());
        assert_eq!(path.exists(), !cocher, "case cochée : {cocher}");
    }
}

#[test]
fn sans_rangement_dans_la_bibliotheque_l_effacement_n_est_pas_propose() {
    let board = carte(deux_vols());
    let dir = dossier("ecran-non-range");
    let mut h = banc(&board, &dir);
    h.cliquer_label(TITLE);
    h.run_steps(2);
    h.cliquer_label("Se connecter");
    jusqu_a(&mut h, |a| a.usb.board.is_some());
    h.cliquer_label("Copier les vols");
    jusqu_a(&mut h, |a| {
        matches!(
            &a.usb.phase,
            Phase::Copied {
                registered: Some(_),
                ..
            }
        )
    });
    // la bibliothèque n'a pas pu s'écrire : la copie n'est connue que d'un fichier, on n'efface pas
    h.state_mut().usb.registered(Err("disque plein".into()));
    h.run_steps(3);
    h.get_by_label_contains("Rangement impossible : disque plein");
    assert!(h.query_by_label("Effacer la mémoire du drone…").is_none());
    assert_eq!(effacements(&board), 0);
}
