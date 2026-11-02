//! Fenêtre principale : barre d'outils dont les boutons servent d'onglets (jamais de fenêtre séparée),
//! pages des outils, ouverture des logs (port de `gui/main_window.py`, socle seulement).
//!
//! Le Guide (`guide_page.rs`) s'affiche au démarrage ; ouvrir un premier log depuis son accueil passe aux courbes, comme en Python.

use std::path::PathBuf;

use otb_log::flightlog::{FlightLog, load};

use crate::guide::ToolTarget;
use crate::guide_page::{GuideAction, GuidePage};
use crate::icons::icon;
use crate::state::AppState;
use crate::theme::{self, Theme};
use crate::tool::Page;
use crate::viewer_page::ViewerPage;
use crate::worker::Runner;

/// À appeler une fois au démarrage (et dans chaque banc de test) : chargeurs d'images, pour les icônes SVG, et police de secours.
///
/// Hack, déjà livrée avec egui mais réservée au texte à chasse fixe, passe en dernier recours du texte normal : elle a « → ▲ ● ○ ↑ ↓ ← ◇ ■ »
/// (mesuré avec `fc-query`, 2026-11-02) que Ubuntu-Light n'a pas, sans octet de plus dans l'exécutable (D-039, L-068). Les signes
/// absents des deux (« ✔ ✖ ℹ ⚠ ») restent des icônes (`icons.rs`).
pub fn setup(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
    let mut fonts = egui::FontDefinitions::default();
    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        list.push("Hack".to_string());
    }
    ctx.set_fonts(fonts);
}

/// Écran affiché.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Guide,
    Viewer,
    /// Index dans `App::pages`.
    Tool(usize),
}

pub struct App {
    pub state: AppState,
    pub pages: Vec<Box<dyn Page>>,
    /// Page affichée (`None` : le Guide).
    pub screen: Screen,
    pub viewer: ViewerPage,
    pub guide: GuidePage,
    had_logs: bool,
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
    /// Guide sans fichier de fiches : les bancs de test n'écrivent jamais le vrai `drones.json`.
    pub fn new(pages: Vec<Box<dyn Page>>) -> Self {
        Self::with_guide(pages, GuidePage::in_memory())
    }

    pub fn with_guide(pages: Vec<Box<dyn Page>>, guide: GuidePage) -> Self {
        Self {
            state: AppState::new(),
            pages,
            screen: Screen::Guide,
            viewer: ViewerPage::new(),
            guide,
            had_logs: false,
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
        // glisser-déposer de fichiers sur la fenêtre
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !dropped.is_empty() {
            self.open(dropped);
        }
        self.poll_opening(&ctx);
        // premier log ouvert depuis l'accueil du guide : on passe aux courbes
        if !self.state.logs.is_empty() && !self.had_logs && self.screen == Screen::Guide && self.guide.at_home() {
            self.screen = Screen::Viewer;
        }
        self.had_logs = !self.state.logs.is_empty();
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
                if ui.add(guide.selected(self.screen == Screen::Guide)).clicked() {
                    self.screen = Screen::Guide;
                }
                // le bouton d'un outil reste mis en valeur quand on est sur un outil qui s'ouvre depuis lui
                // (« Courbes » aussi : la Configuration s'ouvre depuis le visualiseur)
                let shown = match self.screen {
                    Screen::Tool(j) => self.pages.get(j).map(|p| p.tab()),
                    _ => None,
                };
                let courbes = self.screen == Screen::Viewer || shown == Some("Courbes");
                if ui.selectable_label(courbes, "Courbes").clicked() {
                    self.screen = Screen::Viewer;
                }
                for (i, p) in self.pages.iter().enumerate().filter(|(_, p)| !p.hidden()) {
                    let on = self.screen == Screen::Tool(i) || shown == Some(p.title());
                    if ui.selectable_label(on, p.title()).clicked() {
                        self.screen = Screen::Tool(i);
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
        let mut goto = None;
        let mut guide_actions = Vec::new();
        egui::CentralPanel::default().show(root, |ui| match self.screen {
            Screen::Viewer => {
                self.viewer.ui(ui, &mut self.state, &theme);
                goto = self.viewer.take_goto();
            }
            Screen::Tool(i) if i < self.pages.len() => {
                self.pages[i].ui(ui, &mut self.state, &theme);
                goto = self.pages[i].take_goto();
            }
            _ => guide_actions = self.guide.ui(ui, &mut self.state, &theme),
        });
        if let Some(title) = goto {
            self.open_tool(title);
        }
        for a in guide_actions {
            match a {
                GuideAction::Tool(ToolTarget::Viewer) => self.screen = Screen::Viewer,
                GuideAction::Tool(ToolTarget::Page(title)) => self.open_tool(title),
                GuideAction::Open(paths) => self.open(paths),
            }
        }
    }

    /// Affiche l'outil de ce titre (`MainWindow.open_tool`) ; sans log ouvert, un message d'état à la place.
    pub fn open_tool(&mut self, title: &str) {
        if self.state.logs.is_empty() {
            // la barre d'état est recomposée à chaque cadre depuis `errors` (effacées à la prochaine ouverture)
            let msg = "Ouvrez d'abord un log Blackbox.".to_string();
            if !self.errors.contains(&msg) {
                self.errors.push(msg);
            }
        } else if let Some(i) = self.pages.iter().position(|p| p.title() == title) {
            self.screen = Screen::Tool(i);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.ui(ui);
    }

    /// Mémoire du Guide entre deux lancements (D-039) : phase, fiche choisie, rôle de chaque log.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.guide.save(storage);
    }
}
