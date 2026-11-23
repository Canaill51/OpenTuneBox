//! L'écran « Récupérer par USB » (lot 32b, D-074) : hors rail, ouvert depuis le tableau de bord. Une colonne d'étapes :
//! la carte branchée, le drone auquel la ranger, la copie des vols (puis, en deux temps, l'effacement de la mémoire),
//! et la sauvegarde de la configuration. La connexion et ses règles sont dans `usb.rs` ; l'écran ne touche jamais la
//! bibliothèque : il rend des `UsbAction` que la fenêtre exécute.
//!
//! Lot 32c (D-075) : la copie **par le mode disque** (bouton à côté de la copie par USB série ; seule voie d'une carte
//! SD), l'attente du disque avec « Choisir le dossier… » en secours, la reconnexion seule après rebranchement pour
//! effacer la mémoire, la suppression des logs d'une carte SD en deux clics, et une carte SD **dans un lecteur** quand
//! aucun drone n'est branché. Le fil du disque est dans `usb_disk.rs`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{RichText, Ui, Vec2};
use otb_store::Library;
use otb_usb::erase::ErasePolling;
use otb_usb::{CopyReport, Disk, DiskKind, Identity};

use crate::icons::icon;
use crate::theme::{Theme, with_alpha};
use crate::tools::common::safe_text;
use crate::usb::{
    ARMED, Board, Connection, Copied, DroneChoice, Event, Held, MSP_MO_PER_MIN, Ports, Request, board_text,
    default_choice, duration_text, firmware_text, reader_choice, size_text,
};
use crate::usb_disk::{DiskEvent, DiskJob, DiskPlan, DiskRequest, DiskSource, Volumes, craft_name};

pub const TITLE: &str = "Récupérer par USB";
pub const PROPS_OFF: &str = "Retirez les hélices avant de brancher la batterie : le drone peut s'armer par erreur.";
pub const NO_BOARD: &str =
    "Aucune carte trouvée. Branchez le drone en USB (sans le configurateur Betaflight ouvert), puis « Chercher ».";
pub const ERASE_WARNING: &str = "La mémoire du drone sera vidée. Les vols copiés restent dans la bibliothèque.";
pub const SD_ERASE_WARNING: &str = "Les logs copiés et vérifiés seront supprimés de la carte SD, chacun relu juste avant ; \
     les autres fichiers de la carte restent. Les vols copiés restent dans la bibliothèque.";
pub const REPLUG: &str = "Pour effacer la mémoire : débranchez le drone (USB et batterie), puis rebranchez-le en USB. \
     OpenTuneBox s'y reconnecte seul et propose l'effacement si c'est la même carte.";
pub const DISK_HINT: &str = "Le disque n'apparaît pas ? Ouvrez-le dans le gestionnaire de fichiers (disque « BETAFLT » \
     ou carte SD), ou choisissez son dossier.";
pub const EJECT: &str = "Éjectez la carte depuis le système avant de la retirer ou de débrancher le drone.";
/// Délai entre deux recherches de la carte rebranchée.
const REPLUG_SCAN: Duration = Duration::from_secs(1);
/// Après ce délai sans disque, l'aide s'affiche.
const DISK_HINT_AFTER: Duration = Duration::from_secs(10);

const PAGE_PAD_X: i8 = 26;
const PAGE_PAD_Y: i8 = 18;
const GAP: f32 = 14.0;
const COLUMN: f32 = 760.0;
const SECTION_RADIUS: u8 = 10;

/// Ce que l'écran demande à la fenêtre (seule propriétaire de la bibliothèque).
#[derive(Clone, Debug, PartialEq)]
pub enum UsbAction {
    /// Retour au tableau de bord.
    Leave,
    /// Ranger l'identité de la carte sur ce drone, puis copier (`UsbPage::start_copy`).
    Copy(DroneChoice),
    /// Ranger cette copie vérifiée sous le drone, écrire la bibliothèque, puis le dire (`UsbPage::registered`).
    Register(Copied, String),
    /// Ouvrir les vols copiés dans la barre d'analyse.
    Open(Vec<PathBuf>),
    /// Ranger ce `diff all` sous le drone choisi (gardé seulement s'il a changé).
    SaveConfig(DroneChoice, Identity, String),
    /// Ranger l'identité de la carte sur ce drone, puis la faire redémarrer en disque (`UsbPage::start_disk`).
    ToDisk(DroneChoice),
    /// Copier le disque trouvé (`UsbPage::disk`) : plan des fichiers à faire par la fenêtre (`UsbPage::copy_disk`).
    /// `Some` : carte SD d'un lecteur, drone choisi à la main ; `None` : le drone de la carte branchée.
    CopyDisk(Option<DroneChoice>),
    /// Ranger ces copies vérifiées sous le drone, écrire la bibliothèque, puis le dire (`UsbPage::registered`).
    RegisterAll(Vec<Copied>, String),
}

/// Où en est la récupération.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Idle,
    Copying {
        done: u64,
        total: u64,
    },
    /// Copie vérifiée ; `registered` : rangée dans la bibliothèque et la bibliothèque écrite (sinon la raison).
    Copied {
        report: CopyReport,
        registered: Option<Result<(), String>>,
        confirm: bool,
    },
    Erasing(u64),
    Erased(u32),
    ReadingConfig,
    /// Redémarrage en disque demandé.
    ToDisk,
    /// La carte redémarre en disque : attente que le système le monte.
    WaitingDisk,
    /// Copie du disque finie : rapports des copies vérifiées, refus, rangement, confirmation de suppression (SD).
    DiskCopied {
        reports: Vec<CopyReport>,
        refused: Vec<String>,
        registered: Option<Result<(), String>>,
        confirm: bool,
        sd: bool,
    },
    SdErasing,
    SdErased {
        deleted: usize,
        kept: Vec<String>,
    },
}

