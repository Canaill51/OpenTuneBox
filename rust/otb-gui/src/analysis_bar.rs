//! La **barre de fragments** du haut de la fenêtre : la liste d'analyse à l'écran (lot 29c, D-066).
//!
//! Une pastille par log ouvert, dans l'ordre de la liste : rond de couleur, nom du fragment, nom du drone, et un
//! « × » pour le retirer. C'est elle, et non plus chaque page, qui décide de ce que montrent les outils :
//!
//! - **clic sur la pastille** : la désactiver (barrée, en pointillés) ou la réactiver, sans la fermer ;
//! - **clic sur le rond** : en faire le **fragment maître** (rond plus gros, coché), celui des pages à un seul vol ;
//! - **glisser** : changer l'ordre des courbes ;
//! - **survol** : la fiche du fragment (rôle, date, début, durée, PID roll, firmware).
//!
//! Mesures relevées dans le prototype qui fait foi (D-061), en constantes nommées ci-dessous.

use egui::{Color32, CursorIcon, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

use crate::icons::icon;
use crate::state::AppState;
use crate::theme::{Theme, plot};

/// Hauteur d'une pastille (maquette : 28 px de contenu dans un trait d'un point).
pub const CHIP_H: f32 = 30.0;
/// Marge gauche, jusqu'au rond.
const PAD_LEFT: f32 = 10.0;
/// Marge droite, après le « × ».
const PAD_RIGHT: f32 = 4.0;
/// Espace entre deux éléments d'une pastille.
const GAP: f32 = 7.0;
/// Diamètre du rond : 15 points, 20 pour le fragment maître.
const DOT: f32 = 15.0;
const DOT_MASTER: f32 = 20.0;
/// Bouton « × » : un rond de 22 points, croix de 13.
const CLOSE: f32 = 22.0;
const CLOSE_ICON: f32 = 13.0;
/// Coche blanche du fragment maître.
const CHECK: f32 = 10.0;
/// Texte : nom en 13 (gras), drone en 12.
const NAME_SIZE: f32 = 13.0;
const DRONE_SIZE: f32 = 12.0;
/// Espace entre deux pastilles (maquette : `gap: 8px`).
pub const SPACING: f32 = 8.0;
/// Pointillés d'une pastille désactivée.
const DASH: f32 = 4.0;
const DASH_GAP: f32 = 3.0;

/// Ce qu'une pastille montre d'un log : calculé par l'application, qui seule connaît la bibliothèque.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChipInfo {
    pub name: String,
    pub drone: String,
    pub role: String,
    pub date: String,
    pub start: String,
    pub duration: String,
    pub pid: String,
    pub firmware: String,
}

/// Un geste sur la barre ; l'application l'applique à l'état et à la bibliothèque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarAction {
    /// Désactiver ou réactiver le log de cet indice.
    Toggle(usize),
    /// En faire le fragment maître.
    Master(usize),
    /// Le retirer de l'analyse (le fermer).
    Remove(usize),
    /// Le déplacer à la place `to`.
    Move { from: usize, to: usize },
}

/// Libellés des gestes, pour les lecteurs d'écran et les tests.
pub fn remove_label(name: &str) -> String {
    format!("Retirer {name} de l'analyse")
}

pub fn master_label(name: &str, master: bool) -> String {
    if master {
        format!("{name} : fragment maître")
    } else {
        format!("Faire de {name} le fragment maître")
    }
}

/// La place où tombe une pastille lâchée : le nombre d'**autres** pastilles qui la précèdent dans l'ordre de
/// lecture (ligne par ligne, puis de gauche à droite), la pastille tirée étant retirée de la liste.
/// Fonction pure : un glisser se teste mal à la souris simulée (L-071).
pub fn drop_index(rects: &[Rect], dragged: usize, pointer: Pos2) -> usize {
    rects
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != dragged)
        .filter(|(_, r)| {
            // une pastille d'une ligne au-dessus précède toujours ; sur la même ligne, selon son milieu
            r.max.y <= pointer.y || (r.min.y <= pointer.y && r.center().x < pointer.x)
        })
        .count()
}

#[derive(Default)]
pub struct AnalysisBar {
    /// Pastille en cours de glisser.
    drag: Option<usize>,
}

