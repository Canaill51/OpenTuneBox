//! Outil « Assistant » (port de `gui/tools/advisor_panel.py`) : le rapport de diagnostic d'un log, constats groupés par catégorie,
//! et à droite la réponse indicielle de chaque axe (moyenne et bande ± écart-type) qui appuie les constats.

use std::sync::Arc;

use egui::{Align2, RichText, Ui};
use egui_plot::{FilledArea, HLine, Line, LineStyle, Plot, PlotPoints};
use otb_advisor::profiles::{PROFILES, profile};
use otb_advisor::rules::{Report, analyze};

use crate::plots::{corner_text, nice_axes};
use crate::state::AppState;
use crate::theme::{Theme, with_alpha};
use crate::tool::{LogInput, Tool};
use crate::tools::common::{FileSelection, safe_text};
use crate::tools::report_ui::finding_ui;

/// Ordre des catégories ; les autres suivent, triées.
pub const CATEGORY_ORDER: [&str; 8] = [
    "Enregistrement",
    "Vol",
    "Réponse",
    "Bruit",
    "Filtres",
    "Moteurs",
    "Mécanique",
    "Réglage",
];

pub const HELP: &str = "Les conseils portent sur la période d'analyse choisie dans le visualiseur. Changez une seule chose à la fois, \
     revolez, et comparez les logs.";
pub const FOOTER: &str = "Diagnostic automatique à partir des mesures du log : à confirmer par le ressenti en vol et la température des moteurs.";
/// Largeur des courbes à l'ouverture.
const PLOTS_WIDTH: f32 = 420.0;
/// Durée affichée par défaut quand un axe n'a aucune réponse (ms).
const EMPTY_X_MS: f64 = 500.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// Clé du profil choisi (`None` : détection automatique).
    pub profile: Option<&'static str>,
}

pub struct AdvisorOutput {
    pub label: String,
    pub color: egui::Color32,
    pub firmware: String,
    pub report: Arc<Report>,
}

/// Catégories à afficher : l'ordre fixé, puis les autres triées (celles qui n'ont aucun constat sont omises par l'appelant).
pub fn categories(report: &Report) -> Vec<String> {
    let mut extra: Vec<String> = report
        .findings
        .iter()
        .map(|f| f.category.clone())
        .filter(|c| !CATEGORY_ORDER.contains(&c.as_str()))
        .collect();
    extra.sort();
    extra.dedup();
    CATEGORY_ORDER.iter().map(|s| s.to_string()).chain(extra).collect()
}

/// « pic 1.17 · latence 22.9 ms · n=9 » (`f"pic {m.peak:.2f} · latence {m.latency_ms:.1f} ms · n={r.n}"`).
pub fn step_text(r: &otb_analysis::step::StepResult) -> String {
    if r.n == 0 {
        return "données insuffisantes".to_string();
    }
    format!(
        "pic {} · latence {} ms · n={}",
        otb_format::fixed(r.metrics.peak, 2, false),
        otb_format::fixed(r.metrics.latency_ms, 1, false),
        r.n
    )
}

pub struct AdvisorTool {
    profile: Option<&'static str>,
    files: FileSelection,
}

impl Default for AdvisorTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvisorTool {
    pub fn new() -> Self {
        Self {
            profile: None,
            files: FileSelection::new(false),
        }
    }

