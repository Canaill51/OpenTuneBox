//! Outil « Assistant », en cartes comme le prototype (D-061 ; lot 30f, D-070) : l'en-tête du fragment analysé, une pastille par
//! gravité, une carte par constat triée par gravité (la mesure contre son seuil en jauge, les « info » repliées), les vérifications
//! OK regroupées sur une ligne qu'on déplie, et à droite la réponse indicielle de chaque axe en petit.
//!
//! Lot 31d (D-073) : le badge du drone et le vol **comparé** dans l'en-tête (par défaut le vol précédent du drone),
//! un repère du vol comparé sur chaque jauge, les cartes nouvelles marquées et celles qui ont disparu listées ; un
//! pouce levé (« appliqué ») ou baissé (« écarté », avec sa raison) par conseil, inscrit tout de suite dans la
//! bibliothèque, et le bandeau « Fragment déjà traité » quand le drone a volé depuis. Calculs : `crate::advice`.

use std::collections::HashMap;
use std::sync::Arc;

use otb_store::Library;
use otb_store::model::Advice;

use egui::{Color32, FontId, RichText, Sense, Stroke, Ui, vec2};
use egui_plot::{FilledArea, HLine, Line, LineStyle, Plot, PlotPoints};
use otb_advisor::profiles::{PROFILES, profile};
use otb_advisor::rules::{self, Finding, Gauge, Report, analyze};

use crate::advice::{self, CompareTarget, Comparison, HistoryLine};
use crate::icons::icon;
use crate::plots::nice_axes;
use crate::state::{AppState, Famille};
use crate::theme::{Theme, file_color, with_alpha};
use crate::tool::{LibraryEdit, LogInput, Tool};
use crate::tools::common::{FileSelection, safe_text};

pub const HELP: &str = "Les conseils portent sur la période d'analyse : les limites du fragment de chaque log (crayon d'un \
     fragment au tableau de bord). Changez une seule chose à la fois, revolez, et comparez les logs.";
pub const FOOTER: &str = "Diagnostic automatique à partir des mesures du log : à confirmer par le ressenti en vol et la température des moteurs.";
/// Rappel à droite des pastilles (prototype).
pub const ONE_CHANGE: &str = "Changez une seule chose à la fois, revolez, comparez.";
/// Titre du panneau de droite.
pub const STEP_TITLE: &str = "RÉPONSE INDICIELLE";

// Mesures du prototype (`Application.dc.html`, écran Assistant).
const PAGE_PAD: egui::Margin = egui::Margin {
    left: 28,
    right: 28,
    top: 22,
    bottom: 22,
};
const PAGE_GAP: f32 = 14.0;
const CARD_PAD: egui::Margin = egui::Margin {
    left: 18,
    right: 18,
    top: 16,
    bottom: 16,
};
const CARD_RADIUS: u8 = 10;
const CARD_GAP_IN: f32 = 12.0;
const TITLE_SIZE: f32 = 16.0;
const BODY_SIZE: f32 = 13.5;
const BADGE_SIZE: f32 = 11.0;
const SMALL: f32 = 12.5;
const TINY: f32 = 12.0;
/// Le « pourquoi » d'une carte ne dépasse pas cette largeur ; la jauge en a une fixe, à 28 points de lui.
const WHY_MAX: f32 = 440.0;
const GAUGE_W: f32 = 300.0;
const GAUGE_H: f32 = 24.0;
const GAUGE_GAP: f32 = 28.0;
const PILL_DOT: f32 = 8.0;
/// Panneau de droite : 300 points, petites courbes de 130 de haut.
const PLOTS_WIDTH: f32 = 300.0;
const PLOT_H: f32 = 130.0;
/// Durée affichée par défaut quand un axe n'a aucune réponse (ms).
const EMPTY_X_MS: f64 = 500.0;

/// Sous « comparé à », sans vol à comparer (D-073 : pas de « drones de même taille », les seuils le sont déjà).
pub const NO_COMPARE: &str = "Aucun vol à comparer";
pub const NO_COMPARE_META: &str = "pas de vol précédent pour ce drone";
/// Sous une carte appliquée (prototype).
pub const FLY_AGAIN: &str = "Revolez avec le même parcours, puis comparez le nouveau fragment à celui-ci.";
pub const REASON_QUESTION: &str = "Pourquoi ce conseil ne convient pas ?";
pub const REASON_HINT: &str = "Ex. : déjà essayé, le drone oscille plus ; ou le bruit vient d'une hélice abîmée.";
pub const NO_HISTORY: &str = "Aucun conseil de ce fragment n'a été appliqué ni écarté.";

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// Clé du profil choisi (`None` : détection automatique).
    pub profile: Option<&'static str>,
    /// Le vol auquel comparer, résolu dans la bibliothèque (`follow_state`).
    pub compare: Option<CompareTarget>,
}

pub struct AdvisorOutput {
    pub label: String,
    pub color: egui::Color32,
    pub firmware: String,
    pub report: Arc<Report>,
    /// Fragment de la bibliothèque analysé : sans lui, ni vote ni historique.
    pub fragment: Option<String>,
    /// Le vol comparé et ce qui a changé depuis lui, ou la raison pour laquelle il n'a pas pu s'analyser.
    pub compare: Option<(CompareTarget, Result<Comparison, String>)>,
}

/// Les cartes, triées par gravité (à gravité égale, l'ordre des règles), et à part les vérifications OK.
pub fn split_cards(report: &Report) -> (Vec<Finding>, Vec<Finding>) {
    report.sorted().into_iter().partition(|f| f.severity != rules::OK)
}

