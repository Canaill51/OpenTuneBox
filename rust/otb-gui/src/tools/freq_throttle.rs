//! Fréquence × gaz (port de `gui/tools/freq_throttle.py`) : une carte (fréquence en ordonnée, gaz ou moteur en abscisse)
//! par axe et par colonne. Jusqu'à 4 colonnes, chacune avec son jeu de données, son fichier et son échelle de couleurs ;
//! neuf préréglages de PIDtoolbox les remplissent d'un coup.

use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};

use egui::{Align2, RichText, TextureHandle, Ui};
use egui_plot::{Plot, PlotImage, PlotPoint};
use otb_analysis::datasets::{self, DATASETS};
use otb_analysis::motors::mean_axis0;
use otb_analysis::npstat::percentile_f32;
use otb_analysis::spectral::{Mode, freq_throttle, smooth_map, to_ptb_db};
use otb_log::flightlog::FlightLog;

use super::common::{AxisToggles, FileSelection, fmt3, motor_freqs, nice_ceil, percentile_linear, wide};
use crate::plots::{colorbar, corner_text, heatmap_texture, nice_axes};
use crate::state::AppState;
use crate::theme::{AXIS_NAMES, Palette, Theme, plot};
use crate::tool::{LogInput, Tool};

pub const N_COLUMNS: usize = 4;
/// Lissage des cartes à l'affichage : (niveau, libellé).
pub const SMOOTHING: [(i64, &str); 6] = [(0, "aucun"), (1, "1"), (2, "2"), (3, "3"), (4, "4"), (6, "6")];
/// Nombre de tranches de l'axe X.
pub const NBINS: usize = 100;

/// Grandeur en abscisse (`X_AXES`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum XAxis {
    Throttle,
    MotorPct,
    MotorHz,
}

impl XAxis {
    pub const ALL: [(XAxis, &'static str); 3] = [
        (XAxis::Throttle, "% gaz"),
        (XAxis::MotorPct, "% moteur moyen"),
        (XAxis::MotorHz, "Hz moteur (eRPM)"),
    ];

    /// Titre de l'axe (`X_LABELS`).
    pub fn label(self) -> &'static str {
        match self {
            XAxis::Throttle => "% gaz",
            XAxis::MotorPct => "% moteur (moyenne)",
            XAxis::MotorHz => "fréquence moteur (Hz)",
        }
    }
}

pub type ColumnChoice = (Option<&'static str>, Option<usize>);

/// Préréglages de PIDtoolbox : (libellé, jeux de données des 4 colonnes, rang du fichier de chaque colonne dans la liste
/// des fichiers choisis : A = 0, B = 1…).
pub const PRESETS: [(&str, [&str; 4], [usize; 4]); 9] = [
    (
        "1 : Gyro pf | Gyro | D pf | D",
        ["gyro_prefilt", "gyro", "dterm_prefilt", "dterm"],
        [0, 0, 0, 0],
    ),
    (
        "2 : Gyro pf | Gyro | P | D",
        ["gyro_prefilt", "gyro", "pterm", "dterm"],
        [0, 0, 0, 0],
    ),
    (
        "3 : Gyro | D | Consigne | Erreur PID",
        ["gyro", "dterm", "setpoint", "pid_error"],
        [0, 0, 0, 0],
    ),
    (
        "4 : A|A|B|B Gyro pf | Gyro",
        ["gyro_prefilt", "gyro", "gyro_prefilt", "gyro"],
        [0, 0, 1, 1],
    ),
    (
        "5 : A|A|B|B D pf | D",
        ["dterm_prefilt", "dterm", "dterm_prefilt", "dterm"],
        [0, 0, 1, 1],
    ),
    ("6 : A|B|C|D Gyro pf", ["gyro_prefilt"; 4], [0, 1, 2, 3]),
    ("7 : A|B|C|D Gyro", ["gyro"; 4], [0, 1, 2, 3]),
    ("8 : A|B|C|D D", ["dterm"; 4], [0, 1, 2, 3]),
    ("9 : A|B|C|D Erreur PID", ["pid_error"; 4], [0, 1, 2, 3]),
];

fn static_key(key: &str) -> Option<&'static str> {
    DATASETS.iter().find(|(k, _)| *k == key).map(|(k, _)| *k)
}

