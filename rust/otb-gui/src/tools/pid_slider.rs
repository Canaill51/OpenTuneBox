//! Outil « Curseurs PID » (port de `gui/tools/pid_slider.py`) : positions des curseurs du réglage simplifié Betaflight et simulation.
//!
//! En haut : position des 8 curseurs de chaque fichier sélectionné (lus dans l'en-tête, ou estimés à partir des PID quand le réglage
//! simplifié est désactivé) et PID actuels par axe. En bas : éditeur « et si… » — on bouge les curseurs (de 5 en 5 %, comme le
//! configurateur), on voit aussitôt les PID que calculerait le firmware (même arithmétique que `simplified_tuning.c`), l'écart avec les
//! PID actuels, les fréquences des filtres, et les commandes CLI à coller.

use std::sync::Arc;

use egui::{Color32, RichText, Ui};
use otb_analysis::sliders::{
    PTB_CATEGORIES, SliderView, cli_commands, filters_from_sliders, pids_from_sliders, slider_view, sliders_from_pids,
};
use otb_log::flightlog::FlightLog;
use otb_log::settings::{AxisPid, Settings};

use crate::state::AppState;
use crate::theme::{AXIS_NAMES, Theme};
use crate::tool::{LogInput, Tool};
use crate::tools::common::{FileSelection, safe_text};
use crate::tools::step_common::{category_name, header_mode, mode_text, pid_text, slider_plot};

/// (clé du curseur, libellé du configurateur Betaflight, explication courte).
pub const SLIDERS: [(&str, &str, &str); 8] = [
    (
        "d_gain",
        "Damping",
        "D Gains (et D max) : freine les dépassements, plus de bruit moteur.",
    ),
    (
        "pi_gain",
        "Tracking",
        "P & I Gains ensemble : colle mieux à la consigne, trop → oscillations.",
    ),
    (
        "ff_gain",
        "Stick Response",
        "FF Gains (feedforward) : réactivité aux mouvements de manche.",
    ),
    (
        "dmax_gain",
        "Dynamic Damping",
        "D Max : D renforcé seulement pendant les mouvements vifs.",
    ),
    (
        "i_gain",
        "Drift - Wobble",
        "I Gains : tient le cap (vent, gaz) ; trop → lente oscillation.",
    ),
    ("pitch_d", "Pitch Damping", "Pitch:Roll D : rapport D tangage / roulis."),
    (
        "pitch_pi",
        "Pitch Tracking",
        "Pitch:Roll P, I & FF : rapport P, I, F tangage / roulis.",
    ),
    (
        "master",
        "Master Multiplier",
        "Multiplie tous les gains (P, I, D, F) à la fois.",
    ),
];
pub const PID_COLS: [&str; 5] = ["P", "I", "D", "Dmax", "F"];
const GYRO_HINT: &str = "Multiplie les coupures des filtres gyro (plus haut = moins de retard).";
const DTERM_HINT: &str = "Multiplie les coupures des filtres du D.";

/// Un fichier tel que l'outil le voit.
pub struct FileSliders {
    pub label: String,
    pub color: Color32,
    pub log: Arc<FlightLog>,
    pub view: SliderView,
    /// `simplified_pids_mode` de l'en-tête (0 = absent).
    pub mode: i64,
}

impl FileSliders {
    pub fn settings(&self) -> &Settings {
        &self.log.settings
    }

    pub fn family(&self) -> &'static str {
        self.log.settings.family()
    }
}

pub struct PidOutput {
    pub files: Vec<FileSliders>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options;

/// Position des curseurs de la simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct Sim {
    /// Dans l'ordre de `SLIDERS`, en %.
    pub values: [i64; 8],
    pub gyro_mult: i64,
    pub dterm_mult: i64,
    pub gyro_on: bool,
    pub dterm_on: bool,
    /// 0 = OFF, 1 = RP, 2 = RPY.
    pub mode: usize,
}

/// `_snap` : 5 % le plus proche (arrondi au pair comme `round`), borné à 0–200.
pub fn snap(v: f64) -> i64 {
    (5.0 * (v / 5.0).round_ties_even()).clamp(0.0, 200.0) as i64
}

