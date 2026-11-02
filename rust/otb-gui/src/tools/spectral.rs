//! Analyse spectrale (port de `gui/tools/spectral_window.py`, l'équivalent du « Spectral Analyzer » de PIDtoolbox).
//!
//! Une ligne de graphiques par axe (Roll, Pitch, Yaw) : à gauche le spectre complet (0 → Nyquist), à droite, au choix,
//! le détail sous 100 Hz (avec les retards des filtres) ou le « bruit moteur » (niveau aux harmoniques 1, 2, 3 des moteurs).
//! Les courbes en « S » montrent où se situent les harmoniques moteur (et les notchs dynamiques) pendant la période.
//! La case « Diagnostic du bruit » ajoute un rapport à côté des courbes, calculé seulement quand elle est cochée.

use std::collections::BTreeMap;

use egui::{Align2, RichText, Ui};
use egui_plot::{Legend, Line, LineStyle, Plot, PlotPoints, Points, Span};
use otb_advisor::noise::{NoiseReport, noise_report};
use otb_analysis::datasets::{self, DATASETS, is_prefilt};
use otb_analysis::delay::compute_delays;
use otb_analysis::motors::{dyn_notch_centres, motor_noise};
use otb_analysis::npstat::percentile_f32;
use otb_analysis::spectral::{Mode, smooth_freq, spectrum, to_ptb_db};

use super::common::{
    AxisToggles, FileSelection, MotorToggles, ViewMemo, cdf, fmt_ms, harmonic_style, motor_freqs, rpm_filter_text,
    sel_rows, sorted_above_zero, sorted_positive, wide,
};
use super::report_ui::{REPORT_FOOTER, diagnosis_ui};
use crate::plots::{corner_text, error_bars, nice_axes};
use crate::state::AppState;
use crate::theme::{AXIS_NAMES, Theme, motor_color, plot, with_alpha};
use crate::tool::{LogInput, Tool};

pub const RESOLUTIONS: [(f64, &str); 4] = [(0.5, "0,5 Hz"), (1.0, "1 Hz"), (2.0, "2 Hz"), (4.0, "4 Hz")];
/// Jeux de données cochés au départ.
pub const DEFAULT_DATASETS: [&str; 2] = ["gyro", "gyro_prefilt"];
const REPORT_WIDTH: f32 = 460.0;

/// Harmoniques moteur tracées en courbes en « S » (`HARMONIC_CHOICES`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Harmonics {
    None,
    First,
    Three,
    /// Plage couverte par le filtre RPM.
    Rpm,
}

impl Harmonics {
    pub const ALL: [(Harmonics, &'static str); 4] = [
        (Harmonics::None, "Harmoniques : aucune"),
        (Harmonics::First, "1re harmonique"),
        (Harmonics::Three, "1re, 2e, 3e"),
        (Harmonics::Rpm, "Plage du filtre RPM"),
    ];

    pub fn ranks(self) -> &'static [i64] {
        match self {
            Harmonics::First => &[1],
            Harmonics::Three => &[1, 2, 3],
            _ => &[],
        }
    }
}

/// Graphique de droite (`RIGHT_MODES`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RightMode {
    Sub100,
    MotorNoise,
}

/// Réglages qui demandent un nouveau calcul.
#[derive(Clone, PartialEq, Debug)]
pub struct Options {
    pub keys: Vec<&'static str>,
    pub psd: bool,
    pub resolution: usize,
    pub multiplier: f64,
    pub diagnosis: bool,
}

