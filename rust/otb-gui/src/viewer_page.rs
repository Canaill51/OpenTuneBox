//! Page du visualiseur (port de `gui/main_window.py`, 6-1b) : trois graphiques Roll / Pitch / Yaw et un graphique
//! des moteurs avec un axe de rotation (Hz) à droite ; période d'analyse et curseur déplaçables, synchronisés entre
//! les graphiques. Mesure entre deux points, lecteur et manettes : 6-1c.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use egui::{Color32, Id, Ui};
use egui_plot::{AxisHints, HPlacement, Line, LineStyle, Plot, PlotPoint, PlotPoints, Span, Text, VLine};
use otb_dsp::minmax::{MinMaxPyramid, window_points};
use otb_log::flightlog::FlightLog;
use otb_log::modes::mode_changes;

use crate::icons::icon;
use crate::plots::{VDrag, right_axis_scale};
use crate::state::AppState;
use crate::theme::{self, AXIS_NAMES, Theme};
use crate::viewer::{
    DEFAULT_TRACES, SMOOTHING, TRACE_GROUPS, has_trace, measure_text, readout_lines, smooth, stick_positions,
    trace_data, trace_label,
};

/// Groupe des axes X liés (zoom et déplacement communs aux quatre graphiques).
const LINK: &str = "otb-viewer-x";

/// Une courbe en cache : valeurs (lissage déjà appliqué) et index multi-résolution qui rend le tracé indépendant
/// du nombre d'échantillons (D-048).
///
/// ⚠️ Les deux vont **ensemble** : les indices de l'index désignent des positions dans `y`. Les ranger dans le même
/// objet de cache interdit de tracer une courbe avec l'index d'une autre (D-048).
struct Series {
    y: Vec<f64>,
    pyramid: MinMaxPyramid,
}

impl Series {
    fn new(y: Vec<f64>) -> Self {
        let pyramid = MinMaxPyramid::build(&y);
        Self { y, pyramid }
    }
}

