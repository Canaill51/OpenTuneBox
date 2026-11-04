//! Briques communes à la Réponse indicielle et aux Curseurs PID (port de `gui/tools/_step_common.py`) : calcul de la réponse
//! indicielle d'un log (par axe, avec les métriques fenêtre par fenêtre qui servent aux barres d'erreur) et graphique des 8
//! curseurs Betaflight (position 0 à 2).

use std::ops::Range;
use std::sync::Arc;

use egui::{Color32, RichText, Ui};
use egui_plot::{GridMark, LineStyle, MarkerShape, Plot, PlotPoints, Points, VLine};
use otb_analysis::delay::xcorr_delay_ms;
use otb_analysis::npstat::std_f32;
use otb_analysis::open_loop::Terms;
use otb_analysis::sliders::{PTB_CATEGORIES, SliderView, slider_view};
use otb_analysis::step::{LoopInput, StepOptions, StepResult, segment_metrics, step_response};
use otb_analysis::step_series::{FlightStep, filter_signature, flight_conditions};
use otb_log::flightlog::{FlightLog, clamp_range};
use otb_log::settings::AxisPid;

use crate::theme::Theme;
use crate::tools::common::wide;

/// Nom des 8 curseurs dans le configurateur Betaflight (`locales/en/messages.json`, `pidTuning*Slider`) : catégorie de PIDtoolbox, nom
/// affiché et, dessous, ce que le curseur règle (même ordre que `PTB_CATEGORIES`).
pub const CATEGORY_BF: [(&str, &str, &str); 8] = [
    ("Damping", "Damping", "D Gains"),
    ("Tracking", "Tracking", "P & I Gains"),
    ("Stick Resp.", "Stick Response", "FF Gains"),
    ("Dynamic D.", "Dynamic Damping", "D Max"),
    ("Drift-Wobble", "Drift - Wobble", "I Gains"),
    ("Pitch D.", "Pitch Damping", "Pitch:Roll D"),
    ("Pitch T.", "Pitch Tracking", "Pitch:Roll P, I & FF"),
    ("Master", "Master Multiplier", ""),
];

/// Nom affiché d'une catégorie de curseur (« Drift - Wobble » pour `Drift-Wobble`).
pub fn category_name(category: &str) -> &'static str {
    CATEGORY_BF.iter().find(|c| c.0 == category).map_or("?", |c| c.1)
}

/// Séparation petits / grands mouvements de la consigne (`SPLITS` de Python).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Split {
    All,
    Small,
    Large,
}

impl Split {
    pub const ALL: [Split; 3] = [Split::All, Split::Small, Split::Large];

    pub fn label(self) -> &'static str {
        match self {
            Split::All => "tous",
            Split::Small => "< 500 °/s",
            Split::Large => "≥ 500 °/s",
        }
    }

    /// `(min_input, max_input)`.
    pub fn bounds(self) -> (f64, Option<f64>) {
        match self {
            Split::All => (20.0, None),
            Split::Small => (20.0, Some(500.0)),
            Split::Large => (500.0, None),
        }
    }
}

/// Réglages de calcul de la réponse indicielle : ce qui change le résultat (aucun NaN possible : valeurs choisies dans des listes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StepOpts {
    pub seg_s: f64,
    pub smooth: i64,
    pub split: Split,
    pub y_corr: bool,
}

impl Default for StepOpts {
    fn default() -> Self {
        Self {
            seg_s: 2.0,
            smooth: 0,
            split: Split::All,
            y_corr: false,
        }
    }
}

/// Mode du réglage simplifié (`simplified_pids_mode`) en texte.
pub fn mode_text(mode: i64) -> &'static str {
    match mode {
        0 => "OFF",
        1 => "RP",
        2 => "RPY",
        _ => "?",
    }
}

/// `AxisPID.text()` de Python : « P,I,D,Dmax,F ».
pub fn pid_text(p: &AxisPid) -> String {
    format!("{},{},{},{},{}", p.p, p.i, p.d, p.dmax, p.f)
}

/// Une réponse indicielle d'un axe, avec le pic et la latence de chaque fenêtre (barres d'erreur).
#[derive(Clone, Debug)]
pub struct AxisStep {
    pub res: StepResult,
    /// Pic de chaque réponse individuelle.
    pub seg_peak: Vec<f64>,
    /// Latence 50 % de chaque réponse individuelle (ms).
    pub seg_latency: Vec<f64>,
    /// Retard consigne → gyro (intercorrélation), NaN si la consigne bouge trop peu.
    pub sp_delay_ms: f64,
}

