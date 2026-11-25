//! Briques communes des outils (port de `gui/tools/base.py` pour la sélection des fichiers et des axes, et de
//! `gui/tools/_spectral_common.py` pour le reste) : formats, fréquences moteur, réduction des courbes, cases à cocher.

use std::ops::Range;

use egui::{RichText, Ui};
use otb_analysis::motors;
use otb_log::flightlog::{FlightLog, Rows};

use crate::state::{AppState, NoiseSignal};
use crate::theme::{AXIS_NAMES, Theme, motor_color};
use crate::tools::filter_sim_calc::DYN_COUNT_MAX;

pub const MOTOR_LABELS: [&str; 4] = ["M1/5", "M2/6", "M3/7", "M4/8"];

/// Nom de l'espace qui réunit le bruit et les filtres (D-060) : une seule entrée de rail pour quatre vues.
pub const ESPACE_BRUIT: &str = "Bruit et filtres";

/// Les quatre vues de l'espace « Bruit et filtres » : **nom court affiché** (celui des pages du Guide et des
/// maquettes), titre de la page à ouvrir et icône Lucide de l'onglet (celles du prototype ; « Et si », absent du
/// prototype, prend l'entonnoir des filtres).
pub const VUES_BRUIT: [(&str, &str, &str); 4] = [
    ("Spectre", "Analyse spectrale", "audio-waveform"),
    ("Fréq. × gaz", "Fréquence × gaz", "grid-3x3"),
    ("Fréq. × temps", "Fréquence × temps", "clock"),
    ("Et si", "Simulateur de filtres", "funnel"),
];

/// Jeux de données que l'espace « Bruit et filtres » ne propose pas : le terme D avant ses filtres, reconstruit à partir
/// du gyro et non enregistré (D-078). Il reste calculé ailleurs (retard des filtres D, « Et si », visualiseur).
pub const NOT_IN_NOISE_SPACE: [&str; 1] = ["dterm_prefilt"];

/// Le jeu de données est-il proposé dans l'espace « Bruit et filtres » ?
pub fn in_noise_space(key: &str) -> bool {
    !NOT_IN_NOISE_SPACE.contains(&key)
}

/// Jeux de données des listes de l'espace « Bruit et filtres » (clé, libellé), dans l'ordre de `DATASETS`.
pub fn noise_datasets() -> impl Iterator<Item = (&'static str, &'static str)> {
    otb_analysis::datasets::DATASETS
        .iter()
        .copied()
        .filter(|(k, _)| in_noise_space(k))
}

/// La barre d'onglets d'un espace, en haut de chacune de ses vues : rend le titre de la page à ouvrir quand on
/// clique une autre vue. Dessin du prototype (`tabOn`) : icône et nom, gris pour les autres vues, bleu et souligné de
/// 2 px pour la vue affichée, qui ne fait rien ; un trait fin sous toute la barre. Au survol, un onglet passe à la
/// couleur du texte, se souligne en gris et la souris prend la main : sans cela, on ne voyait pas que c'étaient des
/// boutons (retour de Cédric, 2026-11-25).
pub fn space_tabs(ui: &mut Ui, courante: &'static str, th: &Theme) -> Option<&'static str> {
    let mut aller = None;
    let blue = th.accent("blue");
    let mut underlines = Vec::new();
    let row = ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 20.0;
        for (court, titre, nom) in VUES_BRUIT {
            let ici = titre == courante;
            let mut texte = RichText::new(court).size(13.5);
            if ici {
                texte = texte.strong();
            }
            let galley = egui::WidgetText::from(texte).into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Button,
            );
            // prototype : 2 px de marge à gauche et à droite, 8 en haut, 7 en bas ; icône de 16, 7 px avant le nom
            let size = egui::vec2(
                2.0 + 16.0 + 7.0 + galley.size().x + 2.0,
                8.0 + galley.size().y.max(16.0) + 7.0,
            );
            let (rect, r) = ui.allocate_exact_size(size, egui::Sense::click());
            r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, ici, court));
            let color = if ici {
                blue
            } else if r.hovered() {
                th.ui.text
            } else {
                th.ui.muted
            };
            let cy = rect.top() + 8.0 + galley.size().y.max(16.0) / 2.0;
            let icon_rect =
                egui::Rect::from_center_size(egui::pos2(rect.left() + 2.0 + 8.0, cy), egui::Vec2::splat(16.0));
            crate::icons::icon(nom, 16.0).tint(color).paint_at(ui, icon_rect);
            let text_pos = egui::pos2(icon_rect.right() + 7.0, cy - galley.size().y / 2.0);
            ui.painter().galley(text_pos, galley, color);
            if ici {
                underlines.push((rect.x_range(), blue));
            } else {
                if r.hovered() {
                    underlines.push((rect.x_range(), th.ui.muted));
                }
                if r.clicked() {
                    aller = Some(titre);
                }
                r.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
        }
    });
    // un trait fin sous toute la barre, et celui de l'onglet par-dessus (`border-bottom` du prototype)
    let y = row.response.rect.bottom();
    let painter = ui.painter();
    painter.hline(ui.max_rect().x_range(), y - 0.5, egui::Stroke::new(1.0, th.ui.border));
    for (x, c) in underlines {
        painter.hline(x, y - 1.0, egui::Stroke::new(2.0, c));
    }
    ui.add_space(6.0);
    aller
}

