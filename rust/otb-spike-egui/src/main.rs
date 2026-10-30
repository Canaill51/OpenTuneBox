use otb_spike_egui::{App, Spike};

fn main() -> eframe::Result {
    let dir = std::env::var("OTB_SPIKE_DIR").expect("OTB_SPIKE_DIR");
    let spike = Spike::load(dir.into());
    eframe::run_native(
        "OpenTuneBox — prototype egui",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(App(spike)))),
    )
}
