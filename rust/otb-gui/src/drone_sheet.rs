//! La fiche d'un drone : le second écran **hors rail** du prototype (D-060), ouvert par le crayon d'une carte du
//! tableau de bord, par « Nouveau drone », ou par le Guide. Une seule fiche par drone, rangée **dans la
//! bibliothèque** (D-068) : le Guide la lit, il n'a plus de fichier à lui.
//!
//! Le fichier se coupe en deux : ce qui n'a pas de fenêtre (choix proposés, traduction pour l'assistant, reprise de
//! l'ancien fichier des fiches, enregistrement), testé sans écran ; puis l'écran, au dessin du prototype
//! (`docs/maquettes/Application.dc.html`, écran `droneEdit`), dont les mesures sont des constantes nommées.
//!
//! Écarts au prototype, écrits ici et dans D-068 (D-061 : un écart se justifie par écrit) :
//! - **pas de « Fiche cible »** : elle arrive avec le suivi des mesures (lot 31), seul à s'en servir ; sa colonne
//!   reste vide pour que la colonne de gauche garde la largeur du prototype ;
//! - **taille « 8″ et plus »** en plus des sept du prototype : l'assistant a un profil pour ces drones ;
//! - **boucle PID à 1,6 kHz** en plus des cinq du prototype : la moitié d'un gyro BMI270, déjà proposée avant.

use egui::{Align, Layout, RichText, Ui, Vec2};
use otb_advisor::drone_card::{DEFAULT_SIZE, DroneCard};
use otb_store::Library;
use otb_store::model::{Config, Drone, DroneSheet, config_changes};

use crate::icons::icon;
use crate::theme::{Theme, file_color, plot, with_alpha};
use crate::tracking::{Progress, TrackingView};

// ---------------------------------------------------------------- choix proposés

/// Tailles proposées et profil de l'assistant qu'elles choisissent (`otb_advisor::profiles::PROFILES`).
pub const SIZES: [(&str, &str); 8] = [
    ("65 mm", "whoop"),
    ("2,5″", "micro"),
    ("3″", "micro"),
    ("4″", "five"),
    ("5″", "five"),
    ("6″", "seven"),
    ("7″", "seven"),
    ("8″ et plus", "large"),
];
pub const CELLS: [&str; 6] = ["1S", "2S", "3S", "4S", "5S", "6S"];
/// Dans l'ordre du prototype ; les valeurs sont celles de `otb_advisor::drone_card::ESCS`.
pub const ESC_CHOICES: [&str; 4] = ["BLHeli_S", "Bluejay", "BLHeli_32", "AM32"];
pub const BF_VERSIONS: [&str; 4] = ["4.4", "4.5", "2025.12", "2026.6"];
/// Fréquences de la boucle PID en kHz, écrites avec un point (affichées avec une virgule).
pub const PID_KHZ: [&str; 6] = ["8", "4", "3.2", "2", "1.6", "1"];

/// La fiche d'un drone neuf : les valeurs que le prototype propose.
pub fn new_sheet() -> DroneSheet {
    DroneSheet {
        size: "5″".into(),
        cells: "6S".into(),
        poles: "14".into(),
        esc: "BLHeli_32".into(),
        betaflight: "2026.6".into(),
        pid_khz: "8".into(),
        ..DroneSheet::default()
    }
}

/// Le profil de l'assistant d'une taille affichée ; une taille inconnue prend le profil par défaut.
pub fn profile_of_size(size: &str) -> &'static str {
    SIZES.iter().find(|s| s.0 == size).map_or(DEFAULT_SIZE, |s| s.1)
}

/// La taille affichée pour un profil de l'assistant (reprise d'une ancienne fiche, fiche tirée d'un log). Un
/// profil couvre plusieurs tailles : on prend la plus courante.
pub fn size_of_profile(key: &str) -> &'static str {
    match key {
        "whoop" => "65 mm",
        "micro" => "3″",
        "seven" => "7″",
        "large" => "8″ et plus",
        _ => "5″",
    }
}

/// La version de Betaflight proposée qui correspond au firmware d'un log (« Betaflight 2026.6.0-rc3 … »).
pub fn version_of_firmware(firmware: &str) -> Option<&'static str> {
    let v = firmware.split_whitespace().nth(1)?;
    BF_VERSIONS
        .iter()
        .copied()
        .find(|b| v.starts_with(&format!("{b}.")) || v == *b)
}

/// La famille de version que l'assistant connaît : 4.4, 4.5, ou 2025.12 et plus récent.
fn family(version: &str) -> &'static str {
    match version {
        "4.4" => "4.4",
        "4.5" => "4.5",
        _ => "2025.12",
    }
}