    fn profile_label(key: Option<&'static str>) -> &'static str {
        key.and_then(profile).map_or("Détection automatique", |p| p.label)
    }

    fn report_ui(ui: &mut Ui, theme: &Theme, out: &AdvisorOutput) {
        let rep = &out.report;
        ui.heading(RichText::new(&out.label).color(out.color));
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Drone :").strong());
            ui.label(safe_text(rep.profile.label));
            ui.label(RichText::new(safe_text(&format!("({})", rep.profile_reason))).italics());
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Période analysée :").strong());
            ui.label(format!(
                "{} – {} s ·",
                otb_format::fixed(rep.period.0, 1, false),
                otb_format::fixed(rep.period.1, 1, false)
            ));
            ui.label(RichText::new("Firmware :").strong());
            ui.label(safe_text(&out.firmware));
        });
        let findings = rep.sorted();
        for cat in categories(rep) {
            let items: Vec<_> = findings.iter().filter(|f| f.category == cat).collect();
            if items.is_empty() {
                continue;
            }
            ui.add_space(8.0);
            ui.label(RichText::new(safe_text(&cat)).strong().size(17.0));
            for f in items {
                finding_ui(ui, f);
            }
        }
        ui.separator();
        ui.label(RichText::new(FOOTER).color(theme.ui.muted));
    }

    fn plots_ui(ui: &mut Ui, theme: &Theme, out: &AdvisorOutput) {
        let n = out.report.steps.len().max(1) as f32;
        let sp = ui.spacing().item_spacing.y;
        let h = ((ui.available_height() - n * (22.0 + 2.0 * sp)) / n).clamp(120.0, 400.0);
        let last = out.report.steps.len().saturating_sub(1);
        for (row, st) in out.report.steps.iter().enumerate() {
            ui.label(RichText::new(format!("{} — réponse indicielle", st.axis)).strong());
            let r = &st.result;
            let x_hi = r.t_ms.last().copied().unwrap_or(EMPTY_X_MS);
            let x_lo = r.t_ms.first().copied().unwrap_or(0.0);
            let mut plot = nice_axes(Plot::new(("assistant-reponse", row)))
                .width(ui.available_width())
                .height(h)
                .auto_bounds(false)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .grid_color(theme.grid_color());
            if row == last {
                plot = plot.x_axis_label("Temps (ms)");
            }
            let resp = plot.show(ui, |pu| {
                pu.set_plot_bounds_x(x_lo..=x_hi);
                pu.set_plot_bounds_y(0.0..=1.5);
                pu.hline(
                    HLine::new("", 1.0)
                        .color(theme.fg)
                        .width(1.0)
                        .style(LineStyle::dashed_loose()),
                );
                if r.n > 0 {
                    let lo: Vec<f64> = r.mean.iter().zip(&r.std).map(|(m, s)| m - s).collect();
                    let hi: Vec<f64> = r.mean.iter().zip(&r.std).map(|(m, s)| m + s).collect();
                    pu.add(FilledArea::new("", &r.t_ms, &lo, &hi).fill_color(with_alpha(out.color, 40)));
                    let pts: Vec<[f64; 2]> = r.t_ms.iter().zip(&r.mean).map(|(t, m)| [*t, *m]).collect();
                    pu.line(Line::new("", PlotPoints::from(pts)).color(out.color).width(2.0));
                }
            });
            let (align, text) = if r.n > 0 {
                (Align2::RIGHT_TOP, step_text(r))
            } else {
                (Align2::LEFT_TOP, step_text(r))
            };
            corner_text(
                ui,
                *resp.transform.frame(),
                align,
                &[(text, theme.fg)],
                theme.legend_fill,
            );
        }
    }
}

impl Tool for AdvisorTool {
    type Options = Options;
    type Output = AdvisorOutput;
    const TITLE: &'static str = "Assistant";

    fn options(&self) -> Options {
        Options { profile: self.profile }
    }

    fn selected(&self, state: &AppState) -> Vec<usize> {
        self.files.indices(state)
    }

    fn compute(o: Options, logs: Vec<LogInput>) -> Result<AdvisorOutput, String> {
        let li = logs.first().ok_or("Sélectionnez au moins un log.")?;
        // la clé contient le profil ET la période : un rapport calculé avant un changement ne peut pas servir
        let key = format!("advisor:{}:{:?}", o.profile.unwrap_or("auto"), li.period);
        let report = match li.cache.get::<Report>(&key) {
            Some(r) => r,
            None => {
                let sel = li.log.slice_time(li.period.0, li.period.1);
                let prof = match o.profile {
                    Some(k) => Some(profile(k).ok_or_else(|| format!("Profil inconnu : {k}"))?),
                    None => None,
                };
                let r = Arc::new(analyze(&li.log, Some(sel), prof)?);
                li.cache.put(key, r.clone());
                r
            }
        };
        Ok(AdvisorOutput {
            label: li.label.clone(),
            color: li.color,
            firmware: li.log.settings.firmware.clone(),
            report,
        })
    }

    fn controls(&mut self, ui: &mut Ui, state: &AppState, theme: &Theme) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.files.ui(ui, state);
            ui.separator();
            ui.label("Taille du drone :");
            egui::ComboBox::from_id_salt("assistant-profil")
                .selected_text(safe_text(Self::profile_label(self.profile)))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.profile, None, Self::profile_label(None));
                    for p in PROFILES.iter() {
                        ui.selectable_value(&mut self.profile, Some(p.key), safe_text(p.label));
                    }
                })
                .response
                .accessible_name("Taille du drone");
            ui.label(RichText::new(HELP).color(theme.ui.muted));
        });
    }

    fn render(&mut self, ui: &mut Ui, _state: &mut AppState, out: &AdvisorOutput, theme: &Theme) {
        egui::Panel::right("assistant-courbes")
            .default_size(PLOTS_WIDTH)
            .size_range(280.0..=800.0)
            .show(ui, |ui| Self::plots_ui(ui, theme, out));
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("assistant-rapport")
                .auto_shrink([false, false])
                .show(ui, |ui| Self::report_ui(ui, theme, out));
        });
    }
}
