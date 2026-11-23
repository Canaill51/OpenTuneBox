//! Fenêtre principale : barre d'outils dont les boutons servent d'onglets (jamais de fenêtre séparée),
//! pages des outils, ouverture des logs (port de `gui/main_window.py`, socle seulement).
//!
//! Le Guide (`guide_page.rs`) s'affiche au démarrage ; ouvrir un premier log depuis son accueil passe à l'écran
//! d'extraction (les courbes), comme en Python. Cet écran est **hors rail** (D-060) : il a sa propre barre du haut et
//! se referme par « Annuler » ou « Enregistrer le fragment » (lot 29b).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use otb_log::flightlog::FlightLog;
use otb_log::parallel::{MAX_PARALLEL_FILES, load_many};
use otb_store::Library;

use crate::dashboard::{DashAction, DashboardPage};
use crate::export::{crop, default_export_path, today_utc, with_png_extension, write_png};
use crate::guide::ToolTarget;
use crate::guide_page::{GuideAction, GuidePage};
use crate::icons::icon;
use crate::state::AppState;
use crate::theme::{self, Theme};
use crate::tool::Page;
use crate::viewer_page::{ExtAction, FragmentInfo, ViewerPage};
use crate::worker::Runner;

/// Titre de la fenêtre : le nom du programme **suivi de son numéro de version**, pour qu'on sache toujours quelle version on utilise,
/// sous Linux comme sous Windows (demandé par Cédric à l'essai de la 0.9.0 ; `contracts.rs::le_titre_de_la_fenetre_porte_la_version`).
/// Le numéro est celui du paquet (`rust/Cargo.toml`, `version.workspace`), jamais recopié à la main.
///
/// ⚠️ L'`app_id` de la fenêtre (`main.rs`) garde le nom **seul** : c'est le nom du fichier `.desktop`, qui ne change pas d'une version
/// à l'autre (L-076).
pub fn window_title() -> String {
    format!("OpenTuneBox {}", env!("CARGO_PKG_VERSION"))
}

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
    /// Le tableau de bord : les drones et les logs que la bibliothèque connaît (lot 30c).
    Dashboard,
    Guide,
    /// L'écran d'extraction d'un fragment : le visualiseur de courbes, **hors rail** (lot 29b, W-069). Il s'ouvre
    /// en ouvrant un log, par le crayon d'un fragment du tableau de bord ou par le Guide, et se referme vers
    /// l'écran d'où l'on venait (`App::return_to`).
    Viewer,
    /// Index dans `App::pages`.
    Tool(usize),
}

/// Icône et couleur d'accent de l'onglet de la barre du haut, comme `TOOLS` de `gui/main_window.py` (`None` : pas d'onglet à lui,
/// il s'ouvre depuis un autre bouton ; couleur `None` : celle du texte).
pub fn tab_icon(title: &str) -> Option<(&'static str, Option<&'static str>)> {
    match title {
        "Tableau de bord" => Some(("layout-dashboard", None)),
        "Guide" => Some(("book-open", None)),
        "Configuration" => Some(("cpu", None)),
        // l'espace du bruit et des filtres garde l'icône et la couleur de sa première vue (D-060)
        "Bruit et filtres" | "Analyse spectrale" => Some(("audio-waveform", Some("blue"))),
        "Réponse indicielle" => Some(("chart-line", Some("green"))),
        "Curseurs PID" => Some(("sliders-horizontal", Some("pink"))),
        "Simulateur de filtres" => Some(("funnel", Some("teal"))),
        "Assistant" => Some(("lightbulb", Some("orange"))),
        _ => None,
    }
}

/// Mesures du rail, prises dans le prototype qui fait foi (D-060) : largeur, marges du panneau, espace entre
/// deux entrées, et le dessin d'une entrée (coins, marges intérieures, taille du texte).
pub const RAIL_WIDTH: f32 = 212.0;
const RAIL_PAD_X: i8 = 10;
const RAIL_PAD_Y: i8 = 14;
const RAIL_GAP: f32 = 6.0;
const ENTRY_RADIUS: u8 = 6;
const ENTRY_PAD: egui::Vec2 = egui::vec2(10.0, 7.0);
const ENTRY_TEXT: f32 = 13.5;