/// Une carte s'ouvre d'elle-même, sauf une « info » (prototype : repliée, son titre suffit).
pub fn open_by_default(f: &Finding) -> bool {
    f.severity != rules::INFO
}

/// Identité d'une carte pour garder son état replié : la clé et l'axe, jamais le titre (D-028).
pub fn card_id(f: &Finding) -> String {
    format!("{}|{}", f.key, f.axis.as_deref().unwrap_or(""))
}

/// Pastilles en tête : une par gravité, dans l'ordre de gravité ; l'alerte, absente du prototype, seulement si elle sert.
pub fn pills(findings: &[Finding]) -> Vec<(&'static str, String)> {
    rules::SEVERITY_ORDER
        .iter()
        .filter_map(|(sev, _)| {
            let n = findings.iter().filter(|f| f.severity == *sev).count();
            if *sev == rules::ALERTE && n == 0 {
                return None;
            }
            let word = match *sev {
                rules::ALERTE => plural(n, "alerte", "alertes"),
                rules::CONSEIL => plural(n, "conseil", "conseils"),
                rules::ATTENTION => "attention",
                rules::INFO => "info",
                _ => "OK",
            };
            Some((*sev, format!("{n} {word}")))
        })
        .collect()
}

fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n > 1 { many } else { one }
}

/// Ligne des vérifications OK : « 4 vérifications OK » et la liste de leurs titres, en minuscules (prototype).
pub fn oks_line(oks: &[Finding]) -> (String, String) {
    let n = oks.len();
    let list: Vec<String> = oks.iter().map(|f| lower_first(&f.title)).collect();
    (
        format!("{n} {} OK", plural(n, "vérification", "vérifications")),
        list.join(", "),
    )
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        // un sigle (« RPM », « PID ») garde ses capitales
        Some(first) if !c.clone().next().is_some_and(char::is_uppercase) => first.to_lowercase().chain(c).collect(),
        _ => s.to_string(),
    }
}

/// « pic 1,04 · 13,8 ms » au-dessus de chaque petite courbe (prototype) ; « données insuffisantes » sans fenêtre.
pub fn step_text(r: &otb_analysis::step::StepResult) -> String {
    if r.n == 0 {
        return "données insuffisantes".to_string();
    }
    format!(
        "pic {} · {} ms",
        comma(otb_format::fixed(r.metrics.peak, 2, false)),
        comma(otb_format::fixed(r.metrics.latency_ms, 1, false))
    )
}

fn comma(s: String) -> String {
    s.replace('.', ",")
}

pub struct AdvisorTool {
    profile: Option<&'static str>,
    files: FileSelection,
    /// Cartes repliées ou dépliées à la main (`card_id`) ; les autres suivent `open_by_default`.
    open: HashMap<String, bool>,
    /// Vérifications OK dépliées.
    oks_open: bool,
    /// Le vol choisi dans « comparé à » (identifiant de fragment) et le fragment analysé à ce moment : un autre
    /// fragment analysé revient au vol précédent (`None`).
    pick: Option<(String, String)>,
    /// Le vol comparé, résolu à chaque image d'après la bibliothèque.
    compare: Option<CompareTarget>,
    /// Le fragment analysé (premier fragment actif), lu avec `compare`.
    analysed: Option<String>,
    /// Raisons en cours d'écriture d'un conseil écarté, par fragment et carte : rien n'est inscrit avant « Enregistrer ».
    drafts: HashMap<(String, String), String>,
    /// Ce que la page a lu dans la bibliothèque pour le fragment analysé.
    view: LibView,
    /// Votes à inscrire dans la bibliothèque, rendus à la fenêtre après l'affichage.
    edits: Vec<LibraryEdit>,
}

/// Le badge du drone dans l'en-tête (prototype) : nom, sous-titre et rang (sa couleur, comme au tableau de bord).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DroneBadge {
    pub name: String,
    pub sub: String,
    pub rank: usize,
}

/// Ce que la page Assistant lit dans la bibliothèque à chaque image, pour le fragment analysé (`read_library`) : la
/// bibliothèque appartient à la fenêtre, la page n'en garde que ceci.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LibView {
    pub drone: Option<DroneBadge>,
    /// Date du fragment analysé.
    pub date: String,
    /// Les vols au choix de « comparé à » : identifiant, nom, sous-titre.
    pub candidates: Vec<(String, String, String)>,
    /// « · 2 vols plus récents : … » ; vide si le drone n'a pas volé depuis.
    pub newer: String,
    /// Les choix faits sur ses conseils, dans l'ordre où ils ont été faits.
    pub history: Vec<HistoryLine>,
    /// Le choix fait sur chaque carte (`card_id`) et sa ligne d'historique.
    pub votes: HashMap<String, (Advice, HistoryLine)>,
}