/// Commande segmentée de la maquette (« Tous | Roll | Pitch | Yaw », « Filtres | Série PID ») : fond gris, la
/// valeur choisie en blanc. Rend l'indice cliqué.
pub fn segmented(ui: &mut Ui, th: &Theme, labels: &[&str], selected: Option<usize>) -> Option<usize> {
    segmented_in(ui, th, labels, selected, false)
}

/// [`segmented`] ; `stretch` : les boutons se partagent toute la largeur disponible (`flex: 1` du prototype, en tête
/// d'un panneau).
pub fn segmented_in(ui: &mut Ui, th: &Theme, labels: &[&str], selected: Option<usize>, stretch: bool) -> Option<usize> {
    let mut clicked = None;
    // marge intérieure (3 de chaque côté) et écart entre boutons (2) retirés de la largeur à partager
    let n = labels.len().max(1) as f32;
    let width = (ui.available_width() - 6.0 - 2.0 * (n - 1.0)) / n;
    egui::Frame::new()
        .fill(th.ui.hover)
        .corner_radius(7)
        .inner_margin(3)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.spacing_mut().button_padding = egui::vec2(12.0, 5.0);
                for (i, l) in labels.iter().enumerate() {
                    let on = selected == Some(i);
                    let mut texte = RichText::new(*l).size(13.0);
                    texte = if on {
                        texte.strong().color(th.ui.text)
                    } else {
                        texte.color(th.ui.muted)
                    };
                    // ⚠️ `selected` met le bouton à l'état « coché » pour un lecteur d'écran (et pour les tests)
                    let b = egui::Button::new(texte)
                        .selected(on)
                        .fill(if on { th.ui.base } else { egui::Color32::TRANSPARENT })
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(5);
                    let b = if stretch { b.min_size(egui::vec2(width, 0.0)) } else { b };
                    if ui.add(b).clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
    clicked
}

/// Choix « Gyro | Terme D » en tête du panneau des vues de l'espace « Bruit et filtres » (D-069). `shown` : le
/// signal que montre la vue (`None` : un autre, choisi dans ses cases ; aucun bouton n'est alors allumé). Rend le
/// signal cliqué.
pub fn signal_tabs(ui: &mut Ui, th: &Theme, shown: Option<NoiseSignal>) -> Option<NoiseSignal> {
    let labels = NoiseSignal::ALL.map(NoiseSignal::label);
    let selected = shown.and_then(|s| NoiseSignal::ALL.iter().position(|t| *t == s));
    segmented_in(ui, th, &labels, selected, true).map(|i| NoiseSignal::ALL[i])
}

/// Mémoire de la vue d'un graphique : les bornes ne sont remises aux valeurs des réglages que quand la clé de ces
/// réglages change (le zoom de l'utilisateur reste sinon) ; « Tout voir » l'oublie pour les remettre au cadre suivant.
#[derive(Clone, Debug, Default)]
pub struct ViewMemo<K>(Option<K>);

impl<K: PartialEq> ViewMemo<K> {
    /// Les bornes doivent être (re)posées : la clé a changé, ou la vue a été oubliée.
    pub fn is_fresh(&self, key: &K) -> bool {
        self.0.as_ref() != Some(key)
    }

    /// Retient la clé des bornes qui viennent d'être posées.
    pub fn remember(&mut self, key: K) {
        self.0 = Some(key);
    }

    /// Oublie la vue : le cadre suivant repose les bornes.
    pub fn reset(&mut self) {
        self.0 = None;
    }

    /// Aucune vue retenue (au départ, ou après `reset`).
    pub fn is_unset(&self) -> bool {
        self.0.is_none()
    }
}

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

/// Les logs que montre une page, **lus dans la liste d'analyse** (la barre de pastilles du haut, D-066) : une page
/// n'a plus de cases à elle. Une page à plusieurs vols prend les fragments actifs, dans l'ordre de la barre ; une
/// page à un seul vol prend le **fragment maître**. Tout désactiver laisse la page vide : c'est un choix.
pub struct FileSelection {
    multi: bool,
}

impl FileSelection {
    pub fn new(multi: bool) -> Self {
        Self { multi }
    }

    /// Indices des logs à montrer (vide : aucun fragment actif).
    pub fn indices(&self, state: &AppState) -> Vec<usize> {
        if self.multi {
            state.active_indices()
        } else {
            state.master().into_iter().collect()
        }
    }
}

/// Harmoniques moteur proposées, chacune sa case : (rang, libellé court, info-bulle). Partagées par le Spectre
/// et Fréquence × temps, qui les tracent par-dessus leurs courbes.
pub const HARMONICS: [(i64, &str, &str); 3] = [
    (1, "1re", "1re harmonique moteur (trait plein)"),
    (2, "2e", "2e harmonique moteur (tirets)"),
    (3, "3e", "3e harmonique moteur (pointillés)"),
];
/// Nombre de notchs dynamiques qu'un réglage Betaflight peut demander (`dyn_notch_count`) : 7 depuis la 4.5
/// (5 en 4.4). Même valeur que `filter_sim_calc::DYN_COUNT_MAX`, que l'éditeur du Simulateur fait respecter.
pub const MAX_NOTCHES: usize = DYN_COUNT_MAX as usize;

/// Rangs des harmoniques cochées, dans l'ordre (vide : aucune à tracer).
pub fn ranks(cases: &[bool; HARMONICS.len()]) -> Vec<i64> {
    HARMONICS
        .iter()
        .zip(cases)
        .filter(|(_, on)| **on)
        .map(|((h, ..), _)| *h)
        .collect()
}

/// Nombre de notchs dynamiques proposés : le plus grand nombre présent dans les logs ouverts (0 : aucun à tracer).
/// Lu dans les réglages, pas dans un résultat de calcul : le panneau est dessiné avant que le calcul soit rendu.
pub fn notch_count(state: &AppState) -> usize {
    state
        .logs
        .iter()
        .map(|l| motors::dyn_notch_rows(&l.log).len())
        .max()
        .unwrap_or(0)
        .min(MAX_NOTCHES)
}

/// Résumé du menu des notchs dynamiques, pour `n` notchs présents : « tous », « aucun », les numéros quand il y en a
/// peu, sinon un compte. Le bouton du menu doit dire ce qui est affiché sans qu'on l'ouvre.
pub fn notches_summary(cases: &[bool]) -> String {
    let choisis: Vec<usize> = cases
        .iter()
        .enumerate()
        .filter(|(_, on)| **on)
        .map(|(i, _)| i + 1)
        .collect();
    if cases.is_empty() {
        return "Notchs : aucun".into();
    }
    match choisis.len() {
        0 => "Notchs : aucun".into(),
        n if n == cases.len() => "Notchs : tous".into(),
        1..=3 => format!(
            "Notchs : {}",
            choisis.iter().map(|n| format!("N{n}")).collect::<Vec<_>>().join(", ")
        ),
        n => format!("Notchs : {n} sur {}", cases.len()),
    }
}

/// Cases à cocher des harmoniques moteur puis menu des notchs dynamiques présents (panneau de réglages d'un outil).
pub fn harmonics_and_notches_ui(
    ui: &mut Ui,
    harmonics: &mut [bool; HARMONICS.len()],
    notches: &mut [bool; MAX_NOTCHES],
    state: &AppState,
    muted: egui::Color32,
) {
    ui.label("Harmoniques moteur");
    ui.horizontal_wrapped(|ui| {
        for (i, (_, court, bulle)) in HARMONICS.iter().enumerate() {
            ui.checkbox(&mut harmonics[i], *court).on_hover_text(*bulle);
        }
    });
    let n = notch_count(state);
    if n == 0 {
        ui.label("Notchs dynamiques");
        ui.label(RichText::new("aucun dans ces logs (mode débug FFT_FREQ)").color(muted));
        return;
    }
    // le bouton du menu porte déjà le mot « Notchs » : pas de titre au-dessus, le panneau est étroit
    // Menu déroulant plutôt qu'une rangée de cases : jusqu'à 7 notchs (Betaflight 4.5+), le panneau est étroit,
    // et le bouton dit ce qui est tracé sans qu'on l'ouvre (demandé par Cédric le 2026-11-06).
    egui::ComboBox::from_id_salt("notchs-dyn")
        // ⚠️ sans cela, le menu se referme au premier clic (comportement d'une liste de choix) : on veut cocher
        // plusieurs notchs de suite, le menu ne se ferme que par un clic à côté ou par Échap.
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text(notches_summary(&notches[..n]))
        .show_ui(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Tous").clicked() {
                    notches[..n].fill(true);
                }
                if ui.button("Aucun").clicked() {
                    notches[..n].fill(false);
                }
            });
            ui.separator();
            for (i, case) in notches.iter_mut().enumerate().take(n) {
                ui.checkbox(case, format!("N{}", i + 1))
                    .on_hover_text(format!("Centre du notch dynamique n° {}", i + 1));
            }
        })
        .response
        .accessible_name("Notchs dynamiques");
}
/// Échelle verticale des spectres en dB, celle de PIDtoolbox (`docs/spec-ecrans.md`, « Full Spectrum … −50..20 ») :
/// le Spectre l'a par défaut, le Simulateur de filtres s'y tient aussi pour que les deux pages se comparent (2026-11-06).
pub const PTB_DB_RANGE: (f64, f64) = (-50.0, 20.0);

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
        _ => crate::plots::dotted(),
    }
}