/// Colonnes d'un préréglage. `chosen` : indices des fichiers choisis (à défaut, le log courant `current`). Une colonne dont
/// le fichier n'existe pas (pas assez de fichiers choisis) est vide.
pub fn preset_columns(preset: usize, chosen: &[usize], current: usize) -> [ColumnChoice; N_COLUMNS] {
    let (_, keys, files) = PRESETS[preset];
    let sel: Vec<usize> = if chosen.is_empty() {
        vec![current]
    } else {
        chosen.to_vec()
    };
    let mut out = [(None, None); N_COLUMNS];
    for c in 0..N_COLUMNS {
        if let Some(&f) = sel.get(files[c]) {
            out[c] = (static_key(keys[c]), Some(f));
        }
    }
    out
}

/// Réglages qui demandent un nouveau calcul.
#[derive(Clone, PartialEq, Debug)]
pub struct Options {
    pub columns: [ColumnChoice; N_COLUMNS],
    pub psd: bool,
    pub xaxis: XAxis,
    pub multiplier: f64,
}

/// Une carte : `z` est `nf × NBINS`, rangée par fréquence (ligne 0 = la plus basse).
pub struct Map {
    pub freqs: Vec<f64>,
    pub bins: Vec<f64>,
    pub z: Vec<f64>,
    pub counts: Vec<i64>,
}

impl Map {
    /// Moyenne et pic dans une bande de fréquences, sur les tranches non vides (`FreqThrottle.band_stats`).
    pub fn band_stats(&self, f_lo: f64, f_hi: f64) -> (f64, f64) {
        let nb = self.counts.len();
        let (mut sum, mut n, mut peak) = (0.0, 0usize, f64::NAN);
        for (r, &f) in self.freqs.iter().enumerate() {
            if f < f_lo || f > f_hi {
                continue;
            }
            for c in (0..nb).filter(|&c| self.counts[c] > 0) {
                let v = self.z[r * nb + c];
                if !v.is_nan() {
                    sum += v;
                    n += 1;
                    peak = if peak.is_nan() || v > peak { v } else { peak };
                }
            }
        }
        if n == 0 {
            (f64::NAN, f64::NAN)
        } else {
            (sum / n as f64, peak)
        }
    }
}

pub struct Column {
    pub key: &'static str,
    pub label: String,
    pub color: egui::Color32,
    /// Axe → carte.
    pub maps: BTreeMap<usize, Map>,
    pub x_range: (f64, f64),
}

pub struct FreqThrottleOutput {
    pub psd: bool,
    pub xaxis: XAxis,
    pub columns: Vec<Option<Column>>,
    /// Change à chaque calcul : les textures des cartes en dépendent.
    pub generation: u64,
}

static GENERATION: AtomicU64 = AtomicU64::new(1);

/// Valeurs de l'axe X et son intervalle (`_x_values`) ; `None` s'il n'y a pas de moteurs (sauf pour les gaz).
pub fn x_values(log: &FlightLog, sel: &Range<usize>, xaxis: XAxis, multiplier: f64) -> Option<(Vec<f32>, (f64, f64))> {
    if xaxis == XAxis::Throttle {
        return Some((log.throttle.clone(), (0.0, 100.0)));
    }
    if log.n_motors() == 0 {
        return None;
    }
    if xaxis == XAxis::MotorPct {
        return Some((mean_axis0(&log.motors), (0.0, 100.0)));
    }
    let m = mean_axis0(&motor_freqs(log, multiplier)?);
    let part = &m[sel.start.min(m.len())..sel.end.min(m.len())];
    let finite: Vec<f32> = part.iter().copied().filter(|v| !v.is_nan()).collect();
    let top = if part.is_empty() {
        100.0
    } else if finite.is_empty() {
        f32::NAN
    } else {
        percentile_f32(&finite, 99.5)
    };
    let top = (top / 50.0).ceil() * 50.0;
    Some((m, (0.0, f64::from(top).max(50.0))))
}