impl LibView {
    pub fn read(lib: &Library, fragment_id: &str) -> Self {
        let drone = advice::drone_of(lib, fragment_id).map(|d| {
            let sheet: Vec<&str> = d
                .sheet
                .iter()
                .flat_map(|s| [s.size.as_str(), s.cells.as_str()])
                .filter(|t| !t.is_empty())
                .collect();
            let sub = if sheet.is_empty() {
                d.firmware.split_whitespace().take(2).collect::<Vec<_>>().join(" ")
            } else {
                sheet.join(" · ")
            };
            DroneBadge {
                name: d.name.clone(),
                sub,
                rank: lib.drones().iter().position(|x| x.id == d.id).unwrap_or(0),
            }
        });
        let candidates = advice::candidates(lib, fragment_id)
            .into_iter()
            .map(|f| {
                let drone = lib
                    .log(&f.log_id)
                    .and_then(|l| lib.drone(&l.drone_id))
                    .map_or(String::new(), |d| d.name.clone());
                (f.id.clone(), advice::fragment_name(f), format!("{drone} · {}", f.date))
            })
            .collect();
        let votes = lib
            .advice_of(fragment_id)
            .into_iter()
            .map(|a| (a.card.clone(), (a.clone(), advice::history_line(lib, a))))
            .collect();
        LibView {
            drone,
            date: lib.fragment(fragment_id).map(|f| f.date.clone()).unwrap_or_default(),
            candidates,
            newer: advice::newer_text(&advice::newer_flights(lib, fragment_id)),
            history: advice::history(lib, fragment_id),
            votes,
        }
    }
}

impl Default for AdvisorTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvisorTool {
    pub fn new() -> Self {
        Self {
            profile: None,
            files: FileSelection::new(false),
            open: HashMap::new(),
            oks_open: false,
            pick: None,
            compare: None,
            analysed: None,
            drafts: HashMap::new(),
            view: LibView::default(),
            edits: Vec::new(),
        }
    }

