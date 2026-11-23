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
use otb_advisor::drone_card::{DroneCard, card_for_log, card_from_log};
use otb_advisor::log_check::{
    BLOCK, FILTERS, FilterTargets, GroupCheck, INFO, LogCheck, OK, PID, WARN, check_log, check_logs, filter_targets,
};
use otb_advisor::tuning_path::{AUTO_OK, Phase, Step, auto_status, phases};
use otb_log::flightlog::FlightLog;
use otb_store::Library;

use crate::guide::*;
use crate::icons::icon;
use crate::state::{AppState, Famille};
use crate::theme::Theme;

/// Texte du bouton d'aide d'une étape (nom accessible, lu par les tests) et son info-bulle.
pub const AIDE_ETAPE: &str = "Aide";
const AIDE_BULLE: &str = "Afficher l'aide de cette étape";
use crate::worker::Runner;

/// Ce que la page demande à l'application.
#[derive(Debug, Clone, PartialEq)]
pub enum GuideAction {
    /// Afficher cet outil (le garde-fou « ouvrez d'abord un log » est celui de `App::open_tool`).
    Tool(ToolTarget),
    /// Ouvrir ces fichiers de log ; avec un rôle, celui-ci est donné aux fragments de leurs sessions (le pilote l'a
    /// choisi en demandant « Ouvrir les logs des filtres… »).
    Open(Vec<PathBuf>, Option<&'static str>),
    /// « → classer dans les … » : le rôle de ce fragment de la bibliothèque (W-074 : par fragment, plus par fichier).
    Role { fragment: String, role: &'static str },
    /// Les cases que les logs du drone de ce nom vérifient, relevées par le Guide : `App` les range sur sa fiche pour
    /// la carte du tableau de bord (D-071). Rendu seulement quand le relevé change.
    Verified { name: String, keys: Vec<String> },
    /// « Nouveau drone » : l'écran de la fiche (D-068), prérempli d'après le log ouvert s'il y en a un.
    NewDrone(Option<DroneCard>),
    /// « Modifier » : l'écran de la fiche du drone de ce nom.
    EditDrone(String),
    /// Une case cochée ou décochée à la main, sur la fiche de ce nom : `App` l'écrit dans la bibliothèque.
    Done { name: String, key: String, on: bool },
    /// « Recommencer » confirmé : toutes les cases de cette fiche décochées.
    Reset(String),
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
    /// Fragment de la bibliothèque : c'est lui qui porte le rôle (W-074).
    fragment: Option<String>,
    /// Avec le rôle choisi par l'utilisateur.
    check: LogCheck,
    targets: Option<FilterTargets>,
    exact: bool,
    /// La vérification complète a échoué : on montre la rapide et ce message.
    error: Option<String>,
}

enum Dialog {
    /// Fiche à remettre à zéro : son numéro et son nom au moment du clic (la fiche choisie peut changer pendant la
    /// confirmation).
    Reset { index: usize, name: String },
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
    Role(Option<String>, &'static str),
    CopyCli,
}

pub struct GuidePage {
    pub cards: Cards,
    /// Numéro de la fiche choisie dans `cards.list`.
    pub selected: Option<usize>,
    /// `HOME` ou une clé de `PHASE_KEYS`.
    pub phase: &'static str,
    /// **Anciens** rôles, rangés par fichier (chemin → `FILTERS` / `PID`) avant que le fragment ne les porte (W-074).
    /// Relus et réécrits tels quels : `App` en donne un au fragment d'une session qui n'a pas encore de rôle, à
    /// l'ouverture de son fichier. Plus rien ne les change.
    pub old_roles: BTreeMap<String, String>,
    /// Ce que le Guide a vu au dernier cadre : (uid du log, rôle) de chaque log vérifié, dans l'ordre de la barre.
    seen: Vec<(u64, &'static str)>,
    /// Fragments du drone désactivés dans la barre : ni vérifiés ni comptés (D-066), leurs noms seulement.
    inactive: Vec<String>,
    /// Dernier relevé rendu à `App` : (nom de la fiche, clés vérifiées).
    reported: Option<(String, Vec<String>)>,
    help_open: BTreeSet<String>,
    md: CommonMarkCache,
    dialog: Option<Dialog>,
    runner: Runner<(u64, String, CheckResult)>,
    /// Vérification complète en cours : (uid du log, clé du cache).
    running: Option<(u64, String)>,
    last_uid: Option<u64>,
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
    // ⚠️ Pas de `ui.horizontal` ici : imbriqué dans le `horizontal_wrapped` de l'en-tête, il formerait un bloc
    // insécable que le parent ne sait pas renvoyer à la ligne — la dernière phase se faisait **rogner** au lieu de
    // passer à la ligne dès que la fenêtre se resserrait (vu le 2026-11-19, quand le rail a pris 212 points).
    if finished {
        ui.add(icon("check", 14.0).tint(th.accent("green")).alt_text("Phase terminée"));
    }
    ui.selectable_label(selected, text).clicked()
}

/// Icône colorée suivie d'un texte.
fn marked(ui: &mut Ui, th: &Theme, accent: &str, icon_name: &str, alt: &str, text: impl Into<String>) {
    ui.horizontal_wrapped(|ui| {
        ui.add(icon(icon_name, 14.0).tint(th.accent(accent)).alt_text(alt));
        ui.label(text.into());
    });
}

impl Default for GuidePage {
    fn default() -> Self {
        Self::new()
    }
}

impl GuidePage {
    /// Une page sans fiche : `App` lui donne celles de la bibliothèque (`refresh`).
    pub fn new() -> GuidePage {
        let cards = Cards::default();
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
            old_roles: BTreeMap::new(),
            seen: Vec::new(),
            inactive: Vec::new(),
            reported: None,
            help_open: BTreeSet::new(),
            md,
            dialog: None,
            runner: Runner::default(),
            running: None,
            last_uid: None,
            cli: Vec::new(),
            cli_note: String::new(),
            groups: None,
            failures: Vec::new(),
        }
    }

    /// Les fiches de la bibliothèque ont changé : la fiche choisie le reste, retrouvée par son nom.
    pub fn refresh(&mut self, list: Vec<DroneCard>) {
        let keep = self.card().map(|c| c.name.clone());
        self.cards.list = list;
        match keep {
            Some(n) => self.select_by_name(&n),
            None => self.selected = if self.cards.list.is_empty() { None } else { Some(0) },
        }
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
            self.old_roles = r.into_iter().filter(|(_, v)| role_static(v).is_some()).collect();
        }
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "guide.phase", &self.phase.to_string());
        eframe::set_value(
            storage,
            "guide.card",
            &self.card().map(|c| c.name.clone()).unwrap_or_default(),
        );
        eframe::set_value(storage, "guide.roles", &self.old_roles);
    }

    /// (uid, rôle) de chaque log que le Guide a vérifié au dernier cadre, dans l'ordre de la barre : pour les tests.
    pub fn seen(&self) -> &[(u64, &'static str)] {
        &self.seen
    }

    // ------------------------------------------------------------ logs du drone et leur vérification
    /// Les logs ouverts du drone de la fiche choisie, actifs ou non.
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
    ///
    /// ⚠️ Le rôle d'un log est celui de **son fragment** dans la bibliothèque, pas celui de son fichier : deux fragments
    /// d'une même session peuvent servir à deux phases (W-074). Un fragment désactivé dans la barre n'est pas vérifié,
    /// comme sur toutes les pages (D-066).
    fn views(&mut self, ctx: &Context, state: &AppState, lib: &Library) -> Vec<LogView> {
        let inch = self.inch();
        let mut out = Vec::new();
        let mut budget = 1;
        self.failures.clear();
        self.inactive.clear();
        let mut wanted: Option<(usize, String)> = None;
        for i in self.card_logs(state) {
            let ll = &state.logs[i];
            let stored = ll.fragment.as_deref().and_then(|id| lib.fragment(id));
            let label = stored.map_or_else(|| ll.label.clone(), |f| f.name.clone());
            if !ll.active {
                self.inactive.push(label);
                continue;
            }
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
            let role = stored.map(|f| f.role.as_str()).filter(|r| !r.is_empty());
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
                    label,
                    color: ll.color,
                    fragment: ll.fragment.clone(),
                    check: with_role(&g.check, role),
                    targets: g.targets.clone(),
                    exact: g.exact,
                    error,
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
                actions.push(GuideAction::NewDrone(
                    state.current_log().map(|l| card_from_log(&l.log)),
                ));
            }
            Intent::EditCard => {
                if let Some(c) = self.card() {
                    actions.push(GuideAction::EditDrone(c.name.clone()));
                }
            }
            Intent::Done(key, on) => {
                if let Some(i) = self.selected {
                    // la copie tout de suite (l'image suivante la montre), la bibliothèque par `App`
                    self.cards.set_done(i, &key, on);
                    let name = self.cards.list[i].name.clone();
                    actions.push(GuideAction::Done { name, key, on });
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
                    actions.push(GuideAction::Open(files, Some(role)));
                }
            }
            Intent::Role(fragment, role) => {
                if let Some(fragment) = fragment {
                    actions.push(GuideAction::Role { fragment, role });
                }
            }
            Intent::CopyCli => {
                ctx.copy_text(cli_text(&self.cli));
                self.cli_note = format!("{} commande(s) copiée(s), suivies de « save ».", self.cli.len());
            }
        }
    }

    // ------------------------------------------------------------ dessin
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, lib: &Library, th: &Theme) -> Vec<GuideAction> {
        let ctx = ui.ctx().clone();
        self.follow_logs(state);
        self.poll_checks(&ctx, state);
        let views = self.views(&ctx, state, lib);
        self.seen = views.iter().map(|v| (state.logs[v.index].uid, v.check.role)).collect();
        let card = self.card().cloned();
        let (checks, targets) = Self::card_checks(&views);
        let mut actions = Vec::new();
        let verified = card
            .as_ref()
            .map(|c| verified_keys(c, &checks, targets))
            .unwrap_or_default();
        if let Some(c) = &card {
            self.report(c, state, &views, verified.clone(), &mut actions);
        }
        let progress = card.as_ref().map(|c| progress_with(c, &verified)).unwrap_or_default();
        let mut intents: Vec<Intent> = Vec::new();

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
        self.dialogs(&ctx, &mut actions);
        for it in intents {
            self.run(it, &ctx, state, &views, &mut actions);
        }
        actions
    }