/// Courbe de l'analyse : (fréquences ou abscisses, valeurs).
pub type Curve = (Vec<f64>, Vec<f64>);
/// Jeu de données et axe d'une courbe.
pub type CurveKey = (&'static str, usize);

/// Résultat pour un fichier (`FileSpectra`).
pub struct FileSpectra {
    pub label: String,
    pub color: egui::Color32,
    /// (clé du jeu de données, axe) → (fréquences, valeurs non lissées).
    pub spectra: BTreeMap<CurveKey, Curve>,
    /// (clé, axe) → (moyennes, écarts-types) aux harmoniques 1, 2, 3.
    pub noise: BTreeMap<CurveKey, Curve>,
    pub gyro_ms: f64,
    pub dterm_ms: f64,
    /// Assez de mouvements pour que les retards soient représentatifs.
    pub delays_moving: bool,
    /// Le terme D existe sur cet axe (le yaw peut ne pas en avoir).
    pub dterm_axes: [bool; 3],
    /// Fréquences moteur triées, par moteur.
    pub motor_sorted: Vec<Vec<f64>>,
    /// Centres des notchs dynamiques triés.
    pub notch_sorted: Vec<Vec<f64>>,
    pub rpm_text: String,
    pub nyquist: f64,
    pub rpm_harmonics: i64,
    pub rpm_weights: Vec<i64>,
    pub rpm_min_hz: i64,
    /// Seulement si la case « Diagnostic du bruit » est cochée.
    pub diagnosis: Option<NoiseReport>,
    pub fs: f64,
}

pub struct SpectralOutput {
    pub psd: bool,
    pub keys: Vec<&'static str>,
    pub files: Vec<FileSpectra>,
}

/// Plage couverte par les notchs du filtre RPM : pour chaque harmonique active, (rang, début, fin, poids), du 5e au 95e
/// centile des fréquences moteur (`_draw_rpm_range`).
pub fn rpm_regions(f: &FileSpectra) -> Vec<(i64, f64, f64, i64)> {
    if f.rpm_harmonics <= 0 || f.motor_sorted.is_empty() {
        return Vec::new();
    }
    let all: Vec<f32> = f.motor_sorted.iter().flatten().map(|&v| v as f32).collect();
    if all.is_empty() {
        return Vec::new();
    }
    let (lo, hi) = (
        f64::from(percentile_f32(&all, 5.0)),
        f64::from(percentile_f32(&all, 95.0)),
    );
    let mut out = Vec::new();
    for h in 1..=f.rpm_harmonics {
        let w = f.rpm_weights.get((h - 1) as usize).copied().unwrap_or(100);
        if w <= 0 {
            continue;
        }
        let (a, b) = ((lo * h as f64).max(f.rpm_min_hz as f64), hi * h as f64);
        if b > a {
            out.push((h, a, b, w));
        }
    }
    out
}

/// Opacité du remplissage d'une harmonique : plus opaque si son poids est fort (`int(10 + 25 * w / 100)`).
pub fn region_alpha(weight: i64) -> u8 {
    (10.0 + 25.0 * weight as f64 / 100.0) as u8
}

/// Courbe en « S » : part du temps où chaque valeur (× `rank`) est sous la fréquence, mise à l'échelle de `y0` à `y0 + span`.
pub fn s_curve(sorted: &[f64], rank: f64, freqs: &[f64], y0: f64, span: f64) -> Vec<[f64; 2]> {
    let scaled: Vec<f64> = sorted.iter().map(|v| v * rank).collect();
    cdf(&scaled, freqs)
        .into_iter()
        .zip(freqs)
        .map(|(c, &f)| [f, y0 + span * c])
        .collect()
}

pub struct SpectralTool {
    datasets: [bool; 11],
    psd: bool,
    right: RightMode,
    ymin: f64,
    ymax: f64,
    smoothing: usize,
    resolution: usize,
    harmonics: Harmonics,
    notch: bool,
    motors: MotorToggles,
    multiplier: f64,
    diagnosis: bool,
    axes: AxisToggles,
    files: FileSelection,
    goto: Option<&'static str>,
    /// Clé des réglages de la vue : les bornes ne sont remises que quand elle change (le zoom de l'utilisateur reste).
    pub view: ViewMemo<[u64; 5]>,
}

impl Default for SpectralTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectralTool {
    pub fn new() -> Self {
        let mut datasets = [false; 11];
        for (i, (k, _)) in DATASETS.iter().enumerate() {
            datasets[i] = DEFAULT_DATASETS.contains(k);
        }
        Self {
            datasets,
            psd: true,
            right: RightMode::Sub100,
            ymin: -50.0,
            ymax: 20.0,
            smoothing: 2,
            resolution: 0,
            harmonics: Harmonics::Three,
            notch: true,
            motors: MotorToggles::default(),
            multiplier: 2.1,
            diagnosis: false,
            axes: AxisToggles::default(),
            files: FileSelection::new(true),
            goto: None,
            view: ViewMemo::default(),
        }
    }

    /// Passe du mode PSD à l'amplitude : l'échelle Y change aussi (`_on_mode_toggled`).
    pub fn set_psd(&mut self, psd: bool) {
        if self.psd != psd {
            self.psd = psd;
            (self.ymin, self.ymax) = if psd { (-50.0, 20.0) } else { (0.0, 1.0) };
        }
    }

    pub fn set_dataset(&mut self, key: &str, on: bool) {
        if let Some(i) = DATASETS.iter().position(|(k, _)| *k == key) {
            self.datasets[i] = on;
        }
    }

    pub fn set_diagnosis(&mut self, on: bool) {
        self.diagnosis = on;
    }

    pub fn set_right(&mut self, r: RightMode) {
        self.right = r;
    }

    pub fn y_range(&self) -> (f64, f64) {
        (self.ymin, self.ymax)
    }

    pub fn set_y_range(&mut self, lo: f64, hi: f64) {
        (self.ymin, self.ymax) = (lo, hi);
    }

    pub fn set_smoothing(&mut self, level: usize) {
        self.smoothing = level.min(5);
    }

    pub fn set_harmonics(&mut self, h: Harmonics) {
        self.harmonics = h;
    }

    fn pen(key: &str) -> (f32, LineStyle) {
        if is_prefilt(key) {
            (1.5, LineStyle::dotted_dense())
        } else {
            (2.5, LineStyle::Solid)
        }
    }

    fn spectra_lines(&self, out: &SpectralOutput, ax: usize, legend: bool) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for f in &out.files {
            for &key in &out.keys {
                let Some((fr, v)) = f.spectra.get(&(key, ax)) else {
                    continue;
                };
                let Ok(sm) = smooth_freq(v, &[v.len()], self.smoothing as i64) else {
                    continue;
                };
                // dernier point (Nyquist) écarté : il n'est pas doublé et crée un crochet
                let n = fr.len().saturating_sub(1);
                let pts: Vec<[f64; 2]> = (0..n).map(|i| [fr[i], sm[i]]).collect();
                let name = if legend {
                    let label = DATASETS.iter().find(|(k, _)| *k == key).map_or(key, |(_, l)| *l);
                    format!("{label} | {}", f.label)
                } else {
                    String::new()
                };
                let (width, style) = Self::pen(key);
                lines.push(
                    Line::new(name, PlotPoints::from(pts))
                        .color(f.color)
                        .width(width)
                        .style(style),
                );
            }
        }
        lines
    }

    fn overlays(&self, pu: &mut egui_plot::PlotUi, out: &SpectralOutput, y0: f64, y1: f64, nyq: f64) {
        let enabled = self.motors.enabled();
        let freqs: Vec<f64> = (0..800).map(|i| nyq * i as f64 / 799.0).collect();
        let span = y1 - y0;
        for f in &out.files {
            if self.harmonics == Harmonics::Rpm {
                for (h, a, b, w) in rpm_regions(f) {
                    pu.span(
                        Span::new("", a..=b)
                            .fill(with_alpha(f.color, region_alpha(w)))
                            .border_color(f.color)
                            .border_width(1.0)
                            .border_style(harmonic_style(h)),
                    );
                }
            } else {
                for (m, vals) in f.motor_sorted.iter().enumerate() {
                    if !enabled.contains(&(m % 4)) {
                        continue;
                    }
                    for &h in self.harmonics.ranks() {
                        let pts = s_curve(vals, h as f64, &freqs, y0, span);
                        pu.line(
                            Line::new("", PlotPoints::from(pts))
                                .color(motor_color(m))
                                .width(1.2)
                                .style(harmonic_style(h)),
                        );
                    }
                }
            }
            if self.notch {
                for vals in &f.notch_sorted {
                    let pts = s_curve(vals, 1.0, &freqs, y0, span);
                    pu.line(
                        Line::new("", PlotPoints::from(pts))
                            .color(plot::NEUTRAL)
                            .width(1.5)
                            .style(LineStyle::dashed_dense()),
                    );
                }
            }
        }
    }

    fn delay_lines(out: &SpectralOutput, ax: usize) -> Vec<(String, egui::Color32)> {
        out.files
            .iter()
            .map(|f| {
                let mut t = format!("Gyro : {}", fmt_ms(f.gyro_ms));
                if f.dterm_axes[ax] {
                    t += &format!(" | D : {}", fmt_ms(f.dterm_ms));
                }
                if !f.delays_moving {
                    t += " (vol sans mouvements : peu fiable)";
                }
                (t, f.color)
            })
            .collect()
    }

    fn motor_noise_plot(&self, pu: &mut egui_plot::PlotUi, out: &SpectralOutput, ax: usize) {
        let x = [1.0, 2.0, 3.0];
        for f in &out.files {
            for &key in &out.keys {
                let Some((mean, std)) = f.noise.get(&(key, ax)) else {
                    continue;
                };
                let ok: Vec<usize> = (0..3.min(mean.len())).filter(|&i| mean[i].is_finite()).collect();
                if ok.is_empty() {
                    continue;
                }
                let xs: Vec<f64> = ok.iter().map(|&i| x[i]).collect();
                let ms: Vec<f64> = ok.iter().map(|&i| mean[i]).collect();
                let es: Vec<f64> = ok.iter().map(|&i| if std[i].is_nan() { 0.0 } else { std[i] }).collect();
                let (width, style) = Self::pen(key);
                let pts: Vec<[f64; 2]> = xs.iter().zip(&ms).map(|(&a, &b)| [a, b]).collect();
                pu.line(
                    Line::new("", PlotPoints::from(pts.clone()))
                        .color(f.color)
                        .width(width)
                        .style(style),
                );
                pu.points(
                    Points::new("", PlotPoints::from(pts))
                        .color(f.color)
                        .radius(3.5)
                        .filled(true),
                );
                for bar in error_bars(&xs, &ms, &es, 1) {
                    pu.line(Line::new("", PlotPoints::from(bar)).color(f.color).width(1.5));
                }
            }
        }
    }

    fn plots(&mut self, ui: &mut Ui, out: &SpectralOutput, theme: &Theme) {
        let axes = self.axes.enabled();
        if out.files.is_empty() || axes.is_empty() {
            return;
        }
        let nyq = out.files.iter().map(|f| f.nyquist).fold(0.0, f64::max);
        let unit = if out.psd { "PSD (dB)" } else { "amplitude" };
        let right_key = matches!(self.right, RightMode::MotorNoise) as u64;
        let key = [
            self.ymin.to_bits(),
            self.ymax.to_bits(),
            nyq.to_bits(),
            right_key,
            u64::from(out.psd),
        ];
        let fresh = self.view.is_fresh(&key);
        let spacing = ui.spacing().item_spacing;
        let w = ((ui.available_width() - spacing.x) / 2.0).max(120.0);
        let rows = axes.len() as f32;
        let h = ((ui.available_height() - 22.0 - spacing.y * rows) / rows).max(110.0);
        ui.horizontal(|ui| {
            ui.add_sized([w, 18.0], egui::Label::new(RichText::new("Spectre complet").strong()));
            let t = if self.right == RightMode::Sub100 {
                "Sous 100 Hz"
            } else {
                "Bruit moteur"
            };
            ui.add_sized([w, 18.0], egui::Label::new(RichText::new(t).strong()));
        });
        let (ymin, ymax) = (self.ymin, self.ymax);
        // deux champs indépendants : Y min peut dépasser Y max, la vue prend les deux dans l'ordre
        let (ylo, yhi) = (ymin.min(ymax), ymin.max(ymax));
        for (row, &ax) in axes.iter().enumerate() {
            let name = AXIS_NAMES[ax];
            ui.horizontal(|ui| {
                let y1 = if out.psd { ymax.min(0.0) } else { ymax };
                let mut left = nice_axes(Plot::new(("spectre-g", ax)))
                    .width(w)
                    .height(h)
                    .auto_bounds(false)
                    .x_axis_label("Fréquence (Hz)")
                    .y_axis_label(format!("{name} | {unit}"))
                    .grid_color(theme.grid_color());
                if row == 0 {
                    left = left.legend(Legend::default());
                }
                let resp = left.show(ui, |pu| {
                    if fresh {
                        pu.set_plot_bounds_x(0.0..=nyq);
                        pu.set_plot_bounds_y(ylo..=yhi);
                    }
                    for l in self.spectra_lines(out, ax, row == 0) {
                        pu.line(l);
                    }
                    self.overlays(pu, out, ymin, y1, nyq);
                });
                let _ = resp;
                match self.right {
                    RightMode::Sub100 => {
                        let resp = nice_axes(Plot::new(("spectre-d", ax)))
                            .width(w)
                            .height(h)
                            .auto_bounds(false)
                            .x_axis_label("Fréquence (Hz)")
                            .y_axis_label(format!("{name} | {unit}"))
                            .grid_color(theme.grid_color())
                            .show(ui, |pu| {
                                if fresh {
                                    pu.set_plot_bounds_x(0.0..=100.0);
                                    pu.set_plot_bounds_y(ylo..=yhi);
                                }
                                for l in self.spectra_lines(out, ax, false) {
                                    pu.line(l);
                                }
                            });
                        corner_text(
                            ui,
                            *resp.transform.frame(),
                            Align2::RIGHT_TOP,
                            &Self::delay_lines(out, ax),
                            theme.legend_fill,
                        );
                    }
                    RightMode::MotorNoise => {
                        let resp = nice_axes(Plot::new(("spectre-b", ax)))
                            .width(w)
                            .height(h)
                            .auto_bounds(false)
                            .x_axis_label("Harmonique moteur")
                            .y_axis_label(format!("{name} | bruit moteur moyen (dB)"))
                            .x_axis_formatter(|m, _| match m.value.round() as i64 {
                                1 if (m.value - 1.0).abs() < 1e-6 => "1re".into(),
                                2 if (m.value - 2.0).abs() < 1e-6 => "2e".into(),
                                3 if (m.value - 3.0).abs() < 1e-6 => "3e".into(),
                                _ => String::new(),
                            })
                            .grid_color(theme.grid_color())
                            .show(ui, |pu| {
                                if fresh {
                                    pu.set_plot_bounds_x(0.7..=3.3);
                                    if out.psd {
                                        pu.set_plot_bounds_y(ylo..=yhi);
                                    } else {
                                        pu.set_plot_bounds_y(-50.0..=20.0);
                                    }
                                }
                                self.motor_noise_plot(pu, out, ax);
                            });
                        if row == 0 {
                            let mut lines = vec![("Poids filtre RPM | Q".to_string(), theme.fg)];
                            lines.extend(out.files.iter().map(|f| (f.rpm_text.clone(), f.color)));
                            corner_text(
                                ui,
                                *resp.transform.frame(),
                                Align2::RIGHT_TOP,
                                &lines,
                                theme.legend_fill,
                            );
                        }
                    }
                }
            });
        }
        self.view.remember(key);
    }
}

