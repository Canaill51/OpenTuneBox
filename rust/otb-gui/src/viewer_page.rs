//! L'écran d'extraction d'un fragment (lot 29b, maquette « Modifier le fragment » du prototype, D-060) : c'est le
//! visualiseur de courbes, **hors rail**, qui règle les bornes du fragment de la session affichée.
//!
//! Trois graphiques Roll / Pitch / Yaw et un graphique des moteurs (axe de rotation en Hz à droite) ; les bornes du
//! fragment (traits bleus, poignées) et le curseur se déplacent depuis n'importe quel graphique ; une barre de
//! navigation sous les graphiques montre le log entier et déplace la fenêtre visible. Le panneau de droite porte le
//! fragment (nom, rôle, début et fin à l'échantillon près), les traces, l'affichage, le lecteur et les valeurs au
//! curseur.
//!
//! ⚠️ Rien de ce qu'on règle ici n'atteint les outils ni la bibliothèque avant « Enregistrer le fragment » : les
//! bornes vivent dans un **brouillon** ([`Draft`]). « Annuler » et la flèche de retour le jettent.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use egui::{Align, Color32, Id, Layout, RichText, Sense, Ui, Vec2};
use egui_plot::{AxisHints, HPlacement, Line, LineStyle, Plot, PlotPoint, PlotPoints, Span, Text, VLine};
use otb_advisor::log_check::{FILTERS, PID};
use otb_dsp::minmax::{MinMaxPyramid, window_points};
use otb_log::flightlog::FlightLog;
use otb_log::modes::mode_changes;

use crate::icons::icon;
use crate::plots::{VDrag, right_axis_scale};
use crate::state::{AppState, default_period};
use crate::theme::{self, AXIS_NAMES, Theme, with_alpha};
use crate::viewer::{
    DEFAULT_TRACES, SMOOTHING, has_trace, measure_text, readout_lines, smooth, stick_positions, trace_data, trace_label,
};

/// Groupe des axes X liés (zoom et déplacement communs aux quatre graphiques).
const LINK: &str = "otb-viewer-x";

// ---------------------------------------------------------------- mesures relevées dans la maquette (D-061)

/// Hauteur de la barre du haut de l'écran.
const HEADER_H: f32 = 56.0;
/// Largeur du panneau de droite.
const PANEL_W: f32 = 300.0;
/// Hauteur relative des graphiques : 138 points pour un axe, 100 pour les moteurs.
const AXIS_H: f32 = 138.0;
const MOTOR_H: f32 = 100.0;
/// Barre de navigation : hauteur, coins, poignées.
const NAV_H: f32 = 46.0;
const NAV_HANDLE: Vec2 = Vec2::new(14.0, 28.0);
/// Onglet « Début 2,2 s » posé en haut d'une borne du fragment.
const TAB_SIZE: Vec2 = Vec2::new(88.0, 22.0);
/// Poignée d'une borne, au milieu de chaque graphique d'axe.
const GRIP_SIZE: Vec2 = Vec2::new(10.0, 28.0);
/// Trait d'une borne du fragment.
const BOUND_W: f32 = 2.5;
/// Un fragment dure au moins une seconde (la maquette ne laisse pas les deux bornes se croiser).
const MIN_SPAN: f64 = 1.0;
/// Pas des boutons ‹ › du début et de la fin.
const NUDGE: f64 = 0.1;

/// Ordre des traces dans le panneau, celui de la maquette (Gyro, Gyro préfiltre, Consigne, termes P, I, D), puis
/// les traces que la maquette ne montre pas mais que le visualiseur offrait déjà.
const TRACE_ORDER: [&str; 11] = [
    "gyro",
    "gyro_prefilt",
    "setpoint",
    "pterm",
    "iterm",
    "dterm",
    "dterm_prefilt",
    "fterm",
    "pid_sum",
    "pid_error",
    "debug",
];

/// Une courbe en cache : valeurs (lissage déjà appliqué) et index multi-résolution qui rend le tracé indépendant
/// du nombre d'échantillons (D-048).
///
/// ⚠️ Les deux vont **ensemble** : les indices de l'index désignent des positions dans `y`. Les ranger dans le même
/// objet de cache interdit de tracer une courbe avec l'index d'une autre (D-048).
struct Series {
    y: Vec<f64>,
    pyramid: MinMaxPyramid,
}

impl Series {
    fn new(y: Vec<f64>) -> Self {
        let pyramid = MinMaxPyramid::build(&y);
        Self { y, pyramid }
    }
}

/// Une courbe prête à tracer : lissage déjà appliqué.
struct Curve {
    name: String,
    color: Color32,
    width: f32,
    data: Arc<Series>,
    dotted: bool,
}

/// Le fragment en cours de modification. Il n'atteint ni les outils ni la bibliothèque avant l'enregistrement.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// Log modifié (`LoadedLog::uid`) : un brouillon ne survit pas à un changement de log (L-072).
    pub uid: u64,
    /// Bornes, en secondes, toujours posées sur un échantillon.
    pub period: (f64, f64),
    pub name: String,
    /// Rôle au sens du Guide (`FILTERS`, `PID`) ; vide : aucun.
    pub role: String,
}

/// Ce que la fenêtre sait du fragment affiché, lu dans la bibliothèque : l'écran, lui, n'y a pas accès.
#[derive(Clone, Debug, Default)]
pub struct FragmentInfo {
    pub drone: String,
    pub file: String,
    /// Numéro de la session dans son fichier (à partir de 1) et nombre de sessions (0 : inconnu).
    pub session: usize,
    pub sessions: usize,
    pub name: String,
    pub role: String,
    /// Le fragment est rangé dans la bibliothèque ; sinon (log sans nom de machine), les bornes valent pour la
    /// séance seulement.
    pub stored: bool,
}

/// Ce que l'écran demande à la fenêtre.
#[derive(Clone, Debug, PartialEq)]
pub enum ExtAction {
    /// Flèche de retour ou « Annuler » : le brouillon est jeté, rien n'est enregistré.
    Leave,
    /// « Enregistrer le fragment » : les bornes deviennent la période d'analyse du log `index`, et la bibliothèque
    /// les garde.
    Save { index: usize, draft: Draft },
    /// Bouton du thème de la barre du haut.
    ToggleTheme,
}

/// Glissement en cours dans la barre de navigation.
#[derive(Clone, Copy, Debug, PartialEq)]
enum NavMode {
    Pan,
    Left,
    Right,
}

/// Sections repliables du panneau de droite, dans leur ordre.
const SECTIONS: [&str; 5] = ["Fragment", "Traces", "Affichage", "Lecteur", "Au curseur"];