    /// Rend le relevé des cases vérifiées quand il a changé (D-071). Seulement quand **chaque** fragment actif du drone a
    /// sa vérification, rapide au moins : avant, un log pas encore vérifié retirerait ses cases pour un cadre. Sans
    /// fragment actif du drone, rien n'est rendu : la carte garde le dernier relevé.
    fn report(
        &mut self,
        card: &DroneCard,
        state: &AppState,
        views: &[LogView],
        keys: Vec<String>,
        actions: &mut Vec<GuideAction>,
    ) {
        let active = self
            .card_logs(state)
            .into_iter()
            .filter(|&i| state.logs[i].active)
            .count();
        if views.is_empty() || views.len() + self.failures.len() < active {
            return;
        }
        let now = (card.name.clone(), keys);
        if self.reported.as_ref() != Some(&now) {
            actions.push(GuideAction::Verified {
                name: now.0.clone(),
                keys: now.1.clone(),
            });
            self.reported = Some(now);
        }
    }

    /// (vérifications des logs du drone, repères du dernier log des filtres) comme `_card_checks`.
    fn card_checks(views: &[LogView]) -> (Vec<LogCheck>, Option<&FilterTargets>) {
        let checks = views.iter().map(|v| v.check.clone()).collect();
        let roles: Vec<&str> = views.iter().map(|v| v.check.role).collect();
        let targets = last_index_with_role(&roles, FILTERS).and_then(|i| views[i].targets.as_ref());
        (checks, targets)
    }