impl AnalysisBar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dessine la barre dans `ui` (disposée en lignes qui passent à la ligne) ; rend les gestes du cadre.
    pub fn ui(&mut self, ui: &mut Ui, state: &AppState, infos: &[ChipInfo], th: &Theme) -> Vec<BarAction> {
        let mut actions = Vec::new();
        if state.logs.is_empty() {
            self.drag = None;
            ui.label(
                egui::RichText::new("Aucun fragment — ajoutez-en avec le rond + d'un drone")
                    .size(NAME_SIZE)
                    .color(th.ui.muted),
            );
            return actions;
        }
        let master = state.master();
        let mut rects = Vec::with_capacity(state.logs.len());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(SPACING);
            for (i, ll) in state.logs.iter().enumerate() {
                let empty = ChipInfo::default();
                let info = infos.get(i).unwrap_or(&empty);
                let name = if info.name.is_empty() {
                    ll.label.as_str()
                } else {
                    info.name.as_str()
                };
                let r = self.chip(
                    ui,
                    th,
                    i,
                    ll.uid,
                    name,
                    info,
                    ll.color,
                    ll.active,
                    master == Some(i),
                    &mut actions,
                );
                rects.push(r);
            }
        });
        // glisser : une barre d'insertion où la pastille tomberait, puis le déplacement au lâcher
        if let Some(from) = self.drag {
            let pointer = ui.ctx().pointer_latest_pos();
            if let Some(p) = pointer {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                let to = drop_index(&rects, from, p);
                let others: Vec<&Rect> = rects
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != from)
                    .map(|(_, r)| r)
                    .collect();
                let x_y = match (others.get(to), to.checked_sub(1).and_then(|k| others.get(k))) {
                    (Some(r), _) => Some((r.min.x - SPACING / 2.0, r.y_range())),
                    (None, Some(r)) => Some((r.max.x + SPACING / 2.0, r.y_range())),
                    _ => None,
                };
                if let Some((x, ys)) = x_y {
                    ui.painter().vline(x, ys, Stroke::new(2.0, th.ui.highlight));
                }
            }
            if !ui.ctx().input(|i| i.pointer.any_down()) {
                if let Some(p) = pointer {
                    let to = drop_index(&rects, from, p);
                    if to != from {
                        actions.push(BarAction::Move { from, to });
                    }
                }
                self.drag = None;
            }
        }
        actions
    }

    #[allow(clippy::too_many_arguments)]
    fn chip(
        &mut self,
        ui: &mut Ui,
        th: &Theme,
        i: usize,
        uid: u64,
        name: &str,
        info: &ChipInfo,
        colour: Color32,
        active: bool,
        master: bool,
        actions: &mut Vec<BarAction>,
    ) -> Rect {
        let text = if active { th.ui.text } else { th.ui.disabled };
        let nom = ui
            .painter()
            .layout_no_wrap(name.to_string(), FontId::proportional(NAME_SIZE), text);
        let drone = ui.painter().layout_no_wrap(
            info.drone.clone(),
            FontId::proportional(DRONE_SIZE),
            if active { th.ui.muted } else { th.ui.disabled },
        );
        let dot = if master { DOT_MASTER } else { DOT };
        let drone_w = if info.drone.is_empty() {
            0.0
        } else {
            GAP + drone.size().x
        };
        let w = PAD_LEFT + dot + GAP + nom.size().x + drone_w + GAP + CLOSE + PAD_RIGHT;
        let id = ui.scope_id().with(("pastille", uid));
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, CHIP_H), Sense::click_and_drag());
        let resp = resp.on_hover_cursor(CursorIcon::Grab);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, active, name));
        let p = ui.painter();
        // fond et trait : plein si actif, pointillés sur fond de fenêtre sinon
        let radius = CHIP_H / 2.0;
        if active {
            p.rect(
                rect,
                radius,
                th.ui.base,
                Stroke::new(1.0, th.ui.mid),
                egui::StrokeKind::Inside,
            );
        } else {
            p.rect_filled(rect, radius, th.ui.window);
            p.extend(Shape::dashed_line(
                &pill(rect.shrink(0.5)),
                Stroke::new(1.0, th.ui.mid),
                DASH,
                DASH_GAP,
            ));
        }
        if self.drag == Some(i) {
            p.rect_stroke(
                rect,
                radius,
                Stroke::new(1.0, th.ui.highlight),
                egui::StrokeKind::Inside,
            );
        }
        // le rond : la couleur du fragment, grisée s'il est désactivé ; coche blanche pour le maître
        let c = Pos2::new(rect.min.x + PAD_LEFT + dot / 2.0, rect.center().y);
        let fill = if active { colour } else { th.ui.mid };
        p.circle_filled(c, dot / 2.0, fill);
        if master {
            icon("check", CHECK)
                .tint(plot::ON_ACCENT)
                .paint_at(ui, Rect::from_center_size(c, Vec2::splat(CHECK)));
        }
        let x = rect.min.x + PAD_LEFT + dot + GAP;
        let y = rect.center().y;
        let name_rect = Rect::from_min_size(Pos2::new(x, y - nom.size().y / 2.0), nom.size());
        let p = ui.painter();
        p.galley(name_rect.min, nom.clone(), text);
        if !active {
            // barré, comme dans la maquette (`text-decoration: line-through`)
            p.hline(name_rect.x_range(), y, Stroke::new(1.0, text));
        }
        if !info.drone.is_empty() {
            p.galley(Pos2::new(name_rect.max.x + GAP, y - drone.size().y / 2.0), drone, text);
        }
        // gestes : le rond et le « × » passent avant la pastille (ils sont posés après elle)
        let dot_rect = Rect::from_center_size(c, Vec2::splat(dot));
        let dot_resp = ui
            .interact(dot_rect, id.with("rond"), Sense::click())
            .on_hover_text(if master {
                "Fragment maître"
            } else {
                "Cliquer pour en faire le fragment maître"
            });
        dot_resp
            .widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, master, master_label(name, master)));
        let close_rect = Rect::from_center_size(Pos2::new(rect.max.x - PAD_RIGHT - CLOSE / 2.0, y), Vec2::splat(CLOSE));
        let close_resp = ui
            .interact(close_rect, id.with("retirer"), Sense::click())
            .on_hover_text(remove_label(name));
        close_resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, remove_label(name)));
        if close_resp.hovered() {
            ui.painter()
                .circle_filled(close_rect.center(), CLOSE / 2.0, th.ui.hover);
        }
        icon("x", CLOSE_ICON)
            .tint(th.ui.muted)
            .paint_at(ui, Rect::from_center_size(close_rect.center(), Vec2::splat(CLOSE_ICON)));
        if close_resp.clicked() {
            actions.push(BarAction::Remove(i));
        } else if dot_resp.clicked() {
            actions.push(BarAction::Master(i));
        } else if resp.clicked() {
            actions.push(BarAction::Toggle(i));
        }
        if resp.drag_started() {
            self.drag = Some(i);
        }
        if self.drag.is_none() {
            let tip = if active {
                "Cliquer pour désactiver, glisser pour réordonner"
            } else {
                "Désactivé · cliquer pour réactiver, glisser pour réordonner"
            };
            resp.on_hover_ui(|ui| details(ui, th, name, info, tip));
        }
        rect
    }
}

