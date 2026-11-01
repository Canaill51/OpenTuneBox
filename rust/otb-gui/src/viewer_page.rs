//! Page du visualiseur (port de `gui/main_window.py`, 6-1b) : trois graphiques Roll / Pitch / Yaw et un graphique
//! des moteurs avec un axe de rotation (Hz) à droite ; période d'analyse et curseur déplaçables, synchronisés entre
//! les graphiques. Mesure entre deux points, lecteur et manettes : 6-1c.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use egui::{Color32, Id, Ui};
use egui_plot::{AxisHints, HPlacement, Line, LineStyle, Plot, PlotPoint, PlotPoints, Span, Text, VLine};
use otb_log::flightlog::FlightLog;
use otb_log::modes::mode_changes;

use crate::plots::{VDrag, right_axis_scale, visible_points};
use crate::state::AppState;
use crate::theme::{self, AXIS_NAMES, Theme};
use crate::viewer::{DEFAULT_TRACES, SMOOTHING, TRACE_GROUPS, has_trace, smooth, trace_data, trace_label};

/// Groupe des axes X liés (zoom et déplacement communs aux quatre graphiques).
const LINK: &str = "otb-viewer-x";

/// Une courbe prête à tracer : lissage déjà appliqué.
struct Curve {
    name: String,
    color: Color32,
    width: f32,
    y: Arc<Vec<f64>>,
    dotted: bool,
}

pub struct ViewerPage {
    traces: HashSet<&'static str>,
    pub motors: bool,
    pub rps: bool,
    pub throttle: bool,
    pub yscale: u32,
    /// Index dans `SMOOTHING`.
    pub smoothing: usize,
    pub width: f32,
    /// 0 : les trois axes ; 1 à 3 : un seul.
    pub focus: usize,
    drags: [VDrag; 4],
    /// Période en cours de glissement (appliquée au relâchement : les calculs ne repartent pas à chaque pixel).
    pub period_edit: Option<(f64, f64)>,
    /// Quel log est affiché (adresse de son `Arc`) : le changer vide les caches et recadre les graphiques.
    shown: Option<usize>,
    /// Recadrer en X sur tout le log au prochain cadre.
    fit: bool,
    series: HashMap<(&'static str, usize, usize), Arc<Vec<f64>>>,
    present: HashMap<&'static str, bool>,
    modes: Vec<(f64, String)>,
    link: Id,
    /// Transformation (temps ↔ pixels) de chaque graphique au dernier cadre : Roll, Pitch, Yaw, moteurs.
    pub transforms: [Option<egui_plot::PlotTransform>; 4],
}

impl Default for ViewerPage {
    fn default() -> Self {
        Self {
            traces: DEFAULT_TRACES.iter().copied().collect(),
            motors: true,
            rps: false,
            throttle: true,
            yscale: 500,
            smoothing: 0,
            width: 2.0,
            focus: 0,
            drags: [VDrag::default(); 4],
            period_edit: None,
            shown: None,
            fit: true,
            series: HashMap::new(),
            present: HashMap::new(),
            modes: Vec::new(),
            link: Id::unique(LINK),
            transforms: [None; 4],
        }
    }
}

fn log_id(log: &Arc<FlightLog>) -> usize {
    Arc::as_ptr(log) as usize
}

impl ViewerPage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trace_on(&self, key: &str) -> bool {
        self.traces.contains(key)
    }

    pub fn set_trace(&mut self, key: &'static str, on: bool) {
        if on {
            self.traces.insert(key);
        } else {
            self.traces.remove(key);
        }
    }

    /// Recadre les graphiques sur tout le log (« Vue entière »).
    pub fn fit_all(&mut self) {
        self.fit = true;
    }

    fn series(&mut self, key: &'static str, ax: usize, w: usize, make: impl FnOnce() -> Vec<f64>) -> Arc<Vec<f64>> {
        self.series
            .entry((key, ax, w))
            .or_insert_with(|| Arc::new(make()))
            .clone()
    }

    fn is_present(&mut self, log: &FlightLog, key: &'static str) -> bool {
        *self.present.entry(key).or_insert_with(|| has_trace(log, key))
    }