impl Tool for SpectralTool {
    type Options = Options;
    type Output = SpectralOutput;
    const TITLE: &'static str = "Analyse spectrale";
    const HAS_VIEW: bool = true;

    fn reset_view(&mut self) {
        self.view.reset();
    }

    fn options(&self) -> Options {
        Options {
            keys: DATASETS
                .iter()
                .enumerate()
                .filter(|(i, _)| self.datasets[*i])
                .map(|(_, (k, _))| *k)
                .collect(),
            psd: self.psd,
            resolution: self.resolution,
            multiplier: self.multiplier,
            diagnosis: self.diagnosis,
        }
    }

    fn selected(&self, state: &AppState) -> Vec<usize> {
        self.files.indices(state)
    }

    fn compute(o: Options, logs: Vec<LogInput>) -> Result<SpectralOutput, String> {
        let mode = if o.psd { Mode::Psd } else { Mode::Amp };
        let resolution = RESOLUTIONS[o.resolution.min(RESOLUTIONS.len() - 1)].0;
        let mut files = Vec::new();
        for li in &logs {
            let log = &*li.log;
            let sel = log.slice_time(li.period.0, li.period.1);
            let s = &log.settings;
            let mut f = FileSpectra {
                label: li.label.clone(),
                color: li.color,
                spectra: BTreeMap::new(),
                noise: BTreeMap::new(),
                gyro_ms: f64::NAN,
                dterm_ms: f64::NAN,
                delays_moving: true,
                dterm_axes: [false; 3],
                motor_sorted: Vec::new(),
                notch_sorted: Vec::new(),
                rpm_text: rpm_filter_text(log),
                nyquist: log.fs / 2.0,
                rpm_harmonics: s.rpm_harmonics,
                rpm_weights: s.rpm_weights.clone(),
                rpm_min_hz: s.rpm_min_hz,
                diagnosis: None,
                fs: log.fs,
            };
            let mhz = motor_freqs(log, o.multiplier).map(|m| sel_rows(&m, &sel));
            if let Some(m) = &mhz {
                f.motor_sorted = (0..m.k).map(|k| sorted_positive(&wide(m.row(k)))).collect();
            }
            if let Some(centres) = dyn_notch_centres(log) {
                f.notch_sorted = centres
                    .iter()
                    .map(|c| sorted_above_zero(&c[sel.start.min(c.len())..sel.end.min(c.len())]))
                    .collect();
            }
            let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
            for &key in &o.keys {
                for ax in 0..3 {
                    if !datasets::has_axis(log, key, ax)? {
                        continue;
                    }
                    let Some(data) = datasets::get(log, key)? else { continue };
                    let row = data.row(ax);
                    let x = wide(&row[sel.start.min(row.len())..sel.end.min(row.len())]);
                    let sp = spectrum(&x, log.fs, mode, resolution, 0, Some(&br))?;
                    let values = if o.psd { to_ptb_db(&sp.values) } else { sp.values };
                    f.spectra.insert((key, ax), (sp.freqs, values));
                    if let Some(m) = &mhz {
                        let mn = motor_noise(&x, m, log.fs, 3, 300.0, 20.0, Some(&br))?;
                        f.noise.insert((key, ax), (to_ptb_db(&mn.mean_db), mn.std_db));
                    }
                }
            }
            let d = compute_delays(log, sel.clone())?;
            (f.gyro_ms, f.dterm_ms, f.delays_moving) = (d.gyro_ms, d.dterm_ms, d.moving());
            for ax in 0..3 {
                f.dterm_axes[ax] = ax < 2 || datasets::has_axis(log, "dterm", ax)?;
            }
            if o.diagnosis {
                f.diagnosis = Some(noise_report(log, Some(sel.clone()), None)?);
            }
            files.push(f);
        }
        Ok(SpectralOutput {
            psd: o.psd,
            keys: o.keys,
            files,
        })
    }

