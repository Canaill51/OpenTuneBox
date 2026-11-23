//! Un outil = une page de la fenêtre principale (port de `gui/tools/base.py:ToolWindow`).
//!
//! Le compilateur remplace le test AST de `test_gui_tools.py` : `Tool::compute` est une fonction associée
//! sans `self`, elle ne peut donc pas lire les réglages de l'interface. Elle ne reçoit que `Options`
//! (copie des réglages au moment de la demande) et `LogInput`, deux types `Send`.

use std::sync::Arc;

use egui::{Color32, RichText, Ui};
use otb_log::flightlog::FlightLog;

use crate::state::{AppState, Cache, NoiseSignal};
use crate::theme::Theme;
use crate::tools::common::AxisToggles;
use crate::worker::Runner;

/// Copie d'un log chargé, envoyée au thread de calcul.
#[derive(Clone)]
pub struct LogInput {
    pub log: Arc<FlightLog>,
    /// Identifiant du chargement (`LoadedLog::uid`) : à préférer à l'adresse du log pour reconnaître un log (L-072).
    pub uid: u64,
    pub period: (f64, f64),
    pub label: String,
    pub color: Color32,
    /// Résultats déjà calculés pour ce log (voir `Cache` : la clé doit contenir réglages et période).
    pub cache: Cache,
}

impl AppState {
    /// Entrées de calcul des logs `indices` (les indices hors limites sont ignorés).
    pub fn inputs(&self, indices: &[usize]) -> Vec<LogInput> {
        indices
            .iter()
            .filter_map(|&i| self.logs.get(i))
            .map(|l| LogInput {
                log: l.log.clone(),
                uid: l.uid,
                period: l.period,
                label: l.label.clone(),
                color: l.color,
                cache: l.extras.clone(),
            })
            .collect()
    }
}

/// Message d'état quand aucun log n'est sélectionné.
const NO_LOG: &str = "Activez au moins un fragment dans la barre du haut.";

/// Longueur gardée d'un message d'erreur de calcul (`err[-600:]` de `ToolWindow`).
pub const ERROR_CHARS: usize = 600;

/// Texte de la barre d'état pour une erreur de calcul : les `ERROR_CHARS` derniers caractères (la fin d'une trace d'erreur dit le plus).
pub fn error_status(e: &str) -> String {
    let skip = e.chars().count().saturating_sub(ERROR_CHARS);
    format!("Erreur de calcul : {}", e.chars().skip(skip).collect::<String>())
}

pub trait Tool {
    /// Réglages au moment de la demande : comparés pour savoir s'il faut recalculer.
    type Options: Clone + PartialEq + Send + 'static;
    type Output: Send + 'static;

    /// Libellé de l'onglet.
    const TITLE: &'static str;
    /// Bouton « Calculer » (la Configuration n'en a pas : elle se met à jour sans calcul, comme en Python).
    const RUN_BUTTON: bool = true;
    /// Bouton « Tout voir » : l'outil a des graphiques zoomables dont `reset_view` remet les bornes au cadre suivant.
    const HAS_VIEW: bool = false;
    /// Outil ouvert depuis un autre (pas de bouton dans la barre) : `OTHER_TOOLS` de `main_window.py`.
    const HIDDEN: bool = false;
    /// Bouton de la barre qui reste mis en valeur quand cet outil est affiché (par défaut, le sien).
    const TAB: Option<&'static str> = None;
    /// Espace auquel cet outil appartient (D-060) : le rail porte alors le nom de l'espace, pas celui de l'outil.
    /// Plusieurs outils d'un même espace n'y font qu'**une** entrée.
    const SPACE: Option<&'static str> = None;