// ---------------------------------------------------------------- traduction pour l'assistant

/// La fiche du Guide (`DroneCard`) d'un drone de la bibliothèque, `None` s'il n'a pas de fiche.
pub fn card_of(drone: &Drone) -> Option<DroneCard> {
    let s = drone.sheet.as_ref()?;
    let mut c = DroneCard::new(&drone.name);
    c.bf = family(&s.betaflight).to_string();
    c.size = profile_of_size(&s.size).to_string();
    c.pid_khz = s.pid_khz.parse().unwrap_or(c.pid_khz);
    if ESC_CHOICES.contains(&s.esc.as_str()) {
        c.esc = s.esc.clone();
    }
    c.done = s.done.iter().map(|k| (k.clone(), true)).collect();
    Some(c)
}

/// Les fiches du Guide de toute la bibliothèque, dans l'ordre des drones.
pub fn cards_of(lib: &Library) -> Vec<DroneCard> {
    lib.drones().iter().filter_map(card_of).collect()
}

/// Une fiche de bibliothèque d'après une fiche du Guide : reprise de l'ancien fichier, fiche tirée d'un log.
/// Ce que l'ancienne fiche ne disait pas (batterie, poids…) reste vide.
pub fn sheet_of_card(card: &DroneCard) -> DroneSheet {
    let pid = PID_KHZ
        .iter()
        .find(|k| k.parse::<f64>().is_ok_and(|v| v == card.pid_khz))
        .map_or_else(|| card.pid_khz.to_string(), |k| k.to_string());
    DroneSheet {
        size: size_of_profile(&card.size).to_string(),
        esc: card.esc.clone(),
        betaflight: card.bf.clone(),
        pid_khz: pid,
        done: card.done.iter().filter(|d| d.1).map(|d| d.0.clone()).collect(),
        ..DroneSheet::default()
    }
}

/// Reprend les fiches de l'ancien fichier du Guide (`drones.json`, D-039) dans la bibliothèque. Un drone qui a
/// déjà une fiche garde la sienne (la bibliothèque gagne) ; un drone connu par ses seuls logs reçoit celle-ci ;
/// sinon le drone est créé, son nom pour identifiant. Rend le nombre de fiches reprises.
pub fn import_cards(lib: &mut Library, cards: &[DroneCard]) -> usize {
    let mut n = 0;
    for card in cards {
        let mut d = lib
            .drone_by_name(&card.name)
            .cloned()
            .unwrap_or_else(|| Drone::new(&free_id(lib, &card.name), &card.name));
        if d.sheet.is_some() {
            continue;
        }
        d.sheet = Some(sheet_of_card(card));
        lib.put_drone(d);
        n += 1;
    }
    n
}

/// Un identifiant libre pour un drone créé à la main : son nom, suivi d'un numéro s'il est pris.
fn free_id(lib: &Library, name: &str) -> String {
    let mut id = name.to_string();
    let mut n = 2;
    while lib.drone(&id).is_some() {
        id = format!("{name} ({n})");
        n += 1;
    }
    id
}

// ---------------------------------------------------------------- formulaire et enregistrement

/// Ce que l'écran est en train de modifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SheetForm {
    /// Identifiant du drone modifié ; `None` : nouveau drone.
    pub id: Option<String>,
    pub name: String,
    pub sheet: DroneSheet,
    /// « Supprimer… » cliqué : on attend « Garder » ou « Supprimer définitivement ».
    pub ask_delete: bool,
    /// Configuration dépliée : (identifiant, `true` pour « Comparer », `false` pour « Voir »).
    pub shown: Option<(String, bool)>,
    /// Pourquoi le dernier « Enregistrer » a été refusé.
    pub error: String,
}

impl SheetForm {
    /// La fiche d'un drone de la bibliothèque ; un drone sans fiche part des valeurs d'un drone neuf, avec la
    /// version de Betaflight de ses logs quand on la reconnaît.
    pub fn edit(drone: &Drone) -> SheetForm {
        let sheet = drone.sheet.clone().unwrap_or_else(|| {
            let mut s = new_sheet();
            if let Some(v) = version_of_firmware(&drone.firmware) {
                s.betaflight = v.to_string();
            }
            s
        });
        SheetForm {
            id: Some(drone.id.clone()),
            name: drone.name.clone(),
            sheet,
            ask_delete: false,
            shown: None,
            error: String::new(),
        }
    }

