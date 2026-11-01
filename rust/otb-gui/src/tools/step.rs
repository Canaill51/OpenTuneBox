//! Outil « Réponse indicielle » (port de `gui/tools/step_window.py`, équivalent du Step Response Tool de PIDtoolbox).
//!
//! À gauche, une ligne par axe (yaw masqué au départ) : réponse indicielle moyenne de chaque fichier (couleur du fichier), bande ± écart-type
//! et réponses brutes en option, point creux à l'arrêt de la montée quand il y a une cassure, et dans le graphique le bloc
//! « P, I, D, Dm, F (n=…) · pic · latence · cassure ». Au milieu, deux colonnes de métriques au choix (une valeur par fichier = « test » 1…N,
//! barres d'erreur = dispersion entre fenêtres pour le pic et la latence, écart-type jackknife pour les marges et la cassure ; vols conseillés
//! entourés). Marges de stabilité : point plein = boucle ouverte mesurée, point creux = méthode approchée, triangle = minorant (pic de
//! sensibilité au bord de la bande mesurable). À droite, le « Diagnostic du réglage » (`step_tuning`) et, dessous, les curseurs Betaflight.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use egui::{Align2, Color32, RichText, Ui};
use egui_plot::{FilledArea, HLine, Line, LineStyle, MarkerShape, Plot, PlotPoint, PlotPoints, Points};
use otb_advisor::step_tuning::{RIPPLE_EARLY, TuningReport, step_tuning};
use otb_analysis::open_loop;

use crate::plots::{corner_text, nice_axes, swatch};
use crate::state::AppState;
use crate::theme::{AXIS_NAMES, Theme, plot, with_alpha};
use crate::tool::{LogInput, Tool};
use crate::tools::common::{AxisToggles, FileSelection, safe_text};
use crate::tools::report_ui::finding_ui;
use crate::tools::step_common::{
    FileData, FileStep, Split, StepOpts, compute_file_data, dot, mode_text, pid_text, slider_plot,
};

/// (clé, titre de la liste, libellé d'axe).
pub const METRICS: [(&str, &str, &str); 11] = [
    ("peak", "Pic", "Pic"),
    ("latency", "Latence (50 %)", "Latence (ms)"),
    ("sp_delay", "Retard consigne→gyro", "Retard consigne→gyro (ms)"),
    ("rise", "Temps de montée", "Montée 10–90 % (ms)"),
    ("ripple", "Cassure (ripple)", "Cassure"),
    ("bandwidth", "Bande passante", "Bande passante (Hz)"),
    ("sens_peak", "Pic de sensibilité", "Pic de sensibilité (dB)"),
    ("sens_hz", "Fréq. du pic de sensibilité", "Fréq. pic sensibilité (Hz)"),
    ("phase_margin", "Marge de phase", "Marge de phase (°)"),
    ("gain_margin", "Marge de gain", "Marge de gain (dB)"),
    ("crossover", "Fréquence de croisement", "Fréq. de croisement (Hz)"),
];

/// Champ de `LoopMetrics` lu par une métrique de boucle.
fn loop_field(key: &str) -> Option<&'static str> {
    Some(match key {
        "sens_peak" => "sensitivity_peak_db",
        "sens_hz" => "sensitivity_peak_hz",
        "phase_margin" => "phase_margin_deg",
        "gain_margin" => "gain_margin_db",
        "crossover" => "crossover_hz",
        _ => return None,
    })
}

/// Réponses brutes dessinées au plus par fichier et par axe.
pub const MAX_RAW: usize = 300;
/// Points gardés par réponse brute (la courbe est plus fine que l'écran).
const RAW_POINTS: usize = 250;
/// Largeur du rapport « Diagnostic du réglage » à son ouverture.
const REPORT_WIDTH: f32 = 400.0;
/// Durées affichées (ms) : 150 pour bien voir la montée et la cassure.
pub const X_RANGES: [u32; 3] = [500, 250, 150];
pub const X_DEFAULT: u32 = 250;
const SEG_CHOICES: [f64; 3] = [1.0, 2.0, 3.0];
const SMOOTH_CHOICES: [i64; 4] = [0, 20, 40, 60];

/// Réponses brutes prêtes à tracer : (fichier, axe) → courbes.
type RawCache = HashMap<(usize, usize), Vec<Vec<PlotPoint>>>;