pub struct ViewerPage {
    traces: HashSet<&'static str>,
    pub motors: bool,
    pub rps: bool,
    pub throttle: bool,
    pub yscale: u32,
    /// Index dans `SMOOTHING`.
    pub smoothing: usize,
    pub width: f32,
    /// 0 : les trois axes ; 1 à 3 : un seul.
    pub focus: usize,
    /// Mode « Mesurer » : un clic sur Roll, Pitch ou Yaw place un point, deux points donnent Δt et la fréquence.
    pub measure_on: bool,
    pub measure: Vec<f64>,
    /// Lecture en cours : le curseur avance à la vitesse du temps réel.
    pub playing: bool,
    /// Le fragment en cours de modification (`None` : pas encore lu pour le log affiché).
    pub draft: Option<Draft>,
    /// Sections ouvertes du panneau de droite (ordre de `SECTIONS`).
    pub open: [bool; 5],
    drags: [VDrag; 4],
    /// Quel log est affiché (`LoadedLog::uid`) : le changer vide les caches et recadre les graphiques.
    shown: Option<u64>,
    /// Recadrer en X sur tout le log au prochain cadre.
    fit: bool,
    /// Fenêtre visible demandée par la barre de navigation ou « Cadrer le fragment », posée au prochain cadre.
    pub window: Option<(f64, f64)>,
    nav_drag: Option<(NavMode, (f64, f64))>,
    /// Silhouette du gaz pour la barre de navigation : (log, largeur en points, valeur moyenne par colonne).
    nav_shape: Option<(u64, usize, Vec<f32>)>,
    series: HashMap<(&'static str, usize, usize), Arc<Series>>,
    present: HashMap<&'static str, bool>,
    modes: Vec<(f64, String)>,
    link: Id,
    /// Transformation (temps ↔ pixels) de chaque graphique au dernier cadre : Roll, Pitch, Yaw, moteurs.
    pub transforms: [Option<egui_plot::PlotTransform>; 4],
    /// Rectangle de la barre de navigation au dernier cadre (pour les tests).
    pub nav_rect: Option<egui::Rect>,
}

impl Default for ViewerPage {
    fn default() -> Self {
        Self {
            traces: DEFAULT_TRACES.iter().copied().collect(),
            motors: true,
            rps: false,
            throttle: true,
            yscale: 500,
            smoothing: 0,
            // la maquette trace ses courbes à 1,2 point
            width: 1.2,
            focus: 0,
            measure_on: false,
            measure: Vec::new(),
            playing: false,
            draft: None,
            // la maquette ouvre tout sauf « Au curseur » ; « Affichage » (absent de la maquette) est replié
            open: [true, true, false, true, false],
            drags: [VDrag::default(); 4],
            shown: None,
            fit: true,
            window: None,
            nav_drag: None,
            nav_shape: None,
            series: HashMap::new(),
            present: HashMap::new(),
            modes: Vec::new(),
            link: Id::unique(LINK),
            transforms: [None; 4],
            nav_rect: None,
        }
    }
}

/// Un nombre de secondes à la française : `decimals` chiffres après la virgule.
pub fn secs(v: f64, decimals: usize) -> String {
    format!("{v:.decimals$}").replace('.', ",")
}

/// La durée minimale d'un fragment dans un log de `span` secondes : une seconde, ou la moitié d'un log plus court.
fn min_span(span: f64) -> f64 {
    MIN_SPAN.min(span / 2.0)
}

/// Pas des graduations de la barre de navigation : le plus petit qui garde au plus huit étiquettes.
pub fn tick_step(span: f64) -> f64 {
    [1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 60.0, 120.0, 300.0, 600.0]
        .into_iter()
        .find(|s| span / s <= 8.0)
        .unwrap_or(1200.0)
}

/// Bornes du fragment posées sur un échantillon du log, remises dans l'ordre, et écartées d'au moins
/// [`min_span`] : ce que l'écran garde dans son brouillon, quelle que soit la manière dont on les a déplacées.
///
/// `moved` dit quelle borne vient de bouger (0 : le début, 1 : la fin) : c'est elle qui cède quand les deux sont trop
/// proches, l'autre ne bouge pas.
pub fn snap_bounds(log: &FlightLog, (a, b): (f64, f64), moved: usize) -> (f64, f64) {
    let (lo, hi) = (log.t[0], *log.t.last().unwrap());
    let gap = min_span(hi - lo);
    let at = |t: f64| log.t[log.index_of(t.clamp(lo, hi))];
    let (a, b) = if moved == 0 {
        let b = at(b);
        (at(a.min(b - gap)), b)
    } else {
        let a = at(a);
        (a, at(b.max(a + gap)))
    };
    // l'échantillon le plus proche peut tomber juste sous l'écart minimal : on garde au moins un échantillon d'écart
    if b > a {
        (a, b)
    } else {
        (a, log.t[(log.index_of(a) + 1).min(log.n() - 1)])
    }
}

impl ViewerPage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trace_on(&self, key: &str) -> bool {
        self.traces.contains(key)
    }

    pub fn set_trace(&mut self, key: &'static str, on: bool) {
        if on {
            self.traces.insert(key);
        } else {
            self.traces.remove(key);
        }
    }

    /// Mémoire du cache de courbes : valeurs d'un côté, index multi-résolution de l'autre (octets). Sert au banc
    /// `otb-gui --example banc_cadre` à confronter le coût de l'index au pic de mémoire du processus (D-048).
    pub fn cache_bytes(&self) -> (usize, usize) {
        self.series.values().fold((0, 0), |(v, p), s| {
            (v + s.y.len() * std::mem::size_of::<f64>(), p + s.pyramid.bytes())
        })
    }

    /// Recadre les graphiques sur tout le log (« Vue entière »).
    pub fn fit_all(&mut self) {
        self.fit = true;
    }

    /// La fenêtre visible au dernier cadre (secondes), lue sur le graphique des moteurs, toujours dessiné.
    pub fn visible(&self) -> Option<(f64, f64)> {
        self.transforms[3].map(|t| (t.bounds().min()[0], t.bounds().max()[0]))
    }

    fn series(&mut self, key: &'static str, ax: usize, w: usize, make: impl FnOnce() -> Vec<f64>) -> Arc<Series> {
        self.series
            .entry((key, ax, w))
            .or_insert_with(|| Arc::new(Series::new(make())))
            .clone()
    }

    fn is_present(&mut self, log: &FlightLog, key: &'static str) -> bool {
        *self.present.entry(key).or_insert_with(|| has_trace(log, key))
    }

    /// Courbes de chaque axe, du graphique des moteurs et de la rotation (Hz, à l'échelle de l'axe de gauche).
    fn curves(&mut self, log: &FlightLog, th: &Theme) -> ([Vec<Curve>; 3], Vec<Curve>, Vec<Curve>, f64) {
        let w = SMOOTHING[self.smoothing].1;
        let mut axes: [Vec<Curve>; 3] = Default::default();
        for key in TRACE_ORDER {
            if !self.traces.contains(key) {
                continue;
            }
            let Some(data) = trace_data(log, key) else { continue };
            for (ax, curves) in axes.iter_mut().enumerate().take(data.k.min(3)) {
                let row = data.row(ax);
                if matches!(key, "dterm" | "dterm_prefilt") && !row.iter().any(|&v| v != 0.0) {
                    continue;
                }
                let data = self.series(key, ax, w, || smooth(row, w));
                curves.push(Curve {
                    name: trace_label(key).into(),
                    color: th.trace_color(key),
                    width: self.width,
                    data,
                    dotted: false,
                });
            }
        }
        let mut motors = Vec::new();
        if self.motors {
            for m in 0..log.n_motors() {
                let data = self.series("motor", m, w, || smooth(log.motors.row(m), w));
                motors.push(Curve {
                    name: format!("Moteur {}", m + 1),
                    color: with_alpha(theme::motor_color(m), 217), // moteurs à 85 %, comme la maquette
                    width: self.width,
                    data,
                    dotted: false,
                });
            }
        }
        if self.throttle {
            let data = self.series("throttle", 0, 1, || log.throttle.iter().map(|&v| v as f64).collect());
            motors.push(Curve {
                name: "Gaz".into(),
                color: th.trace_color("throttle"),
                width: self.width + 0.4,
                data,
                dotted: false,
            });
        }
        let mut rps = Vec::new();
        let mut k = 1.0;
        if let (true, Some(hz)) = (self.rps, log.motor_hz.as_ref()) {
            let top = hz.data.iter().copied().filter(|v| !v.is_nan()).fold(0.0f32, f32::max) as f64 * 1.05;
            k = right_axis_scale(100.0, top);
            for m in 0..hz.k {
                let data = self.series("rps", m, w, || {
                    smooth(hz.row(m), w).into_iter().map(|v| v * k).collect()
                });
                rps.push(Curve {
                    name: format!("Rotation {} (Hz)", m + 1),
                    color: theme::motor_color(m),
                    width: self.width,
                    data,
                    dotted: true,
                });
            }
        }
        (axes, motors, rps, k)
    }

