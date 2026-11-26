//! Essayer l'écran « Récupérer par USB » à la main, sans drone : la vraie fenêtre, branchée sur une carte simulée
//! d'`otb-usb` (D-085). La bibliothèque est un dossier temporaire neuf : la vraie n'est jamais touchée.
//!
//! ```sh
//! cargo run --release -p otb-gui --example usb_simule -- vieux        # Betaflight 3.4 (API 1.39) : USB série
//! cargo run --release -p otb-gui --example usb_simule -- sans-disque  # carte récente compilée sans mode disque
//! cargo run --release -p otb-gui --example usb_simule -- sd-vieux     # carte SD, Betaflight 3.4 : lecteur de cartes
//! cargo run --release -p otb-gui --example usb_simule -- arme         # carte récente, drone armé
//! ```
//!
//! La carte simulée garde sa mémoire tant que la fenêtre est ouverte (effacer la vide vraiment).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use otb_gui::app::{App, Screen};
use otb_gui::usb::{AnyLink, Ports};
use otb_gui::usb_page::UsbPage;
use otb_store::Library;
use otb_usb::Link;
use otb_usb::sim::SimBoard;

/// La carte simulée, partagée entre les ouvertures du port.
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

fn deux_vols() -> Vec<u8> {
    let f =
        std::fs::read(format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"))).expect("log d'essai");
    let mut v = f.clone();
    v.extend(f);
    v
}

fn carte(cas: &str) -> SimBoard {
    let mut b = SimBoard::new(deux_vols());
    match cas {
        "vieux" => {
            b.api = (1, 39);
            b.version = (3, 4, 0);
        }
        "sans-disque" => b.msc = false,
        "sd-vieux" => {
            b.flash.clear();
            b.has_flash = false;
            b.sd_state = Some(4);
            b.api = (1, 39);
            b.version = (3, 4, 0);
        }
        "arme" => b.armed = true,
        _ => {
            eprintln!("cas inconnu « {cas} » : vieux, sans-disque, sd-vieux ou arme");
            std::process::exit(2);
        }
    }
    // une lecture de port réelle rend par morceaux : la barre de progression a le temps de se voir
    b.max_block = 256;
    b
}

fn main() -> eframe::Result {
    let cas = std::env::args().nth(1).unwrap_or_else(|| "vieux".into());
    let board = Arc::new(Mutex::new(carte(&cas)));
    let lib: PathBuf = std::env::temp_dir().join(format!("otb-usb-simule-{cas}-{}", std::process::id()));
    std::fs::create_dir_all(&lib).expect("dossier temporaire");
    println!("Carte simulée « {cas} » ; bibliothèque temporaire : {}", lib.display());
    let b = board.clone();
    let ports = Ports {
        list: Arc::new(|| vec![PathBuf::from("/dev/simule0")]),
        open: Arc::new(move |_: &Path| Ok(AnyLink(Box::new(Partagee(b.clone()))))),
    };
    eframe::run_native(
        &format!("OpenTuneBox — carte simulée « {cas} »"),
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 900.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            otb_gui::app::setup(&cc.egui_ctx);
            let mut app = App::new(otb_gui::tools::all_pages());
            app.library = Library::open(lib);
            app.usb = UsbPage::new(ports);
            app.screen = Screen::Dashboard;
            Ok(Box::new(app))
        }),
    )
}