static STAMP: AtomicU64 = AtomicU64::new(1);

/// Forme du point d'une marge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    /// Boucle ouverte mesurée (point plein).
    Measured,
    /// Méthode approchée (point creux).
    Approx,
    /// Pic de sensibilité au bord de la bande mesurable : au moins cette valeur (triangle).
    Lower,
}

pub struct StepOutput {
    pub files: Vec<FileStep>,
    pub report: TuningReport,
    /// Change à chaque calcul : les bornes des graphiques sont remises (comme `render` de Python qui refait tout).
    pub stamp: u64,
}

/// Écart-type des valeurs finies (0 s'il y en a moins de deux) : dispersion entre fenêtres.
pub fn spread(v: &[f64]) -> f64 {
    let f: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if f.len() < 2 {
        return 0.0;
    }
    let m = f.iter().sum::<f64>() / f.len() as f64;
    (f.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / f.len() as f64).sqrt()
}

/// `(valeur, demi-barre d'erreur)` d'une métrique pour un fichier et un axe ; NaN s'il n'y a pas de réponse.
pub fn metric_value(d: &FileData, ax: usize, key: &str) -> (f64, f64) {
    let Some(a) = d.axes.get(ax) else {
        return (f64::NAN, 0.0);
    };
    if a.res.n == 0 {
        return (f64::NAN, 0.0);
    }
    let (m, lp) = (&a.res.metrics, &a.res.loop_);
    match key {
        "peak" => (m.peak, spread(&a.seg_peak)),
        "latency" => (m.latency_ms, spread(&a.seg_latency)),
        "sp_delay" => (a.sp_delay_ms, 0.0),
        "rise" => (m.rise_ms, 0.0),
        "ripple" => (m.ripple, if m.ripple_std.is_finite() { m.ripple_std } else { 0.0 }),
        "bandwidth" => (lp.bandwidth_hz, 0.0),
        _ => {
            let field = loop_field(key).unwrap_or("sensitivity_peak_db");
            let err = lp.std.iter().find(|(k, _)| *k == field).map_or(0.0, |(_, v)| *v);
            let v = match field {
                "sensitivity_peak_db" => lp.sensitivity_peak_db,
                "sensitivity_peak_hz" => lp.sensitivity_peak_hz,
                "phase_margin_deg" => lp.phase_margin_deg,
                "gain_margin_db" => lp.gain_margin_db,
                _ => lp.crossover_hz,
            };
            (v, if err.is_finite() { err } else { 0.0 })
        }
    }
}

/// Forme du point d'une marge (`loop_marker`).
pub fn loop_marker(d: &FileData, ax: usize, key: &str) -> Marker {
    let Some(a) = d.axes.get(ax) else {
        return Marker::Measured;
    };
    if loop_field(key).is_none() || a.res.n == 0 {
        return Marker::Measured;
    }
    let lp = &a.res.loop_;
    if lp.method != open_loop::MEASURED {
        return Marker::Approx;
    }
    if key == "sens_peak" && lp.sensitivity_peak_at_edge {
        return Marker::Lower;
    }
    Marker::Measured
}

/// Bornes Y d'une colonne de métriques : étendue des valeurs ± barres d'erreur, plus un quart de marge de chaque côté.
pub fn metric_y_range(ys: &[f64], err: &[f64], key: &str) -> Option<(f64, f64)> {
    let ok: Vec<usize> = (0..ys.len()).filter(|&i| ys[i].is_finite()).collect();
    if ok.is_empty() {
        return None;
    }
    let mut lo = ok.iter().map(|&i| ys[i] - err[i]).fold(f64::INFINITY, f64::min);
    let mut hi = ok.iter().map(|&i| ys[i] + err[i]).fold(f64::NEG_INFINITY, f64::max);
    if key == "sens_peak" {
        lo = lo.min(0.0);
        hi = hi.max(7.0);
    }
    let span = (hi - lo).max(0.1 * hi.abs().max(1e-3)).max(0.02);
    Some((lo - 0.25 * span, hi + 0.25 * span))
}

/// Un nombre à `n` décimales avec la virgule (`f"{x:.2f}".replace(".", ",")`).
fn fr(x: f64, n: usize) -> String {
    if x.is_nan() {
        return "nan".to_string(); // comme `f"{nan:.2f}"` de Python
    }
    format!("{x:.n$}").replace('.', ",")
}