    /// L'écran entier : barre du haut, panneau de droite, barre d'outils, graphiques et barre de navigation.
    /// `ui` est la fenêtre sous le rail absent (écran hors rail, D-060).
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, info: &FragmentInfo, th: &Theme) -> Vec<ExtAction> {
        // changement de log : caches vidés, brouillon jeté, graphiques recadrés
        let current = state.current_log().map(|l| l.uid);
        if current != self.shown {
            self.shown = current;
            self.series.clear();
            self.present.clear();
            self.measure.clear();
            self.playing = false;
            self.fit = true;
            self.window = None;
            self.nav_drag = None;
            self.modes = state
                .current_log()
                .map(|l| mode_changes(l.log.mode_flags.as_deref(), &l.log.t, &l.log.settings.version))
                .unwrap_or_default();
            if let Some(l) = state.current_log() {
                let (lo, hi) = (l.log.t[0], *l.log.t.last().unwrap());
                if !(lo..=hi).contains(&state.cursor) {
                    state.set_cursor(lo);
                }
            }
        }
        match state.current_log() {
            Some(ll) if self.draft.as_ref().is_none_or(|d| d.uid != ll.uid) => {
                self.draft = Some(Draft {
                    uid: ll.uid,
                    period: snap_bounds(&ll.log, ll.period, 0),
                    name: info.name.clone(),
                    role: info.role.clone(),
                });
            }
            None => self.draft = None,
            _ => {}
        }
        self.advance(ui, state);
        let mut actions = Vec::new();
        let barre = egui::Panel::top("ext-barre")
            .exact_size(HEADER_H)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(th.ui.base)
                    .inner_margin(egui::Margin::symmetric(16, 0)),
            )
            .show(ui, |ui| self.header(ui, state, info, th, &mut actions))
            .response
            .rect;
        let panneau = egui::Panel::right("ext-panneau")
            .exact_size(PANEL_W)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(th.ui.base))
            .show(ui, |ui| self.side_panel(ui, state, info, th))
            .response
            .rect;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(th.ui.window)
                    .inner_margin(egui::Margin::symmetric(20, 16)),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 12.0;
                self.toolbar(ui, state, th);
                let Some(ll) = state.current_log() else {
                    ui.label(
                        "Aucun log ouvert : bouton « Ouvrir des logs… » du tableau de bord, ou glisser-déposer d'un \
                         fichier .bbl / .bfl sur la fenêtre.",
                    );
                    return;
                };
                let ll_log = ll.log.clone();
                egui::Frame::new()
                    .fill(th.ui.base)
                    .stroke(egui::Stroke::new(1.0, th.ui.border))
                    .corner_radius(8)
                    .inner_margin(egui::Margin {
                        left: 6,
                        right: 12,
                        top: 14,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        let nav_space = NAV_H + 14.0 + 20.0 + 3.0 * 4.0;
                        self.plots(ui, state, th, nav_space);
                        self.navigator(ui, &ll_log, th);
                    });
            });
        // traits de séparation de la maquette : sous la barre du haut, à gauche du panneau
        let p = ui.painter();
        let stroke = egui::Stroke::new(1.0, th.ui.border);
        p.hline(barre.x_range(), barre.bottom(), stroke);
        p.vline(panneau.left(), panneau.y_range(), stroke);
        actions
    }

    /// La barre du haut : retour, nom du fragment, thème, « Annuler », « Enregistrer le fragment ».
    fn header(&mut self, ui: &mut Ui, state: &AppState, info: &FragmentInfo, th: &Theme, actions: &mut Vec<ExtAction>) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            let retour = egui::Button::image(icon("arrow-left", 18.0).tint(th.ui.text).alt_text("Retour"))
                .min_size(Vec2::splat(34.0))
                .fill(th.ui.base)
                .stroke(egui::Stroke::new(1.0, th.ui.mid))
                .corner_radius(8);
            if ui.add(retour).on_hover_text("Revenir sans enregistrer").clicked() {
                actions.push(ExtAction::Leave);
            }
            let ll = state.current_log();
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                let mut sous_titre: Vec<String> = Vec::new();
                if !info.drone.is_empty() {
                    sous_titre.push(info.drone.clone());
                }
                if !info.file.is_empty() {
                    sous_titre.push(info.file.clone());
                }
                if info.session > 0 {
                    sous_titre.push(if info.sessions > 0 {
                        format!("session {} / {}", info.session, info.sessions)
                    } else {
                        format!("session {}", info.session)
                    });
                }
                ui.label(RichText::new(sous_titre.join(" · ")).size(12.0).color(th.ui.muted));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if let Some(ll) = ll {
                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 5.0, ll.color);
                    }
                    let nom = self.draft.as_ref().map_or("", |d| d.name.as_str());
                    let titre = if ll.is_some() {
                        format!("Modifier le fragment {nom}")
                    } else {
                        "Extraire un fragment".to_string()
                    };
                    ui.label(RichText::new(titre).size(16.0).strong().color(th.ui.text));
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.spacing_mut().button_padding = Vec2::new(14.0, 7.0);
                let on_accent = theme::plot::ON_ACCENT;
                let enregistrer = egui::Button::image_and_text(
                    icon("check", 15.0).tint(on_accent),
                    RichText::new("Enregistrer le fragment")
                        .size(13.0)
                        .strong()
                        .color(on_accent),
                )
                .fill(th.ui.highlight)
                .stroke(egui::Stroke::new(1.0, th.ui.highlight))
                .corner_radius(6);
                let aide = if info.stored {
                    "Les bornes deviennent la période d'analyse de ce log, et la bibliothèque les garde."
                } else {
                    "Les bornes deviennent la période d'analyse de ce log pour cette séance (log rangé sous aucun drone)."
                };
                if ui.add_enabled(ll.is_some(), enregistrer).on_hover_text(aide).clicked()
                    && let (Some(index), Some(draft)) = (state.current, self.draft.clone())
                {
                    actions.push(ExtAction::Save { index, draft });
                }
                let annuler = egui::Button::new(RichText::new("Annuler").size(13.0).color(th.ui.text))
                    .fill(th.ui.base)
                    .stroke(egui::Stroke::new(1.0, th.ui.mid))
                    .corner_radius(6);
                if ui.add(annuler).on_hover_text("Revenir sans enregistrer").clicked() {
                    actions.push(ExtAction::Leave);
                }
                let texte = if th.dark { "Thème clair" } else { "Thème sombre" };
                let bouton = egui::Button::image(icon("moon", 17.0).tint(th.ui.muted).alt_text(texte))
                    .min_size(Vec2::splat(32.0))
                    .fill(Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(6);
                if ui.add(bouton).on_hover_text(texte).clicked() {
                    actions.push(ExtAction::ToggleTheme);
                }
            });
        });
    }

    /// La barre d'outils au-dessus des graphiques.
    fn toolbar(&mut self, ui: &mut Ui, state: &AppState, th: &Theme) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            if let Some(i) = segmented(ui, th, &["Tous", "Roll", "Pitch", "Yaw"], Some(self.focus)) {
                self.focus = i;
            }
            let (sep, _) = ui.allocate_exact_size(Vec2::new(1.0, 22.0), Sense::hover());
            ui.painter().rect_filled(sep, 0.0, th.ui.border);
            let was = self.measure_on;
            let mesurer = outline_button(th, Some("ruler"), "Mesurer", self.measure_on).selected(self.measure_on);
            if ui
                .add(mesurer)
                .on_hover_text(
                    "Mesure le temps entre deux points (ms) et la fréquence correspondante (Hz) : cliquez deux points sur \
                     un graphique roll, pitch ou yaw. Les repères se déplacent à la souris ; un nouveau clic recommence la \
                     mesure.",
                )
                .clicked()
            {
                self.measure_on = !self.measure_on;
            }
            if was != self.measure_on {
                self.measure.clear();
            }
            if ui.add(outline_button(th, None, "Vue entière", false)).clicked() {
                self.fit = true;
            }
            if ui
                .add(outline_button(th, None, "Cadrer le fragment", false))
                .on_hover_text("Montre le fragment en entier, et lui seul")
                .clicked()
                && let Some(d) = &self.draft
            {
                self.window = Some(d.period);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (couleur, texte) = if self.measure_on && state.current_log().is_some() {
                    match self.measure.len() {
                        0 => (th.ui.muted, "Cliquez un 1er point sur roll, pitch ou yaw.".to_string()),
                        1 => (th.ui.muted, "Cliquez le 2e point.".to_string()),
                        _ => (th.accent("purple"), measure_text(&self.measure)),
                    }
                } else {
                    (
                        th.ui.muted,
                        "Traits bleus : limites du fragment · barre du bas : fenêtre visible".to_string(),
                    )
                };
                ui.label(RichText::new(texte).size(13.0).color(couleur));
                let (place, _) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
                icon("info", 15.0).tint(th.ui.muted).paint_at(ui, place);
            });
        });
    }

    /// Le panneau de droite, section par section.
    fn side_panel(&mut self, ui: &mut Ui, state: &mut AppState, info: &FragmentInfo, th: &Theme) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (k, titre) in SECTIONS.iter().enumerate() {
                let mut open = self.open[k];
                section(ui, th, titre, &mut open, |ui| match k {
                    0 => self.fragment_section(ui, state, info, th),
                    1 => self.traces_section(ui, state, th),
                    2 => self.display_section(ui),
                    3 => self.player(ui, state, th),
                    _ => self.readout(ui, state, th),
                });
                self.open[k] = open;
            }
        });
    }

    /// « Fragment » : nom, rôle, début et fin, durée, limites automatiques ou session entière.
    fn fragment_section(&mut self, ui: &mut Ui, state: &AppState, info: &FragmentInfo, th: &Theme) {
        let (Some(ll), Some(draft)) = (state.current_log(), self.draft.as_mut()) else {
            ui.label(RichText::new("Aucun log ouvert.").size(13.0).color(th.ui.muted));
            return;
        };
        let log = &ll.log;
        field_label(ui, th, "Nom");
        let nom = ui.add(
            egui::TextEdit::singleline(&mut draft.name)
                .font(egui::FontId::proportional(14.0))
                .desired_width(f32::INFINITY)
                .margin(Vec2::new(10.0, 7.0)),
        );
        nom.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, "Nom du fragment"));
        ui.add_space(10.0);
        field_label(ui, th, "Rôle");
        let roles = [FILTERS, PID];
        let actuel = roles.iter().position(|r| *r == draft.role);
        if let Some(i) = segmented(ui, th, &["Filtres", "Série PID"], actuel) {
            draft.role = roles[i].to_string();
        }
        ui.add_space(10.0);
        let mut period = draft.period;
        let mut moved = None;
        ui.columns(2, |cols| {
            for (k, col) in cols.iter_mut().enumerate() {
                let nom = if k == 0 { "Début" } else { "Fin" };
                field_label(col, th, nom);
                let v = if k == 0 { &mut period.0 } else { &mut period.1 };
                if stepper(col, th, nom, v, log) {
                    moved = Some(k);
                }
            }
        });
        if let Some(k) = moved {
            draft.period = snap_bounds(log, period, k);
        }
        ui.add_space(10.0);
        let (a, b) = draft.period;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Durée").size(13.0).color(th.ui.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("{} s", secs(b - a, 3))).size(13.0).monospace());
            });
        });
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if ui
                .add(outline_button(th, Some("scissors"), "Limites auto", false))
                .on_hover_text("Du décollage à l'atterrissage, avant un éventuel crash")
                .clicked()
            {
                draft.period = snap_bounds(log, default_period(log), 0);
            }
            if ui.add(outline_button(th, None, "Session entière", false)).clicked() {
                draft.period = (log.t[0], *log.t.last().unwrap());
            }
        });
        ui.add_space(10.0);
        let note = |ui: &mut Ui, texte: &str| {
            ui.label(RichText::new(texte).size(12.0).color(th.ui.muted));
        };
        note(ui, "Limites auto : décollage et atterrissage retirés, crash exclu.");
        if !info.stored {
            ui.add_space(6.0);
            note(
                ui,
                "Ce log n'est rangé sous aucun drone (pas de nom de machine dans l'en-tête) : ces limites valent pour \
                 cette séance, la bibliothèque ne les garde pas.",
            );
        }
    }

    /// « Traces » : une case par trace présente dans le log, sur deux colonnes, avec sa couleur.
    fn traces_section(&mut self, ui: &mut Ui, state: &AppState, th: &Theme) {
        let log = state.current_log().map(|l| l.log.clone());
        let mut items: Vec<(&'static str, String, Option<Color32>)> = Vec::new();
        for key in TRACE_ORDER {
            if log.as_ref().is_none_or(|l| self.is_present(l, key)) {
                items.push((key, trace_label(key).to_string(), Some(th.trace_color(key))));
            }
        }
        items.push(("motors", "Moteurs".into(), None));
        items.push(("throttle", "Gaz".into(), None));
        if log.as_ref().is_some_and(|l| l.motor_hz.is_some()) {
            items.push(("rps", "Rotation moteurs (Hz)".into(), None));
        }
        // deux colonnes de largeur fixe : un nom long s'abrège au lieu d'élargir le panneau
        let colonne = ((ui.available_width() - 10.0) / 2.0).floor();
        egui::Grid::new("ext-traces")
            .num_columns(2)
            .spacing(Vec2::new(10.0, 8.0))
            .min_col_width(colonne)
            .max_col_width(colonne)
            .show(ui, |ui| {
                for (n, (key, label, color)) in items.into_iter().enumerate() {
                    let mut on = match key {
                        "motors" => self.motors,
                        "throttle" => self.throttle,
                        "rps" => self.rps,
                        k => self.trace_on(k),
                    };
                    if trace_check(ui, th, &mut on, color, &label, colonne) {
                        match key {
                            "motors" => self.motors = on,
                            "throttle" => self.throttle = on,
                            "rps" => self.rps = on,
                            k => self.set_trace(k, on),
                        }
                    }
                    if n % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
        if let Some(l) = &log {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("Debug : {}", l.settings.debug_mode))
                    .size(12.0)
                    .color(th.ui.muted),
            );
        }
    }

    /// « Affichage » : réglages du tracé que la maquette ne montre pas, gardés du visualiseur (écart écrit, D-061).
    fn display_section(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let label = ui.label("Échelle Y (°/s)");
            ui.add(egui::DragValue::new(&mut self.yscale).range(10..=3000).speed(10))
                .labelled_by(label.id);
        });
        egui::ComboBox::from_label("Lissage")
            .selected_text(SMOOTHING[self.smoothing].0)
            .show_ui(ui, |ui| {
                for (i, (name, _)) in SMOOTHING.iter().enumerate() {
                    ui.selectable_value(&mut self.smoothing, i, *name);
                }
            });
        ui.horizontal(|ui| {
            let label = ui.label("Épaisseur");
            ui.add(egui::DragValue::new(&mut self.width).range(1.0..=5.0).speed(0.1))
                .labelled_by(label.id);
        });
    }

    /// Lecture : le curseur avance en temps réel jusqu'à la fin du log.
    fn advance(&mut self, ui: &Ui, state: &mut AppState) {
        if !self.playing {
            return;
        }
        let Some(end) = state.current_log().map(|l| *l.log.t.last().unwrap()) else {
            self.playing = false;
            return;
        };
        if state.cursor >= end {
            self.playing = false;
            return;
        }
        let dt = ui.input(|i| i.stable_dt.min(0.1)) as f64;
        self.seek_rel(state, Some(dt), 0);
        ui.ctx().request_repaint();
    }

    /// Déplace le curseur : de `dt` secondes, ou (`dt` vide) au début (`end` < 0) ou à la fin (`end` > 0) du fragment.
    pub fn seek_rel(&self, state: &mut AppState, dt: Option<f64>, end: i32) {
        let Some(ll) = state.current_log() else { return };
        let (p0, p1) = self
            .draft
            .as_ref()
            .filter(|d| d.uid == ll.uid)
            .map_or(ll.period, |d| d.period);
        let t = match dt {
            None if end < 0 => p0,
            None => p1,
            Some(d) => state.cursor + d,
        };
        let (lo, hi) = (ll.log.t[0], *ll.log.t.last().unwrap());
        state.set_cursor(t.clamp(lo, hi));
    }

    /// Déplace le curseur d'un échantillon (« Image précédente », « Image suivante »).
    pub fn step_sample(&self, state: &mut AppState, delta: isize) {
        let Some(ll) = state.current_log() else { return };
        let i = ll.log.index_of(state.cursor) as isize + delta;
        let t = ll.log.t[i.clamp(0, ll.log.n() as isize - 1) as usize];
        state.set_cursor(t);
    }

    /// « Lecteur » : début et fin du fragment, échantillon par échantillon, lecture.
    fn player(&mut self, ui: &mut Ui, state: &mut AppState, th: &Theme) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let plat = |name: &str, tip: &str| {
                egui::Button::image(icon(name, 17.0).tint(th.ui.muted).alt_text(tip.to_string()))
                    .min_size(Vec2::splat(32.0))
                    .fill(Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(6)
            };
            if ui
                .add(plat("skip-back", "Début"))
                .on_hover_text("Début du fragment")
                .clicked()
            {
                self.seek_rel(state, None, -1);
            }
            if ui
                .add(plat("chevron-left", "Image précédente"))
                .on_hover_text("Image précédente")
                .clicked()
            {
                self.step_sample(state, -1);
            }
            let (nom, tip) = if self.playing {
                ("pause", "Pause")
            } else {
                ("play", "Lecture")
            };
            let lecture = egui::Button::image(icon(nom, 15.0).tint(theme::plot::ON_ACCENT).alt_text(tip))
                .min_size(Vec2::new(36.0, 32.0))
                .fill(th.ui.highlight)
                .stroke(egui::Stroke::NONE)
                .corner_radius(6);
            if ui.add(lecture).on_hover_text(tip).clicked() {
                self.playing = !self.playing;
            }
            if ui
                .add(plat("chevron-right", "Image suivante"))
                .on_hover_text("Image suivante")
                .clicked()
            {
                self.step_sample(state, 1);
            }
            if ui
                .add(plat("skip-forward", "Fin"))
                .on_hover_text("Fin du fragment")
                .clicked()
            {
                self.seek_rel(state, None, 1);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let texte = state.current_log().map_or("—".to_string(), |ll| {
                    format!("{} s", secs(ll.log.t[ll.log.index_of(state.cursor)], 2))
                });
                ui.label(RichText::new(texte).size(14.0).monospace().color(th.ui.text));
            });
        });
    }

    /// « Au curseur » : les manches et les valeurs à l'instant du curseur.
    fn readout(&mut self, ui: &mut Ui, state: &AppState, th: &Theme) {
        let Some(ll) = state.current_log() else {
            ui.label("—");
            return;
        };
        let i = ll.log.index_of(state.cursor);
        let (left, right) = stick_positions(&ll.log, i);
        ui.horizontal(|ui| {
            stick(ui, th, "Y", "G", left);
            stick(ui, th, "R", "P", right);
        });
        ui.label(RichText::new(readout_lines(&ll.log, i).join("\n")).monospace());
    }

    fn plots(&mut self, ui: &mut Ui, state: &mut AppState, th: &Theme, reserve: f32) {
        let Some(ll) = state.current_log() else { return };
        let log = ll.log.clone();
        let (lo, hi) = (log.t[0], *log.t.last().unwrap());
        let (p0, p1) = self.draft.as_ref().map_or(ll.period, |d| d.period);
        let (axes, motors, rps, k) = self.curves(&log, th);

        let shown: Vec<usize> = (0..3).filter(|&a| self.focus == 0 || self.focus == a + 1).collect();
        let total = ui.available_height() - reserve - 4.0 * ui.spacing().item_spacing.y;
        let weights: Vec<f32> = shown
            .iter()
            .map(|_| if self.focus == 0 { AXIS_H } else { 5.0 * MOTOR_H })
            .chain([if self.focus == 0 { MOTOR_H } else { 2.0 * MOTOR_H }])
            .collect();
        let sum: f32 = weights.iter().sum();

        // repères glissables, par priorité : points de mesure (absents = NaN, jamais attrapés), curseur, bornes du
        // fragment
        let m = |k: usize| {
            if self.measure_on {
                self.measure.get(k).copied().unwrap_or(f64::NAN)
            } else {
                f64::NAN
            }
        };
        let mut xs = [m(0), m(1), state.cursor, p0, p1];
        let before = xs;
        let press = ui.input(|i| i.pointer.press_origin()).map(|p| p.x);
        let fit = std::mem::take(&mut self.fit);
        let request = self.window.take();
        let width_px = ui.available_width();
        let ymax = self.yscale as f64;
        let mut drags = self.drags;
        let mut transforms = self.transforms;
        let link = self.link;
        let t = &log.t;
        let measuring = self.measure_on;
        let result = measure_text(&self.measure);
        let purple = th.accent("purple");
        let mut clicks: Vec<f64> = Vec::new();
        let mut frames: Vec<(usize, egui::Rect)> = Vec::new();

        let n_plots = shown.len() + 1;
        for (row, &ax) in shown.iter().chain(std::iter::once(&3)).enumerate() {
            let is_motor = ax == 3;
            let last = row + 1 == n_plots;
            let h = (total * weights[row] / sum).max(60.0);
            let (curves, extra): (&[Curve], &[Curve]) = if is_motor { (&motors, &rps) } else { (&axes[ax], &[]) };
            let left = AxisHints::new_y()
                .label(if is_motor {
                    "Gaz | moteurs (%)".to_string()
                } else {
                    format!("{} (°/s)", AXIS_NAMES[ax])
                })
                .min_thickness(62.0);
            let right = if is_motor && !rps.is_empty() {
                AxisHints::new_y()
                    .placement(HPlacement::Right)
                    .label("Hz")
                    .min_thickness(48.0)
                    .formatter(move |m, _| format!("{:.0}", m.value / k))
            } else {
                AxisHints::new_y()
                    .placement(HPlacement::Right)
                    .min_thickness(48.0)
                    .formatter(|_, _| String::new())
            };
            let drag = &mut drags[ax];
            let tr = &mut transforms[ax];
            let busy = drag.busy();
            let modes = &self.modes;
            Plot::new(("viewer", ax))
                .height(h)
                .custom_y_axes(vec![left, right])
                .show_axes([last, true])
                .x_axis_label(if last { "Temps (s)" } else { "" })
                .auto_bounds(false)
                .allow_zoom([true, false])
                .allow_drag([!busy, false])
                .allow_scroll(false)
                .link_axis(link, [true, false])
                .link_cursor(link, [true, false])
                .grid_color(th.grid_color())
                .show(ui, |pu| {
                    if fit {
                        // une marge de 1 % : une borne ou le curseur posés au tout début (ou à la toute fin) du log
                        // restent à portée de souris, au lieu de tomber sur le bord du graphique
                        let pad = (hi - lo) * 0.01;
                        pu.set_plot_bounds_x((lo - pad)..=(hi + pad));
                    } else if let Some((a, b)) = request {
                        pu.set_plot_bounds_x(a..=b);
                    }
                    if is_motor {
                        pu.set_plot_bounds_y(-2.0..=102.0);
                    } else {
                        pu.set_plot_bounds_y(-ymax..=ymax);
                    }
                    let b = pu.plot_bounds();
                    let (x0, x1) = (b.min()[0], b.max()[0]);
                    let max_points = (width_px as usize).max(200) * 2;
                    // hors du fragment : un voile léger, comme la maquette ; dedans, rien
                    pu.span(Span::new("", lo..=xs[3].min(xs[4])).fill(th.shade));
                    pu.span(Span::new("", xs[3].max(xs[4])..=hi).fill(th.shade));
                    for c in curves.iter().chain(extra) {
                        let mut line = Line::new(
                            c.name.as_str(),
                            PlotPoints::from(window_points(t, &c.data.y, &c.data.pyramid, x0, x1, max_points)),
                        )
                        .color(c.color)
                        .width(c.width);
                        if c.dotted {
                            line = line.style(LineStyle::dotted_loose());
                        }
                        pu.line(line);
                    }
                    if is_motor {
                        for (tm, label) in modes {
                            pu.text(
                                Text::new("", PlotPoint::new(*tm, 0.0), label.as_str())
                                    .color(th.fg)
                                    .anchor(egui::Align2::LEFT_BOTTOM),
                            );
                        }
                    }
                    pu.vline(VLine::new("Début", xs[3]).color(th.ui.highlight).width(BOUND_W));
                    pu.vline(VLine::new("Fin", xs[4]).color(th.ui.highlight).width(BOUND_W));
                    if measuring && !is_motor {
                        for &x in xs.iter().take(self.measure.len().min(2)) {
                            pu.vline(
                                VLine::new("Mesure", x)
                                    .color(purple)
                                    .width(2.0)
                                    .style(LineStyle::dashed_loose()),
                            );
                        }
                        if self.measure.len() == 2 {
                            let mid = (xs[0] + xs[1]) / 2.0;
                            pu.text(
                                Text::new("", PlotPoint::new(mid, ymax), result.as_str())
                                    .color(purple)
                                    .anchor(egui::Align2::CENTER_TOP),
                            );
                        }
                        if !busy
                            && pu.response().clicked()
                            && let Some(p) = pu.pointer_coordinate()
                        {
                            clicks.push(p.x);
                        }
                    }
                    pu.vline(
                        VLine::new("Curseur", xs[2])
                            .color(th.fg)
                            .width(1.0)
                            .style(LineStyle::dashed_loose()),
                    );
                    let limits = [(lo, hi); 5];
                    if is_motor {
                        // pas de repères de mesure sur ce graphique
                        let mut tmp = xs;
                        tmp[0] = f64::NAN;
                        tmp[1] = f64::NAN;
                        drag.update(pu, press, &mut tmp, &limits);
                        xs[2..].copy_from_slice(&tmp[2..]);
                    } else {
                        drag.update(pu, press, &mut xs, &limits);
                    }
                    *tr = Some(*pu.transform());
                });
            if let Some(tr) = tr {
                frames.push((ax, *tr.frame()));
            }
        }
        self.drags = drags;
        self.transforms = transforms;
        if self.drags.iter().any(|d| d.busy()) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        for k in 0..self.measure.len().min(2) {
            if self.measure_on && xs[k] != before[k] {
                self.measure[k] = xs[k];
            }
        }
        // un clic place un point ; le 3e clic recommence la mesure
        if let Some(&x) = clicks.first() {
            if self.measure.len() == 1 {
                self.measure.push(x);
            } else {
                self.measure = vec![x];
            }
        }
        if xs[2] != before[2] {
            state.set_cursor(xs[2]);
        }
        if let Some(d) = self.draft.as_mut() {
            if xs[3] != before[3] {
                d.period = snap_bounds(&log, (xs[3], xs[4]), 0);
            } else if xs[4] != before[4] {
                d.period = snap_bounds(&log, (xs[3], xs[4]), 1);
            }
        }
        let (a, b) = self.draft.as_ref().map_or((p0, p1), |d| d.period);
        self.bound_marks(ui, th, &frames, (a, b));
    }

    /// Onglets « Début … s » / « Fin … s » en haut du premier graphique, et poignées au milieu de chaque graphique
    /// d'axe, posés sur les bornes du fragment quand elles sont dans la fenêtre visible.
    fn bound_marks(&self, ui: &Ui, th: &Theme, frames: &[(usize, egui::Rect)], (a, b): (f64, f64)) {
        let p = ui.painter();
        let on_accent = theme::plot::ON_ACCENT;
        for (k, t) in [a, b].into_iter().enumerate() {
            for (n, (ax, frame)) in frames.iter().enumerate() {
                let Some(tr) = self.transforms[*ax] else { continue };
                let x = tr.position_from_point_x(t);
                if x < frame.left() - 0.5 || x > frame.right() + 0.5 {
                    continue;
                }
                if n == 0 {
                    // l'onglet du début s'ouvre à droite de son trait, celui de la fin à gauche
                    let left = if k == 0 { x - 2.0 } else { x + 2.0 - TAB_SIZE.x };
                    let tab = egui::Rect::from_min_size(egui::pos2(left, frame.top()), TAB_SIZE);
                    p.rect_filled(tab, 0.0, th.ui.highlight);
                    let texte = format!("{} {} s", if k == 0 { "Début" } else { "Fin" }, secs(t, 1));
                    p.text(
                        tab.center(),
                        egui::Align2::CENTER_CENTER,
                        texte,
                        egui::FontId::proportional(11.5),
                        on_accent,
                    );
                }
                if *ax < 3 {
                    let grip = egui::Rect::from_center_size(egui::pos2(x, frame.center().y), GRIP_SIZE);
                    p.rect(
                        grip,
                        3.0,
                        th.ui.base,
                        egui::Stroke::new(1.5, th.ui.highlight),
                        egui::StrokeKind::Middle,
                    );
                    for dx in [-1.5, 1.5] {
                        p.vline(
                            x + dx,
                            (grip.center().y - 7.0)..=(grip.center().y + 7.0),
                            egui::Stroke::new(1.0, th.ui.highlight),
                        );
                    }
                }
            }
        }
    }

    /// La barre de navigation : le log entier (silhouette du gaz), le fragment en bleu, la fenêtre visible dans un
    /// cadre qu'on déplace ou qu'on élargit par ses poignées ; un clic ailleurs y centre la fenêtre.
    fn navigator(&mut self, ui: &mut Ui, log: &Arc<FlightLog>, th: &Theme) {
        let (lo, hi) = (log.t[0], *log.t.last().unwrap());
        let span = (hi - lo).max(f64::EPSILON);
        // alignée sur la zone tracée des graphiques (au dernier cadre), comme la maquette
        let row = ui
            .allocate_exact_size(Vec2::new(ui.available_width(), NAV_H), Sense::hover())
            .0;
        let (left, right) =
            self.transforms[3].map_or((row.left(), row.right()), |t| (t.frame().left(), t.frame().right()));
        let rect = egui::Rect::from_x_y_ranges(left..=right, row.y_range());
        self.nav_rect = Some(rect);
        let to_x = |t: f64| rect.left() + ((t - lo) / span) as f32 * rect.width();
        let to_t = |x: f32| lo + ((x - rect.left()) / rect.width()) as f64 * span;
        let (ws, we) = self.visible().unwrap_or((lo, hi));
        let (ws, we) = (ws.max(lo), we.min(hi));

        // interaction : poignées, cadre, ou ailleurs
        let id = ui.scope_id().with("ext-nav");
        let resp = ui.interact(rect.expand2(Vec2::new(8.0, 3.0)), id, Sense::click_and_drag());
        let (xa, xb) = (to_x(ws), to_x(we));
        let mode_at = |x: f32| {
            if (x - xa).abs() <= NAV_HANDLE.x / 2.0 + 1.0 {
                Some(NavMode::Left)
            } else if (x - xb).abs() <= NAV_HANDLE.x / 2.0 + 1.0 {
                Some(NavMode::Right)
            } else if x > xa && x < xb {
                Some(NavMode::Pan)
            } else {
                None
            }
        };
        let gap = min_span(span);
        if resp.drag_started() {
            let press = ui.input(|i| i.pointer.press_origin()).map_or(rect.center().x, |p| p.x);
            self.nav_drag = Some(match mode_at(press) {
                Some(m) => (m, (ws, we)),
                None => {
                    // pressé hors du cadre : la fenêtre y saute, puis suit le pointeur
                    let w = we - ws;
                    let a = (to_t(press) - w / 2.0).clamp(lo, hi - w);
                    (NavMode::Pan, (a, a + w))
                }
            });
        }
        if let Some((mode, (a0, b0))) = self.nav_drag {
            if resp.dragged() {
                let origin = ui.input(|i| i.pointer.press_origin()).map_or(0.0, |p| p.x);
                let now = resp.interact_pointer_pos().map_or(origin, |p| p.x);
                let dt = ((now - origin) / rect.width()) as f64 * span;
                let w = b0 - a0;
                self.window = Some(match mode {
                    NavMode::Pan => {
                        let a = (a0 + dt).clamp(lo, hi - w);
                        (a, a + w)
                    }
                    NavMode::Left => ((a0 + dt).clamp(lo, b0 - gap), b0),
                    NavMode::Right => (a0, (b0 + dt).clamp(a0 + gap, hi)),
                });
            } else {
                self.nav_drag = None;
            }
        } else if resp.clicked()
            && let Some(p) = resp.interact_pointer_pos()
            && mode_at(p.x).is_none()
        {
            let w = we - ws;
            let a = (to_t(p.x) - w / 2.0).clamp(lo, hi - w);
            self.window = Some((a, a + w));
        }
        if let Some(p) = resp.hover_pos() {
            let icone = match self.nav_drag.map(|d| d.0).or(mode_at(p.x)) {
                Some(NavMode::Left | NavMode::Right) => egui::CursorIcon::ResizeHorizontal,
                Some(NavMode::Pan) if self.nav_drag.is_some() => egui::CursorIcon::Grabbing,
                Some(NavMode::Pan) => egui::CursorIcon::Grab,
                None => egui::CursorIcon::PointingHand,
            };
            ui.ctx().set_cursor_icon(icone);
        }

        // dessin
        let p = ui.painter();
        p.rect(
            rect,
            6.0,
            th.ui.alt,
            egui::Stroke::new(1.0, th.ui.border),
            egui::StrokeKind::Inside,
        );
        if let Some((a, b)) = self.draft.as_ref().map(|d| d.period) {
            let frag = egui::Rect::from_x_y_ranges(to_x(a)..=to_x(b), rect.y_range());
            p.rect_filled(frag, 0.0, with_alpha(th.ui.highlight, 20));
            for x in [frag.left(), frag.right()] {
                p.vline(x, rect.y_range(), egui::Stroke::new(2.0, th.ui.highlight));
            }
        }
        let largeur = rect.width().max(1.0) as usize;
        if self
            .nav_shape
            .as_ref()
            .is_none_or(|(u, w, _)| Some(*u) != self.shown || *w != largeur)
        {
            self.nav_shape = self.shown.map(|u| (u, largeur, column_means(&log.throttle, largeur)));
        }
        if let Some((_, _, shape)) = &self.nav_shape {
            let (top, bottom) = (rect.top() + 4.0, rect.bottom() - 4.0);
            let points: Vec<egui::Pos2> = shape
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    egui::pos2(
                        rect.left() + i as f32 + 0.5,
                        bottom - (v.clamp(0.0, 100.0) / 100.0) * (bottom - top),
                    )
                })
                .collect();
            p.add(egui::Shape::line(points, egui::Stroke::new(1.0, th.ui.muted)));
        }
        let (xa, xb) = (to_x(ws), to_x(we));
        p.rect_filled(
            egui::Rect::from_x_y_ranges(rect.left()..=xa, rect.y_range()),
            egui::CornerRadius {
                nw: 6,
                sw: 6,
                ..Default::default()
            },
            th.veil,
        );
        p.rect_filled(
            egui::Rect::from_x_y_ranges(xb..=rect.right(), rect.y_range()),
            egui::CornerRadius {
                ne: 6,
                se: 6,
                ..Default::default()
            },
            th.veil,
        );
        let cadre = egui::Rect::from_x_y_ranges(xa..=xb, (rect.top() - 3.0)..=(rect.bottom() + 3.0));
        p.rect(
            cadre,
            6.0,
            with_alpha(th.ui.highlight, 26),
            egui::Stroke::new(2.0, th.ui.highlight),
            egui::StrokeKind::Inside,
        );
        for x in [xa, xb] {
            let poignee = egui::Rect::from_center_size(egui::pos2(x, rect.center().y), NAV_HANDLE);
            p.rect_filled(poignee, 4.0, th.ui.highlight);
            for dx in [-1.5, 1.5] {
                p.vline(
                    x + dx,
                    (poignee.center().y - 8.0)..=(poignee.center().y + 8.0),
                    egui::Stroke::new(1.5, theme::plot::ON_ACCENT),
                );
            }
        }

        // graduations du log entier
        let (ticks, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
        let p = ui.painter();
        let step = tick_step(span);
        let mut t = (lo / step).ceil() * step;
        while t <= hi + 1e-9 {
            p.text(
                egui::pos2(to_x(t), ticks.top()),
                egui::Align2::CENTER_TOP,
                format!("{}", (t * 10.0).round() / 10.0).replace('.', ","),
                egui::FontId::proportional(10.5),
                th.ui.muted,
            );
            t += step;
        }
        ui.horizontal(|ui| {
            ui.add_space((left - ui.min_rect().left()).max(0.0));
            ui.label(
                RichText::new(format!("Log entier · {} s · fragment en bleu", secs(span, 1)))
                    .size(12.0)
                    .color(th.ui.muted),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space((ui.max_rect().right() - right).max(0.0));
                ui.label(
                    RichText::new(format!("{} – {} s ({} s)", secs(ws, 1), secs(we, 1), secs(we - ws, 1)))
                        .size(12.0)
                        .monospace()
                        .strong()
                        .color(th.ui.text),
                );
                ui.label(RichText::new("Fenêtre visible").size(12.0).color(th.ui.muted));
            });
        });
    }
}

