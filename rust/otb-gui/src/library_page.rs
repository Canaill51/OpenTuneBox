//! L'écran « Bibliothèque » : où elle vit, l'ouvrir dans le gestionnaire de fichiers, la déplacer sans rien perdre ou
//! utiliser une bibliothèque déjà présente ailleurs (lot 34, D-081).
//!
//! Écran **hors rail**, ouvert par le bouton « Bibliothèque » du tableau de bord ; il s'ouvre aussi seul au lancement
//! quand le dossier est introuvable (disque débranché) ou qu'un autre poste semble l'avoir ouverte. Le prototype n'a
//! pas d'écran de réglages : celui-ci reprend la mise en page de l'écran USB (sections encadrées, une colonne),
//! écart écrit dans D-081.
//!
//! Le travail sur les fichiers est dans `otb_store::place` ; ici, seulement l'enchaînement : écritures suspendues,
//! copie relue sur un fil de travail, bascule, puis effacement de l'ancien dossier.

use egui::{RichText, Ui, Vec2};
use otb_store::Library;
use otb_store::place::{self, Copied, Place, Target};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use crate::icons::icon;
use crate::theme::Theme;
use crate::tools::common::safe_text;
use crate::usb::size_text;

pub const TITLE: &str = "Bibliothèque";
/// Le déplacement attend la fin d'une récupération USB : ses copies s'écrivent dans la bibliothèque.
pub const USB_BUSY: &str = "Une récupération par USB est en cours : attendez sa fin pour déplacer la bibliothèque.";
const LOST_WHILE_MISSING: &str = "Tant qu'elle n'est pas retrouvée, rien de ce que vous faites n'est enregistré.";

const PAGE_PAD_X: i8 = 26;
const PAGE_PAD_Y: i8 = 18;
const GAP: f32 = 14.0;
const COLUMN: f32 = 760.0;
const SECTION_RADIUS: u8 = 10;

/// Ce que l'écran demande à la fenêtre.
pub enum LibraryAction {
    /// Retour à l'écran d'où l'on venait.
    Leave,
    /// Une autre bibliothèque prend la place de l'actuelle (déjà présente ailleurs, retrouvée, ou celle du dossier
    /// par défaut) : la fenêtre ferme les logs ouverts de l'ancienne et rouvre la liste d'analyse de la nouvelle.
    Switch(Box<Library>),
}

/// Une copie en cours sur un fil de travail.
struct Job {
    from: PathBuf,
    to: PathBuf,
    rx: Receiver<Msg>,
    done: u64,
    total: u64,
}

enum Msg {
    Progress(u64, u64),
    Done(Result<Copied, String>),
}

/// Le choix du pilote, en attente de sa confirmation.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Pending {
    target: Target,
    /// Ce qu'il y a à copier (nouveau dossier) ou ce que contient la bibliothèque trouvée.
    detail: String,
}

pub struct LibraryPage {
    /// Le dossier par défaut, à côté duquel vit le pointeur. `None` : écran inerte (bancs de test, L-075), aucun
    /// pointeur n'est jamais écrit.
    pub default_dir: Option<PathBuf>,
    /// « Changer d'emplacement… » : inerte par défaut, la boîte du système au lancement (L-075).
    pub pick_folder: Box<dyn Fn() -> Option<PathBuf>>,
    /// « Ouvrir le dossier » : inerte par défaut, le gestionnaire de fichiers au lancement.
    pub open_folder: Box<dyn Fn(&Path)>,
    /// Le dossier introuvable au lancement, et pourquoi : tant qu'il est là, la bibliothèque est en mémoire seulement.
    pub missing: Option<String>,
    /// Un autre poste (ou une autre fenêtre) a écrit la bibliothèque il y a peu (D-081).
    pub warning: Option<String>,
    pending: Option<Pending>,
    job: Option<Job>,
    cleanup: Option<Receiver<Result<(), String>>>,
    /// Dernier résultat : (erreur, texte).
    pub message: Option<(bool, String)>,
    ctx: Option<egui::Context>,
}