pub struct UsbPage {
    pub ports: Ports,
    pub polling: ErasePolling,
    found: Vec<PathBuf>,
    port: Option<PathBuf>,
    conn: Option<Connection>,
    pub board: Option<Board>,
    /// Le drone choisi ; `recognized` : par l'identifiant de la carte (rien à demander).
    pub choice: Option<DroneChoice>,
    recognized: bool,
    /// Le drone où la copie en cours se range.
    drone: Option<String>,
    pub phase: Phase,
    started: Option<Instant>,
    pub message: String,
    pub error: String,
    /// Les volumes montés : inerte par défaut (les tests), le vrai relevé au lancement (`App::with_guide`, L-075).
    pub volumes: Volumes,
    /// « Choisir le dossier… » : inerte par défaut, la boîte du système au lancement (L-075).
    pub pick_folder: Box<dyn Fn() -> Option<PathBuf>>,
    /// La copie de la mémoire faite par le disque, qui attend la carte rebranchée.
    pub held: Held,
    disk_job: Option<DiskJob>,
    /// Le disque trouvé (carte en mode disque, ou carte SD d'un lecteur).
    pub disk: Option<Disk>,
    volumes_before: Vec<PathBuf>,
    disk_started: Option<Instant>,
    disk_copies: Vec<Copied>,
    disk_refused: Vec<String>,
    /// Cartes SD trouvées dans un lecteur, et celle choisie.
    pub readers: Vec<Disk>,
    reader: usize,
    pub reader_choice: Option<DroneChoice>,
    last_scan: Option<Instant>,
    /// Le disque vient d'une carte redémarrée (sinon : d'un lecteur de cartes).
    from_board: bool,
    ctx: Option<egui::Context>,
}

impl UsbPage {
    pub fn new(ports: Ports) -> Self {
        UsbPage {
            ports,
            polling: ErasePolling::default(),
            found: Vec::new(),
            port: None,
            conn: None,
            board: None,
            choice: None,
            recognized: false,
            drone: None,
            phase: Phase::Idle,
            started: None,
            message: String::new(),
            error: String::new(),
            volumes: std::sync::Arc::new(Vec::new),
            pick_folder: Box::new(|| None),
            held: Held::default(),
            disk_job: None,
            disk: None,
            volumes_before: Vec::new(),
            disk_started: None,
            disk_copies: Vec::new(),
            disk_refused: Vec::new(),
            readers: Vec::new(),
            reader: 0,
            reader_choice: None,
            last_scan: None,
            from_board: false,
            ctx: None,
        }
    }

    /// L'écran du lancement réel : vrais ports série, vrais disques montés, boîte « Choisir le dossier… » du système.
    pub fn system() -> Self {
        let mut p = Self::new(Ports::serial());
        p.volumes = crate::usb_disk::system_volumes();
        p.pick_folder = Box::new(|| {
            rfd::FileDialog::new()
                .set_title("Dossier du disque de la carte ou de la carte SD")
                .pick_folder()
        });
        p
    }

    /// L'écran s'ouvre : cherche les cartes, sauf si une connexion est déjà là.
    pub fn enter(&mut self) {
        if self.conn.is_none() && !self.in_disk_flow() {
            self.search();
        }
    }

    fn search(&mut self) {
        self.found = (self.ports.list)();
        self.port = self.found.first().cloned();
        self.scan_readers();
        // une carte SD dans un lecteur suffit : pas d'erreur de drone absent
        self.error = if self.found.is_empty() && self.readers.is_empty() {
            NO_BOARD.to_string()
        } else {
            String::new()
        };
    }

    /// Les cartes SD montées (lecteur de cartes) ; garde le choix du drone si la carte est la même.
    fn scan_readers(&mut self) {
        let found: Vec<Disk> = otb_usb::msc::find_disks((self.volumes)())
            .into_iter()
            .filter(|d| matches!(d.kind, DiskKind::SdCard { .. }))
            .collect();
        if found.iter().map(|d| &d.root).ne(self.readers.iter().map(|d| &d.root)) {
            self.reader = 0;
            self.reader_choice = None;
        }
        self.readers = found;
    }

    /// Une demande ou une copie est en cours : on ne quitte pas l'écran, on n'en lance pas d'autre.
    pub fn busy(&self) -> bool {
        self.conn.as_ref().is_some_and(Connection::busy)
            || self.disk_job.as_ref().is_some_and(DiskJob::busy)
            || matches!(
                self.phase,
                Phase::Copying { .. } | Phase::Erasing(_) | Phase::ReadingConfig | Phase::ToDisk | Phase::SdErasing
            )
    }

    /// La récupération passe par le disque (de la demande de redémarrage à la suppression sur la carte SD).
    fn in_disk_flow(&self) -> bool {
        self.disk_job.is_some()
            || matches!(
                self.phase,
                Phase::ToDisk
                    | Phase::WaitingDisk
                    | Phase::DiskCopied { .. }
                    | Phase::SdErasing
                    | Phase::SdErased { .. }
            )
    }

    /// La copie de la mémoire par le disque attend le rebranchement de la carte.
    fn awaiting_replug(&self) -> bool {
        self.conn.is_none()
            && self.held.waiting()
            && matches!(
                self.phase,
                Phase::DiskCopied {
                    sd: false,
                    registered: Some(Ok(())),
                    ..
                }
            )
    }

    /// La fenêtre a rangé l'identité sur le drone `drone` : la carte redémarre en disque (lot 32c).
    pub fn start_disk(&mut self, drone: String) {
        if let Some(c) = &self.conn {
            self.held.clear();
            self.drone = Some(drone);
            self.choice = self.drone.clone().map(DroneChoice::Existing);
            self.recognized = true;
            self.volumes_before = (self.volumes)();
            self.phase = Phase::ToDisk;
            self.message.clear();
            self.error.clear();
            c.request(Request::ToDisk);
        }
    }