    /// Un nouveau drone, éventuellement prérempli (nom et fiche tirés du log ouvert, depuis le Guide).
    pub fn new(name: &str, sheet: DroneSheet) -> SheetForm {
        SheetForm {
            id: None,
            name: name.to_string(),
            sheet,
            ask_delete: false,
            shown: None,
            error: String::new(),
        }
    }

    pub fn is_new(&self) -> bool {
        self.id.is_none()
    }

    /// Le nom enregistré : celui saisi, ou « Nouveau drone » s'il est vide (comme le prototype).
    fn saved_name(&self) -> String {
        let n = self.name.trim();
        if n.is_empty() {
            "Nouveau drone".to_string()
        } else {
            n.to_string()
        }
    }
}

/// Enregistre la fiche dans la bibliothèque ; rend l'identifiant du drone, ou pourquoi c'est refusé.
///
/// ⚠️ Le **nom** relie les logs à la fiche (`craft_name`) : deux drones ne peuvent pas le partager, sinon les logs
/// iraient au premier des deux. Un drone connu par ses seuls logs (sans fiche) **reçoit** la fiche d'un nouveau
/// drone du même nom, plutôt que d'en faire un second.
pub fn save_form(lib: &mut Library, form: &SheetForm) -> Result<String, String> {
    let name = form.saved_name();
    let same_name = lib.drone_by_name(&name).cloned();
    match &form.id {
        Some(id) => {
            if same_name.as_ref().is_some_and(|d| &d.id != id) {
                return Err(format!("Un autre drone porte déjà le nom « {name} »."));
            }
            let mut d = lib
                .drone(id)
                .cloned()
                .ok_or("Ce drone n'est plus dans la bibliothèque.")?;
            d.name = name;
            d.sheet = Some(form.sheet.clone());
            lib.put_drone(d);
            Ok(id.clone())
        }
        None => {
            let mut d = match same_name {
                Some(d) if d.sheet.is_some() => {
                    return Err(format!(
                        "Le drone « {name} » a déjà une fiche : ouvrez-la par le crayon de sa carte."
                    ));
                }
                Some(d) => d,
                None => Drone::new(&free_id(lib, &name), &name),
            };
            d.sheet = Some(form.sheet.clone());
            let id = d.id.clone();
            lib.put_drone(d);
            Ok(id)
        }
    }
}

/// « 2026-11-13T18:15:02 » → « 13 nov. 2026, 18:15 » ; une date seule n'a pas d'heure ; un texte illisible
/// reste tel quel.
pub fn date_fr(iso: &str) -> String {
    const MOIS: [&str; 12] = [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc.",
    ];
    let num = |a: usize, b: usize| iso.get(a..b).and_then(|s| s.parse::<usize>().ok());
    let (Some(an), Some(mois), Some(jour)) = (num(0, 4), num(5, 7), num(8, 10)) else {
        return iso.to_string();
    };
    let Some(m) = mois.checked_sub(1).and_then(|i| MOIS.get(i)) else {
        return iso.to_string();
    };
    match iso.get(11..16) {
        Some(h) if h.as_bytes().get(2) == Some(&b':') => format!("{jour} {m} {an}, {h}"),
        _ => format!("{jour} {m} {an}"),
    }
}

/// La ligne de résumé d'une configuration : ses deux premiers changements, ou « configuration initiale », puis
/// le premier log du drone rangé ce jour-là ou après (« avant FINAL »).
pub fn config_summary(lib: &Library, configs: &[&Config], i: usize) -> String {
    let c = configs[i];
    let mut s = match i.checked_sub(1).map(|p| configs[p]) {
        None => "configuration initiale".to_string(),
        Some(prev) => {
            let ch = config_changes(&prev.text, &c.text);
            let mut parts: Vec<String> = ch.iter().take(2).map(change_text).collect();
            if ch.len() > 2 {
                parts.push(format!("+ {}", ch.len() - 2));
            }
            if parts.is_empty() {
                "aucun réglage changé".to_string()
            } else {
                parts.join(", ")
            }
        }
    };
    let jour = c.recorded.get(..10).unwrap_or(&c.recorded);
    if let Some(l) = lib
        .logs()
        .iter()
        .filter(|l| l.drone_id == c.drone_id && l.added.as_str() >= jour)
        .min_by(|a, b| a.added.cmp(&b.added))
    {
        let nom = std::path::Path::new(&l.name)
            .file_stem()
            .map_or(l.name.clone(), |n| n.to_string_lossy().into_owned());
        s.push_str(&format!(" · avant {nom}"));
    }
    s
}