/// Moyenne de `y` par colonne, sur `columns` colonnes : la silhouette du log dans la barre de navigation.
pub fn column_means(y: &[f32], columns: usize) -> Vec<f32> {
    if y.is_empty() || columns == 0 {
        return Vec::new();
    }
    (0..columns)
        .map(|c| {
            let a = c * y.len() / columns;
            let b = ((c + 1) * y.len() / columns).max(a + 1).min(y.len());
            let part = &y[a.min(y.len() - 1)..b];
            part.iter().sum::<f32>() / part.len() as f32
        })
        .collect()
}

/// Libellé d'un champ du panneau (12 points, couleur discrète).
fn field_label(ui: &mut Ui, th: &Theme, texte: &str) {
    ui.label(RichText::new(texte).size(12.0).color(th.ui.muted));
    ui.add_space(4.0);
}

/// Une section repliable du panneau de droite : titre en capitales, chevron, contenu, trait de séparation.
fn section(ui: &mut Ui, th: &Theme, titre: &str, open: &mut bool, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            let chevron = if *open { "chevron-down" } else { "chevron-right" };
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::click());
            let texte = titre.to_uppercase();
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::DisclosureTriangle, true, *open, &texte));
            if resp.clicked() {
                *open = !*open;
            }
            ui.painter().text(
                rect.left_center(),
                egui::Align2::LEFT_CENTER,
                &texte,
                egui::FontId::proportional(11.0),
                th.ui.muted,
            );
            icon(chevron, 14.0).tint(th.ui.muted).paint_at(
                ui,
                egui::Rect::from_center_size(rect.right_center() - Vec2::new(7.0, 0.0), Vec2::splat(14.0)),
            );
            if *open {
                ui.add_space(10.0);
                body(ui);
            }
        });
    let r = ui.min_rect();
    ui.painter()
        .hline(r.x_range(), ui.cursor().top(), egui::Stroke::new(1.0, th.ui.border));
}