    /// La fenêtre a préparé les fichiers de copie du disque trouvé, sous le drone `drone` : la copie part.
    pub fn copy_disk(&mut self, drone: String, plan: DiskPlan) {
        let Some(disk) = self.disk.clone() else { return };
        self.held.clear();
        self.drone = Some(drone);
        self.disk_copies.clear();
        self.disk_refused.clear();
        self.phase = Phase::Copying { done: 0, total: 0 };
        self.started = Some(Instant::now());
        self.message.clear();
        self.error.clear();
        let ctx = self.ctx.clone().unwrap_or_default();
        let job = self.disk_job.get_or_insert_with(|| {
            DiskJob::start(DiskSource::Reader(disk), self.volumes.clone(), self.held.clone(), ctx)
        });
        job.request(DiskRequest::Copy(plan));
    }

    /// Le drone où la copie en cours se range.
    pub fn drone_id(&self) -> Option<String> {
        self.drone.clone()
    }

    /// Fin de la récupération par le disque : l'écran revient au départ et cherche à nouveau cartes et lecteurs. Une
    /// copie de la mémoire qui attend le rebranchement garde son droit d'effacer (`held`).
    fn finish_disk(&mut self) {
        self.disk_job = None;
        self.disk = None;
        self.disk_started = None;
        self.phase = Phase::Idle;
        self.search();
    }

    /// Abandonne le disque (attente annulée, écran quitté, nouvelle carte).
    fn drop_disk(&mut self) {
        self.disk_job = None;
        self.disk = None;
        self.disk_started = None;
        if matches!(self.phase, Phase::ToDisk | Phase::WaitingDisk) {
            self.phase = Phase::Idle;
        }
    }

    /// La fenêtre a rangé l'identité sur le drone `drone` : la copie part dans `path`.
    pub fn start_copy(&mut self, drone: String, path: PathBuf) {
        if let Some(c) = &self.conn {
            self.held.clear();
            self.drone = Some(drone);
            self.choice = self.drone.clone().map(DroneChoice::Existing);
            self.recognized = true;
            self.phase = Phase::Copying { done: 0, total: 0 };
            self.started = Some(Instant::now());
            self.message.clear();
            self.error.clear();
            c.request(Request::Copy(path));
        }
    }

    /// Le rangement de la copie dans la bibliothèque a réussi (ou pas) : l'effacement n'est proposé qu'après.
    pub fn registered(&mut self, r: Result<(), String>) {
        if let Phase::Copied { registered, .. } | Phase::DiskCopied { registered, .. } = &mut self.phase {
            *registered = Some(r);
        }
    }

    /// Les nouvelles des fils de travail ; celles qui touchent la bibliothèque deviennent des actions.
    pub fn poll(&mut self, lib: &Library) -> Vec<UsbAction> {
        let mut actions = self.poll_disk();
        actions.extend(self.poll_conn(lib));
        actions
    }

    fn poll_disk(&mut self) -> Vec<UsbAction> {
        let Some(job) = &self.disk_job else { return Vec::new() };
        let mut actions = Vec::new();
        for e in job.poll() {
            match e {
                DiskEvent::Found(d) => {
                    self.disk = Some(d);
                    if self.phase == Phase::WaitingDisk {
                        // carte branchée : le drone est déjà choisi, la copie part d'elle-même
                        self.error.clear();
                        actions.push(UsbAction::CopyDisk(None));
                    }
                }
                DiskEvent::NotThisDisk(e) => self.error = e,
                DiskEvent::Progress { done, total } => self.phase = Phase::Copying { done, total },
                DiskEvent::Copied(c) => self.disk_copies.push(c),
                DiskEvent::Refused(e) => self.disk_refused.push(e),
                DiskEvent::Done => {
                    let sd = matches!(self.disk.as_ref().map(|d| &d.kind), Some(DiskKind::SdCard { .. }));
                    let copies = std::mem::take(&mut self.disk_copies);
                    let registered = if copies.is_empty() { Some(Ok(())) } else { None };
                    self.phase = Phase::DiskCopied {
                        reports: copies.iter().map(|c| c.report.clone()).collect(),
                        refused: std::mem::take(&mut self.disk_refused),
                        registered,
                        confirm: false,
                        sd,
                    };
                    if !copies.is_empty()
                        && let Some(d) = &self.drone
                    {
                        actions.push(UsbAction::RegisterAll(copies, d.clone()));
                    }
                }
                DiskEvent::SdErased { deleted, kept } => self.phase = Phase::SdErased { deleted, kept },
                DiskEvent::Failed(e) => {
                    self.error = e;
                    if matches!(self.phase, Phase::Copying { .. } | Phase::SdErasing) {
                        self.phase = Phase::Idle;
                    }
                }
            }
        }
        actions
    }