/// La fiche d'un fragment, au survol de sa pastille (maquette : la pastille s'ouvre vers le bas).
fn details(ui: &mut Ui, th: &Theme, name: &str, info: &ChipInfo, tip: &str) {
    ui.label(egui::RichText::new(name).size(NAME_SIZE).strong().color(th.ui.text));
    egui::Grid::new("fiche-pastille")
        .num_columns(2)
        .spacing([10.0, 3.0])
        .show(ui, |ui| {
            for (k, v) in [
                ("Rôle", &info.role),
                ("Date", &info.date),
                ("Début", &info.start),
                ("Durée", &info.duration),
                ("PID roll", &info.pid),
                ("Firmware", &info.firmware),
            ] {
                ui.label(egui::RichText::new(k).size(DRONE_SIZE).color(th.ui.muted));
                let v = if v.is_empty() { "—" } else { v.as_str() };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(v).size(DRONE_SIZE).monospace().color(th.ui.text));
                });
                ui.end_row();
            }
        });
    ui.label(egui::RichText::new(tip).size(DRONE_SIZE).color(th.ui.muted));
}

/// Le contour d'une pastille (deux demi-cercles reliés), en points, pour les pointillés.
fn pill(r: Rect) -> Vec<Pos2> {
    let rad = r.height() / 2.0;
    let (cl, cr) = (
        Pos2::new(r.min.x + rad, r.center().y),
        Pos2::new(r.max.x - rad, r.center().y),
    );
    let mut pts = Vec::new();
    let n = 12;
    // haut, de gauche à droite, puis le demi-cercle droit, le bas, et le demi-cercle gauche
    for k in 0..=n {
        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / n as f32;
        pts.push(cr + rad * Vec2::new(a.cos(), a.sin()));
    }
    for k in 0..=n {
        let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / n as f32;
        pts.push(cl + rad * Vec2::new(a.cos(), a.sin()));
    }
    pts.push(pts[0]);
    pts
}