    fn profile_label(key: Option<&'static str>) -> &'static str {
        key.and_then(profile).map_or("Détection automatique", |p| p.label)
    }

    /// En-tête (prototype) : le badge du drone, le fragment analysé à sa couleur (drone, période, firmware), puis
    /// « comparé à » et le vol comparé, qu'un menu change. `cmp_color` : la couleur du vol comparé s'il est ouvert.
    fn header(&mut self, ui: &mut Ui, th: &Theme, out: &AdvisorOutput, cmp_color: Option<Color32>) {
        let rep = &out.report;
        let meta = format!(
            "{} ({}) · période {} – {} s · {}",
            rep.profile.label,
            rep.profile_reason,
            comma(otb_format::fixed(rep.period.0, 1, false)),
            comma(otb_format::fixed(rep.period.1, 1, false)),
            out.firmware
        );
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            if let Some(d) = &self.view.drone {
                drone_badge(ui, th, d);
                ui.add(icon("chevron-right", 16.0).tint(th.ui.mid));
            }
            egui::Frame::new()
                .fill(th.ui.base)
                .stroke(Stroke::new(1.5, out.color))
                .corner_radius(12)
                .inner_margin(egui::Margin {
                    left: 14,
                    right: 14,
                    top: 8,
                    bottom: 8,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 8.0, out.color);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(RichText::new(safe_text(&out.label)).size(TITLE_SIZE).strong());
                            ui.label(RichText::new(safe_text(&meta)).size(SMALL).color(th.ui.muted));
                        });
                    });
                });
            if out.fragment.is_some() {
                ui.label(RichText::new("comparé à").size(SMALL).color(th.ui.muted));
                self.compare_ui(ui, th, out, cmp_color);
            }
        });
    }

    /// Le vol comparé, cadre pointillé (prototype) : son nom dans un menu (le vol précédent d'office, ou un autre vol),
    /// et dessous d'où il vient — ou pourquoi il n'a pas pu s'analyser.
    fn compare_ui(&mut self, ui: &mut Ui, th: &Theme, out: &AdvisorOutput, cmp_color: Option<Color32>) {
        let (name, meta) = match &out.compare {
            Some((t, Ok(_))) => (t.name.clone(), t.meta.clone()),
            Some((t, Err(e))) => (t.name.clone(), format!("analyse impossible : {e}")),
            None => (NO_COMPARE.to_string(), NO_COMPARE_META.to_string()),
        };
        let resp = egui::Frame::new()
            .fill(th.ui.base)
            .corner_radius(12)
            .inner_margin(egui::Margin {
                left: 14,
                right: 12,
                top: 8,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let (r, _) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::hover());
                    ui.painter()
                        .circle_filled(r.center(), 7.5, cmp_color.unwrap_or(th.ui.mid));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let automatic = self.pick.is_none();
                        let mut choice: Option<Option<String>> = None;
                        egui::ComboBox::from_id_salt("assistant-compare")
                            .selected_text(RichText::new(safe_text(&name)).size(14.0).strong())
                            .show_ui(ui, |ui| {
                                ui.label(RichText::new("COMPARER À").size(11.0).strong().color(th.ui.muted));
                                if ui.selectable_label(automatic, "Vol précédent (automatique)").clicked() {
                                    choice = Some(None);
                                }
                                for (id, n, m) in &self.view.candidates {
                                    let sel = self.pick.as_ref().is_some_and(|(p, _)| p == id);
                                    let text = format!("{} · {}", safe_text(n), safe_text(m));
                                    if ui.selectable_label(sel, text).clicked() {
                                        choice = Some(Some(id.clone()));
                                    }
                                }
                            })
                            .response
                            .accessible_name("Vol comparé")
                            .on_hover_text("Changer le fragment de comparaison");
                        if let Some(c) = choice {
                            self.pick = c.zip(self.analysed.clone());
                        }
                        ui.label(RichText::new(safe_text(&meta)).size(TINY).color(th.ui.muted));
                    });
                });
            })
            .response;
        dashed_rect(ui, resp.rect, Stroke::new(1.0, th.ui.mid));
    }

    /// « Fragment déjà traité » (prototype) : le drone a volé depuis ; les choix faits sur ses conseils, ou aucun.
    fn treated_ui(&self, ui: &mut Ui, th: &Theme) {
        if self.view.newer.is_empty() {
            return;
        }
        egui::Frame::new()
            .fill(th.ui.alt)
            .stroke(Stroke::new(1.0, th.ui.border))
            .corner_radius(CARD_RADIUS)
            .inner_margin(egui::Margin::symmetric(16, 14))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.add(icon("clock", 16.0).tint(th.ui.muted));
                    ui.label(RichText::new("Fragment déjà traité").size(BODY_SIZE).strong());
                    ui.label(
                        RichText::new(safe_text(&self.view.newer))
                            .size(BODY_SIZE)
                            .color(th.ui.muted),
                    );
                });
                for h in &self.view.history {
                    history_ui(ui, th, h);
                }
                if self.view.history.is_empty() {
                    ui.label(RichText::new(NO_HISTORY).size(13.0).color(th.ui.muted));
                }
            });
    }

    fn pills_ui(ui: &mut Ui, th: &Theme, findings: &[Finding]) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (sev, text) in pills(findings) {
                egui::Frame::new()
                    .fill(th.ui.base)
                    .stroke(Stroke::new(1.0, th.ui.border))
                    .corner_radius(14)
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 10,
                        top: 4,
                        bottom: 4,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let (r, _) = ui.allocate_exact_size(vec2(PILL_DOT, PILL_DOT), Sense::hover());
                            ui.painter().circle_filled(r.center(), PILL_DOT / 2.0, th.severity(sev));
                            ui.label(RichText::new(text).size(13.0));
                        });
                    });
            }
            ui.label(RichText::new(ONE_CHANGE).size(SMALL).color(th.ui.muted));
        });
    }

    /// Une carte : gravité, titre (« nouveau » s'il n'était pas sur le vol comparé), pouces et bouton qui la replie ;
    /// dépliée, le pourquoi, la jauge (avec le repère du vol comparé) et les gestes conseillés, puis le choix du pilote.
    fn card(&mut self, ui: &mut Ui, th: &Theme, f: &Finding, ctx: &CardContext) {
        let id = card_id(f);
        let is_new = ctx.cmp.is_some_and(|c| c.new_cards.contains(&id));
        let previous = ctx.cmp.and_then(|c| c.values.get(&id).map(|v| (*v, c.name.as_str())));
        let vote = ctx.fragment.filter(|_| advice::can_vote(f));
        let open = self.open.get(&id).copied().unwrap_or_else(|| open_by_default(f));
        let col = th.severity(&f.severity);
        let mut toggle = false;
        egui::Frame::new()
            .fill(th.ui.base)
            .stroke(Stroke::new(1.0, th.ui.border))
            .corner_radius(CARD_RADIUS)
            .inner_margin(CARD_PAD)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = CARD_GAP_IN;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.label(
                        RichText::new(format!(" {} ", f.severity.to_uppercase()))
                            .size(BADGE_SIZE)
                            .strong()
                            .background_color(col)
                            .color(th.on_accent("")),
                    );
                    ui.label(RichText::new(safe_text(&f.title)).size(TITLE_SIZE).strong());
                    if is_new {
                        ui.label(RichText::new(" NOUVEAU ").size(BADGE_SIZE).strong().color(th.ui.muted))
                            .on_hover_text(format!(
                                "Absent de {}",
                                safe_text(ctx.cmp.map_or("", |c| c.name.as_str()))
                            ));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (glyph, verb) = if open {
                            ("chevron-up", "Replier")
                        } else {
                            ("chevron-down", "Déplier")
                        };
                        let alt = format!("{verb} « {} »", safe_text(&f.title));
                        if ui
                            .add(egui::Button::image(icon(glyph, 15.0).tint(th.ui.muted).alt_text(alt)).frame(false))
                            .clicked()
                        {
                            toggle = true;
                        }
                        if let Some(frag) = vote {
                            self.votes_ui(ui, th, f, frag);
                        }
                    });
                });
                if open {
                    Self::card_body(ui, th, f, col, previous);
                    if let Some(frag) = vote {
                        self.choice_ui(ui, th, f, frag);
                    }
                }
            });
        if toggle {
            self.open.insert(id, !open);
        }
    }

    /// Les deux pouces d'un conseil (de droite à gauche : baissé puis levé). Un clic sur le pouce déjà enfoncé annule
    /// le choix ; le pouce levé s'inscrit tout de suite, le baissé ouvre la raison à écrire.
    fn votes_ui(&mut self, ui: &mut Ui, th: &Theme, f: &Finding, frag: &str) {
        let id = card_id(f);
        let key = (frag.to_string(), id.clone());
        let current = self.view.votes.get(&id).map(|(a, _)| a.applied);
        let drafting = self.drafts.contains_key(&key);
        let down_on = current == Some(false) || drafting;
        let up_on = current == Some(true) && !drafting;
        let thumb = |ui: &mut Ui, glyph: &str, on: bool, accent: Color32, alt: &str| {
            let (fill, tint, stroke) = if on {
                (accent, th.on_accent(""), accent)
            } else {
                (th.ui.base, th.ui.muted, th.ui.mid)
            };
            ui.add(
                egui::Button::image(icon(glyph, 15.0).tint(tint).alt_text(alt))
                    .fill(fill)
                    .stroke(Stroke::new(1.0, stroke))
                    .corner_radius(8)
                    .selected(on),
            )
            .on_hover_text(alt)
            .clicked()
        };
        let down = thumb(
            ui,
            "thumbs-down",
            down_on,
            th.accent("red"),
            "Conseil écarté : indiquer la raison",
        );
        let up = thumb(
            ui,
            "thumbs-up",
            up_on,
            th.accent("green"),
            "Conseil appliqué : je l'ai réglé sur le drone",
        );
        if up {
            self.drafts.remove(&key);
            if current == Some(true) {
                self.edits.push(LibraryEdit::RemoveAdvice {
                    fragment: frag.to_string(),
                    card: id,
                });
            } else {
                self.edits.push(LibraryEdit::SetAdvice(Advice {
                    fragment_id: frag.to_string(),
                    card: id,
                    title: f.title.clone(),
                    applied: true,
                    reason: String::new(),
                    when: advice::now_iso(),
                }));
            }
        } else if down {
            if drafting {
                self.drafts.remove(&key);
            } else if current == Some(false) {
                self.edits.push(LibraryEdit::RemoveAdvice {
                    fragment: frag.to_string(),
                    card: id,
                });
            } else {
                self.drafts.insert(key, String::new());
            }
            // la carte s'ouvre pour montrer la raison à écrire
            self.open.insert(card_id(f), true);
        }
    }

    /// Sous les gestes d'une carte : le choix inscrit (appliqué, ou écarté et sa raison), ou la raison en cours d'écriture.
    fn choice_ui(&mut self, ui: &mut Ui, th: &Theme, f: &Finding, frag: &str) {
        let id = card_id(f);
        let key = (frag.to_string(), id.clone());
        if let Some(draft) = self.drafts.get_mut(&key) {
            let mut save = false;
            let mut cancel = false;
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.label(RichText::new(REASON_QUESTION).size(BODY_SIZE).strong());
                ui.add(
                    egui::TextEdit::multiline(draft)
                        .hint_text(REASON_HINT)
                        .desired_rows(3)
                        .desired_width(ui.available_width().min(WHY_MAX * 1.6)),
                )
                .accessible_name(REASON_QUESTION);
                ui.horizontal(|ui| {
                    let ok = !draft.trim().is_empty();
                    save = ui
                        .add_enabled(ok, egui::Button::new("Enregistrer"))
                        .on_disabled_hover_text("Indiquez la raison pour l'inscrire à l'historique")
                        .clicked();
                    cancel = ui.button("Annuler").clicked();
                });
            });
            if save {
                let reason = draft.trim().to_string();
                self.drafts.remove(&key);
                self.edits.push(LibraryEdit::SetAdvice(Advice {
                    fragment_id: frag.to_string(),
                    card: id,
                    title: f.title.clone(),
                    applied: false,
                    reason,
                    when: advice::now_iso(),
                }));
            } else if cancel {
                self.drafts.remove(&key);
            }
            return;
        }
        let Some((a, h)) = self.view.votes.get(&id) else {
            return;
        };
        let col = if a.applied {
            th.accent("green")
        } else {
            th.accent("red")
        };
        let mut edit = None;
        egui::Frame::new()
            .fill(with_alpha(col, 20))
            .stroke(Stroke::new(1.0, with_alpha(col, 90)))
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let glyph = if a.applied { "check" } else { "x" };
                    ui.add(icon(glyph, 14.0).tint(col));
                    ui.label(RichText::new(&h.status).size(13.0).strong().color(col));
                    if a.applied {
                        ui.label(RichText::new(format!("· {}", lower_first(&h.detail))).size(13.0));
                    }
                });
                if a.applied {
                    ui.label(RichText::new(FLY_AGAIN).size(TINY).color(th.ui.muted));
                } else {
                    ui.add(egui::Label::new(RichText::new(safe_text(&h.detail)).size(13.0)).wrap());
                    if ui.small_button("Modifier la raison").clicked() {
                        edit = Some(a.reason.clone());
                    }
                }
            });
        if let Some(reason) = edit {
            self.drafts.insert(key, reason);
        }
    }

    fn card_body(ui: &mut Ui, th: &Theme, f: &Finding, col: Color32, previous: Option<(f64, &str)>) {
        let gauge = f.gauge.as_ref().filter(|g| g.marks().is_some());
        let avail = ui.available_width();
        let side_by_side = gauge.is_none() || avail >= GAUGE_W + GAUGE_GAP + 220.0;
        let why_w = if gauge.is_some() && side_by_side {
            (avail - GAUGE_W - GAUGE_GAP).min(WHY_MAX)
        } else {
            avail.min(WHY_MAX * 1.6)
        };
        let why = |ui: &mut Ui| {
            if !f.detail.is_empty() {
                // ⚠️ pas `add_sized` : il centre chaque ligne du texte replié
                ui.vertical(|ui| {
                    ui.set_width(why_w);
                    ui.add(egui::Label::new(RichText::new(safe_text(&f.detail)).size(BODY_SIZE)).wrap());
                });
            }
        };
        match gauge {
            Some(g) if side_by_side => {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = GAUGE_GAP;
                    ui.vertical(|ui| {
                        ui.set_width(why_w);
                        why(ui);
                    });
                    gauge_ui(ui, th, g, col, previous);
                });
            }
            Some(g) => {
                why(ui);
                gauge_ui(ui, th, g, col, previous);
            }
            None => why(ui),
        }
        if !f.actions.is_empty() {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 7.0;
                for a in &f.actions {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        ui.label(RichText::new("→").size(BODY_SIZE).color(col));
                        ui.add(egui::Label::new(RichText::new(safe_text(a)).size(BODY_SIZE)).wrap());
                    });
                }
            });
        }
    }

    /// La ligne des vérifications OK, bord pointillé ; un clic la déplie en cartes.
    fn oks_ui(&mut self, ui: &mut Ui, th: &Theme, oks: &[Finding]) {
        if oks.is_empty() {
            return;
        }
        let (head, list) = oks_line(oks);
        let green = th.severity(rules::OK);
        let mut toggle = false;
        let resp = egui::Frame::new()
            .corner_radius(CARD_RADIUS)
            .inner_margin(egui::Margin {
                left: 18,
                right: 18,
                top: 12,
                bottom: 12,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    ui.add(icon("check", 16.0).tint(green));
                    ui.label(RichText::new(&head).size(BODY_SIZE).strong().color(green));
                    ui.add(
                        egui::Label::new(
                            RichText::new(format!("· {}", safe_text(&list)))
                                .size(BODY_SIZE)
                                .color(th.ui.muted),
                        )
                        .truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (glyph, alt) = if self.oks_open {
                            ("chevron-up", "Masquer les vérifications OK")
                        } else {
                            ("chevron-down", "Afficher les vérifications OK")
                        };
                        if ui
                            .add(egui::Button::image(icon(glyph, 15.0).tint(th.ui.muted).alt_text(alt)).frame(false))
                            .clicked()
                        {
                            toggle = true;
                        }
                    });
                });
            })
            .response;
        dashed_rect(ui, resp.rect, Stroke::new(1.0, th.ui.mid));
        if toggle {
            self.oks_open = !self.oks_open;
        }
        if self.oks_open {
            for f in oks {
                self.card(ui, th, f, &CardContext::default());
            }
        }
    }

    fn report_ui(&mut self, ui: &mut Ui, th: &Theme, out: &AdvisorOutput, cmp_color: Option<Color32>) {
        ui.spacing_mut().item_spacing.y = PAGE_GAP;
        self.header(ui, th, out, cmp_color);
        self.treated_ui(ui, th);
        Self::pills_ui(ui, th, &out.report.findings);
        let (cards, oks) = split_cards(&out.report);
        let cmp = match &out.compare {
            Some((_, Ok(c))) => Some(c),
            _ => None,
        };
        let ctx = CardContext {
            cmp,
            fragment: out.fragment.as_deref(),
        };
        for f in &cards {
            self.card(ui, th, f, &ctx);
        }
        if let Some(c) = cmp
            && !c.gone.is_empty()
        {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add(icon("check", 15.0).tint(th.severity(rules::OK)));
                ui.label(
                    RichText::new(safe_text(&gone_text(c)))
                        .size(BODY_SIZE)
                        .color(th.ui.muted),
                );
            });
        }
        self.oks_ui(ui, th, &oks);
        ui.label(RichText::new(FOOTER).size(SMALL).color(th.ui.muted));
    }

    fn plots_ui(ui: &mut Ui, theme: &Theme, out: &AdvisorOutput) {
        ui.label(RichText::new(STEP_TITLE).size(11.0).strong().color(theme.ui.muted));
        for (row, st) in out.report.steps.iter().enumerate() {
            let r = &st.result;
            ui.horizontal(|ui| {
                ui.label(RichText::new(safe_text(&st.axis)).size(TINY).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(step_text(r))
                            .font(FontId::monospace(TINY))
                            .color(theme.ui.muted),
                    )
                    .on_hover_text(format!("{} fenêtres exploitables", r.n));
                });
            });
            let x_hi = r.t_ms.last().copied().unwrap_or(EMPTY_X_MS);
            let x_lo = r.t_ms.first().copied().unwrap_or(0.0);
            let plot = nice_axes(Plot::new(("assistant-reponse", row)))
                .width(ui.available_width())
                .height(PLOT_H)
                .auto_bounds(false)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .grid_color(theme.grid_color());
            plot.show(ui, |pu| {
                pu.set_plot_bounds_x(x_lo..=x_hi);
                pu.set_plot_bounds_y(0.0..=1.5);
                pu.hline(
                    HLine::new("", 1.0)
                        .color(theme.fg)
                        .width(1.0)
                        .style(LineStyle::dashed_loose()),
                );
                if r.n > 0 {
                    let lo: Vec<f64> = r.mean.iter().zip(&r.std).map(|(m, s)| m - s).collect();
                    let hi: Vec<f64> = r.mean.iter().zip(&r.std).map(|(m, s)| m + s).collect();
                    pu.add(FilledArea::new("", &r.t_ms, &lo, &hi).fill_color(with_alpha(out.color, 40)));
                    let pts: Vec<[f64; 2]> = r.t_ms.iter().zip(&r.mean).map(|(t, m)| [*t, *m]).collect();
                    pu.line(Line::new("", PlotPoints::from(pts)).color(out.color).width(2.0));
                }
            });
            ui.add_space(6.0);
        }
    }
}