    fn poll_conn(&mut self, lib: &Library) -> Vec<UsbAction> {
        let Some(conn) = &self.conn else { return Vec::new() };
        let mut actions = Vec::new();
        let mut lost = false;
        for e in conn.poll() {
            match e {
                Event::Connected(b) => {
                    if self.choice.is_none() || self.board.as_ref().map(|x| &x.identity) != Some(&b.identity) {
                        let (c, known) = default_choice(lib, &b.identity);
                        self.choice = Some(c);
                        self.recognized = known;
                    }
                    self.board = Some(b);
                    self.error.clear();
                }
                Event::Lost(e) => {
                    self.error = e;
                    lost = true;
                }
                Event::Failed(e) => {
                    self.error = e;
                    if !matches!(self.phase, Phase::Copied { .. } | Phase::DiskCopied { .. }) {
                        self.phase = Phase::Idle;
                    }
                }
                Event::Progress { done, total } => self.phase = Phase::Copying { done, total },
                Event::Copied(c) => {
                    self.phase = Phase::Copied {
                        report: c.report.clone(),
                        registered: None,
                        confirm: false,
                    };
                    if let Some(d) = &self.drone {
                        actions.push(UsbAction::Register(c, d.clone()));
                    }
                }
                Event::NotVerified(e) => {
                    self.error = e;
                    self.phase = Phase::Idle;
                }
                Event::Erasing(t) => self.phase = Phase::Erasing(t.as_secs()),
                Event::Erased(used) => {
                    self.phase = Phase::Erased(used);
                    if let Some(b) = &mut self.board {
                        b.flash.used = used;
                    }
                }
                Event::Config(text) => {
                    if let (Some(c), Some(b)) = (self.choice.clone(), &self.board) {
                        actions.push(UsbAction::SaveConfig(c, b.identity.clone(), text));
                    }
                    self.phase = Phase::Idle;
                    lost = true;
                    self.message = "La carte redémarre : rebranchez-vous pour continuer.".into();
                }
                Event::ToDisk(before) => {
                    // le port série disparaît : la suite passe par le disque
                    self.conn = None;
                    self.board = None;
                    self.disk = None;
                    self.phase = Phase::WaitingDisk;
                    self.from_board = true;
                    self.disk_started = Some(Instant::now());
                    self.message = "La carte redémarre en disque USB : attente du disque…".into();
                    let ctx = self.ctx.clone().unwrap_or_default();
                    self.disk_job = Some(DiskJob::start(
                        DiskSource::Board(before, std::mem::take(&mut self.volumes_before)),
                        self.volumes.clone(),
                        self.held.clone(),
                        ctx,
                    ));
                    return actions;
                }
            }
        }
        if lost {
            self.conn = None;
            self.board = None;
            if matches!(
                self.phase,
                Phase::Copying { .. } | Phase::Erasing(_) | Phase::ReadingConfig
            ) {
                self.phase = Phase::Idle;
            }
        }
        actions
    }