/// Une entrée du rail, au dessin de la maquette : à plat, le texte à gauche ; choisie, un fond bleu très pâle et
/// le texte en bleu d'accent, en demi-gras.
fn rail_entry<'a>(title: &'a str, on: bool, theme: &Theme) -> egui::Button<'a> {
    let (fond, texte) = if on {
        (theme.ui.select, theme.ui.highlight)
    } else {
        (egui::Color32::TRANSPARENT, theme.ui.text)
    };
    let mut rich = egui::RichText::new(title).size(ENTRY_TEXT).color(texte);
    if on {
        rich = rich.strong();
    }
    let bouton = match tab_icon(title) {
        Some((name, color)) => {
            let tint = color.map_or(texte, |c| theme.accent(c));
            egui::Button::image_and_text(icon(name, 16.0).tint(tint), rich)
        }
        None => egui::Button::new(rich),
    };
    // ⚠️ `selected` ne sert pas qu'à l'œil : il met l'entrée à l'état « coché » dans l'arbre d'accessibilité, ce
    // qu'un lecteur d'écran annonce et ce que les tests lisent. Les couleurs, elles, restent celles de la maquette.
    bouton
        .selected(on)
        .fill(fond)
        .stroke(egui::Stroke::NONE)
        .corner_radius(ENTRY_RADIUS)
}

/// Ce que l'écran d'extraction attend d'une ouverture en cours (`App::edit_after_open`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditTarget {
    /// Le premier log ouvert, quel qu'il soit (« Ouvrir des logs… » du tableau de bord).
    Any,
    /// Le log de ce fragment (crayon d'un fragment dont le log n'est pas ouvert).
    Fragment(String),
}

/// Ordre des entrées du rail après le Guide, celui du prototype (D-060) ; une page absente de la liste vient à la fin.
const RAIL_ORDER: [&str; 5] = [
    "Assistant",
    "Bruit et filtres",
    "Réponse indicielle",
    "Curseurs PID",
    "Configuration",
];

pub struct App {
    pub state: AppState,
    pub pages: Vec<Box<dyn Page>>,
    /// Page affichée (`None` : le Guide).
    pub screen: Screen,
    pub viewer: ViewerPage,
    pub guide: GuidePage,
    pub dash: DashboardPage,
    /// La bibliothèque de données (lot 28). ⚠️ `in_memory` par défaut : comme pour les fiches drone, **aucun banc
    /// de test n'écrit la vraie bibliothèque de l'utilisateur**. Seul `main.rs` ouvre celle du poste.
    pub library: Library,
    /// Écran où revenir en quittant l'écran d'extraction.
    pub return_to: Screen,
    /// Passer à l'écran d'extraction quand ce log aura fini de s'ouvrir.
    pub edit_after_open: Option<EditTarget>,
    had_logs: bool,
    pub theme: Theme,
    applied: Option<&'static str>,
    /// Thème déjà annoncé à la fenêtre du système, et cadres écoulés depuis le lancement (L-091).
    window_theme_sent: Option<&'static str>,
    frames: u32,
    /// Paquet de fichiers en cours d'ouverture : un résultat par fichier, dans l'ordre demandé.
    opening: Runner<Vec<(PathBuf, std::io::Result<Vec<FlightLog>>)>>,
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

/// Nombre de fichiers du prochain paquet : au plus [`MAX_PARALLEL_FILES`] décodés en même temps (D-051), et jamais
/// plus que les places restantes — décoder un fichier coûte ≈ 180 Mo (L-081), autant ne pas le faire pour le jeter.
///
/// Rend au moins 1 quand il reste des fichiers : le dernier sert à afficher le message « N logs au plus ».
pub fn taille_du_paquet(en_attente: usize, deja_ouverts: usize) -> usize {
    let places = crate::state::MAX_LOGS.saturating_sub(deja_ouverts).max(1);
    en_attente.min(MAX_PARALLEL_FILES).min(places)
}

/// Nom du fichier seul, pour un message d'état court (le chemin complet reste dans les erreurs de lecture).
/// Identifiant du fragment d'une session dans la bibliothèque : l'empreinte du fichier et le numéro de session.
pub fn fragment_id(log_id: &str, log: &FlightLog) -> String {
    format!("{log_id}-{}", log.session)
}

fn nom_court(path: &Path) -> String {
    path.file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned())
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
            dash: DashboardPage::new(),
            library: Library::in_memory(),
            return_to: Screen::Dashboard,
            edit_after_open: None,
            had_logs: false,
            theme: theme::CLAIR,
            applied: None,
            window_theme_sent: None,
            frames: 0,
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

