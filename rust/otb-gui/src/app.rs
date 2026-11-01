//! Fenêtre principale : barre d'outils dont les boutons servent d'onglets (jamais de fenêtre séparée),
//! pages des outils, ouverture des logs (port de `gui/main_window.py`, socle seulement).
//!
//! Le Guide s'affiche au démarrage ; son contenu vient en 6-5 (le passage automatique aux courbes à
//! l'ouverture d'un log, comme en Python, viendra avec le visualiseur en 6-1).

use std::path::PathBuf;

use otb_log::flightlog::{FlightLog, load};

use crate::icons::icon;
use crate::state::AppState;
use crate::theme::{self, Theme};
use crate::tool::Page;
use crate::worker::Runner;

/// À appeler une fois au démarrage (et dans chaque banc de test) : chargeurs d'images, pour les icônes SVG.
pub fn setup(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
}

pub struct App {
    pub state: AppState,
    pub pages: Vec<Box<dyn Page>>,
    /// Page affichée (`None` : le Guide).
    pub page: Option<usize>,
    pub theme: Theme,
    applied: Option<&'static str>,
    opening: Runner<Vec<FlightLog>>,
    /// Fichiers qui attendent leur tour.
    queue: Vec<PathBuf>,
    errors: Vec<String>,
    /// Message d'état (barre du bas) : erreur d'ouverture, logs refusés.
    pub status: String,
}

impl App {
    pub fn new(pages: Vec<Box<dyn Page>>) -> Self {
        Self {
            state: AppState::new(),
            pages,
            page: None,
            theme: theme::CLAIR,
            applied: None,
            opening: Runner::default(),
            queue: Vec::new(),
            errors: Vec::new(),
            status: String::new(),
        }
    }

    /// Ouvre des fichiers `.bbl` / `.bfl` en arrière-plan, un par un : un fichier illisible n'empêche pas les
    /// suivants, et chaque erreur est écrite dans la barre d'état.
    pub fn open(&mut self, paths: Vec<PathBuf>) {
        self.errors.clear();
        self.queue.extend(paths);
    }

    fn poll_opening(&mut self, ctx: &egui::Context) {
        match self.opening.poll(ctx) {
            Some(Ok(logs)) if logs.is_empty() => self.errors.push("fichier sans session exploitable".into()),
            Some(Ok(logs)) => {
                for log in logs {
                    if !self.state.add(log) {
                        self.errors.push(format!(
                            "{} logs au plus : les suivants sont ignorés.",
                            crate::state::MAX_LOGS
                        ));
                        self.queue.clear();
                        break;
                    }
                }
            }
            Some(Err(e)) => self.errors.push(e),
            None => {}
        }
        if !self.opening.busy() && !self.queue.is_empty() {
            let p = self.queue.remove(0);
            self.opening.request(ctx, move || {
                load(&p).map_err(|e| format!("{} illisible : {e}", p.display()))
            });
        }
        self.status = self.errors.join(" · ");
    }

    pub fn ui(&mut self, root: &mut egui::Ui) {
        let ctx = root.ctx().clone();
        if self.applied != Some(self.theme.name) {
            theme::apply(&ctx, &self.theme);
            self.applied = Some(self.theme.name);
            ctx.request_repaint(); // le thème ne vaut qu'au cadre suivant
        }
        self.poll_opening(&ctx);
        let theme = self.theme;
        egui::Panel::top("barre").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                let open = egui::Button::image_and_text(icon("folder-open", 16.0).tint(theme.ui.text), "Ouvrir…");
                if ui.add(open).clicked()
                    && let Some(paths) = rfd::FileDialog::new()
                        .add_filter("Blackbox", &["bbl", "bfl", "BBL", "BFL", "txt", "TXT"])
                        .pick_files()
                {
                    self.open(paths);
                }
                ui.separator();
                let guide = egui::Button::image_and_text(icon("book-open", 16.0).tint(theme.ui.text), "Guide");
                if ui.add(guide.selected(self.page.is_none())).clicked() {
                    self.page = None;
                }
                for (i, p) in self.pages.iter().enumerate() {
                    if ui.selectable_label(self.page == Some(i), p.title()).clicked() {
                        self.page = Some(i);
                    }
                }
                ui.separator();
                let mut dark = theme.dark;
                if ui.checkbox(&mut dark, "Thème sombre").changed() {
                    self.theme = if dark { theme::SOMBRE } else { theme::CLAIR };
                }
            });
        });
        egui::Panel::bottom("etat").show(root, |ui| {
            let busy = if self.opening.busy() || !self.queue.is_empty() {
                "Ouverture en cours…"
            } else {
                ""
            };
            ui.label(if self.status.is_empty() { busy } else { &self.status });
        });
        egui::CentralPanel::default().show(root, |ui| match self.page.and_then(|i| self.pages.get_mut(i)) {
            Some(page) => page.ui(ui, &mut self.state, &theme),
            None => {
                ui.heading("Guide");
                ui.label("Le guide intégré arrive à la sous-étape 6-5.");
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui(ui);
    }
}
