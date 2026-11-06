//! Outil « Simulateur de filtres » (port de `gui/tools/filter_sim.py`, partie interface ; les calculs sont dans `filter_sim_calc`).
//!
//! Un seul log. Les champs du panneau de droite modifient les filtres (gyro, D, notchs, notch dynamique, filtre RPM) :
//! - à gauche, les diagrammes de Bode des chaînes gyro et D (réglages du log en trait plein, réglages modifiés en tirets),
//!   et les retards à 20 / 50 / 100 Hz : redessinés tout de suite, calcul léger fait dans `render` ;
//! - à droite, les spectres enregistrés et le spectre « et si » : calcul lourd (≈ 1 s) lancé seulement par « Appliquer ».
//!
//! L'état de l'éditeur vit dans l'outil, pas dans `Options` : `Options` ne reçoit les réglages qu'à « Appliquer », ce qui
//! évite tout recalcul pendant la saisie.

use otb_format::fixed;
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, RichText, TextStyle, Ui};
use egui_plot::{GridMark, Legend, Line, LineStyle, Plot, PlotPoints};
use otb_analysis::motors::hover_motor_hz;
use otb_log::settings::Settings;

use crate::plots::{format_hz, log_grid, nice_axes, nice_grid};
use crate::state::AppState;
use crate::theme::{AXIS_NAMES, Theme, plot};
use crate::tool::{LogInput, Tool};
use crate::tools::common::{AxisToggles, FileSelection, PTB_DB_RANGE, safe_text};
use crate::tools::filter_sim_calc::{
    Bode, DELAY_FREQS, DYN_COUNT_MAX, DYN_COUNT_MAX_44, DYN_MAX_RANGE, DYN_MIN_RANGE, DYN_Q_RANGE, Delays, EXPO_RANGE,
    Editor, FilterSimResult, HZ_RANGE, LPF_MODES, LPF_TYPES, Lpf1Edit, MOTOR_HZ_DEFAULT, MOTOR_HZ_RANGE,
    RPM_HARM_RANGE, RPM_MIN_RANGE, RPM_Q_RANGE, RPM_WEIGHT_RANGE, THROTTLE_DEFAULT, THROTTLE_RANGE, bode, cli_values,
    delays, filter_cli, simulate_filters,
};

/// Texte d'aide du panneau (mot pour mot celui de Python).
pub const HELP: &str =
    "Bode : trait plein = log, tirets = modifié. Spectres : « Appliquer » relance la simulation (≈ 1 s).";
/// Info-bulle de la case « simulation réglages actuels ».
pub const CURRENT_HINT: &str = "La simulation tourne à la cadence du log : comparez « simulé modifié » à « simulé actuel » plutôt qu'au gyro du log.";
/// Statut quand l'éditeur diffère du dernier calcul.
pub const STALE_STATUS: &str = "Réglages modifiés : cliquez sur « Appliquer » pour simuler les spectres.";
pub const COPIED_STATUS: &str = "Commandes CLI copiées dans le presse-papiers.";
/// Note grisée sous le tableau des retards.
pub const DELAY_NOTE: &str = "Spectres simulés = spectre enregistré × (filtres modifiés / filtres actuels), calculé à la cadence PID fenêtre par fenêtre.";
/// Titre du spectre gyro quand le log n'a pas de gyro non filtré.
pub const NO_UNFILT_TITLE: &str = "Spectre gyro — pas de gyro non filtré dans ce log : pas de simulation";
/// Écart (ms) au-delà duquel un retard est vert (plus court) ou rouge (plus long).
const DELAY_TOL_MS: f64 = 0.005;
/// Largeurs relatives des colonnes de graphiques : gain, phase, spectre.
const COL_WEIGHTS: [f32; 3] = [2.0, 2.0, 3.0];
/// Graduations de fréquence des diagrammes de Bode.
const BODE_Y_GAIN: (f64, f64) = (-40.0, 3.0);
const BODE_Y_PHASE: (f64, f64) = (-270.0, 10.0);
const BODE_X_MIN_HZ: f64 = 10.0;
/// Marge sous la borne basse des Bode (part de la hauteur) : sans elle, l'étiquette du bas de l'axe Y (« -40 ») et celle de
/// l'axe X (« 10 ») se touchent dans le coin (W-036 (5)).
const BODE_Y_MARGIN: f64 = 0.06;