/// Commande segmentée de la maquette (« Tous | Roll | Pitch | Yaw », « Filtres | Série PID ») : fond gris, la
/// valeur choisie en blanc. Rend l'indice cliqué.
fn segmented(ui: &mut Ui, th: &Theme, labels: &[&str], selected: Option<usize>) -> Option<usize> {
    let mut clicked = None;
    egui::Frame::new()
        .fill(th.ui.hover)
        .corner_radius(7)
        .inner_margin(3)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.spacing_mut().button_padding = Vec2::new(12.0, 5.0);
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
                        .fill(if on { th.ui.base } else { Color32::TRANSPARENT })
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(5);
                    if ui.add(b).clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
    clicked
}

/// Bouton à bordure de la maquette (13 points, coins de 6), avec une icône facultative ; `on` : mis en valeur.
fn outline_button<'a>(th: &Theme, icone: Option<&str>, texte: &'a str, on: bool) -> egui::Button<'a> {
    let couleur = if on { th.ui.highlight } else { th.ui.text };
    let rich = RichText::new(texte).size(13.0).color(couleur);
    let b = match icone {
        Some(n) => egui::Button::image_and_text(icon(n, 15.0).tint(couleur), rich),
        None => egui::Button::new(rich),
    };
    b.fill(if on { th.ui.select } else { th.ui.base })
        .stroke(egui::Stroke::new(1.0, if on { th.ui.highlight } else { th.ui.mid }))
        .corner_radius(6)
}

