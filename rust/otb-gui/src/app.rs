//! Fenêtre principale : barre d'outils dont les boutons servent d'onglets (jamais de fenêtre séparée),
//! pages des outils, ouverture des logs (port de `gui/main_window.py`, socle seulement).
//!
//! Le Guide (`guide_page.rs`) s'affiche au démarrage ; ouvrir un premier log depuis son accueil passe aux courbes, comme en Python.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use otb_log::flightlog::{FlightLog, load};

use crate::export::{crop, default_export_path, today_utc, with_png_extension, write_png};
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
    /// Résultat du dernier export (barre du bas, sous les erreurs d'ouverture).
    pub export_status: String,
    /// Dernier dossier d'export, gardé entre deux lancements (`last_dir` de Python).
    pub export_dir: Option<PathBuf>,
    /// Les tests remplacent la boîte du système.
    pub save_dialog: SaveDialog,
    /// Zone en attente d'une capture d'écran (`None` : pas d'export en cours).
    shot_rect: Option<egui::Rect>,
    shot_tx: Sender<Arc<egui::ColorImage>>,
    shot_rx: Receiver<Arc<egui::ColorImage>>,
}

/// Boîte « Exporter l'image » : reçoit le nom proposé, rend le fichier choisi (`None` : annulé).
pub type SaveDialog = Box<dyn FnMut(&Path) -> Option<PathBuf>>;

/// Boîte d'enregistrement du système (`QFileDialog.getSaveFileName`).
fn system_save_dialog(suggested: &Path) -> Option<PathBuf> {
    let mut d = rfd::FileDialog::new()
        .set_title("Exporter l'image")
        .add_filter("Image PNG", &["png"]);
    if let Some(dir) = suggested.parent() {
        d = d.set_directory(dir);
    }
    if let Some(name) = suggested.file_name() {
        d = d.set_file_name(name.to_string_lossy());
    }
    d.save_file()
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

impl App {
    /// Guide sans fichier de fiches : les bancs de test n'écrivent jamais le vrai `drones.json`.
    pub fn new(pages: Vec<Box<dyn Page>>) -> Self {
        Self::with_guide(pages, GuidePage::in_memory())
    }

    pub fn with_guide(pages: Vec<Box<dyn Page>>, guide: GuidePage) -> Self {
        let (shot_tx, shot_rx) = channel();
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
            export_status: String::new(),
            export_dir: None,
            save_dialog: Box::new(system_save_dialog),
            shot_rect: None,
            shot_tx,
            shot_rx,
        }
    }

    /// Demande la capture de `rect` (en points) ; l'image arrive à un cadre suivant (`finish_export`).
    pub fn request_export(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        if self.shot_rect.is_some() {
            return; // une capture est déjà en route
        }
        self.shot_rect = Some(rect);
        let (tx, c) = (self.shot_tx.clone(), ctx.clone());
        ctx.request_screenshot(move |img| {
            let _ = tx.send(img);
            c.request_repaint(); // le rappel de capture ne demande pas de cadre
        });
        ctx.request_repaint();
    }

    /// Recadre la capture reçue, demande le fichier, l'écrit. Python n'affiche rien ; ici le résultat va dans la barre du bas (W-038).
    fn finish_export(&mut self, ctx: &egui::Context, shot: &egui::ColorImage) {
        let Some(rect) = self.shot_rect.take() else {
            return;
        };
        let Some(part) = crop(shot, rect, ctx.pixels_per_point()) else {
            self.export_status = "Export impossible : zone vide.".into();
            return;
        };
        let dir = self.export_dir.clone().unwrap_or_else(home_dir);
        let suggested = default_export_path(&dir, today_utc(), &|p| p.exists());
        let Some(chosen) = (self.save_dialog)(&suggested) else {
            return; // annulé
        };
        let path = with_png_extension(chosen);
        self.export_status = match write_png(&part, &path) {
            Ok(()) => {
                self.export_dir = path.parent().map(Path::to_path_buf);
                format!("Image enregistrée : {}", path.display())
            }
            Err(e) => format!("Export impossible : {} ({e})", path.display()),
        };
    }

    /// Dossier du dernier export, relu au lancement.
    pub fn restore(&mut self, storage: &dyn eframe::Storage) {
        self.export_dir = eframe::get_value::<String>(storage, "export_dir")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from);
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
        if let Ok(shot) = self.shot_rx.try_recv() {
            self.finish_export(&ctx, &shot);
        }
        // premier log ouvert depuis l'accueil du guide : on passe aux courbes
        if !self.state.logs.is_empty() && !self.had_logs && self.screen == Screen::Guide && self.guide.at_home() {
            self.screen = Screen::Viewer;
        }
        self.had_logs = !self.state.logs.is_empty();
        let theme = self.theme;
        let mut export_all = false;
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
                ui.separator();
                let export = egui::Button::image(
                    icon("image-down", 16.0)
                        .tint(theme.ui.text)
                        .alt_text("Exporter la fenêtre en image PNG"),
                );
                if ui
                    .add(export)
                    .on_hover_text("Exporter la fenêtre en image PNG")
                    .clicked()
                {
                    export_all = true;
                }
            });
        });
        egui::Panel::bottom("etat").show(root, |ui| {
            let busy = if self.opening.busy() || !self.queue.is_empty() {
                "Ouverture en cours…"
            } else {
                ""
            };
            let msg = if !self.status.is_empty() {
                &self.status
            } else if !self.export_status.is_empty() {
                &self.export_status
            } else {
                busy
            };
            ui.label(msg);
        });
        let mut goto = None;
        let mut export_page = false;
        let mut guide_actions = Vec::new();
        // la page d'un outil : sous la barre du haut, au-dessus de la barre d'état, sur toute la largeur de la fenêtre
        // (le `Ui` racine laisse 8 points de marge de chaque côté)
        let page_rect = {
            let (libre, fenetre) = (root.available_rect_before_wrap(), ctx.content_rect());
            egui::Rect::from_min_max(
                egui::pos2(fenetre.min.x, libre.min.y),
                egui::pos2(fenetre.max.x, libre.max.y),
            )
        };
        egui::CentralPanel::default().show(root, |ui| match self.screen {
            Screen::Viewer => {
                self.viewer.ui(ui, &mut self.state, &theme);
                goto = self.viewer.take_goto();
            }
            Screen::Tool(i) if i < self.pages.len() => {
                self.pages[i].ui(ui, &mut self.state, &theme);
                goto = self.pages[i].take_goto();
                export_page = self.pages[i].take_export();
            }
            _ => guide_actions = self.guide.ui(ui, &mut self.state, &theme),
        });
        if let Some(title) = goto {
            self.open_tool(title);
        }
        if export_all {
            self.request_export(&ctx, ctx.content_rect());
        } else if export_page {
            // toute la page, marges du panneau comprises (`widget.grab()` de Python)
            self.request_export(&ctx, page_rect);
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
        eframe::set_value(
            storage,
            "export_dir",
            &self
                .export_dir
                .as_ref()
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
    }
}