/// Bornes Y d'un diagramme de Bode (`col` 0 : gain, 1 : phase), marge sous la borne basse comprise.
pub fn bode_y_bounds(col: usize) -> (f64, f64) {
    let (lo, hi) = if col == 0 { BODE_Y_GAIN } else { BODE_Y_PHASE };
    (lo - BODE_Y_MARGIN * (hi - lo), hi)
}

/// Le résultat affiché vient-il d'un autre log que celui choisi ? Pendant le calcul d'un nouveau log, la page garde l'ancien
/// résultat : on le masque plutôt que de montrer ses spectres sous les filtres du nouveau (W-036 (8)).
pub fn result_is_for_other_log(result_log: u64, loaded: Option<u64>) -> bool {
    loaded.is_some_and(|id| id != result_log)
}

/// Réglages envoyés au calcul par « Appliquer », avec le log auquel ils s'appliquent.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    /// Identifiant du chargement du log (`LoadedLog::uid`) : des réglages d'un autre log ne servent pas au nouveau (L-072).
    pub log_id: u64,
    pub settings: Settings,
}

/// `None` : l'éditeur tel que chargé depuis le log (rien d'appliqué).
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub applied: Option<Applied>,
    pub with_current: bool,
}

/// Bode des quatre chaînes : réglages du log et réglages modifiés.
struct BodeSet {
    cur: (Bode, Bode),
    modf: (Bode, Bode),
}

/// Ce qui se déduit de l'éditeur et des réglages de Bode : recalculé seulement quand l'un d'eux change.
struct Derived {
    key: (Editor, u64, i64, i64, bool),
    cli: String,
    bode: Result<BodeSet, String>,
    delays: Result<Delays, String>,
}

pub struct FilterSimTool {
    files: FileSelection,
    axes: AxisToggles,
    tab: usize,
    editor: Editor,
    /// Adresse du log dont l'éditeur est chargé.
    loaded: Option<u64>,
    base: Option<Settings>,
    applied: Option<Applied>,
    /// Éditeur au moment du dernier « Appliquer » (ou du chargement) : ce que le dernier calcul a simulé.
    applied_editor: Editor,
    throttle: i64,
    motor_hz: i64,
    bode_dyn: bool,
    with_current: bool,
    message: String,
    derived: Option<Derived>,
    /// Résultat dessiné (gardé vivant : son adresse ne peut pas resservir), axes cochés, case « actuel ».
    spectrum_view: Option<(Arc<FilterSimResult>, u8, bool)>,
    bode_view: Option<(u64, u64)>,
    link: [egui::Id; 2],
}

impl Default for FilterSimTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Liste déroulante de `items` ; vrai si le choix a changé.
fn combo(ui: &mut Ui, id: &str, value: &mut usize, items: &[&str], name: &str) -> bool {
    let before = *value;
    egui::ComboBox::from_id_salt(id)
        .selected_text(safe_text(items[(*value).min(items.len() - 1)]))
        .show_ui(ui, |ui| {
            for (i, it) in items.iter().enumerate() {
                ui.selectable_value(value, i, safe_text(it));
            }
        })
        .response
        .accessible_name(name);
    *value != before
}

/// Champ entier borné ; vrai si la valeur a changé.
fn drag(ui: &mut Ui, v: &mut i64, (lo, hi): (i64, i64), suffix: &str, name: &str) -> bool {
    ui.add(egui::DragValue::new(v).range(lo..=hi).suffix(suffix))
        .accessible_name(name)
        .changed()
}

/// Les six champs d'un LPF1.
fn lpf1_grid(ui: &mut Ui, id: &str, who: &str, e: &mut Lpf1Edit) -> bool {
    let mut ch = false;
    ui.label(RichText::new("LPF1").strong());
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        ui.label("Type");
        ch |= combo(
            ui,
            &format!("{id}-type"),
            &mut e.kind,
            &LPF_TYPES,
            &format!("{who} LPF1 type"),
        );
        ui.end_row();
        ui.label("Mode");
        ch |= combo(
            ui,
            &format!("{id}-mode"),
            &mut e.mode,
            &LPF_MODES,
            &format!("{who} LPF1 mode"),
        );
        ui.end_row();
        ui.label("Statique");
        ch |= drag(ui, &mut e.static_hz, HZ_RANGE, " Hz", &format!("{who} LPF1 statique"));
        ui.end_row();
        ui.label("Dyn. min");
        ch |= drag(ui, &mut e.min_hz, HZ_RANGE, " Hz", &format!("{who} LPF1 dynamique min"));
        ui.end_row();
        ui.label("Dyn. max");
        ch |= drag(ui, &mut e.max_hz, HZ_RANGE, " Hz", &format!("{who} LPF1 dynamique max"));
        ui.end_row();
        ui.label("Expo");
        ch |= drag(ui, &mut e.expo, EXPO_RANGE, "", &format!("{who} LPF1 expo"));
        ui.end_row();
    });
    ch
}