    pub fn ui(&mut self, ui: &mut Ui, lib: &Library, th: &Theme) -> Vec<UsbAction> {
        let mut actions = Vec::new();
        self.ctx = Some(ui.ctx().clone());
        self.reconnect_if_replugged(ui.ctx());
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(th.ui.window).inner_margin(egui::Margin {
                left: PAGE_PAD_X,
                right: PAGE_PAD_X,
                top: PAGE_PAD_Y,
                bottom: PAGE_PAD_Y,
            }))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = GAP;
                self.header(ui, th, &mut actions);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_max_width(COLUMN);
                    ui.spacing_mut().item_spacing.y = GAP;
                    note(ui, th, "triangle-alert", th.accent("orange"), PROPS_OFF);
                    if !self.error.is_empty() {
                        ui.label(RichText::new(safe_text(&self.error)).color(th.accent("red")));
                    }
                    if !self.message.is_empty() {
                        ui.label(RichText::new(safe_text(&self.message)).color(th.ui.muted));
                    }
                    self.board_section(ui, th);
                    if self.board.is_some() {
                        self.drone_section(ui, lib, th);
                        self.flights_section(ui, th, &mut actions);
                        self.config_section(ui, th);
                    } else if self.in_disk_flow() {
                        self.disk_section(ui, th, &mut actions);
                    } else if self.conn.is_none() {
                        self.reader_section(ui, lib, th, &mut actions);
                    }
                });
            });
        actions
    }

    fn header(&mut self, ui: &mut Ui, th: &Theme, actions: &mut Vec<UsbAction>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let busy = self.busy();
            let retour = egui::Button::image(
                icon("arrow-left", 18.0)
                    .tint(th.ui.text)
                    .alt_text("Retour au tableau de bord"),
            )
            .min_size(Vec2::splat(34.0))
            .fill(th.ui.base)
            .stroke(egui::Stroke::new(1.0, th.ui.mid))
            .corner_radius(8);
            let r = ui
                .add_enabled(!busy, retour)
                .on_disabled_hover_text("Attendez la fin de la copie ou de l'effacement");
            if r.clicked() {
                // quitter ferme la connexion (le port se libère pour le configurateur) et abandonne l'attente du disque ;
                // une copie de la mémoire qui attend le rebranchement reste (`held`)
                self.conn = None;
                self.board = None;
                if self.awaiting_replug() {
                    // la copie attend la carte : on la retrouvera au retour
                } else if self.in_disk_flow() && !busy {
                    self.disk_job = None;
                    self.disk = None;
                    self.phase = Phase::Idle;
                }
                actions.push(UsbAction::Leave);
            }
            ui.label(RichText::new(TITLE).size(22.0).strong());
        });
    }

    fn board_section(&mut self, ui: &mut Ui, th: &Theme) {
        let connected = self.conn.is_some();
        section(ui, th, "Carte branchée", |ui| {
            if let Some(b) = &self.board {
                let id = &b.identity;
                let name = if id.name.is_empty() {
                    "(sans nom)"
                } else {
                    id.name.as_str()
                };
                ui.label(RichText::new(safe_text(name)).size(16.0).strong());
                ui.label(
                    RichText::new(format!("{} · {}", safe_text(&board_text(id)), firmware_text(id))).color(th.ui.muted),
                );
                ui.label(format!(
                    "Mémoire : {} occupés sur {}",
                    size_text(u64::from(b.flash.used)),
                    size_text(u64::from(b.flash.total))
                ));
                if b.armed {
                    ui.label(RichText::new(ARMED).color(th.accent("red")).strong());
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.busy(), egui::Button::new("Relire l'état"))
                        .clicked()
                        && let Some(c) = &self.conn
                    {
                        c.request(Request::Refresh);
                    }
                    if ui.add_enabled(!self.busy(), egui::Button::new("Déconnecter")).clicked() {
                        self.conn = None;
                        self.board = None;
                        self.phase = Phase::Idle;
                    }
                });
                return;
            }
            if connected {
                ui.label("Connexion à la carte…");
                return;
            }
            if self.awaiting_replug() {
                ui.label(RichText::new(REPLUG).strong());
                ui.label(RichText::new("Recherche de la carte…").color(th.ui.muted));
                return;
            }
            if self.in_disk_flow() {
                ui.label(if self.from_board {
                    "La carte est en mode disque (stockage de masse) : son port série a disparu."
                } else {
                    "Aucun drone branché : la carte SD est lue dans le lecteur de cartes."
                });
                return;
            }
            ui.horizontal(|ui| {
                if self.found.len() > 1 {
                    let shown = self.port.as_ref().map_or(String::new(), |p| p.display().to_string());
                    egui::ComboBox::from_id_salt("usb-port")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            for p in &self.found {
                                ui.selectable_value(&mut self.port, Some(p.clone()), p.display().to_string());
                            }
                        })
                        .response
                        .accessible_name("Port de la carte");
                } else if let Some(p) = &self.port {
                    ui.label(format!("Port : {}", p.display()));
                }
                if ui.button("Chercher").clicked() {
                    self.search();
                }
                if ui
                    .add_enabled(self.port.is_some(), egui::Button::new("Se connecter"))
                    .clicked()
                    && let Some(p) = self.port.clone()
                {
                    self.error.clear();
                    self.message.clear();
                    self.phase = Phase::Idle;
                    self.choice = None;
                    self.drop_disk();
                    self.conn = Some(Connection::open_with(
                        &self.ports,
                        p,
                        self.polling,
                        self.held.clone(),
                        ui.ctx().clone(),
                    ));
                }
            });
        });
    }

    fn drone_section(&mut self, ui: &mut Ui, lib: &Library, th: &Theme) {
        let locked = !matches!(self.phase, Phase::Idle | Phase::Erased(_));
        let board_name = self.board.as_ref().map_or(String::new(), |b| b.identity.name.clone());
        section(ui, th, "Drone", |ui| {
            let Some(choice) = self.choice.as_mut() else { return };
            if self.recognized
                && let DroneChoice::Existing(id) = &*choice
            {
                let name = lib.drone(id).map_or(id.as_str(), |d| d.name.as_str());
                ui.label(
                    RichText::new(format!("Reconnu par l'identifiant de sa carte : {}", safe_text(name))).strong(),
                );
                return;
            }
            ui.label(
                "Première connexion de cette carte : à quel drone la ranger ? Il sera reconnu ensuite, même renommé.",
            );
            ui.add_enabled_ui(!locked, |ui| {
                drone_combo(ui, lib, choice, &board_name, "usb-drone", "Drone de cette carte");
            });
        });
    }

    fn flights_section(&mut self, ui: &mut Ui, th: &Theme, actions: &mut Vec<UsbAction>) {
        let Some(b) = self.board.clone() else { return };
        let busy = self.busy();
        section(ui, th, "Vols enregistrés", |ui| match self.phase.clone() {
            Phase::Copying { done, total } => self.progress_ui(ui, th, done, total),
            Phase::Copied {
                report,
                registered,
                confirm,
            } => self.copied_ui(ui, th, &report, registered, confirm, actions),
            Phase::Erasing(s) => {
                ui.label(format!("Effacement en cours… {s} s (jusqu'à une minute pour 16 Mo)."));
            }
            Phase::Erased(used) => {
                let col = if used == 0 { th.severity("ok") } else { th.accent("red") };
                ui.label(
                    RichText::new(format!(
                        "Mémoire effacée ; place occupée relue : {}.",
                        size_text(u64::from(used))
                    ))
                    .color(col),
                );
            }
            Phase::DiskCopied {
                reports,
                refused,
                registered,
                confirm,
                sd: false,
            } if self.held.matches(&b) => {
                // la carte rebranchée est celle copiée par le disque : l'effacement se propose ici
                ui.label(RichText::new("Copie faite par le mode disque, vérifiée et rangée.").strong());
                self.disk_copied_ui(ui, th, &reports, &refused, registered.clone(), actions);
                if registered == Some(Ok(())) {
                    self.erase_ui(ui, th, confirm);
                }
            }
            _ => {
                if let Phase::DiskCopied { sd: false, .. } = &self.phase
                    && self.held.waiting()
                {
                    ui.label(
                        RichText::new(
                            "Ce n'est pas la carte copiée par le disque, ou un vol a été enregistré depuis : \
                             effacement non proposé.",
                        )
                        .color(th.ui.muted),
                    );
                }
                self.copy_buttons(ui, th, &b, busy, actions);
            }
        });
    }

    /// Avant la copie : la copie par USB série (bouton principal) et celle par le mode disque, à côté (D-075).
    fn copy_buttons(&mut self, ui: &mut Ui, th: &Theme, b: &Board, busy: bool, actions: &mut Vec<UsbAction>) {
        let ok = !busy && !b.armed && self.choice.is_some();
        let why = if b.armed { ARMED } else { "Choisissez d'abord le drone" };
        if !b.flash.supported {
            ui.label("Cette carte n'a pas de mémoire interne : ses logs sont sur carte SD, copiés par le mode disque.");
            if ui
                .add_enabled(ok, egui::Button::new("Copier la carte SD (mode disque)"))
                .on_disabled_hover_text(why)
                .clicked()
                && let Some(c) = self.choice.clone()
            {
                actions.push(UsbAction::ToDisk(c));
            }
            return;
        }
        let used = u64::from(b.flash.used);
        if used == 0 {
            ui.label("La mémoire est vide : rien à copier.");
            return;
        }
        ui.label(before_copy_text(used));
        ui.horizontal(|ui| {
            let blue = th.accent("blue");
            let primary = egui::Button::new(RichText::new("Copier les vols").color(th.on_accent("blue"))).fill(blue);
            if ui.add_enabled(ok, primary).on_disabled_hover_text(why).clicked()
                && let Some(c) = self.choice.clone()
            {
                actions.push(UsbAction::Copy(c));
            }
            if ui
                .add_enabled(ok, egui::Button::new("Copier en mode disque"))
                .on_disabled_hover_text(why)
                .clicked()
                && let Some(c) = self.choice.clone()
            {
                actions.push(UsbAction::ToDisk(c));
            }
        });
        ui.label(
            RichText::new(
                "Mode disque : plus rapide, mais il faudra débrancher puis rebrancher le drone pour effacer sa mémoire.",
            )
            .color(th.ui.muted)
            .size(12.0),
        );
    }

    /// La barre de progression d'une copie et le temps restant au débit observé.
    fn progress_ui(&self, ui: &mut Ui, th: &Theme, done: u64, total: u64) {
        let part = if total > 0 { done as f32 / total as f32 } else { 0.0 };
        ui.add(egui::ProgressBar::new(part).fill(th.accent("blue")).text(format!(
            "{} / {}",
            size_text(done),
            size_text(total)
        )));
        let left = self
            .started
            .map(|s| s.elapsed().as_secs_f64())
            .filter(|&s| s > 2.0 && done > 0);
        let text = match left {
            Some(s) => {
                let rate = done as f64 / 1_048_576.0 / (s / 60.0);
                format!("Restant : {}", duration_text(total.saturating_sub(done), rate))
            }
            None => "Copie en cours…".to_string(),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(text).color(th.ui.muted));
            if ui.button("Arrêter").clicked() {
                if let Some(c) = &self.conn {
                    c.cancel();
                }
                if let Some(j) = &self.disk_job {
                    j.cancel();
                }
            }
        });
    }

    fn copied_ui(
        &mut self,
        ui: &mut Ui,
        th: &Theme,
        report: &CopyReport,
        registered: Option<Result<(), String>>,
        confirm: bool,
        actions: &mut Vec<UsbAction>,
    ) {
        let s = if report.fragments > 1 { "s" } else { "" };
        ui.label(
            RichText::new(format!(
                "{} vol{s} copié{s} et vérifié{s} ({}), rangé{s} dans la bibliothèque.",
                report.fragments,
                size_text(report.bytes)
            ))
            .color(th.severity("ok"))
            .strong(),
        );
        ui.label(
            RichText::new(safe_text(&report.path.display().to_string()))
                .color(th.ui.muted)
                .size(12.0),
        );
        match registered {
            None => {
                ui.label("Rangement dans la bibliothèque…");
                return;
            }
            Some(Err(e)) => {
                ui.label(
                    RichText::new(format!(
                        "Rangement impossible : {e}. Effacement refusé : la copie reste dans {}.",
                        report.path.display()
                    ))
                    .color(th.accent("red")),
                );
                return;
            }
            Some(Ok(())) => {}
        }
        if ui.button("Ouvrir les vols").clicked() {
            actions.push(UsbAction::Open(vec![report.path.clone()]));
        }
        self.erase_ui(ui, th, confirm);
    }

    /// L'effacement de la mémoire en deux clics (copie vérifiée et rangée, drone désarmé).
    fn erase_ui(&mut self, ui: &mut Ui, th: &Theme, confirm: bool) {
        let armed = self.board.as_ref().is_some_and(|b| b.armed);
        if !confirm {
            let rouge = th.accent("red");
            let b = egui::Button::new(RichText::new("Effacer la mémoire du drone…").color(rouge))
                .fill(th.ui.base)
                .stroke(egui::Stroke::new(1.0, rouge));
            if ui.add_enabled(!armed, b).on_disabled_hover_text(ARMED).clicked() {
                self.set_confirm(true);
            }
            return;
        }
        let rouge = th.accent("red");
        egui::Frame::new()
            .stroke(egui::Stroke::new(1.0, with_alpha(rouge, 96)))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(ERASE_WARNING).strong());
                ui.label(RichText::new(PROPS_OFF).color(th.ui.muted));
                ui.horizontal(|ui| {
                    let oui = egui::Button::new(RichText::new("Effacer définitivement").color(th.on_accent("red")))
                        .fill(rouge);
                    if ui.add(oui).clicked()
                        && let Some(c) = &self.conn
                    {
                        self.phase = Phase::Erasing(0);
                        c.request(Request::Erase);
                    }
                    if ui.button("Garder").clicked() {
                        self.set_confirm(false);
                    }
                });
            });
    }

    fn set_confirm(&mut self, on: bool) {
        if let Phase::Copied { confirm, .. } | Phase::DiskCopied { confirm, .. } = &mut self.phase {
            *confirm = on;
        }
    }

    /// Cherche la carte rebranchée (une fois par seconde) et s'y reconnecte seul (D-075).
    fn reconnect_if_replugged(&mut self, ctx: &egui::Context) {
        if !self.awaiting_replug() {
            return;
        }
        ctx.request_repaint_after(REPLUG_SCAN);
        if self.last_scan.is_some_and(|t| t.elapsed() < REPLUG_SCAN) {
            return;
        }
        self.last_scan = Some(Instant::now());
        let Some(port) = (self.ports.list)().into_iter().next() else {
            return;
        };
        // le disque n'a plus de raison d'être : la carte est revenue en mode normal
        self.disk_job = None;
        self.error.clear();
        self.message.clear();
        self.choice = None;
        self.port = Some(port.clone());
        self.conn = Some(Connection::open_with(
            &self.ports,
            port,
            self.polling,
            self.held.clone(),
            ctx.clone(),
        ));
    }

    /// Les copies faites par le disque : vols vérifiés et rangés, refus, « Ouvrir les vols ».
    fn disk_copied_ui(
        &mut self,
        ui: &mut Ui,
        th: &Theme,
        reports: &[CopyReport],
        refused: &[String],
        registered: Option<Result<(), String>>,
        actions: &mut Vec<UsbAction>,
    ) {
        let flights: usize = reports.iter().map(|r| r.fragments).sum();
        let bytes: u64 = reports.iter().map(|r| r.bytes).sum();
        if reports.is_empty() {
            ui.label(RichText::new("Aucun log copié et vérifié : rien n'est rangé, rien ne sera effacé.").strong());
        } else {
            let s = if flights > 1 { "s" } else { "" };
            ui.label(
                RichText::new(format!(
                    "{flights} vol{s} copié{s} et vérifié{s} ({}), rangé{s} dans la bibliothèque.",
                    size_text(bytes)
                ))
                .color(th.severity("ok"))
                .strong(),
            );
        }
        for r in refused {
            ui.label(RichText::new(format!("Refusé : {}", safe_text(r))).color(th.accent("red")));
        }
        if !refused.is_empty() {
            ui.label(RichText::new("Les logs refusés restent sur la carte.").color(th.ui.muted));
        }
        match registered {
            None => {
                ui.label("Rangement dans la bibliothèque…");
            }
            Some(Err(e)) => {
                ui.label(
                    RichText::new(format!(
                        "Rangement impossible : {e}. Rien ne sera effacé : les copies restent dans la bibliothèque."
                    ))
                    .color(th.accent("red")),
                );
            }
            Some(Ok(())) if !reports.is_empty() => {
                if ui.button("Ouvrir les vols").clicked() {
                    actions.push(UsbAction::Open(reports.iter().map(|r| r.path.clone()).collect()));
                }
            }
            Some(Ok(())) => {}
        }
    }

    /// La récupération par le disque, carte débranchée du port série : attente, copie, résultat, suppression SD.
    fn disk_section(&mut self, ui: &mut Ui, th: &Theme, actions: &mut Vec<UsbAction>) {
        section(ui, th, "Mode disque", |ui| match self.phase.clone() {
            Phase::ToDisk => {
                ui.label("Redémarrage de la carte en disque…");
            }
            Phase::WaitingDisk => {
                ui.label("Attente du disque de la carte…");
                if self.disk_started.is_some_and(|t| t.elapsed() >= DISK_HINT_AFTER) {
                    ui.label(RichText::new(DISK_HINT).color(th.ui.muted));
                } else {
                    ui.ctx().request_repaint_after(Duration::from_secs(1));
                }
                ui.horizontal(|ui| {
                    if ui.button("Choisir le dossier…").clicked()
                        && let Some(p) = (self.pick_folder)()
                        && let Some(j) = &self.disk_job
                    {
                        j.request(DiskRequest::Folder(p));
                    }
                    if ui.button("Annuler").clicked() {
                        self.drop_disk();
                        self.message = "Débranchez puis rebranchez le drone pour le ramener en mode normal.".into();
                    }
                });
            }
            Phase::Copying { done, total } => self.progress_ui(ui, th, done, total),
            Phase::DiskCopied {
                reports,
                refused,
                registered,
                confirm,
                sd,
            } => {
                self.disk_copied_ui(ui, th, &reports, &refused, registered.clone(), actions);
                if registered != Some(Ok(())) || reports.is_empty() {
                    if sd {
                        ui.label(RichText::new(EJECT).color(th.ui.muted));
                    }
                    if ui.button("Terminé").clicked() {
                        self.finish_disk();
                    }
                    return;
                }
                // mémoire interne : la section « Carte branchée » dit de rebrancher (`REPLUG`)
                if sd {
                    self.sd_erase_ui(ui, th, reports.len(), confirm);
                }
            }
            Phase::SdErasing => {
                ui.label("Suppression des logs de la carte SD…");
            }
            Phase::SdErased { deleted, kept } => {
                let s = if deleted > 1 { "s" } else { "" };
                ui.label(
                    RichText::new(format!("{deleted} log{s} supprimé{s} de la carte SD."))
                        .color(th.severity("ok"))
                        .strong(),
                );
                for k in &kept {
                    ui.label(RichText::new(format!("Gardé sur la carte : {}", safe_text(k))).color(th.accent("red")));
                }
                ui.label(RichText::new(EJECT).strong());
                if ui.button("Terminé").clicked() {
                    self.finish_disk();
                }
            }
            _ => {}
        });
    }

    /// La suppression des logs de la carte SD en deux clics.
    fn sd_erase_ui(&mut self, ui: &mut Ui, th: &Theme, n: usize, confirm: bool) {
        let rouge = th.accent("red");
        let s = if n > 1 { "s" } else { "" };
        if !confirm {
            let b = egui::Button::new(RichText::new(format!("Supprimer {n} log{s} de la carte SD…")).color(rouge))
                .fill(th.ui.base)
                .stroke(egui::Stroke::new(1.0, rouge));
            ui.horizontal(|ui| {
                if ui.add(b).clicked() {
                    self.set_confirm(true);
                }
                if ui
                    .button("Terminé")
                    .on_hover_text("Garder les logs sur la carte")
                    .clicked()
                {
                    self.finish_disk();
                }
            });
            ui.label(RichText::new(EJECT).color(th.ui.muted));
            return;
        }
        egui::Frame::new()
            .stroke(egui::Stroke::new(1.0, with_alpha(rouge, 96)))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(SD_ERASE_WARNING).strong());
                ui.horizontal(|ui| {
                    let oui = egui::Button::new(RichText::new("Supprimer définitivement").color(th.on_accent("red")))
                        .fill(rouge);
                    if ui.add(oui).clicked()
                        && let Some(j) = &self.disk_job
                    {
                        self.phase = Phase::SdErasing;
                        j.request(DiskRequest::EraseSd);
                    }
                    if ui.button("Garder").clicked() {
                        self.set_confirm(false);
                    }
                });
            });
    }

    /// Une carte SD dans un lecteur de cartes, aucun drone branché (D-075) : le drone se choisit à la main.
    fn reader_section(&mut self, ui: &mut Ui, lib: &Library, th: &Theme, actions: &mut Vec<UsbAction>) {
        section(ui, th, "Carte SD dans un lecteur", |ui| {
            ui.label(
                "Une carte SD sortie du drone et mise dans un lecteur de cartes : ses logs se copient ici, sans brancher \
                 le drone.",
            );
            ui.horizontal(|ui| {
                if ui.button("Chercher une carte SD").clicked() {
                    self.scan_readers();
                }
                if ui.button("Choisir le dossier…").clicked()
                    && let Some(p) = (self.pick_folder)()
                {
                    match otb_usb::msc::inspect(&p) {
                        Some(d) if matches!(d.kind, DiskKind::SdCard { .. }) => {
                            self.readers = vec![d];
                            self.reader = 0;
                            self.reader_choice = None;
                            self.error.clear();
                        }
                        _ => {
                            self.error = format!(
                                "{} n'est pas une carte SD Betaflight : pas de dossier LOGS.",
                                p.display()
                            )
                        }
                    }
                }
            });
            let Some(disk) = self.readers.get(self.reader).cloned() else {
                ui.label(RichText::new("Aucune carte SD trouvée.").color(th.ui.muted));
                return;
            };
            let DiskKind::SdCard { logs } = &disk.kind else { return };
            let bytes: u64 = logs.iter().map(|f| f.len).sum();
            let s = if logs.len() > 1 { "s" } else { "" };
            ui.label(format!(
                "Carte SD : {} — {} log{s} ({})",
                safe_text(&disk.root.display().to_string()),
                logs.len(),
                size_text(bytes)
            ));
            if logs.is_empty() {
                ui.label("Aucun log sur cette carte : rien à copier.");
                return;
            }
            let busy = self.busy();
            let craft = craft_name(&disk);
            let choice = self.reader_choice.get_or_insert_with(|| reader_choice(lib, &craft));
            ui.label("À quel drone ranger ces vols ?");
            drone_combo(ui, lib, choice, &craft, "usb-reader-drone", "Drone de cette carte SD");
            let ok = !busy && !matches!(choice, DroneChoice::New(n) if n.trim().is_empty());
            if ui
                .add_enabled(ok, egui::Button::new("Copier la carte SD"))
                .on_disabled_hover_text("Donnez un nom au drone")
                .clicked()
            {
                self.from_board = false;
                self.disk = Some(disk.clone());
                self.disk_job = None;
                actions.push(UsbAction::CopyDisk(self.reader_choice.clone()));
            }
        });
    }

    fn config_section(&mut self, ui: &mut Ui, th: &Theme) {
        let armed = self.board.as_ref().is_some_and(|b| b.armed);
        let busy = self.busy();
        section(ui, th, "Configuration", |ui| {
            ui.label("Lit le « diff all » de la carte et le garde s'il a changé depuis la dernière fois ; la carte redémarre ensuite.");
            let ok = !busy && !armed && self.choice.is_some();
            if ui
                .add_enabled(ok, egui::Button::new("Sauvegarder la configuration"))
                .on_disabled_hover_text(if armed {
                    ARMED
                } else {
                    "Attendez la fin de l'opération en cours"
                })
                .clicked()
                && let Some(c) = &self.conn
            {
                self.phase = Phase::ReadingConfig;
                c.request(Request::Config);
            }
        });
    }
}

