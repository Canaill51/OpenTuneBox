//! Un outil = une page de la fenêtre principale (port de `gui/tools/base.py:ToolWindow`).
//!
//! Le compilateur remplace le test AST de `test_gui_tools.py` : `Tool::compute` est une fonction associée
//! sans `self`, elle ne peut donc pas lire les réglages de l'interface. Elle ne reçoit que `Options`
//! (copie des réglages au moment de la demande) et `LogInput`, deux types `Send`.

use std::sync::Arc;

use egui::{Color32, Ui};
use otb_log::flightlog::FlightLog;

use crate::state::{AppState, Cache};
use crate::theme::Theme;
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
    /// Outil ouvert depuis un autre (pas de bouton dans la barre) : `OTHER_TOOLS` de `main_window.py`.
    const HIDDEN: bool = false;
    /// Bouton de la barre qui reste mis en valeur quand cet outil est affiché (par défaut, le sien).
    const TAB: Option<&'static str> = None;

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
    /// Un calcul est en cours (captures et tests : attendre sa fin).
    fn busy(&self) -> bool {
        false
    }
    /// Page à ouvrir ensuite (son titre), demandée pendant le dernier affichage.
    fn take_goto(&mut self) -> Option<&'static str> {
        None
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
}

impl<T: Tool> ToolPage<T> {
    pub fn new(tool: T) -> Self {
        Self {
            tool,
            runner: Runner::default(),
            last: None,
            status: String::new(),
            asked: None,
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
        self.runner.request(ctx, move || T::compute(opts, inputs));
    }

    /// Lit un résultat terminé et relance au besoin si les réglages, la sélection ou les logs ont changé.
    pub fn update(&mut self, ctx: &egui::Context, state: &AppState) {
        if let Some(res) = self.runner.poll(ctx) {
            match res {
                Ok(out) => {
                    self.last = Some(out);
                    self.status.clear();
                }
                Err(e) => self.status = error_status(&e),
            }
        }
        let now = (self.tool.options(), self.tool.selected(state), state.revision());
        if self.asked.as_ref() != Some(&now) {
            if now.1.is_empty() {
                // comme Python : pas de calcul sans log, l'ancien résultat disparaît
                self.last = None;
                self.status = "Sélectionnez au moins un log.".into();
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

    fn take_goto(&mut self) -> Option<&'static str> {
        self.tool.take_goto()
    }

    fn ui(&mut self, ui: &mut Ui, state: &mut AppState, theme: &Theme) {
        self.update(ui.ctx(), state);
        egui::Panel::right(T::TITLE).show(ui, |ui| self.tool.controls(ui, state, theme));
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
