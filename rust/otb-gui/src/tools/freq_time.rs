//! Fréquence × temps (port de `gui/tools/freq_time.py`) : spectrogramme d'un fichier, un graphique par axe, avec les
//! fréquences des moteurs (et leurs harmoniques) et des notchs dynamiques tracées par-dessus, et le curseur du lecteur,
//! déplaçable, partagé avec le visualiseur.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};

use egui::{RichText, TextureHandle, Ui};
use egui_plot::{Line, LineStyle, Plot, PlotImage, PlotPoint, PlotPoints, VLine};
use otb_analysis::datasets::{self, DATASETS};
use otb_analysis::motors::dyn_notch_centres;
use otb_analysis::spectral::{freq_time, to_ptb_db};

use super::common::{AxisToggles, FileSelection, MotorToggles, decimate, harmonic_style, motor_freqs, sel_rows, wide};
use crate::plots::{VDrag, colorbar, heatmap_texture, nice_axes};
use crate::state::AppState;
use crate::theme::{AXIS_NAMES, Palette, Theme, motor_color, plot};
use crate::tool::{LogInput, Tool};

/// Lissage fréquentiel : (nombre de niveaux, libellé).
pub const FREQ_SMOOTHING: [(i64, &str); 4] = [(0, "aucun"), (2, "5 points"), (5, "10 points"), (10, "20 points")];
/// Pas entre deux fenêtres de 300 ms : (diviseur, libellé).
pub const TIME_STEPS: [(i64, &str); 4] = [(1, "300 ms"), (2, "150 ms"), (5, "60 ms"), (10, "30 ms")];
/// Harmoniques moteur tracées : (rangs, libellé).
pub const HARMONIC_CHOICES: [(&[i64], &str); 5] = [
    (&[], "Harmoniques : aucune"),
    (&[1], "1re harmonique"),
    (&[2], "2e harmonique"),
    (&[3], "3e harmonique"),
    (&[1, 2, 3], "1re, 2e, 3e"),
];
pub const MAX_TRACE_POINTS: usize = 3000;
/// Largeur maximale d'une texture de spectrogramme (limite des cartes graphiques) : au-delà, les colonnes sont moyennées.
pub const MAX_TEXTURE_COLUMNS: usize = 4096;

/// Réglages qui demandent un nouveau calcul.
#[derive(Clone, PartialEq, Debug)]
pub struct Options {
    pub key: &'static str,
    pub freq_smoothing: i64,
    pub hop_div: i64,
    pub multiplier: f64,
}

pub struct Spectro {
    pub times: Vec<f64>,
    pub freqs: Vec<f64>,
    /// `nf × nt` (ligne 0 = fréquence la plus basse), en dB à l'échelle de PIDtoolbox.
    pub z: Vec<f64>,
}

pub struct FreqTimeOutput {
    pub label: String,
    pub color: egui::Color32,
    pub key: &'static str,
    /// Période d'analyse du log au moment du calcul (étendue de l'axe du temps).
    pub period: (f64, f64),
    pub maps: BTreeMap<usize, Spectro>,
    pub motor_t: Option<Vec<f64>>,
    pub motor_hz: Vec<Vec<f64>>,
    pub notches: Vec<(Vec<f64>, Vec<f64>)>,
    pub nyquist: f64,
    pub generation: u64,
}

/// Sel du lien d'axes des trois spectrogrammes.
const LINK: &str = "freq-temps";

static GENERATION: AtomicU64 = AtomicU64::new(1);

/// Tronçons de points consécutifs finis `[x, y]` : un NaN coupe la ligne (`connect="finite"`).
pub fn finite_runs(x: &[f64], y: &[f64]) -> Vec<Vec<[f64; 2]>> {
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut run: Vec<[f64; 2]> = Vec::new();
    for (&a, &b) in x.iter().zip(y) {
        if a.is_finite() && b.is_finite() {
            run.push([a, b]);
        } else if !run.is_empty() {
            out.push(std::mem::take(&mut run));
        }
    }
    if !run.is_empty() {
        out.push(run);
    }
    out
}