/// Bloc par fichier, en bas à droite du graphique (zone sans courbe) : gains, pic, latence, cassure.
pub fn text_lines(files: &[FileStep], ax: usize, theme: &Theme) -> Vec<(String, Color32)> {
    let mut rows = vec![("P, I, D, Dm, F · pic · latence · cassure".to_string(), theme.fg)];
    for (i, f) in files.iter().enumerate() {
        let d = &f.data;
        let txt = match d.axes.get(ax) {
            Some(a) if a.res.n > 0 => {
                let m = &a.res.metrics;
                let rip = if m.ripple.is_finite() {
                    format!(" · {}", format!("{:+.3}", m.ripple).replace('.', ","))
                } else {
                    String::new()
                };
                format!(
                    "{} (n={}) · {} · {} ms{rip}",
                    pid_text(&d.pids[ax]),
                    a.res.n,
                    fr(m.peak, 2),
                    fr(m.latency_ms, 1)
                )
            }
            _ => "données insuffisantes".to_string(),
        };
        rows.push((format!("{}) {txt}", i + 1), f.color));
    }
    rows
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub calc: StepOpts,
}

pub struct StepTool {
    calc: StepOpts,
    ymax: f64,
    /// 0 = non, 1 à 3 = un seul axe en grand.
    expand: usize,
    xmax: u32,
    raw: bool,
    sd: bool,
    metric1: usize,
    metric2: usize,
    axes: AxisToggles,
    files: FileSelection,
    /// Empreinte des réglages d'affichage et du calcul : les bornes ne sont remises que si elle change.
    view: Option<u64>,
    /// Réponses brutes prêtes à tracer, par (fichier, axe), pour le résultat `stamp` : refaites seulement si le résultat change.
    raw_cache: (u64, RawCache),
}

impl Default for StepTool {
    fn default() -> Self {
        Self::new()
    }
}

impl StepTool {
    pub fn new() -> Self {
        Self {
            calc: StepOpts::default(),
            ymax: 1.5,
            expand: 0,
            xmax: X_DEFAULT,
            raw: false,
            sd: false,
            metric1: 1,                             // latence
            metric2: 2,                             // retard consigne → gyro
            axes: AxisToggles([true, true, false]), // yaw : ni jugé par le diagnostic, ni souvent mesurable
            files: FileSelection::new(true),
            view: None,
            raw_cache: (0, HashMap::new()),
        }
    }

    pub fn set_calc(&mut self, calc: StepOpts) {
        self.calc = calc;
    }

    pub fn set_display(&mut self, raw: bool, sd: bool) {
        (self.raw, self.sd) = (raw, sd);
    }

    pub fn set_metrics(&mut self, m1: &str, m2: &str) {
        let find = |k: &str| METRICS.iter().position(|m| m.0 == k).unwrap_or(0);
        (self.metric1, self.metric2) = (find(m1), find(m2));
    }

    pub fn set_expand(&mut self, expand: usize) {
        self.expand = expand.min(3);
    }

    pub fn set_xmax(&mut self, xmax: u32) {
        self.xmax = xmax;
    }

    pub fn set_ymax(&mut self, ymax: f64) {
        self.ymax = ymax.clamp(0.5, 5.0);
    }

    /// Axes dessinés : un seul si « Agrandir », sinon ceux qui sont cochés.
    pub fn shown_axes(&self) -> Vec<usize> {
        if self.expand > 0 {
            vec![self.expand - 1]
        } else {
            self.axes.enabled()
        }
    }