impl Default for LibraryPage {
    fn default() -> Self {
        LibraryPage {
            default_dir: None,
            pick_folder: Box::new(|| None),
            open_folder: Box::new(|_| {}),
            missing: None,
            warning: None,
            pending: None,
            job: None,
            cleanup: None,
            message: None,
            ctx: None,
        }
    }
}

/// Ouvre un dossier dans le gestionnaire de fichiers du système.
fn open_in_file_manager(dir: &Path) {
    let program = match std::env::consts::OS {
        "windows" => "explorer",
        "macos" => "open",
        _ => "xdg-open",
    };
    let _ = std::process::Command::new(program).arg(dir).spawn();
}

/// « 3 drones, 41 logs dont 12 copiés (180 Mo) ».
pub fn contents_text(lib: &Library) -> String {
    let copied = lib.logs().iter().filter(|l| l.copied).count();
    let mut s = format!(
        "{} drone{}, {} log{}",
        lib.drones().len(),
        if lib.drones().len() > 1 { "s" } else { "" },
        lib.logs().len(),
        if lib.logs().len() > 1 { "s" } else { "" },
    );
    if copied > 0 {
        s += &format!(
            " dont {copied} copié{} ({})",
            if copied > 1 { "s" } else { "" },
            size_text(lib.copied_bytes())
        );
    }
    s
}

impl LibraryPage {
    /// L'écran du lancement réel : dossier par défaut du poste, vraies boîtes du système.
    pub fn system() -> Self {
        LibraryPage {
            default_dir: Some(otb_store::default_dir()),
            pick_folder: Box::new(|| {
                rfd::FileDialog::new()
                    .set_title("Nouvel emplacement de la bibliothèque")
                    .pick_folder()
            }),
            open_folder: Box::new(open_in_file_manager),
            ..Self::default()
        }
    }

    /// Un déplacement est en cours : la fenêtre reste sur cet écran, et rien d'autre n'écrit la bibliothèque.
    pub fn moving(&self) -> bool {
        self.job.is_some()
    }

    /// Ouvre la bibliothèque `dir` et réclame son témoin ; l'avertissement éventuel est gardé pour l'écran.
    fn opened(&mut self, dir: PathBuf) -> Library {
        let mut lib = Library::open(dir);
        self.warning = lib.claim();
        lib
    }

    // ------------------------------------------------------------ enchaînement

    /// Le pilote a choisi `chosen` : ce qu'il deviendra, à confirmer.
    pub fn choose(&mut self, lib: &Library, chosen: &Path) {
        self.message = None;
        self.pending = None;
        let current = lib.location().map(Path::to_path_buf);
        let target = match &current {
            Some(cur) => place::target_of(cur, chosen),
            // bibliothèque introuvable : on ne peut que désigner une bibliothèque existante
            None => [
                chosen.to_path_buf(),
                chosen.join(place::SUBFOLDER),
                chosen.join(otb_store::FOLDER),
            ]
            .into_iter()
            .find(|d| place::is_library(d))
            .map(Target::Existing)
            .ok_or_else(|| format!("Aucune bibliothèque dans {}.", chosen.display())),
        };
        match target {
            Err(e) => self.message = Some((true, e)),
            Ok(Target::Existing(dir)) => {
                let found = Library::open(dir.clone());
                self.pending = Some(Pending {
                    detail: contents_text(&found),
                    target: Target::Existing(dir),
                });
            }
            Ok(Target::New(dir)) => {
                let detail = match current.as_deref().map(place::library_size) {
                    Some(Ok((n, bytes))) => format!(
                        "{} à copier ({n} fichier{}).",
                        size_text(bytes),
                        if n > 1 { "s" } else { "" }
                    ),
                    _ => String::new(),
                };
                self.pending = Some(Pending {
                    target: Target::New(dir),
                    detail,
                });
            }
        }
    }