    fn controls(&mut self, ui: &mut Ui, state: &AppState, theme: &Theme) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.files.ui(ui, state);
            ui.separator();
            self.axes.ui(ui);
            ui.separator();
            ui.label(RichText::new("Jeux de données").strong());
            // tous les jeux visibles sans faire défiler (le panneau défile déjà : W-034 (10))
            for (i, (_, label)) in DATASETS.iter().enumerate() {
                ui.checkbox(&mut self.datasets[i], *label);
            }
            ui.horizontal(|ui| {
                for (title, color) in [("Fréq. × gaz", "red"), ("Fréq. × temps", "purple")] {
                    let b = egui::Button::new(RichText::new(title).color(theme.accent(color)).strong());
                    if ui.add(b).clicked() {
                        self.goto = Some(if title == "Fréq. × gaz" { "Fréquence × gaz" } else { "Fréquence × temps" });
                    }
                }
            });
            ui.separator();
            let mut psd = self.psd;
            ui.horizontal(|ui| {
                ui.checkbox(&mut psd, "PSD (dB)");
                egui::ComboBox::from_id_salt("droite")
                    .selected_text(if self.right == RightMode::Sub100 { "< 100 Hz" } else { "Bruit moteur" })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.right, RightMode::Sub100, "< 100 Hz");
                        ui.selectable_value(&mut self.right, RightMode::MotorNoise, "Bruit moteur");
                    }).response.accessible_name("Graphique de droite");
            });
            self.set_psd(psd);
            egui::Grid::new("echelle").show(ui, |ui| {
                ui.label("Y min");
                ui.label("Y max");
                ui.end_row();
                ui.add(egui::DragValue::new(&mut self.ymin).range(-200.0..=1e4).speed(0.5)).accessible_name("Y min");
                ui.add(egui::DragValue::new(&mut self.ymax).range(-200.0..=1e4).speed(0.5)).accessible_name("Y max");
                ui.end_row();
                ui.label("Lissage");
                ui.label("Résolution");
                ui.end_row();
                egui::ComboBox::from_id_salt("lissage")
                    .selected_text(self.smoothing.to_string())
                    .show_ui(ui, |ui| {
                        for i in 0..6 {
                            ui.selectable_value(&mut self.smoothing, i, i.to_string());
                        }
                    }).response.accessible_name("Lissage");
                egui::ComboBox::from_id_salt("resolution")
                    .selected_text(RESOLUTIONS[self.resolution].1)
                    .show_ui(ui, |ui| {
                        for (i, (_, l)) in RESOLUTIONS.iter().enumerate() {
                            ui.selectable_value(&mut self.resolution, i, *l);
                        }
                    }).response.accessible_name("Résolution");
                ui.end_row();
            });
            egui::ComboBox::from_id_salt("harmoniques")
                .selected_text(Harmonics::ALL.iter().find(|(h, _)| *h == self.harmonics).map_or("", |(_, l)| *l))
                .show_ui(ui, |ui| {
                    for (h, l) in Harmonics::ALL {
                        ui.selectable_value(&mut self.harmonics, h, l);
                    }
                }).response.accessible_name("Harmoniques");
            ui.checkbox(&mut self.notch, "Notchs dynamiques");
            self.motors.ui(ui);
            ui.horizontal(|ui| {
                ui.label("RPM estimé ×");
                ui.add(egui::DragValue::new(&mut self.multiplier).range(0.1..=20.0).speed(0.05).max_decimals(2))
                    .on_hover_text("Utilisé seulement si le log n'a pas l'eRPM (télémétrie DShot).");
            });
            ui.separator();
            ui.label(RichText::new("Diagnostic").strong());
            ui.checkbox(&mut self.diagnosis, "Diagnostic du bruit").on_hover_text(
                "Rapport sur le bruit du gyro et du terme D avant et après filtrage, avec la stratégie de filtrage \
                 conseillée (mêmes règles que l'assistant).",
            );
            ui.label(
                RichText::new(
                    "Rapport à côté des courbes : bruit gyro et D avant / après filtres, conseils par filtre (RPM, notch \
                     dynamique, passe-bas) et stratégie de filtrage.",
                )
                .color(theme.ui.muted),
            );
        });
    }

    fn render(&mut self, ui: &mut Ui, _state: &mut AppState, out: &SpectralOutput, theme: &Theme) {
        let reports: Vec<&FileSpectra> = out.files.iter().filter(|f| f.diagnosis.is_some()).collect();
        if self.diagnosis && !reports.is_empty() {
            egui::Panel::right("spectre-diagnostic")
                .default_size(REPORT_WIDTH)
                .size_range(320.0..=900.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for f in &reports {
                            if let Some(nr) = &f.diagnosis {
                                diagnosis_ui(ui, theme, &f.label, f.color, f.fs, nr);
                            }
                        }
                        ui.separator();
                        ui.label(RichText::new(REPORT_FOOTER).color(theme.ui.muted));
                    });
                });
        }
        egui::CentralPanel::default().show(ui, |ui| self.plots(ui, out, theme));
    }

    fn take_goto(&mut self) -> Option<&'static str> {
        self.goto.take()
    }
}