    /// Empreinte de ce qui décide des bornes des graphiques : elles ne sont remises que si elle change.
    pub fn view_key(&self, stamp: u64) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (
            self.ymax.to_bits(),
            self.xmax,
            self.expand,
            self.metric1,
            self.metric2,
            stamp,
        )
            .hash(&mut h);
        self.axes.0.hash(&mut h);
        h.finish()
    }

    // ---------------------------------------------------------------- graphiques

    fn draw_step(&self, ui: &mut Ui, out: &StepOutput, ax: usize, size: [f32; 2], fresh: bool, theme: &Theme) {
        let xmax = f64::from(self.xmax);
        let ymax = self.ymax;
        let resp = nice_axes(Plot::new(("step-r", ax)))
            .width(size[0])
            .height(size[1])
            .auto_bounds(false)
            .x_axis_label("Temps (ms)")
            .y_axis_label(format!("{} — réponse", AXIS_NAMES[ax]))
            .grid_color(theme.grid_color())
            .show(ui, |pu| {
                if fresh {
                    pu.set_plot_bounds_x(-0.025 * xmax..=1.025 * xmax);
                    pu.set_plot_bounds_y(-0.04 * ymax..=1.04 * ymax);
                }
                pu.hline(
                    HLine::new("", 1.0)
                        .color(theme.fg)
                        .width(1.0)
                        .style(LineStyle::dashed_loose()),
                );
                let valid: Vec<&FileStep> = out
                    .files
                    .iter()
                    .filter(|f| f.data.axes.get(ax).is_some_and(|a| a.res.n > 0))
                    .collect();
                // 1) réponses brutes, 2) bandes ± écart-type, 3) moyennes : les moyennes restent lisibles
                if self.raw {
                    for (fi, f) in out.files.iter().enumerate() {
                        let color = with_alpha(f.color, 25);
                        for seg in self.raw_cache.1.get(&(fi, ax)).into_iter().flatten() {
                            pu.line(Line::new("", PlotPoints::Borrowed(seg)).color(color).width(1.0));
                        }
                    }
                }
                if self.sd {
                    for f in &valid {
                        let r = &f.data.axes[ax].res;
                        let lo: Vec<f64> = r.mean.iter().zip(&r.std).map(|(m, s)| m - s).collect();
                        let hi: Vec<f64> = r.mean.iter().zip(&r.std).map(|(m, s)| m + s).collect();
                        pu.add(FilledArea::new("", &r.t_ms, &lo, &hi).fill_color(with_alpha(f.color, 50)));
                    }
                }
                for f in &valid {
                    let r = &f.data.axes[ax].res;
                    let pts: Vec<[f64; 2]> = r.t_ms.iter().zip(&r.mean).map(|(t, m)| [*t, *m]).collect();
                    if self.raw {
                        // liseré sombre pour détacher la moyenne du nuage
                        pu.line(Line::new("", PlotPoints::from(pts.clone())).color(theme.fg).width(4.5));
                    }
                    pu.line(Line::new("", PlotPoints::from(pts)).color(f.color).width(2.5));
                    let m = &r.metrics;
                    if m.ripple.is_finite() && m.ripple >= RIPPLE_EARLY {
                        // arrêt de la montée : la cassure
                        let y = interp(&r.t_ms, &r.mean, m.ripple_ms);
                        pu.points(
                            Points::new("", PlotPoints::from(vec![[m.ripple_ms, y]]))
                                .color(f.color)
                                .radius(5.0)
                                .filled(false),
                        );
                    }
                }
            });
        corner_text(
            ui,
            *resp.transform.frame(),
            Align2::RIGHT_BOTTOM,
            &text_lines(&out.files, ax, theme),
            theme.legend_fill,
        );
    }

    /// `cell` : (axe, colonne, clé de la métrique, taille du graphique).
    fn draw_metric(
        &self,
        ui: &mut Ui,
        out: &StepOutput,
        cell: (usize, usize, &str, [f32; 2]),
        fresh: bool,
        theme: &Theme,
    ) {
        let (ax, col, key, size) = cell;
        let unit = safe_text(METRICS.iter().find(|m| m.0 == key).map_or("", |m| m.2));
        let n = out.files.len();
        let (mut ys, mut err) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for f in &out.files {
            let (v, e) = metric_value(&f.data, ax, key);
            ys.push(v);
            err.push(e);
        }
        let best: std::collections::BTreeSet<usize> = out.report.blocks.iter().filter_map(|b| b.best).collect();
        let range = metric_y_range(&ys, &err, key);
        Plot::new(("step-m", ax, col)) // la colonne, pas la métrique : deux colonnes peuvent montrer la même
            .width(size[0])
            .height(size[1])
            .auto_bounds(false)
            .allow_drag([false, true])
            .allow_zoom([false, true])
            .allow_scroll(false)
            .x_axis_label("Test")
            .y_axis_label(format!("{} — {unit}", AXIS_NAMES[ax]))
            .grid_color(theme.grid_color())
            .x_grid_spacer(move |_| {
                (1..=n)
                    .map(|i| egui_plot::GridMark {
                        value: i as f64,
                        step_size: 1.0,
                    })
                    .collect()
            })
            .x_axis_formatter(|m, _| {
                if (m.value - m.value.round()).abs() < 1e-6 {
                    format!("{}", m.value.round() as i64)
                } else {
                    String::new()
                }
            })
            .y_grid_spacer(|i| crate::plots::nice_grid(i, 3.0))
            .show(ui, |pu| {
                if fresh {
                    pu.set_plot_bounds_x(0.5..=n as f64 + 0.5);
                    if let Some((lo, hi)) = range {
                        pu.set_plot_bounds_y(lo..=hi);
                    }
                }
                if key == "sens_peak" {
                    pu.hline(HLine::new("", 4.0).color(plot::LIMIT_WARN).width(2.0));
                    pu.hline(HLine::new("", 6.0).color(plot::LIMIT_ALERT).width(2.0));
                }
                if key == "peak" {
                    pu.hline(
                        HLine::new("", 1.0)
                            .color(theme.fg)
                            .width(1.0)
                            .style(LineStyle::dashed_loose()),
                    );
                }
                for (i, f) in out.files.iter().enumerate() {
                    if !ys[i].is_finite() {
                        continue;
                    }
                    let x = (i + 1) as f64;
                    if err[i] > 0.0 {
                        for seg in [
                            vec![[x, ys[i] - err[i]], [x, ys[i] + err[i]]],
                            vec![[x - 0.06, ys[i] - err[i]], [x + 0.06, ys[i] - err[i]]],
                            vec![[x - 0.06, ys[i] + err[i]], [x + 0.06, ys[i] + err[i]]],
                        ] {
                            pu.line(Line::new("", PlotPoints::from(seg)).color(f.color).width(1.5));
                        }
                    }
                    let kind = loop_marker(&f.data, ax, key);
                    let p = Points::new("", PlotPoints::from(vec![[x, ys[i]]]))
                        .color(f.color)
                        .radius(5.5);
                    pu.points(match kind {
                        Marker::Lower => p.shape(MarkerShape::Up).filled(true),
                        Marker::Approx => p.filled(false),
                        Marker::Measured => p.filled(true),
                    });
                    if best.contains(&i) {
                        // vol conseillé par le diagnostic
                        pu.points(
                            Points::new("", PlotPoints::from(vec![[x, ys[i]]]))
                                .color(theme.fg)
                                .radius(12.0)
                                .filled(false),
                        );
                    }
                }
            });
    }

    fn plots(&mut self, ui: &mut Ui, out: &StepOutput, theme: &Theme) {
        let axes = self.shown_axes();
        ui.horizontal(|ui| {
            ui.label("Colonne 1 :");
            metric_combo(ui, "step-col1", &mut self.metric1, "Colonne 1");
            ui.add_space(24.0);
            ui.label("Colonne 2 :");
            metric_combo(ui, "step-col2", &mut self.metric2, "Colonne 2");
        });
        if out.files.is_empty() || axes.is_empty() {
            return;
        }
        if self.raw {
            if self.raw_cache.0 != out.stamp {
                self.raw_cache = (out.stamp, HashMap::new());
            }
            for &ax in &axes {
                for (fi, f) in out.files.iter().enumerate() {
                    self.raw_cache.1.entry((fi, ax)).or_insert_with(|| raw_segments(f, ax));
                }
            }
        }
        let key = self.view_key(out.stamp);
        let fresh = self.view != Some(key);
        let sp = ui.spacing().item_spacing;
        let usable = (ui.available_width() - 2.0 * sp.x).max(300.0);
        let (w1, w2) = (usable * 5.0 / 9.0, usable * 2.0 / 9.0);
        let rows = axes.len() as f32;
        let h = ((ui.available_height() - 22.0 - sp.y * rows) / rows).max(120.0);
        ui.horizontal(|ui| {
            ui.add_sized(
                [w1, 18.0],
                egui::Label::new(RichText::new("Réponse indicielle").strong()),
            );
            for m in [self.metric1, self.metric2] {
                ui.add_sized([w2, 18.0], egui::Label::new(RichText::new(metric_title(m)).strong()));
            }
        });
        let keys = [METRICS[self.metric1].0, METRICS[self.metric2].0];
        for &ax in &axes {
            ui.horizontal(|ui| {
                self.draw_step(ui, out, ax, [w1, h], fresh, theme);
                for (col, k) in keys.into_iter().enumerate() {
                    self.draw_metric(ui, out, (ax, col, k, [w2, h]), fresh, theme);
                }
            });
        }
        self.view = Some(key);
    }

    // ---------------------------------------------------------------- diagnostic du réglage

    fn side(&self, ui: &mut Ui, out: &StepOutput, theme: &Theme) {
        let total = ui.available_height();
        let slider_h = (total * 0.42).clamp(280.0, 520.0);
        // 120 px réservés : titre, légende des modes (deux lignes) et séparateur
        let report_h = (total - slider_h - 120.0).max(100.0);
        egui::ScrollArea::vertical()
            .id_salt("step-rapport")
            .max_height(report_h)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                report_ui(ui, theme, &out.report, &out.files);
            });
        ui.separator();
        let files: Vec<(Color32, &otb_analysis::sliders::SliderView)> =
            out.files.iter().map(|f| (f.color, &f.data.sliders)).collect();
        slider_plot(
            ui,
            "step-curseurs",
            &files,
            None,
            "Curseurs PID Betaflight",
            theme,
            slider_h,
        );
        modes_legend(ui, theme, &out.files);
    }
}