/// LPF2 (type et coupure).
fn lpf2_grid(ui: &mut Ui, id: &str, who: &str, kind: &mut usize, hz: &mut i64) -> bool {
    let mut ch = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new("LPF2").strong());
        ui.label("(0 = désactivé)");
    });
    egui::Grid::new(id).num_columns(2).show(ui, |ui| {
        ui.label("Type");
        ch |= combo(ui, &format!("{id}-type"), kind, &LPF_TYPES, &format!("{who} LPF2 type"));
        ui.end_row();
        ui.label("Coupure");
        ch |= drag(ui, hz, HZ_RANGE, " Hz", &format!("{who} LPF2 coupure"));
        ui.end_row();
    });
    ch
}

/// Un notch : centre et coupure côte à côte.
fn notch_row(ui: &mut Ui, label: &str, name: &str, centre: &mut i64, cut: &mut i64) -> bool {
    let mut ch = false;
    ui.label(label);
    ui.horizontal(|ui| {
        ch |= drag(ui, centre, HZ_RANGE, "", &format!("{name} centre"));
        ch |= drag(ui, cut, HZ_RANGE, "", &format!("{name} coupure"));
    });
    ui.end_row();
    ch
}

/// Points `[log10(f), y]` d'une courbe sur axe logarithmique (les fréquences < 1 Hz sont écartées).
fn log_points64(f: &[f64], y: &[f64]) -> Vec<[f64; 2]> {
    f.iter()
        .zip(y)
        .filter(|(f, _)| **f >= 1.0)
        .map(|(f, y)| [f.log10(), *y])
        .collect()
}

/// Graduations 10, 20, 50, 100… de l'axe des fréquences : toutes de même rang, donc toutes étiquetées (comme les graduations
/// fixes de Python) ; avec `log_grid` seul, egui_plot n'étiquette ni 20 ni 50 sur un graphique étroit.
fn bode_grid(input: egui_plot::GridInput) -> Vec<GridMark> {
    log_grid(input)
        .into_iter()
        .map(|m| GridMark { step_size: 1.0, ..m })
        .collect()
}

/// Moyenne en linéaire des spectres (dB) des axes choisis, rendue en dB : `10 log10(max(mean(10^(dB/10)), 1e-12))`.
pub fn mean_db(arrs: &[&[f64]]) -> Vec<f64> {
    let n = arrs.iter().map(|a| a.len()).min().unwrap_or(0);
    (0..n)
        .map(|i| {
            let lin = arrs.iter().map(|a| 10f64.powf(a[i] / 10.0)).sum::<f64>() / arrs.len() as f64;
            10.0 * lin.max(1e-12).log10()
        })
        .collect()
}

/// Sens d'un changement de retard : plus court, plus long, ou inchangé à `DELAY_TOL_MS` près.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    Shorter,
    Longer,
    Same,
}

pub fn delay_trend(actual: f64, modified: f64) -> Trend {
    if modified < actual - DELAY_TOL_MS {
        Trend::Shorter
    } else if modified > actual + DELAY_TOL_MS {
        Trend::Longer
    } else {
        Trend::Same
    }
}

/// Échelle verticale des spectres simulés : celle de PIDtoolbox, la même que le Spectre (2026-11-06).
/// ⚠️ Pas d'échelle calée sur le sommet des courbes : deux pages, ou les deux lignes gyro et D, ne se comparaient plus.
pub fn y_bounds() -> (f64, f64) {
    PTB_DB_RANGE
}

/// Courbe d'un spectre : nom, couleur, style, épaisseur et points.
struct Series {
    label: &'static str,
    color: Color32,
    style: LineStyle,
    width: f32,
    pts: Vec<[f64; 2]>,
}

impl FilterSimTool {
    /// Aucune vue de graphique retenue (au départ, ou après « Tout voir »).
    pub fn view_is_unset(&self) -> bool {
        self.spectrum_view.is_none() && self.bode_view.is_none()
    }

