//! Le tableau de bord : une carte par drone connu de la bibliothèque, et les fragments de celui qu'on choisit.
//!
//! C'est l'écran qui rend visible `otb-store` (lot 28) : les drones et les fragments y sont ceux que la
//! bibliothèque a retenus, pas seulement ceux qui sont ouverts en ce moment. Un drone est reconnu par le **nom de
//! machine** de l'en-tête des logs (`Craft name`) tant que la récupération par USB n'apporte pas l'identifiant de
//! la carte (lot 32).
//!
//! ⚠️ **Le dessin suit le prototype complet de l'application** (canevas du 2026-11-16, la maquette qui fait foi,
//! D-060) : les mesures de ce fichier — tailles de texte, coins, marges, hauteur de la barre d'avancement — en
//! sont relevées, et les couleurs sont celles de `theme.rs`, qui s'y trouvaient déjà (le bleu `highlight`, les
//! accents vert et orange, le gris `muted`). La maquette n'existe qu'en **clair** : le thème sombre garde ses
//! formes et prend les couleurs sombres du thème (arbitrage de Cédric, 2026-11-19).

use egui::{Align, Layout, RichText, Ui, Vec2};
use otb_advisor::drone_card::DroneCard;
use otb_store::Library;
use std::path::PathBuf;

use crate::icons::icon;
use crate::state::AppState;
use crate::theme::{Theme, file_color, plot, with_alpha};

/// Ce que l'utilisateur demande depuis le tableau de bord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DashAction {
    /// Ouvrir le Guide sur ce drone (nom de la fiche) ; `None` : le Guide, sans fiche choisie.
    Guide(Option<String>),
    /// Ouvrir la boîte « Ouvrir des logs… ».
    OpenDialog,
    /// Rond « + » d'un fragment : l'ajouter à la liste d'analyse (en n'ouvrant que sa session) ou l'en retirer.
    Toggle { fragment: String, path: PathBuf },
    /// Crayon d'un fragment : l'écran d'extraction sur ce fragment (son log est ouvert d'abord s'il le faut).
    Edit { fragment: String, path: PathBuf },
}

// ---------------------------------------------------------------- mesures relevées dans la maquette

const PAGE_PAD: i8 = 24;
const PAGE_GAP: f32 = 18.0;
const TITLE_SIZE: f32 = 24.0;
const CARD_MIN: f32 = 280.0;
const CARD_GAP: f32 = 16.0;
const CARD_RADIUS: u8 = 12;
const CARD_PAD: i8 = 18;
const CARD_GAP_IN: f32 = 14.0;
const BADGE: f32 = 44.0;
const BADGE_RADIUS: u8 = 10;
const NAME_SIZE: f32 = 16.0;
const SMALL: f32 = 12.5;
const TINY: f32 = 11.5;
const COUNT_SIZE: f32 = 20.0;
const BAR_H: f32 = 6.0;
/// Opacité de la teinte d'un badge : `couleur + '1f'` dans la maquette.
const TINT: u8 = 0x1f;
const PANEL_W: f32 = 400.0;
const DOT: f32 = 28.0;
/// Fond d'un fragment en analyse : sa couleur à 8 % (maquette : `couleur + '14'`).
const ADDED_TINT: u8 = 0x14;
/// Indice du jaune dans la palette des fichiers : la coche y est sombre.
const YELLOW: usize = 2;

/// Date `AAAA-MM-JJ` d'un instant donné en secondes depuis 1970 (UTC).
///
/// La conversion « jours → date civile » est celle de `rust/otb-gui/src/export.rs` (nom des captures) : une seule
/// implémentation dans le programme, et elle est déjà éprouvée par ses tests.
pub fn date_iso(epoch_secs: i64) -> String {
    let (y, m, d) = crate::export::date_from_secs(epoch_secs);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Date de dernière modification d'un fichier, en `AAAA-MM-JJ`, ou `""` si le système ne la donne pas.
///
/// C'est la date du **fichier**, pas celle du vol : l'en-tête des logs porte bien un « Log start datetime », mais
/// il vaut `0000-01-01` dès que la carte de vol n'a ni horloge ni GPS à l'armement — le cas de la plupart des
/// logs du corpus.
pub fn file_date(path: &std::path::Path) -> String {
    let Ok(meta) = std::fs::metadata(path) else {
        return String::new();
    };
    let Ok(modified) = meta.modified() else {
        return String::new();
    };
    match modified.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => date_iso(d.as_secs() as i64),
        Err(_) => String::new(),
    }
}

