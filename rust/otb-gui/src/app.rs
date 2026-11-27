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

use std::collections::{HashMap, HashSet};

use crate::analysis_bar::{AnalysisBar, BarAction, ChipInfo};
use crate::dashboard::{DashAction, DashboardPage};
use crate::drone_sheet::{DroneSheetPage, SheetAction, SheetForm};
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
    /// La fiche d'un drone (D-068) : ouverte par le crayon d'une carte, « Nouveau drone » ou le Guide, elle se
    /// referme vers l'écran d'où l'on venait (`App::sheet_return`). Le rail reste, « Tableau de bord » choisi.
    DroneSheet,
    /// La récupération des logs par USB (lot 32b, D-074) : ouverte par le tableau de bord, elle y revient. Le rail
    /// reste, « Tableau de bord » choisi.
    Usb,
    /// L'emplacement de la bibliothèque (lot 34, D-081) : ouvert par le tableau de bord, ou seul au lancement si elle
    /// est introuvable ou ouverte ailleurs ; il revient d'où l'on venait (`App::library_return`). Le rail reste,
    /// « Tableau de bord » choisi.
    Library,
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
        "Simulateur de filtres" => Some(("funnel", Some("teal"))),
        "Assistant" => Some(("lightbulb", Some("orange"))),
        _ => None,
    }
}

/// Mesures du rail, prises dans le prototype qui fait foi (D-060) : largeur, marges du panneau, espace entre
/// deux entrées, et le dessin d'une entrée (coins, marges intérieures, taille du texte).
pub const RAIL_WIDTH: f32 = 212.0;
/// Barre du haut (maquette) : hauteur d'une ligne, bloc du logo, son carré bleu, marge verticale des pastilles,
/// espace entre les blocs, et largeur réservée à droite (« Ouvrir… », export, thème).
const TOP_H: f32 = 52.0;
const LOGO_W: f32 = 196.0;
const LOGO_BADGE: f32 = 26.0;
const CHIPS_PAD: i8 = 11;
const TOP_GAP: f32 = 12.0;
const TOP_RIGHT_W: f32 = 190.0;
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
const RAIL_ORDER: [&str; 4] = ["Assistant", "Bruit et filtres", "Réponse indicielle", "Configuration"];

pub struct App {
    pub state: AppState,
    pub pages: Vec<Box<dyn Page>>,
    /// Page affichée (`None` : le Guide).
    pub screen: Screen,
    pub viewer: ViewerPage,
    pub guide: GuidePage,
    pub dash: DashboardPage,
    /// L'écran de la fiche d'un drone, et l'écran où revenir en le quittant.
    pub sheet: DroneSheetPage,
    /// L'écran de la récupération par USB ; ses ports se remplacent dans les tests par une carte simulée.
    pub usb: crate::usb_page::UsbPage,
    pub sheet_return: Screen,
    /// L'écran de l'emplacement de la bibliothèque, et l'écran où revenir en le quittant.
    pub libpage: crate::library_page::LibraryPage,
    pub library_return: Screen,
    /// Mesures suivies dans le temps : lecture de leur table et calcul en arrière-plan (lot 31a).
    pub tracking: crate::tracking::Tracking,
    /// Version de la table des drones déjà donnée au Guide (`Library::drones_revision`) ; `None` : jamais.
    cards_seen: Option<u64>,
    /// La barre de fragments du haut (liste d'analyse, D-066).
    pub bar: AnalysisBar,
    /// Logs ouverts qu'aucun drone ne reconnaît : la fenêtre qui demande leur fiche (D-084).
    pub attach: crate::attach::AttachDialog,
    /// Fragments « enregistrés » sur un log rangé sous aucun drone, par `LoadedLog::uid` : leurs bornes, nom et rôle
    /// valent pour la séance et passent à la bibliothèque si le log est rattaché à un drone (D-084).
    unsaved: HashMap<u64, crate::viewer_page::Draft>,
    /// Fichiers dont on n'ouvre que certaines sessions (le « + » d'un fragment, la liste relue au lancement) :
    /// identifiants des fragments voulus. Un fichier absent d'ici s'ouvre en entier.
    wanted: HashMap<PathBuf, HashSet<String>>,
    /// Fichiers ouverts par « Ouvrir les logs des filtres… » (ou de la série) du Guide : le rôle à donner aux
    /// fragments de leurs sessions, consommé quand le fichier est rangé.
    open_roles: HashMap<PathBuf, &'static str>,
    /// Liste d'analyse relue au lancement, en attente que ses logs soient ouverts : tant qu'elle est là, la
    /// bibliothèque n'est pas récrite depuis l'écran (elle serait vidée avant même d'être rouverte).
    restoring: Option<otb_store::model::Analysis>,
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

/// L'heure de début d'un log, lue dans son en-tête (« 2026-11-13T18:31:47.000+00:00 » → « 18:31:47 ») ; vide sans
/// horloge (carte sans RTC : l'en-tête porte l'an 0000 ou 1970).
pub fn start_time(raw: Option<&str>) -> String {
    let Some(v) = raw else { return String::new() };
    if v.starts_with("0000") || v.starts_with("1970") {
        return String::new();
    }
    v.split_once('T')
        .map(|(_, t)| t.chars().take(8).collect())
        .unwrap_or_default()
}

/// Indice d'une couleur de fichier dans la palette (0 si elle n'y est pas).
fn colour_index(c: egui::Color32) -> usize {
    theme::FILE_COLORS.iter().position(|f| *f == c).unwrap_or(0)
}

/// Identifiant du fragment **de la session** dans la bibliothèque : l'empreinte du fichier et le numéro de session.
/// Une session peut porter d'autres fragments, ajoutés dans l'écran d'extraction (D-067).
pub fn fragment_id(log_id: &str, log: &FlightLog) -> String {
    otb_store::link::session_fragment_id(log_id, log.session as u64)
}

/// Nom du fichier seul, pour un message d'état court (le chemin complet reste dans les erreurs de lecture).
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
        let mut app = Self::with_guide(pages, GuidePage::new());
        app.save_dialog = Box::new(|_| None);
        // ni disques du poste, ni boîte du système dans les bancs de test (L-075)
        app.usb.volumes = std::sync::Arc::new(Vec::new);
        app.usb.pick_folder = Box::new(|| None);
        // ni pointeur du poste, ni boîte du système, ni gestionnaire de fichiers (L-075)
        app.libpage = crate::library_page::LibraryPage::default();
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
            sheet: DroneSheetPage::default(),
            usb: crate::usb_page::UsbPage::system(),
            sheet_return: Screen::Dashboard,
            libpage: crate::library_page::LibraryPage::system(),
            library_return: Screen::Dashboard,
            tracking: Default::default(),
            cards_seen: None,
            bar: AnalysisBar::new(),
            attach: Default::default(),
            unsaved: HashMap::new(),
            wanted: HashMap::new(),
            open_roles: HashMap::new(),
            restoring: None,
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

