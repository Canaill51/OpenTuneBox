//! Affichage des constats de l'assistant et du rapport « Diagnostic du bruit » (port de `advisor_panel.finding_html` et de
//! `spectral_window._diagnosis_html`). Python fabrique du HTML pour un `QTextBrowser` ; ici, ce sont des widgets egui
//! (pas de HTML dans egui), avec les mêmes textes.

use egui::{Color32, RichText, Ui};
use otb_advisor::noise::{AxisNoise, FilterBlock, NoiseReport, level_note};
use otb_advisor::rules::Finding;

use super::common::safe_text;

use crate::theme::{Theme, plot, severity_color};

/// « −12 dB » (signe moins typographique), « – » si la valeur n'est pas finie (`_db`).
pub fn db_text(x: f64) -> String {
    if x.is_finite() {
        format!("{} dB", otb_format::fixed(x, 0, false)).replace('-', "−")
    } else {
        "–".into()
    }
}

/// Les cinq cellules d'une ligne du tableau des niveaux : axe, avant, fréquence du sommet, après, baisse.
pub fn level_row(r: &AxisNoise) -> [String; 5] {
    [
        r.axis.clone(),
        db_text(r.before_db),
        if r.peak_hz.is_finite() {
            format!("{} Hz", otb_format::fixed(r.peak_hz, 0, false))
        } else {
            "–".into()
        },
        db_text(r.after_db),
        if r.reduction_db.is_finite() {
            format!("{} dB", otb_format::fixed(r.reduction_db, 1, false))
        } else {
            "–".into()
        },
    ]
}

/// Légende sous un tableau de niveaux.
pub fn levels_note(band: (f64, f64), kind: &str) -> String {
    format!(
        "Sommet du spectre lissé entre {} et {} Hz (échelle de PIDtoolbox) et baisse du niveau moyen dans cette bande. {}",
        otb_format::fixed(band.0, 0, false),
        otb_format::fixed(band.1, 0, false),
        level_note(kind)
    )
}

/// Phrase « Log : … » de l'en-tête d'un rapport.
pub fn flight_text(moving: bool) -> &'static str {
    if moving {
        "vol avec mouvements : le retard des filtres est jugé"
    } else {
        "vol surtout stationnaire : bon pour identifier le bruit, pas pour juger le retard des filtres"
    }
}

fn muted(ui: &mut Ui, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(safe_text(&text.into())).color(theme.ui.muted));
}

/// Un constat : pastille de gravité, titre en gras, détail, puis les actions en liste.
pub fn finding_ui(ui: &mut Ui, f: &Finding) {
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        let badge = RichText::new(format!(" {} ", f.severity.to_uppercase()))
            .background_color(severity_color(&f.severity))
            .color(plot::ON_ACCENT)
            .strong();
        ui.label(badge);
        ui.label(RichText::new(safe_text(&f.title)).strong());
    });
    ui.label(safe_text(&f.detail));
    for a in &f.actions {
        ui.label(safe_text(&format!("  •  {a}")));
    }
}

fn levels_table(ui: &mut Ui, theme: &Theme, id: &str, rows: &[AxisNoise], band: (f64, f64), kind: &str) {
    if rows.is_empty() {
        return;
    }
    egui::Grid::new(id).striped(true).show(ui, |ui| {
        for h in ["Axe", "Avant", "à", "Après", "Baisse"] {
            ui.label(RichText::new(h).strong());
        }
        ui.end_row();
        for r in rows {
            for cell in level_row(r) {
                ui.label(cell);
            }
            ui.end_row();
        }
    });
    muted(ui, theme, levels_note(band, kind));
}

fn block_ui(ui: &mut Ui, theme: &Theme, b: &FilterBlock) {
    ui.add_space(6.0);
    ui.label(RichText::new(safe_text(&b.title)).strong().size(15.0));
    muted(ui, theme, format!("Réglages : {}", b.settings));
    for o in &b.observations {
        ui.label(safe_text(&format!("  •  {o}")));
    }
    for f in &b.findings {
        finding_ui(ui, f);
    }
    if let Some(v) = &b.verdict {
        finding_ui(ui, v);
    }
}

/// Le rapport d'un fichier (`_diagnosis_html`). `label` et `color` : le libellé et la couleur du fichier ; `fs` : sa
/// fréquence d'enregistrement.
pub fn diagnosis_ui(ui: &mut Ui, theme: &Theme, label: &str, color: Color32, fs: f64, nr: &NoiseReport) {
    let rep = &nr.report;
    ui.push_id(label, |ui| {
        ui.heading(RichText::new(label).color(color));
        let need = rep.measures.log_rate_needed_hz.map_or(f64::NAN, |(n, _)| n);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Drone :").strong());
            ui.label(rep.profile.label);
            ui.label(RichText::new(safe_text(&format!("({})", rep.profile_reason))).italics());
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Période :").strong());
            ui.label(format!(
                "{} – {} s",
                otb_format::fixed(rep.period.0, 1, false),
                otb_format::fixed(rep.period.1, 1, false)
            ));
            ui.label(RichText::new("· Log :").strong());
            ui.label(format!(
                "{} Hz (au moins {} Hz) · {}.",
                otb_format::fixed(fs, 0, false),
                otb_format::fixed(need, 0, false),
                flight_text(nr.moving)
            ));
        });
        ui.add_space(6.0);
        ui.label(RichText::new("1. Conditions du log").strong().size(16.0));
        if nr.conditions.is_empty() {
            ui.label("Log exploitable pour juger le bruit et les filtres.");
        }
        for f in &nr.conditions {
            finding_ui(ui, f);
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new("2. Gyro : bruit avant et après filtrage")
                .strong()
                .size(16.0),
        );
        if !nr.has_unfiltered {
            ui.label("Pas de gyro non filtré dans ce log : seul le gyro filtré est mesuré.");
        }
        levels_table(ui, theme, "niveaux-gyro", &nr.gyro, nr.band, "gyro");
        for f in &nr.gyro_other {
            finding_ui(ui, f);
        }
        for b in &nr.gyro_blocks {
            block_ui(ui, theme, b);
        }
        let mut n = 3;
        if !nr.dterm.is_empty() {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("{n}. Terme D : bruit avant et après filtrage"))
                    .strong()
                    .size(16.0),
            );
            n += 1;
            levels_table(ui, theme, "niveaux-dterm", &nr.dterm, nr.band, "dterm");
            muted(ui, theme, "Terme D avant filtres reconstruit à partir du gyro filtré.");
            if let Some(b) = &nr.dterm_block {
                block_ui(ui, theme, b);
            }
        }
        ui.add_space(6.0);
        ui.label(RichText::new(format!("{n}. Stratégie de filtrage")).strong().size(16.0));
        for f in &nr.strategy {
            finding_ui(ui, f);
        }
    });
}

/// Pied de page du rapport.
pub const REPORT_FOOTER: &str = "Diagnostic automatique à partir des mesures du log, mêmes règles que l'assistant : un seul changement à la \
     fois, puis vol d'essai court et température des moteurs.";