fn change_text(c: &(String, String, String)) -> String {
    let v = |x: &str| {
        if x.is_empty() {
            "défaut".to_string()
        } else {
            x.to_string()
        }
    };
    format!("{} {} → {}", c.0, v(&c.1), v(&c.2))
}

// ---------------------------------------------------------------- écran

/// Ce que l'utilisateur demande depuis la fiche ; `App` l'exécute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SheetAction {
    /// Retour ou « Annuler » : rien n'est enregistré.
    Leave,
    /// « Enregistrer » / « Créer le drone ».
    Save,
    /// « Supprimer définitivement » : ce drone.
    Delete(String),
}

// mesures relevées dans le prototype (écran `droneEdit`)
const PAGE_PAD_X: i8 = 26;
const PAGE_PAD_Y: i8 = 18;
const GAP: f32 = 14.0;
const BACK: f32 = 34.0;
const BADGE: f32 = 40.0;
const TITLE: f32 = 22.0;
const KICKER: f32 = 12.0;
/// Colonne de gauche : `minmax(0, 1.15fr) minmax(0, 1fr)`.
const LEFT_SHARE: f32 = 1.15 / 2.15;
const SECTION_RADIUS: u8 = 10;
const SECTION_PAD_X: i8 = 18;
const SECTION_PAD_Y: i8 = 16;
const H2: f32 = 15.0;
const LABEL: f32 = 12.5;
const HINT: f32 = 11.5;
const FIELD: f32 = 14.0;
const FIELD_RADIUS: u8 = 6;
const MONO: f32 = 13.5;
/// Opacité du teint du badge (`couleur + '1f'`) et du trait du cadre « Supprimer » (prototype : `#f1c7c4`, le
/// rouge du thème à 25 % sur le blanc).
const TINT: u8 = 0x1f;
const DANGER_LINE: u8 = 64;

#[derive(Default)]
pub struct DroneSheetPage {
    /// `None` : aucune fiche ouverte (l'écran ne s'affiche pas).
    pub form: Option<SheetForm>,
}

impl DroneSheetPage {
    /// `suivi` : où en sont les mesures du drone (`None` pour un drone pas encore enregistré).
    pub fn ui(&mut self, ui: &mut Ui, lib: &Library, th: &Theme, suivi: Option<&TrackingView>) -> Vec<SheetAction> {
        let mut actions = Vec::new();
        let Some(form) = self.form.as_mut() else {
            actions.push(SheetAction::Leave);
            return actions;
        };
        let rang = form
            .id
            .as_ref()
            .and_then(|id| lib.drones().iter().position(|d| &d.id == id))
            .unwrap_or(lib.drones().len());
        let couleur = file_color(rang);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(th.ui.window).inner_margin(egui::Margin {
                left: PAGE_PAD_X,
                right: PAGE_PAD_X,
                top: PAGE_PAD_Y,
                bottom: PAGE_PAD_Y,
            }))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = GAP;
                header(ui, form, couleur, th, &mut actions);
                if !form.error.is_empty() {
                    ui.label(RichText::new(&form.error).color(th.accent("red")));
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let largeur = ((ui.available_width() - GAP) * LEFT_SHARE).floor();
                    ui.allocate_ui_with_layout(Vec2::new(largeur, 0.0), Layout::top_down(Align::LEFT), |ui| {
                        ui.set_width(largeur);
                        ui.spacing_mut().item_spacing.y = GAP;
                        identity(ui, form, lib, th);
                        machine(ui, form, th);
                        configurations(ui, form, lib, th);
                        if let Some(v) = suivi {
                            tracking(ui, th, v);
                        }
                        if let Some(id) = form.id.clone() {
                            delete(ui, form, lib, &id, th, &mut actions);
                        }
                    });
                });
            });
        actions
    }
}