/// Le choix du drone : ceux de la bibliothèque, ou « Nouveau drone » et son nom.
/// `new_name` : le nom proposé pour un nouveau drone (celui lu sur la carte ou dans le log).
/// ⚠️ `widget`, pas `salt` : CodeQL prend tout nom en « salt » pour un sel cryptographique écrit en dur (L-117).
fn drone_combo(ui: &mut Ui, lib: &Library, choice: &mut DroneChoice, new_name: &str, widget: &str, name: &str) {
    let shown = match &*choice {
        DroneChoice::Existing(id) => lib.drone(id).map_or(id.clone(), |d| d.name.clone()),
        DroneChoice::New(_) => "Nouveau drone".to_string(),
    };
    egui::ComboBox::from_id_salt(widget)
        .selected_text(safe_text(&shown))
        .show_ui(ui, |ui| {
            for d in lib.drones() {
                let sel = *choice == DroneChoice::Existing(d.id.clone());
                if ui.selectable_label(sel, safe_text(&d.name)).clicked() {
                    *choice = DroneChoice::Existing(d.id.clone());
                }
            }
            if ui
                .selectable_label(matches!(choice, DroneChoice::New(_)), "Nouveau drone")
                .clicked()
                && !matches!(choice, DroneChoice::New(_))
            {
                *choice = DroneChoice::New(new_name.to_string());
            }
        })
        .response
        .accessible_name(name);
    if let DroneChoice::New(n) = choice {
        ui.add(egui::TextEdit::singleline(n).hint_text("Nom du drone"))
            .accessible_name("Nom du nouveau drone");
    }
}

/// Une section encadrée, au titre en tête.
fn section(ui: &mut Ui, th: &Theme, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(th.ui.base)
        .stroke(egui::Stroke::new(1.0, th.ui.border))
        .corner_radius(SECTION_RADIUS)
        .inner_margin(egui::Margin::symmetric(18, 16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(RichText::new(title).size(15.0).strong());
            body(ui);
        });
}

/// Une ligne d'avertissement avec son icône.
fn note(ui: &mut Ui, th: &Theme, glyph: &str, col: egui::Color32, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.add(icon(glyph, 16.0).tint(col));
        ui.label(RichText::new(text).color(th.ui.text));
    });
}

/// La ligne d'avant la copie : la place à copier et la durée annoncée au débit mesuré.
pub fn before_copy_text(used: u64) -> String {
    format!(
        "{} à copier, {} au débit de l'USB (MSP).",
        size_text(used),
        duration_text(used, MSP_MO_PER_MIN)
    )
}