/// Une courbe prête à tracer : lissage déjà appliqué.
struct Curve {
    name: String,
    color: Color32,
    width: f32,
    data: Arc<Series>,
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
    /// Mode « Mesurer » : un clic sur Roll, Pitch ou Yaw place un point, deux points donnent Δt et la fréquence.
    pub measure_on: bool,
    pub measure: Vec<f64>,
    /// Lecture en cours : le curseur avance à la vitesse du temps réel.
    pub playing: bool,
    drags: [VDrag; 4],
    /// Période en cours de glissement (appliquée au relâchement : les calculs ne repartent pas à chaque pixel).
    pub period_edit: Option<(f64, f64)>,
    /// Quel log est affiché (adresse de son `Arc`) : le changer vide les caches et recadre les graphiques.
    shown: Option<usize>,
    /// Recadrer en X sur tout le log au prochain cadre.
    fit: bool,
    series: HashMap<(&'static str, usize, usize), Arc<Series>>,
    present: HashMap<&'static str, bool>,
    modes: Vec<(f64, String)>,
    link: Id,
    /// Outil demandé par un bouton du panneau (son titre) ; lu une fois par la fenêtre.
    goto: Option<&'static str>,
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
            measure_on: false,
            measure: Vec::new(),
            playing: false,
            drags: [VDrag::default(); 4],
            period_edit: None,
            shown: None,
            fit: true,
            series: HashMap::new(),
            present: HashMap::new(),
            modes: Vec::new(),
            link: Id::unique(LINK),
            goto: None,
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

    /// Mémoire du cache de courbes : valeurs d'un côté, index multi-résolution de l'autre (octets). Sert au banc
    /// `otb-gui --example banc_cadre` à confronter le coût de l'index au pic de mémoire du processus (D-048).
    pub fn cache_bytes(&self) -> (usize, usize) {
        self.series.values().fold((0, 0), |(v, p), s| {
            (v + s.y.len() * std::mem::size_of::<f64>(), p + s.pyramid.bytes())
        })
    }

    /// Recadre les graphiques sur tout le log (« Vue entière »).
    pub fn fit_all(&mut self) {
        self.fit = true;
    }

    fn series(&mut self, key: &'static str, ax: usize, w: usize, make: impl FnOnce() -> Vec<f64>) -> Arc<Series> {
        self.series
            .entry((key, ax, w))
            .or_insert_with(|| Arc::new(Series::new(make())))
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
                let data = self.series(key, ax, w, || smooth(row, w));
                curves.push(Curve {
                    name: trace_label(key).into(),
                    color: th.trace_color(key),
                    width: self.width,
                    data,
                    dotted: false,
                });
            }
        }
        let mut motors = Vec::new();
        if self.motors {
            for m in 0..log.n_motors() {
                let data = self.series("motor", m, w, || smooth(log.motors.row(m), w));
                motors.push(Curve {
                    name: format!("Moteur {}", m + 1),
                    color: theme::motor_color(m),
                    width: self.width,
                    data,
                    dotted: false,
                });
            }
        }
        if self.throttle {
            let data = self.series("throttle", 0, 1, || log.throttle.iter().map(|&v| v as f64).collect());
            motors.push(Curve {
                name: "Gaz".into(),
                color: th.trace_color("throttle"),
                width: self.width + 1.0,
                data,
                dotted: false,
            });
        }
        let mut rps = Vec::new();
        let mut k = 1.0;
        if let (true, Some(hz)) = (self.rps, log.motor_hz.as_ref()) {
            let top = hz.data.iter().copied().filter(|v| !v.is_nan()).fold(0.0f32, f32::max) as f64 * 1.05;
            k = right_axis_scale(100.0, top);
            for m in 0..hz.k {
                let data = self.series("rps", m, w, || {
                    smooth(hz.row(m), w).into_iter().map(|v| v * k).collect()
                });
                rps.push(Curve {
                    name: format!("Rotation {} (Hz)", m + 1),
                    color: theme::motor_color(m),
                    width: self.width,
                    data,
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
            self.measure.clear();
            self.playing = false;
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
        self.advance(ui, state);
        egui::Panel::left("viewer-traces")
            .default_size(190.0)
            .show(ui, |ui| self.traces_panel(ui, state, th));
        egui::Panel::right("viewer-controls")
            .default_size(270.0)
            .size_range(240.0..=420.0)
            .show(ui, |ui| self.controls_panel(ui, state, th));
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

    fn controls_panel(&mut self, ui: &mut Ui, state: &mut AppState, th: &Theme) {
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
        self.measure_controls(ui, state, th);
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
        self.player(ui, state, th);
        ui.separator();
        if ui
            .button("Configuration")
            .on_hover_text("Infos de configuration (en-tête du log) : réglages des logs côte à côte")
            .clicked()
        {
            self.goto = Some("Configuration");
        }
    }

    /// Outil à ouvrir ensuite (son titre), demandé par un bouton du panneau.
    pub fn take_goto(&mut self) -> Option<&'static str> {
        self.goto.take()
    }

    fn measure_controls(&mut self, ui: &mut Ui, state: &AppState, th: &Theme) {
        ui.horizontal(|ui| {
            let was = self.measure_on;
            ui.toggle_value(&mut self.measure_on, "Mesurer").on_hover_text(
                "Mesure le temps entre deux points (ms) et la fréquence correspondante (Hz) : cliquez deux points sur un \
                 graphique roll, pitch ou yaw. Les repères se déplacent à la souris ; un nouveau clic recommence la mesure.",
            );
            if was != self.measure_on {
                self.measure.clear();
            }
            if !self.measure_on || state.current_log().is_none() {
                return;
            }
            match self.measure.len() {
                0 => ui.colored_label(th.ui.muted, "Cliquez un 1er point sur roll, pitch ou yaw."),
                1 => ui.colored_label(th.ui.muted, "Cliquez le 2e point."),
                _ => ui.colored_label(th.accent("purple"), measure_text(&self.measure)),
            };
        });
    }

    /// Lecture : le curseur avance en temps réel jusqu'à la fin du log.
    fn advance(&mut self, ui: &Ui, state: &mut AppState) {
        if !self.playing {
            return;
        }
        let Some(end) = state.current_log().map(|l| *l.log.t.last().unwrap()) else {
            self.playing = false;
            return;
        };
        if state.cursor >= end {
            self.playing = false;
            return;
        }
        let dt = ui.input(|i| i.stable_dt.min(0.1)) as f64;
        self.seek_rel(state, Some(dt), 0);
        ui.ctx().request_repaint();
    }

    /// Déplace le curseur : de `dt` secondes, ou (`dt` vide) au début (`end` < 0) ou à la fin (`end` > 0) de la période.
    pub fn seek_rel(&self, state: &mut AppState, dt: Option<f64>, end: i32) {
        let Some(ll) = state.current_log() else { return };
        let t = match dt {
            None if end < 0 => ll.period.0,
            None => ll.period.1,
            Some(d) => state.cursor + d,
        };
        let (lo, hi) = (ll.log.t[0], *ll.log.t.last().unwrap());
        state.set_cursor(t.clamp(lo, hi));
    }

    fn player(&mut self, ui: &mut Ui, state: &mut AppState, th: &Theme) {
        ui.add_space(6.0);
        ui.colored_label(th.ui.muted, "Lecteur");
        let button = |ui: &mut Ui, name: &str, tip: &str| {
            ui.add(egui::Button::image(
                icon(name, 16.0).tint(th.ui.text).alt_text(tip.to_string()),
            ))
            .on_hover_text(tip.to_string())
            .clicked()
        };
        ui.horizontal(|ui| {
            if button(ui, "skip-back", "Début de la période") {
                self.seek_rel(state, None, -1);
            }
            if button(ui, "chevron-left", "Reculer de 0,1 s") {
                self.seek_rel(state, Some(-0.1), 0);
            }
            let play = if self.playing { "pause" } else { "play" };
            if button(ui, play, "Lecture / pause") {
                self.playing = !self.playing;
            }
            if button(ui, "chevron-right", "Avancer de 0,1 s") {
                self.seek_rel(state, Some(0.1), 0);
            }
            if button(ui, "skip-forward", "Fin de la période") {
                self.seek_rel(state, None, 1);
            }
        });
        let Some(ll) = state.current_log() else {
            ui.label("temps : —");
            return;
        };
        let i = ll.log.index_of(state.cursor);
        ui.label(format!("temps : {:.3} s", ll.log.t[i]));
        let (left, right) = stick_positions(&ll.log, i);
        ui.horizontal(|ui| {
            stick(ui, th, "Y", "G", left);
            stick(ui, th, "R", "P", right);
        });
        ui.label(egui::RichText::new(readout_lines(&ll.log, i).join("\n")).monospace());
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

        // repères glissables, par priorité : points de mesure (absents = NaN, jamais attrapés), curseur, bords de période
        let m = |k: usize| {
            if self.measure_on {
                self.measure.get(k).copied().unwrap_or(f64::NAN)
            } else {
                f64::NAN
            }
        };
        let mut xs = [m(0), m(1), state.cursor, p0, p1];
        let before = xs;
        let press = ui.input(|i| i.pointer.press_origin()).map(|p| p.x);
        let fit = std::mem::take(&mut self.fit);
        let width_px = ui.available_width();
        let ymax = self.yscale as f64;
        let mut drags = self.drags;
        let mut transforms = self.transforms;
        let link = self.link;
        let t = &log.t;
        let measuring = self.measure_on;
        let result = measure_text(&self.measure);
        let purple = th.accent("purple");
        let mut clicks: Vec<f64> = Vec::new();

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
                            PlotPoints::from(window_points(t, &c.data.y, &c.data.pyramid, x0, x1, max_points)),
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
                    if measuring && !is_motor {
                        for &x in xs.iter().take(self.measure.len().min(2)) {
                            pu.vline(
                                VLine::new("Mesure", x)
                                    .color(purple)
                                    .width(2.0)
                                    .style(LineStyle::dashed_loose()),
                            );
                        }
                        if self.measure.len() == 2 {
                            let mid = (xs[0] + xs[1]) / 2.0;
                            pu.text(
                                Text::new("", PlotPoint::new(mid, ymax), result.as_str())
                                    .color(purple)
                                    .anchor(egui::Align2::CENTER_TOP),
                            );
                        }
                        if !busy
                            && pu.response().clicked()
                            && let Some(p) = pu.pointer_coordinate()
                        {
                            clicks.push(p.x);
                        }
                    }
                    pu.vline(
                        VLine::new("Curseur", xs[2])
                            .color(th.fg)
                            .style(LineStyle::dashed_loose()),
                    );
                    let limits = [(lo, hi); 5];
                    if is_motor {
                        // pas de repères de mesure sur ce graphique
                        let mut tmp = xs;
                        tmp[0] = f64::NAN;
                        tmp[1] = f64::NAN;
                        drag.update(pu, press, &mut tmp, &limits);
                        xs[2..].copy_from_slice(&tmp[2..]);
                    } else {
                        drag.update(pu, press, &mut xs, &limits);
                    }
                    *tr = Some(*pu.transform());
                });
        }
        self.drags = drags;
        self.transforms = transforms;

        for k in 0..self.measure.len().min(2) {
            if self.measure_on && xs[k] != before[k] {
                self.measure[k] = xs[k];
            }
        }
        // un clic place un point ; le 3e clic recommence la mesure
        if let Some(&x) = clicks.first() {
            if self.measure.len() == 1 {
                self.measure.push(x);
            } else {
                self.measure = vec![x];
            }
        }
        if xs[2] != before[2] {
            state.set_cursor(xs[2]);
        }
        if (xs[3], xs[4]) != (before[3], before[4]) {
            self.period_edit = Some((xs[3], xs[4]));
        }
        // relâché : la période est appliquée une seule fois (les outils recalculent)
        if self.drags.iter().all(|d| d.active.is_none())
            && let Some((a, b)) = self.period_edit.take()
        {
            state.set_period(index, a.min(b), a.max(b));
        }
    }
}