/// L'avancement d'un drone dans le parcours du Guide : « Étape N / 4 · titre », et la part faite (0 à 1).
///
/// ⚠️ Ne compte que les cases **cochées à la main** (arbitrage de Cédric, 2026-11-19) : les cases que le Guide
/// vérifie tout seul demanderaient une analyse par log, trop chère pour une carte. Le chiffre du Guide peut donc
/// être plus élevé que celui-ci.
fn stage(card: &DroneCard) -> (String, f32) {
    let phases = otb_advisor::tuning_path::phases(card);
    let faites: Vec<(usize, usize)> = phases
        .iter()
        .map(|ph| {
            (
                ph.steps.iter().filter(|s| crate::guide::is_done(card, &s.key)).count(),
                ph.steps.len(),
            )
        })
        .collect();
    let total: usize = faites.iter().map(|(_, t)| t).sum();
    let fait: usize = faites.iter().map(|(n, _)| n).sum();
    let courante = faites.iter().position(|(n, t)| n < t).unwrap_or(0);
    let titre = phases.get(courante).map_or("", |p| p.title);
    (
        format!("Étape {} / {} · {titre}", courante + 1, phases.len()),
        if total == 0 { 0.0 } else { fait as f32 / total as f32 },
    )
}

/// L'état d'un drone, tel que la pastille de sa carte l'annonce.
fn status(part: f32, fragments: usize) -> (&'static str, &'static str) {
    if fragments == 0 {
        ("À préparer", "muted")
    } else if part >= 1.0 {
        ("Vérifié", "green")
    } else {
        ("En cours", "orange")
    }
}

/// L'écran du tableau de bord : le drone choisi, dont les fragments s'affichent à droite.
#[derive(Debug, Default)]
pub struct DashboardPage {
    /// Identifiant du drone choisi (`None` : aucun panneau ouvert).
    pub selected: Option<String>,
}