pub struct FreqThrottleTool {
    columns: [ColumnChoice; N_COLUMNS],
    sub100: [bool; N_COLUMNS],
    scale: [f64; N_COLUMNS],
    zmin: f64,
    psd: bool,
    xaxis: XAxis,
    palette: Palette,
    smoothing: i64,
    flo: f64,
    fhi: f64,
    multiplier: f64,
    axes: AxisToggles,
    files: FileSelection,
    auto_scale: bool,
    preset: Option<usize>,
    /// Texture de chaque carte (colonne, axe) et la clé des réglages qui l'ont produite.
    textures: HashMap<(usize, usize), (u64, TextureHandle)>,
    view: Option<[u64; 3]>,
}

impl Default for FreqThrottleTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FreqThrottleTool {
    pub fn new() -> Self {
        let mut columns = [(None, None); N_COLUMNS];
        for (c, key) in PRESETS[0].1.iter().enumerate() {
            columns[c].0 = static_key(key);
        }
        Self {
            columns,
            sub100: [false; N_COLUMNS],
            scale: [0.5; N_COLUMNS],
            zmin: 0.0,
            psd: false,
            xaxis: XAxis::Throttle,
            palette: Palette::Hot,
            smoothing: 2,
            flo: 20.0,
            fhi: 60.0,
            multiplier: 2.1,
            axes: AxisToggles::default(),
            files: FileSelection::new(true),
            auto_scale: false,
            preset: None,
            textures: HashMap::new(),
            view: None,
        }
    }

    pub fn columns(&self) -> [ColumnChoice; N_COLUMNS] {
        self.columns
    }

    pub fn set_columns(&mut self, c: [ColumnChoice; N_COLUMNS]) {
        self.columns = c;
    }

    pub fn set_xaxis(&mut self, x: XAxis) {
        self.xaxis = x;
    }

    pub fn scale(&self) -> [f64; N_COLUMNS] {
        self.scale
    }

    pub fn zmin(&self) -> f64 {
        self.zmin
    }

    pub fn set_sub100(&mut self, c: usize, on: bool) {
        self.sub100[c] = on;
    }

    /// Applique un préréglage aux colonnes (`_apply_preset`).
    pub fn apply_preset(&mut self, preset: usize, chosen: &[usize], current: usize) {
        self.columns = preset_columns(preset, chosen, current);
    }

    /// Passe de l'amplitude à la PSD : bas de l'échelle −40 / 0 et haut 10 / 0,5 (`_on_mode_toggled`).
    pub fn set_psd(&mut self, psd: bool) {
        if self.psd != psd {
            self.psd = psd;
            self.zmin = if psd { -40.0 } else { 0.0 };
            self.scale = [if psd { 10.0 } else { 0.5 }; N_COLUMNS];
        }
    }

    /// Haut de l'échelle de chaque colonne d'après ses données (`_auto_scale`) : 99e centile (amplitude) ou maximum (PSD)
    /// dans la bande des valeurs « moy. / pic » (les très basses fréquences des mouvements de manche écraseraient sinon
    /// le bruit).
    pub fn auto_scale(&mut self, out: &FreqThrottleOutput) {
        for (c, col) in out.columns.iter().enumerate() {
            let Some(col) = col.as_ref().filter(|c| !c.maps.is_empty()) else {
                continue;
            };
            let (lo, hi) = if self.sub100[c] {
                (self.flo, self.fhi)
            } else {
                (100.0, 1000.0)
            };
            let mut z: Vec<f64> = Vec::new();
            for m in col.maps.values() {
                let nb = m.counts.len();
                for (r, &f) in m.freqs.iter().enumerate() {
                    if f >= lo && f <= hi {
                        z.extend(m.z[r * nb..(r + 1) * nb].iter().filter(|v| v.is_finite()));
                    }
                }
            }
            if z.is_empty() {
                continue;
            }
            self.scale[c] = if out.psd {
                (z.iter().copied().fold(f64::NEG_INFINITY, f64::max) / 5.0).ceil() * 5.0
            } else {
                z.sort_by(|a, b| a.total_cmp(b));
                nice_ceil(percentile_linear(&z, 99.0))
            };
        }
    }