/// Position d'un manche : carré avec un point, `pos` de −1 à 1 (y = +1 en haut).
fn stick(ui: &mut Ui, th: &Theme, x_label: &str, y_label: &str, pos: [f32; 2]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 96.0), egui::Sense::hover());
    let side = rect.width() - 20.0;
    let sq = egui::Rect::from_min_size(rect.min + egui::vec2(14.0, 2.0), egui::vec2(side, side));
    let p = ui.painter();
    p.rect_stroke(sq, 0.0, egui::Stroke::new(1.0, th.ui.text), egui::StrokeKind::Inside);
    let mid = egui::Stroke::new(1.0, th.grid_color());
    p.line_segment([sq.center_top(), sq.center_bottom()], mid);
    p.line_segment([sq.left_center(), sq.right_center()], mid);
    let c = egui::pos2(
        sq.left() + (pos[0] + 1.0) / 2.0 * side,
        sq.top() + (1.0 - (pos[1] + 1.0) / 2.0) * side,
    );
    p.circle_stroke(c, 5.0, egui::Stroke::new(2.0, th.ui.text));
    let font = egui::FontId::proportional(11.0);
    p.text(
        sq.left_center() - egui::vec2(4.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        y_label,
        font.clone(),
        th.ui.text,
    );
    p.text(
        sq.center_bottom() + egui::vec2(0.0, 2.0),
        egui::Align2::CENTER_TOP,
        x_label,
        font,
        th.ui.text,
    );
}
