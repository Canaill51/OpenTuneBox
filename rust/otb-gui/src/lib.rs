//! Interface egui d'OpenTuneBox (phase 6, D-035). Le calcul reste dans `otb-log`, `otb-analysis` et
//! `otb-advisor` : cette crate n'affiche que ce qu'ils calculent.

pub mod app;
pub mod colormaps;
pub mod guide;
pub mod guide_page;
pub mod icons;
pub mod plots;
pub mod state;
pub mod theme;
pub mod tool;
pub mod tools;
pub mod viewer;
pub mod viewer_page;
pub mod worker;
