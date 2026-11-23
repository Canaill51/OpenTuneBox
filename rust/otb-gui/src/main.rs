// Pas de fenêtre de console avec l'application sous Windows. Sans console, `stdout` et `stderr` n'existent plus :
// le code de l'interface n'imprime rien (`println!` planterait).
#![cfg_attr(windows, windows_subsystem = "windows")]

use otb_gui::app::App;
use otb_gui::guide::cards_path;
use otb_gui::guide_page::GuidePage;

/// Icône de la fenêtre (barre des tâches, titre) : le PNG des paquets Linux, 256 × 256.
fn window_icon() -> Option<egui::IconData> {
    let png = include_bytes!("../../../packaging/linux/opentunebox.png");
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result {
    // `app_id` : sous Linux (Wayland), le bureau ne lit pas l'icône de la fenêtre, il la cherche dans le fichier `.desktop` dont le
    // **nom** est cet identifiant, majuscules comprises (`OpenTuneBox.desktop`, écrit par `packaging/linux/*.sh` ; test
    // `contracts.rs::le_fichier_desktop_porte_le_nom_de_l_app_id`, L-076).
    // Taille au **premier** lancement : sans consigne, winit ouvre une fenêtre de 800 × 600, trop petite pour six graphiques et le
    // panneau de réglages (la version Python ouvrait à 95 % de l'écran, `gui/geometry.py:fit_to_screen`). La fenêtre s'ouvre donc
    // agrandie, et `with_inner_size` donne une taille correcte quand l'utilisateur la restaure. Les lancements suivants reprennent la
    // taille laissée par l'utilisateur : eframe l'enregistre lui-même (clé `window`) et ses réglages l'emportent sur ceux-ci (L-089).
    let mut viewport = egui::ViewportBuilder::default()
        .with_app_id("OpenTuneBox")
        .with_inner_size([1280.0, 800.0])
        .with_maximized(true);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    eframe::run_native(
        &otb_gui::app::window_title(),
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| {
            otb_gui::app::setup(&cc.egui_ctx);
            // seul le lancement réel ouvre la bibliothèque du poste : les bancs de test gardent la leur en mémoire
            let mut library = otb_store::Library::open(otb_store::dir());
            for p in library.problems() {
                eprintln!("bibliothèque : {p}");
            }
            // les fiches de l'ancien fichier du Guide entrent dans la bibliothèque (D-068)
            let reprise = otb_gui::drone_sheet::import_old_file(&mut library, &cards_path());
            // le Guide reçoit ses fiches **avant** de retrouver la fiche choisie la dernière fois (par son nom)
            let mut guide = GuidePage::new();
            guide.refresh(otb_gui::drone_sheet::cards_of(&library));
            if let Some(storage) = cc.storage {
                guide.restore(storage);
            }
            let mut app = App::from_launch(otb_gui::tools::all_pages(), guide, cc.storage, std::env::args_os());
            app.library = library;
            if let Some(m) = reprise {
                eprintln!("{m}");
                app.status = m;
            }
            // les pastilles de la dernière fois, rouvertes en arrière-plan (D-066)
            app.restore_analysis();
            Ok(Box::new(app))
        }),
    )
}
