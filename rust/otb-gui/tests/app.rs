//! Fenêtre principale : thème conservé quand le système change de thème, ouverture de fichiers fichier par fichier.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use otb_gui::app::App;
use otb_gui::theme;

fn banc(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(900.0, 500.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app)
}

#[test]
fn le_theme_choisi_survit_a_un_changement_de_theme_du_systeme() {
    let mut app = App::new(Vec::new());
    app.theme = theme::SOMBRE;
    let mut h = banc(app);
    h.run();
    assert_eq!(h.ctx.global_style().visuals.panel_fill, theme::SOMBRE.ui.window);
    // le bureau passe en clair puis en sombre : notre palette doit rester
    for sys in [egui::Theme::Light, egui::Theme::Dark] {
        h.input_mut().system_theme = Some(sys);
        h.run();
        assert_eq!(
            h.ctx.global_style().visuals.panel_fill,
            theme::SOMBRE.ui.window,
            "système {sys:?}"
        );
    }
}

fn donnees() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")
}

#[test]
fn un_fichier_illisible_n_empeche_pas_les_suivants() {
    assert!(donnees().exists(), "{}", donnees().display());
    let mut h = banc(App::new(Vec::new()));
    h.state_mut()
        .open(vec![PathBuf::from("/inexistant/absent.bbl"), donnees(), donnees()]);
    let debut = Instant::now();
    while h.state().state.logs.len() < 2 {
        h.run();
        assert!(debut.elapsed() < Duration::from_secs(30), "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(5));
    }
    for _ in 0..3 {
        h.run();
    }
    let a = h.state();
    assert_eq!(a.state.logs.len(), 2, "les deux bons fichiers sont ouverts");
    assert!(
        a.status.contains("absent.bbl") && a.status.contains("illisible"),
        "{}",
        a.status
    );
    assert_eq!(a.state.logs[1].label, format!("{} (2)", a.state.logs[0].label));
}