    /// Copie les réglages de l'interface (appelé dans le thread de l'interface).
    fn options(&self) -> Self::Options;
    /// Indices des logs à analyser.
    fn selected(&self, state: &AppState) -> Vec<usize>;
    /// Calcul, dans le thread de calcul.
    fn compute(options: Self::Options, logs: Vec<LogInput>) -> Result<Self::Output, String>;
    /// Panneau de réglages.
    fn controls(&mut self, ui: &mut Ui, state: &AppState, theme: &Theme);
    /// Affichage du dernier résultat. `state` : le curseur partagé et la période, que certains outils déplacent.
    fn render(&mut self, ui: &mut Ui, state: &mut AppState, output: &Self::Output, theme: &Theme);
    /// Outil à ouvrir ensuite (son `TITLE`), demandé par un bouton de cet outil ; lu une fois par la fenêtre.
    fn take_goto(&mut self) -> Option<&'static str> {
        None
    }
    /// Message d'état propre à l'outil (barre d'état, hors erreur de calcul et calcul en cours).
    fn status(&self) -> Option<String> {
        None
    }
    /// Oublie la vue des graphiques (bouton « Tout voir »).
    fn reset_view(&mut self) {}

    /// Signal que montrent les réglages de la vue, pour le choix « Gyro | Terme D » de son espace (D-069) ; `None` :
    /// un autre (erreur PID, gyro et terme D mêlés…).
    fn signal(&self) -> Option<NoiseSignal> {
        None
    }

    /// Met la vue sur ce signal, choisi dans une vue de son espace. ⚠️ Ne touche à rien si elle le montre déjà : un
    /// choix plus fin fait dans ses cases (le gyro filtré seul…) doit survivre au passage d'une vue à l'autre.
    fn show_signal(&mut self, _signal: NoiseSignal, _state: &AppState) {}

    /// Cases des axes, partagées par les vues de son espace (D-069).
    fn axes_mut(&mut self) -> Option<&mut AxisToggles> {
        None
    }
}

/// Une page d'outil, vue par la fenêtre principale.
pub trait Page {
    fn title(&self) -> &'static str;
    /// Pas de bouton dans la barre (outil ouvert depuis un autre).
    fn hidden(&self) -> bool {
        false
    }
    /// Titre du bouton de la barre mis en valeur quand cette page est affichée.
    fn tab(&self) -> &'static str {
        self.title()
    }
    /// Espace auquel la page appartient (D-060) : son nom est celui de l'entrée du rail.
    fn space(&self) -> Option<&'static str> {
        None
    }
    /// Nom de l'entrée du rail pour cette page : celui de son espace, sinon son titre.
    fn entry(&self) -> &'static str {
        self.space().unwrap_or_else(|| self.title())
    }
    /// Un calcul est en cours (captures et tests : attendre sa fin).
    fn busy(&self) -> bool {
        false
    }
    /// Page à ouvrir ensuite (son titre), demandée pendant le dernier affichage.
    fn take_goto(&mut self) -> Option<&'static str> {
        None
    }
    /// Vue d'espace demandée par ses onglets (D-060). Séparée de `take_goto` : passer d'une vue à l'autre d'un
    /// espace déjà ouvert ne demande **pas** qu'un log soit ouvert, là où les boutons du Guide, eux, l'exigent.
    fn take_space_goto(&mut self) -> Option<&'static str> {
        None
    }
    /// Le bouton « Exporter image » a été cliqué pendant le dernier affichage : la fenêtre exporte alors toute la page.
    fn take_export(&mut self) -> bool {
        false
    }
    fn ui(&mut self, ui: &mut Ui, state: &mut AppState, theme: &Theme);
}

