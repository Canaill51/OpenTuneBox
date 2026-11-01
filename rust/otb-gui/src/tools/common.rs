//! Briques communes des outils (port de `gui/tools/base.py` pour la sélection des fichiers et des axes, et de
//! `gui/tools/_spectral_common.py` pour le reste) : formats, fréquences moteur, réduction des courbes, cases à cocher.

use std::ops::Range;

use egui::{RichText, Ui};
use otb_analysis::motors;
use otb_log::flightlog::{FlightLog, Rows};

use crate::state::{AppState, MAX_LOGS};
use crate::theme::{AXIS_NAMES, motor_color};

pub const MOTOR_LABELS: [&str; 4] = ["M1/5", "M2/6", "M3/7", "M4/8"];

/// Texte de l'assistant prêt à afficher : les polices d'egui n'ont pas « → » (un carré s'afficherait, et le banc de test
/// panique), remplacé par « -> ». Mesuré le 2026-11-01 : « → ✔ ▲ ↑ ↓ ⚠ » absents, tous les autres caractères du texte
/// de l'assistant présents (W-034).
pub fn safe_text(s: &str) -> String {
    s.replace('→', "->")
}

/// Un tableau `float32` lu en `float64` (les analyses travaillent en `float64`).
pub fn wide(x: &[f32]) -> Vec<f64> {
    x.iter().map(|&v| f64::from(v)).collect()
}

/// Colonnes `sel` de chaque canal (`a[:, sel]`).
pub fn sel_rows(r: &Rows, sel: &Range<usize>) -> Rows {
    let (a, b) = (sel.start.min(r.n), sel.end.min(r.n));
    let mut out = Rows::zeros(r.k, b.saturating_sub(a));
    for k in 0..r.k {
        out.row_mut(k).copy_from_slice(&r.row(k)[a..a.max(b)]);
    }
    out
}

/// Fréquence de chaque moteur (Hz). Le multiplicateur ne sert que sans eRPM (`sc.motor_freqs`).
pub fn motor_freqs(log: &FlightLog, multiplier: f64) -> Option<Rows> {
    motors::motor_hz(log, if log.motor_hz.is_none() { Some(multiplier) } else { None }).0
}

/// « 100,25,100 % | Q500 » (poids des harmoniques du filtre RPM et Q), ou « filtre RPM inactif ».
pub fn rpm_filter_text(log: &FlightLog) -> String {
    let s = &log.settings;
    if s.rpm_harmonics <= 0 {
        return "filtre RPM inactif".into();
    }
    let n = (s.rpm_harmonics as usize).min(s.rpm_weights.len());
    let w: Vec<String> = s.rpm_weights[..n].iter().map(|v| v.to_string()).collect();
    format!("{} % | Q{}", w.join(","), s.rpm_q)
}

/// « 1,21 ms » (zéros inutiles retirés), « — » si la valeur n'est pas finie.
pub fn fmt_ms(v: f64) -> String {
    if !v.is_finite() {
        return "—".into();
    }
    let s = otb_format::fixed(v, 2, false);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    format!("{} ms", s.replace('.', ","))
}

/// Valeur à 3 chiffres significatifs, virgule décimale (`{:.3g}`).
pub fn fmt3(v: f64) -> String {
    if !v.is_finite() {
        return "—".into();
    }
    otb_format::general(v, 3).replace('.', ",")
}

/// Réduit une courbe longue par moyennes sur des blocs de `k` points pour garder un affichage fluide : le temps du
/// premier point de chaque bloc, la moyenne du bloc (`sc.decimate`). Les points en trop à la fin sont écartés.
pub fn decimate(t: &[f64], y: &[f64], max_points: usize) -> (Vec<f64>, Vec<f64>) {
    let n = t.len().min(y.len());
    if n <= max_points {
        return (t[..n].to_vec(), y[..n].to_vec());
    }
    let k = n.div_ceil(max_points);
    let m = (n / k) * k;
    let ts = t[..m].iter().step_by(k).copied().collect();
    let ys = y[..m].chunks(k).map(|c| c.iter().sum::<f64>() / k as f64).collect();
    (ts, ys)
}

/// Arrondi supérieur « rond » (1, 2, 2,5, 5 × 10^n) pour une échelle de couleurs (`sc.nice_ceil`).
pub fn nice_ceil(v: f64) -> f64 {
    if !v.is_finite() || v <= 0.0 {
        return 1.0;
    }
    let e = 10f64.powf(v.log10().floor());
    for m in [1.0, 2.0, 2.5, 5.0, 10.0] {
        if v <= m * e * (1.0 + 1e-9) {
            return m * e;
        }
    }
    10.0 * e
}

/// Part des valeurs (triées) qui sont ≤ chaque fréquence : la courbe en « S » des harmoniques (`_cdf`).
pub fn cdf(sorted_vals: &[f64], freqs: &[f64]) -> Vec<f64> {
    if sorted_vals.is_empty() {
        return vec![0.0; freqs.len()];
    }
    let n = sorted_vals.len() as f64;
    freqs
        .iter()
        .map(|&f| sorted_vals.partition_point(|&v| v <= f) as f64 / n)
        .collect()
}