impl DashboardPage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        cards: &[DroneCard],
        state: &AppState,
        th: &Theme,
    ) -> Vec<DashAction> {
        let mut actions = Vec::new();
        let ouvert = self
            .selected
            .clone()
            .filter(|id| lib.drone(id).is_some() || lib.logs().iter().any(|l| &l.drone_id == id));
        if let Some(id) = ouvert {
            egui::Panel::right("tableau-drone")
                .default_size(PANEL_W)
                .size_range(320.0..=560.0)
                .frame(egui::Frame::new().fill(th.ui.base).stroke(egui::Stroke::NONE))
                .show(ui, |ui| self.panel(ui, lib, state, th, &id, &mut actions));
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(th.ui.window).inner_margin(PAGE_PAD))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = PAGE_GAP;
                self.header(ui, th, &mut actions);
                if lib.drones().is_empty() {
                    ui.label(
                        "Aucun drone connu pour l'instant. Ouvrez un log : son nom de machine devient un drone, \
                         et la bibliothèque le retient d'un lancement à l'autre.",
                    );
                    return;
                }
                egui::ScrollArea::vertical().show(ui, |ui| self.grid(ui, lib, cards, th, &mut actions));
            });
        actions
    }

    /// Le titre de l'écran et les deux boutons de la maquette.
    fn header(&mut self, ui: &mut Ui, th: &Theme, actions: &mut Vec<DashAction>) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Tableau de bord").size(TITLE_SIZE).color(th.ui.text));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // bouton plein : le geste principal de l'écran
                let on_accent = crate::theme::plot::ON_ACCENT;
                let neuf = egui::Button::image_and_text(
                    icon("plus", 15.0).tint(on_accent),
                    RichText::new("Nouveau drone").color(on_accent),
                )
                .fill(th.ui.highlight)
                .corner_radius(6);
                if ui.add(neuf).clicked() {
                    actions.push(DashAction::Guide(None));
                }
                let ouvrir = egui::Button::image_and_text(
                    icon("folder-open", 15.0).tint(th.ui.text),
                    RichText::new("Ouvrir des logs…").color(th.ui.text),
                )
                .fill(th.ui.base)
                .stroke(egui::Stroke::new(1.0, th.ui.mid))
                .corner_radius(6);
                if ui.add(ouvrir).clicked() {
                    actions.push(DashAction::OpenDialog);
                }
            });
        });
    }

    /// La grille des cartes : autant de colonnes que la largeur en permet, 280 points au moins par carte.
    fn grid(&mut self, ui: &mut Ui, lib: &Library, cards: &[DroneCard], th: &Theme, actions: &mut Vec<DashAction>) {
        let large = ui.available_width();
        let colonnes = (((large + CARD_GAP) / (CARD_MIN + CARD_GAP)).floor() as usize).max(1);
        let largeur = ((large - CARD_GAP * (colonnes as f32 - 1.0)) / colonnes as f32).floor();
        let drones: Vec<String> = lib.drones().iter().map(|d| d.id.clone()).collect();
        ui.spacing_mut().item_spacing = Vec2::splat(CARD_GAP);
        for (ligne, ids) in drones.chunks(colonnes).enumerate() {
            ui.horizontal_top(|ui| {
                for (colonne, id) in ids.iter().enumerate() {
                    self.card(ui, lib, cards, th, id, ligne * colonnes + colonne, largeur, actions);
                }
            });
        }
    }

    /// Une carte de drone, au dessin de la maquette.
    #[allow(clippy::too_many_arguments)]
    fn card(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        cards: &[DroneCard],
        th: &Theme,
        id: &str,
        rang: usize,
        largeur: f32,
        actions: &mut Vec<DashAction>,
    ) {
        let drone = lib.drone(id);
        let nom = drone.map_or(id, |d| d.name.as_str()).to_string();
        let fiche = cards.iter().find(|c| c.name == nom);
        let couleur = file_color(rang);
        let choisi = self.selected.as_deref() == Some(id);
        let logs: Vec<&otb_store::model::LogEntry> = lib.logs().iter().filter(|l| l.drone_id == id).collect();
        let fragments: usize = logs
            .iter()
            .map(|l| lib.fragments().iter().filter(|f| f.log_id == l.id).count())
            .sum();
        let dernier = logs.iter().map(|l| l.added.as_str()).max().unwrap_or("—").to_string();
        let (etape, part) = fiche.map_or_else(|| ("Pas de fiche drone".to_string(), 0.0), stage);
        let (etat, accent) = status(part, fragments);
        let sous_titre = fiche.map_or_else(
            || drone.map(|d| d.firmware.clone()).unwrap_or_default(),
            |c| {
                if c.pid_khz > 0.0 {
                    format!("{} · {} kHz", c.size, c.pid_khz)
                } else {
                    c.size.clone()
                }
            },
        );
        // l'en-tête d'un log donne « Betaflight 2026.6.0-rc3 (f625a9996) STM32F405 » : la carte n'en montre que
        // le firmware et sa version, comme la maquette
        let firmware = fiche.map_or_else(
            || {
                drone
                    .map(|d| d.firmware.split_whitespace().take(2).collect::<Vec<_>>().join(" "))
                    .unwrap_or_default()
            },
            |c| format!("Betaflight {}", c.bf),
        );

        let mut cadre = egui::Frame::new()
            .fill(th.ui.base)
            .corner_radius(CARD_RADIUS)
            .inner_margin(CARD_PAD)
            .stroke(if choisi {
                egui::Stroke::new(2.0, th.ui.highlight)
            } else {
                egui::Stroke::new(1.0, th.ui.border)
            });
        if choisi {
            cadre = cadre.shadow(egui::epaint::Shadow {
                offset: [0, 4],
                blur: 16,
                spread: 0,
                color: with_alpha(th.ui.highlight, 38),
            });
        }
        let reponse = cadre
            .show(ui, |ui| {
                let interieur = largeur - f32::from(CARD_PAD) * 2.0 - 2.0;
                // ⚠️ La grille pose les cartes **côte à côte** : sans ce `vertical`, la carte hérite de son
                // layout horizontal et ses trois rangées se mettent sur une seule ligne.
                ui.vertical(|ui| {
                    ui.set_width(interieur);
                    ui.set_max_width(interieur);
                    ui.spacing_mut().item_spacing.y = CARD_GAP_IN;
                    // 1. badge teinté, nom, sous-titre, pastille d'état
                    ui.horizontal_top(|ui| {
                        let (badge, _) = ui.allocate_exact_size(Vec2::splat(BADGE), egui::Sense::hover());
                        ui.painter().rect_filled(badge, BADGE_RADIUS, with_alpha(couleur, TINT));
                        icon("drone", 24.0)
                            .tint(couleur)
                            .paint_at(ui, egui::Rect::from_center_size(badge.center(), Vec2::splat(24.0)));
                        ui.add_space(12.0);
                        // la pastille se pose à droite d'abord ; le nom prend ce qui reste, sans déborder
                        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                            ui.add_space(26.0); // la place du crayon, en haut à droite
                            chip(ui, th, etat, accent);
                            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.add(
                                    egui::Label::new(RichText::new(&nom).size(NAME_SIZE).strong().color(th.ui.text))
                                        .truncate(),
                                );
                                ui.add(
                                    egui::Label::new(RichText::new(&sous_titre).size(SMALL).color(th.ui.muted))
                                        .truncate(),
                                );
                            });
                        });
                    });
                    // 2. les deux nombres, côte à côte
                    ui.columns(2, |col| {
                        col[0].spacing_mut().item_spacing.y = 2.0;
                        col[0].label(RichText::new("Fragments").size(TINY).color(th.ui.muted));
                        col[0].label(
                            RichText::new(fragments.to_string())
                                .size(COUNT_SIZE)
                                .strong()
                                .monospace()
                                .color(th.ui.text),
                        );
                        col[1].spacing_mut().item_spacing.y = 2.0;
                        col[1].label(RichText::new("Dernier vol").size(TINY).color(th.ui.muted));
                        col[1].add_space(4.0);
                        col[1].label(RichText::new(&dernier).size(14.0).strong().color(th.ui.text));
                    });
                    // 3. l'étape, le firmware, et la barre d'avancement
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        ui.horizontal(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(&firmware).size(12.0).color(th.ui.muted)).truncate(),
                                );
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    ui.add(
                                        egui::Label::new(RichText::new(&etape).size(12.0).color(th.ui.muted))
                                            .truncate(),
                                    );
                                });
                            });
                        });
                        let (piste, _) =
                            ui.allocate_exact_size(Vec2::new(ui.available_width(), BAR_H), egui::Sense::hover());
                        ui.painter().rect_filled(piste, BAR_H / 2.0, th.ui.hover);
                        if part > 0.0 {
                            let plein = egui::Rect::from_min_size(
                                piste.min,
                                Vec2::new(piste.width() * part.clamp(0.0, 1.0), BAR_H),
                            );
                            ui.painter().rect_filled(plein, BAR_H / 2.0, th.ui.highlight);
                        }
                    });
                });
            })
            .response;

        // toute la carte est cliquable ; le crayon se pose par-dessus, en haut à droite
        if ui
            .interact(reponse.rect, ui.scope_id().with(("carte", id)), egui::Sense::click())
            .clicked()
        {
            self.selected = if choisi { None } else { Some(id.to_string()) };
        }
        let crayon = egui::Rect::from_min_size(reponse.rect.right_top() + Vec2::new(-42.0, 12.0), Vec2::splat(30.0));
        let bouton = egui::Button::image(icon("pencil", 15.0).tint(th.ui.muted).alt_text("Modifier le drone"))
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE)
            .corner_radius(7);
        if ui.put(crayon, bouton).on_hover_text("Modifier le drone").clicked() {
            actions.push(DashAction::Guide(Some(nom)));
        }
    }

    /// Le panneau de droite : les fragments du drone choisi, du plus récent en haut.
    fn panel(
        &mut self,
        ui: &mut Ui,
        lib: &Library,
        state: &AppState,
        th: &Theme,
        id: &str,
        actions: &mut Vec<DashAction>,
    ) {
        let nom = lib.drone(id).map_or(id, |d| d.name.as_str()).to_string();
        let rang = lib.drones().iter().position(|d| d.id == id).unwrap_or(0);
        let couleur = file_color(rang);
        let logs: Vec<&otb_store::model::LogEntry> = lib.logs().iter().filter(|l| l.drone_id == id).collect();
        let mut frags: Vec<(&otb_store::model::Fragment, &otb_store::model::LogEntry)> = logs
            .iter()
            .flat_map(|l| {
                lib.fragments()
                    .iter()
                    .filter(move |f| f.log_id == l.id)
                    .map(move |f| (f, *l))
            })
            .collect();
        frags.sort_by(|a, b| b.0.date.cmp(&a.0.date).then(b.0.name.cmp(&a.0.name)));

        // en-tête : badge, nom, nombre de fragments, croix
        egui::Frame::new()
            .fill(th.ui.base)
            .inner_margin(egui::Margin {
                left: 20,
                right: 20,
                top: 18,
                bottom: 14,
            })
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let (badge, _) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
                    ui.painter().rect_filled(badge, 9, with_alpha(couleur, TINT));
                    icon("drone", 20.0)
                        .tint(couleur)
                        .paint_at(ui, egui::Rect::from_center_size(badge.center(), Vec2::splat(20.0)));
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new(&nom).size(17.0).strong().color(th.ui.text));
                        ui.label(
                            RichText::new(format!(
                                "{} fragment{} · plus récent en haut",
                                frags.len(),
                                if frags.len() > 1 { "s" } else { "" }
                            ))
                            .size(SMALL)
                            .color(th.ui.muted),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        let fermer =
                            egui::Button::image(icon("x", 18.0).tint(th.ui.muted).alt_text("Fermer le panneau"))
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE);
                        if ui.add(fermer).on_hover_text("Fermer le panneau").clicked() {
                            self.selected = None;
                        }
                    });
                });
            });
        if frags.is_empty() {
            ui.label("Aucun fragment pour ce drone.");
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 20,
                    right: 20,
                    top: 20,
                    bottom: 8,
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 18.0;
                    for (f, l) in frags {
                        let chemin = PathBuf::from(&l.path);
                        // en analyse : la couleur de sa pastille (D-066)
                        let couleur = state
                            .logs
                            .iter()
                            .find(|o| o.fragment.as_deref() == Some(f.id.as_str()))
                            .map(|o| o.color);
                        self.timeline_item(ui, th, f, &chemin, couleur, actions);
                    }
                });
        });
    }

    /// Un fragment dans la timeline : le rond « + » (ou la coche, à la couleur de sa pastille, s'il est dans la
    /// liste d'analyse), son nom et sa date.
    fn timeline_item(
        &mut self,
        ui: &mut Ui,
        th: &Theme,
        f: &otb_store::model::Fragment,
        chemin: &std::path::Path,
        couleur: Option<egui::Color32>,
        actions: &mut Vec<DashAction>,
    ) {
        ui.horizontal_top(|ui| {
            let (rond, _) = ui.allocate_exact_size(Vec2::splat(DOT), egui::Sense::hover());
            // maquette : anneau et « + » bleus sur fond blanc ; ajouté, rond plein à sa couleur, coche blanche
            // (sombre sur le jaune, trop clair pour du blanc)
            let (anneau, fond, trait_) = match couleur {
                Some(c) => (
                    c,
                    c,
                    if c == file_color(YELLOW) {
                        th.ui.text
                    } else {
                        plot::ON_ACCENT
                    },
                ),
                None => (th.ui.highlight, th.ui.base, th.ui.highlight),
            };
            ui.painter()
                .circle(rond.center(), DOT / 2.0 - 1.0, fond, egui::Stroke::new(2.0, anneau));
            // ⚠️ « ✓ » et « + » n'existent dans aucune police embarquée : ce sont des icônes (L-068)
            let signe = if couleur.is_some() { "check" } else { "plus" };
            icon(signe, 15.0)
                .tint(trait_)
                .paint_at(ui, egui::Rect::from_center_size(rond.center(), Vec2::splat(15.0)));
            let label = if couleur.is_some() {
                format!("Retirer {} de l'analyse", f.name)
            } else {
                format!("Ajouter {} à l'analyse", f.name)
            };
            let r = ui
                .interact(rond, ui.scope_id().with(("frag", &f.id)), egui::Sense::click())
                .on_hover_text(&label);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
            if r.clicked() {
                actions.push(DashAction::Toggle {
                    fragment: f.id.clone(),
                    path: chemin.to_path_buf(),
                });
            }
            ui.add_space(14.0);
            egui::Frame::new()
                .fill(couleur.map_or(th.ui.base, |c| with_alpha(c, ADDED_TINT)))
                .stroke(egui::Stroke::new(1.0, th.ui.border))
                .corner_radius(8)
                .inner_margin(egui::Margin {
                    left: 12,
                    right: 12,
                    top: 10,
                    bottom: 10,
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&f.name).size(14.0).strong().color(th.ui.text));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            // le crayon de la maquette : régler les limites du fragment dans l'écran d'extraction
                            let label = format!("Modifier les limites de {}", f.name);
                            let crayon = egui::Button::image(icon("pencil", 14.0).tint(th.ui.muted).alt_text(label))
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE);
                            if ui.add(crayon).on_hover_text("Modifier les limites").clicked() {
                                actions.push(DashAction::Edit {
                                    fragment: f.id.clone(),
                                    path: chemin.to_path_buf(),
                                });
                            }
                            ui.label(RichText::new(&f.date).size(12.0).color(th.ui.muted));
                        });
                    });
                });
        });
    }
}