/// Retour, badge, titre, « Annuler » et « Enregistrer ».
fn header(ui: &mut Ui, form: &SheetForm, couleur: egui::Color32, th: &Theme, actions: &mut Vec<SheetAction>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        let retour = egui::Button::image(
            icon("arrow-left", 18.0)
                .tint(th.ui.text)
                .alt_text("Retour au tableau de bord"),
        )
        .min_size(Vec2::splat(BACK))
        .fill(th.ui.base)
        .stroke(egui::Stroke::new(1.0, th.ui.mid))
        .corner_radius(8);
        if ui.add(retour).on_hover_text("Revenir sans enregistrer").clicked() {
            actions.push(SheetAction::Leave);
        }
        let (badge, _) = ui.allocate_exact_size(Vec2::splat(BADGE), egui::Sense::hover());
        ui.painter().rect_filled(badge, 10, with_alpha(couleur, TINT));
        icon("drone", 22.0)
            .tint(couleur)
            .paint_at(ui, egui::Rect::from_center_size(badge.center(), Vec2::splat(22.0)));
        let (kicker, titre) = if form.is_new() {
            let n = form.name.trim();
            (
                "Tableau de bord",
                if n.is_empty() {
                    "Nouveau drone".to_string()
                } else {
                    format!("Nouveau drone : {n}")
                },
            )
        } else {
            let n = form.name.trim();
            (
                "Tableau de bord · fiche du drone",
                if n.is_empty() {
                    "Sans nom".to_string()
                } else {
                    n.to_string()
                },
            )
        };
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.label(RichText::new(kicker).size(KICKER).color(th.ui.muted));
            ui.label(RichText::new(titre).size(TITLE).color(th.ui.text));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.spacing_mut().button_padding = Vec2::new(14.0, 8.0);
            let on_accent = plot::ON_ACCENT;
            let libelle = if form.is_new() {
                "Créer le drone"
            } else {
                "Enregistrer"
            };
            let enregistrer = egui::Button::image_and_text(
                icon("check", 15.0).tint(on_accent),
                RichText::new(libelle).size(13.0).strong().color(on_accent),
            )
            .fill(th.ui.highlight)
            .corner_radius(6);
            if ui.add(enregistrer).clicked() {
                actions.push(SheetAction::Save);
            }
            let annuler = egui::Button::new(RichText::new("Annuler").size(13.0).color(th.ui.text))
                .fill(th.ui.base)
                .stroke(egui::Stroke::new(1.0, th.ui.mid))
                .corner_radius(6);
            if ui.add(annuler).clicked() {
                actions.push(SheetAction::Leave);
            }
        });
    });
}

/// Un cadre de section : fond blanc, trait fin, coins de 10, titre de 15 et sous-titre facultatif.
fn section(ui: &mut Ui, th: &Theme, title: &str, subtitle: Option<&str>, body: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(th.ui.base)
        .stroke(egui::Stroke::new(1.0, th.ui.border))
        .corner_radius(SECTION_RADIUS)
        .inner_margin(egui::Margin {
            left: SECTION_PAD_X,
            right: SECTION_PAD_X,
            top: SECTION_PAD_Y,
            bottom: SECTION_PAD_Y,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = GAP;
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(title).size(H2).strong().color(th.ui.text));
                if let Some(s) = subtitle {
                    ui.label(RichText::new(s).size(LABEL).color(th.ui.muted));
                }
            });
            body(ui);
        });
}

/// Libellé d'un champ (12,5, gras) ; rend sa réponse pour relier le champ à son libellé (lecteur d'écran, tests).
fn field_label(ui: &mut Ui, th: &Theme, text: &str) -> egui::Response {
    ui.label(RichText::new(text).size(LABEL).strong().color(th.ui.text))
}

fn hint(ui: &mut Ui, th: &Theme, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(HINT).color(th.ui.muted)).wrap());
}

/// Un champ de saisie encadré, avec son unité à droite (`g`, `KV`) s'il en a une, et sa note dessous.
fn text_field(ui: &mut Ui, th: &Theme, label: &str, value: &mut String, unit: Option<&str>, note: Option<&str>) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        let l = field_label(ui, th, label);
        egui::Frame::new()
            .fill(th.ui.base)
            .stroke(egui::Stroke::new(1.0, th.ui.mid))
            .corner_radius(FIELD_RADIUS)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let reste = unit.map_or(0.0, |_| 24.0);
                    ui.add(
                        egui::TextEdit::singleline(value)
                            .frame(egui::Frame::NONE)
                            .font(egui::FontId::proportional(FIELD))
                            .text_color(th.ui.text)
                            .desired_width(ui.available_width() - reste),
                    )
                    .labelled_by(l.id);
                    if let Some(u) = unit {
                        ui.label(RichText::new(u).size(LABEL).color(th.ui.muted));
                    }
                });
            });
        if let Some(n) = note {
            hint(ui, th, n);
        }
    });
}