    /// Courbes de chaque axe, du graphique des moteurs et de la rotation (Hz, à l'échelle de l'axe de gauche).
    fn curves(&mut self, log: &FlightLog, th: &Theme) -> ([Vec<Curve>; 3], Vec<Curve>, Vec<Curve>, f64) {
        let w = SMOOTHING[self.smoothing].1;
        let mut axes: [Vec<Curve>; 3] = Default::default();
        let keys: Vec<&'static str> = TRACE_GROUPS.iter().flat_map(|(_, k)| k.iter().copied()).collect();
        for key in keys {
            if !self.traces.contains(key) {
                continue;
            }
            let Some(data) = trace_data(log, key) else { continue };
            for (ax, curves) in axes.iter_mut().enumerate().take(data.k.min(3)) {
                let row = data.row(ax);
                if matches!(key, "dterm" | "dterm_prefilt") && !row.iter().any(|&v| v != 0.0) {
                    continue;
                }
                let y = self.series(key, ax, w, || smooth(row, w));
                curves.push(Curve {
                    name: trace_label(key).into(),
                    color: th.trace_color(key),
                    width: self.width,
                    y,
                    dotted: false,
                });
            }
        }
        let mut motors = Vec::new();
        if self.motors {
            for m in 0..log.n_motors() {
                let y = self.series("motor", m, w, || smooth(log.motors.row(m), w));
                motors.push(Curve {
                    name: format!("Moteur {}", m + 1),
                    color: theme::motor_color(m),
                    width: self.width,
                    y,
                    dotted: false,
                });
            }
        }
        if self.throttle {
            let y = self.series("throttle", 0, 1, || log.throttle.iter().map(|&v| v as f64).collect());
            motors.push(Curve {
                name: "Gaz".into(),
                color: th.trace_color("throttle"),
                width: self.width + 1.0,
                y,
                dotted: false,
            });
        }
        let mut rps = Vec::new();
        let mut k = 1.0;
        if let (true, Some(hz)) = (self.rps, log.motor_hz.as_ref()) {
            let top = hz.data.iter().copied().filter(|v| !v.is_nan()).fold(0.0f32, f32::max) as f64 * 1.05;
            k = right_axis_scale(100.0, top);
            for m in 0..hz.k {
                let y = self.series("rps", m, w, || {
                    smooth(hz.row(m), w).into_iter().map(|v| v * k).collect()
                });
                rps.push(Curve {
                    name: format!("Rotation {} (Hz)", m + 1),
                    color: theme::motor_color(m),
                    width: self.width,
                    y,
                    dotted: true,
                });
            }
        }
        (axes, motors, rps, k)
    }

    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, th: &Theme) {
        // changement de log : caches vidés, graphiques recadrés
        let current = state.current_log().map(|l| log_id(&l.log));
        if current != self.shown {
            self.shown = current;
            self.series.clear();
            self.present.clear();
            self.period_edit = None;
            self.fit = true;
            self.modes = state
                .current_log()
                .map(|l| mode_changes(l.log.mode_flags.as_deref(), &l.log.t, &l.log.settings.version))
                .unwrap_or_default();
            if let Some(l) = state.current_log() {
                let (lo, hi) = (l.log.t[0], *l.log.t.last().unwrap());
                if !(lo..=hi).contains(&state.cursor) {
                    state.set_cursor(lo);
                }
            }
        }
        egui::Panel::left("viewer-traces").show(ui, |ui| self.traces_panel(ui, state, th));
        egui::Panel::right("viewer-controls").show(ui, |ui| self.controls_panel(ui, state));
        egui::CentralPanel::default().show(ui, |ui| self.plots(ui, state, th));
    }

    fn traces_panel(&mut self, ui: &mut Ui, state: &AppState, th: &Theme) {
        ui.horizontal(|ui| {
            let label = ui.label("Échelle Y (°/s)");
            ui.add(egui::DragValue::new(&mut self.yscale).range(10..=3000).speed(10))
                .labelled_by(label.id);
        });
        let log = state.current_log().map(|l| l.log.clone());
        for (heading, keys) in TRACE_GROUPS {
            let shown: Vec<&'static str> = match &log {
                Some(l) => keys.iter().copied().filter(|k| self.is_present(l, k)).collect(),
                None => keys.to_vec(),
            };
            if shown.is_empty() {
                continue;
            }
            ui.add_space(6.0);
            ui.colored_label(th.ui.muted, heading);
            for key in shown {
                let mut on = self.trace_on(key);
                ui.horizontal(|ui| {
                    crate::plots::swatch(ui, th.trace_color(key));
                    if ui.checkbox(&mut on, trace_label(key)).changed() {
                        self.set_trace(key, on);
                    }
                });
            }
        }
        if let Some(l) = &log {
            ui.colored_label(th.ui.muted, format!("Debug : {}", l.settings.debug_mode));
        }
        ui.add_space(6.0);
        ui.colored_label(th.ui.muted, "Moteurs");
        ui.checkbox(&mut self.motors, "Moteurs (%)");
        if log.as_ref().is_some_and(|l| l.motor_hz.is_some()) {
            ui.checkbox(&mut self.rps, "Rotation moteurs (Hz)");
        }
        ui.checkbox(&mut self.throttle, "Gaz (%)");
    }

    fn controls_panel(&mut self, ui: &mut Ui, state: &mut AppState) {
        let labels: Vec<String> = state.logs.iter().map(|l| l.label.clone()).collect();
        let mut cur = state.current;
        let label = ui.label("Log affiché");
        egui::ComboBox::from_id_salt("viewer-log")
            .width(ui.available_width() - 8.0)
            .selected_text(
                cur.and_then(|i| labels.get(i))
                    .map_or("Aucun log ouvert", |s| s.as_str()),
            )
            .show_ui(ui, |ui| {
                for (i, l) in labels.iter().enumerate() {
                    ui.selectable_value(&mut cur, Some(i), l);
                }
            })
            .response
            .labelled_by(label.id);
        if cur != state.current {
            state.set_current(cur);
        }
        if let Some(l) = state.current_log() {
            let s = &l.log.settings;
            ui.label(format!(
                "{}\n{}  {:.0} Hz · {:.1} s",
                s.firmware,
                s.craft_name,
                l.log.fs,
                l.log.duration()
            ));
        }
        ui.horizontal(|ui| {
            for (i, name) in ["Tous", "Roll", "Pitch", "Yaw"].into_iter().enumerate() {
                ui.radio_value(&mut self.focus, i, name);
            }
        });
        ui.horizontal(|ui| {
            if ui
                .button("Période auto")
                .on_hover_text("Du décollage à l'atterrissage, avant un éventuel crash.")
                .clicked()
                && let Some(i) = state.current
            {
                state.reset_period(i);
                self.period_edit = None;
            }
            if ui.button("Vue entière").clicked() {
                self.fit = true;
            }
        });
        ui.collapsing("Affichage", |ui| {
            egui::ComboBox::from_label("Lissage")
                .selected_text(SMOOTHING[self.smoothing].0)
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in SMOOTHING.iter().enumerate() {
                        ui.selectable_value(&mut self.smoothing, i, *name);
                    }
                });
            ui.horizontal(|ui| {
                let label = ui.label("Épaisseur");
                ui.add(egui::DragValue::new(&mut self.width).range(1.0..=5.0).speed(0.1))
                    .labelled_by(label.id);
            });
        });
    }

    fn plots(&mut self, ui: &mut Ui, state: &mut AppState, th: &Theme) {
        let Some(ll) = state.current_log() else {
            ui.label("Aucun log ouvert : bouton « Ouvrir… » ou glisser-déposer d'un fichier .bbl / .bfl.");
            return;
        };
        let (log, index) = (ll.log.clone(), state.current.unwrap());
        let (lo, hi) = (log.t[0], *log.t.last().unwrap());
        let (p0, p1) = self.period_edit.unwrap_or(ll.period);
        let (axes, motors, rps, k) = self.curves(&log, th);

        let shown: Vec<usize> = (0..3).filter(|&a| self.focus == 0 || self.focus == a + 1).collect();
        let total = ui.available_height() - 4.0 * ui.spacing().item_spacing.y;
        let weights: Vec<f32> = shown
            .iter()
            .map(|_| if self.focus == 0 { 2.0 } else { 5.0 })
            .chain([if self.focus == 0 { 1.0 } else { 2.0 }])
            .collect();
        let sum: f32 = weights.iter().sum();

        let mut xs = [state.cursor, p0, p1];
        let before = xs;
        let press = ui.input(|i| i.pointer.press_origin()).map(|p| p.x);
        let fit = std::mem::take(&mut self.fit);
        let width_px = ui.available_width();
        let ymax = self.yscale as f64;
        let mut drags = self.drags;
        let mut transforms = self.transforms;
        let link = self.link;
        let t = &log.t;

        let n_plots = shown.len() + 1;
        for (row, &ax) in shown.iter().chain(std::iter::once(&3)).enumerate() {
            let is_motor = ax == 3;
            let last = row + 1 == n_plots;
            let h = (total * weights[row] / sum).max(60.0);
            let (curves, extra): (&[Curve], &[Curve]) = if is_motor { (&motors, &rps) } else { (&axes[ax], &[]) };
            let left = AxisHints::new_y()
                .label(if is_motor {
                    "Gaz | moteurs (%)".to_string()
                } else {
                    format!("{} (°/s)", AXIS_NAMES[ax])
                })
                .min_thickness(62.0);
            let right = if is_motor && !rps.is_empty() {
                AxisHints::new_y()
                    .placement(HPlacement::Right)
                    .label("Hz")
                    .min_thickness(48.0)
                    .formatter(move |m, _| format!("{:.0}", m.value / k))
            } else {
                AxisHints::new_y()
                    .placement(HPlacement::Right)
                    .min_thickness(48.0)
                    .formatter(|_, _| String::new())
            };
            let drag = &mut drags[ax];
            let tr = &mut transforms[ax];
            let busy = drag.busy();
            let modes = &self.modes;
            Plot::new(("viewer", ax))
                .height(h)
                .custom_y_axes(vec![left, right])
                .show_axes([last, true])
                .x_axis_label(if last { "Temps (s)" } else { "" })
                .auto_bounds(false)
                .allow_zoom([true, false])
                .allow_drag([!busy, false])
                .allow_scroll(false)
                .link_axis(link, [true, false])
                .link_cursor(link, [true, false])
                .grid_color(th.grid_color())
                .show(ui, |pu| {
                    if fit {
                        let pad = (hi - lo) * 0.01;
                        pu.set_plot_bounds_x((lo - pad)..=(hi + pad));
                    }
                    if is_motor {
                        pu.set_plot_bounds_y(-2.0..=102.0);
                    } else {
                        pu.set_plot_bounds_y(-ymax..=ymax);
                    }
                    let b = pu.plot_bounds();
                    let (x0, x1) = (b.min()[0], b.max()[0]);
                    let max_points = (width_px as usize).max(200) * 2;
                    // période : zones grisées dehors, zone claire dedans
                    pu.span(Span::new("", lo..=p0.min(p1)).fill(th.shade));
                    pu.span(Span::new("", p0.max(p1)..=hi).fill(th.shade));
                    pu.span(Span::new("", p0.min(p1)..=p0.max(p1)).fill(th.region_fill()));
                    for c in curves.iter().chain(extra) {
                        let mut line = Line::new(
                            c.name.as_str(),
                            PlotPoints::from(visible_points(t, &c.y, x0, x1, max_points)),
                        )
                        .color(c.color)
                        .width(c.width);
                        if c.dotted {
                            line = line.style(LineStyle::dotted_loose());
                        }
                        pu.line(line);
                    }
                    if is_motor {
                        for (tm, label) in modes {
                            pu.text(
                                Text::new("", PlotPoint::new(*tm, 0.0), label.as_str())
                                    .color(th.fg)
                                    .anchor(egui::Align2::LEFT_BOTTOM),
                            );
                        }
                    }
                    pu.vline(VLine::new("Début", p0).color(th.ui.highlight).width(2.0));
                    pu.vline(VLine::new("Fin", p1).color(th.ui.highlight).width(2.0));
                    pu.vline(
                        VLine::new("Curseur", xs[0])
                            .color(th.fg)
                            .style(LineStyle::dashed_loose()),
                    );
                    drag.update(pu, press, &mut xs, &[(lo, hi), (lo, hi), (lo, hi)]);
                    *tr = Some(*pu.transform());
                });
        }
        self.drags = drags;
        self.transforms = transforms;

        if xs[0] != before[0] {
            state.set_cursor(xs[0]);
        }
        if (xs[1], xs[2]) != (before[1], before[2]) {
            self.period_edit = Some((xs[1], xs[2]));
        }
        // relâché : la période est appliquée une seule fois (les outils recalculent)
        if self.drags.iter().all(|d| d.active.is_none())
            && let Some((a, b)) = self.period_edit.take()
        {
            state.set_period(index, a.min(b), a.max(b));
        }
    }
}
