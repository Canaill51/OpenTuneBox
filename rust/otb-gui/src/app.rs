//! Fenêtre principale : barre d'outils dont les boutons servent d'onglets (jamais de fenêtre séparée),
//! pages des outils, ouverture des logs (port de `gui/main_window.py`, socle seulement).
//!
//! Le Guide (`guide_page.rs`) s'affiche au démarrage ; ouvrir un premier log depuis son accueil passe aux courbes, comme en Python.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

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

/// Fichiers à ouvrir au lancement : les arguments de la ligne de commande sans les options (même règle que la version Python).
/// C'est ce que reçoit l'application par l'association `.bbl` / `.bfl` de Windows (`OpenTuneBox.exe "%1"`) et par l'AppImage (`%F`).
pub fn files_from_args(args: impl IntoIterator<Item = std::ffi::OsString>) -> Vec<PathBuf> {
    args.into_iter()
        .filter(|a| !a.to_string_lossy().starts_with('-'))
        .map(PathBuf::from)
        .collect()
}

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

/// Bouton d'onglet de la barre du haut : icône colorée (si l'onglet en a une) puis le titre.
fn tab_button<'a>(title: &'a str, on: bool, theme: &Theme) -> egui::Button<'a> {
    match tab_icon(title) {
        Some((name, color)) => {
            let tint = color.map_or(theme.ui.text, |c| theme.accent(c));
            egui::Button::image_and_text(icon(name, 16.0).tint(tint), title).selected(on)
        }
        None => egui::Button::new(title).selected(on),
    }
}

