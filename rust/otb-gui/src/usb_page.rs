//! L'écran « Récupérer par USB » (lot 32b, D-074) : hors rail, ouvert depuis le tableau de bord. Une colonne d'étapes :
//! la carte branchée, le drone auquel la ranger, la copie des vols (puis, en deux temps, l'effacement de la mémoire),
//! et la sauvegarde de la configuration. La connexion et ses règles sont dans `usb.rs` ; l'écran ne touche jamais la
//! bibliothèque : il rend des `UsbAction` que la fenêtre exécute.

use std::path::PathBuf;
use std::time::Instant;

use egui::{RichText, Ui, Vec2};
use otb_store::Library;
use otb_usb::erase::ErasePolling;
use otb_usb::{CopyReport, Identity};

use crate::icons::icon;
use crate::theme::{Theme, with_alpha};
use crate::tools::common::safe_text;
use crate::usb::{
    ARMED, Board, Connection, Copied, DroneChoice, Event, MSP_MO_PER_MIN, Ports, Request, board_text, default_choice,
    duration_text, firmware_text, size_text,
};

pub const TITLE: &str = "Récupérer par USB";
pub const PROPS_OFF: &str = "Retirez les hélices avant de brancher la batterie : le drone peut s'armer par erreur.";
pub const NO_BOARD: &str =
    "Aucune carte trouvée. Branchez le drone en USB (sans le configurateur Betaflight ouvert), puis « Chercher ».";
pub const ERASE_WARNING: &str = "La mémoire du drone sera vidée. Les vols copiés restent dans la bibliothèque.";

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
    Open(PathBuf),
    /// Ranger ce `diff all` sous le drone choisi (gardé seulement s'il a changé).
    SaveConfig(DroneChoice, Identity, String),
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
        }
    }

    /// L'écran s'ouvre : cherche les cartes, sauf si une connexion est déjà là.
    pub fn enter(&mut self) {
        if self.conn.is_none() {
            self.search();
        }
    }

    fn search(&mut self) {
        self.found = (self.ports.list)();
        self.port = self.found.first().cloned();
        self.error = if self.found.is_empty() {
            NO_BOARD.to_string()
        } else {
            String::new()
        };
    }

    /// Une demande ou une copie est en cours : on ne quitte pas l'écran, on n'en lance pas d'autre.
    pub fn busy(&self) -> bool {
        self.conn.as_ref().is_some_and(Connection::busy)
            || matches!(
                self.phase,
                Phase::Copying { .. } | Phase::Erasing(_) | Phase::ReadingConfig
            )
    }

    /// La fenêtre a rangé l'identité sur le drone `drone` : la copie part dans `path`.
    pub fn start_copy(&mut self, drone: String, path: PathBuf) {
        if let Some(c) = &self.conn {
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
        if let Phase::Copied { registered, .. } = &mut self.phase {
            *registered = Some(r);
        }
    }

    /// Les nouvelles du fil de travail ; celles qui touchent la bibliothèque deviennent des actions.
    pub fn poll(&mut self, lib: &Library) -> Vec<UsbAction> {
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
                    if !matches!(self.phase, Phase::Copied { .. }) {
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
                // quitter ferme la connexion (le port se libère pour le configurateur)
                self.conn = None;
                self.board = None;
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
                    self.conn = Some(Connection::open(&self.ports, p, self.polling, ui.ctx().clone()));
                }
            });
        });
    }

    fn drone_section(&mut self, ui: &mut Ui, lib: &Library, th: &Theme) {
        let locked = !matches!(self.phase, Phase::Idle | Phase::Erased(_));
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
                let shown = match &*choice {
                    DroneChoice::Existing(id) => lib.drone(id).map_or(id.clone(), |d| d.name.clone()),
                    DroneChoice::New(_) => "Nouveau drone".to_string(),
                };
                egui::ComboBox::from_id_salt("usb-drone")
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
                            let name = self.board.as_ref().map_or(String::new(), |b| b.identity.name.clone());
                            *choice = DroneChoice::New(name);
                        }
                    })
                    .response
                    .accessible_name("Drone de cette carte");
                if let DroneChoice::New(name) = choice {
                    ui.add(egui::TextEdit::singleline(name).hint_text("Nom du drone"))
                        .accessible_name("Nom du nouveau drone");
                }
            });
        });
    }

    fn flights_section(&mut self, ui: &mut Ui, th: &Theme, actions: &mut Vec<UsbAction>) {
        let Some(b) = self.board.clone() else { return };
        let busy = self.busy();
        section(ui, th, "Vols enregistrés", |ui| match self.phase.clone() {
            Phase::Copying { done, total } => {
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
                    if ui.button("Arrêter").clicked()
                        && let Some(c) = &self.conn
                    {
                        c.cancel();
                    }
                });
            }
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
            Phase::Idle | Phase::ReadingConfig => {
                let used = u64::from(b.flash.used);
                if !b.flash.supported {
                    ui.label("Cette carte n'a pas de mémoire interne : ses logs sont sur carte SD (lot 32c).");
                    return;
                }
                if used == 0 {
                    ui.label("La mémoire est vide : rien à copier.");
                    return;
                }
                ui.label(before_copy_text(used));
                let ok = !busy && !b.armed && self.choice.is_some();
                if ui
                    .add_enabled(ok, egui::Button::new("Copier les vols"))
                    .on_disabled_hover_text(if b.armed { ARMED } else { "Choisissez d'abord le drone" })
                    .clicked()
                    && let Some(c) = self.choice.clone()
                {
                    actions.push(UsbAction::Copy(c));
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
            actions.push(UsbAction::Open(report.path.clone()));
        }
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
        if let Phase::Copied { confirm, .. } = &mut self.phase {
            *confirm = on;
        }
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
