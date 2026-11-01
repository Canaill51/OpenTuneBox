//! Icônes [Lucide](https://lucide.dev) (licence ISC, `assets/icons/LICENSE-lucide.txt`), port de `gui/icons.py`.
//!
//! Les fichiers SVG sont copiés sans modification. Lucide dessine en `currentColor` : on le remplace par du
//! blanc au chargement, puis l'image est teintée de la couleur voulue (`Image::tint`).

use egui::{Image, Vec2};

pub const ICONS: [(&str, &str); 21] = [
    ("activity", include_str!("../assets/icons/activity.svg")),
    ("audio-waveform", include_str!("../assets/icons/audio-waveform.svg")),
    ("book-open", include_str!("../assets/icons/book-open.svg")),
    ("chart-line", include_str!("../assets/icons/chart-line.svg")),
    ("chevron-down", include_str!("../assets/icons/chevron-down.svg")),
    ("chevron-left", include_str!("../assets/icons/chevron-left.svg")),
    ("chevron-right", include_str!("../assets/icons/chevron-right.svg")),
    ("chevron-up", include_str!("../assets/icons/chevron-up.svg")),
    ("cpu", include_str!("../assets/icons/cpu.svg")),
    ("folder-open", include_str!("../assets/icons/folder-open.svg")),
    ("funnel", include_str!("../assets/icons/funnel.svg")),
    ("image-down", include_str!("../assets/icons/image-down.svg")),
    ("lightbulb", include_str!("../assets/icons/lightbulb.svg")),
    ("pause", include_str!("../assets/icons/pause.svg")),
    ("play", include_str!("../assets/icons/play.svg")),
    ("ruler", include_str!("../assets/icons/ruler.svg")),
    ("skip-back", include_str!("../assets/icons/skip-back.svg")),
    ("skip-forward", include_str!("../assets/icons/skip-forward.svg")),
    (
        "sliders-horizontal",
        include_str!("../assets/icons/sliders-horizontal.svg"),
    ),
    ("trash-2", include_str!("../assets/icons/trash-2.svg")),
    ("x", include_str!("../assets/icons/x.svg")),
];

/// Source SVG d'une icône par son nom (`None` si elle n'existe pas).
pub fn source(name: &str) -> Option<&'static str> {
    ICONS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

/// Image d'une icône carrée de côté `size` points, à teinter avec `.tint(couleur)`.
/// Panique si le nom est inconnu (faute de frappe du code : `tests/contracts.rs` les vérifie toutes).
pub fn icon(name: &str, size: f32) -> Image<'static> {
    let svg = source(name)
        .unwrap_or_else(|| panic!("icône inconnue : {name}"))
        .replace("currentColor", "white");
    Image::from_bytes(format!("bytes://otb-icon-{name}.svg"), svg.into_bytes()).fit_to_exact_size(Vec2::splat(size))
}