/// Curseur de `lo` à 200 % : l'utilisateur le déplace de 5 en 5, mais une valeur venue de l'en-tête (103, réglable au CLI) reste telle
/// quelle tant qu'on n'y touche pas, comme le `QSpinBox` de Python. Un `step_by(5.0)` d'egui recalait la valeur dès l'affichage.
fn five_percent_slider(ui: &mut Ui, value: &mut i64, lo: i64, name: &str, hint: &str) {
    let r = ui
        .add(egui::Slider::new(value, lo..=200).suffix(" %"))
        .on_hover_text(hint)
        .accessible_name(name);
    if r.changed() {
        *value = snap(*value as f64).max(lo);
    }
}

/// `_init_from_ref` : la simulation repart des curseurs du fichier (de l'en-tête, ou estimés depuis ses PID).
pub fn sim_from_ref(f: &FileSliders) -> Sim {
    let s = f.settings();
    let sl = s.sliders();
    let get = |k: &str| sl.iter().find(|(n, _)| *n == k).map(|(_, v)| *v);
    let mut values = [100i64; 8];
    let mode = if f.view.from_header {
        for (i, (k, _, _)) in SLIDERS.iter().enumerate() {
            values[i] = get(k).unwrap_or(100);
        }
        f.mode
    } else {
        let est = sliders_from_pids(&s.pids, f.family());
        for (i, (k, _, _)) in SLIDERS.iter().enumerate() {
            values[i] = est.iter().find(|(n, _)| n == k).map_or(100, |(_, v)| snap(*v));
        }
        2
    };
    let mult = |k: &str| match get(k) {
        Some(v) if v >= 0 => v,
        _ => 100,
    };
    Sim {
        values,
        gyro_mult: mult("gyro_mult"),
        dterm_mult: mult("dterm_mult"),
        gyro_on: get("gyro_filter").unwrap_or(0) == 1,
        dterm_on: get("dterm_filter").unwrap_or(0) == 1,
        mode: mode.clamp(0, 2) as usize,
    }
}