    /// Le pilote confirme son choix.
    pub fn confirm(&mut self, lib: &mut Library) -> Vec<LibraryAction> {
        let Some(p) = self.pending.take() else {
            return Vec::new();
        };
        match p.target {
            Target::Existing(dir) => self.use_existing(dir),
            Target::New(dir) => {
                self.start_move(lib, dir);
                Vec::new()
            }
        }
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }

    /// Une bibliothèque trouvée ailleurs prend la place de l'actuelle, **sans rien copier ni fusionner** : l'actuelle
    /// reste où elle est, intacte.
    fn use_existing(&mut self, dir: PathBuf) -> Vec<LibraryAction> {
        let Some(default) = self.default_dir.clone() else {
            self.message = Some((true, "Écran inerte : aucun emplacement ne peut être changé ici.".into()));
            return Vec::new();
        };
        if let Err(e) = place::set_pointer(&default, &dir) {
            self.message = Some((true, format!("Emplacement impossible à enregistrer : {e}.")));
            return Vec::new();
        }
        self.missing = None;
        let lib = self.opened(dir.clone());
        self.message = Some((false, format!("Bibliothèque de {} en service.", dir.display())));
        vec![LibraryAction::Switch(Box::new(lib))]
    }

    /// Lance la copie relue de la bibliothèque vers `to`, sur un fil de travail.
    fn start_move(&mut self, lib: &mut Library, to: PathBuf) {
        let Some(from) = lib.location().map(Path::to_path_buf) else {
            return;
        };
        if self.default_dir.is_none() {
            self.message = Some((true, "Écran inerte : aucun emplacement ne peut être changé ici.".into()));
            return;
        }
        // ⚠️ Mesures lues **avant** la copie : une lecture en arrière-plan qui finirait après l'effacement de l'ancien
        // dossier rendrait une table vide, et la prochaine mesure écrirait cette table vide dans le nouveau (D-064).
        lib.load_measures();
        if let Err(e) = lib.save() {
            self.message = Some((
                true,
                format!("Bibliothèque impossible à écrire avant le déplacement : {e}."),
            ));
            return;
        }
        lib.pause_writes(true);
        let (tx, rx) = channel();
        let ctx = self.ctx.clone();
        let (f, t) = (from.clone(), to.clone());
        std::thread::spawn(move || {
            let mut last = std::time::Instant::now();
            let r = place::copy_library(&f, &t, &mut |done, total| {
                // un message tous les dixièmes de seconde suffit à la barre
                if last.elapsed().as_millis() >= 100 || done == total {
                    last = std::time::Instant::now();
                    let _ = tx.send(Msg::Progress(done, total));
                    if let Some(c) = &ctx {
                        c.request_repaint();
                    }
                }
            });
            let _ = tx.send(Msg::Done(r));
            if let Some(c) = &ctx {
                c.request_repaint();
            }
        });
        self.job = Some(Job {
            from,
            to,
            rx,
            done: 0,
            total: 0,
        });
    }

    /// Relève l'avancement de la copie et de l'effacement ; à appeler à chaque image, sur n'importe quel écran.
    pub fn poll(&mut self, lib: &mut Library) {
        if let Some(rx) = &self.cleanup
            && let Ok(r) = rx.try_recv()
        {
            self.cleanup = None;
            if let Err(e) = r {
                self.message = Some((
                    true,
                    format!("Bibliothèque déplacée, mais l'ancien dossier reste : {e}."),
                ));
            } else if let Some((false, m)) = &mut self.message {
                *m = m.replace(" Effacement de l'ancien dossier…", " Ancien dossier effacé.");
            }
        }
        let Some(job) = &mut self.job else {
            return;
        };
        let mut done = None;
        while let Ok(m) = job.rx.try_recv() {
            match m {
                Msg::Progress(d, t) => (job.done, job.total) = (d, t),
                Msg::Done(r) => done = Some(r),
            }
        }
        let Some(result) = done else {
            return;
        };
        let job = self.job.take().expect("copie en cours");
        lib.pause_writes(false);
        match result {
            Err(e) => self.message = Some((true, e)),
            Ok(c) => self.switch_to_copy(lib, job.from, job.to, &c),
        }
    }