    fn texture(
        &mut self,
        ctx: &egui::Context,
        slot: (usize, usize),
        gen_id: u64,
        z: &[f32],
        nf: usize,
        levels: (f32, f32),
    ) -> TextureHandle {
        let key = gen_id
            ^ (self.smoothing as u64).rotate_left(7)
            ^ u64::from(levels.0.to_bits()).rotate_left(13)
            ^ u64::from(levels.1.to_bits()).rotate_left(29)
            ^ (self.palette as u64).rotate_left(41)
            ^ (nf as u64).rotate_left(51);
        if let Some((k, t)) = self.textures.get(&slot)
            && *k == key
        {
            return t.clone();
        }
        let t = heatmap_texture(
            ctx,
            &format!("carte-{}-{}", slot.0, slot.1),
            z,
            nf,
            NBINS,
            levels.0,
            levels.1,
            self.palette,
            plot::MAP_BACKGROUND,
        );
        self.textures.insert(slot, (key, t.clone()));
        t
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_map(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        out: &FreqThrottleOutput,
        c: usize,
        ax: usize,
        col: &Column,
        levels: (f64, f64),
        h: f32,
        w: f32,
        fresh: bool,
    ) {
        let Some(m) = col.maps.get(&ax) else { return };
        let sub = self.sub100[c];
        let f_top = if sub { 100.0 } else { *m.freqs.last().unwrap_or(&0.0) };
        let n = m.freqs.iter().filter(|&&f| f <= f_top + 1e-9).count();
        let nf = m.freqs.len();
        let smoothed = smooth_map(&m.z, &[nf, NBINS], self.smoothing).unwrap_or_else(|_| m.z.clone());
        let z: Vec<f32> = smoothed[..n * NBINS].iter().map(|&v| v as f32).collect();
        let tex = self.texture(
            ui.ctx(),
            (c, ax),
            out.generation,
            &z,
            n,
            (levels.0 as f32, levels.1 as f32),
        );
        let df = if n > 1 { m.freqs[1] - m.freqs[0] } else { 1.0 };
        let (x0, x1) = col.x_range;
        let y0 = m.freqs.first().copied().unwrap_or(0.0) - df / 2.0;
        let resp = nice_axes(Plot::new(("fgaz", c, ax)))
            .width(w)
            .height(h)
            .auto_bounds(false)
            .x_axis_label(out.xaxis.label())
            .y_axis_label(format!("{} | fréq. (Hz)", AXIS_NAMES[ax]))
            .grid_color(theme.grid_color())
            .show(ui, |pu| {
                if fresh {
                    pu.set_plot_bounds_x(x0..=x1);
                    pu.set_plot_bounds_y(0.0..=f_top);
                }
                pu.image(PlotImage::new(
                    "",
                    tex.id(),
                    PlotPoint::new((x0 + x1) / 2.0, y0 + df * n as f64 / 2.0),
                    [(x1 - x0) as f32, (df * n as f64) as f32],
                ));
            });
        let (lo, hi) = if sub { (self.flo, self.fhi) } else { (100.0, 1000.0) };
        let (mean, peak) = m.band_stats(lo, hi);
        corner_text(
            ui,
            *resp.transform.frame(),
            Align2::RIGHT_TOP,
            &[
                (format!("moy.={}", fmt3(mean)), plot::OVERLAY),
                (format!("pic={}", fmt3(peak)), plot::OVERLAY),
            ],
            plot::OVERLAY_FILL,
        );
    }
}

impl Tool for FreqThrottleTool {
    type Options = Options;
    type Output = FreqThrottleOutput;
    const TITLE: &'static str = "Fréquence × gaz";
    const HIDDEN: bool = true;
    const TAB: Option<&'static str> = Some("Analyse spectrale");

    fn options(&self) -> Options {
        Options {
            columns: self.columns,
            psd: self.psd,
            xaxis: self.xaxis,
            multiplier: self.multiplier,
        }
    }

    /// Tous les logs ouverts : chaque colonne désigne le sien par son indice (comme Python, qui lit `state.logs`).
    fn selected(&self, state: &AppState) -> Vec<usize> {
        (0..state.logs.len()).collect()
    }