/// `whatif_sliders` : le dictionnaire de curseurs de la simulation, tel que le reçoivent les fonctions du firmware.
pub fn sim_sliders(sim: &Sim) -> Vec<(&'static str, f64)> {
    let mut out: Vec<(&'static str, f64)> = SLIDERS
        .iter()
        .zip(sim.values)
        .map(|((k, _, _), v)| (*k, v as f64))
        .collect();
    out.push(("mode", sim.mode as f64));
    out.push(("gyro_filter", f64::from(u8::from(sim.gyro_on))));
    out.push(("dterm_filter", f64::from(u8::from(sim.dterm_on))));
    out.push(("gyro_mult", sim.gyro_mult as f64));
    out.push(("dterm_mult", sim.dterm_mult as f64));
    out
}

/// Positions de la simulation (fractions) dans l'ordre des catégories, pour le losange du graphique.
pub fn sim_extra(sim: &Sim) -> [f64; 8] {
    let sl = sim_sliders(sim);
    let mut out = [0.0; 8];
    for (i, (_, key)) in PTB_CATEGORIES.iter().enumerate() {
        out[i] = sl.iter().find(|(k, _)| k == key).map_or(f64::NAN, |(_, v)| v / 100.0);
    }
    out
}

/// PID que calculerait le firmware pour cette simulation (base = PID du fichier).
pub fn sim_pids(sim: &Sim, f: &FileSliders) -> Result<Vec<AxisPid>, String> {
    pids_from_sliders(&sim_sliders(sim), f.family(), Some(&f.settings().pids))
}

/// Texte d'une case du tableau : la valeur et, s'il y a un écart, `  (+3)` ; et le signe de l'écart.
pub fn pid_cell(v: i64, v0: i64) -> (String, i64) {
    let d = v - v0;
    (
        if d != 0 {
            format!("{v}  ({d:+})")
        } else {
            format!("{v}")
        },
        d.signum(),
    )
}

/// Valeur d'une case du tableau des PID (`getattr(pid, name)`).
pub fn pid_field(p: &AxisPid, col: usize) -> i64 {
    [p.p, p.i, p.d, p.dmax, p.f][col]
}

/// `_filters_html` : une ligne par filtre, découpée en morceaux `(texte, en gras)`.
pub fn filter_rows(sim: &Sim, s: &Settings) -> Vec<Vec<(String, bool)>> {
    let new: std::collections::HashMap<&str, i64> = filters_from_sliders(sim.gyro_mult, sim.dterm_mult, Some(s))
        .into_iter()
        .collect();
    let cur = |lp1: &otb_log::settings::Lowpass, lp2: &otb_log::settings::Lowpass, p: &str| {
        [
            (format!("{p}_lpf1_dyn_min_hz"), lp1.dyn_min_hz),
            (format!("{p}_lpf1_dyn_max_hz"), lp1.dyn_max_hz),
            (format!("{p}_lpf1_static_hz"), lp1.static_hz),
            (format!("{p}_lpf2_static_hz"), lp2.static_hz),
        ]
    };
    let mut now: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    now.extend(cur(&s.gyro_lpf1, &s.gyro_lpf2, "gyro"));
    now.extend(cur(&s.dterm_lpf1, &s.dterm_lpf2, "dterm"));
    let mut rows = Vec::new();
    for (prefix, name, on) in [("gyro", "Gyro", sim.gyro_on), ("dterm", "D", sim.dterm_on)] {
        let mut row = vec![(name.to_string(), true)];
        if !on {
            row.push((" : curseur inactif -> filtres inchangés".to_string(), false));
            rows.push(row);
            continue;
        }
        row.push((" : ".to_string(), false));
        let mut parts: Vec<Vec<(String, bool)>> = Vec::new();
        for (suffix, lab) in [
            ("lpf1_dyn_min_hz", "LPF1 dyn min"),
            ("lpf1_dyn_max_hz", "max"),
            ("lpf1_static_hz", "LPF1"),
            ("lpf2_static_hz", "LPF2"),
        ] {
            let k = format!("{prefix}_{suffix}");
            let (a, b) = (now[&k], new.get(k.as_str()).copied().unwrap_or(0));
            if a == 0 && b == 0 {
                continue;
            }
            if suffix == "lpf1_dyn_max_hz" {
                let kmin = format!("{prefix}_lpf1_dyn_min_hz");
                if now[&kmin] == 0 && new.get(kmin.as_str()).copied().unwrap_or(0) == 0 {
                    continue; // LPF1 dynamique désactivé (min = 0) : le max n'a pas d'effet
                }
            }
            parts.push(if a != b {
                vec![(format!("{lab} {a}->"), false), (b.to_string(), true)]
            } else {
                vec![(format!("{lab} {a}"), false)]
            });
        }
        if parts.is_empty() {
            row.push(("aucun filtre actif".to_string(), false));
        } else {
            for (i, p) in parts.into_iter().enumerate() {
                if i > 0 {
                    row.push((", ".to_string(), false));
                }
                row.extend(p);
            }
        }
        row.push((" Hz".to_string(), false));
        rows.push(row);
    }
    rows
}

/// `_cli_lines` : les commandes à coller dans le CLI Betaflight, avec leur en-tête.
pub fn cli_lines(sim: &Sim, f: &FileSliders) -> Result<Vec<String>, String> {
    let sl = sim_sliders(sim);
    let mut lines = vec![format!(
        "# OpenTuneBox — curseurs pour {} (Betaflight {})",
        f.label,
        f.family()
    )];
    if sim.mode == 0 {
        lines.push("# mode OFF : les curseurs PID ne seront pas appliqués".to_string());
    }
    lines.extend(cli_commands(&sl, f.family())?);
    Ok(lines)
}

/// Texte d'information à droite du graphique (`_info_html`) : une ligne de titre, puis trois par fichier.
pub fn info_lines(files: &[FileSliders]) -> Vec<(String, Option<Color32>)> {
    let mut rows = vec![("PID actuels (P, I, D, Dmax, F)".to_string(), None)];
    for (i, f) in files.iter().enumerate() {
        let src = if f.view.from_header {
            "curseurs de l'en-tête"
        } else {
            "curseurs estimés"
        };
        let warn = if f.view.consistent {
            ""
        } else {
            " — (!) PID modifiés à la main"
        };
        rows.push((
            format!(
                "{}) {}  BF {}, mode {}, {src}{warn}",
                i + 1,
                f.label,
                f.family(),
                mode_text(f.mode)
            ),
            Some(f.color),
        ));
        let pid = f
            .settings()
            .pids
            .iter()
            .zip(AXIS_NAMES)
            .map(|(p, a)| format!("{} {}", &a[..1], pid_text(p)))
            .collect::<Vec<_>>()
            .join("  |  ");
        rows.push((format!("   {pid}"), Some(f.color)));
        let pos = f
            .view
            .values
            .iter()
            .map(|(k, v)| format!("{} {v:.2}", category_name(k)))
            .collect::<Vec<_>>()
            .join(", ");
        rows.push((format!("   {pos}"), None));
    }
    rows
}

pub struct PidSliderTool {
    files: FileSelection,
    sim: Option<Sim>,
    /// Fichier de référence de la simulation : son libellé et son log (repartir de zéro quand l'un change).
    reference: Option<(String, Arc<FlightLog>)>,
    copied: bool,
}

impl Default for PidSliderTool {
    fn default() -> Self {
        Self::new()
    }
}

impl PidSliderTool {
    pub fn new() -> Self {
        Self {
            files: FileSelection::new(true),
            sim: None,
            reference: None,
            copied: false,
        }
    }

    pub fn sim(&self) -> Option<&Sim> {
        self.sim.as_ref()
    }

    pub fn sim_mut(&mut self) -> Option<&mut Sim> {
        self.sim.as_mut()
    }

    /// Repart des curseurs du fichier de référence si celui-ci a changé (ou n'est pas encore fixé).
    pub fn sync_reference(&mut self, out: &PidOutput) {
        let Some(r) = out.files.first() else { return };
        let same = self
            .reference
            .as_ref()
            .is_some_and(|(l, log)| *l == r.label && Arc::ptr_eq(log, &r.log));
        if !same || self.sim.is_none() {
            self.reference = Some((r.label.clone(), r.log.clone()));
            self.sim = Some(sim_from_ref(r));
            self.copied = false;
        }
    }

    fn editor(&mut self, ui: &mut Ui, out: &PidOutput, theme: &Theme) {
        let Some(r) = out.files.first() else { return };
        let Some(mut sim) = self.sim.clone() else { return };
        let before = sim.clone();
        ui.group(|ui| {
            ui.label(RichText::new("Simulation « et si… »").strong());
            ui.columns(2, |cols| {
                // ---- à gauche : les curseurs
                let ui = &mut cols[0];
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Fichier : {}  (BF {})", r.label, r.family()))
                            .strong()
                            .color(r.color),
                    );
                    ui.label("Mode");
                    egui::ComboBox::from_id_salt("pid-mode")
                        .selected_text(["OFF", "RP", "RPY"][sim.mode])
                        .show_ui(ui, |ui| {
                            for (i, l) in ["OFF", "RP", "RPY"].iter().enumerate() {
                                ui.selectable_value(&mut sim.mode, i, *l);
                            }
                        })
                        .response
                        .accessible_name("Mode");
                    if ui.button("Repartir du fichier").clicked() {
                        sim = sim_from_ref(r);
                    }
                });
                ui.spacing_mut().slider_width = 170.0;
                // largeur laissée à l'explication : le reste de la colonne après le nom, le curseur et sa case (≈ 410 px mesurés sur capture) ; trop long : tronqué, le texte complet est dans l'info-bulle
                let hint_w = (ui.available_width() - 410.0).max(120.0);
                egui::Grid::new("pid-curseurs-grille")
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        for (i, (_, name, hint)) in SLIDERS.iter().enumerate() {
                            let hint = safe_text(hint); // « → » absent des polices (L-068)
                            ui.label(*name).on_hover_text(&hint);
                            five_percent_slider(ui, &mut sim.values[i], 0, name, &hint);
                            ui.scope(|ui| {
                                ui.set_max_width(hint_w);
                                ui.add(egui::Label::new(RichText::new(&hint).color(theme.ui.muted)).truncate())
                                    .on_hover_text(&hint);
                            });
                            ui.end_row();
                        }
                        ui.label("Filtre gyro").on_hover_text(GYRO_HINT);
                        five_percent_slider(ui, &mut sim.gyro_mult, 10, "Filtre gyro", GYRO_HINT);
                        ui.checkbox(&mut sim.gyro_on, "actif");
                        ui.end_row();
                        ui.label("Filtre D").on_hover_text(DTERM_HINT);
                        five_percent_slider(ui, &mut sim.dterm_mult, 10, "Filtre D", DTERM_HINT);
                        ui.checkbox(&mut sim.dterm_on, "actif (D)");
                        ui.end_row();
                    });
                // ---- à droite : PID résultants, filtres, CLI
                let ui = &mut cols[1];
                ui.label("PID résultants (écart avec le fichier) :");
                match sim_pids(&sim, r) {
                    Ok(new) => {
                        egui::Grid::new("pid-resultats")
                            .striped(true)
                            .min_col_width(70.0)
                            .show(ui, |ui| {
                                ui.label("");
                                for c in PID_COLS {
                                    ui.label(RichText::new(c).strong());
                                }
                                ui.end_row();
                                for (ax, name) in AXIS_NAMES.iter().enumerate() {
                                    ui.label(RichText::new(*name).strong());
                                    for col in 0..PID_COLS.len() {
                                        let (text, sign) =
                                            pid_cell(pid_field(&new[ax], col), pid_field(&r.settings().pids[ax], col));
                                        let color = match sign {
                                            1 => theme.accent("green"),
                                            -1 => theme.accent("red"),
                                            _ => theme.fg,
                                        };
                                        ui.label(RichText::new(text).color(color));
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                    Err(e) => {
                        ui.colored_label(theme.accent("red"), format!("Calcul impossible : {e}"));
                    }
                }
                for row in filter_rows(&sim, r.settings()) {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0; // les morceaux d'une même phrase se touchent
                        for (t, bold) in row {
                            ui.label(if bold {
                                RichText::new(t).strong()
                            } else {
                                RichText::new(t)
                            });
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Commandes CLI Betaflight :");
                    if ui.button("Copier").clicked()
                        && let Ok(l) = cli_lines(&sim, r)
                    {
                        ui.ctx().copy_text(l.join("\n"));
                        self.copied = true;
                    }
                    if self.copied {
                        ui.label(RichText::new("Commandes CLI copiées dans le presse-papiers.").color(theme.ui.muted));
                    }
                });
                let text = match cli_lines(&sim, r) {
                    Ok(l) => l.join("\n"),
                    Err(e) => format!("# erreur : {e}"),
                };
                let rows = text.lines().count().max(4);
                egui::ScrollArea::vertical()
                    .id_salt("pid-cli")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut text.as_str())
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .desired_rows(rows),
                        )
                        .accessible_name("Commandes CLI");
                    });
            });
        });
        if sim != before {
            self.copied = false;
        }
        self.sim = Some(sim);
    }
}