/// La pastille d'état d'une carte : texte court, bordure arrondie, couleur d'accent.
fn chip(ui: &mut Ui, th: &Theme, texte: &str, accent: &str) {
    let couleur = if accent == "muted" {
        th.ui.muted
    } else {
        th.accent(accent)
    };
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0, th.ui.border))
        .corner_radius(10)
        .inner_margin(egui::Margin {
            left: 8,
            right: 8,
            top: 3,
            bottom: 3,
        })
        .show(ui, |ui| {
            ui.label(RichText::new(texte).size(TINY).strong().color(couleur));
        });
}

#[cfg(test)]
mod tests {
    use super::{date_iso, status};

    /// La date d'un enregistrement de la bibliothèque, au format qui sert aussi à la ranger (l'ordre
    /// alphabétique de `AAAA-MM-JJ` est l'ordre chronologique).
    #[test]
    fn la_date_suit_le_calendrier() {
        assert_eq!(date_iso(0), "1970-01-01");
        assert_eq!(date_iso(86_399), "1970-01-01", "la veille de minuit");
        assert_eq!(date_iso(86_400), "1970-01-02");
        // une année bissextile et son 29 février
        assert_eq!(date_iso(1_709_164_800), "2024-02-29");
        assert_eq!(date_iso(1_763_510_400), "2025-11-19");
        // 2000 est bissextile, 1900 ne l'était pas : la règle des siècles
        assert_eq!(date_iso(951_782_400), "2000-02-29");
        assert_eq!(date_iso(-2_208_988_800), "1900-01-01");
        // 1900 n'est pas bissextile : au 28 février succède le 1er mars
        assert_eq!(date_iso(-2_203_977_600), "1900-02-28");
        assert_eq!(
            date_iso(-2_203_977_600 + 86_400),
            "1900-03-01",
            "pas de 29 février en 1900"
        );
    }

    #[test]
    fn la_pastille_dit_ou_en_est_le_drone() {
        assert_eq!(status(0.0, 0).0, "À préparer", "aucun fragment : rien n'a commencé");
        assert_eq!(status(0.4, 3).0, "En cours");
        assert_eq!(status(1.0, 3).0, "Vérifié");
        assert_eq!(
            status(1.0, 0).0,
            "À préparer",
            "un parcours coché sans vol ne vérifie rien"
        );
    }
}