/// Valeur de `y(x)` par interpolation linéaire (`np.interp`), bornée aux extrémités.
pub fn interp(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    if xs.is_empty() {
        return f64::NAN;
    }
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[xs.len() - 1] {
        return ys[ys.len() - 1];
    }
    let j = xs.partition_point(|&v| v <= x);
    let (x0, x1, y0, y1) = (xs[j - 1], xs[j], ys[j - 1], ys[j]);
    y0 + (y1 - y0) * (x - x0) / (x1 - x0)
}

/// Réponses brutes d'un fichier, prêtes à tracer : au plus `MAX_RAW` lignes régulièrement espacées, chacune réduite à `RAW_POINTS` points
/// et coupée aux valeurs non finies (`connect="finite"` de pyqtgraph). Calculé une fois par résultat, pas à chaque cadre.
pub fn raw_segments(f: &FileStep, ax: usize) -> Vec<Vec<PlotPoint>> {
    let Some(a) = f.data.axes.get(ax) else {
        return Vec::new();
    };
    let r = &a.res;
    let len = r.t_ms.len();
    if len == 0 || r.n == 0 {
        return Vec::new();
    }
    let rows: Vec<usize> = if r.n > MAX_RAW {
        // np.linspace(0, n - 1, MAX_RAW).astype(int)
        (0..MAX_RAW)
            .map(|i| (i as f64 * (r.n - 1) as f64 / (MAX_RAW - 1) as f64) as usize)
            .collect()
    } else {
        (0..r.n).collect()
    };
    let step = len.div_ceil(RAW_POINTS).max(1);
    let mut out = Vec::with_capacity(rows.len());
    for i in rows {
        let row = &r.responses[i * len..(i + 1) * len];
        let mut seg: Vec<PlotPoint> = Vec::new();
        for k in (0..len).step_by(step) {
            if row[k].is_finite() {
                seg.push(PlotPoint::new(r.t_ms[k], row[k]));
            } else if !seg.is_empty() {
                out.push(std::mem::take(&mut seg)); // la courbe est coupée à une valeur non finie
            }
        }
        if !seg.is_empty() {
            out.push(seg);
        }
    }
    out
}

