//! Page Guide (port de `GuidePage` de `gui/guide.py`) : accueil, quatre phases à cocher, fiches drone, logs du drone vérifiés.
//!
//! Tout ce qui ne dépend pas d'egui est dans `guide.rs`. Ici : la mise en page, les boîtes de dialogue et la vérification des logs
//! (rapide dans le fil de l'interface, complète en arrière-plan : mesuré en release, 2 ms puis 3 ms sur 5 s de vol, 26 ms puis 45 ms
//! sur 185 000 points). Une page ne change jamais l'application elle-même : elle rend des `GuideAction` que `App` exécute.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use egui::{Context, RichText, Ui};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use otb_advisor::drone_card::{BF_FAMILIES, DroneCard, ESCS, PID_RATES_KHZ, card_for_log, card_from_log};
use otb_advisor::log_check::{
    BLOCK, FILTERS, FilterTargets, GroupCheck, INFO, LogCheck, OK, PID, WARN, check_log, check_logs, filter_targets,
};
use otb_advisor::profiles::PROFILES;
use otb_advisor::tuning_path::{AUTO_OK, Phase, Step, auto_status, phases};
use otb_log::flightlog::FlightLog;

use crate::guide::*;
use crate::icons::icon;
use crate::state::{AppState, Famille};
use crate::theme::Theme;

/// Texte du bouton d'aide d'une étape (nom accessible, lu par les tests) et son info-bulle.
pub const AIDE_ETAPE: &str = "Aide";
const AIDE_BULLE: &str = "Afficher l'aide de cette étape";
use crate::worker::Runner;

/// Ce que la page demande à l'application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuideAction {
    /// Afficher cet outil (le garde-fou « ouvrez d'abord un log » est celui de `App::open_tool`).
    Tool(ToolTarget),
    /// Ouvrir ces fichiers de log.
    Open(Vec<PathBuf>),
}

/// Résultat de `check_log` pour un log, vérification rapide ou complète (avec les repères des filtres).
pub struct GuideCheck {
    pub check: LogCheck,
    pub targets: Option<FilterTargets>,
    pub exact: bool,
}

/// Résultat d'une vérification de log du Guide (rangé dans le cache : `banc_cache` le mesure).
pub type CheckResult = Result<GuideCheck, String>;

/// Un log du drone tel que les cases et le panneau le voient pour ce cadre.
struct LogView {
    index: usize,
    label: String,
    color: egui::Color32,
    path: String,
    /// Avec le rôle choisi par l'utilisateur.
    check: LogCheck,
    targets: Option<FilterTargets>,
    exact: bool,
    /// La vérification complète a échoué : on montre la rapide et ce message.
    error: Option<String>,
}

#[derive(Default)]
struct CardForm {
    /// Fiche modifiée (`None` : nouvelle fiche).
    editing: Option<usize>,
    name: String,
    bf: String,
    size: String,
    pid: f64,
    esc: String,
    done: Vec<(String, bool)>,
}

impl CardForm {
    /// Une valeur absente des choix tombe sur le premier, comme `findData` de Qt dans `CardDialog._combo`.
    fn new(card: &DroneCard, editing: Option<usize>) -> CardForm {
        let mut f = CardForm {
            editing,
            done: card.done.clone(),
            ..CardForm::default()
        };
        f.set(card);
        f
    }

    fn set(&mut self, card: &DroneCard) {
        let pick = |v: &str, all: &[&str]| all.iter().find(|a| **a == v).unwrap_or(&all[0]).to_string();
        self.name = card.name.clone();
        self.bf = pick(&card.bf, &BF_FAMILIES.map(|c| c.0));
        self.size = pick(&card.size, &PROFILES.map(|p| p.key));
        self.esc = pick(&card.esc, &ESCS.map(|c| c.0));
        self.pid = PID_RATES_KHZ
            .iter()
            .map(|c| c.0)
            .find(|k| *k == card.pid_khz)
            .unwrap_or(PID_RATES_KHZ[0].0);
    }

    /// `CardDialog.card` : un nom vide devient « Mon drone », la boucle PID reste un flottant (`4.0`).
    fn card(&self) -> DroneCard {
        let name = self.name.trim();
        let mut c = DroneCard::new(if name.is_empty() { "Mon drone" } else { name });
        c.bf = self.bf.clone();
        c.size = self.size.clone();
        c.pid_khz = self.pid;
        c.esc = self.esc.clone();
        c.done = self.done.clone();
        c
    }
}

enum Dialog {
    Card(CardForm),
    /// Fiche à supprimer : son numéro et son nom au moment du clic (la fiche choisie peut changer pendant la confirmation).
    Delete {
        index: usize,
        name: String,
    },
    /// Fiche à remettre à zéro, idem.
    Reset {
        index: usize,
        name: String,
    },
}