/// Case d'une trace : la case, un trait de sa couleur, son nom (dessin de la maquette), sur `width` points au plus
/// (un nom trop long s'abrège par « … », le nom entier reste en infobulle). Rend `true` si elle a changé.
fn trace_check(ui: &mut Ui, th: &Theme, on: &mut bool, color: Option<Color32>, label: &str, width: f32) -> bool {
    let swatch = if color.is_some() { 14.0 + 8.0 } else { 0.0 };
    let place = (width - 15.0 - 8.0 - swatch).max(10.0);
    let mut job =
        egui::text::LayoutJob::simple_singleline(label.to_string(), egui::FontId::proportional(13.0), th.ui.text);
    job.wrap = egui::text::TextWrapping::truncate_at_width(place);
    let galley = ui.painter().layout_job(job);
    let abrege = galley.elided;
    let size = Vec2::new(width, 18.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if abrege {
        resp = resp.on_hover_text(label);
    }
    if resp.clicked() {
        *on = !*on;
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, *on, label));
    let p = ui.painter();
    let case = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.center().y - 7.5), Vec2::splat(15.0));
    if *on {
        p.rect(
            case,
            4.0,
            th.ui.highlight,
            egui::Stroke::new(1.0, th.ui.highlight),
            egui::StrokeKind::Inside,
        );
        icon("check", 11.0)
            .tint(theme::plot::ON_ACCENT)
            .paint_at(ui, egui::Rect::from_center_size(case.center(), Vec2::splat(11.0)));
    } else {
        p.rect(
            case,
            4.0,
            th.ui.base,
            egui::Stroke::new(1.0, th.ui.mid),
            egui::StrokeKind::Inside,
        );
    }
    let mut x = case.right() + 8.0;
    if let Some(c) = color {
        let trait_ = egui::Rect::from_min_size(egui::pos2(x, rect.center().y - 1.5), Vec2::new(14.0, 3.0));
        p.rect_filled(trait_, 2.0, c);
        x += 14.0 + 8.0;
    }
    p.galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        th.ui.text,
    );
    resp.clicked()
}