    /// L'application telle que `main.rs` la lance : réglages relus, puis fichiers de la ligne de commande ouverts.
    /// `args` est la liste complète des arguments du processus (`std::env::args_os()`) : le premier, le nom du programme, est sauté ici
    /// (testé par `tests/app.rs`, car le branchement de `main.rs` n'est lui-même couvert par aucun test).
    pub fn from_launch(
        pages: Vec<Box<dyn Page>>,
        guide: GuidePage,
        storage: Option<&dyn eframe::Storage>,
        args: impl IntoIterator<Item = std::ffi::OsString>,
    ) -> Self {
        let mut app = Self::with_guide(pages, guide);
        if let Some(storage) = storage {
            app.restore(storage);
        }
        // Logs ouverts par double-clic : directement les courbes.
        app.open_at_launch(files_from_args(args.into_iter().skip(1)));
        app
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

    /// Expéditeur du canal des captures, **pour les tests** : poser une image dedans imite egui-wgpu sans attendre un
    /// vrai rendu, et permet de vérifier l'ordre « relever la capture, puis examiner l'échéance » sans dépendre d'une
    /// horloge (L-095). Même rôle que le champ remplaçable `save_dialog` pour la boîte d'enregistrement.
    pub fn shot_sender(&self) -> Sender<Arc<egui::ColorImage>> {
        self.shot_tx.clone()
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
            self.enter_extraction();
        }
        self.open(paths);
    }

    /// Ouvre des fichiers `.bbl` / `.bfl` en arrière-plan, par paquets de [`MAX_PARALLEL_FILES`] décodés en même
    /// temps (D-051) : un fichier illisible n'empêche pas les autres, et chaque erreur est écrite dans la barre
    /// d'état avec le nom de son fichier.
    pub fn open(&mut self, paths: Vec<PathBuf>) {
        self.errors.clear();
        self.export_status.clear();
        self.queue.extend(paths);
    }

    fn poll_opening(&mut self, ctx: &egui::Context) {
        if let Some(res) = self.opening.poll(ctx) {
            match res {
                Ok(paquet) => {
                    for (path, r) in paquet {
                        if !self.accepte(&path, r) {
                            break; // plus de place : les fichiers restants du paquet et de la file sont abandonnés
                        }
                    }
                }
                // une panique du décodage : sans nom de fichier, elle vaut pour tout le paquet
                Err(e) => self.errors.push(e),
            }
        }
        if !self.opening.busy() && !self.queue.is_empty() {
            let n = taille_du_paquet(self.queue.len(), self.state.logs.len());
            let paquet: Vec<PathBuf> = self.queue.drain(..n).collect();
            self.opening.request(ctx, move || Ok(load_many(&paquet)));
        }
        self.status = self.errors.join(" · ");
    }

    /// Passe à l'écran d'extraction en retenant l'écran d'où l'on vient ; le brouillon repart du log affiché.
    pub fn enter_extraction(&mut self) {
        if self.screen != Screen::Viewer {
            self.return_to = self.screen;
        }
        self.screen = Screen::Viewer;
        self.viewer.draft = None;
    }

    /// Quitte l'écran d'extraction vers l'écran d'où l'on venait (le tableau de bord à défaut).
    fn leave_extraction(&mut self) {
        self.viewer.draft = None;
        self.screen = if self.return_to == Screen::Viewer {
            Screen::Dashboard
        } else {
            self.return_to
        };
    }

    /// Ce que l'écran d'extraction doit savoir du fragment du log affiché : drone, fichier, session, nom et rôle.
    pub fn fragment_info(&self) -> FragmentInfo {
        let Some(ll) = self.state.current_log() else {
            return FragmentInfo::default();
        };
        let chemin = ll.log.path.to_string_lossy().into_owned();
        let role_guide = self.guide.roles.get(&chemin).cloned().unwrap_or_default();
        let file = nom_court(&ll.log.path);
        let stored = ll.fragment.as_ref().and_then(|id| self.library.fragment(id));
        match stored {
            Some(f) => {
                let entry = self.library.log(&f.log_id);
                let drone = entry
                    .and_then(|e| self.library.drone(&e.drone_id))
                    .map_or_else(|| ll.log.settings.craft_name.clone(), |d| d.name.clone());
                let sessions = self.library.fragments().iter().filter(|g| g.log_id == f.log_id).count();
                FragmentInfo {
                    drone,
                    file,
                    session: f.session as usize,
                    sessions,
                    name: f.name.clone(),
                    role: if f.role.is_empty() { role_guide } else { f.role.clone() },
                    stored: true,
                }
            }
            None => FragmentInfo {
                drone: ll.log.settings.craft_name.trim().to_string(),
                file,
                session: ll.log.session,
                sessions: 0,
                name: ll.log.name(),
                role: role_guide,
                stored: false,
            },
        }
    }

