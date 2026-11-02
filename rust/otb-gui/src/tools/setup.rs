//! Outil « Configuration » (port de `gui/tools/setup_info.py`) : les réglages des logs côte à côte, les lignes qui
//! diffèrent d'un log à l'autre surlignées. Ouvert depuis le visualiseur (pas de bouton dans la barre).
//!
//! Filtres d'affichage (différences seulement, paramètres bruts, recherche) : ils vivent dans l'outil, hors `Options`,
//! car ils ne changent pas le calcul.

use egui::{RichText, Ui};
use egui_extras::{Column, TableBuilder};
use otb_log::flightlog::FlightLog;
use otb_log::settings::{AxisPid, Lowpass, Settings, rates_type_name};

use crate::state::{AppState, MAX_LOGS};
use crate::theme::{Theme, plot::ROW_HIGHLIGHT};
use crate::tool::{LogInput, Tool};
use crate::tools::common::{FileSelection, safe_text};

/// Titre du groupe des paramètres bruts.
pub const RAW_TITLE: &str = "Tous les paramètres (en-tête brut)";
/// Valeur d'un réglage absent.
pub const MISSING: &str = "—";

/// Un groupe de réglages : titre et lignes (clé, texte).
pub type Section = (&'static str, Vec<(String, String)>);

fn lp(l: &Lowpass) -> String {
    if l.dynamic() {
        format!(
            "{} dynamique {}–{} Hz (expo {})",
            l.kind, l.dyn_min_hz, l.dyn_max_hz, l.expo
        )
    } else if l.static_hz != 0 {
        format!("{} {} Hz", l.kind, l.static_hz)
    } else {
        "désactivé".into()
    }
}

fn q(v: i64) -> String {
    otb_format::fixed(v as f64 / 100.0, 1, false)
}

fn pid_rows(s: &Settings) -> Vec<(String, String)> {
    ["Roll", "Pitch", "Yaw"]
        .iter()
        .zip(s.pids.iter())
        .map(|(n, p): (&&str, &AxisPid)| (format!("{n} P, I, D, Dmax, F"), p.text()))
        .collect()
}

/// `_sections` : les groupes de réglages d'un log, textes identiques à Python. `fs` : fréquence d'enregistrement,
/// `duration` : durée du log (s).
pub fn sections_from(s: &Settings, fs: f64, duration: f64) -> Vec<Section> {
    let g = |k: &str| s.get(k).unwrap_or(MISSING).to_string();
    let rates_type = rates_type_name(s.get_int("rates_type", -1))
        .map(str::to_string)
        .unwrap_or_else(|| s.get("rates_type").unwrap_or("?").to_string());
    let pairs = |keys: &[&str]| -> Vec<(String, String)> { keys.iter().map(|k| (k.to_string(), g(k))).collect() };
    let row = |k: &str, v: String| (k.to_string(), v);
    let sl = s.sliders();
    let sliders = if sl.is_empty() {
        vec![row("—", "absents".into())]
    } else {
        sl.iter().map(|(k, v)| row(k, v.to_string())).collect()
    };
    let mut pid = pid_rows(s);
    pid.push(row(
        "D max : gain / avance",
        format!("{} / {}", g("d_max_gain"), g("d_max_advance")),
    ));
    pid.push(row(
        "Level (angle)",
        s.get("levelPID")
            .filter(|v| !v.is_empty())
            .unwrap_or(MISSING)
            .to_string(),
    ));
    let notches = if s.gyro_notches.is_empty() {
        "aucun".to_string()
    } else {
        s.gyro_notches
            .iter()
            .map(|(h, c)| format!("{h} Hz (coupure {c})"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let dyn_notch = if s.dyn_notch_count != 0 {
        format!(
            "{} × Q{}, {}–{} Hz",
            s.dyn_notch_count,
            q(s.dyn_notch_q),
            s.dyn_notch_min_hz,
            s.dyn_notch_max_hz
        )
    } else {
        "désactivé".into()
    };
    let rpm = if s.rpm_harmonics != 0 {
        let w: Vec<String> = s.rpm_weights.iter().map(|v| v.to_string()).collect();
        format!(
            "{} harmoniques, Q{}, min {} Hz, poids {} %, fondu {} Hz",
            s.rpm_harmonics,
            q(s.rpm_q),
            s.rpm_min_hz,
            w.join(","),
            s.rpm_fade_hz
        )
    } else {
        "désactivé".into()
    };
    let d_notch = if s.dterm_notch.0 != 0 {
        format!("{} Hz (coupure {})", s.dterm_notch.0, s.dterm_notch.1)
    } else {
        "aucun".into()
    };
    let yaw_lp = if s.yaw_lowpass_hz != 0 {
        format!("{} Hz", s.yaw_lowpass_hz)
    } else {
        MISSING.into()
    };
    let idle = s
        .get("motor_idle")
        .map(str::to_string)
        .unwrap_or_else(|| g("dshot_idle_value"));
    vec![
        (
            "Général",
            vec![
                row("Firmware", s.firmware.clone()),
                row("Carte", s.board.clone()),
                row(
                    "Nom du drone",
                    if s.craft_name.is_empty() {
                        MISSING.into()
                    } else {
                        s.craft_name.clone()
                    },
                ),
                row(
                    "Fréquence gyro / PID",
                    format!(
                        "{} Hz / {} Hz",
                        otb_format::fixed(s.gyro_rate_hz(), 0, false),
                        otb_format::fixed(s.pid_rate_hz(), 0, false)
                    ),
                ),
                row(
                    "Fréquence d'enregistrement",
                    format!("{} Hz", otb_format::fixed(fs, 0, false)),
                ),
                row("Mode debug", s.debug_mode.clone()),
                row("Durée du log", format!("{} s", otb_format::fixed(duration, 1, false))),
            ],
        ),
        ("PID", pid),
        ("Curseurs simplifiés", sliders),
        (
            "Filtres gyro",
            vec![
                row("LPF1", lp(&s.gyro_lpf1)),
                row("LPF2", lp(&s.gyro_lpf2)),
                row("Notchs statiques", notches),
                row("Notch dynamique", dyn_notch),
                row("Filtre RPM", rpm),
            ],
        ),
        (
            "Filtres D",
            vec![
                row("LPF1", lp(&s.dterm_lpf1)),
                row("LPF2", lp(&s.dterm_lpf2)),
                row("Notch D", d_notch),
                row("Passe-bas P yaw", yaw_lp),
            ],
        ),
        (
            "Feedforward et lissage RC",
            pairs(&[
                "feedforward_transition",
                "feedforward_averaging",
                "feedforward_smooth_factor",
                "feedforward_jitter_factor",
                "feedforward_boost",
                "feedforward_max_rate_limit",
                "rc_smoothing_mode",
                "rc_smoothing_auto_factor",
                "rc_smoothing_setpoint_cutoff",
            ]),
        ),
        (
            "Terme I, TPA, anti-gravité",
            pairs(&[
                "iterm_relax",
                "iterm_relax_type",
                "iterm_relax_cutoff",
                "iterm_windup",
                "anti_gravity_gain",
                "anti_gravity_cutoff_hz",
                "tpa_mode",
                "tpa_rate",
                "tpa_breakpoint",
                "tpa_low_rate",
                "tpa_low_breakpoint",
                "throttle_boost",
            ]),
        ),
        (
            "Rates",
            vec![
                row("Type", rates_type),
                row("rc_rates", g("rc_rates")),
                row("rates (super / max)", g("rates")),
                row("rc_expo", g("rc_expo")),
                row("thr_mid / thr_expo", format!("{} / {}", g("thr_mid"), g("thr_expo"))),
            ],
        ),
        (
            "Moteurs",
            vec![
                row(
                    "Protocole / DShot bidirectionnel",
                    format!(
                        "{} / {}",
                        g("motor_pwm_protocol"),
                        if s.dshot_bidir { "oui" } else { "non" }
                    ),
                ),
                row("Pôles moteur", s.motor_poles.to_string()),
                row(
                    "KV moteur",
                    if s.motor_kv != 0 {
                        s.motor_kv.to_string()
                    } else {
                        MISSING.into()
                    },
                ),
                row("Ralenti", idle),
                row("Sortie moteurs (min, max)", g("motorOutput")),
                row(
                    "Limite moteur / gaz",
                    format!("{} % / {} %", g("motor_output_limit"), g("throttle_limit_percent")),
                ),
                row("Ralenti dynamique (min tr/min ×100)", g("dyn_idle_min_rpm")),
                row("Linéarisation de poussée", g("thrust_linear")),
            ],
        ),
    ]
}

/// `_sections` d'un log ouvert.
pub fn sections(s: &Settings, log: &FlightLog) -> Vec<Section> {
    sections_from(s, log.fs, log.duration())
}

/// Les réglages d'un log, prêts à être alignés avec ceux des autres.
#[derive(Clone, Debug)]
pub struct LogSetup {
    pub label: String,
    pub color: egui::Color32,
    pub sections: Vec<Section>,
    /// Paramètres bruts de l'en-tête, sans les lignes `Field …` et `H …`, triés par nom.
    pub raw: Vec<(String, String)>,
}

impl LogSetup {
    pub fn new(label: String, color: egui::Color32, sections: Vec<Section>, s: &Settings) -> Self {
        let raw = s
            .raw
            .iter()
            .filter(|(k, _)| !k.starts_with("Field ") && !k.starts_with("H "))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Self {
            label,
            color,
            sections,
            raw,
        }
    }
}

pub struct SetupOutput {
    pub logs: Vec<LogSetup>,
}

/// Une ligne alignée : le nom du réglage et sa valeur dans chaque log (`MISSING` si le log ne l'a pas).
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub key: String,
    pub values: Vec<String>,
}

impl Row {
    pub fn differs(&self) -> bool {
        self.values.iter().any(|v| *v != self.values[0])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub title: String,
    pub rows: Vec<Row>,
}

/// Aligne les réglages des logs, groupe par groupe.
///
/// Écart volontaire avec Python (décidé par Cédric) : les lignes sont alignées **par nom de réglage**, pas par
/// position. Python rangeait la n-ième ligne de chaque log côte à côte, ce qui mélange deux réglages différents
/// dès que les logs n'ont pas les mêmes lignes (curseurs simplifiés d'une autre version de Betaflight). Ici
/// l'ordre est celui du 1er log ; les noms propres aux autres logs viennent à la suite de leur groupe ; un log
/// qui n'a pas le réglage montre « — ». Le repère « — / absents » d'un log sans curseurs n'est pas un nom de
/// réglage : il est écarté dès qu'un autre log a de vrais curseurs.
pub fn align(logs: &[LogSetup], with_raw: bool) -> Vec<Group> {
    let mut titles: Vec<&'static str> = Vec::new();
    for l in logs {
        for (t, _) in &l.sections {
            if !titles.contains(t) {
                titles.push(t);
            }
        }
    }
    let mut groups: Vec<Group> = titles
        .into_iter()
        .map(|title| {
            let per_log: Vec<&[(String, String)]> = logs
                .iter()
                .map(|l| {
                    l.sections
                        .iter()
                        .find(|(t, _)| *t == title)
                        .map_or(&[][..], |(_, r)| r.as_slice())
                })
                .collect();
            Group {
                title: title.to_string(),
                rows: union_rows(&per_log),
            }
        })
        .collect();
    if with_raw {
        let per_log: Vec<&[(String, String)]> = logs.iter().map(|l| l.raw.as_slice()).collect();
        let mut rows = union_rows(&per_log);
        rows.sort_by(|a, b| a.key.cmp(&b.key));
        groups.push(Group {
            title: RAW_TITLE.to_string(),
            rows,
        });
    }
    groups
}

fn union_rows(per_log: &[&[(String, String)]]) -> Vec<Row> {
    let mut keys: Vec<&str> = Vec::new();
    for rows in per_log {
        for (k, _) in rows.iter() {
            if !keys.contains(&k.as_str()) {
                keys.push(k);
            }
        }
    }
    if keys.len() > 1 {
        keys.retain(|k| *k != MISSING);
    }
    keys.into_iter()
        .map(|k| Row {
            key: k.to_string(),
            values: per_log
                .iter()
                .map(|rows| {
                    rows.iter()
                        .find(|(rk, _)| rk == k)
                        .map_or(MISSING.to_string(), |(_, v)| v.clone())
                })
                .collect(),
        })
        .collect()
}

/// Lignes visibles : « Seulement les différences » et recherche (insensible à la casse, dans le nom ou les valeurs) ;
/// un groupe sans ligne visible disparaît.
pub fn visible(groups: &[Group], only_diff: bool, needle: &str) -> Vec<Group> {
    let needle = needle.trim().to_lowercase();
    groups
        .iter()
        .filter_map(|g| {
            let rows: Vec<Row> = g
                .rows
                .iter()
                .filter(|r| !only_diff || r.differs())
                .filter(|r| {
                    needle.is_empty()
                        || r.key.to_lowercase().contains(&needle)
                        || r.values.iter().any(|v| v.to_lowercase().contains(&needle))
                })
                .cloned()
                .collect();
            (!rows.is_empty()).then(|| Group {
                title: g.title.clone(),
                rows,
            })
        })
        .collect()
}

pub struct SetupTool {
    files: FileSelection,
    pub only_diff: bool,
    pub show_raw: bool,
    pub filter: String,
}

impl Default for SetupTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SetupTool {
    pub fn new() -> Self {
        Self {
            files: FileSelection::new(true),
            only_diff: false,
            show_raw: false,
            filter: String::new(),
        }
    }
}

impl Tool for SetupTool {
    type Options = ();
    type Output = SetupOutput;
    const TITLE: &'static str = "Configuration";
    const RUN_BUTTON: bool = false;
    const HIDDEN: bool = true;
    /// S'ouvre depuis le visualiseur : son bouton, « Courbes », reste mis en valeur (`OTHER_TOOLS["setup"]`).
    const TAB: Option<&'static str> = Some("Courbes");

    fn options(&self) {}

    fn selected(&self, state: &AppState) -> Vec<usize> {
        let mut v = self.files.indices(state);
        v.truncate(MAX_LOGS);
        v
    }

    fn compute(_: (), logs: Vec<LogInput>) -> Result<SetupOutput, String> {
        if logs.is_empty() {
            return Err("Sélectionnez au moins un log.".into());
        }
        Ok(SetupOutput {
            logs: logs
                .iter()
                .map(|l| {
                    LogSetup::new(
                        l.label.clone(),
                        l.color,
                        sections(&l.log.settings, &l.log),
                        &l.log.settings,
                    )
                })
                .collect(),
        })
    }

    fn controls(&mut self, ui: &mut Ui, state: &AppState, _theme: &Theme) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.files.ui(ui, state);
            ui.separator();
            ui.checkbox(&mut self.only_diff, "Seulement les différences");
            ui.checkbox(&mut self.show_raw, "Tous les paramètres bruts");
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("Rechercher un paramètre…"))
                .accessible_name("Rechercher un paramètre");
        });
    }

    fn render(&mut self, ui: &mut Ui, _state: &mut AppState, out: &SetupOutput, theme: &Theme) {
        let groups = visible(&align(&out.logs, self.show_raw), self.only_diff, &self.filter);
        let n = out.logs.len();
        let h = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
        let mut table = TableBuilder::new(ui)
            .id_salt("configuration")
            .striped(false)
            .column(Column::initial(240.0).at_least(120.0).resizable(true).clip(true));
        for _ in 0..n {
            table = table.column(Column::remainder().at_least(120.0).clip(true));
        }
        table
            .header(h, |mut header| {
                header.col(|ui| {
                    ui.label(RichText::new("Paramètre").strong());
                });
                for l in &out.logs {
                    header.col(|ui| {
                        ui.label(RichText::new(&l.label).strong().color(l.color));
                    });
                }
            })
            .body(|mut body| {
                for g in &groups {
                    body.row(h, |mut row| {
                        // `strong()` d'egui ne change que la couleur (pas de police grasse) : un bandeau distingue les titres
                        row.col(|ui| {
                            ui.painter().rect_filled(ui.max_rect(), 0.0, theme.ui.hover);
                            ui.label(RichText::new(safe_text(&g.title)).strong().color(theme.fg));
                        });
                        for _ in 0..n {
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, theme.ui.hover);
                            });
                        }
                    });
                    for r in &g.rows {
                        let differs = r.differs();
                        body.row(h, |mut row| {
                            let cell = |ui: &mut Ui, text: &str| {
                                if differs {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, ROW_HIGHLIGHT);
                                }
                                let text = safe_text(text);
                                ui.add(egui::Label::new(&text).truncate()).on_hover_text(&text);
                            };
                            row.col(|ui| cell(ui, &r.key));
                            for v in &r.values {
                                row.col(|ui| cell(ui, v));
                            }
                        });
                    }
                }
            });
    }
}