/// Tout ce que le calcul d'un fichier produit, **sans** libellé ni couleur : c'est ce qu'on range dans le cache (le fichier peut
/// changer de rang, donc de couleur, sans que le résultat change).
#[derive(Clone, Debug)]
pub struct FileData {
    pub family: String,
    pub pids: [AxisPid; 3],
    pub sliders: SliderView,
    /// `simplified_pids_mode` (0 = réglage simplifié désactivé ou absent).
    pub mode: i64,
    pub axes: Vec<AxisStep>,
    /// Roll / pitch pour le « Diagnostic du réglage ».
    pub flight: FlightStep,
}

/// Un fichier tel qu'on l'affiche.
#[derive(Clone, Debug)]
pub struct FileStep {
    pub label: String,
    pub color: Color32,
    pub data: Arc<FileData>,
}

/// Le mode lu dans l'en-tête : `int(sl.get("mode", 0) or 0)`.
pub fn header_mode(sliders: &[(&'static str, i64)]) -> i64 {
    sliders.iter().find(|(k, _)| *k == "mode").map_or(0, |(_, v)| *v)
}

/// Réponse indicielle des 3 axes d'un log sur sa période d'analyse (`compute_file_step`).
pub fn compute_file_data(log: &FlightLog, sel: Range<usize>, o: &StepOpts) -> Result<FileData, String> {
    let s = &log.settings;
    let sliders = slider_view(s)?;
    let (min_input, max_input) = o.split.bounds();
    let breaks: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let br = if breaks.is_empty() {
        None
    } else {
        Some(breaks.as_slice())
    };
    let opts = StepOptions {
        seg_s: o.seg_s,
        min_input,
        max_input,
        smooth: o.smooth,
        y_correction: o.y_corr,
        ..Default::default()
    };
    let mut axes = Vec::with_capacity(3);
    for ax in 0..3 {
        let rows = |r: &otb_log::flightlog::Rows| r.row(ax)[clamp_range(sel.start, sel.end, r.row(ax).len())].to_vec();
        let (sp32, gy32) = (rows(&log.setpoint), rows(&log.gyro));
        let (sp, gy) = (wide(&sp32), wide(&gy32));
        let terms = log.pid_terms(ax, sel.clone());
        let (tp, ti, td, tf) = (wide(&terms.p), wide(&terms.i), wide(&terms.d), wide(&terms.f));
        let inst = terms.instrument.as_ref().map(|v| wide(v));
        let input = if terms.note.is_empty() {
            LoopInput::Terms(Terms {
                p: &tp,
                i: &ti,
                d: &td,
                f: &tf,
                instrument: inst.as_deref(),
            })
        } else {
            LoopInput::Unmeasurable(&terms.note)
        };
        let res = step_response(&sp, &gy, log.fs, &opts, &input, br)?;
        let (seg_peak, seg_latency) = segment_metrics(&res.t_ms, &res.responses, res.n)?;
        // `np.std(sp) >= 5.0` sur le tableau float32 du journal
        let sp_delay_ms = if std_f32(&sp32) >= 5.0f32 {
            xcorr_delay_ms(&sp, &gy, log.fs, 100.0, 0)?
        } else {
            f64::NAN
        };
        axes.push(AxisStep {
            res,
            seg_peak,
            seg_latency,
            sp_delay_ms,
        });
    }
    let flight = FlightStep {
        label: String::new(), // remis au libellé du fichier à l'affichage
        sliders: sliders.values.clone(),
        filters: filter_signature(s),
        conditions: flight_conditions(log, sel)?,
        metrics: axes[..2].iter().map(|a| a.res.metrics).collect(),
        n: axes[..2].iter().map(|a| a.res.n).collect(),
        sp_delay_ms: axes[..2].iter().map(|a| a.sp_delay_ms).collect(),
        loops: axes[..2].iter().map(|a| a.res.loop_.clone()).collect(),
    };
    Ok(FileData {
        family: s.family().to_string(),
        pids: s.pids,
        sliders,
        mode: header_mode(&s.sliders()),
        axes,
        flight,
    })
}

/// Petit disque plein ou creux devant un libellé de légende, dessiné : « ● » et « ○ » ne sont pas dans les polices d'egui (mesuré le
/// 2026-11-02, avec « ◇ ■ ▲ ✔ → » : seuls « ≥ ≤ ° · ± × « » – — … • ≈ ÷ Δ Ω µ » y sont).
pub fn dot(ui: &mut Ui, color: Color32, filled: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    let c = rect.center();
    if filled {
        ui.painter().circle_filled(c, 5.0, color);
    } else {
        ui.painter().circle_stroke(c, 4.5, egui::Stroke::new(2.0, color));
    }
}

// ------------------------------------------------------------------ graphique des curseurs Betaflight

/// Ordonnée de la catégorie `i` (la première en haut).
fn row_y(i: usize) -> f64 {
    (PTB_CATEGORIES.len() - 1 - i) as f64
}

/// Décalage vertical du fichier `k` sur `nf` : les points d'une même catégorie ne se superposent pas.
pub fn file_offset(k: usize, nf: usize) -> f64 {
    if nf <= 1 {
        0.0
    } else {
        0.36 * (0.5 - k as f64 / (nf - 1) as f64)
    }
}

/// Positions des 8 curseurs (0 à 2) : un point par fichier, décalé s'il y a plusieurs fichiers. Point plein = curseurs de l'en-tête ;
/// point creux = estimation depuis les PID. `extra` : positions supplémentaires (fractions, dans l'ordre des catégories), dessinées en
/// losange (simulation).
pub fn slider_plot(
    ui: &mut Ui,
    id: &str,
    files: &[(Color32, &SliderView)],
    extra: Option<&[f64; 8]>,
    title: &str,
    theme: &Theme,
    height: f32,
) {
    ui.label(RichText::new(title).strong());
    let n = PTB_CATEGORIES.len();
    Plot::new(id.to_string())
        .height(height)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .show_grid([true, true])
        .grid_color(theme.grid_color())
        .x_axis_label("Position du curseur")
        .y_axis_min_width(150.0)
        .x_grid_spacer(|_| {
            (0..=10)
                .map(|x| GridMark {
                    value: x as f64 / 5.0,
                    step_size: 0.2,
                })
                .collect()
        })
        .y_grid_spacer(move |_| {
            (0..n)
                .map(|i| GridMark {
                    value: i as f64,
                    step_size: 1.0,
                })
                .collect()
        })
        .x_axis_formatter(|m, _| format!("{}", (m.value * 10.0).round() / 10.0))
        .y_axis_formatter(move |m, _| {
            let i = (n as f64 - 1.0 - m.value).round();
            if (m.value - m.value.round()).abs() > 1e-6 || !(0.0..n as f64).contains(&i) {
                return String::new();
            }
            let c = CATEGORY_BF[i as usize];
            if c.2.is_empty() {
                c.1.to_string()
            } else {
                format!("{}\n{}", c.1, c.2)
            }
        })
        .show(ui, |pu| {
            pu.set_plot_bounds_x(-0.1..=2.1);
            pu.set_plot_bounds_y(-0.6..=n as f64 - 0.4);
            pu.vline(
                VLine::new("", 1.0)
                    .color(theme.fg)
                    .width(1.0)
                    .style(LineStyle::dashed_loose()),
            );
            let nf = files.len().max(1);
            for (k, (color, view)) in files.iter().enumerate() {
                let off = file_offset(k, nf);
                let pts: Vec<[f64; 2]> = PTB_CATEGORIES
                    .iter()
                    .enumerate()
                    .map(|(i, (cat, _))| [value_of(view, cat), row_y(i) + off])
                    .collect();
                if view.from_header {
                    // point plein cerclé de la couleur du texte
                    pu.points(
                        Points::new("", PlotPoints::from(pts.clone()))
                            .color(theme.fg)
                            .radius(6.5)
                            .filled(true),
                    );
                    pu.points(
                        Points::new("", PlotPoints::from(pts))
                            .color(*color)
                            .radius(5.0)
                            .filled(true),
                    );
                } else {
                    pu.points(
                        Points::new("", PlotPoints::from(pts))
                            .color(*color)
                            .radius(5.5)
                            .filled(false),
                    );
                }
            }
            if let Some(x) = extra {
                let pts: Vec<[f64; 2]> = x.iter().enumerate().map(|(i, v)| [*v, row_y(i)]).collect();
                pu.points(
                    Points::new("", PlotPoints::from(pts))
                        .color(theme.fg)
                        .shape(MarkerShape::Diamond)
                        .radius(7.0)
                        .filled(false),
                );
            }
        });
}

fn value_of(view: &SliderView, category: &str) -> f64 {
    view.values
        .iter()
        .find(|(k, _)| *k == category)
        .map_or(f64::NAN, |(_, v)| *v)
}