/// La jauge : piste, bande teintée entre la mesure et le seuil, seuil en tirets, mesure en rond ; dessous les deux valeurs.
/// `previous` : la valeur du même constat sur le vol comparé et son nom, en rond creux et sur une ligne de plus.
fn gauge_ui(ui: &mut Ui, th: &Theme, g: &Gauge, col: Color32, previous: Option<(f64, &str)>) {
    let Some(m) = g.marks() else { return };
    let before = previous.and_then(|(v, name)| {
        let pg = Gauge { value: v, ..g.clone() };
        pg.marks()
            .map(|pm| (pm.value, format!("{} : {}", name, pg.value_text())))
    });
    ui.vertical(|ui| {
        ui.set_width(GAUGE_W);
        ui.spacing_mut().item_spacing.y = 6.0;
        let (rect, _) = ui.allocate_exact_size(vec2(GAUGE_W, GAUGE_H), Sense::hover());
        let p = ui.painter();
        let x = |t: f64| rect.left() + rect.width() * t as f32;
        let track = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + 8.0),
            egui::pos2(rect.right(), rect.top() + 16.0),
        );
        p.rect_filled(track, 4.0, th.ui.border);
        let band = egui::Rect::from_min_max(
            egui::pos2(x(m.band_start), track.top()),
            egui::pos2(x(m.band_start + m.band_width), track.bottom()),
        );
        p.rect_filled(band, 0.0, with_alpha(col, 89));
        let tx = x(m.threshold);
        p.add(egui::Shape::dashed_line(
            &[egui::pos2(tx, rect.top() + 2.0), egui::pos2(tx, rect.top() + 22.0)],
            Stroke::new(1.5, th.ui.text),
            3.0,
            2.0,
        ));
        if let Some((t, _)) = &before {
            p.circle_stroke(egui::pos2(x(*t), rect.top() + 12.0), 5.0, Stroke::new(2.0, th.ui.muted));
        }
        p.circle_filled(egui::pos2(x(m.value), rect.top() + 12.0), 6.0, col);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                RichText::new(g.value_text())
                    .font(FontId::monospace(TINY))
                    .strong()
                    .color(col),
            );
            ui.label(RichText::new(g.value_label).size(TINY).color(th.ui.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(g.threshold_label).size(TINY).color(th.ui.muted));
                ui.label(
                    RichText::new(g.threshold_text())
                        .font(FontId::monospace(TINY))
                        .color(th.ui.text),
                );
            });
        });
        if let Some((_, text)) = before {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter()
                    .circle_stroke(r.center(), 4.0, Stroke::new(1.5, th.ui.muted));
                ui.label(
                    RichText::new(safe_text(&text))
                        .font(FontId::monospace(TINY))
                        .color(th.ui.muted),
                );
            });
        }
    });
}