    pub fn new() -> Self {
        let editor = Editor::from_settings(&Settings::default());
        Self {
            files: FileSelection::new(false),
            axes: AxisToggles::default(),
            tab: 0,
            applied_editor: editor.clone(),
            editor,
            loaded: None,
            base: None,
            applied: None,
            throttle: THROTTLE_DEFAULT,
            motor_hz: MOTOR_HZ_DEFAULT,
            bode_dyn: false,
            with_current: true,
            message: String::new(),
            derived: None,
            spectrum_view: None,
            bode_view: None,
            link: [
                egui::Id::unique("filtres-lien-gyro"),
                egui::Id::unique("filtres-lien-d"),
            ],
        }
    }

    fn family(&self) -> &'static str {
        self.base.as_ref().map_or("", |b| b.family())
    }

    /// Nouveau log : l'éditeur est rechargé depuis ses réglages (`_ensure_base`), le moteur préréglé sur son vol stationnaire.
    fn sync(&mut self, state: &AppState) {
        let Some(&i) = self.files.indices(state).first() else {
            self.base = None;
            self.loaded = None;
            return;
        };
        let l = &state.logs[i];
        let id = l.uid;
        if self.loaded == Some(id) {
            return;
        }
        self.loaded = Some(id);
        let base = l.log.settings.clone();
        self.editor = Editor::from_settings(&base);
        self.applied_editor = self.editor.clone();
        self.applied = None;
        self.motor_hz = hover_motor_hz(&l.log)
            .filter(|h| *h != 0.0)
            .map_or(MOTOR_HZ_DEFAULT, |h| h.round_ties_even() as i64)
            .clamp(MOTOR_HZ_RANGE.0, MOTOR_HZ_RANGE.1);
        self.base = Some(base);
        self.message.clear();
        self.derived = None;
    }

    /// L'éditeur diffère-t-il de ce que le dernier calcul a simulé ?
    fn stale(&self) -> bool {
        let Some(base) = &self.base else { return false };
        if self.editor == self.applied_editor {
            return false;
        }
        cli_values(&self.editor.modified_settings(base)) != cli_values(&self.applied_editor.modified_settings(base))
    }

    /// Texte du cadre CLI, Bode et retards pour l'état courant (recalculés seulement si l'éditeur, les gaz ou le moteur changent).
    fn refresh(&mut self) {
        let Some(base) = &self.base else { return };
        let id = self.loaded.unwrap_or(0);
        let key = (self.editor.clone(), id, self.throttle, self.motor_hz, self.bode_dyn);
        if self.derived.as_ref().is_some_and(|d| d.key == key) {
            return;
        }
        let modf = self.editor.modified_settings(base);
        let (thr, motor, dynamic) = (self.throttle, self.motor_hz as f64, self.bode_dyn);
        let bode = (|| -> Result<BodeSet, String> {
            Ok(BodeSet {
                cur: bode(base, thr, motor, dynamic)?,
                modf: bode(&modf, thr, motor, dynamic)?,
            })
        })();
        self.derived = Some(Derived {
            cli: filter_cli(base, &modf),
            bode,
            delays: delays(base, &modf, thr, motor, dynamic),
            key,
        });
    }

    // ------------------------------------------------------------ panneau

    fn tab_fields(&mut self, ui: &mut Ui) -> bool {
        let e = &mut self.editor;
        match self.tab {
            0 => {
                let mut ch = lpf1_grid(ui, "filtres-g1", "Gyro", &mut e.g1);
                ch |= lpf2_grid(ui, "filtres-g2", "Gyro", &mut e.g2_type, &mut e.g2_hz);
                ui.label(RichText::new("Notchs statiques").strong());
                ui.label("(centre / coupure)");
                egui::Grid::new("filtres-gn").num_columns(2).show(ui, |ui| {
                    ch |= notch_row(ui, "Notch 1", "Gyro notch 1", &mut e.n1_hz, &mut e.n1_cut);
                    ch |= notch_row(ui, "Notch 2", "Gyro notch 2", &mut e.n2_hz, &mut e.n2_cut);
                });
                ch
            }
            1 => {
                let mut ch = lpf1_grid(ui, "filtres-d1", "D", &mut e.d1);
                ch |= lpf2_grid(ui, "filtres-d2", "D", &mut e.d2_type, &mut e.d2_hz);
                ui.label(RichText::new("Notch D").strong());
                ui.label("(centre / coupure)");
                egui::Grid::new("filtres-dn").num_columns(2).show(ui, |ui| {
                    ch |= notch_row(ui, "Notch", "D notch", &mut e.dn_hz, &mut e.dn_cut);
                });
                ch
            }
            _ => {
                let mut ch = false;
                let max = if self.base.as_ref().is_some_and(|b| b.family() == "4.4") {
                    DYN_COUNT_MAX_44
                } else {
                    DYN_COUNT_MAX
                };
                ui.label(RichText::new("Notch dynamique").strong());
                egui::Grid::new("filtres-dyn").num_columns(2).show(ui, |ui| {
                    ui.label("Nombre");
                    ch |= drag(ui, &mut e.dyn_count, (0, max), "", "Notch dynamique nombre");
                    ui.end_row();
                    ui.label("Q");
                    ch |= drag(ui, &mut e.dyn_q, DYN_Q_RANGE, "", "Notch dynamique Q");
                    ui.end_row();
                    ui.label("Min");
                    ch |= drag(ui, &mut e.dyn_min, DYN_MIN_RANGE, " Hz", "Notch dynamique min");
                    ui.end_row();
                    ui.label("Max");
                    ch |= drag(ui, &mut e.dyn_max, DYN_MAX_RANGE, " Hz", "Notch dynamique max");
                    ui.end_row();
                });
                ui.label(RichText::new("Filtre RPM").strong());
                egui::Grid::new("filtres-rpm").num_columns(2).show(ui, |ui| {
                    ui.label("Harmoniques");
                    ch |= drag(ui, &mut e.rpm_harm, RPM_HARM_RANGE, "", "Filtre RPM harmoniques");
                    ui.end_row();
                    ui.label("Q");
                    ch |= drag(ui, &mut e.rpm_q, RPM_Q_RANGE, "", "Filtre RPM Q");
                    ui.end_row();
                    ui.label("Min");
                    ch |= drag(ui, &mut e.rpm_min, RPM_MIN_RANGE, " Hz", "Filtre RPM min");
                    ui.end_row();
                    ui.label("Poids %");
                    ui.horizontal(|ui| {
                        for (i, w) in e.rpm_w.iter_mut().enumerate() {
                            ch |= drag(ui, w, RPM_WEIGHT_RANGE, "", &format!("Filtre RPM poids {}", i + 1));
                        }
                    });
                    ui.end_row();
                });
                ch
            }
        }
    }

    // ------------------------------------------------------------ graphiques

    /// Courbes d'un spectre (gyro ou D) pour les axes choisis, et leur plus haut niveau au-dessus de 2 Hz.
    fn series(&self, out: &FilterSimResult, gyro: bool, theme: &Theme) -> Vec<Series> {
        let axes = self.axes.enabled();
        let spec: Vec<(&str, &'static str, Color32, LineStyle, f32)> = if gyro {
            let mut v = vec![
                (
                    "gyro_pf",
                    "gyro avant filtres (log)",
                    plot::NEUTRAL,
                    LineStyle::Solid,
                    1.5,
                ),
                ("gyro", "gyro filtré (log)", theme.fg, LineStyle::Solid, 2.0),
                (
                    "gyro_sim",
                    "gyro simulé (modifié)",
                    plot::MODIFIED,
                    LineStyle::dashed_dense(),
                    2.0,
                ),
            ];
            if self.with_current {
                v.push((
                    "gyro_sim_cur",
                    "gyro simulé (actuel)",
                    plot::SIM_CURRENT,
                    LineStyle::dotted_dense(),
                    2.0,
                ));
            }
            v
        } else {
            let mut v = vec![
                ("d_pf", "D avant filtres (log)", plot::NEUTRAL, LineStyle::Solid, 1.5),
                ("d", "D filtré (log)", theme.fg, LineStyle::Solid, 2.0),
                (
                    "d_sim",
                    "D simulé (modifié)",
                    plot::MODIFIED,
                    LineStyle::dashed_dense(),
                    2.0,
                ),
            ];
            if self.with_current {
                v.push((
                    "d_sim_cur",
                    "D simulé (actuel)",
                    plot::SIM_CURRENT,
                    LineStyle::dotted_dense(),
                    2.0,
                ));
            }
            v
        };
        let mut lines = Vec::new();
        for (key, label, color, style, width) in spec {
            let arrs: Vec<&[f64]> = axes.iter().filter_map(|&a| out.spectrum(key, a)).collect();
            if arrs.is_empty() || out.freqs.is_empty() {
                continue;
            }
            let db = mean_db(&arrs);
            let pts = out.freqs.iter().zip(&db).map(|(f, v)| [*f, *v]).collect();
            lines.push(Series {
                label,
                color,
                style,
                width,
                pts,
            });
        }
        lines
    }

    fn plots(&mut self, ui: &mut Ui, out: &Arc<FilterSimResult>, theme: &Theme) {
        let sp = ui.spacing().item_spacing;
        let total = (ui.available_width() - 2.0 * sp.x).max(360.0);
        let wsum: f32 = COL_WEIGHTS.iter().sum();
        let w: Vec<f32> = COL_WEIGHTS.iter().map(|k| (total * k / wsum).max(110.0)).collect();
        let h = ((ui.available_height() - 2.0 * (22.0 + 2.0 * sp.y)) / 2.0).clamp(110.0, 600.0);
        let nyq = self.base.as_ref().map_or(out.fs / 2.0, |b| b.pid_rate_hz() / 2.0);
        let axes_mask = self
            .axes
            .0
            .iter()
            .enumerate()
            .fold(0u8, |m, (i, &on)| m | (u8::from(on) << i));
        let fresh_spec = !matches!(&self.spectrum_view,
            Some((a, m, w)) if Arc::ptr_eq(a, out) && *m == axes_mask && *w == self.with_current);
        let bode_key = (self.loaded.unwrap_or(0), nyq.to_bits());
        let fresh_bode = self.bode_view != Some(bode_key);
        let lab: String = self
            .axes
            .enabled()
            .iter()
            .map(|&a| AXIS_NAMES[a][..1].to_string())
            .collect::<Vec<_>>()
            .join("+");
        for (row, gyro) in [true, false].into_iter().enumerate() {
            let who = if gyro { "Gyro" } else { "D" };
            let alert = gyro && !out.has_unfilt;
            ui.horizontal(|ui| {
                let titles = [
                    (format!("Filtres {who} — gain"), theme.fg),
                    (format!("Filtres {who} — phase"), theme.fg),
                    if alert {
                        (NO_UNFILT_TITLE.to_string(), theme.accent("red"))
                    } else {
                        (format!("Spectre {} ({lab})", if gyro { "gyro" } else { "D" }), theme.fg)
                    },
                ];
                for ((t, c), w) in titles.into_iter().zip(&w) {
                    ui.add_sized(
                        [*w, 18.0],
                        egui::Label::new(RichText::new(t).strong().color(c)).truncate(),
                    );
                }
            });
            let link = self.link[row];
            let lines = self.series(out, gyro, theme);
            ui.horizontal(|ui| {
                for (col, name) in ["gain", "phase"].into_iter().enumerate() {
                    let (y_lo, y_hi) = bode_y_bounds(col);
                    let resp = nice_axes(Plot::new(("filtres-bode", row, col)))
                        .width(w[col])
                        .height(h)
                        .auto_bounds(false)
                        .x_axis_label("Fréquence (Hz)")
                        .y_axis_label(if col == 0 { "Gain (dB)" } else { "Phase (°)" })
                        .x_grid_spacer(bode_grid)
                        .x_axis_formatter(|m, _| format_hz(10f64.powf(m.value)))
                        .y_grid_spacer(|i| nice_grid(i, 3.0))
                        .label_formatter(move |p| {
                            let pos = match p {
                                egui_plot::HoverPosition::NearDataPoint { position, .. }
                                | egui_plot::HoverPosition::Elsewhere { position } => position,
                            };
                            let unit = if col == 0 { "dB" } else { "°" };
                            Some(format!("{:.1} Hz · {:.1} {unit}", 10f64.powf(pos.x), pos.y))
                        })
                        .link_axis(link, [true, false])
                        .grid_color(theme.grid_color())
                        .show(ui, |pu| {
                            if fresh_bode {
                                pu.set_plot_bounds_x(BODE_X_MIN_HZ.log10()..=nyq.log10());
                                pu.set_plot_bounds_y(y_lo..=y_hi);
                            }
                            let Some(Ok(set)) = self.derived.as_ref().map(|d| &d.bode) else {
                                return;
                            };
                            let pick = |b: &(Bode, Bode)| if gyro { b.0.clone() } else { b.1.clone() };
                            for (b, color, style) in [
                                (pick(&set.cur), theme.fg, LineStyle::Solid),
                                (pick(&set.modf), plot::MODIFIED, LineStyle::dashed_dense()),
                            ] {
                                let y = if col == 0 { &b.gain_db } else { &b.phase_deg };
                                pu.line(
                                    Line::new(name, PlotPoints::from(log_points64(&b.freqs, y)))
                                        .color(color)
                                        .width(2.0)
                                        .style(style),
                                );
                            }
                        });
                    let _ = resp;
                }
                // spectre
                let plot = nice_axes(Plot::new(("filtres-spectre", row)))
                    .width(w[2])
                    .height(h)
                    .auto_bounds(false)
                    .x_axis_label("Fréquence (Hz)")
                    .y_axis_label("PSD (dB)")
                    .legend(Legend::default())
                    .grid_color(theme.grid_color());
                plot.show(ui, |pu| {
                    if fresh_spec {
                        pu.set_plot_bounds_x(0.0..=out.fs / 2.0);
                        let (lo, hi) = y_bounds();
                        pu.set_plot_bounds_y(lo..=hi);
                    }
                    for s in lines {
                        pu.line(
                            Line::new(s.label, PlotPoints::from(s.pts))
                                .color(s.color)
                                .width(s.width)
                                .style(s.style),
                        );
                    }
                });
            });
        }
        self.spectrum_view = Some((out.clone(), axes_mask, self.with_current));
        self.bode_view = Some(bode_key);
    }

    // ------------------------------------------------------------ bas : CLI et retards

    fn bottom(&mut self, ui: &mut Ui, theme: &Theme) {
        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            let cli = self.derived.as_ref().map(|d| d.cli.clone()).unwrap_or_default();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.label("CLI des filtres modifiés :");
                    if ui.button("Copier").clicked() {
                        ui.ctx().copy_text(cli.clone());
                        self.message = COPIED_STATUS.to_string();
                    }
                });
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .id_salt("filtres-cli")
                    .show(ui, |ui| {
                        let mut text: &str = &cli;
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .font(TextStyle::Monospace)
                                .desired_rows(6)
                                .desired_width(f32::INFINITY),
                        )
                        .accessible_name("CLI des filtres modifiés");
                    });
            });
            let ui = &mut cols[1];
            match self.derived.as_ref().map(|d| &d.delays) {
                Some(Ok(d)) => self.delays_ui(ui, theme, d),
                Some(Err(e)) => {
                    ui.colored_label(theme.accent("red"), format!("Calcul impossible : {e}"));
                }
                None => {}
            }
        });
    }

    fn delays_ui(&self, ui: &mut Ui, theme: &Theme, d: &Delays) {
        let font = TextStyle::Body.resolve(ui.style());
        let fmt = |color: Color32| TextFormat::simple(font.clone(), color);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(RichText::new("Retard des filtres (ms)").strong());
            ui.label(safe_text(&format!(
                " à {} % de gaz, moteur {} Hz — actuel → ",
                self.throttle, self.motor_hz
            )));
            ui.label(RichText::new("modifié").color(theme.accent("blue")));
        });
        egui::Grid::new("filtres-retards").num_columns(4).show(ui, |ui| {
            ui.label("");
            for f in DELAY_FREQS {
                ui.label(RichText::new(format!("{} Hz", f as i64)).strong());
            }
            ui.end_row();
            for row in &d.rows {
                ui.label(row.name);
                for (a, b) in row.cells {
                    let color = match delay_trend(a, b) {
                        Trend::Shorter => theme.accent("green"),
                        Trend::Longer => theme.accent("red"),
                        Trend::Same => theme.ui.text,
                    };
                    let mut job = LayoutJob::default();
                    job.append(&format!("{} -> ", fixed(a, 2, false)), 0.0, fmt(theme.ui.text));
                    job.append(&fixed(b, 2, false), 0.0, fmt(color));
                    ui.label(job);
                }
                ui.end_row();
            }
        });
        for (name, hz) in &d.dyn_cutoffs {
            ui.label(format!(
                "LPF1 {name} dynamique (modifié) à ce gaz : {} Hz",
                fixed(*hz, 0, false)
            ));
        }
        ui.label(RichText::new(DELAY_NOTE).color(theme.ui.muted));
    }
}

