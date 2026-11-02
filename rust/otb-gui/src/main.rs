use otb_gui::app::App;
use otb_gui::guide::{Cards, cards_path};
use otb_gui::guide_page::GuidePage;

fn main() -> eframe::Result {
    eframe::run_native(
        "OpenTuneBox",
        eframe::NativeOptions::default(),
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
