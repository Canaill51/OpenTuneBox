//! Les outils d'analyse, une page chacun (`gui/tools/` de Python). Les briques qu'ils partagent sont dans `common`.

pub mod common;
pub mod freq_throttle;
pub mod freq_time;
pub mod pid_slider;
pub mod report_ui;
pub mod spectral;
pub mod step;
pub mod step_common;

use crate::tool::{Page, ToolPage};

/// Les pages de tous les outils, dans l'ordre de la barre (`REGISTRY` de Python).
pub fn all_pages() -> Vec<Box<dyn Page>> {
    vec![
        Box::new(ToolPage::new(spectral::SpectralTool::new())),
        Box::new(ToolPage::new(freq_throttle::FreqThrottleTool::new())),
        Box::new(ToolPage::new(freq_time::FreqTimeTool::new())),
        Box::new(ToolPage::new(step::StepTool::new())),
        Box::new(ToolPage::new(pid_slider::PidSliderTool::new())),
    ]
}