/// Champ « Début » ou « Fin » : ‹ valeur s ›. Les flèches déplacent de 0,1 s, la valeur se tape ou se glisse
/// à l'échantillon près. Rend `true` si la valeur a changé (l'appelant la pose ensuite sur un échantillon).
fn stepper(ui: &mut Ui, th: &Theme, nom: &str, v: &mut f64, log: &FlightLog) -> bool {
    let mut changed = false;
    egui::Frame::new()
        .fill(th.ui.base)
        .stroke(egui::Stroke::new(1.0, th.ui.mid))
        .corner_radius(6)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let fleche = |n: &str, tip: String| {
                    egui::Button::image(icon(n, 14.0).tint(th.ui.muted).alt_text(tip))
                        .min_size(Vec2::new(28.0, 32.0))
                        .fill(th.ui.alt)
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(0)
                };
                if ui
                    .add(fleche("chevron-left", format!("{nom} : reculer de 0,1 s")))
                    .clicked()
                {
                    *v -= NUDGE;
                    changed = true;
                }
                let valeur_w = (ui.available_width() - 28.0).max(40.0);
                let dt = 1.0 / log.fs.max(1.0);
                // la valeur s'écrit à nu entre les deux flèches, comme la maquette : pas de cadre au repos
                let w = &mut ui.visuals_mut().widgets.inactive;
                w.bg_stroke = egui::Stroke::NONE;
                w.weak_bg_fill = Color32::TRANSPARENT;
                w.bg_fill = Color32::TRANSPARENT;
                let r = ui.add_sized(
                    Vec2::new(valeur_w, 32.0),
                    egui::DragValue::new(v)
                        .speed(dt)
                        .min_decimals(3)
                        .max_decimals(4)
                        .custom_formatter(|x, _| secs(x, 3))
                        .custom_parser(|s| s.trim().trim_end_matches('s').trim().replace(',', ".").parse().ok())
                        .suffix(" s"),
                );
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::SpinButton, true, nom));
                let i = log.index_of(*v);
                let r = r.on_hover_text(format!("Échantillon {i}"));
                if r.changed() {
                    changed = true;
                }
                if ui
                    .add(fleche("chevron-right", format!("{nom} : avancer de 0,1 s")))
                    .clicked()
                {
                    *v += NUDGE;
                    changed = true;
                }
            });
        });
    changed
}

