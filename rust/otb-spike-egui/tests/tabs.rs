//! Test d'interface sans écran : chaque onglet est dessiné par egui_kittest (rendu wgpu logiciel)
//! et enregistré en PNG dans `OTB_SPIKE_OUT`. Vérifie qu'aucun onglet ne panique et que l'image n'est pas vide.

use egui_kittest::Harness;
use otb_spike_egui::{Spike, Tab};

#[test]
fn chaque_onglet_se_dessine_dans_les_deux_themes() {
    let dir = std::env::var("OTB_SPIKE_DIR").expect("OTB_SPIKE_DIR");
    let out = std::path::PathBuf::from(std::env::var("OTB_SPIKE_OUT").expect("OTB_SPIKE_OUT"));
    std::fs::create_dir_all(&out).unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1100.0, 700.0))
        .build_ui_state(|ui, s: &mut Spike| s.ui(ui), Spike::load(dir.into()));
    for (tab, name) in [
        (Tab::Spectre, "spectre"),
        (Tab::Carte, "carte"),
        (Tab::Serie, "serie"),
        (Tab::Reperes, "reperes"),
        (Tab::DeuxAxes, "axes"),
        (Tab::Texte, "texte"),
    ] {
        for dark in [true, false] {
            h.state_mut().tab = tab;
            h.state_mut().dark = dark;
            h.run();
            let img = h.render().expect("rendu");
            let distinct: std::collections::HashSet<_> = img.pixels().map(|p| p.0).collect();
            assert!(distinct.len() > 20, "{name} (sombre={dark}) : image quasi vide");
            img.save(out.join(format!("{name}-{}.png", if dark { "sombre" } else { "clair" })))
                .unwrap();
        }
    }
}