/// Icône et couleur d'accent de l'onglet de la barre du haut, comme `TOOLS` de `gui/main_window.py` (`None` : pas d'onglet à lui,
/// il s'ouvre depuis un autre bouton ; couleur `None` : celle du texte).
pub fn tab_icon(title: &str) -> Option<(&'static str, Option<&'static str>)> {
    match title {
        "Courbes" => Some(("activity", None)),
        "Analyse spectrale" => Some(("audio-waveform", Some("blue"))),
        "Réponse indicielle" => Some(("chart-line", Some("green"))),
        "Curseurs PID" => Some(("sliders-horizontal", Some("pink"))),
        "Simulateur de filtres" => Some(("funnel", Some("teal"))),
        "Assistant" => Some(("lightbulb", Some("orange"))),
        _ => None,
    }
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
    /// Zone en attente d'une capture d'écran (`None` : pas d'export en cours) et instant de la demande.
    shot_rect: Option<egui::Rect>,
    shot_at: Option<Instant>,
    /// Délai après lequel une capture non reçue est abandonnée : egui-wgpu jette parfois la demande sans appeler le rappel
    /// (surface de taille nulle, lecture refusée), et l'export resterait bloqué jusqu'à la fermeture de l'application.
    pub capture_timeout: Duration,
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

/// Texte de la barre du bas : erreurs d'ouverture et résultat du dernier export ensemble (aucun ne masque l'autre),
/// sinon « Ouverture en cours… » ou rien.
pub fn status_line(open_status: &str, export_status: &str, busy: &str) -> String {
    let parts: Vec<&str> = [open_status, export_status]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        busy.to_string()
    } else {
        parts.join(" · ")
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
}

impl App {
    /// Guide sans fichier de fiches : les bancs de test n'écrivent jamais le vrai `drones.json`. De même, la boîte
    /// d'enregistrement répond « annulé » sans rien ouvrir : un test qui déclencherait un export par erreur ne doit pas
    /// ouvrir une vraie fenêtre du système (elle bloque le test et se ferme mal). Seul `with_guide` (le lancement réel)
    /// garde la boîte du système.
    pub fn new(pages: Vec<Box<dyn Page>>) -> Self {
        let mut app = Self::with_guide(pages, GuidePage::in_memory());
        app.save_dialog = Box::new(|_| None);
        app
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
            shot_at: None,
            capture_timeout: Duration::from_secs(3),
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
        self.shot_at = Some(Instant::now());
        let (tx, c) = (self.shot_tx.clone(), ctx.clone());
        ctx.request_screenshot(move |img| {
            let _ = tx.send(img);
            c.request_repaint(); // le rappel de capture ne demande pas de cadre
        });
        ctx.request_repaint();
    }

    /// Recadre la capture reçue, demande le fichier, l'écrit. Python n'affiche rien ; ici le résultat va dans la barre du bas (W-038).
    fn finish_export(&mut self, ctx: &egui::Context, shot: &egui::ColorImage) {
        self.shot_at = None;
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
        let path = with_png_extension(chosen.clone());
        // la boîte du système ne confirme l'écrasement que pour le nom tapé : « vol1 » devenu « vol1.png » ne doit pas
        // remplacer sans le dire un fichier existant
        if path != chosen && path.exists() {
            self.export_status = format!("Ce fichier existe déjà : {} (choisissez un autre nom).", path.display());
            return;
        }
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

    /// Fichiers reçus au lancement (double-clic) : ouverts, et l'écran passe aux courbes sans attendre le premier log,
    /// même si le Guide était sur une autre page que l'accueil (`poll_opening` ne bascule que depuis l'accueil).
    pub fn open_at_launch(&mut self, paths: Vec<PathBuf>) {
        if !paths.is_empty() {
            self.screen = Screen::Viewer;
        }
        self.open(paths);
    }

    /// Ouvre des fichiers `.bbl` / `.bfl` en arrière-plan, un par un : un fichier illisible n'empêche pas les
    /// suivants, et chaque erreur est écrite dans la barre d'état.
    pub fn open(&mut self, paths: Vec<PathBuf>) {
        self.errors.clear();
        self.export_status.clear();
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
        if let Some(at) = self.shot_at {
            if at.elapsed() > self.capture_timeout {
                self.shot_at = None;
                self.shot_rect = None;
                self.export_status = "Export impossible : la capture d'écran n'est pas arrivée.".into();
            } else {
                ctx.request_repaint_after(self.capture_timeout); // pour constater l'échéance même sans autre événement
            }
        }
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
                // bouton principal : plein, couleur d'accent (comme `QToolButton#primary` de Python)
                let on_accent = theme::plot::ON_ACCENT;
                let open = egui::Button::image_and_text(
                    icon("folder-open", 16.0).tint(on_accent),
                    egui::RichText::new("Ouvrir…").color(on_accent).strong(),
                )
                .fill(theme.ui.highlight);
                if ui.add(open).on_hover_text("Ouvrir des logs Blackbox").clicked()
                    && let Some(paths) = rfd::FileDialog::new()
                        .add_filter("Blackbox", &["bbl", "bfl", "BBL", "BFL", "txt", "TXT"])
                        .pick_files()
                {
                    self.open(paths);
                }
                // fermer le log affiché / tous les logs (grisés sans log)
                let any = !self.state.logs.is_empty();
                let close = egui::Button::image(icon("x", 16.0).tint(theme.ui.text).alt_text("Fermer le log affiché"));
                if ui
                    .add_enabled(any, close)
                    .on_hover_text("Fermer le log affiché")
                    .clicked()
                    && let Some(current) = self.state.current
                {
                    self.state.remove(current);
                }
                let close_all = egui::Button::image(
                    icon("trash-2", 16.0)
                        .tint(theme.ui.text)
                        .alt_text("Fermer tous les logs"),
                );
                if ui
                    .add_enabled(any, close_all)
                    .on_hover_text("Fermer tous les logs")
                    .clicked()
                {
                    self.state.clear();
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
                if ui.add(tab_button("Courbes", courbes, &theme)).clicked() {
                    self.screen = Screen::Viewer;
                }
                for (i, p) in self.pages.iter().enumerate().filter(|(_, p)| !p.hidden()) {
                    let on = self.screen == Screen::Tool(i) || shown == Some(p.title());
                    if ui.add(tab_button(p.title(), on, &theme)).clicked() {
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
            ui.label(status_line(&self.status, &self.export_status, busy));
        });
        let mut goto = None;
        let mut export_page = false;
        let mut guide_actions = Vec::new();
        // la page d'un outil : sous la barre du haut, au-dessus de la barre d'état, sur toute la largeur de la fenêtre
        // (dans eframe le `Ui` racine couvre déjà la fenêtre ; le banc de test d'egui_kittest l'entoure d'une marge de 8 points)
        let page_rect = {
            let (free, window) = (root.available_rect_before_wrap(), ctx.content_rect());
            egui::Rect::from_min_max(
                egui::pos2(window.min.x, free.min.y),
                egui::pos2(window.max.x, free.max.y),
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