/// Position d'un manche : carré avec un point, `pos` de −1 à 1 (y = +1 en haut).
fn stick(ui: &mut Ui, th: &Theme, x_label: &str, y_label: &str, pos: [f32; 2]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 96.0), egui::Sense::hover());
    let side = rect.width() - 20.0;
    let sq = egui::Rect::from_min_size(rect.min + egui::vec2(14.0, 2.0), egui::vec2(side, side));
    let p = ui.painter();
    p.rect_stroke(sq, 0.0, egui::Stroke::new(1.0, th.ui.text), egui::StrokeKind::Inside);
    let mid = egui::Stroke::new(1.0, th.grid_color());
    p.line_segment([sq.center_top(), sq.center_bottom()], mid);
    p.line_segment([sq.left_center(), sq.right_center()], mid);
    let c = egui::pos2(
        sq.left() + (pos[0] + 1.0) / 2.0 * side,
        sq.top() + (1.0 - (pos[1] + 1.0) / 2.0) * side,
    );
    p.circle_stroke(c, 5.0, egui::Stroke::new(2.0, th.ui.text));
    let font = egui::FontId::proportional(11.0);
    p.text(
        sq.left_center() - egui::vec2(4.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        y_label,
        font.clone(),
        th.ui.text,
    );
    p.text(
        sq.center_bottom() + egui::vec2(0.0, 2.0),
        egui::Align2::CENTER_TOP,
        x_label,
        font,
        th.ui.text,
    );
}