/// Titre d'une métrique prêt à afficher (« → » absent des polices d'egui : L-068).
pub fn metric_title(i: usize) -> String {
    safe_text(METRICS[i].1)
}

fn metric_combo(ui: &mut Ui, id: &str, current: &mut usize, name: &str) {
    egui::ComboBox::from_id_salt(id)
        .height(320.0) // les 11 choix sans ascenseur (200 px par défaut : les derniers sortaient de la liste)
        .selected_text(metric_title(*current))
        .show_ui(ui, |ui| {
            for i in 0..METRICS.len() {
                ui.selectable_value(current, i, metric_title(i));
            }
        })
        .response
        .accessible_name(name);
}

/// Texte du « Diagnostic du réglage » (`_tuning_html`).
fn report_ui(ui: &mut Ui, theme: &Theme, rep: &TuningReport, files: &[FileStep]) {
    ui.heading("Diagnostic du réglage");
    ui.label(RichText::new("Conditions des vols").strong().size(15.0));
    if rep.conditions.is_empty() {
        ui.label("Feedforward à 0 ou mode Angle, curseur I bas, assez de mouvements : vols exploitables.");
    }
    for f in &rep.conditions {
        finding_ui(ui, f);
    }
    if rep.blocks.is_empty() {
        ui.label(
            "Aucune série reconnue : chargez plusieurs vols du même drone, mêmes filtres, où un seul curseur change \
             (Damping, Master, Pitch Tracking ou Pitch Damping).",
        );
    }
    for b in &rep.blocks {
        ui.add_space(6.0);
        ui.label(RichText::new(safe_text(&b.title)).strong().size(15.0));
        for l in &b.lines {
            ui.label(safe_text(&format!("  •  {l}")));
        }
        if let Some(f) = &b.finding {
            finding_ui(ui, f);
        }
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label("Vols :");
        for f in files {
            swatch(ui, f.color);
            ui.label(&f.label);
        }
    });
    ui.label(
        RichText::new(
            "Dans les colonnes de mesures, un cercle entoure les vols conseillés ; sur les courbes, un point creux marque \
             l'arrêt de la montée quand il y a une cassure.\nOrdre de la méthode : Damping avec Master 1,0, puis Master, \
             puis roll / pitch, puis affinage. Cassure (« ripple ») : la montée s'arrête avant la consigne puis reprend \
             lentement ; vagues : oscillation sur le palier, comparée au bruit de mesure (≈ 1 : bruit).",
        )
        .color(theme.ui.muted),
    );
}