    /// « Enregistrer le fragment » : les bornes deviennent la période d'analyse du log (tous les outils la lisent),
    /// et la bibliothèque les garde comme **choisies** ; le rôle est aussi donné au Guide, qui le retient par fichier.
    fn save_fragment(&mut self, index: usize, draft: crate::viewer_page::Draft) {
        let Some(ll) = self.state.logs.get(index) else { return };
        if ll.uid != draft.uid {
            return; // brouillon d'un autre log : jamais appliqué ailleurs (L-072)
        }
        let (log, fragment) = (ll.log.clone(), ll.fragment.clone());
        let chemin = log.path.to_string_lossy().into_owned();
        self.state.set_period(index, draft.period.0, draft.period.1);
        if !draft.role.is_empty() {
            self.guide.roles.insert(chemin, draft.role.clone());
        }
        let nom = if draft.name.trim().is_empty() {
            log.name()
        } else {
            draft.name.trim().to_string()
        };
        let range = log.slice_time(draft.period.0, draft.period.1);
        if let Some(mut f) = fragment.and_then(|id| self.library.fragment(&id).cloned()) {
            f.start = range.start as u64;
            f.end = range.end as u64;
            f.name = nom.clone();
            f.role = draft.role;
            f.chosen = true;
            self.library.put_fragment(f);
            self.errors.push(format!("Fragment « {nom} » enregistré."));
        } else {
            self.errors.push(format!(
                "Limites de « {nom} » appliquées pour cette séance (log rangé sous aucun drone)."
            ));
        }
    }

    /// Range les sessions d'un fichier ouvert (ou son erreur) ; `false` quand il n'y a plus de place pour un log.
    fn accepte(&mut self, path: &Path, r: std::io::Result<Vec<FlightLog>>) -> bool {
        let logs = match r {
            Ok(logs) if logs.is_empty() => {
                self.errors
                    .push(format!("{} : fichier sans session exploitable", nom_court(path)));
                return true;
            }
            Ok(logs) => logs,
            Err(e) => {
                self.errors.push(format!("{} illisible : {e}", path.display()));
                return true;
            }
        };
        let log_id = self.remember(path, &logs);
        for log in logs {
            if let Some(m) = log.settings.implausible() {
                self.errors.push(format!("{} : {m}", log.name()));
            }
            let fid = log_id.as_ref().map(|id| fragment_id(id, &log));
            // bornes choisies dans l'écran d'extraction : elles deviennent la période d'analyse
            let choisi = fid
                .as_ref()
                .and_then(|id| self.library.fragment(id))
                .filter(|f| f.chosen)
                .map(|f| otb_store::link::range_of(f, log.n()));
            if !self.state.add(log) {
                self.errors.push(format!(
                    "{} logs au plus : les suivants sont ignorés.",
                    crate::state::MAX_LOGS
                ));
                self.queue.clear();
                return false;
            }
            let index = self.state.logs.len() - 1;
            self.state.logs[index].fragment = fid.clone();
            match choisi {
                Some(Ok(range)) => {
                    if let Some((a, b)) = crate::state::period_of_range(&self.state.logs[index].log, range) {
                        self.state.set_period(index, a, b);
                    }
                }
                Some(Err(m)) => self.errors.push(m),
                None => {}
            }
            let attendu = match &self.edit_after_open {
                Some(EditTarget::Any) => true,
                Some(EditTarget::Fragment(id)) => fid.as_ref() == Some(id),
                None => false,
            };
            if attendu {
                self.edit_after_open = None;
                self.state.set_current(Some(index));
                self.enter_extraction();
            }
        }
        true
    }

    /// Demande des fichiers au système, puis les ouvre. La barre du haut et le tableau de bord passent tous
    /// deux par ici : une seule porte vers la boîte de dialogue.
    fn ask_files(&mut self) -> bool {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Blackbox", &["bbl", "bfl", "BBL", "BFL", "txt", "TXT"])
            .pick_files()
        {
            self.open(paths);
            return true;
        }
        false
    }