impl Tool for PidSliderTool {
    type Options = Options;
    type Output = PidOutput;
    const TITLE: &'static str = "Curseurs PID";

    fn options(&self) -> Options {
        Options
    }

    fn selected(&self, state: &AppState) -> Vec<usize> {
        self.files.indices(state)
    }

    fn compute(_o: Options, logs: Vec<LogInput>) -> Result<PidOutput, String> {
        let mut files = Vec::with_capacity(logs.len());
        for li in logs {
            let s = &li.log.settings;
            files.push(FileSliders {
                label: li.label,
                color: li.color,
                view: slider_view(s)?,
                mode: header_mode(&s.sliders()),
                log: li.log.clone(),
            });
        }
        Ok(PidOutput { files })
    }

    fn controls(&mut self, ui: &mut Ui, state: &AppState, theme: &Theme) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.files.ui(ui, state);
            ui.separator();
            ui.label(RichText::new("Les curseurs Betaflight").strong());
            for l in [
                "Damping = D",
                "Tracking = P et I",
                "Stick Response = F (feedforward)",
                "Dynamic Damping = D max",
                "Drift - Wobble = I",
                "Pitch Damping / Pitch Tracking = rapport tangage/roulis",
                "Master Multiplier = tout",
            ] {
                ui.label(format!("•  {l}"));
            }
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Point plein : curseurs lus dans l'en-tête du log. Point creux : réglage simplifié désactivé, position \
                     estimée à partir des PID. Losange : votre simulation.",
                )
                .color(theme.ui.muted),
            );
        });
    }

    fn render(&mut self, ui: &mut Ui, _state: &mut AppState, out: &PidOutput, theme: &Theme) {
        self.sync_reference(out);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let plot_h = (ui.available_height() - 420.0).clamp(300.0, 600.0);
            let files: Vec<(Color32, &SliderView)> = out.files.iter().map(|f| (f.color, &f.view)).collect();
            let extra = self.sim.as_ref().map(sim_extra);
            ui.columns(2, |cols| {
                slider_plot(
                    &mut cols[0],
                    "pid-curseurs",
                    &files,
                    extra.as_ref(),
                    "Curseurs PID Betaflight  (losange = simulation)",
                    theme,
                    plot_h,
                );
                for (text, color) in info_lines(&out.files) {
                    let t = RichText::new(text);
                    cols[1].label(match color {
                        Some(c) => t.color(c),
                        None => t.color(theme.fg),
                    });
                }
            });
            self.editor(ui, out, theme);
        });
    }
}