/// Réduit un spectrogramme `nf × nt` à au plus `max` colonnes, par moyenne des valeurs finies de chaque bloc de colonnes
/// (une colonne sans valeur reste NaN). Rend les valeurs et le nombre de colonnes.
pub fn reduce_columns(z: &[f32], nf: usize, nt: usize, max: usize) -> (Vec<f32>, usize) {
    if nt <= max || max == 0 {
        return (z.to_vec(), nt);
    }
    let k = nt.div_ceil(max);
    let nt2 = nt.div_ceil(k);
    let mut out = vec![f32::NAN; nf * nt2];
    for r in 0..nf {
        for c in 0..nt2 {
            let block = &z[r * nt + c * k..r * nt + ((c + 1) * k).min(nt)];
            let (mut s, mut n) = (0.0f64, 0usize);
            for v in block.iter().filter(|v| !v.is_nan()) {
                s += f64::from(*v);
                n += 1;
            }
            if n > 0 {
                out[r * nt2 + c] = (s / n as f64) as f32;
            }
        }
    }
    (out, nt2)
}

pub struct FreqTimeTool {
    key: &'static str,
    freq_smoothing: i64,
    hop_div: i64,
    multiplier: f64,
    palette: Palette,
    zmin: f64,
    zmax: f64,
    sub100: bool,
    notch: bool,
    harmonics: usize,
    motors: MotorToggles,
    axes: AxisToggles,
    files: FileSelection,
    drags: [VDrag; 3],
    link: egui::Id,
    /// Repère écran ↔ données de chaque graphique au dernier affichage (tests, captures).
    pub transforms: [Option<egui_plot::PlotTransform>; 3],
    textures: HashMap<usize, (u64, TextureHandle)>,
    view: Option<[u64; 3]>,
}

impl Default for FreqTimeTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FreqTimeTool {
    pub fn new() -> Self {
        Self {
            key: "gyro_prefilt",
            freq_smoothing: 2,
            hop_div: 2,
            multiplier: 2.1,
            palette: Palette::Hot,
            zmin: -30.0,
            zmax: 10.0,
            sub100: false,
            notch: false,
            harmonics: 1,
            motors: MotorToggles::default(),
            axes: AxisToggles::default(),
            files: FileSelection::new(false),
            drags: [VDrag::default(); 3],
            link: egui::Id::unique(LINK),
            transforms: [None; 3],
            textures: HashMap::new(),
            view: None,
        }
    }

    pub fn set_dataset(&mut self, key: &str) {
        if let Some((k, _)) = DATASETS.iter().find(|(k, _)| *k == key) {
            self.key = k;
        }
    }

    pub fn set_hop_div(&mut self, d: i64) {
        self.hop_div = d;
    }

    pub fn set_sub100(&mut self, on: bool) {
        self.sub100 = on;
    }

    fn texture(
        &mut self,
        ctx: &egui::Context,
        ax: usize,
        out: &FreqTimeOutput,
        sg: &Spectro,
        levels: (f32, f32),
    ) -> TextureHandle {
        let key = out.generation
            ^ u64::from(levels.0.to_bits()).rotate_left(11)
            ^ u64::from(levels.1.to_bits()).rotate_left(27)
            ^ (self.palette as u64).rotate_left(41);
        if let Some((k, t)) = self.textures.get(&ax)
            && *k == key
        {
            return t.clone();
        }
        let (nf, nt) = (sg.freqs.len(), sg.times.len());
        let z: Vec<f32> = sg.z.iter().map(|&v| v as f32).collect();
        let (z, nt) = reduce_columns(&z, nf, nt, MAX_TEXTURE_COLUMNS);
        let t = heatmap_texture(
            ctx,
            &format!("spectrogramme-{ax}"),
            &z,
            nf,
            nt,
            levels.0,
            levels.1,
            self.palette,
            plot::MAP_BACKGROUND,
        );
        self.textures.insert(ax, (key, t.clone()));
        t
    }
}