/// `np.percentile(a, q)` (méthode linéaire) d'un tableau **trié** non vide, `q` en pourcentage.
pub fn percentile_linear(sorted: &[f64], q: f64) -> f64 {
    let pos = q / 100.0 * (sorted.len() - 1) as f64;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

/// Valeurs finies et positives de `row`, triées (fréquences moteur ou centres de notch utiles).
pub fn sorted_positive(row: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = row.iter().copied().filter(|x| x.is_finite() && *x > 0.0).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v
}

/// Valeurs strictement positives de `row`, triées (centres des notchs dynamiques : `np.sort(c[c > 0])`).
pub fn sorted_above_zero(row: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = row.iter().copied().filter(|x| *x > 0.0).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v
}

/// La ligne avec NaN à la place des valeurs nulles ou négatives (`np.where(c > 0, c, np.nan)`).
pub fn nan_unless_positive(row: &[f64]) -> Vec<f64> {
    row.iter().map(|&v| if v > 0.0 { v } else { f64::NAN }).collect()
}

// ------------------------------------------------------------------ widgets

/// Liste des logs ouverts à cocher, avec la couleur de chaque fichier (`FileSelector`). La sélection est gardée par
/// libellé : elle survit à l'ouverture ou à la fermeture d'un autre log. Sans sélection valide, le log courant (ou le
/// premier) est pris.
pub struct FileSelection {
    multi: bool,
    labels: Vec<String>,
}

impl FileSelection {
    pub fn new(multi: bool) -> Self {
        Self {
            multi,
            labels: Vec::new(),
        }
    }

    fn explicit(&self, state: &AppState) -> Vec<usize> {
        state
            .logs
            .iter()
            .enumerate()
            .filter(|(_, l)| self.labels.contains(&l.label))
            .map(|(i, _)| i)
            .collect()
    }

    /// Indices des logs choisis, dans l'ordre d'ouverture.
    pub fn indices(&self, state: &AppState) -> Vec<usize> {
        let mut v = self.explicit(state);
        if !self.multi {
            v.truncate(1);
        }
        Self::fallback(v, state)
    }

    fn fallback(v: Vec<usize>, state: &AppState) -> Vec<usize> {
        if !v.is_empty() || state.logs.is_empty() {
            return v;
        }
        vec![state.current.filter(|c| *c < state.logs.len()).unwrap_or(0)]
    }

    pub fn ui(&mut self, ui: &mut Ui, state: &AppState) {
        // la sélection implicite devient réelle (un clic suivant s'y ajoute), les libellés disparus sont oubliés
        let now: Vec<String> = self
            .indices(state)
            .iter()
            .map(|&i| state.logs[i].label.clone())
            .collect();
        self.labels = now;
        ui.label(format!("Fichiers (max {MAX_LOGS})"));
        for l in &state.logs {
            let on = self.labels.contains(&l.label);
            let text = RichText::new(&l.label).color(l.color);
            let clicked = if self.multi {
                let mut v = on;
                ui.checkbox(&mut v, text).changed()
            } else {
                ui.radio(on, text).clicked()
            };
            if clicked {
                if !self.multi {
                    self.labels = vec![l.label.clone()];
                } else if on {
                    self.labels.retain(|x| *x != l.label);
                } else {
                    self.labels.push(l.label.clone());
                }
            }
        }
    }
}

/// Cases R, P, Y : axes affichés (réglage d'affichage, pas de recalcul).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AxisToggles(pub [bool; 3]);

impl Default for AxisToggles {
    fn default() -> Self {
        Self([true; 3])
    }
}

impl AxisToggles {
    pub fn enabled(&self) -> Vec<usize> {
        (0..3).filter(|&i| self.0[i]).collect()
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (i, name) in AXIS_NAMES.iter().enumerate() {
                ui.checkbox(&mut self.0[i], &name[..1]);
            }
        });
    }
}

/// Cases M1/5…M4/8 aux couleurs des moteurs (`MotorToggles`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MotorToggles(pub [bool; 4]);

impl Default for MotorToggles {
    fn default() -> Self {
        Self([true; 4])
    }
}

impl MotorToggles {
    pub fn enabled(&self) -> Vec<usize> {
        (0..4).filter(|&i| self.0[i]).collect()
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        egui::Grid::new("moteurs").show(ui, |ui| {
            for (i, label) in MOTOR_LABELS.iter().enumerate() {
                let text = RichText::new(*label).color(motor_color(i)).strong();
                ui.checkbox(&mut self.0[i], text);
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    }
}

/// Style de trait de l'harmonique de rang `h` : 1re pleine, 2e en tirets, 3e en pointillés (comme PIDtoolbox).
pub fn harmonic_style(h: i64) -> egui_plot::LineStyle {
    match h {
        1 => egui_plot::LineStyle::Solid,
        2 => egui_plot::LineStyle::dashed_dense(),
        _ => egui_plot::LineStyle::dotted_dense(),
    }
}
