//! Outil « Assistant », en cartes comme le prototype (D-061 ; lot 30f, D-070) : l'en-tête du fragment analysé, une pastille par
//! gravité, une carte par constat triée par gravité (la mesure contre son seuil en jauge, les « info » repliées), les vérifications
//! OK regroupées sur une ligne qu'on déplie, et à droite la réponse indicielle de chaque axe en petit.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{Color32, FontId, RichText, Sense, Stroke, Ui, vec2};
use egui_plot::{FilledArea, HLine, Line, LineStyle, Plot, PlotPoints};
use otb_advisor::profiles::{PROFILES, profile};
use otb_advisor::rules::{self, Finding, Gauge, Report, analyze};

use crate::icons::icon;
use crate::plots::nice_axes;
use crate::state::{AppState, Famille};
use crate::theme::{Theme, with_alpha};
use crate::tool::{LogInput, Tool};
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

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// Clé du profil choisi (`None` : détection automatique).
    pub profile: Option<&'static str>,
}

pub struct AdvisorOutput {
    pub label: String,
    pub color: egui::Color32,
    pub firmware: String,
    pub report: Arc<Report>,
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
        }
    }

    fn profile_label(key: Option<&'static str>) -> &'static str {
        key.and_then(profile).map_or("Détection automatique", |p| p.label)
    }

    /// En-tête : le fragment analysé, à sa couleur, et sous son nom le drone, la période et le firmware.
    fn header(ui: &mut Ui, th: &Theme, out: &AdvisorOutput) {
        let rep = &out.report;
        let meta = format!(
            "{} ({}) · période {} – {} s · {}",
            rep.profile.label,
            rep.profile_reason,
            comma(otb_format::fixed(rep.period.0, 1, false)),
            comma(otb_format::fixed(rep.period.1, 1, false)),
            out.firmware
        );
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

    /// Une carte : gravité, titre et bouton qui la replie ; dépliée, le pourquoi, la jauge et les gestes conseillés.
    fn card(&mut self, ui: &mut Ui, th: &Theme, f: &Finding) {
        let id = card_id(f);
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
                    });
                });
                if open {
                    Self::card_body(ui, th, f, col);
                }
            });
        if toggle {
            self.open.insert(id, !open);
        }
    }

    fn card_body(ui: &mut Ui, th: &Theme, f: &Finding, col: Color32) {
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
                    gauge_ui(ui, th, g, col);
                });
            }
            Some(g) => {
                why(ui);
                gauge_ui(ui, th, g, col);
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
                self.card(ui, th, f);
            }
        }
    }

    fn report_ui(&mut self, ui: &mut Ui, th: &Theme, out: &AdvisorOutput) {
        ui.spacing_mut().item_spacing.y = PAGE_GAP;
        Self::header(ui, th, out);
        Self::pills_ui(ui, th, &out.report.findings);
        let (cards, oks) = split_cards(&out.report);
        for f in &cards {
            self.card(ui, th, f);
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
fn gauge_ui(ui: &mut Ui, th: &Theme, g: &Gauge, col: Color32) {
    let Some(m) = g.marks() else { return };
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
    });
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

    fn options(&self) -> Options {
        Options { profile: self.profile }
    }

    fn selected(&self, state: &AppState) -> Vec<usize> {
        self.files.indices(state)
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
        Ok(AdvisorOutput {
            label: li.label.clone(),
            color: li.color,
            firmware: li.log.settings.firmware.clone(),
            report,
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

    fn render(&mut self, ui: &mut Ui, _state: &mut AppState, out: &AdvisorOutput, theme: &Theme) {
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
                    .show(ui, |ui| self.report_ui(ui, theme, out));
            });
    }
}