pub struct ToolPage<T: Tool> {
    pub tool: T,
    runner: Runner<T::Output>,
    last: Option<T::Output>,
    /// Message d'erreur du dernier calcul (barre d'état).
    pub status: String,
    asked: Option<(T::Options, Vec<usize>, u64)>,
    /// Numéro de la dernière demande (ou du dernier vidage) : seul le résultat qui le porte est affiché (C2).
    latest: u64,
    /// Numéro de la demande du calcul en cours dans le `Runner`.
    in_flight: u64,
    /// Numéro de la demande mise en attente derrière le calcul en cours.
    queued: Option<u64>,
    export: bool,
    /// Vue d'espace demandée par ses onglets au dernier affichage (D-060).
    space_goto: Option<&'static str>,
    /// Dernier choix de l'espace (`SpaceChoice::revision`) appliqué à cette vue ; `None` : jamais affichée.
    seen_space: Option<u64>,
}

impl<T: Tool> ToolPage<T> {
    pub fn new(tool: T) -> Self {
        Self {
            tool,
            runner: Runner::default(),
            last: None,
            status: String::new(),
            asked: None,
            latest: 0,
            in_flight: 0,
            queued: None,
            export: false,
            space_goto: None,
            seen_space: None,
        }
    }

    pub fn busy(&self) -> bool {
        self.runner.busy()
    }

    pub fn result(&self) -> Option<&T::Output> {
        self.last.as_ref()
    }

    /// Lance (ou met en attente) un calcul avec les réglages actuels.
    pub fn run(&mut self, ctx: &egui::Context, state: &AppState) {
        let (opts, sel) = (self.tool.options(), self.tool.selected(state));
        let inputs = state.inputs(&sel);
        self.asked = Some((opts.clone(), sel, state.revision()));
        self.latest += 1;
        // même règle que `Runner::request` : lancé tout de suite, ou en attente (la dernière remplace la précédente)
        if self.runner.busy() {
            self.queued = Some(self.latest);
        } else {
            self.in_flight = self.latest;
        }
        self.runner.request(ctx, move || T::compute(opts, inputs));
    }

    /// Bouton « Calculer » : relance même si rien n'a changé. Sans log sélectionné, rien à calculer : `update` a déjà
    /// effacé le résultat et affiché « Activez au moins un fragment dans la barre du haut. ».
    fn run_now(&mut self, ctx: &egui::Context, state: &AppState) {
        if !self.tool.selected(state).is_empty() {
            self.run(ctx, state);
        }
    }

    /// Met la vue au choix de son espace (D-069) si elle ne l'a pas encore vu : au premier affichage, puis à chaque
    /// choix fait dans une autre vue. Appelé **avant** `update`, pour que le calcul parte avec le bon signal.
    pub fn follow_space(&mut self, state: &AppState) {
        if T::SPACE.is_none() || self.seen_space == Some(state.space.revision) {
            return;
        }
        self.seen_space = Some(state.space.revision);
        self.tool.show_signal(state.space.signal, state);
        if let Some(axes) = self.tool.axes_mut() {
            axes.0 = state.space.axes;
        }
    }

    /// Fait d'un changement fait dans cette vue le choix de tout l'espace : un clic sur « Gyro | Terme D », un axe
    /// coché, ou un signal choisi dans ses cases (`before` : le signal de la vue avant ses réglages). Un passage à
    /// « autre chose » (`None`) ne change pas l'espace.
    fn publish_space(&mut self, state: &mut AppState, before: Option<NoiseSignal>) {
        let mut next = state.space;
        let now = self.tool.signal();
        if let Some(s) = now
            && now != before
        {
            next.signal = s;
        }
        if let Some(axes) = self.tool.axes_mut() {
            next.axes = axes.0;
        }
        if (next.signal, next.axes) != (state.space.signal, state.space.axes) {
            next.revision += 1;
            state.space = next;
            self.seen_space = Some(next.revision);
        }
    }