impl Tool for FreqTimeTool {
    type Options = Options;
    type Output = FreqTimeOutput;
    const TITLE: &'static str = "Fréquence × temps";
    const HIDDEN: bool = true;
    const TAB: Option<&'static str> = Some("Analyse spectrale");

    fn options(&self) -> Options {
        Options {
            key: self.key,
            freq_smoothing: self.freq_smoothing,
            hop_div: self.hop_div,
            multiplier: self.multiplier,
        }
    }

    fn selected(&self, state: &AppState) -> Vec<usize> {
        self.files.indices(state)
    }

    fn compute(o: Options, logs: Vec<LogInput>) -> Result<FreqTimeOutput, String> {
        let li = logs.first().ok_or("aucun log sélectionné")?;
        let log = &*li.log;
        let sel = log.slice_time(li.period.0, li.period.1);
        let mut out = FreqTimeOutput {
            label: li.label.clone(),
            color: li.color,
            key: o.key,
            period: li.period,
            maps: BTreeMap::new(),
            motor_t: None,
            motor_hz: Vec::new(),
            notches: Vec::new(),
            nyquist: log.fs / 2.0,
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
        };
        let t = &log.t[sel.start.min(log.t.len())..sel.end.min(log.t.len())];
        if let Some(data) = datasets::get(log, o.key)? {
            let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
            for ax in 0..3 {
                if !datasets::has_axis(log, o.key, ax)? {
                    continue;
                }
                let row = data.row(ax);
                let x = wide(&row[sel.start.min(row.len())..sel.end.min(row.len())]);
                let m = freq_time(&x, t, log.fs, 300.0, o.hop_div, o.freq_smoothing, Some(&br))?;
                out.maps.insert(
                    ax,
                    Spectro {
                        times: m.times,
                        freqs: m.freqs,
                        z: to_ptb_db(&m.z),
                    },
                );
            }
        }
        if let Some(mhz) = motor_freqs(log, o.multiplier) {
            let part = sel_rows(&mhz, &sel);
            for k in 0..part.k {
                let (tt, hz) = decimate(t, &wide(part.row(k)), MAX_TRACE_POINTS);
                out.motor_hz.push(hz);
                out.motor_t = Some(tt);
            }
        }
        if let Some(centres) = dyn_notch_centres(log) {
            for c in &centres {
                let part = &c[sel.start.min(c.len())..sel.end.min(c.len())];
                let masked: Vec<f64> = part.iter().map(|&v| if v > 0.0 { v } else { f64::NAN }).collect();
                out.notches.push(decimate(t, &masked, MAX_TRACE_POINTS));
            }
        }
        Ok(out)
    }