    /// Dossier du dernier export et thème choisi, relus au lancement. Un nom de thème absent ou inconnu
    /// laisse le thème clair (`theme::by_name`).
    pub fn restore(&mut self, storage: &dyn eframe::Storage) {
        self.export_dir = eframe::get_value::<String>(storage, "export_dir")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from);
        if let Some(name) = eframe::get_value::<String>(storage, "theme") {
            self.theme = theme::by_name(&name);
        }
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
        self.open_as(paths, None);
    }

    /// Comme `open`, et avec un rôle (« filtres », « série PID ») : les fragments des sessions de ces fichiers le
    /// prennent, à la place de celui qu'ils avaient (le pilote vient de le dire en choisissant ces fichiers).
    pub fn open_as(&mut self, paths: Vec<PathBuf>, role: Option<&'static str>) {
        if let Some(role) = role {
            self.open_roles.extend(paths.iter().map(|p| (p.clone(), role)));
        }
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
        let file = nom_court(&ll.log.path);
        let stored = ll.fragment.as_ref().and_then(|id| self.library.fragment(id));
        match stored {
            Some(f) => {
                let entry = self.library.log(&f.log_id);
                let drone = entry
                    .and_then(|e| self.library.drone(&e.drone_id))
                    .map_or_else(|| ll.log.settings.craft_name.clone(), |d| d.name.clone());
                // des sessions, pas des fragments : une session peut en porter plusieurs (D-067)
                let sessions = self
                    .library
                    .fragments()
                    .iter()
                    .filter(|g| g.log_id == f.log_id)
                    .map(|g| g.session)
                    .collect::<HashSet<_>>()
                    .len();
                FragmentInfo {
                    drone,
                    file,
                    session: f.session as usize,
                    sessions,
                    name: f.name.clone(),
                    role: f.role.clone(),
                    stored: true,
                    added: otb_store::link::is_added(f),
                }
            }
            None => FragmentInfo {
                drone: ll.log.settings.craft_name.trim().to_string(),
                file,
                session: ll.log.session,
                sessions: 0,
                name: ll.log.name(),
                role: String::new(),
                stored: false,
                added: false,
            },
        }
    }

    /// « Enregistrer le fragment » : les bornes deviennent la période d'analyse du log (tous les outils la lisent),
    /// et la bibliothèque les garde comme **choisies**, avec son rôle, que le Guide lit sur le fragment (W-074).
    fn save_fragment(&mut self, index: usize, draft: crate::viewer_page::Draft) {
        let Some(ll) = self.state.logs.get(index) else { return };
        if ll.uid != draft.uid {
            return; // brouillon d'un autre log : jamais appliqué ailleurs (L-072)
        }
        let (log, fragment) = (ll.log.clone(), ll.fragment.clone());
        self.state.set_period(index, draft.period.0, draft.period.1);
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
            // log rangé sous aucun drone : le fragment attend son rattachement (D-084), que la fenêtre demande aussitôt
            self.unsaved.insert(draft.uid, draft);
            self.ask_attach(index);
            self.errors.push(format!(
                "Limites de « {nom} » appliquées pour cette séance : rattachez le log à un drone pour les garder."
            ));
        }
    }

    /// Range les sessions d'un fichier ouvert (ou son erreur) ; `false` quand il n'y a plus de place pour un log.
    fn accepte(&mut self, path: &Path, r: std::io::Result<Vec<FlightLog>>) -> bool {
        // le « + » d'un fragment ou la liste relue au lancement : seulement ces sessions-là, et pas deux fois
        let voulus = self.wanted.remove(path);
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
        let role_choisi = self.open_roles.remove(path);
        let log_id = self.remember(path, &logs, role_choisi);
        for log in logs {
            // ⚠️ Une session peut porter **plusieurs** fragments (D-067) : « Ouvrir… » ne met en analyse que celui de
            // la session ; le « + » d'un fragment ajouté, la liste relue au lancement ou son crayon le demandent par
            // son identifiant, qui n'est pas celui de la session — d'où la recherche dans la bibliothèque.
            let fids: Vec<Option<String>> = match (&voulus, &log_id) {
                (None, _) => vec![log_id.as_ref().map(|id| fragment_id(id, &log))],
                (Some(_), None) => Vec::new(),
                (Some(v), Some(id)) => self
                    .library
                    .fragments()
                    .iter()
                    .filter(|f| f.log_id == *id && f.session == log.session as u64 && v.contains(&f.id))
                    .filter(|f| !self.state.logs.iter().any(|l| l.fragment.as_ref() == Some(&f.id)))
                    .map(|f| Some(f.id.clone()))
                    .collect(),
            };
            if fids.is_empty() {
                continue;
            }
            if let Some(m) = log.settings.implausible() {
                self.errors.push(format!("{} : {m}", log.name()));
            }
            let log = Arc::new(log);
            for fid in fids {
                if let Some(id) = &fid {
                    self.give_role(path, id, role_choisi);
                }
                let Some(index) = self.place(log.clone(), fid.clone()) else {
                    self.errors.push(format!(
                        "{} logs au plus : les suivants sont ignorés.",
                        crate::state::MAX_LOGS
                    ));
                    self.queue.clear();
                    self.wanted.clear();
                    return false;
                };
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
        }
        true
    }

    /// Le rôle d'un fragment qui entre dans la liste d'analyse : celui que le pilote vient de choisir en ouvrant
    /// son fichier, sinon l'**ancien** rôle de son fichier (W-074 : le Guide les rangeait par chemin) quand le
    /// fragment n'en a pas encore. Un rôle déjà porté par le fragment n'est jamais remplacé par un ancien.
    fn give_role(&mut self, path: &Path, id: &str, choisi: Option<&'static str>) {
        let Some(mut f) = self.library.fragment(id).cloned() else {
            return;
        };
        let ancien = || {
            self.guide
                .old_roles
                .get(&*path.to_string_lossy())
                .filter(|_| f.role.is_empty())
                .cloned()
        };
        let Some(role) = choisi.map(str::to_string).or_else(ancien) else {
            return;
        };
        if f.role != role {
            f.role = role;
            self.library.put_fragment(f);
        }
    }

    /// Pose une pastille pour le fragment `fid` d'un log décodé (`None` : log rangé sous aucun drone) et rend son
    /// rang ; `None` quand la liste d'analyse est pleine. Ses bornes choisies deviennent sa période d'analyse.
    fn place(&mut self, log: Arc<FlightLog>, fid: Option<String>) -> Option<usize> {
        let choisi = fid
            .as_ref()
            .and_then(|id| self.library.fragment(id))
            .filter(|f| f.chosen)
            .map(|f| otb_store::link::range_of(f, log.n()));
        if !self.state.add_shared(log) {
            return None;
        }
        let index = self.state.logs.len() - 1;
        self.state.logs[index].fragment = fid.clone();
        // relu au lancement : la couleur et l'état de sa pastille, si la couleur est encore libre
        if let Some(item) = self
            .restoring
            .as_ref()
            .and_then(|a| a.items.iter().find(|it| Some(&it.fragment_id) == fid.as_ref()))
        {
            let c = theme::file_color(item.colour as usize);
            if self.state.logs.iter().all(|l| l.color != c) {
                self.state.logs[index].color = c;
            }
            let active = item.active;
            self.state.set_active(index, active);
        }
        match choisi {
            Some(Ok(range)) => {
                if let Some((a, b)) = crate::state::period_of_range(&self.state.logs[index].log, range) {
                    self.state.set_period(index, a, b);
                }
            }
            Some(Err(m)) => self.errors.push(m),
            None => {}
        }
        Some(index)
    }

    /// Le log déjà décodé de la session de `fragment`, quand elle est ouverte sous un autre fragment : un second
    /// fragment s'y pose **sans relire le fichier** (D-067).
    fn loaded_session(&self, fragment: &str) -> Option<Arc<FlightLog>> {
        let f = self.library.fragment(fragment)?;
        let chemin = PathBuf::from(&self.library.log(&f.log_id)?.path);
        self.state
            .logs
            .iter()
            .find(|l| l.log.session as u64 == f.session && l.log.path == chemin)
            .map(|l| l.log.clone())
    }

    /// Pose la pastille de `fragment` sur sa session déjà ouverte ; `None` (dit dans la barre d'état) si la liste
    /// d'analyse est pleine.
    fn place_shared(&mut self, log: Arc<FlightLog>, fragment: &str) -> Option<usize> {
        let index = self.place(log, Some(fragment.to_string()));
        if index.is_none() {
            self.errors.push(format!(
                "{} logs au plus dans la liste d'analyse.",
                crate::state::MAX_LOGS
            ));
        }
        index
    }

    /// « Enregistrer comme nouveau fragment » (D-067) : le fragment du log `index` ne change pas ; un autre est rangé
    /// dans la même session avec les bornes, le nom et le rôle du brouillon, puis posé en pastille juste après
    /// lui (même échantillons, aucune relecture du fichier) et devient le fragment maître.
    fn save_new_fragment(&mut self, index: usize, draft: crate::viewer_page::Draft) {
        let Some(ll) = self.state.logs.get(index) else { return };
        if ll.uid != draft.uid {
            return; // brouillon d'un autre log : jamais appliqué ailleurs (L-072)
        }
        let log = ll.log.clone();
        let Some(origine) = ll.fragment.as_ref().and_then(|id| self.library.fragment(id)).cloned() else {
            self.errors.push(
                "Nouveau fragment impossible : ce log n'est rangé sous aucun drone (pas de nom de machine).".into(),
            );
            return;
        };
        let freres: Vec<&otb_store::model::Fragment> = self
            .library
            .fragments()
            .iter()
            .filter(|f| f.log_id == origine.log_id)
            .collect();
        let id = otb_store::link::added_fragment_id(&origine.log_id, origine.session, freres.iter().copied());
        // un nom distinct de ceux du même fichier : deux pastilles « FINAL.02 » ne se distingueraient pas
        let base = match draft.name.trim() {
            "" => origine.name.clone(),
            n => n.to_string(),
        };
        let pris = |n: &str| freres.iter().any(|f| f.name == n);
        let nom = if pris(&base) {
            (2..)
                .map(|k| format!("{base} ({k})"))
                .find(|n| !pris(n))
                .expect("une suite infinie a toujours un nom libre")
        } else {
            base
        };
        let range = log.slice_time(draft.period.0, draft.period.1);
        self.library.put_fragment(otb_store::model::Fragment {
            id: id.clone(),
            log_id: origine.log_id.clone(),
            session: origine.session,
            start: range.start as u64,
            end: range.end as u64,
            name: nom.clone(),
            role: draft.role,
            date: origine.date.clone(),
            chosen: true,
        });
        match self.place(log, Some(id)) {
            Some(nouveau) => {
                self.state.move_log(nouveau, index + 1);
                self.errors.push(format!("Nouveau fragment « {nom} » enregistré."));
            }
            None => self.errors.push(format!(
                "Nouveau fragment « {nom} » enregistré ; liste d'analyse pleine ({} au plus) : ajoutez-le par son \
                 rond « + » au tableau de bord.",
                crate::state::MAX_LOGS
            )),
        }
    }

    /// Le Guide reçoit les fiches de la bibliothèque **quand la table des drones a changé** (D-068) : c'est elle qui
    /// les tient, le Guide n'en a qu'une copie. Une comparaison de compteur par image, rien de plus.
    pub fn sync_cards(&mut self) {
        let rev = self.library.drones_revision();
        if self.cards_seen != Some(rev) {
            self.guide.refresh(crate::drone_sheet::cards_of(&self.library));
            self.cards_seen = Some(rev);
        }
    }

    /// Range une fiche du Guide dans la bibliothèque (drone créé s'il n'existe pas) et la donne au Guide : pour
    /// les bancs de test, qui partent d'une fiche toute faite.
    pub fn put_card(&mut self, card: otb_advisor::drone_card::DroneCard) {
        let mut d = self
            .library
            .drone_by_name(&card.name)
            .cloned()
            .unwrap_or_else(|| otb_store::model::Drone::new(&card.name, &card.name));
        d.sheet = Some(crate::drone_sheet::sheet_of_card(&card));
        self.library.put_drone(d);
        self.sync_cards();
    }

    /// Ouvre l'écran de la fiche ; on reviendra à l'écran d'où l'on vient.
    pub fn open_sheet(&mut self, form: SheetForm) {
        if self.screen != Screen::DroneSheet {
            self.sheet_return = self.screen;
        }
        self.sheet.form = Some(form);
        self.screen = Screen::DroneSheet;
    }

    fn close_sheet(&mut self) {
        self.sheet.form = None;
        self.screen = match self.sheet_return {
            Screen::DroneSheet => Screen::Dashboard,
            s => s,
        };
    }

    /// Le relevé des cases vérifiées par les logs, rangé sur la fiche de ce nom pour la carte du tableau de bord
    /// (D-071). Écrit seulement s'il change : chaque écriture fait relire les fiches au Guide.
    fn set_verified(&mut self, name: &str, keys: Vec<String>) {
        let Some(mut d) = self.library.drone_by_name(name).cloned() else {
            return;
        };
        let Some(sheet) = d.sheet.as_mut() else { return };
        if sheet.verified != keys {
            sheet.verified = keys;
            self.library.put_drone(d);
        }
    }

    /// Les cases cochées de la fiche de ce nom, changées par `f` dans la bibliothèque.
    fn edit_cases(&mut self, name: &str, f: impl FnOnce(&mut Vec<String>)) {
        let Some(mut d) = self.library.drone_by_name(name).cloned() else {
            return;
        };
        let Some(sheet) = d.sheet.as_mut() else { return };
        f(&mut sheet.done);
        self.library.put_drone(d);
    }

    pub fn apply_sheet(&mut self, action: SheetAction) {
        match action {
            SheetAction::Leave => self.close_sheet(),
            SheetAction::Save => {
                let Some(form) = self.sheet.form.as_mut() else { return };
                match crate::drone_sheet::save_form(&mut self.library, form) {
                    Ok(id) => {
                        let name = self.library.drone(&id).map(|d| d.name.clone()).unwrap_or_default();
                        self.dash.selected = Some(id);
                        self.sync_cards();
                        self.guide.select_by_name(&name);
                        self.close_sheet();
                    }
                    Err(e) => form.error = e,
                }
            }
            SheetAction::Delete(id, copies) => {
                let name = self.library.drone(&id).map(|d| d.name.clone()).unwrap_or_default();
                // W-075 : les fichiers copiés ne partent que si la case a été cochée, et avant les enregistrements
                // qui les désignent
                if copies {
                    let (n, refus) = self.library.delete_copies_of_drone(&id);
                    if n > 0 {
                        self.errors
                            .push(format!("{n} fichier(s) copié(s) effacé(s) du disque."));
                    }
                    self.errors.extend(refus);
                }
                // ses fragments quittent aussi l'écran : rien ne doit montrer un fragment absent de la bibliothèque
                for f in self.library.remove_drone(&id) {
                    while let Some(i) = self.state.logs.iter().position(|l| l.fragment.as_ref() == Some(&f)) {
                        self.state.remove(i);
                    }
                }
                if self.dash.selected.as_deref() == Some(id.as_str()) {
                    self.dash.selected = None;
                }
                self.errors.push(format!("Drone « {name} » supprimé."));
                self.sheet.form = None;
                self.screen = Screen::Dashboard;
            }
        }
    }

    /// « Supprimer définitivement » (D-067) : le fragment **ajouté** du log `index` quitte la bibliothèque (avec ses
    /// mesures) et toutes ses pastilles. ⚠️ Jamais le fragment d'une session : il reviendrait à la prochaine
    /// ouverture du fichier, l'écran ne le propose pas et ce garde-fou le refuse.
    fn delete_fragment(&mut self, index: usize) {
        let Some(id) = self.state.logs.get(index).and_then(|l| l.fragment.clone()) else {
            return;
        };
        let Some(f) = self.library.fragment(&id).cloned() else {
            return;
        };
        if !otb_store::link::is_added(&f) {
            return;
        }
        self.library.remove_fragment(&id);
        while let Some(i) = self.state.logs.iter().position(|l| l.fragment.as_ref() == Some(&id)) {
            self.state.remove(i);
        }
        self.errors.push(format!("Fragment « {} » supprimé.", f.name));
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
    /// des fragments, lot 29b), et son drone reconnu par l'**empreinte** d'un log déjà rangé, sinon par le **nom de
    /// machine** de l'en-tête (D-068). Quand aucun drone ne le reconnaît, l'application **demande** sa fiche (D-084) :
    /// le fichier attend dans [`crate::attach::AttachDialog`], ouvert mais rangé sous aucun drone, jusqu'au choix.
    /// Seule exception : un nom de machine dans une bibliothèque sans aucun drone — il n'y a rien d'autre à proposer
    /// qu'un nouveau drone de ce nom, créé sans question comme avant.
    ///
    /// Le fichier est relu pour son empreinte (quelques millisecondes : il vient d'être décodé, donc il est dans
    /// le cache du système). C'est le prix du refus des doublons quand un même vol est ouvert depuis deux dossiers,
    /// et ce qui reconnaît un log **sans nom** déjà rattaché à la main.
    ///
    /// Rend l'empreinte du fichier (identifiant du log dans la bibliothèque), `None` s'il n'est pas rangé.
    fn remember(&mut self, path: &Path, logs: &[FlightLog], role: Option<&'static str>) -> Option<String> {
        let first = logs.first()?;
        let nom = first.settings.craft_name.trim().to_string();
        let bytes = std::fs::read(path).ok()?;
        let id = otb_store::fingerprint(&bytes);
        // ⚠️ un log déjà rangé garde son drone : une copie par USB a été rangée sous le drone choisi au branchement
        // (D-074), que son nom de machine ne désigne pas forcément ; un log sans nom, sous celui choisi à la main (D-084)
        let known = self
            .library
            .log(&id)
            .and_then(|l| self.library.drone(&l.drone_id))
            .cloned();
        // sinon le nom relie un log à son drone (D-068) : un drone créé à la main garde l'identifiant de sa création
        let by_name = || {
            (!nom.is_empty())
                .then(|| self.library.drone_by_name(&nom).or_else(|| self.library.drone(&nom)))
                .flatten()
                .cloned()
        };
        let drone = match known.or_else(by_name) {
            Some(d) => d,
            None if !crate::attach::must_ask(&self.library, &nom) => otb_store::model::Drone::new(&nom, &nom),
            None => {
                let choice = crate::attach::proposal(&self.library, &nom, self.guide_card().as_deref());
                self.attach.ask(crate::attach::Pending {
                    path: path.to_path_buf(),
                    craft: nom,
                    choice,
                    role,
                    error: None,
                });
                return None;
            }
        };
        let sessions: Vec<&FlightLog> = logs.iter().collect();
        Some(self.store_log(path, &sessions, drone, &bytes))
    }

    /// Le nom de la fiche choisie dans le Guide, proposée d'office pour un log sans nom de machine (D-084).
    fn guide_card(&self) -> Option<String> {
        self.guide
            .selected
            .and_then(|i| self.guide.cards.list.get(i))
            .map(|c| c.name.clone())
    }

    /// Range le fichier `path` (contenu `bytes`, sessions `logs`) sous `drone`, créé ou mis à jour : l'en-tête du
    /// premier log renseigne la fiche, chaque session devient un fragment. Rend l'empreinte du fichier.
    fn store_log(
        &mut self,
        path: &Path,
        logs: &[&FlightLog],
        mut drone: otb_store::model::Drone,
        bytes: &[u8],
    ) -> String {
        let first = logs[0];
        let date = crate::dashboard::file_date(path);
        let id = otb_store::fingerprint(bytes);
        let drone_id = drone.id.clone();
        drone.firmware = first.settings.firmware.clone();
        // pôles et boucle PID de l'en-tête (D-077) : la fiche suit la boucle PID, compare les pôles
        crate::drone_sheet::take_log(
            &mut drone,
            crate::drone_sheet::machine_of_log(&first.settings, first.vbat.as_deref(), &date),
        );
        if drone.first_seen.is_empty() {
            drone.first_seen = date.clone();
        }
        drone.last_seen = date.clone();
        self.library.put_drone(drone);
        self.library.add_log(otb_store::model::LogEntry {
            id: id.clone(),
            drone_id,
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
        id
    }

    /// « Rattacher » de la fenêtre (D-084) : le fichier est rangé sous le drone choisi (créé s'il est nouveau) et ses
    /// sessions ouvertes reçoivent leur fragment. Un fragment déjà « enregistré » pendant la séance (bornes, nom,
    /// rôle, gardés dans `unsaved`) devient celui de la bibliothèque, comme choisi. Un refus (nom vide ou déjà pris,
    /// fichier disparu) est rendu : la ligne reste dans la fenêtre.
    pub fn attach_log(&mut self, path: &Path, choice: &crate::usb::DroneChoice) -> Result<String, String> {
        let mut sessions: Vec<Arc<FlightLog>> = Vec::new();
        for l in &self.state.logs {
            if l.log.path == path && !sessions.iter().any(|s| s.session == l.log.session) {
                sessions.push(l.log.clone());
            }
        }
        if sessions.is_empty() {
            return Err("Ce log n'est plus ouvert : rouvrez-le pour le rattacher.".into());
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{} illisible : {e}", nom_court(path)))?;
        let date = crate::dashboard::file_date(path);
        let drone = crate::usb::resolve_choice(&self.library, choice, &date)?;
        let drone_name = drone.name.clone();
        let refs: Vec<&FlightLog> = sessions.iter().map(|a| a.as_ref()).collect();
        let id = self.store_log(path, &refs, drone, &bytes);
        let role = self.attach.pending.iter().find(|p| p.path == path).and_then(|p| p.role);
        for i in 0..self.state.logs.len() {
            let ll = &self.state.logs[i];
            if ll.log.path != path || ll.fragment.is_some() {
                continue;
            }
            let (uid, log) = (ll.uid, ll.log.clone());
            let fid = fragment_id(&id, &log);
            if let Some(d) = self.unsaved.remove(&uid)
                && let Some(mut f) = self.library.fragment(&fid).cloned()
            {
                let range = log.slice_time(d.period.0, d.period.1);
                f.start = range.start as u64;
                f.end = range.end as u64;
                if !d.name.trim().is_empty() {
                    f.name = d.name.trim().to_string();
                }
                f.role = d.role;
                f.chosen = true;
                self.library.put_fragment(f);
            }
            self.state.logs[i].fragment = Some(fid.clone());
            self.give_role(path, &fid, role);
        }
        self.attach.forget(path);
        self.errors
            .push(format!("« {} » rattaché à « {drone_name} ».", nom_court(path)));
        Ok(id)
    }

    /// Applique les choix de la fenêtre « Rattacher à un drone ».
    fn apply_attach(&mut self, actions: Vec<crate::attach::AttachAction>) {
        for a in actions {
            match a {
                crate::attach::AttachAction::Attach(path, choice) => {
                    if let Err(e) = self.attach_log(&path, &choice)
                        && let Some(p) = self.attach.pending.iter_mut().find(|p| p.path == path)
                    {
                        p.error = Some(e);
                    }
                }
                crate::attach::AttachAction::Skip(path) => self.attach.forget(&path),
            }
        }
    }

    /// Rouvre la fenêtre pour le log `index`, s'il n'est rangé sous aucun drone (« Rattacher à un drone… » de l'écran
    /// d'extraction, ou « Enregistrer le fragment » sur un tel log).
    pub fn ask_attach(&mut self, index: usize) {
        let Some(ll) = self.state.logs.get(index) else { return };
        if ll.fragment.is_some() {
            return;
        }
        let path = ll.log.path.clone();
        let craft = ll.log.settings.craft_name.trim().to_string();
        let choice = crate::attach::proposal(&self.library, &craft, self.guide_card().as_deref());
        self.attach.ask(crate::attach::Pending {
            path,
            craft,
            choice,
            role: None,
            error: None,
        });
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
                        .add(rail_entry(
                            "Tableau de bord",
                            matches!(
                                self.screen,
                                Screen::Dashboard | Screen::DroneSheet | Screen::Usb | Screen::Library
                            ),
                            theme,
                        ))
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
        self.tracking.poll(&mut self.library);
        self.sync_analysis();
        self.sync_cards();
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
        // la fenêtre « Rattacher à un drone » (D-084), au-dessus de tous les écrans ; un fichier fermé entre-temps en sort
        let logs = &self.state.logs;
        self.attach
            .pending
            .retain(|p| logs.iter().any(|l| l.log.path == p.path));
        let choix = self.attach.ui(&ctx, &self.library, &theme);
        self.apply_attach(choix);
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
                    ExtAction::SaveAsNew { index, draft } => {
                        self.save_new_fragment(index, draft);
                        self.leave_extraction();
                    }
                    ExtAction::Delete { index } => {
                        self.delete_fragment(index);
                        self.leave_extraction();
                    }
                    ExtAction::ToggleTheme => {
                        self.theme = if theme.dark { theme::CLAIR } else { theme::SOMBRE };
                    }
                    ExtAction::Attach { index } => self.ask_attach(index),
                }
            }
            return;
        }
        let mut export_all = false;
        let infos = self.chip_infos();
        let mut bar_actions = Vec::new();
        // La barre du haut du prototype : logo, **barre de fragments** (la liste d'analyse, D-066), thème. « Ouvrir… »
        // et l'export y restent, écarts écrits (D-066) : la maquette n'ouvre qu'au tableau de bord et n'exporte pas.
        egui::Panel::top("barre")
            .frame(egui::Frame::new().fill(theme.ui.base).inner_margin(egui::Margin {
                left: 16,
                right: 12,
                top: 0,
                bottom: 0,
            }))
            .show(root, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = TOP_GAP;
                    ui.allocate_ui_with_layout(
                        egui::vec2(LOGO_W, TOP_H),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_size(egui::vec2(LOGO_W, TOP_H));
                            ui.spacing_mut().item_spacing.x = 9.0;
                            let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(LOGO_BADGE), egui::Sense::hover());
                            ui.painter().rect_filled(r, 7.0, theme.ui.highlight);
                            icon("drone", 17.0)
                                .tint(theme::plot::ON_ACCENT)
                                .paint_at(ui, egui::Rect::from_center_size(r.center(), egui::Vec2::splat(17.0)));
                            ui.label(
                                egui::RichText::new("OpenTuneBox")
                                    .size(15.0)
                                    .strong()
                                    .color(theme.ui.text),
                            );
                        },
                    );
                    let chips_w = (ui.available_width() - TOP_RIGHT_W - TOP_GAP).max(120.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(chips_w, TOP_H),
                        egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
                        |ui| {
                            ui.set_width(chips_w);
                            ui.add_space(0.0);
                            egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(0, CHIPS_PAD))
                                .show(ui, |ui| {
                                    ui.set_width(chips_w);
                                    ui.set_min_height(crate::analysis_bar::CHIP_H);
                                    bar_actions = self.bar.ui(ui, &self.state, &infos, &theme);
                                });
                        },
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), TOP_H),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.set_min_height(TOP_H);
                            ui.spacing_mut().item_spacing.x = 8.0;
                            let texte = if theme.dark { "Thème clair" } else { "Thème sombre" };
                            let bouton = egui::Button::image(icon("moon", 17.0).tint(theme.ui.muted).alt_text(texte))
                                .min_size(egui::Vec2::splat(32.0))
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(6);
                            if ui.add(bouton).on_hover_text(texte).clicked() {
                                self.theme = if theme.dark { theme::CLAIR } else { theme::SOMBRE };
                            }
                            let export = egui::Button::image(
                                icon("image-down", 17.0)
                                    .tint(theme.ui.muted)
                                    .alt_text("Exporter la fenêtre en image PNG"),
                            )
                            .min_size(egui::Vec2::splat(32.0))
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(6);
                            if ui
                                .add(export)
                                .on_hover_text("Exporter la fenêtre en image PNG")
                                .clicked()
                            {
                                export_all = true;
                            }
                            // bouton principal : plein, couleur d'accent
                            let on_accent = theme::plot::ON_ACCENT;
                            let open = egui::Button::image_and_text(
                                icon("folder-open", 15.0).tint(on_accent),
                                egui::RichText::new("Ouvrir…").size(13.0).color(on_accent).strong(),
                            )
                            .fill(theme.ui.highlight)
                            .min_size(egui::vec2(0.0, 32.0))
                            .corner_radius(6);
                            if ui.add(open).on_hover_text("Ouvrir des logs Blackbox").clicked() {
                                self.ask_files();
                            }
                        },
                    );
                });
            });
        for a in bar_actions {
            self.apply_bar(a);
        }
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
        let mut sheet_actions = Vec::new();
        // la connexion USB vit hors de son écran : une copie finie pendant qu'on regarde ailleurs se range quand même
        let mut usb_actions = self.usb.poll(&self.library);
        // déplacement de la bibliothèque : relevé à chaque image ; tant qu'il dure, la fenêtre reste sur son écran
        self.libpage.poll(&mut self.library);
        if self.libpage.moving() {
            self.screen = Screen::Library;
        }
        let mut library_actions = Vec::new();
        egui::CentralPanel::default().show(root, |ui| match self.screen {
            Screen::Dashboard => {
                dash_actions = self
                    .dash
                    .ui(ui, &self.library, &self.guide.cards.list, &self.state, &theme);
            }
            Screen::DroneSheet => {
                // la fiche d'un drone enregistré demande ses mesures : lecture de la table, puis calcul en arrière-plan
                let suivi = self.sheet.form.as_ref().and_then(|f| f.id.clone()).map(|id| {
                    let open: Vec<_> = self.state.logs.iter().map(|l| l.log.clone()).collect();
                    self.tracking.wish(&ctx, &self.library, &id, &open);
                    self.tracking.view(&self.library, &id)
                });
                sheet_actions = self.sheet.ui(ui, &self.library, &theme, suivi.as_ref());
            }
            Screen::Usb => usb_actions.extend(self.usb.ui(ui, &self.library, &theme)),
            Screen::Library => {
                let blocked = (self.usb.busy() || self.usb.held.waiting()).then_some(crate::library_page::USB_BUSY);
                library_actions = self.libpage.ui(ui, &mut self.library, &theme, blocked);
            }
            Screen::Viewer => {} // dessiné plus haut, hors rail
            Screen::Tool(i) if i < self.pages.len() => {
                self.pages[i].read_library(&self.library, &self.state);
                self.pages[i].ui(ui, &mut self.state, &theme);
                for edit in self.pages[i].take_library_edits() {
                    edit.apply(&mut self.library);
                }
                goto = self.pages[i].take_goto();
                space_goto = self.pages[i].take_space_goto();
                export_page = self.pages[i].take_export();
            }
            _ => guide_actions = self.guide.ui(ui, &mut self.state, &self.library, &theme),
        });
        // une vue d'espace s'ouvre directement : pas la peine d'avoir un log pour passer du Spectre au Simulateur
        if let Some(title) = space_goto
            && let Some(j) = self.pages.iter().position(|p| p.title() == title)
        {
            self.screen = Screen::Tool(j);
        }
        for a in dash_actions {
            match a {
                DashAction::Usb => {
                    self.usb.enter();
                    self.screen = Screen::Usb;
                }
                DashAction::Library => self.open_library_screen(),
                DashAction::NewDrone => self.open_sheet(SheetForm::new("", crate::drone_sheet::new_sheet())),
                DashAction::EditDrone(id) => {
                    if let Some(d) = self.library.drone(&id) {
                        self.open_sheet(SheetForm::edit(d));
                    }
                }
                DashAction::OpenDialog => {
                    // la maquette passe à l'écran d'extraction du log qu'on vient d'ouvrir
                    if self.ask_files() {
                        self.edit_after_open = Some(EditTarget::Any);
                    }
                }
                DashAction::Toggle { fragment, path } => self.toggle_fragment(&fragment, path),
                DashAction::Edit { fragment, path } => self.edit_fragment(&fragment, path),
            }
        }
        for a in library_actions {
            match a {
                crate::library_page::LibraryAction::Leave => self.screen = self.library_return,
                crate::library_page::LibraryAction::Switch(lib) => self.switch_library(*lib),
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
                GuideAction::Open(paths, role) => self.open_as(paths, role),
                GuideAction::Role { fragment, role } => {
                    if let Some(mut f) = self.library.fragment(&fragment).cloned() {
                        f.role = role.to_string();
                        self.library.put_fragment(f);
                    }
                }
                GuideAction::Verified { name, keys } => self.set_verified(&name, keys),
                GuideAction::NewDrone(card) => {
                    let form = match card {
                        Some(c) => {
                            let mut sheet = crate::drone_sheet::sheet_of_card(&c);
                            sheet.cells = "6S".into();
                            sheet.poles = "14".into();
                            if let Some(l) = self.state.current_log() {
                                if let Some(v) = crate::drone_sheet::version_of_firmware(&l.log.settings.firmware) {
                                    sheet.betaflight = v.to_string();
                                }
                                // ce que le log dit remplace les valeurs d'un drone neuf (D-077, D-086)
                                let m = crate::drone_sheet::machine_of_log(&l.log.settings, l.log.vbat.as_deref(), "");
                                crate::drone_sheet::fill_from_log(&mut sheet, &m);
                            }
                            // un drone que la bibliothèque connaît déjà par ses logs : c'est sa fiche qu'on remplit
                            match self.library.drone_by_name(&c.name) {
                                Some(d) if d.sheet.is_none() => {
                                    let mut f = SheetForm::edit(d);
                                    f.sheet = sheet;
                                    f
                                }
                                _ => SheetForm::new(&c.name, sheet),
                            }
                        }
                        None => SheetForm::new("", crate::drone_sheet::new_sheet()),
                    };
                    self.open_sheet(form);
                }
                GuideAction::EditDrone(name) => {
                    if let Some(d) = self.library.drone_by_name(&name) {
                        self.open_sheet(SheetForm::edit(d));
                    }
                }
                GuideAction::Done { name, key, on } => self.edit_cases(&name, |done| {
                    if !on {
                        done.retain(|k| k != &key);
                    } else if !done.contains(&key) {
                        done.push(key.clone());
                    }
                }),
                GuideAction::Reset(name) => {
                    // le relevé repart de zéro avec les cases : le Guide le refait à son prochain passage
                    self.edit_cases(&name, Vec::clear);
                    self.set_verified(&name, Vec::new());
                }
            }
        }
        for a in sheet_actions {
            self.apply_sheet(a);
        }
        for a in usb_actions {
            self.apply_usb(a);
        }
    }

    /// Ce que l'écran de récupération demande à la bibliothèque (lot 32b, D-074).
    fn apply_usb(&mut self, a: crate::usb_page::UsbAction) {
        use crate::usb_page::UsbAction;
        let today = crate::advice::now_iso()[..10].to_string();
        match a {
            UsbAction::Leave => self.screen = Screen::Dashboard,
            UsbAction::Copy(choice, file) => {
                let Some(identity) = self.usb.board.as_ref().map(|b| b.identity.clone()) else {
                    return;
                };
                let drone = match crate::usb::apply_choice(&mut self.library, &choice, &identity, &today) {
                    Ok(id) => id,
                    Err(e) => return self.usb.error = e,
                };
                let name = self.library.drone(&drone).map_or(drone.clone(), |d| d.name.clone());
                match self.library.copy_path(&name, &file) {
                    None => self.usb.error = "Bibliothèque sans dossier : rien ne peut y être copié.".into(),
                    Some(Err(e)) => self.usb.error = format!("Dossier des copies impossible à créer : {e}."),
                    Some(Ok(path)) => self.usb.start_copy(drone, path),
                }
            }
            UsbAction::Register(copied, drone) => {
                crate::usb::register_copy(&mut self.library, &copied, &drone, &today);
                // ⚠️ écrite tout de suite : l'effacement n'est proposé qu'une fois la copie connue **sur le disque**
                let r = self
                    .library
                    .save()
                    .map_err(|e| format!("écriture de la bibliothèque impossible ({e})"));
                self.usb.registered(r);
            }
            UsbAction::Open(paths) => self.open(paths),
            UsbAction::ToDisk(choice) => {
                let Some(identity) = self.usb.board.as_ref().map(|b| b.identity.clone()) else {
                    return;
                };
                match crate::usb::apply_choice(&mut self.library, &choice, &identity, &today) {
                    Ok(id) => self.usb.start_disk(id),
                    Err(e) => self.usb.error = e,
                }
            }
            UsbAction::CopyDisk(choice, names) => {
                // carte SD d'un lecteur : le drone choisi à la main ; carte branchée : celui déjà rangé (`start_disk`)
                let drone = match choice {
                    Some(c) => match crate::usb::apply_reader_choice(&mut self.library, &c, &today) {
                        Ok(id) => id,
                        Err(e) => return self.usb.error = e,
                    },
                    None => match self.usb.drone_id() {
                        Some(id) => id,
                        None => return,
                    },
                };
                let Some(disk) = self.usb.disk.clone() else { return };
                let name = self.library.drone(&drone).map_or(drone.clone(), |d| d.name.clone());
                match crate::usb::disk_plan(&self.library, &disk, &name, &names) {
                    Ok(plan) => self.usb.copy_disk(drone, plan),
                    Err(e) => self.usb.error = e,
                }
            }
            UsbAction::RegisterAll(copies, drone) => {
                for c in &copies {
                    crate::usb::register_copy(&mut self.library, c, &drone, &today);
                }
                // ⚠️ écrite tout de suite : la suppression ou l'effacement n'est proposé qu'une fois les copies connues
                // **sur le disque**
                let r = self
                    .library
                    .save()
                    .map_err(|e| format!("écriture de la bibliothèque impossible ({e})"));
                self.usb.registered(r);
            }
            UsbAction::SaveConfig(choice, identity, text) => {
                match crate::usb::apply_choice(&mut self.library, &choice, &identity, &today) {
                    Ok(id) => {
                        let kept = self.library.record_config(&id, &crate::advice::now_iso(), &text);
                        self.usb.message = if kept {
                            "Configuration sauvegardée : la fiche du drone peut la comparer à la précédente.".into()
                        } else {
                            "Configuration identique à la dernière sauvegardée : rien de nouveau à garder.".into()
                        };
                    }
                    Err(e) => self.usb.error = e,
                }
            }
        }
    }

    /// Ce que montre la pastille de chaque log ouvert : nom du fragment et du drone pris dans la bibliothèque, fiche
    /// du survol lue dans le log.
    pub fn chip_infos(&self) -> Vec<ChipInfo> {
        self.state
            .logs
            .iter()
            .map(|ll| {
                let st = &ll.log.settings;
                let stored = ll.fragment.as_ref().and_then(|id| self.library.fragment(id));
                let drone = stored
                    .and_then(|f| self.library.log(&f.log_id))
                    .and_then(|e| self.library.drone(&e.drone_id))
                    .map_or_else(|| st.craft_name.trim().to_string(), |d| d.name.clone());
                let role = stored.map(|f| f.role.clone()).unwrap_or_default();
                let r = &st.pids[0];
                ChipInfo {
                    name: stored.map_or_else(|| ll.label.clone(), |f| f.name.clone()),
                    drone,
                    role,
                    date: stored.map(|f| f.date.clone()).unwrap_or_default(),
                    start: start_time(st.raw.get("Log start datetime").map(String::as_str)),
                    duration: format!("{:.1} s", ll.period.1 - ll.period.0).replace('.', ","),
                    pid: format!("{}·{}·{}·{}", r.p, r.i, r.d, r.f),
                    firmware: st.firmware.clone(),
                }
            })
            .collect()
    }

    /// Applique un geste de la barre de fragments.
    pub fn apply_bar(&mut self, a: BarAction) {
        match a {
            BarAction::Toggle(i) => {
                if let Some(on) = self.state.logs.get(i).map(|l| l.active) {
                    self.state.set_active(i, !on);
                }
            }
            BarAction::Master(i) => {
                // le maître est montré : le choisir réactive sa pastille
                self.state.set_active(i, true);
                self.state.set_current(Some(i));
            }
            BarAction::Remove(i) => self.state.remove(i),
            BarAction::Move { from, to } => self.state.move_log(from, to),
        }
    }

    /// Le « + » d'un fragment au tableau de bord : l'ajoute à l'analyse (en n'ouvrant **que sa session**) ou l'en
    /// retire s'il y est déjà.
    pub fn toggle_fragment(&mut self, fragment: &str, path: PathBuf) {
        if let Some(i) = self
            .state
            .logs
            .iter()
            .position(|l| l.fragment.as_deref() == Some(fragment))
        {
            self.state.remove(i);
            return;
        }
        if let Some(log) = self.loaded_session(fragment) {
            self.place_shared(log, fragment);
            return;
        }
        self.want(path, fragment);
    }

    /// Ouvre seulement la session `fragment` de `path` (ajoutée aux sessions déjà voulues de ce fichier).
    fn want(&mut self, path: PathBuf, fragment: &str) {
        let attendu = self.queue.contains(&path) || self.wanted.contains_key(&path);
        self.wanted
            .entry(path.clone())
            .or_default()
            .insert(fragment.to_string());
        if !attendu {
            self.open(vec![path]);
        }
    }

    /// Relit la liste d'analyse de la bibliothèque et rouvre ses fragments, en arrière-plan : on retrouve ses
    /// pastilles comme on les a laissées (D-066). Un fragment dont le log n'est plus connu est signalé, pas ouvert.
    /// À appeler une fois la vraie bibliothèque posée (`main.rs`).
    pub fn restore_analysis(&mut self) {
        let saved = self.library.analysis().clone();
        if saved.items.is_empty() {
            return;
        }
        let mut perdus = Vec::new();
        for item in &saved.items {
            let chemin = self
                .library
                .fragment(&item.fragment_id)
                .and_then(|f| self.library.log(&f.log_id))
                .map(|l| PathBuf::from(&l.path));
            match chemin {
                Some(p) if p.exists() => self.want(p, &item.fragment_id),
                Some(p) => perdus.push(nom_court(&p)),
                None => perdus.push(item.fragment_id.clone()),
            }
        }
        if !perdus.is_empty() {
            self.errors
                .push(format!("Liste d'analyse : introuvable(s) {}", perdus.join(", ")));
        }
        self.restoring = Some(saved);
    }

    /// Fin de la relecture : l'ordre et le maître de la liste enregistrée sont rendus aux logs rouverts.
    fn finish_restore(&mut self, saved: &otb_store::model::Analysis) {
        let rang = |l: &crate::state::LoadedLog| {
            l.fragment
                .as_ref()
                .and_then(|id| saved.items.iter().position(|it| &it.fragment_id == id))
                .unwrap_or(usize::MAX)
        };
        // tri par insertion, avec les seuls gestes de l'état (le log courant suit son log)
        for k in 1..self.state.logs.len() {
            let mut j = k;
            while j > 0 && rang(&self.state.logs[j - 1]) > rang(&self.state.logs[j]) {
                self.state.move_log(j, j - 1);
                j -= 1;
            }
        }
        if let Some(w) = &saved.working
            && let Some(i) = self.state.logs.iter().position(|l| l.fragment.as_ref() == Some(w))
        {
            self.state.set_current(Some(i));
        }
    }

    /// Recopie la liste d'analyse de l'écran dans la bibliothèque (écrite quand eframe enregistre) : ordre,
    /// couleur, actif, maître. Un log rangé sous aucun drone n'a pas de fragment : il n'est pas gardé.
    fn sync_analysis(&mut self) {
        if let Some(saved) = &self.restoring {
            if self.opening.busy() || !self.queue.is_empty() {
                return;
            }
            let saved = saved.clone();
            self.restoring = None;
            self.finish_restore(&saved);
        }
        // un même fichier ouvert deux fois donne deux logs du même fragment : la liste n'en garde qu'un
        let mut vus = HashSet::new();
        let items = self
            .state
            .logs
            .iter()
            .filter_map(|l| {
                let id = l.fragment.clone()?;
                vus.insert(id.clone()).then(|| otb_store::model::AnalysisItem {
                    fragment_id: id,
                    colour: colour_index(l.color) as u64,
                    active: l.active,
                })
            })
            .collect();
        let working = self.state.current_log().and_then(|l| l.fragment.clone());
        let now = otb_store::model::Analysis { items, working };
        if *self.library.analysis() != now {
            self.library.set_analysis(now);
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
        } else if let Some(log) = self.loaded_session(fragment) {
            if let Some(i) = self.place_shared(log, fragment) {
                self.state.set_current(Some(i));
                self.enter_extraction();
            }
        } else if self.library.fragment(fragment).is_some_and(otb_store::link::is_added) {
            // un fragment ajouté n'est pas celui que l'ouverture du fichier met en analyse : on le demande (D-067)
            self.edit_after_open = Some(EditTarget::Fragment(fragment.to_string()));
            self.want(path, fragment);
        } else {
            self.edit_after_open = Some(EditTarget::Fragment(fragment.to_string()));
            self.open(vec![path]);
        }
    }

    /// L'écran de l'emplacement de la bibliothèque ; il revient à l'écran d'où l'on vient.
    pub fn open_library_screen(&mut self) {
        if self.screen != Screen::Library {
            self.library_return = self.screen;
        }
        self.screen = Screen::Library;
    }

    /// Au lancement (`main.rs`) : la bibliothèque est introuvable (`missing`, la raison), ou un autre poste semble
    /// l'avoir ouverte (`warning`) ; l'écran de la bibliothèque s'ouvre pour le dire (D-081).
    pub fn show_library_problem(&mut self, missing: Option<String>, warning: Option<String>) {
        if missing.is_none() && warning.is_none() {
            return;
        }
        if let Some(why) = &missing {
            self.status = format!("Bibliothèque introuvable : {why}. Rien n'est enregistré.");
        }
        self.libpage.missing = missing;
        self.libpage.warning = warning;
        self.open_library_screen();
    }

    /// Une autre bibliothèque prend la place de l'actuelle (D-081) : l'actuelle écrit ses derniers changements et
    /// rend son témoin, les logs ouverts sont fermés (leurs fragments sont ceux de l'ancienne), puis la liste
    /// d'analyse de la nouvelle est rouverte, comme au lancement.
    pub fn switch_library(&mut self, library: Library) {
        if let Err(e) = self.library.save() {
            self.errors.push(format!("bibliothèque : écriture impossible ({e})"));
        }
        self.library.release();
        self.state.clear();
        self.tracking = Default::default();
        self.restoring = None;
        self.wanted.clear();
        self.open_roles.clear();
        self.edit_after_open = None;
        self.cards_seen = None;
        self.library = library;
        self.restore_analysis();
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

    /// Fermeture, après le dernier `save` : un déplacement interrompu laisse la bibliothèque à l'ancien dossier, qui
    /// reçoit les changements faits pendant la copie ; puis le témoin d'ouverture est retiré (D-081).
    fn on_exit(&mut self) {
        if self.libpage.moving() {
            self.library.pause_writes(false);
            let _ = self.library.save();
        }
        self.library.release();
    }

    /// Mémoire entre deux lancements : celle du Guide (D-039 : phase, fiche choisie, rôle de chaque log), le dossier
    /// du dernier export et le thème choisi.
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
        eframe::set_value(storage, "theme", &self.theme.name.to_string());
    }
}