    fn header(&mut self, ui: &mut Ui, th: &Theme, progress: &[(String, usize, usize)], intents: &mut Vec<Intent>) {
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
            if ui
                .button("Nouveau drone")
                .on_hover_text("Ouvrir une fiche de drone neuve, préremplie d'après le log ouvert")
                .clicked()
            {
                intents.push(Intent::NewCard);
            }
            let has = self.card().is_some();
            if ui
                .add_enabled(has, egui::Button::new("Modifier"))
                .on_hover_text("Ouvrir la fiche de ce drone (on la supprime aussi depuis là)")
                .clicked()
            {
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
            if !self.inactive.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(format!(
                        "Désactivés dans la barre, non vérifiés : {}.",
                        self.inactive.join(", ")
                    ))
                    .color(muted),
                );
            }
            // les fragments désactivés du drone ne sont pas « d'un autre drone »
            let mine: Vec<usize> = views
                .iter()
                .map(|v| v.index)
                .chain(self.card_logs(state).into_iter().filter(|&i| !state.logs[i].active))
                .collect();
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
            // sans fragment (log rangé sous aucun drone), rien ne garderait le rôle : pas de lien
            if v.fragment.is_some()
                && ui
                    .link(format!("→ classer dans les {}", role_name(other_role(role))))
                    .clicked()
            {
                intents.push(Intent::Role(v.fragment.clone(), other_role(role)));
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

    /// La confirmation de « Recommencer ».
    fn dialogs(&mut self, ctx: &Context, actions: &mut Vec<GuideAction>) {
        let Some(dialog) = self.dialog.take() else { return };
        let Dialog::Reset { index, ref name } = dialog;
        let name = name.clone();
        let mut answer = None;
        let m = egui::Modal::new(egui::Id::unique("guide-confirmer")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Recommencer le parcours");
            ui.label(format!(
                "Décocher toutes les cases de « {name} » ? Les cases vérifiées par un log ouvert resteront cochées \
                 tant que ce log est ouvert."
            ));
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
            // seulement si la fiche du clic est toujours là sous ce nom (sinon la liste a changé : on ne touche à rien)
            Some(true) if self.cards.list.get(index).is_some_and(|c| c.name == name) => {
                self.cards.reset(index);
                // `App` efface aussi le relevé : celui des logs encore ouverts est rendu de nouveau au cadre suivant
                self.reported = None;
                actions.push(GuideAction::Reset(name));
            }
            Some(_) => {}
            None => self.dialog = Some(dialog),
        }
    }
}
