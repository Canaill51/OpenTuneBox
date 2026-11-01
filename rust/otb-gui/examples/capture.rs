//! Capture d'écran sans fenêtre (rendu wgpu de egui_kittest) pour regarder l'interface : jamais dans la CI.
//!
//! `cargo run --release -p otb-gui --example capture -- LOG.bbl DOSSIER_DE_SORTIE`
//! Écrit `courbes-clair.png` et `courbes-sombre.png`.

use egui_kittest::Harness;
use otb_gui::app::{App, Screen};
use otb_gui::theme;

fn main() {
    let mut args = std::env::args().skip(1);
    let log = args.next().expect("chemin du log");
    let out = std::path::PathBuf::from(args.next().expect("dossier de sortie"));
    for th in [theme::CLAIR, theme::SOMBRE] {
        let mut app = App::new(Vec::new());
        app.theme = th;
        let mut h = Harness::builder()
            .with_size(egui::vec2(1500.0, 900.0))
            .with_context_setup(otb_gui::app::setup)
            .wgpu()
            .build_ui_state(|ui, a: &mut App| a.ui(ui), app);
        h.state_mut().open(vec![log.clone().into()]);
        for _ in 0..200 {
            h.run();
            if !h.state().state.logs.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        for _ in 0..4 {
            h.run();
        }
        assert_eq!(h.state().screen, Screen::Viewer);
        // une mesure et un curseur au milieu du vol, pour voir le lecteur et les repères
        let (lo, hi) = {
            let l = &h.state().state.logs[0].log;
            (l.t[0], *l.t.last().unwrap())
        };
        h.state_mut().state.set_cursor(lo + (hi - lo) * 0.45);
        h.state_mut().viewer.measure_on = true;
        h.state_mut().viewer.measure = vec![lo + (hi - lo) * 0.30, lo + (hi - lo) * 0.33];
        h.run_steps(4);
        let img = h.render().expect("rendu");
        img.save(out.join(format!("courbes-{}.png", th.name))).unwrap();
    }
}