/// Une demande de l'utilisateur, exécutée une fois le cadre dessiné (pas de modification de la page pendant qu'on la dessine).
enum Intent {
    Phase(&'static str),
    SelectCard(usize),
    NewCard,
    EditCard,
    Done(String, bool),
    ToggleHelp(String),
    Tool(&'static str, Option<&'static str>),
    OpenLogs(&'static str),
    Role(String, &'static str),
    CopyCli,
}

pub struct GuidePage {
    pub cards: Cards,
    /// Numéro de la fiche choisie dans `cards.list`.
    pub selected: Option<usize>,
    /// `HOME` ou une clé de `PHASE_KEYS`.
    pub phase: &'static str,
    /// Rôle choisi pour chaque fichier de log (chemin → `FILTERS` / `PID`).
    pub roles: BTreeMap<String, String>,
    help_open: BTreeSet<String>,
    md: CommonMarkCache,
    dialog: Option<Dialog>,
    runner: Runner<(u64, String, CheckResult)>,
    /// Vérification complète en cours : (uid du log, clé du cache).
    running: Option<(u64, String)>,
    last_uid: Option<u64>,
    /// Dernière erreur d'écriture du fichier des fiches.
    notice: String,
    cli: Vec<String>,
    cli_note: String,
    groups: Option<(String, Vec<GroupCheck>)>,
    /// Logs du drone dont la vérification a échoué ce cadre : (numéro, message).
    failures: Vec<(usize, String)>,
}

fn period_key(prefix: &str, period: (f64, f64), extra: Option<f64>) -> String {
    format!(
        "{prefix}|{:x}|{:x}|{:x}",
        period.0.to_bits(),
        period.1.to_bits(),
        extra.map_or(0, f64::to_bits)
    )
}

/// Vérification d'un log pour le Guide : rapide, ou complète avec les repères des filtres. Le résultat est rangé dans
/// le cache du log (`period_key`), d'où l'accès public : `examples/banc_cache.rs` le pèse comme l'interface le range.
pub fn run_check(
    log: &FlightLog,
    sel: std::ops::Range<usize>,
    label: &str,
    exact: bool,
    inch: Option<f64>,
) -> CheckResult {
    let check = check_log(log, Some(sel), label, exact)?;
    let targets = if exact {
        filter_targets(log, check.hover, check.moving_part, inch)?
    } else {
        None
    };
    Ok(GuideCheck { check, targets, exact })
}

const STATUS_STYLE: [(&str, &str, &str); 4] = [
    (OK, "green", "check"),
    (WARN, "orange", "triangle-alert"),
    (BLOCK, "red", "x"),
    (INFO, "blue", "info"),
];

fn status_style(status: &str) -> (&'static str, &'static str) {
    STATUS_STYLE
        .iter()
        .find(|s| s.0 == status)
        .map_or(("blue", "info"), |s| (s.1, s.2))
}

/// Un titre de phase (liste de gauche et frise), précédé d'une coche quand la phase est finie. Vrai si on le clique.
fn phase_button(ui: &mut Ui, th: &Theme, selected: bool, text: String, finished: bool) -> bool {
    ui.horizontal(|ui| {
        if finished {
            ui.add(icon("check", 14.0).tint(th.accent("green")).alt_text("Phase terminée"));
        }
        ui.selectable_label(selected, text).clicked()
    })
    .inner
}

/// Icône colorée suivie d'un texte.
fn marked(ui: &mut Ui, th: &Theme, accent: &str, icon_name: &str, alt: &str, text: impl Into<String>) {
    ui.horizontal_wrapped(|ui| {
        ui.add(icon(icon_name, 14.0).tint(th.accent(accent)).alt_text(alt));
        ui.label(text.into());
    });
}

impl GuidePage {
    pub fn new(cards: Cards) -> GuidePage {
        let mut md = CommonMarkCache::default();
        for part in HOME_TEXT.split("](").skip(1) {
            let dest = part.split(')').next().unwrap_or("");
            if dest.starts_with("otb:") {
                md.add_link_hook(dest);
            }
        }
        let selected = if cards.list.is_empty() { None } else { Some(0) };
        GuidePage {
            cards,
            selected,
            phase: HOME,
            roles: BTreeMap::new(),
            help_open: BTreeSet::new(),
            md,
            dialog: None,
            runner: Runner::default(),
            running: None,
            last_uid: None,
            notice: String::new(),
            cli: Vec::new(),
            cli_note: String::new(),
            groups: None,
            failures: Vec::new(),
        }
    }

    /// Fiches en mémoire seulement (bancs de test).
    pub fn in_memory() -> GuidePage {
        GuidePage::new(Cards::memory())
    }

    /// Une vérification complète est en cours (captures et tests : attendre sa fin).
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    /// Les commandes CLI proposées par « Copier les commandes CLI » (dernier log du drone parcouru, comme Python).
    pub fn cli_lines(&self) -> &[String] {
        &self.cli
    }

    pub fn at_home(&self) -> bool {
        self.phase == HOME
    }

    pub fn card(&self) -> Option<&DroneCard> {
        self.selected.and_then(|i| self.cards.list.get(i))
    }

    /// Numéro et nom de la fiche choisie (pour une confirmation).
    fn selected_target(&self) -> Option<(usize, String)> {
        let i = self.selected?;
        self.cards.list.get(i).map(|c| (i, c.name.clone()))
    }

    pub fn select_by_name(&mut self, name: &str) {
        self.selected = self
            .cards
            .list
            .iter()
            .position(|c| c.name == name)
            .or(if self.cards.list.is_empty() { None } else { Some(0) });
    }

    // ------------------------------------------------------------ mémoire entre deux lancements (D-039)
    pub fn restore(&mut self, storage: &dyn eframe::Storage) {
        if let Some(p) = eframe::get_value::<String>(storage, "guide.phase")
            && let Some(k) = PHASE_KEYS.iter().find(|k| **k == p)
        {
            self.phase = k;
        }
        if let Some(name) = eframe::get_value::<String>(storage, "guide.card") {
            self.select_by_name(&name);
        }
        if let Some(r) = eframe::get_value::<BTreeMap<String, String>>(storage, "guide.roles") {
            self.roles = r.into_iter().filter(|(_, v)| role_static(v).is_some()).collect();
        }
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "guide.phase", &self.phase.to_string());
        eframe::set_value(
            storage,
            "guide.card",
            &self.card().map(|c| c.name.clone()).unwrap_or_default(),
        );
        eframe::set_value(storage, "guide.roles", &self.roles);
    }

    // ------------------------------------------------------------ logs du drone et leur vérification
    fn card_logs(&self, state: &AppState) -> Vec<usize> {
        let Some(card) = self.card() else { return Vec::new() };
        (0..state.logs.len())
            .filter(|&i| card_for_log(std::slice::from_ref(card), &state.logs[i].log.settings.craft_name).is_some())
            .collect()
    }

    /// Un nouveau dernier log d'un drone qui a une fiche : on passe sur cette fiche (`_on_logs_changed`).
    fn follow_logs(&mut self, state: &AppState) {
        let last = state.logs.last().map(|l| l.uid);
        if last == self.last_uid {
            return;
        }
        self.last_uid = last;
        if let Some(ll) = state.logs.last()
            && let Some(m) = card_for_log(&self.cards.list, &ll.log.settings.craft_name)
            && let Some(pos) = self.cards.list.iter().position(|c| std::ptr::eq(c, m))
        {
            self.selected = Some(pos);
        }
    }

    fn inch(&self) -> Option<f64> {
        self.card()
            .and_then(|c| otb_advisor::profiles::profile(&c.size))
            .and_then(|p| p.nominal_inch)
    }

    /// Reçoit le résultat de la vérification complète terminée, s'il y en a un.
    fn poll_checks(&mut self, ctx: &Context, state: &AppState) {
        let Some(res) = self.runner.poll(ctx) else { return };
        let Some((uid, key)) = self.running.take() else { return };
        let res = match res {
            Ok((_, _, r)) => r,
            Err(e) => Err(e), // panique du calcul : le message est gardé, pas de nouvel essai en boucle
        };
        if let Some(ll) = state.logs.iter().find(|l| l.uid == uid)
            && period_key("guide-full", ll.period, self.inch()) == key
        {
            ll.extras.put(Famille::Guide, key, Arc::new(res));
        }
    }

    /// Les logs du drone vus par les cases et le panneau. Calcule au plus une vérification rapide par cadre et lance la complète
    /// du premier log qui n'en a pas (une à la fois).
    fn views(&mut self, ctx: &Context, state: &AppState) -> Vec<LogView> {
        let inch = self.inch();
        let mut out = Vec::new();
        let mut budget = 1;
        self.failures.clear();
        let mut wanted: Option<(usize, String)> = None;
        for i in self.card_logs(state) {
            let ll = &state.logs[i];
            let (qk, fk) = (
                period_key("guide-quick", ll.period, None),
                period_key("guide-full", ll.period, inch),
            );
            let full = ll.extras.get::<CheckResult>(&fk);
            let quick = ll.extras.get::<CheckResult>(&qk).or_else(|| {
                if budget == 0 {
                    return None;
                }
                budget -= 1;
                let r = Arc::new(run_check(&ll.log, ll.sel(), &ll.label, false, inch));
                ll.extras.put(Famille::Guide, qk.clone(), r.clone());
                Some(r)
            });
            if full.is_none() && wanted.is_none() && self.running.as_ref().is_none_or(|r| r.0 != ll.uid) {
                wanted = Some((i, fk.clone()));
            }
            let path = ll.log.path.to_string_lossy().into_owned();
            let (shown, error) = match (&full, &quick) {
                (Some(f), q) => match f.as_ref() {
                    Ok(_) => (Some(f.clone()), None),
                    Err(e) => (q.clone(), Some(e.clone())),
                },
                (None, q) => (q.clone(), None),
            };
            let Some(shown) = shown else {
                ctx.request_repaint(); // vérification rapide pas encore faite (une par cadre)
                continue;
            };
            match shown.as_ref() {
                Ok(g) => out.push(LogView {
                    index: i,
                    label: ll.label.clone(),
                    color: ll.color,
                    check: with_role(&g.check, self.roles.get(&path).map(String::as_str)),
                    targets: g.targets.clone(),
                    exact: g.exact,
                    error,
                    path,
                }),
                Err(e) => self.failures.push((i, e.clone())),
            }
        }
        if let Some((i, key)) = wanted
            && !self.runner.busy()
            && self.running.is_none()
        {
            let ll = &state.logs[i];
            let (log, sel, label, uid) = (ll.log.clone(), ll.sel(), ll.label.clone(), ll.uid);
            self.running = Some((uid, key.clone()));
            self.runner.request(ctx, move || {
                let r = run_check(&log, sel, &label, true, inch);
                Ok((uid, key, r))
            });
        }
        out
    }

    // ------------------------------------------------------------ actions
    fn write_result(&mut self, r: std::io::Result<()>) {
        match r {
            Ok(()) => self.notice.clear(),
            Err(e) => self.notice = format!("Enregistrement des fiches impossible : {e}"),
        }
    }

    fn run(
        &mut self,
        intent: Intent,
        ctx: &Context,
        state: &mut AppState,
        views: &[LogView],
        actions: &mut Vec<GuideAction>,
    ) {
        match intent {
            Intent::Phase(k) => self.phase = k,
            Intent::SelectCard(i) => {
                self.selected = Some(i);
            }
            Intent::NewCard => {
                let form = match state.current_log() {
                    Some(l) => CardForm::new(&card_from_log(&l.log), None),
                    None => CardForm::new(&DroneCard::new("Mon drone"), None),
                };
                self.dialog = Some(Dialog::Card(form));
            }
            Intent::EditCard => {
                if let (Some(i), Some(c)) = (self.selected, self.card()) {
                    self.dialog = Some(Dialog::Card(CardForm::new(c, Some(i))));
                }
            }
            Intent::Done(key, on) => {
                if let Some(i) = self.selected {
                    let r = self.cards.set_done(i, &key, on);
                    self.write_result(r);
                }
            }
            Intent::ToggleHelp(key) => {
                if !self.help_open.remove(&key) {
                    self.help_open.insert(key);
                }
            }
            Intent::Tool(tool, part) => {
                let Some(target) = tool_target(tool) else { return };
                // avec une partie du vol : période d'analyse du dernier log des filtres réglée sur elle
                let checks: Vec<&LogCheck> = views.iter().map(|v| &v.check).collect();
                if let Some(part) = part
                    && let Some((k, (t0, t1))) = part_target(&checks, part)
                {
                    let index = views[k].index;
                    state.set_current(Some(index));
                    state.set_period(index, t0, t1);
                }
                actions.push(GuideAction::Tool(target));
            }
            Intent::OpenLogs(role) => {
                let picked = rfd::FileDialog::new()
                    .set_title(format!("Ouvrir les logs des {}", role_name(role)))
                    .add_filter("Logs Blackbox", &["bbl", "bfl", "BBL", "BFL", "txt", "TXT"])
                    .pick_files();
                if let Some(files) = picked {
                    for f in &files {
                        self.roles.insert(f.to_string_lossy().into_owned(), role.to_string());
                    }
                    actions.push(GuideAction::Open(files));
                }
            }
            Intent::Role(path, role) => {
                self.roles.insert(path, role.to_string());
            }
            Intent::CopyCli => {
                ctx.copy_text(cli_text(&self.cli));
                self.cli_note = format!("{} commande(s) copiée(s), suivies de « save ».", self.cli.len());
            }
        }
    }

    // ------------------------------------------------------------ dessin
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, th: &Theme) -> Vec<GuideAction> {
        let ctx = ui.ctx().clone();
        self.follow_logs(state);
        self.poll_checks(&ctx, state);
        let views = self.views(&ctx, state);
        let card = self.card().cloned();
        let (checks, targets) = Self::card_checks(&views);
        let progress = card.as_ref().map(|c| progress(c, &checks, targets)).unwrap_or_default();
        let mut intents: Vec<Intent> = Vec::new();
        let mut actions = Vec::new();

        egui::Panel::top("guide-haut").show(ui, |ui| self.header(ui, th, &progress, &mut intents));
        egui::Panel::left("guide-phases").default_size(230.0).show(ui, |ui| {
            if ui.selectable_label(self.phase == HOME, "Accueil").clicked() {
                intents.push(Intent::Phase(HOME));
            }
            for (k, key) in PHASE_KEYS.iter().enumerate() {
                let (n, tot) = progress.iter().find(|p| p.0 == *key).map_or((0, 0), |p| (p.1, p.2));
                if phase_button(
                    ui,
                    th,
                    self.phase == *key,
                    phase_text(k, n, tot),
                    phase_finished(n, tot),
                ) {
                    intents.push(Intent::Phase(key));
                }
            }
        });
        egui::Panel::bottom("guide-logs")
            .default_size(250.0)
            .size_range(120.0..=600.0)
            .show(ui, |ui| {
                self.logs_panel(ui, th, state, &views, &mut intents);
            });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("guide-contenu")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if self.phase == HOME {
                        self.home(ui, &mut intents);
                    } else {
                        self.phase_content(ui, th, card.as_ref(), &checks, targets, &mut intents);
                    }
                });
        });
        // `card_from_log` (profil détecté…) ne se calcule que pendant que la fenêtre de fiche est ouverte
        let fill = if matches!(self.dialog, Some(Dialog::Card(_))) {
            state.current_log().map(|l| card_from_log(&l.log))
        } else {
            None
        };
        self.dialogs(&ctx, fill.as_ref());
        for it in intents {
            self.run(it, &ctx, state, &views, &mut actions);
        }
        actions
    }

    /// (vérifications des logs du drone, repères du dernier log des filtres) comme `_card_checks`.
    fn card_checks(views: &[LogView]) -> (Vec<LogCheck>, Option<&FilterTargets>) {
        let checks = views.iter().map(|v| v.check.clone()).collect();
        let roles: Vec<&str> = views.iter().map(|v| v.check.role).collect();
        let targets = last_index_with_role(&roles, FILTERS).and_then(|i| views[i].targets.as_ref());
        (checks, targets)
    }

    fn header(&mut self, ui: &mut Ui, th: &Theme, progress: &[(String, usize, usize)], intents: &mut Vec<Intent>) {
        if let Some(p) = self.cards.problem() {
            let where_ = self
                .cards
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            ui.label(
                RichText::new(format!(
                    "Le fichier des fiches ({where_}) est illisible : {p}. Il sera mis de côté sous le nom « .illisible » avant \
                     toute nouvelle écriture ; il n'est pas effacé."
                ))
                .color(th.accent("red")),
            );
        }
        if !self.notice.is_empty() {
            ui.label(RichText::new(self.notice.clone()).color(th.accent("red")));
        }
        ui.horizontal_wrapped(|ui| {
            let label = ui.label("Drone :");
            let current = self.card().map_or("Aucune fiche".to_string(), |c| c.name.clone());
            egui::ComboBox::from_id_salt("guide-fiche")
                .width(220.0)
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (i, c) in self.cards.list.iter().enumerate() {
                        if ui.selectable_label(self.selected == Some(i), &c.name).clicked() {
                            intents.push(Intent::SelectCard(i));
                        }
                    }
                })
                .response
                .labelled_by(label.id);
            if ui.button("Nouveau drone").clicked() {
                intents.push(Intent::NewCard);
            }
            let has = self.card().is_some();
            if ui.add_enabled(has, egui::Button::new("Modifier")).clicked() {
                intents.push(Intent::EditCard);
            }
            if ui
                .add_enabled(has, egui::Button::new("Recommencer"))
                .on_hover_text("Décocher toutes les cases de ce drone pour reprendre le parcours au début")
                .clicked()
            {
                self.dialog = self
                    .selected_target()
                    .map(|(index, name)| Dialog::Reset { index, name });
            }
            if ui
                .add_enabled(has, egui::Button::new("Supprimer"))
                .on_hover_text("Supprimer la fiche de ce drone et son avancement (les logs ne sont pas touchés)")
                .clicked()
            {
                self.dialog = self
                    .selected_target()
                    .map(|(index, name)| Dialog::Delete { index, name });
            }
            ui.separator();
            for (k, key) in PHASE_KEYS.iter().enumerate() {
                let (n, tot) = progress.iter().find(|p| p.0 == *key).map_or((0, 0), |p| (p.1, p.2));
                if phase_button(
                    ui,
                    th,
                    self.phase == *key,
                    phase_text(k, n, tot),
                    phase_finished(n, tot),
                ) {
                    intents.push(Intent::Phase(key));
                }
            }
        });
    }

    fn home(&mut self, ui: &mut Ui, intents: &mut Vec<Intent>) {
        CommonMarkViewer::new().show(ui, &mut self.md, HOME_TEXT);
        // les crochets sont remis à zéro au début de chaque `show` : à lire dans le même cadre
        let clicked: Vec<String> = self
            .md
            .link_hooks()
            .iter()
            .filter(|(_, c)| **c)
            .map(|(n, _)| n.clone())
            .collect();
        for dest in clicked {
            match parse_link(&dest) {
                Some(GuideLink::Phase(k)) => intents.push(Intent::Phase(k)),
                Some(GuideLink::Open(role)) => intents.push(Intent::OpenLogs(role)),
                Some(GuideLink::Tool(_)) | Some(GuideLink::Role(..)) | None => {}
            }
        }
    }

    fn phase_content(
        &mut self,
        ui: &mut Ui,
        th: &Theme,
        card: Option<&DroneCard>,
        checks: &[LogCheck],
        targets: Option<&FilterTargets>,
        intents: &mut Vec<Intent>,
    ) {
        let Some(card) = card else {
            ui.label("Créez d'abord la fiche de votre drone (bouton « Nouveau drone » en haut) : les valeurs du guide s'y adaptent.");
            return;
        };
        let all = phases(card);
        let Some(ph) = all.iter().find(|p| p.key == self.phase) else {
            return;
        };
        self.phase_header(ui, ph);
        for step in &ph.steps {
            self.step_ui(ui, th, card, step, checks, targets, intents);
            ui.add_space(4.0);
        }
    }

    fn phase_header(&mut self, ui: &mut Ui, ph: &Phase) {
        ui.heading(ph.title);
        CommonMarkViewer::new().show(ui, &mut self.md, &display_text(ph.intro));
        ui.add_space(6.0);
    }

    // Fonction de dessin : chaque argument est une grandeur de l'écran (bornes, échelle, couleurs). Les réunir en
    // structure n'ajouterait qu'un type intermédiaire, utilisé une seule fois.
    #[allow(clippy::too_many_arguments)]
    fn step_ui(
        &mut self,
        ui: &mut Ui,
        th: &Theme,
        card: &DroneCard,
        step: &Step,
        checks: &[LogCheck],
        targets: Option<&FilterTargets>,
        intents: &mut Vec<Intent>,
    ) {
        let (state, text) = auto_status(step, checks, targets).unwrap_or((None, String::new()));
        let auto_ok = state == Some(AUTO_OK);
        ui.push_id(&step.key, |ui| {
            ui.horizontal_wrapped(|ui| {
                // une case vérifiée par un log s'affiche cochée sans être enregistrée
                let mut checked = is_done(card, &step.key) || auto_ok;
                if ui.checkbox(&mut checked, &step.title).changed() {
                    intents.push(Intent::Done(step.key.clone(), checked));
                }
                // Bouton d'aide : un vrai bouton à icône, pas un « ? » posé à côté du texte. Vu par Cédric le 2026-11-06 :
                // `selectable_label` ne dessine qu'un cadre fin, qui se lit comme un encadré inerte et non comme un bouton.
                let open = self.help_open.contains(&step.key);
                let aide =
                    egui::Button::image_and_text(icon("info", 14.0).tint(th.accent("blue")), AIDE_ETAPE).selected(open);
                if ui.add(aide).on_hover_text(AIDE_BULLE).clicked() {
                    intents.push(Intent::ToggleHelp(step.key.clone()));
                }
                if let Some(tool) = step.tool
                    && ui.button(format!("Ouvrir {}", tool_label(tool, step.part))).clicked()
                {
                    intents.push(Intent::Tool(tool, step.part));
                }
                if step.open_logs && ui.button(open_logs_label(step.logs)).clicked() {
                    intents.push(Intent::OpenLogs(role_static(step.logs).unwrap_or(FILTERS)));
                }
            });
            if let Some(s) = state {
                let (accent, icon_name, word) = if s == AUTO_OK {
                    ("green", "check", "Vérifié")
                } else {
                    ("orange", "triangle-alert", "À revoir")
                };
                ui.horizontal_wrapped(|ui| {
                    ui.add(icon(icon_name, 14.0).tint(th.accent(accent)).alt_text(word));
                    ui.label(RichText::new(word).strong().color(th.accent(accent)));
                    ui.label(text.clone());
                });
            }
            if self.help_open.contains(&step.key) {
                CommonMarkViewer::new().show(ui, &mut self.md, &display_text(&step.help));
            }
        });
    }

    fn logs_panel(&mut self, ui: &mut Ui, th: &Theme, state: &AppState, views: &[LogView], intents: &mut Vec<Intent>) {
        let muted = th.ui.muted;
        let Some(card) = self.card().cloned() else {
            // le défilement remplit le panneau même sans fiche : sinon il se réduit à cette phrase et garde cette hauteur
            egui::ScrollArea::vertical().id_salt("guide-logs-defilement").auto_shrink([false, false]).show(ui, |ui| {
                ui.label(
                    RichText::new(
                        "Pas encore de fiche drone : bouton « Nouveau drone » en haut (préremplie d'après le log ouvert, s'il y en a un).",
                    )
                    .color(muted),
                );
            });
            self.cli.clear();
            return;
        };
        self.cli.clear();
        egui::ScrollArea::vertical().id_salt("guide-logs-defilement").auto_shrink([false, false]).show(ui, |ui| {
            ui.heading(format!("Logs de « {} »", card.name));
            if views.is_empty() {
                ui.label(
                    RichText::new(
                        "Aucun log de ce drone ouvert. Les logs sont reconnus par le nom du drone dans Betaflight (onglet \
                         Configuration), qui doit être celui de la fiche.",
                    )
                    .color(muted),
                );
            }
            for role in [FILTERS, PID] {
                ui.add_space(6.0);
                ui.strong(format!("Logs des {}", role_name(role)));
                let mine: Vec<&LogView> = views.iter().filter(|v| v.check.role == role).collect();
                if mine.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Aucun.").color(muted));
                        if ui.link(open_logs_label(role)).clicked() {
                            intents.push(Intent::OpenLogs(role));
                        }
                    });
                }
                for v in &mine {
                    self.log_block(ui, th, v, role, intents);
                    self.cli = v.check.cli_commands();
                }
                if role == PID {
                    self.group_checks(ui, th, state, &mine);
                }
            }
            // un log du drone dont la vérification a échoué reste au drone, avec son message
            ui.add_space(6.0);
            for (i, e) in &self.failures {
                let label = state.logs.get(*i).map_or("", |l| l.label.as_str());
                marked(ui, th, "red", "x", "Erreur", format!("{label} : vérification impossible : {e}"));
            }
            let mine: Vec<usize> = views.iter().map(|v| v.index).collect();
            let failed: Vec<usize> = self.failures.iter().map(|f| f.0).collect();
            let others: Vec<&str> = other_drone_logs(state.logs.len(), &mine, &failed)
                .into_iter()
                .map(|i| state.logs[i].label.as_str())
                .collect();
            if !others.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("Autres logs ouverts (autre drone) : {}.", others.join(", "))).color(muted));
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.cli.is_empty(), egui::Button::new("Copier les commandes CLI"))
                    .on_hover_text(
                        "Commandes équivalentes aux corrections proposées pour le dernier log du drone, à coller dans \
                         l'onglet « Ligne de commande (CLI) », si vous préférez le CLI",
                    )
                    .clicked()
                {
                    intents.push(Intent::CopyCli);
                }
                ui.label(RichText::new(self.cli_note.clone()).color(muted));
            });
        });
    }

    fn log_block(&self, ui: &mut Ui, th: &Theme, v: &LogView, role: &str, intents: &mut Vec<Intent>) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(&v.label).strong().color(v.color));
            if !v.exact && v.error.is_none() {
                ui.label(RichText::new("(vérification complète en cours…)").color(th.ui.muted));
            }
            if ui
                .link(format!("→ classer dans les {}", role_name(other_role(role))))
                .clicked()
            {
                intents.push(Intent::Role(v.path.clone(), other_role(role)));
            }
        });
        if let Some(e) = &v.error {
            marked(
                ui,
                th,
                "red",
                "x",
                "Erreur",
                format!("vérification complète impossible : {e}"),
            );
        }
        for u in v.check.usages_for(None) {
            let (accent, name) = if u.ok { ("green", "check") } else { ("red", "x") };
            let text = if u.ok {
                u.label.clone()
            } else {
                format!("{} : {}", u.label, u.reason)
            };
            marked(ui, th, accent, name, if u.ok { "Possible" } else { "Impossible" }, text);
        }
        for it in v
            .check
            .items_for(None)
            .into_iter()
            .filter(|it| it.status == WARN || it.status == BLOCK)
        {
            let (accent, name) = status_style(it.status);
            ui.horizontal_wrapped(|ui| {
                ui.add(icon(name, 14.0).tint(th.accent(accent)).alt_text(it.status));
                ui.label(RichText::new(&it.title).strong().color(th.accent(accent)));
                ui.label(format!(": {}", it.detail));
            });
            for f in &it.fixes {
                ui.label(format!("    → {}", f.where_));
            }
        }
    }

    /// Une série : un seul curseur change (rôle PID seulement). Recalculé seulement quand les logs ou leur période changent.
    fn group_checks(&mut self, ui: &mut Ui, th: &Theme, state: &AppState, mine: &[&LogView]) {
        let sig: String = mine
            .iter()
            .map(|v| {
                format!(
                    "{}:{}:{}|",
                    state.logs[v.index].uid, state.logs[v.index].period.0, state.logs[v.index].period.1
                )
            })
            .collect();
        if self.groups.as_ref().is_none_or(|g| g.0 != sig) {
            let logs: Vec<(String, &FlightLog)> = mine
                .iter()
                .map(|v| (v.label.clone(), &*state.logs[v.index].log))
                .collect();
            let delays: BTreeMap<String, Vec<f64>> = mine
                .iter()
                .map(|v| (v.label.clone(), v.check.sp_delay_ms.clone()))
                .collect();
            let list = check_logs(&logs, &delays).unwrap_or_default();
            self.groups = Some((sig, list));
        }
        if let Some((_, list)) = &self.groups {
            for g in list {
                let (accent, name) = status_style(g.status);
                marked(ui, th, accent, name, g.status, g.detail.clone());
            }
        }
    }

    /// `fill` : la fiche que donnerait le log ouvert (bouton « Remplir d'après le log ouvert »).
    fn dialogs(&mut self, ctx: &Context, fill: Option<&DroneCard>) {
        let Some(dialog) = self.dialog.take() else { return };
        match dialog {
            Dialog::Card(mut form) => {
                let mut result = None; // Some(true) : enregistrer ; Some(false) : annuler
                let m = egui::Modal::new(egui::Id::unique("guide-fiche-dialogue")).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading("Fiche drone");
                    egui::Grid::new("guide-fiche-grille").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        let l = ui.label("Nom (comme dans Betaflight)");
                        ui.add(egui::TextEdit::singleline(&mut form.name).desired_width(220.0))
                            .labelled_by(l.id)
                            .on_hover_text(
                                "Le même nom que dans Betaflight (onglet Configuration, « Nom de l'appareil ») : les logs du \
                                 drone sont reconnus grâce à lui",
                            );
                        ui.end_row();
                        combo(ui, "Version de Betaflight", "guide-bf", &BF_FAMILIES.map(|c| (c.0.to_string(), c.1.to_string())), &mut form.bf);
                        combo(ui, "Taille", "guide-taille", &PROFILES.map(|p| (p.key.to_string(), p.label.to_string())), &mut form.size);
                        let mut pid = format!("{}", form.pid);
                        combo(ui, "Boucle PID", "guide-pid", &PID_RATES_KHZ.map(|c| (format!("{}", c.0), c.1.to_string())), &mut pid);
                        form.pid = pid.parse().unwrap_or(form.pid);
                        combo(ui, "ESC", "guide-esc", &ESCS.map(|c| (c.0.to_string(), c.1.to_string())), &mut form.esc);
                    });
                    ui.label(
                        "Boucle PID : onglet Configuration, « Fréquence de la boucle PID ». Gyro : commande « status » du CLI \
                         (BMI270 → 3,2 kHz).",
                    );
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if let Some(fill) = fill
                            && ui.button("Remplir d'après le log ouvert").clicked()
                        {
                            form.set(fill);
                        }
                        if ui.button("Enregistrer").clicked() {
                            result = Some(true);
                        }
                        if ui.button("Annuler").clicked() {
                            result = Some(false);
                        }
                    });
                });
                if m.should_close() && result.is_none() {
                    result = Some(false);
                }
                match result {
                    Some(true) => {
                        let card = form.card();
                        let r = match form.editing {
                            Some(i) => self.cards.replace(i, card.clone()).map(|()| Some(i)),
                            None => self.cards.add(card.clone()).map(Some),
                        };
                        match r {
                            Ok(_) => {
                                self.notice.clear();
                                self.select_by_name(&card.name);
                            }
                            Err(e) => self.notice = format!("Enregistrement des fiches impossible : {e}"),
                        }
                    }
                    Some(false) => {}
                    None => self.dialog = Some(Dialog::Card(form)),
                }
            }
            Dialog::Delete { .. } | Dialog::Reset { .. } => {
                let delete = matches!(dialog, Dialog::Delete { .. });
                let (index, name) = match &dialog {
                    Dialog::Delete { index, name } | Dialog::Reset { index, name } => (*index, name.clone()),
                    Dialog::Card(_) => return,
                };
                let mut answer = None;
                let m = egui::Modal::new(egui::Id::unique("guide-confirmer")).show(ctx, |ui| {
                    ui.set_width(380.0);
                    if delete {
                        ui.heading("Supprimer la fiche");
                        ui.label(format!(
                            "Supprimer la fiche « {name} » et son avancement ? Les logs ne sont pas touchés ; cette suppression ne \
                             peut pas être annulée."
                        ));
                    } else {
                        ui.heading("Recommencer le parcours");
                        ui.label(format!(
                            "Décocher toutes les cases de « {name} » ? Les cases vérifiées par un log ouvert resteront cochées \
                             tant que ce log est ouvert."
                        ));
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Oui").clicked() {
                            answer = Some(true);
                        }
                        if ui.button("Non").clicked() {
                            answer = Some(false);
                        }
                    });
                });
                if m.should_close() && answer.is_none() {
                    answer = Some(false);
                }
                match answer {
                    Some(true) => {
                        // seulement si la fiche du clic est toujours là sous ce nom (sinon la liste a changé : on ne touche à rien)
                        if self.cards.list.get(index).is_some_and(|c| c.name == name) {
                            // la fiche choisie le reste ; si c'est celle qu'on supprime, `select_by_name` retombe sur la première (Python)
                            let keep = self.card().map(|c| c.name.clone()).unwrap_or_default();
                            let r = if delete {
                                self.cards.delete(index)
                            } else {
                                self.cards.reset(index)
                            };
                            if delete {
                                self.select_by_name(&keep);
                            }
                            self.write_result(r);
                        }
                    }
                    Some(false) => {}
                    None => self.dialog = Some(dialog),
                }
            }
        }
    }
}

/// Une ligne « libellé + liste déroulante » de la fenêtre de fiche ; `current` est la valeur enregistrée.
fn combo(ui: &mut Ui, label: &str, id: &str, items: &[(String, String)], current: &mut String) {
    let l = ui.label(label);
    let shown = items
        .iter()
        .find(|(v, _)| v == current)
        .map_or("", |(_, t)| t.as_str())
        .to_string();
    egui::ComboBox::from_id_salt(id)
        .width(220.0)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (v, t) in items {
                ui.selectable_value(current, v.clone(), t);
            }
        })
        .response
        .labelled_by(l.id);
    ui.end_row();
}