    fn compute(o: Options, logs: Vec<LogInput>) -> Result<FreqThrottleOutput, String> {
        let mode = if o.psd { Mode::Psd } else { Mode::Amp };
        let mut columns = Vec::new();
        for (key, fi) in o.columns {
            let (Some(key), Some(li)) = (key, fi.and_then(|i| logs.get(i))) else {
                columns.push(None);
                continue;
            };
            let log = &*li.log;
            let sel = log.slice_time(li.period.0, li.period.1);
            let Some((xv, x_range)) = x_values(log, &sel, o.xaxis, o.multiplier) else {
                columns.push(None);
                continue;
            };
            if datasets::get(log, key)?.is_none() {
                columns.push(None);
                continue;
            }
            let xs = &xv[sel.start.min(xv.len())..sel.end.min(xv.len())];
            let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
            let mut maps = BTreeMap::new();
            for ax in 0..3 {
                if !datasets::has_axis(log, key, ax)? {
                    continue;
                }
                let Some(data) = datasets::get(log, key)? else { continue };
                let row = data.row(ax);
                let x = wide(&row[sel.start.min(row.len())..sel.end.min(row.len())]);
                let m = freq_throttle(&x, &wide(xs), log.fs, mode, 300.0, NBINS, x_range, Some(&br))?;
                let z = if o.psd { to_ptb_db(&m.z) } else { m.z };
                maps.insert(
                    ax,
                    Map {
                        freqs: m.freqs,
                        bins: m.bins,
                        z,
                        counts: m.counts,
                    },
                );
            }
            columns.push(Some(Column {
                key,
                label: li.label.clone(),
                color: li.color,
                maps,
                x_range,
            }));
        }
        Ok(FreqThrottleOutput {
            psd: o.psd,
            xaxis: o.xaxis,
            columns,
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
        })
    }