    /// Lit un résultat terminé et relance au besoin si les réglages, la sélection ou les logs ont changé.
    pub fn update(&mut self, ctx: &egui::Context, state: &AppState) {
        if let Some(res) = self.runner.poll(ctx) {
            let id = self.in_flight;
            // `poll` a lancé la demande en attente, s'il y en avait une
            if let Some(next) = self.queued.take() {
                self.in_flight = next;
            }
            // un résultat qui ne porte pas le dernier numéro vient d'une sélection, de réglages ou d'un état
            // périmés : on le jette, une demande plus récente est en route (ou l'état a été vidé)
            if id == self.latest {
                match res {
                    Ok(out) => {
                        self.last = Some(out);
                        self.status.clear();
                    }
                    Err(e) => self.status = error_status(&e),
                }
            }
        }
        let now = (self.tool.options(), self.tool.selected(state), state.revision());
        if self.asked.as_ref() != Some(&now) {
            if now.1.is_empty() {
                // comme Python : pas de calcul sans log, l'ancien résultat disparaît
                self.last = None;
                self.status = NO_LOG.into();
                self.latest += 1; // un calcul encore en cours devient périmé
                self.asked = Some(now);
            } else {
                self.run(ctx, state);
            }
        }
    }
}

impl<T: Tool> Page for ToolPage<T> {
    fn title(&self) -> &'static str {
        T::TITLE
    }

    fn hidden(&self) -> bool {
        T::HIDDEN
    }

    fn busy(&self) -> bool {
        self.runner.busy()
    }

    fn tab(&self) -> &'static str {
        T::TAB.unwrap_or(T::TITLE)
    }

    fn space(&self) -> Option<&'static str> {
        T::SPACE
    }

    fn take_goto(&mut self) -> Option<&'static str> {
        self.tool.take_goto()
    }

    fn take_space_goto(&mut self) -> Option<&'static str> {
        self.space_goto.take()
    }

    fn take_export(&mut self) -> bool {
        std::mem::take(&mut self.export)
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut AppState, theme: &Theme) {
        self.follow_space(state);
        self.update(ui.ctx(), state);
        // une vue d'espace porte en haut les onglets de son espace (D-060) : les quatre vues du bruit et des
        // filtres s'atteignent les unes depuis les autres, sans repasser par le rail
        if T::SPACE.is_some() {
            egui::Panel::top(format!("{}-espace", T::TITLE)).show(ui, |ui| {
                self.space_goto = crate::tools::common::space_tabs(ui, T::TITLE, theme);
            });
        }
        egui::Panel::right(T::TITLE).show(ui, |ui| {
            // le choix du signal, en tête du panneau comme dans le prototype : il vaut pour les quatre vues (D-069)
            let before = self.tool.signal();
            if T::SPACE.is_some() {
                if let Some(s) = crate::tools::common::signal_tabs(ui, theme, before) {
                    self.tool.show_signal(s, state);
                }
                ui.add_space(6.0);
            }
            ui.horizontal_wrapped(|ui| {
                if T::RUN_BUTTON {
                    let calculer = egui::Button::new(RichText::new("Calculer").strong().color(theme.accent("green")));
                    if ui.add_enabled(!self.runner.busy(), calculer).clicked() {
                        self.run_now(ui.ctx(), state);
                    }
                }
                if ui.button("Exporter image").clicked() {
                    self.export = true;
                }
                if T::HAS_VIEW && ui.button("Tout voir").clicked() {
                    self.tool.reset_view();
                }
            });
            ui.separator();
            self.tool.controls(ui, state, theme);
            if T::SPACE.is_some() {
                self.publish_space(state, before);
            }
        });
        egui::Panel::bottom(format!("{}-statut", T::TITLE)).show(ui, |ui| {
            if self.runner.busy() {
                ui.label("Calcul en cours…");
            } else if !self.status.is_empty() {
                ui.colored_label(theme.accent("red"), &self.status);
            } else if let Some(s) = self.tool.status() {
                ui.label(s);
            } else {
                ui.label("");
            }
        });
        egui::CentralPanel::default().show(ui, |ui| match &self.last {
            Some(out) => self.tool.render(ui, state, out, theme),
            None => {
                ui.label("Ouvrez un log pour commencer.");
            }
        });
    }
}