/// Le badge du drone (prototype) : carré teinté de sa couleur et son icône, son nom et dessous sa taille.
fn drone_badge(ui: &mut Ui, th: &Theme, d: &DroneBadge) {
    let col = file_color(d.rank);
    egui::Frame::new()
        .fill(th.ui.base)
        .stroke(Stroke::new(1.0, th.ui.border))
        .corner_radius(12)
        .inner_margin(egui::Margin {
            left: 8,
            right: 14,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                ui.painter().rect_filled(r, 10.0, with_alpha(col, 31));
                egui::Image::new(icon("drone", 24.0).source(ui.ctx()))
                    .tint(col)
                    .paint_at(ui, egui::Rect::from_center_size(r.center(), vec2(24.0, 24.0)));
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new(safe_text(&d.name)).size(TITLE_SIZE).strong());
                    if !d.sub.is_empty() {
                        ui.label(RichText::new(safe_text(&d.sub)).size(SMALL).color(th.ui.muted));
                    }
                });
            });
        });
}

/// Une ligne de l'historique des choix (bandeau « Fragment déjà traité »).
fn history_ui(ui: &mut Ui, th: &Theme, h: &HistoryLine) {
    let col = if h.applied {
        th.accent("green")
    } else {
        th.accent("red")
    };
    egui::Frame::new()
        .fill(th.ui.base)
        .stroke(Stroke::new(1.0, th.ui.border))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.add(icon(if h.applied { "check" } else { "x" }, 14.0).tint(col));
                ui.label(RichText::new(&h.status).size(13.0).strong().color(col));
                ui.label(RichText::new(format!("· {}", safe_text(&h.title))).size(13.0));
            });
            ui.add(egui::Label::new(RichText::new(safe_text(&h.detail)).size(13.0).color(th.ui.muted)).wrap());
        });
}