/// Une liste déroulante qui prend la largeur de sa colonne ; `shown` traduit une valeur pour l'affichage.
#[allow(clippy::too_many_arguments)]
fn select(
    ui: &mut Ui,
    th: &Theme,
    label: &str,
    id: &str,
    choices: &[&str],
    value: &mut String,
    shown: fn(&str) -> String,
    note: Option<&str>,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        let l = field_label(ui, th, label);
        // la hauteur d'un champ de saisie (14 points de texte, 8 de marge en haut et en bas, le trait) : le
        // prototype donne aux listes la même que celle des champs
        ui.spacing_mut().interact_size.y = FIELD + 2.0 * 8.0 + 6.0;
        ui.spacing_mut().button_padding = Vec2::new(10.0, 8.0);
        ui.visuals_mut().widgets.inactive.corner_radius = FIELD_RADIUS.into();
        ui.visuals_mut().widgets.hovered.corner_radius = FIELD_RADIUS.into();
        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::new(1.0, th.ui.mid);
        ui.visuals_mut().widgets.inactive.weak_bg_fill = th.ui.base;
        egui::ComboBox::from_id_salt(("fiche-drone", id))
            .width(ui.available_width())
            .selected_text(RichText::new(shown(value)).size(FIELD).color(th.ui.text))
            .show_ui(ui, |ui| {
                for c in choices {
                    ui.selectable_value(value, (*c).to_string(), shown(c));
                }
            })
            .response
            .labelled_by(l.id);
        if let Some(n) = note {
            hint(ui, th, n);
        }
    });
}

fn as_is(v: &str) -> String {
    v.to_string()
}

fn khz(v: &str) -> String {
    v.replace('.', ",")
}

/// Une valeur lue au branchement USB : encadrée, sur fond gris, en chiffres de machine.
fn read_only(ui: &mut Ui, th: &Theme, label: &str, value: &str, note: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 5.0;
        field_label(ui, th, label);
        egui::Frame::new()
            .fill(th.ui.alt)
            .stroke(egui::Stroke::new(1.0, th.ui.mid))
            .corner_radius(FIELD_RADIUS)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(RichText::new(value).size(MONO).monospace().color(th.ui.muted)).truncate());
            });
        hint(ui, th, note);
    });
}

fn identity(ui: &mut Ui, form: &mut SheetForm, lib: &Library, th: &Theme) {
    let drone = form.id.as_ref().and_then(|id| lib.drone(id));
    // l'identifiant n'est celui de la carte que si le drone a été vu par USB (sa carte est alors connue)
    let (uid, carte) = match drone {
        Some(d) if !d.board.is_empty() => (d.id.clone(), d.board.clone()),
        _ => (
            "en attente du branchement".to_string(),
            "en attente du branchement".to_string(),
        ),
    };
    section(ui, th, "Identité", None, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            text_field(
                ui,
                th,
                "Nom du drone",
                &mut form.name,
                None,
                Some("Doit être le nom du drone dans Betaflight (craft_name) : c’est lui qui relie les logs à cette fiche."),
            );
        });
        ui.columns(2, |c| {
            read_only(
                &mut c[0],
                th,
                "Identifiant de la carte (UID)",
                &uid,
                "Lu automatiquement au branchement USB.",
            );
            read_only(&mut c[1], th, "Carte de vol", &carte, "Lue au branchement USB.");
        });
    });
}

fn machine(ui: &mut Ui, form: &mut SheetForm, th: &Theme) {
    let s = &mut form.sheet;
    section(ui, th, "Machine", None, |ui| {
        let taille = Some("Sert de profil aux seuils de l’Assistant.");
        ui.columns(3, |c| {
            select(
                &mut c[0],
                th,
                "Taille",
                "taille",
                &SIZES.map(|x| x.0),
                &mut s.size,
                as_is,
                taille,
            );
            select(&mut c[1], th, "Batterie", "batterie", &CELLS, &mut s.cells, as_is, None);
            text_field(&mut c[2], th, "Poids", &mut s.weight, Some("g"), None);
        });
        ui.columns(3, |c| {
            text_field(&mut c[0], th, "Moteurs", &mut s.kv, Some("KV"), None);
            text_field(
                &mut c[1],
                th,
                "Pôles moteur",
                &mut s.poles,
                None,
                Some("Pour le filtre RPM."),
            );
            text_field(&mut c[2], th, "Hélices", &mut s.props, None, None);
        });
        ui.columns(3, |c| {
            select(&mut c[0], th, "ESC", "esc", &ESC_CHOICES, &mut s.esc, as_is, None);
            select(
                &mut c[1],
                th,
                "Betaflight",
                "betaflight",
                &BF_VERSIONS,
                &mut s.betaflight,
                as_is,
                None,
            );
            select(
                &mut c[2],
                th,
                "Boucle PID",
                "boucle-pid",
                &PID_KHZ,
                &mut s.pid_khz,
                khz,
                Some("en kHz"),
            );
        });
    });
}

