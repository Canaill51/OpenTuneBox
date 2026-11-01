//! Palettes des cartes : tables générées par `scripts/colormap_tables.py` depuis celles de l'interface Qt (pyqtgraph).

use egui::Color32;
use otb_gui::theme::Palette;

fn rgb(c: Color32) -> [u8; 3] {
    [c.r(), c.g(), c.b()]
}

#[test]
fn les_extremites_de_chaque_palette_sont_celles_de_l_interface_qt() {
    // valeurs lues dans pyqtgraph (`ColorMap.getLookupTable(0, 1, 256)`) le 2026-11-01
    let attendu = [
        (Palette::Hot, [0, 0, 0], [255, 255, 255]),
        (Palette::Inferno, [0, 0, 4], [252, 255, 164]),
        (Palette::Viridis, [68, 1, 84], [253, 231, 37]),
        (Palette::Jet, [0, 0, 143], [128, 0, 0]),
    ];
    for (p, debut, fin) in attendu {
        assert_eq!(rgb(p.color(0.0)), debut, "{}", p.name());
        assert_eq!(rgb(p.color(1.0)), fin, "{}", p.name());
    }
}

#[test]
fn hors_bornes_et_nan_restent_dans_la_palette() {
    for p in Palette::ALL {
        assert_eq!(p.color(-3.0), p.color(0.0));
        assert_eq!(p.color(7.0), p.color(1.0));
        assert_eq!(p.color(f32::NAN), p.color(0.0));
    }
}

#[test]
fn les_noms_sont_ceux_de_python_dans_le_meme_ordre() {
    let noms: Vec<&str> = Palette::ALL.iter().map(|p| p.name()).collect();
    assert_eq!(noms, ["hot", "inferno", "viridis", "turbo", "jet"]);
}