/// « Plus signalé depuis FINAL.02 : Réponse lente, Terme D bruité » : les cartes du vol comparé qui ont disparu.
pub fn gone_text(c: &Comparison) -> String {
    format!("Plus signalé depuis {} : {}", c.name, c.gone.join(", "))
}

/// Ce qu'une carte sait du vol comparé et du fragment (pour les votes) ; vide pour les vérifications OK dépliées.
#[derive(Default)]
struct CardContext<'a> {
    cmp: Option<&'a Comparison>,
    fragment: Option<&'a str>,
}

/// Le rapport du vol comparé, **avec le profil du fragment analysé** (mêmes seuils, donc mêmes cartes) : sur le log
/// déjà ouvert s'il l'est, sinon en lisant son fichier. Rangé dans le cache du fragment analysé, sous une clé qui porte
/// le profil, le fragment comparé et ses bornes.
fn compare_report(
    li: &LogInput,
    opened: Option<&LogInput>,
    t: &CompareTarget,
    main: &Report,
) -> Result<Arc<Report>, String> {
    let f = &t.fragment;
    let key = format!("advisor-compare:{}:{}:{}-{}", main.profile.key, f.id, f.start, f.end);
    if let Some(r) = li.cache.get::<Report>(&key) {
        return Ok(r);
    }
    let run = |log: &otb_log::flightlog::FlightLog| -> Result<Report, String> {
        let range = otb_store::link::range_of(f, log.n())?;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            analyze(log, Some(range), Some(&main.profile))
        }))
        .unwrap_or_else(|_| Err("panique du calcul".into()))
    };
    let report = match opened {
        Some(o) => run(&o.log)?,
        None => {
            let logs = otb_log::flightlog::load(&t.path).map_err(|e| format!("log illisible ({e})"))?;
            let log = logs
                .iter()
                .find(|l| l.session as u64 == f.session)
                .ok_or_else(|| format!("session {} absente du fichier", f.session))?;
            run(log)?
        }
    };
    let r = Arc::new(report);
    li.cache.put(Famille::Assistant, key, r.clone());
    Ok(r)
}