fn small_button(ui: &mut Ui, th: &Theme, text: &str, on: bool) -> egui::Response {
    ui.spacing_mut().button_padding = Vec2::new(9.0, 4.0);
    ui.add(
        egui::Button::new(RichText::new(text).size(12.0).color(th.ui.text))
            .fill(if on { th.ui.select } else { th.ui.base })
            .stroke(egui::Stroke::new(1.0, th.ui.mid))
            .corner_radius(6)
            .selected(on),
    )
}

/// Les lignes de la section « Suivi des mesures » : où en est le calcul, combien de fragments sont mesurés, et ceux
/// qui n'ont pas pu l'être (avec la raison, pour que le pilote sache quoi faire). Fonction pure : elle se teste sans
/// écran (L-071).
pub fn tracking_lines(v: &TrackingView) -> Vec<String> {
    let pluriel = |n: usize| if n > 1 { "s" } else { "" };
    let mut out = Vec::new();
    match v.progress {
        Progress::Reading => out.push("Lecture des mesures déjà faites…".to_string()),
        Progress::Measuring { done, total } => out.push(format!(
            "Mesure en arrière-plan : {done} fragment{} sur {total}…",
            pluriel(done)
        )),
        Progress::Idle if v.total == 0 => {
            out.push("Aucun fragment : les mesures viendront avec les logs de ce drone.".to_string())
        }
        Progress::Idle => out.push(format!(
            "{} fragment{} mesuré{} sur {}.",
            v.measured,
            pluriel(v.measured),
            pluriel(v.measured),
            v.total
        )),
    }
    for (name, why) in &v.failed {
        out.push(format!("Non mesuré : « {name} » — {why}"));
    }
    out
}

/// « Suivi des mesures » : l'état du calcul des grandeurs de chaque fragment (lot 31a).
fn tracking(ui: &mut Ui, th: &Theme, v: &TrackingView) {
    section(
        ui,
        th,
        "Suivi des mesures",
        Some(
            "Ce que l'assistant mesure sur chaque fragment de ce drone, gardé pour suivre son évolution d'un vol à l'autre.",
        ),
        |ui| {
            for line in tracking_lines(v) {
                hint(ui, th, &line);
            }
        },
    );
}

fn configurations(ui: &mut Ui, form: &mut SheetForm, lib: &Library, th: &Theme) {
    let configs: Vec<&Config> = form.id.as_ref().map(|id| lib.configs_of_drone(id)).unwrap_or_default();
    section(
        ui,
        th,
        "Configurations Betaflight",
        Some("Un « diff all » enregistré à chaque branchement, quand il a changé."),
        |ui| {
            if configs.is_empty() {
                ui.label(
                    RichText::new("Aucune configuration : elle sera enregistrée au premier branchement.")
                        .size(13.0)
                        .color(th.ui.muted),
                );
                return;
            }
            ui.spacing_mut().item_spacing.y = 0.0;
            // la plus récente en haut
            for i in (0..configs.len()).rev() {
                let c = configs[i];
                let ligne = ui.horizontal(|ui| {
                    ui.add_space(0.0);
                    ui.vertical(|ui| {
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(
                            RichText::new(date_fr(&c.recorded))
                                .size(13.0)
                                .strong()
                                .color(th.ui.text),
                        );
                        ui.add(
                            egui::Label::new(
                                RichText::new(config_summary(lib, &configs, i))
                                    .size(12.0)
                                    .color(th.ui.muted),
                            )
                            .truncate(),
                        );
                        ui.add_space(8.0);
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let comparer = form.shown == Some((c.id.clone(), true));
                        let voir = form.shown == Some((c.id.clone(), false));
                        let b = ui
                            .add_enabled_ui(i > 0, |ui| small_button(ui, th, "Comparer", comparer))
                            .inner;
                        if b.on_disabled_hover_text("Première configuration : rien à comparer")
                            .clicked()
                        {
                            form.shown = if comparer { None } else { Some((c.id.clone(), true)) };
                        }
                        if small_button(ui, th, "Voir", voir).clicked() {
                            form.shown = if voir { None } else { Some((c.id.clone(), false)) };
                        }
                    });
                });
                let r = ligne.response.rect;
                ui.painter()
                    .hline(r.x_range(), r.bottom(), egui::Stroke::new(1.0, th.ui.border));
                match &form.shown {
                    Some((id, false)) if id == &c.id => config_text(ui, th, &c.text),
                    Some((id, true)) if id == &c.id && i > 0 => {
                        let lignes: Vec<String> = config_changes(&configs[i - 1].text, &c.text)
                            .iter()
                            .map(change_text)
                            .collect();
                        let texte = if lignes.is_empty() {
                            "Aucun réglage changé.".to_string()
                        } else {
                            lignes.join("\n")
                        };
                        config_text(ui, th, &texte);
                    }
                    _ => {}
                }
            }
        },
    );
}