impl Tool for FilterSimTool {
    type Options = Options;
    type Output = Arc<FilterSimResult>;
    const TITLE: &'static str = "Simulateur de filtres";
    const HAS_VIEW: bool = true;

    fn reset_view(&mut self) {
        self.spectrum_view = None;
        self.bode_view = None;
    }

    fn options(&self) -> Options {
        Options {
            applied: self.applied.clone(),
            // toujours calculée (Python aussi) : la case « actuel » ne fait que masquer la courbe, sans relancer le calcul
            with_current: true,
        }
    }

    fn selected(&self, state: &AppState) -> Vec<usize> {
        self.files.indices(state)
    }

    fn compute(o: Options, logs: Vec<LogInput>) -> Result<Self::Output, String> {
        let li = logs.first().ok_or("Sélectionnez au moins un log.")?;
        let base = &li.log.settings;
        let id = li.uid;
        let modf = match o.applied {
            Some(a) if a.log_id == id => a.settings,
            _ => Editor::from_settings(base).modified_settings(base),
        };
        // la clé contient les réglages modifiés, la case « actuel » ET la période (D-037)
        let key = format!("filtres:{:?}:{}:{:?}", modf, o.with_current, li.period);
        if let Some(r) = li.cache.get::<FilterSimResult>(&key) {
            return Ok(r);
        }
        let sel = li.log.slice_time(li.period.0, li.period.1);
        let mut r = simulate_filters(&li.log, sel, base, &modf, o.with_current)?;
        r.log_id = id;
        let r = Arc::new(r);
        li.cache.put(key, r.clone());
        Ok(r)
    }