    fn controls(&mut self, ui: &mut Ui, state: &AppState, theme: &Theme) {
        let current = state.current.unwrap_or(0).min(state.logs.len().saturating_sub(1));
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.files.ui(ui, state);
            ui.separator();
            self.axes.ui(ui);
            ui.separator();
            ui.label(RichText::new("Colonnes : données, fichier, échelle").strong());
            for c in 0..N_COLUMNS {
                // un fichier absent ou fermé : le log courant (`_refresh_files`)
                if self.columns[c].1.is_none_or(|f| f >= state.logs.len()) && !state.logs.is_empty() {
                    self.columns[c].1 = Some(current);
                }
                ui.horizontal(|ui| {
                    ui.label(format!("{}", c + 1));
                    let shown = self.columns[c]
                        .0
                        .and_then(|k| DATASETS.iter().find(|(d, _)| *d == k))
                        .map_or("(vide)", |(_, l)| *l);
                    egui::ComboBox::from_id_salt(("col-ds", c))
                        .selected_text(shown)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.columns[c].0, None, "(vide)");
                            for (k, l) in DATASETS {
                                ui.selectable_value(&mut self.columns[c].0, Some(k), l);
                            }
                        });
                    ui.checkbox(&mut self.sub100[c], "< 100 Hz");
                });
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    let name = self.columns[c]
                        .1
                        .and_then(|f| state.logs.get(f))
                        .map_or("", |l| l.label.as_str());
                    egui::ComboBox::from_id_salt(("col-file", c))
                        .selected_text(name)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for (i, l) in state.logs.iter().enumerate() {
                                ui.selectable_value(&mut self.columns[c].1, Some(i), l.label.as_str());
                            }
                        });
                    ui.add(
                        egui::DragValue::new(&mut self.scale[c])
                            .range(-200.0..=1e4)
                            .speed(0.05)
                            .max_decimals(2),
                    )
                    .on_hover_text("Échelle : haut de la palette de couleurs de cette colonne");
                });
            }
            let shown = self.preset.map_or("Préréglages…", |i| PRESETS[i].0);
            egui::ComboBox::from_id_salt("preset")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (i, (name, ..)) in PRESETS.iter().enumerate() {
                        if ui.selectable_label(false, *name).clicked() {
                            let chosen = self.files.indices(state);
                            self.apply_preset(i, &chosen, current);
                        }
                    }
                });
            if ui
                .button("Échelle auto")
                .on_hover_text("Adapte l'échelle de chaque colonne à ses données (sinon 0,5 comme PIDtoolbox).")
                .clicked()
            {
                self.auto_scale = true;
            }
            ui.separator();
            let mut psd = self.psd;
            ui.horizontal(|ui| {
                ui.radio_value(&mut psd, false, "Amplitude");
                ui.radio_value(&mut psd, true, "PSD (dB)");
            });
            self.set_psd(psd);
            egui::Grid::new("fgaz-options").show(ui, |ui| {
                ui.label("Axe X");
                egui::ComboBox::from_id_salt("xaxis")
                    .selected_text(
                        XAxis::ALL
                            .iter()
                            .find(|(x, _)| *x == self.xaxis)
                            .map_or("", |(_, l)| *l),
                    )
                    .show_ui(ui, |ui| {
                        for (x, l) in XAxis::ALL {
                            ui.selectable_value(&mut self.xaxis, x, l);
                        }
                    });
                ui.end_row();
                ui.label("Couleurs");
                egui::ComboBox::from_id_salt("palette")
                    .selected_text(self.palette.name())
                    .show_ui(ui, |ui| {
                        for p in Palette::ALL {
                            ui.selectable_value(&mut self.palette, p, p.name());
                        }
                    });
                ui.end_row();
                ui.label("Lissage");
                egui::ComboBox::from_id_salt("lissage-carte")
                    .selected_text(
                        SMOOTHING
                            .iter()
                            .find(|(v, _)| *v == self.smoothing)
                            .map_or("", |(_, l)| *l),
                    )
                    .show_ui(ui, |ui| {
                        for (v, l) in SMOOTHING {
                            ui.selectable_value(&mut self.smoothing, v, l);
                        }
                    });
                ui.end_row();
                ui.label("Bas de l'échelle");
                ui.add(
                    egui::DragValue::new(&mut self.zmin)
                        .range(-200.0..=1e4)
                        .speed(0.05)
                        .max_decimals(2),
                );
                ui.end_row();
            });
            ui.label("Bande moy./pic < 100 Hz (Hz)");
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut self.flo)
                        .range(0.0..=100.0)
                        .speed(1.0)
                        .max_decimals(0),
                );
                ui.add(
                    egui::DragValue::new(&mut self.fhi)
                        .range(0.0..=100.0)
                        .speed(1.0)
                        .max_decimals(0),
                );
            });
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
                RichText::new("Les cartes montrent le niveau (amplitude ou PSD) selon les gaz ou le régime moteur.")
                    .color(theme.ui.muted),
            );
        });
    }

    fn render(&mut self, ui: &mut Ui, out: &FreqThrottleOutput, theme: &Theme) {
        if std::mem::take(&mut self.auto_scale) {
            self.auto_scale(out);
        }
        let axes = self.axes.enabled();
        let spacing = ui.spacing().item_spacing;
        let colw = ((ui.available_width() - spacing.x * (N_COLUMNS as f32 - 1.0)) / N_COLUMNS as f32).max(160.0);
        let head = 22.0 + 56.0;
        let h = if axes.is_empty() {
            100.0
        } else {
            ((ui.available_height() - head - spacing.y * 3.0 * axes.len() as f32) / axes.len() as f32).max(110.0)
        };
        let key = [
            out.generation,
            u64::from(
                self.sub100
                    .iter()
                    .enumerate()
                    .fold(0u8, |a, (i, b)| a | (u8::from(*b) << i)),
            ),
            u64::from(out.xaxis as u8) ^ (self.smoothing as u64) << 8,
        ];
        let fresh = self.view != Some(key);
        ui.horizontal_top(|ui| {
            for c in 0..N_COLUMNS {
                ui.allocate_ui_with_layout(
                    egui::vec2(colw, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(colw);
                        let Some(col) = out.columns[c].as_ref() else { return };
                        let lo = self.zmin;
                        let levels = (lo, self.scale[c].max(lo + 1e-6));
                        let title = DATASETS
                            .iter()
                            .find(|(k, _)| *k == col.key)
                            .map_or(col.key, |(_, l)| *l);
                        ui.label(
                            RichText::new(format!("{title} | {}", col.label))
                                .color(col.color)
                                .strong(),
                        );
                        colorbar(ui, self.palette, levels.0, levels.1, colw - 8.0, false, "");
                        for &ax in &axes {
                            self.draw_map(ui, theme, out, c, ax, col, levels, h, colw - 8.0, fresh);
                        }
                    },
                );
            }
        });
        self.view = Some(key);
    }
}