/// Le texte d'une configuration ou de ses changements, en chiffres de machine, dans un cadre qui défile.
fn config_text(ui: &mut Ui, th: &Theme, text: &str) {
    ui.add_space(8.0);
    egui::Frame::new()
        .fill(th.ui.alt)
        .corner_radius(FIELD_RADIUS)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
                ui.label(RichText::new(text).size(12.0).monospace().color(th.ui.text));
            });
        });
    ui.add_space(8.0);
}

fn delete(ui: &mut Ui, form: &mut SheetForm, lib: &Library, id: &str, th: &Theme, actions: &mut Vec<SheetAction>) {
    let n = lib.fragments_of_drone(id).len();
    let info = format!(
        "{n} fragment{s} de ce drone {v} retiré{s} de la bibliothèque et de la liste d’analyse.",
        s = if n > 1 { "s" } else { "" },
        v = if n > 1 { "seront" } else { "sera" },
    );
    let rouge = th.accent("red");
    egui::Frame::new()
        .fill(th.ui.base)
        .stroke(egui::Stroke::new(1.0, with_alpha(rouge, DANGER_LINE)))
        .corner_radius(SECTION_RADIUS)
        .inner_margin(egui::Margin::symmetric(SECTION_PAD_X, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // les boutons d'abord, à droite ; le texte prend ce qui reste et passe à la ligne (sinon il glisse
            // sous les boutons)
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.spacing_mut().button_padding = Vec2::new(12.0, 6.0);
                if form.ask_delete {
                    let oui = egui::Button::new(
                        RichText::new("Supprimer définitivement")
                            .size(13.0)
                            .color(th.on_accent("red")),
                    )
                    .fill(rouge)
                    .corner_radius(6);
                    if ui.add(oui).clicked() {
                        actions.push(SheetAction::Delete(id.to_string()));
                    }
                    let garder = egui::Button::new(RichText::new("Garder").size(13.0).color(th.ui.text))
                        .fill(th.ui.base)
                        .stroke(egui::Stroke::new(1.0, th.ui.mid))
                        .corner_radius(6);
                    if ui.add(garder).clicked() {
                        form.ask_delete = false;
                    }
                } else {
                    let demander = egui::Button::new(RichText::new("Supprimer…").size(13.0).color(rouge))
                        .fill(th.ui.base)
                        .stroke(egui::Stroke::new(1.0, rouge))
                        .corner_radius(6);
                    if ui.add(demander).clicked() {
                        form.ask_delete = true;
                    }
                }
                ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(
                        RichText::new("Supprimer ce drone")
                            .size(13.0)
                            .strong()
                            .color(th.ui.text),
                    );
                    ui.add(egui::Label::new(RichText::new(info).size(LABEL).color(th.ui.muted)).wrap());
                });
            });
        });
}

/// Au lancement : reprend l'ancien fichier des fiches du Guide dans la bibliothèque, l'écrit, puis renomme l'ancien
/// fichier en `.repris` (jamais effacé). Rend le message à montrer, `None` s'il n'y avait rien à reprendre.
///
/// ⚠️ Dans cet ordre : renommer **avant** d'avoir écrit la bibliothèque perdrait les fiches à la moindre coupure ;
/// ne pas renommer ferait revenir à chaque lancement un drone supprimé entre-temps.
pub fn import_old_file(lib: &mut Library, path: &std::path::Path) -> Option<String> {
    let cards = match crate::guide::read_old_cards(path) {
        Ok(c) if c.is_empty() => return None,
        Ok(c) => c,
        Err(e) => {
            return Some(format!(
                "Ancien fichier des fiches drone illisible ({}) : {e}. Il n'est pas repris et reste tel quel.",
                path.display()
            ));
        }
    };
    let n = import_cards(lib, &cards);
    if let Err(e) = lib.save() {
        return Some(format!(
            "Fiches drone reprises, mais la bibliothèque n'a pas pu s'écrire : {e}"
        ));
    }
    match crate::guide::set_aside_old_cards(path) {
        Ok(to) => Some(format!(
            "{n} fiche{s} drone reprise{s} dans la bibliothèque ; l'ancien fichier est gardé sous {}.",
            to.display(),
            s = if n > 1 { "s" } else { "" }
        )),
        Err(e) => Some(format!(
            "Fiches drone reprises ; l'ancien fichier n'a pas pu être renommé : {e}"
        )),
    }
}