    fn controls(&mut self, ui: &mut Ui, state: &AppState, theme: &Theme) {
        self.sync(state);
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.files.ui(ui, state);
            ui.separator();
            self.axes.ui(ui);
            ui.horizontal(|ui| {
                for (i, t) in ["Gyro", "D", "Notch dyn / RPM"].into_iter().enumerate() {
                    ui.selectable_value(&mut self.tab, i, t);
                }
            });
            if self.tab_fields(ui) {
                let family = self.family();
                self.editor.clamp(family);
                self.message.clear();
            }
            ui.separator();
            egui::Grid::new("filtres-bode-options").num_columns(2).show(ui, |ui| {
                ui.label("Gaz (Bode)");
                ui.add(egui::Slider::new(&mut self.throttle, THROTTLE_RANGE.0..=THROTTLE_RANGE.1).suffix(" %"))
                    .accessible_name("Gaz du Bode");
                ui.end_row();
                ui.label("Moteur (Bode)");
                drag(ui, &mut self.motor_hz, MOTOR_HZ_RANGE, " Hz", "Moteur du Bode");
                ui.end_row();
            });
            ui.checkbox(&mut self.bode_dyn, "notchs dyn. (centres initiaux)");
            ui.checkbox(&mut self.with_current, "simulation réglages actuels (référence)")
                .on_hover_text(CURRENT_HINT);
            ui.horizontal(|ui| {
                if ui.button("Réinitialiser").clicked()
                    && let Some(base) = &self.base
                {
                    self.editor = Editor::from_settings(base);
                    self.message.clear();
                }
                if ui.button(RichText::new("Appliquer").strong()).clicked()
                    && let (Some(base), Some(id)) = (&self.base, self.loaded)
                {
                    self.applied = Some(Applied {
                        log_id: id,
                        settings: self.editor.modified_settings(base),
                    });
                    self.applied_editor = self.editor.clone();
                    self.message.clear();
                }
            });
            ui.label(RichText::new(HELP).color(theme.ui.muted));
        });
    }

    fn render(&mut self, ui: &mut Ui, _state: &mut AppState, out: &Arc<FilterSimResult>, theme: &Theme) {
        self.refresh();
        egui::Panel::bottom("filtres-bas")
            .resizable(false)
            .show(ui, |ui| self.bottom(ui, theme));
        egui::CentralPanel::default().show(ui, |ui| {
            if result_is_for_other_log(out.log_id, self.loaded) {
                ui.label("Calcul en cours…");
                return;
            }
            if let Some(Err(e)) = self.derived.as_ref().map(|d| &d.bode) {
                ui.colored_label(theme.accent("red"), format!("Calcul impossible : {e}"));
            }
            self.plots(ui, out, theme);
        });
    }

    fn status(&self) -> Option<String> {
        if self.stale() {
            Some(STALE_STATUS.to_string())
        } else if self.message.is_empty() {
            None
        } else {
            Some(self.message.clone())
        }
    }
}