    fn controls(&mut self, ui: &mut Ui, state: &AppState, theme: &Theme) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.files.ui(ui, state);
            ui.separator();
            self.axes.ui(ui);
            ui.separator();
            egui::Grid::new("ftemps-options").show(ui, |ui| {
                ui.label("Données");
                egui::ComboBox::from_id_salt("ft-donnees")
                    .selected_text(DATASETS.iter().find(|(k, _)| *k == self.key).map_or("", |(_, l)| *l))
                    .show_ui(ui, |ui| {
                        for (k, l) in DATASETS {
                            ui.selectable_value(&mut self.key, k, l);
                        }
                    })
                    .response
                    .accessible_name("Données");
                ui.end_row();
                ui.label("Lissage fréq.");
                egui::ComboBox::from_id_salt("ft-lissage")
                    .selected_text(
                        FREQ_SMOOTHING
                            .iter()
                            .find(|(v, _)| *v == self.freq_smoothing)
                            .map_or("", |(_, l)| *l),
                    )
                    .show_ui(ui, |ui| {
                        for (v, l) in FREQ_SMOOTHING {
                            ui.selectable_value(&mut self.freq_smoothing, v, l);
                        }
                    })
                    .response
                    .accessible_name("Lissage fréquentiel");
                ui.end_row();
                ui.label("Pas temporel");
                egui::ComboBox::from_id_salt("ft-pas")
                    .selected_text(
                        TIME_STEPS
                            .iter()
                            .find(|(v, _)| *v == self.hop_div)
                            .map_or("", |(_, l)| *l),
                    )
                    .show_ui(ui, |ui| {
                        for (v, l) in TIME_STEPS {
                            ui.selectable_value(&mut self.hop_div, v, l);
                        }
                    })
                    .response
                    .accessible_name("Pas temporel");
                ui.end_row();
                ui.label("Couleurs");
                egui::ComboBox::from_id_salt("ft-palette")
                    .selected_text(self.palette.name())
                    .show_ui(ui, |ui| {
                        for p in Palette::ALL {
                            ui.selectable_value(&mut self.palette, p, p.name());
                        }
                    })
                    .response
                    .accessible_name("Palette de couleurs");
                ui.end_row();
                ui.label("Z min (dB)");
                ui.add(egui::DragValue::new(&mut self.zmin).range(-200.0..=200.0).speed(0.5))
                    .accessible_name("Z min (dB)");
                ui.end_row();
                ui.label("Z max (dB)");
                ui.add(egui::DragValue::new(&mut self.zmax).range(-200.0..=200.0).speed(0.5))
                    .accessible_name("Z max (dB)");
                ui.end_row();
            });
            ui.checkbox(&mut self.sub100, "< 100 Hz");
            ui.checkbox(&mut self.notch, "Notchs dyn.");
            egui::ComboBox::from_id_salt("ft-harmoniques")
                .selected_text(HARMONIC_CHOICES[self.harmonics].1)
                .show_ui(ui, |ui| {
                    for (i, (_, l)) in HARMONIC_CHOICES.iter().enumerate() {
                        ui.selectable_value(&mut self.harmonics, i, *l);
                    }
                })
                .response
                .accessible_name("Harmoniques");
            self.motors.ui(ui);
            ui.horizontal(|ui| {
                ui.label("RPM estimé ×");
                ui.add(
                    egui::DragValue::new(&mut self.multiplier)
                        .range(0.1..=20.0)
                        .speed(0.05)
                        .max_decimals(2),
                )
                .on_hover_text("Utilisé seulement si le log n'a pas l'eRPM (télémétrie DShot).");
            });
            ui.label(
                RichText::new("Cliquez ou déplacez le trait pointillé pour placer le curseur du lecteur.")
                    .color(theme.ui.muted),
            );
        });
    }

    fn render(&mut self, ui: &mut Ui, state: &mut AppState, out: &FreqTimeOutput, theme: &Theme) {
        let axes = self.axes.enabled();
        let title = DATASETS
            .iter()
            .find(|(k, _)| *k == out.key)
            .map_or(out.key, |(_, l)| *l);
        ui.label(
            RichText::new(format!("{title} | {}", out.label))
                .color(out.color)
                .strong(),
        );
        let shown: Vec<usize> = axes
            .iter()
            .copied()
            .filter(|a| out.maps.get(a).is_some_and(|m| !m.times.is_empty()))
            .collect();
        if shown.is_empty() {
            return;
        }
        let levels = (self.zmin, self.zmax.max(self.zmin + 1e-6));
        let f_top = if self.sub100 { 100.0 } else { out.nyquist };
        let (t0, t1) = out.period;
        let key = [
            out.generation,
            f_top.to_bits(),
            t0.to_bits() ^ t1.to_bits().rotate_left(17),
        ];
        let fresh = self.view != Some(key);
        let spacing = ui.spacing().item_spacing;
        let h = ((ui.available_height() - spacing.y * shown.len() as f32) / shown.len() as f32).max(110.0);
        let w = (ui.available_width() - 70.0 - spacing.x).max(200.0);
        let enabled = self.motors.enabled();
        let ranks = HARMONIC_CHOICES[self.harmonics].0;
        let link = self.link;
        let press = ui.input(|i| i.pointer.press_origin()).map(|p| p.x);
        let mut xs = [state.cursor];
        let mut clicked: Option<f64> = None;
        for (row, &ax) in shown.iter().enumerate() {
            let sg = &out.maps[&ax];
            let tex = self.texture(ui.ctx(), ax, out, sg, (levels.0 as f32, levels.1 as f32));
            let (nf, nt) = (sg.freqs.len(), sg.times.len());
            let df = if nf > 1 { sg.freqs[1] - sg.freqs[0] } else { 1.0 };
            let dt = if nt > 1 {
                let mut d: Vec<f64> = sg.times.windows(2).map(|p| p[1] - p[0]).collect();
                d.sort_by(|a, b| a.total_cmp(b));
                // médiane de numpy : moyenne des deux valeurs centrales si le nombre est pair
                if d.len() % 2 == 1 {
                    d[d.len() / 2]
                } else {
                    (d[d.len() / 2 - 1] + d[d.len() / 2]) / 2.0
                }
            } else {
                0.3
            };
            // étendue réelle des colonnes : avec des trous dans le log, le temps n'avance pas d'un pas fixe (D-020)
            let span = if nt > 1 {
                sg.times[nt - 1] - sg.times[0] + dt
            } else {
                dt
            };
            let (x0, y0) = (sg.times[0] - dt / 2.0, sg.freqs[0] - df / 2.0);
            let drag = &mut self.drags[ax];
            let busy = drag.busy();
            ui.horizontal(|ui| {
                let resp = nice_axes(Plot::new(("ftemps", ax)))
                    .width(w)
                    .height(h)
                    .auto_bounds(false)
                    .allow_drag([!busy, false])
                    .allow_scroll(false)
                    .x_axis_label("Temps (s)")
                    .y_axis_label(format!("{} | fréq. (Hz)", AXIS_NAMES[ax]))
                    .link_axis(link, [true, false])
                    .link_cursor(link, [true, false])
                    .grid_color(theme.grid_color())
                    .show(ui, |pu| {
                        if fresh {
                            pu.set_plot_bounds_x(t0..=t1);
                            pu.set_plot_bounds_y(0.0..=f_top);
                        }
                        pu.image(PlotImage::new(
                            "",
                            tex.id(),
                            PlotPoint::new(x0 + span / 2.0, y0 + df * nf as f64 / 2.0),
                            [span as f32, (df * nf as f64) as f32],
                        ));
                        if let Some(tt) = &out.motor_t {
                            for &h in ranks {
                                for (m, hz) in out.motor_hz.iter().enumerate() {
                                    if !enabled.contains(&(m % 4)) {
                                        continue;
                                    }
                                    let y: Vec<f64> = hz.iter().map(|v| v * h as f64).collect();
                                    for run in finite_runs(tt, &y) {
                                        pu.line(
                                            Line::new("", PlotPoints::from(run))
                                                .color(motor_color(m))
                                                .width(1.5)
                                                .style(harmonic_style(h)),
                                        );
                                    }
                                }
                            }
                        }
                        if self.notch {
                            for (tt, hz) in &out.notches {
                                for run in finite_runs(tt, hz) {
                                    pu.line(Line::new("", PlotPoints::from(run)).color(plot::NOTCH_TRACE).width(1.0));
                                }
                            }
                        }
                        pu.vline(
                            VLine::new("", xs[0])
                                .color(if drag.hot || drag.active.is_some() {
                                    plot::OVERLAY_HOVER
                                } else {
                                    plot::OVERLAY
                                })
                                .width(if drag.hot { 2.0 } else { 1.5 })
                                .style(LineStyle::dashed_loose()),
                        );
                        drag.update(pu, press, &mut xs, &[(t0, t1)]);
                        if !drag.busy()
                            && pu.response().clicked()
                            && let Some(p) = pu.pointer_coordinate()
                        {
                            clicked = Some(p.x);
                        }
                    });
                self.transforms[ax] = Some(resp.transform);
                ui.vertical(|ui| {
                    colorbar(
                        ui,
                        self.palette,
                        levels.0,
                        levels.1,
                        h - 8.0,
                        true,
                        if row == 0 { "dB" } else { "" },
                    );
                });
            });
        }
        if let Some(x) = clicked {
            xs[0] = x.clamp(t0, t1);
        }
        if xs[0] != state.cursor {
            state.set_cursor(xs[0]);
        }
        self.view = Some(key);
    }
}