/// Légende courte sous les curseurs : mode du réglage simplifié par fichier ; « (!) » si les PID ne correspondent pas aux curseurs.
fn modes_legend(ui: &mut Ui, theme: &Theme, files: &[FileStep]) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("MODE").strong());
        for (i, f) in files.iter().enumerate() {
            let warn = if f.data.sliders.consistent { "" } else { " (!)" };
            ui.label(RichText::new(format!("{}) {}{warn}", i + 1, mode_text(f.data.mode))).color(f.color));
        }
    });
    ui.horizontal(|ui| {
        dot(ui, theme.ui.muted, true);
        ui.label(RichText::new("curseurs de l'en-tête").color(theme.ui.muted));
        dot(ui, theme.ui.muted, false);
        ui.label(RichText::new("estimés depuis les PID").color(theme.ui.muted));
    });
}

impl Tool for StepTool {
    type Options = Options;
    type Output = StepOutput;
    const TITLE: &'static str = "Réponse indicielle";

    fn options(&self) -> Options {
        Options { calc: self.calc }
    }

    fn selected(&self, state: &AppState) -> Vec<usize> {
        self.files.indices(state)
    }

    fn compute(o: Options, logs: Vec<LogInput>) -> Result<StepOutput, String> {
        let mut files = Vec::with_capacity(logs.len());
        for li in &logs {
            // la clé contient les réglages de calcul ET la période : un résultat calculé avant un changement de période ne peut pas servir
            let key = format!("step:{:?}:{:?}", o.calc, li.period);
            let data = match li.cache.get::<FileData>(&key) {
                Some(d) => d,
                None => {
                    let sel = li.log.slice_time(li.period.0, li.period.1);
                    let d = Arc::new(compute_file_data(&li.log, sel, &o.calc)?);
                    li.cache.put(key, d.clone());
                    d
                }
            };
            // couleur et nom à jour (le fichier a pu changer de rang)
            files.push(FileStep {
                label: li.label.clone(),
                color: li.color,
                data,
            });
        }
        let flights: Vec<_> = files
            .iter()
            .map(|f| {
                let mut fl = f.data.flight.clone();
                fl.label = f.label.clone();
                fl
            })
            .collect();
        Ok(StepOutput {
            report: step_tuning(&flights)?,
            files,
            stamp: STAMP.fetch_add(1, Ordering::Relaxed),
        })
    }