    /// Range un fichier ouvert dans la bibliothèque : **un enregistrement par fichier** (ses sessions deviendront
    /// des fragments, lot 29b), et son drone reconnu par le **nom de machine** de l'en-tête. Tant que la
    /// récupération par USB n'apporte pas l'identifiant de la carte (lot 32), c'est la seule identité disponible :
    /// un log sans nom de machine n'est donc rangé sous aucun drone, plutôt que sous un drone inventé.
    ///
    /// Le fichier est relu pour son empreinte (quelques millisecondes : il vient d'être décodé, donc il est dans
    /// le cache du système). C'est le prix du refus des doublons quand un même vol est ouvert depuis deux dossiers.
    ///
    /// Rend l'empreinte du fichier (identifiant du log dans la bibliothèque), `None` s'il n'est pas rangé.
    fn remember(&mut self, path: &Path, logs: &[FlightLog]) -> Option<String> {
        let first = logs.first()?;
        let nom = first.settings.craft_name.trim().to_string();
        if nom.is_empty() {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        let date = crate::dashboard::file_date(path);
        let mut drone = self
            .library
            .drone(&nom)
            .cloned()
            .unwrap_or_else(|| otb_store::model::Drone::new(&nom, &nom));
        drone.firmware = first.settings.firmware.clone();
        if drone.first_seen.is_empty() {
            drone.first_seen = date.clone();
        }
        drone.last_seen = date.clone();
        self.library.put_drone(drone);
        let id = otb_store::fingerprint(&bytes);
        self.library.add_log(otb_store::model::LogEntry {
            id: id.clone(),
            drone_id: nom,
            name: nom_court(path),
            path: path.display().to_string(),
            copied: false,
            bytes: bytes.len() as u64,
            added: date.clone(),
        });
        // ⚠️ Chaque session devient un **fragment**, nommé comme le prototype le montre (« FINAL », « FINAL.02 ») :
        // sans cela le tableau de bord afficherait « 0 fragment » sur tous les drones. Ses bornes sont les limites
        // automatiques du vol tant qu'on ne les a pas choisies dans l'écran d'extraction (lot 29b) ; un fragment
        // **déjà rangé garde son nom, son rôle et ses bornes choisies** — rouvrir le fichier les remettait à zéro.
        for log in logs {
            let fid = fragment_id(&id, log);
            let p = crate::state::default_period(log);
            let auto = log.slice_time(p.0, p.1);
            match self.library.fragment(&fid).cloned() {
                Some(f) if f.chosen => {}
                Some(mut f) => {
                    f.start = auto.start as u64;
                    f.end = auto.end as u64;
                    self.library.put_fragment(f);
                }
                None => self.library.put_fragment(otb_store::model::Fragment {
                    id: fid,
                    log_id: id.clone(),
                    session: log.session as u64,
                    start: auto.start as u64,
                    end: auto.end as u64,
                    name: log.name(),
                    role: String::new(),
                    date: date.clone(),
                    chosen: false,
                }),
            }
        }
        Some(id)
    }

    /// La barre d'état, en bas : messages d'ouverture, d'export, d'enregistrement.
    fn status_bar(&self, root: &mut egui::Ui) {
        egui::Panel::bottom("etat").show(root, |ui| {
            let busy = if self.opening.busy() || !self.queue.is_empty() {
                "Ouverture en cours…"
            } else {
                ""
            };
            ui.label(status_line(&self.status, &self.export_status, busy));
        });
    }

    /// Le rail de navigation, à gauche (D-060) : une entrée par outil ou par **espace**. Les vues d'un espace
    /// (`HIDDEN`) n'y ont pas d'entrée à elles — c'est celle de leur espace (`TAB`) qui reste mise en valeur, ce
    /// que faisait déjà la barre du haut pour la Fréquence × gaz et la Configuration.
    fn rail(&mut self, root: &mut egui::Ui, theme: &Theme) {
        egui::Panel::left("rail")
            .default_size(RAIL_WIDTH)
            .size_range(RAIL_WIDTH..=RAIL_WIDTH)
            .frame(
                egui::Frame::new()
                    .fill(theme.ui.base)
                    .inner_margin(egui::Margin {
                        left: RAIL_PAD_X,
                        right: RAIL_PAD_X,
                        top: RAIL_PAD_Y,
                        bottom: RAIL_PAD_Y,
                    })
                    .stroke(egui::Stroke::NONE),
            )
            .show(root, |ui| {
                // trait de séparation à droite, comme la maquette (1 pt)
                let decalage = egui::vec2(f32::from(RAIL_PAD_X), 0.0);
                let cadre = ui.max_rect();
                ui.painter().line_segment(
                    [cadre.right_top() + decalage, cadre.right_bottom() + decalage],
                    egui::Stroke::new(1.0, theme.ui.border),
                );
                ui.spacing_mut().item_spacing.y = RAIL_GAP;
                // marges intérieures d'un bouton : celles de la maquette (10 × 7)
                ui.spacing_mut().button_padding = ENTRY_PAD;
                let shown = match self.screen {
                    Screen::Tool(j) => self.pages.get(j).map(|p| p.tab()),
                    _ => None,
                };
                // chaque entrée prend toute la largeur, son texte aligné à gauche comme dans le prototype
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    if ui
                        .add(rail_entry("Tableau de bord", self.screen == Screen::Dashboard, theme))
                        .clicked()
                    {
                        self.screen = Screen::Dashboard;
                    }
                    if ui
                        .add(rail_entry("Guide", self.screen == Screen::Guide, theme))
                        .clicked()
                    {
                        self.screen = Screen::Guide;
                    }
                    // « Courbes » n'a plus d'entrée : c'est l'écran d'extraction, hors rail (W-069, lot 29b)
                    // l'entrée porte le nom de l'espace quand la page en fait partie (D-060)
                    let mut entries: Vec<(usize, &'static str, bool)> = self
                        .pages
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| !p.hidden())
                        .map(|(i, p)| (i, p.entry(), self.screen == Screen::Tool(i) || shown == Some(p.title())))
                        .collect();
                    entries.sort_by_key(|(_, e, _)| RAIL_ORDER.iter().position(|r| r == e).unwrap_or(usize::MAX));
                    for (i, entry, on) in entries {
                        if ui.add(rail_entry(entry, on, theme)).clicked() {
                            self.screen = Screen::Tool(i);
                        }
                    }
                });
            });
    }

    pub fn ui(&mut self, root: &mut egui::Ui) {
        let ctx = root.ctx().clone();
        if self.applied != Some(self.theme.name) {
            theme::apply(&ctx, &self.theme);
            self.applied = Some(self.theme.name);
            ctx.request_repaint(); // le thème ne vaut qu'au cadre suivant
        }
        // Barre de titre et bordures : annoncées nous-mêmes, une fois la fenêtre posée (L-091).
        self.frames = self.frames.saturating_add(1);
        if theme::should_send_window_theme(self.frames, self.window_theme_sent, self.theme.name) {
            ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(theme::window_theme(&self.theme)));
            self.window_theme_sent = Some(self.theme.name);
        } else if self.frames < theme::WINDOW_THEME_DELAY {
            ctx.request_repaint(); // sans cela, une interface au repos n'atteindrait jamais le cadre d'envoi
        }
        // glisser-déposer de fichiers sur la fenêtre
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !dropped.is_empty() {
            self.open(dropped);
        }
        self.poll_opening(&ctx);
        // ⚠️ Relever la capture **avant** d'examiner l'échéance : l'inverse jetait `shot_rect` au cadre où le délai
        // expirait, et `finish_export` ressortait alors sans rien écrire bien que l'image fût là — l'export échouait
        // avec « la capture d'écran n'est pas arrivée » alors qu'elle venait d'arriver (L-095).
        if let Ok(shot) = self.shot_rx.try_recv() {
            self.finish_export(&ctx, &shot);
        }
        if let Some(at) = self.shot_at {
            if at.elapsed() > self.capture_timeout {
                self.shot_at = None;
                self.shot_rect = None;
                self.export_status = "Export impossible : la capture d'écran n'est pas arrivée.".into();
            } else {
                ctx.request_repaint_after(self.capture_timeout); // pour constater l'échéance même sans autre événement
            }
        }
        // premier log ouvert depuis l'accueil du guide : on passe aux courbes
        if !self.state.logs.is_empty() && !self.had_logs && self.screen == Screen::Guide && self.guide.at_home() {
            self.enter_extraction();
        }
        self.had_logs = !self.state.logs.is_empty();
        let theme = self.theme;
        if self.screen == Screen::Viewer {
            // écran hors rail (D-060) : sa propre barre du haut, ni rail ni barre des logs
            self.status_bar(root);
            let info = self.fragment_info();
            for a in self.viewer.ui(root, &mut self.state, &info, &theme) {
                match a {
                    ExtAction::Leave => self.leave_extraction(),
                    ExtAction::Save { index, draft } => {
                        self.save_fragment(index, draft);
                        self.leave_extraction();
                    }
                    ExtAction::ToggleTheme => {
                        self.theme = if theme.dark { theme::CLAIR } else { theme::SOMBRE };
                    }
                }
            }
            return;
        }
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
                if ui.add(open).on_hover_text("Ouvrir des logs Blackbox").clicked() {
                    self.ask_files();
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
        self.rail(root, &theme);
        self.status_bar(root);
        let mut goto = None;
        let mut space_goto = None;
        let mut export_page = false;
        let mut guide_actions = Vec::new();
        // la page d'un outil : sous la barre du haut, au-dessus de la barre d'état, sur toute la largeur de la fenêtre
        // (dans eframe le `Ui` racine couvre déjà la fenêtre ; le banc de test d'egui_kittest l'entoure d'une marge de 8 points)
        // la page va du bord droit du rail au bord droit de la fenêtre (D-060 : le rail n'est pas la page)
        let page_rect = {
            let (free, window) = (root.available_rect_before_wrap(), ctx.content_rect());
            egui::Rect::from_min_max(egui::pos2(free.min.x, free.min.y), egui::pos2(window.max.x, free.max.y))
        };
        let mut dash_actions = Vec::new();
        egui::CentralPanel::default().show(root, |ui| match self.screen {
            Screen::Dashboard => {
                dash_actions = self
                    .dash
                    .ui(ui, &self.library, &self.guide.cards.list, &self.state, &theme);
            }
            Screen::Viewer => {} // dessiné plus haut, hors rail
            Screen::Tool(i) if i < self.pages.len() => {
                self.pages[i].ui(ui, &mut self.state, &theme);
                goto = self.pages[i].take_goto();
                space_goto = self.pages[i].take_space_goto();
                export_page = self.pages[i].take_export();
            }
            _ => guide_actions = self.guide.ui(ui, &mut self.state, &theme),
        });
        // une vue d'espace s'ouvre directement : pas la peine d'avoir un log pour passer du Spectre au « Et si »
        if let Some(title) = space_goto
            && let Some(j) = self.pages.iter().position(|p| p.title() == title)
        {
            self.screen = Screen::Tool(j);
        }
        for a in dash_actions {
            match a {
                DashAction::Guide(nom) => {
                    if let Some(n) = nom {
                        self.guide.select_by_name(&n);
                    }
                    self.screen = Screen::Guide;
                }
                DashAction::OpenDialog => {
                    // la maquette passe à l'écran d'extraction du log qu'on vient d'ouvrir
                    if self.ask_files() {
                        self.edit_after_open = Some(EditTarget::Any);
                    }
                }
                DashAction::Open(path) => self.open(vec![path]),
                DashAction::Edit { fragment, path } => self.edit_fragment(&fragment, path),
            }
        }
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
                GuideAction::Tool(ToolTarget::Viewer) => self.enter_extraction(),
                GuideAction::Tool(ToolTarget::Page(title)) => self.open_tool(title),
                GuideAction::Open(paths) => self.open(paths),
            }
        }
    }

    /// Le crayon d'un fragment : l'écran d'extraction sur son log, ouvert d'abord s'il ne l'est pas encore.
    pub fn edit_fragment(&mut self, fragment: &str, path: PathBuf) {
        if let Some(i) = self
            .state
            .logs
            .iter()
            .position(|l| l.fragment.as_deref() == Some(fragment))
        {
            self.state.set_current(Some(i));
            self.enter_extraction();
        } else {
            self.edit_after_open = Some(EditTarget::Fragment(fragment.to_string()));
            self.open(vec![path]);
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
        // la bibliothèque a ses propres fichiers : une erreur d'écriture se dit dans la barre d'état plutôt que
        // de passer inaperçue (une bibliothèque en mémoire, elle, n'écrit rien et ne peut pas échouer)
        if let Err(e) = self.library.save() {
            self.errors.push(format!("bibliothèque : écriture impossible ({e})"));
        }
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
