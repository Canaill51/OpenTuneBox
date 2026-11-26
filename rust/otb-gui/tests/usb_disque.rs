//! Récupération par le mode disque, côté interface (lot 32c, D-075) : la mémoire copiée par le disque puis effacée
//! après rebranchement, la carte SD du drone, la carte SD d'un lecteur, le disque choisi à la main. Aucune carte en
//! CI : la carte simulée d'`otb-usb`, et des disques fabriqués dans un dossier temporaire, « montés » par le test.
//!
//! ⚠️ Chaque refus d'effacer est un test : c'est la promesse du lot 32 (« un log perdu est un vol à refaire »).

mod support;
use support::Cliquer;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_gui::app::{App, Screen};
use otb_gui::theme::CLAIR;
use otb_gui::usb::{AnyLink, Ports, disk_plan};
use otb_gui::usb_page::{DISK_NOTE, EJECT, Phase, READER_HINT, RECOVER, REPLUG, SD_ERASE_WARNING, TITLE, UsbPage};
use otb_store::Library;
use otb_store::model::LogEntry;
use otb_usb::Link;
use otb_usb::client::Command;
use otb_usb::erase::ErasePolling;
use otb_usb::msc::{FLASH_ALL, inspect};
use otb_usb::sim::SimBoard;

fn jeno() -> Vec<u8> {
    std::fs::read(format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"))).expect("log d'essai")
}

fn deux_vols() -> Vec<u8> {
    let mut f = jeno();
    f.extend(jeno());
    f
}

/// L'en-tête seul d'un log : un vol coupé avant sa première trame.
fn en_tete_seul() -> Vec<u8> {
    let d = jeno();
    let mut fin = 0;
    for line in d.split_inclusive(|&b| b == b'\n') {
        if !line.starts_with(b"H ") {
            break;
        }
        fin += line.len();
    }
    d[..fin].to_vec()
}

fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-gui-disque-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Une carte simulée partagée : le test garde la main dessus, et la remplace pour « rebrancher ».
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

/// Le banc : une carte (ou aucune), son port qui apparaît et disparaît, les volumes « montés » par le test.
struct Banc {
    h: Harness<'static, App>,
    board: Arc<Mutex<SimBoard>>,
    port: Arc<AtomicBool>,
    volumes: Arc<Mutex<Vec<PathBuf>>>,
    picked: Arc<Mutex<Option<PathBuf>>>,
    lib: PathBuf,
}

fn banc(board: SimBoard, port: bool, nom: &str) -> Banc {
    let board = Arc::new(Mutex::new(board));
    let present = Arc::new(AtomicBool::new(port));
    let volumes = Arc::new(Mutex::new(Vec::new()));
    let picked = Arc::new(Mutex::new(None));
    let lib = dossier(&format!("{nom}-biblio"));
    let (b, p) = (board.clone(), present.clone());
    let ports = Ports {
        list: Arc::new(move || {
            if p.load(Ordering::Relaxed) {
                vec![PathBuf::from("/dev/simule0")]
            } else {
                Vec::new()
            }
        }),
        open: Arc::new(move |_: &Path| Ok(AnyLink(Box::new(Partagee(b.clone()))))),
    };
    let mut app = App::new(otb_gui::tools::all_pages());
    app.theme = CLAIR;
    app.library = Library::open(lib.clone());
    app.usb = UsbPage::new(ports);
    app.usb.polling = ErasePolling {
        interval: Duration::from_millis(1),
        limit: Some(Duration::from_secs(5)),
    };
    let v = volumes.clone();
    app.usb.volumes = Arc::new(move || v.lock().unwrap().clone());
    let k = picked.clone();
    app.usb.pick_folder = Box::new(move || k.lock().unwrap().clone());
    app.screen = Screen::Dashboard;
    let mut h = Harness::builder()
        .with_size(egui::vec2(1400.0, 1100.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app);
    h.run_steps(2);
    Banc {
        h,
        board,
        port: present,
        volumes,
        picked,
        lib,
    }
}

impl Banc {
    /// Dessine jusqu'à ce que `fini` soit vrai.
    fn jusqu_a(&mut self, fini: impl Fn(&App) -> bool) {
        let debut = Instant::now();
        while !fini(self.h.state()) {
            self.h.run_steps(1);
            assert!(
                debut.elapsed() < Duration::from_secs(20),
                "jamais arrivé ; phase {:?} ; écran : {:?} / {:?}",
                self.h.state().usb.phase,
                self.h.state().usb.error,
                self.h.state().usb.message
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        self.h.run_steps(3);
    }

    fn connecter(&mut self) {
        self.h.cliquer_label(TITLE);
        self.h.run_steps(2);
        self.h.cliquer_label("Se connecter");
        self.h.run_steps(2);
        self.jusqu_a(|a| a.usb.board.is_some());
    }

    /// Le système monte un disque.
    fn monter(&self, root: &Path) {
        self.volumes.lock().unwrap().push(root.to_path_buf());
    }

    fn effacements(&self) -> usize {
        self.board.lock().unwrap().count(Command::DataflashErase)
    }
}

/// Le disque d'une carte à mémoire interne en mode stockage de masse.
fn disque_memoire(nom: &str, flash: &[u8]) -> PathBuf {
    let root = dossier(nom).join("BETAFLT");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(FLASH_ALL), flash).unwrap();
    root
}

/// Une carte SD : `LOGS/LOG00001.BFL`… et un fichier qui n'est pas un log.
fn carte_sd(nom: &str, logs: &[Vec<u8>]) -> PathBuf {
    let root = dossier(nom).join("SD");
    std::fs::create_dir_all(root.join("LOGS")).unwrap();
    for (k, l) in logs.iter().enumerate() {
        std::fs::write(root.join("LOGS").join(format!("LOG{:05}.BFL", k + 1)), l).unwrap();
    }
    std::fs::write(root.join("LOGS").join("NOTES.TXT"), b"pas un log").unwrap();
    root
}

fn disk_copied(a: &App) -> bool {
    matches!(
        a.usb.phase,
        Phase::DiskCopied {
            registered: Some(_),
            ..
        }
    )
}

#[test]
fn memoire_copiee_par_le_disque_puis_effacee_apres_rebranchement() {
    let flash = deux_vols();
    let mut b = banc(SimBoard::new(flash.clone()), true, "memoire");
    b.connecter();
    b.h.get_by_label(DISK_NOTE);
    assert_eq!(b.h.query_all_by_label(RECOVER).count(), 1, "un seul bouton de copie");
    assert!(b.h.query_by_label("Copier les vols").is_none());
    b.h.cliquer_label(RECOVER);
    b.h.run_steps(2);
    b.jusqu_a(|a| a.usb.phase == Phase::WaitingDisk);
    assert_eq!(
        b.board.lock().unwrap().reboots,
        vec![2],
        "redémarrage en stockage de masse seulement"
    );
    // la carte n'a plus de port série ; le système monte son disque
    b.port.store(false, Ordering::Relaxed);
    b.monter(&disque_memoire("memoire-disque", &flash));
    b.jusqu_a(disk_copied);
    let relue = Library::open(b.lib.clone());
    let d = relue
        .drone_by_name("jeno5")
        .expect("drone créé à la demande du mode disque");
    assert_eq!(d.uid.len(), 24);
    assert_eq!(relue.copies_of_drone(&d.id).0, 1);
    b.h.get_by_label_contains("2 vols copiés et vérifiés");
    b.h.get_by_label("Ouvrir les vols");
    b.h.get_by_label(REPLUG);
    assert_eq!(b.effacements(), 0);
    assert!(b.h.query_by_label("Effacer la mémoire du drone…").is_none());
    // rebranché : même carte, son port revient, OpenTuneBox s'y reconnecte seul
    *b.board.lock().unwrap() = SimBoard::new(flash);
    b.port.store(true, Ordering::Relaxed);
    b.jusqu_a(|a| a.usb.board.is_some());
    b.h.get_by_label("Copie faite par le mode disque, vérifiée et rangée.");
    b.h.cliquer_label("Effacer la mémoire du drone…");
    b.h.run_steps(2);
    assert_eq!(b.effacements(), 0, "un premier clic ne fait que demander");
    b.h.cliquer_label("Effacer définitivement");
    b.h.run_steps(2);
    b.jusqu_a(|a| matches!(a.usb.phase, Phase::Erased(_)));
    assert_eq!(b.effacements(), 1);
    assert!(b.board.lock().unwrap().flash.is_empty());
    assert!(!b.h.state().usb.held.waiting(), "la copie ne sert qu'une fois");
}

/// ⚠️ Rebranché, ce n'est pas la même carte (ou elle a enregistré depuis) : rien n'est proposé, rien n'est effacé.
#[test]
fn autre_carte_rebranchee_rien_n_est_efface() {
    let flash = deux_vols();
    for (cas, carte) in [
        ("autre identifiant", {
            let mut s = SimBoard::new(flash.clone());
            s.chip_id = [1, 2, 3];
            s
        }),
        ("vol enregistré depuis", {
            let mut f = flash.clone();
            f.extend(jeno());
            SimBoard::new(f)
        }),
    ] {
        let mut b = banc(SimBoard::new(flash.clone()), true, &format!("autre-{}", cas.len()));
        b.connecter();
        b.h.cliquer_label(RECOVER);
        b.h.run_steps(2);
        b.jusqu_a(|a| a.usb.phase == Phase::WaitingDisk);
        b.port.store(false, Ordering::Relaxed);
        b.monter(&disque_memoire(&format!("autre-disque-{}", cas.len()), &flash));
        b.jusqu_a(disk_copied);
        *b.board.lock().unwrap() = carte;
        b.port.store(true, Ordering::Relaxed);
        b.jusqu_a(|a| a.usb.board.is_some());
        b.h.get_by_label_contains("Ce n'est pas la carte copiée par le disque");
        assert!(b.h.query_by_label("Effacer la mémoire du drone…").is_none(), "{cas}");
        assert!(
            b.h.state().usb.held.waiting(),
            "{cas} : la copie attend toujours sa carte"
        );
        assert_eq!(b.effacements(), 0, "{cas}");
    }
}

#[test]
fn carte_sd_du_drone_copiee_puis_supprimee_en_deux_temps() {
    let mut s = SimBoard::new(Vec::new());
    s.has_flash = false;
    s.sd_state = Some(4);
    let mut b = banc(s, true, "sd-drone");
    b.connecter();
    b.h.get_by_label_contains("ses logs sont sur carte SD");
    b.h.cliquer_label(RECOVER);
    b.h.run_steps(2);
    b.jusqu_a(|a| a.usb.phase == Phase::WaitingDisk);
    b.port.store(false, Ordering::Relaxed);
    let sd = carte_sd("sd-drone-carte", &[jeno(), en_tete_seul()]);
    b.monter(&sd);
    b.jusqu_a(disk_copied);
    b.h.get_by_label_contains("1 vol copié et vérifié");
    b.h.get_by_label_contains("Refusé : LOG00002.BFL");
    let relue = Library::open(b.lib.clone());
    let d = relue.drone_by_name("jeno5").unwrap();
    assert_eq!(relue.copies_of_drone(&d.id).0, 1);
    // deux clics
    b.h.cliquer_label("Supprimer 1 log de la carte SD…");
    b.h.run_steps(2);
    b.h.get_by_label(SD_ERASE_WARNING);
    assert!(sd.join("LOGS/LOG00001.BFL").exists());
    b.h.cliquer_label("Supprimer définitivement");
    b.h.run_steps(2);
    b.jusqu_a(|a| matches!(a.usb.phase, Phase::SdErased { .. }));
    b.h.get_by_label("1 log supprimé de la carte SD.");
    b.h.get_by_label(EJECT);
    assert!(!sd.join("LOGS/LOG00001.BFL").exists());
    assert!(sd.join("LOGS/LOG00002.BFL").exists(), "le log refusé reste");
    assert!(sd.join("LOGS/NOTES.TXT").exists());
    assert_eq!(b.effacements(), 0, "rien d'envoyé à la mémoire interne");
}

/// ⚠️ Copies pas rangées (bibliothèque non écrite) : la suppression n'est pas proposée.
#[test]
fn carte_sd_non_rangee_pas_de_suppression() {
    let mut b = banc(SimBoard::new(Vec::new()), false, "sd-non-rangee");
    let sd = carte_sd("sd-non-rangee-carte", &[jeno()]);
    b.monter(&sd);
    b.h.cliquer_label(TITLE);
    b.h.run_steps(2);
    b.h.cliquer_label("Copier la carte SD");
    b.h.run_steps(2);
    b.jusqu_a(disk_copied);
    b.h.state_mut().usb.registered(Err("disque plein".into()));
    b.h.run_steps(3);
    b.h.get_by_label_contains("Rangement impossible : disque plein");
    assert!(b.h.query_by_label("Supprimer 1 log de la carte SD…").is_none());
    assert!(sd.join("LOGS/LOG00001.BFL").exists());
}

#[test]
fn carte_sd_dans_un_lecteur_sans_drone_branche() {
    let mut b = banc(SimBoard::new(Vec::new()), false, "lecteur");
    let sd = carte_sd("lecteur-carte", &[jeno(), jeno()]);
    b.monter(&sd);
    b.h.cliquer_label(TITLE);
    b.h.run_steps(2);
    b.h.get_by_label_contains("Carte SD : ");
    b.h.get_by_label_contains("2 logs");
    // le nom lu dans le log est proposé pour un nouveau drone
    assert_eq!(
        b.h.get_by_label("Nom du nouveau drone").value().as_deref(),
        Some("JeNo Ced")
    );
    b.h.cliquer_label("Copier la carte SD");
    b.h.run_steps(2);
    b.jusqu_a(disk_copied);
    let relue = Library::open(b.lib.clone());
    let d = relue.drone_by_name("JeNo Ced").expect("drone créé");
    assert!(d.uid.is_empty(), "pas de carte branchée : pas d'identifiant");
    // le même contenu deux fois : une seule copie rangée, comparée à la première
    assert_eq!(relue.copies_of_drone(&d.id).0, 1);
    b.h.cliquer_label("Supprimer 2 logs de la carte SD…");
    b.h.run_steps(2);
    b.h.cliquer_label("Supprimer définitivement");
    b.h.run_steps(2);
    b.jusqu_a(|a| matches!(a.usb.phase, Phase::SdErased { .. }));
    assert!(!sd.join("LOGS/LOG00001.BFL").exists() && !sd.join("LOGS/LOG00002.BFL").exists());
    // « Terminé » : l'écran revient au départ et retrouve la carte (vide maintenant)
    b.h.cliquer_label("Terminé");
    b.h.run_steps(2);
    assert_eq!(b.h.state().usb.phase, Phase::Idle);
    b.h.get_by_label("Aucun log sur cette carte : rien à copier.");
}

#[test]
fn disque_choisi_a_la_main() {
    let flash = jeno();
    let mut b = banc(SimBoard::new(flash.clone()), true, "a-la-main");
    b.connecter();
    b.h.cliquer_label(RECOVER);
    b.h.run_steps(2);
    b.jusqu_a(|a| a.usb.phase == Phase::WaitingDisk);
    b.port.store(false, Ordering::Relaxed);
    // un dossier qui n'est pas le disque : refusé, on attend toujours
    *b.picked.lock().unwrap() = Some(dossier("a-la-main-vide"));
    b.h.cliquer_label("Choisir le dossier…");
    b.jusqu_a(|a| a.usb.error.contains("n'est pas un disque de carte Betaflight"));
    assert_eq!(b.h.state().usb.phase, Phase::WaitingDisk);
    // le bon dossier, jamais vu par le relevé des volumes (disque monté ailleurs)
    *b.picked.lock().unwrap() = Some(disque_memoire("a-la-main-disque", &flash));
    b.h.cliquer_label("Choisir le dossier…");
    b.jusqu_a(disk_copied);
    b.h.get_by_label_contains("1 vol copié et vérifié");
}

#[test]
fn attente_du_disque_annulee() {
    let mut b = banc(SimBoard::new(jeno()), true, "annulee");
    b.connecter();
    b.h.cliquer_label(RECOVER);
    b.h.run_steps(2);
    b.jusqu_a(|a| a.usb.phase == Phase::WaitingDisk);
    b.h.cliquer_label("Annuler");
    b.h.run_steps(2);
    assert_eq!(b.h.state().usb.phase, Phase::Idle);
    b.h.get_by_label_contains("Débranchez puis rebranchez le drone");
}

/// ⚠️ Le plan ne compte pour « copie déjà rangée » que les fichiers copiés par OpenTuneBox : un log ouvert depuis la
/// carte elle-même n'en est pas un double.
#[test]
fn plan_de_copie() {
    let sd = carte_sd("plan-carte", &[jeno(), jeno()]);
    let disk = inspect(&sd).unwrap();
    let dir = dossier("plan-biblio");
    let mut lib = Library::open(dir.clone());
    lib.add_log(LogEntry {
        id: "ouvert".into(),
        drone_id: "d".into(),
        name: "LOG00001.BFL".into(),
        path: sd.join("LOGS/LOG00001.BFL").display().to_string(),
        copied: false,
        bytes: 1,
        added: "2026-11-23".into(),
    });
    lib.add_log(LogEntry {
        id: "copie".into(),
        drone_id: "d".into(),
        name: "x.bbl".into(),
        path: dir.join("logs/d/x.bbl").display().to_string(),
        copied: true,
        bytes: 1,
        added: "2026-11-23".into(),
    });
    let plan = disk_plan(&lib, &disk, "Mon drone", "2026-11-23_10-00-00").unwrap();
    assert_eq!(
        plan.dests,
        vec![
            dir.join("logs/Mon_drone/2026-11-23_10-00-00_LOG00001.bbl"),
            dir.join("logs/Mon_drone/2026-11-23_10-00-00_LOG00002.bbl"),
        ]
    );
    assert_eq!(plan.existing.len(), 1);
    assert!(plan.existing.contains_key("copie"));
    // bibliothèque en mémoire : rien ne peut être copié
    assert!(disk_plan(&Library::in_memory(), &disk, "d", "s").is_err());
}

/// D-085 : la carte SD d'un firmware sans redémarrage en disque (API 1.39) ne se lit ni par le disque ni par l'USB
/// série : pas de bouton, l'écran renvoie au lecteur de cartes, et rien n'est demandé à la carte.
#[test]
fn carte_sd_d_un_vieux_firmware_renvoie_au_lecteur() {
    let mut s = SimBoard::new(Vec::new());
    s.has_flash = false;
    s.sd_state = Some(4);
    s.api = (1, 39);
    let mut b = banc(s, true, "sd-vieux");
    b.connecter();
    b.h.get_by_label(READER_HINT);
    assert!(b.h.query_by_label(RECOVER).is_none());
    assert!(b.board.lock().unwrap().reboots.is_empty());
}
