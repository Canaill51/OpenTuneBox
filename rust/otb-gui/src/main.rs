use otb_gui::app::App;

fn main() -> eframe::Result {
    eframe::run_native(
        "OpenTuneBox",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            otb_gui::app::setup(&cc.egui_ctx);
            Ok(Box::new(App::new(Vec::new())))
        }),
    )
}
