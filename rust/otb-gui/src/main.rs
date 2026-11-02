// Pas de fenêtre de console avec l'application sous Windows. Sans console, `stdout` et `stderr` n'existent plus :
// le code de l'interface n'imprime rien (`println!` planterait).
#![cfg_attr(windows, windows_subsystem = "windows")]

use otb_gui::app::App;
use otb_gui::guide::{Cards, cards_path};
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
    let mut viewport = egui::ViewportBuilder::default();
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    eframe::run_native(
        "OpenTuneBox",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| {
            otb_gui::app::setup(&cc.egui_ctx);
            let mut guide = GuidePage::new(Cards::load(cards_path()));
            if let Some(storage) = cc.storage {
                guide.restore(storage);
            }
            let mut app = App::with_guide(otb_gui::tools::all_pages(), guide);
            if let Some(storage) = cc.storage {
                app.restore(storage);
            }
            Ok(Box::new(app))
        }),
    )
}