    fn controls(&mut self, ui: &mut Ui, state: &AppState, theme: &Theme) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.files.ui(ui, state);
            ui.separator();
            self.axes.ui(ui);
            ui.separator();
            ui.label(RichText::new("Affichage").strong());
            egui::Grid::new("step-affichage").show(ui, |ui| {
                ui.label("Y max");
                ui.add(egui::DragValue::new(&mut self.ymax).range(0.5..=5.0).speed(0.05).max_decimals(1))
                    .accessible_name("Y max");
                ui.end_row();
                ui.label("Agrandir");
                egui::ComboBox::from_id_salt("step-agrandir")
                    .selected_text(["non", "Roll", "Pitch", "Yaw"][self.expand])
                    .show_ui(ui, |ui| {
                        for (i, l) in ["non", "Roll", "Pitch", "Yaw"].iter().enumerate() {
                            ui.selectable_value(&mut self.expand, i, *l);
                        }
                    })
                    .response
                    .accessible_name("Agrandir");
                ui.end_row();
                ui.label("Temps affiché");
                egui::ComboBox::from_id_salt("step-xmax")
                    .selected_text(format!("{} ms", self.xmax))
                    .show_ui(ui, |ui| {
                        for ms in X_RANGES {
                            ui.selectable_value(&mut self.xmax, ms, format!("{ms} ms"));
                        }
                    })
                    .response
                    .accessible_name("Temps affiché")
                    .on_hover_text("150 ms : la montée en grand, pour voir la cassure (« ripple »).");
                ui.end_row();
            });
            ui.checkbox(&mut self.raw, "Brutes (toutes les fenêtres)");
            ui.checkbox(&mut self.sd, "± écart-type");
            ui.separator();
            ui.label(RichText::new("Calcul").strong());
            egui::Grid::new("step-calcul").show(ui, |ui| {
                ui.label("Lissage gyro");
                egui::ComboBox::from_id_salt("step-lissage")
                    .selected_text(smooth_label(self.calc.smooth))
                    .show_ui(ui, |ui| {
                        for n in SMOOTH_CHOICES {
                            ui.selectable_value(&mut self.calc.smooth, n, smooth_label(n));
                        }
                    })
                    .response
                    .accessible_name("Lissage gyro");
                ui.end_row();
                ui.label("Fenêtre déconv.");
                egui::ComboBox::from_id_salt("step-fenetre")
                    .selected_text(format!("{} s", self.calc.seg_s))
                    .show_ui(ui, |ui| {
                        for s in SEG_CHOICES {
                            ui.selectable_value(&mut self.calc.seg_s, s, format!("{s} s"));
                        }
                    })
                    .response
                    .accessible_name("Fenêtre déconv.");
                ui.end_row();
                ui.label("Consigne max");
                egui::ComboBox::from_id_salt("step-split")
                    .selected_text(self.calc.split.label())
                    .show_ui(ui, |ui| {
                        for s in Split::ALL {
                            ui.selectable_value(&mut self.calc.split, s, s.label());
                        }
                    })
                    .response
                    .accessible_name("Consigne max");
                ui.end_row();
            });
            ui.checkbox(&mut self.calc.y_corr, "Y corr. (recentrer sur 1)");
            ui.separator();
            ui.label(
                RichText::new(
                    "Courbe idéale : montée rapide, pic proche de 1 puis plat (sans feedforward, environ 1,03 ; le feedforward \
                     fait monter le pic). Pic marqué : manque de D (ou trop de P) ; montée molle sans pic : trop de D ou P \
                     faible. Barres d'erreur : dispersion entre fenêtres de 2 s. Marges de stabilité : point plein = mesurées \
                     sur les termes P, I, D, F du log ; point creux = approchées (termes absents) ; triangle = au moins cette \
                     valeur (pic hors de la bande mesurable).",
                )
                .color(theme.ui.muted),
            );
        });
    }

    fn render(&mut self, ui: &mut Ui, _state: &mut AppState, out: &StepOutput, theme: &Theme) {
        if !out.files.is_empty() {
            egui::Panel::right("step-diagnostic")
                .default_size(REPORT_WIDTH)
                .size_range(320.0..=900.0)
                .show(ui, |ui| self.side(ui, out, theme));
        }
        egui::CentralPanel::default().show(ui, |ui| self.plots(ui, out, theme));
    }
}

fn smooth_label(n: i64) -> String {
    if n == 0 {
        "aucun".to_string()
    } else {
        format!("{n} échantillons")
    }
}