/// Contour en pointillés (egui n'a pas de cadre pointillé).
fn dashed_rect(ui: &Ui, r: egui::Rect, stroke: Stroke) {
    let r = r.shrink(0.5);
    let pts = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    ui.painter().add(egui::Shape::dashed_line(&pts, stroke, 4.0, 3.0));
}

impl Tool for AdvisorTool {
    type Options = Options;
    type Output = AdvisorOutput;
    const TITLE: &'static str = "Assistant";

    fn read_library(&mut self, lib: &Library, state: &AppState) {
        let analysed = self
            .files
            .indices(state)
            .first()
            .and_then(|&i| state.logs.get(i))
            .and_then(|l| l.fragment.clone());
        let pick = self
            .pick
            .as_ref()
            .filter(|(_, for_fragment)| Some(for_fragment) == analysed.as_ref())
            .map(|(p, _)| p.as_str());
        self.compare = analysed.as_deref().and_then(|a| advice::compare_target(lib, a, pick));
        self.view = analysed.as_deref().map(|a| LibView::read(lib, a)).unwrap_or_default();
        self.analysed = analysed;
    }

    fn take_library_edits(&mut self) -> Vec<LibraryEdit> {
        std::mem::take(&mut self.edits)
    }

    fn options(&self) -> Options {
        Options {
            profile: self.profile,
            compare: self.compare.clone(),
        }
    }

    /// Le fragment analysé, puis le vol comparé s'il est déjà ouvert (sinon le fil de calcul lit son fichier).
    fn selected(&self, state: &AppState) -> Vec<usize> {
        let mut sel: Vec<usize> = self.files.indices(state).into_iter().take(1).collect();
        if let Some(c) = &self.compare
            && let Some(i) = state
                .logs
                .iter()
                .position(|l| l.fragment.as_deref() == Some(c.fragment.id.as_str()))
            && !sel.contains(&i)
        {
            sel.push(i);
        }
        sel
    }

    fn compute(o: Options, logs: Vec<LogInput>) -> Result<AdvisorOutput, String> {
        let li = logs
            .first()
            .ok_or("Activez au moins un fragment dans la barre du haut.")?;
        // la clé contient le profil ET la période : un rapport calculé avant un changement ne peut pas servir
        let key = format!("advisor:{}:{:?}", o.profile.unwrap_or("auto"), li.period);
        let report = match li.cache.get::<Report>(&key) {
            Some(r) => r,
            None => {
                let sel = li.log.slice_time(li.period.0, li.period.1);
                let prof = match o.profile {
                    Some(k) => Some(profile(k).ok_or_else(|| format!("Profil inconnu : {k}"))?),
                    None => None,
                };
                let r = Arc::new(analyze(&li.log, Some(sel), prof)?);
                li.cache.put(Famille::Assistant, key, r.clone());
                r
            }
        };
        let compare = o.compare.map(|t| {
            let opened = logs
                .get(1)
                .filter(|l| l.fragment.as_deref() == Some(t.fragment.id.as_str()));
            let other = compare_report(li, opened, &t, &report);
            let cmp = other.map(|r| advice::compare_cards(&t.name, &report, &r));
            (t, cmp)
        });
        Ok(AdvisorOutput {
            label: li.label.clone(),
            color: li.color,
            firmware: li.log.settings.firmware.clone(),
            report,
            fragment: li.fragment.clone(),
            compare,
        })
    }

    fn controls(&mut self, ui: &mut Ui, _state: &AppState, theme: &Theme) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label("Taille du drone :");
            egui::ComboBox::from_id_salt("assistant-profil")
                .selected_text(safe_text(Self::profile_label(self.profile)))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.profile, None, Self::profile_label(None));
                    for p in PROFILES.iter() {
                        ui.selectable_value(&mut self.profile, Some(p.key), safe_text(p.label));
                    }
                })
                .response
                .accessible_name("Taille du drone");
            ui.label(RichText::new(HELP).color(theme.ui.muted));
        });
    }

    fn render(&mut self, ui: &mut Ui, state: &mut AppState, out: &AdvisorOutput, theme: &Theme) {
        // la couleur du vol comparé, s'il est ouvert dans la liste d'analyse
        let cmp_color = out.compare.as_ref().and_then(|(t, _)| {
            state
                .logs
                .iter()
                .find(|l| l.fragment.as_deref() == Some(t.fragment.id.as_str()))
                .map(|l| l.color)
        });
        egui::Panel::right("assistant-courbes")
            .default_size(PLOTS_WIDTH)
            .size_range(260.0..=640.0)
            .frame(
                egui::Frame::new()
                    .fill(theme.ui.base)
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ui, |ui| {
                // ⚠️ panneau redimensionnable : contenu dans un ScrollArea qui remplit, sinon il garde sa première taille (L-073)
                egui::ScrollArea::vertical()
                    .id_salt("assistant-courbes-defilement")
                    .auto_shrink([false, false])
                    .show(ui, |ui| Self::plots_ui(ui, theme, out));
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme.ui.window).inner_margin(PAGE_PAD))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("assistant-rapport")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.report_ui(ui, theme, out, cmp_color));
            });
    }
}