    /// La copie est faite et relue : la bibliothèque passe dans le nouveau dossier, puis l'ancien est effacé. Chaque
    /// étape qui échoue ramène la bibliothèque à l'ancien dossier, qu'on n'a pas encore touché.
    fn switch_to_copy(&mut self, lib: &mut Library, from: PathBuf, to: PathBuf, c: &Copied) {
        let back = |lib: &mut Library, why: String| -> Option<(bool, String)> {
            lib.relocate(from.clone());
            let _ = lib.save();
            Some((true, format!("{why} : la bibliothèque reste dans {}.", from.display())))
        };
        lib.relocate(to.clone());
        if let Err(e) = lib.save() {
            self.message = back(lib, format!("Écriture impossible dans le nouveau dossier ({e})"));
            return;
        }
        if let Err(e) = place::finish_move(&to) {
            self.message = back(lib, format!("Copie impossible à terminer ({e})"));
            return;
        }
        let default = self.default_dir.clone().unwrap_or_default();
        if let Err(e) = place::set_pointer(&default, &to) {
            self.message = back(lib, format!("Emplacement impossible à enregistrer ({e})"));
            return;
        }
        self.message = Some((
            false,
            format!(
                "Bibliothèque déplacée dans {} : {} fichier{s} copié{s} et relu{s} ({}). Effacement de l'ancien dossier…",
                to.display(),
                c.files,
                size_text(c.bytes),
                s = if c.files > 1 { "s" } else { "" },
            ),
        ));
        let (tx, rx) = channel();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(place::remove_old(&from));
            if let Some(c) = &ctx {
                c.request_repaint();
            }
        });
        self.cleanup = Some(rx);
    }

    /// Bibliothèque introuvable : on regarde de nouveau (disque rebranché).
    pub fn retry(&mut self) -> Vec<LibraryAction> {
        let Some(default) = self.default_dir.clone() else {
            return Vec::new();
        };
        match place::locate(&default) {
            Place::Ready { dir, .. } => {
                self.missing = None;
                self.message = Some((false, format!("Bibliothèque retrouvée : {}.", dir.display())));
                vec![LibraryAction::Switch(Box::new(self.opened(dir)))]
            }
            Place::Missing { why, .. } => {
                self.missing = Some(why);
                self.message = Some((true, "Toujours introuvable.".into()));
                Vec::new()
            }
        }
    }

    /// Revenir à l'emplacement par défaut. Bibliothèque introuvable : celle du dossier par défaut (vide s'il n'y en a
    /// jamais eu) est ouverte, la bibliothèque perdue reste sur son disque. Sinon : un déplacement comme un autre, ou
    /// la bibliothèque déjà présente dans le dossier par défaut, proposée telle quelle.
    pub fn back_to_default(&mut self, lib: &Library) -> Vec<LibraryAction> {
        let Some(default) = self.default_dir.clone() else {
            return Vec::new();
        };
        if lib.location().is_none() {
            if let Err(e) = place::set_pointer(&default, &default) {
                self.message = Some((true, format!("Emplacement impossible à enregistrer : {e}.")));
                return Vec::new();
            }
            self.missing = None;
            self.message = Some((
                false,
                format!("Bibliothèque du dossier par défaut : {}.", default.display()),
            ));
            return vec![LibraryAction::Switch(Box::new(self.opened(default)))];
        }
        let target = if place::is_library(&default) {
            Target::Existing(default)
        } else {
            Target::New(default)
        };
        self.message = None;
        let detail = match &target {
            Target::Existing(d) => contents_text(&Library::open(d.clone())),
            Target::New(_) => lib
                .location()
                .and_then(|d| place::library_size(d).ok())
                .map(|(_, b)| format!("{} à copier.", size_text(b)))
                .unwrap_or_default(),
        };
        self.pending = Some(Pending { target, detail });
        Vec::new()
    }

    // ------------------------------------------------------------ écran

    /// `blocked` : pourquoi un déplacement ne peut pas commencer maintenant (récupération USB en cours).
    pub fn ui(&mut self, ui: &mut Ui, lib: &mut Library, th: &Theme, blocked: Option<&str>) -> Vec<LibraryAction> {
        self.ctx = Some(ui.ctx().clone());
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
                    if let Some(w) = self.warning.clone() {
                        note(ui, th, "triangle-alert", th.accent("orange"), &w);
                        if ui.button("Compris").clicked() {
                            self.warning = None;
                        }
                    }
                    if let Some((error, m)) = &self.message {
                        let col = if *error { th.accent("red") } else { th.ui.muted };
                        ui.label(RichText::new(safe_text(m)).color(col));
                    }
                    if let Some(why) = self.missing.clone() {
                        self.missing_section(ui, lib, th, &why, &mut actions);
                    } else {
                        self.place_section(ui, lib, th, &mut actions);
                    }
                    if let Some(p) = self.pending.clone() {
                        self.confirm_section(ui, lib, th, &p, blocked, &mut actions);
                    }
                    if let Some(job) = &self.job {
                        progress_section(ui, th, job);
                    }
                });
            });
        actions
    }

    fn header(&mut self, ui: &mut Ui, th: &Theme, actions: &mut Vec<LibraryAction>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let retour = egui::Button::image(icon("arrow-left", 18.0).tint(th.ui.text).alt_text("Retour"))
                .min_size(Vec2::splat(34.0))
                .fill(th.ui.base)
                .stroke(egui::Stroke::new(1.0, th.ui.mid))
                .corner_radius(8);
            let r = ui
                .add_enabled(!self.moving(), retour)
                .on_disabled_hover_text("Attendez la fin du déplacement");
            if r.clicked() {
                self.pending = None;
                actions.push(LibraryAction::Leave);
            }
            ui.label(RichText::new(TITLE).size(22.0).strong());
        });
    }

    fn missing_section(&mut self, ui: &mut Ui, lib: &Library, th: &Theme, why: &str, actions: &mut Vec<LibraryAction>) {
        section(ui, th, "Bibliothèque introuvable", |ui| {
            note(ui, th, "triangle-alert", th.accent("red"), &format!("{why}."));
            ui.label(LOST_WHILE_MISSING);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Réessayer").clicked() {
                    actions.extend(self.retry());
                }
                if ui.button("Choisir un autre dossier…").clicked()
                    && let Some(d) = (self.pick_folder)()
                {
                    self.choose(lib, &d);
                }
                if ui.button("Revenir à l'emplacement par défaut").clicked() {
                    actions.extend(self.back_to_default(lib));
                }
            });
        });
    }

    fn place_section(&mut self, ui: &mut Ui, lib: &Library, th: &Theme, actions: &mut Vec<LibraryAction>) {
        let Some(dir) = lib.location().map(Path::to_path_buf) else {
            section(ui, th, "Emplacement", |ui| {
                ui.label("Bibliothèque en mémoire : elle ne s'écrit nulle part.");
            });
            return;
        };
        let chosen = self.default_dir.as_ref().is_some_and(|d| *d != dir);
        let busy = self.moving() || self.cleanup.is_some();
        section(ui, th, "Emplacement", |ui| {
            ui.label(
                RichText::new(if chosen {
                    "Emplacement choisi"
                } else {
                    "Emplacement par défaut"
                })
                .color(th.ui.muted),
            );
            ui.label(RichText::new(safe_text(&dir.to_string_lossy())).monospace());
            ui.label(RichText::new(contents_text(lib)).color(th.ui.muted));
            ui.label(
                "Les fiches des drones, les fragments, les mesures suivies et les logs copiés par USB y sont rangés. \
                 Les logs ouverts depuis vos dossiers restent où ils sont.",
            );
            ui.horizontal_wrapped(|ui| {
                let ouvrir = egui::Button::image_and_text(
                    icon("folder-open", 15.0).tint(th.ui.text),
                    RichText::new("Ouvrir le dossier").color(th.ui.text),
                );
                if ui.add(ouvrir).clicked() {
                    (self.open_folder)(&dir);
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Changer d'emplacement…"))
                    .clicked()
                    && let Some(d) = (self.pick_folder)()
                {
                    self.choose(lib, &d);
                }
                if chosen
                    && ui
                        .add_enabled(!busy, egui::Button::new("Revenir à l'emplacement par défaut"))
                        .clicked()
                {
                    actions.extend(self.back_to_default(lib));
                }
            });
        });
    }

    fn confirm_section(
        &mut self,
        ui: &mut Ui,
        lib: &mut Library,
        th: &Theme,
        p: &Pending,
        blocked: Option<&str>,
        actions: &mut Vec<LibraryAction>,
    ) {
        let (title, lines, go) = match &p.target {
            Target::New(d) => (
                "Déplacer la bibliothèque",
                vec![
                    format!("Vers {}", d.display()),
                    p.detail.clone(),
                    "Chaque fichier est relu après la copie et comparé à l'original ; l'ancien dossier n'est effacé \
                     qu'une fois la bibliothèque en service dans le nouveau. Ne fermez pas OpenTuneBox pendant le \
                     déplacement."
                        .to_string(),
                ],
                "Déplacer",
            ),
            Target::Existing(d) => (
                "Bibliothèque trouvée",
                vec![
                    format!("{} contient déjà une bibliothèque : {}.", d.display(), p.detail),
                    match lib.location() {
                        Some(cur) => format!(
                            "L'utiliser telle quelle ? La bibliothèque actuelle reste dans {}, sans rien perdre ni \
                             mélanger ; les logs ouverts sont fermés.",
                            cur.display()
                        ),
                        None => "L'utiliser telle quelle ?".to_string(),
                    },
                ],
                "Utiliser cette bibliothèque",
            ),
        };
        section(ui, th, title, |ui| {
            for l in lines.iter().filter(|l| !l.is_empty()) {
                ui.label(safe_text(l));
            }
            let copy = matches!(p.target, Target::New(_));
            if copy && let Some(why) = blocked {
                note(ui, th, "triangle-alert", th.accent("orange"), why);
            }
            ui.horizontal(|ui| {
                let on_accent = th.on_accent("blue");
                let bouton = egui::Button::new(RichText::new(go).color(on_accent)).fill(th.ui.highlight);
                if ui.add_enabled(!(copy && blocked.is_some()), bouton).clicked() {
                    actions.extend(self.confirm(lib));
                }
                if ui.button("Annuler").clicked() {
                    self.cancel();
                }
            });
        });
    }
}

fn progress_section(ui: &mut Ui, th: &Theme, job: &Job) {
    section(ui, th, "Déplacement en cours", |ui| {
        let part = if job.total == 0 {
            0.0
        } else {
            job.done as f32 / job.total as f32
        };
        // le total compte chaque fichier deux fois : la copie, puis la relecture
        let half = job.total / 2;
        let (phase, fait) = if job.done <= half {
            ("Copie", job.done)
        } else {
            ("Relecture", job.done - half)
        };
        ui.add(egui::ProgressBar::new(part).fill(th.accent("blue")).text(format!(
            "{phase} : {} sur {}",
            size_text(fait),
            size_text(half)
        )));
        ui.label(
            RichText::new(format!("{} → {}", job.from.display(), job.to.display()))
                .color(th.ui.muted)
                .small(),
        );
    });
}

/// Une section encadrée, au titre en tête (même dessin que l'écran USB).
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
        ui.label(RichText::new(safe_text(text)).color(th.ui.text));
    });
}
