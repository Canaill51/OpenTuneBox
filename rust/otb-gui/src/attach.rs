//! Rattacher un log ouvert à une fiche drone quand son nom de machine ne le fait pas (D-084).
//!
//! Un log rejoint d'ordinaire le drone qui porte son nom de machine (D-068). Sans nom dans l'en-tête, ou avec un nom
//! qu'aucune fiche ne porte alors que la bibliothèque en a, l'application **demande** : une fenêtre, une ligne par
//! fichier, chacune avec sa liste de drones. « Ne pas ranger » garde l'ancien comportement (bornes pour la séance
//! seulement) ; l'écran d'extraction rouvre la fenêtre par « Rattacher à un drone… ».

use std::path::PathBuf;

use egui::RichText;
use otb_store::Library;

use crate::theme::Theme;
use crate::tools::common::safe_text;
use crate::usb::DroneChoice;

/// Un fichier ouvert qui attend sa fiche.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub path: PathBuf,
    /// Nom de machine lu dans l'en-tête (vide : aucun).
    pub craft: String,
    /// Le drone choisi dans la fenêtre, proposé d'abord par [`proposal`].
    pub choice: DroneChoice,
    /// Rôle demandé à l'ouverture (« Ouvrir les logs des filtres… » du Guide), donné aux fragments une fois rangés.
    pub role: Option<&'static str>,
    /// Le drone (identifiant) sous lequel la bibliothèque range **déjà** ce log : la fenêtre propose de l'y laisser.
    pub known: Option<String>,
    /// La fiche du Guide depuis laquelle le log a été ouvert (D-090), dite dans la raison de la question.
    pub card: Option<String>,
    /// Refus du dernier « Rattacher » (nom vide ou déjà pris, fichier disparu).
    pub error: Option<String>,
}

/// Ce que la fenêtre demande à `App`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachAction {
    /// Ranger ce fichier sous ce drone (créé s'il est nouveau).
    Attach(PathBuf, DroneChoice),
    /// « Ne pas ranger » : le fichier reste ouvert, rangé sous aucun drone, pour la séance.
    Skip(PathBuf),
}

/// La fenêtre « Rattacher à un drone » : vide, elle ne s'affiche pas.
#[derive(Default)]
pub struct AttachDialog {
    pub pending: Vec<Pending>,
}

/// Le drone proposé d'office : avec un nom de machine, le drone de ce nom s'il existe, sinon un nouveau drone de ce
/// nom ; sans nom, la fiche choisie dans le Guide (`guide_card`) si elle est dans la bibliothèque, sinon un nouveau
/// drone à nommer.
pub fn proposal(lib: &Library, craft: &str, guide_card: Option<&str>) -> DroneChoice {
    let craft = craft.trim();
    if !craft.is_empty() {
        return crate::usb::reader_choice(lib, craft);
    }
    guide_card
        .and_then(|n| lib.drone_by_name(n).or_else(|| lib.drone(n)))
        .map_or(DroneChoice::New(String::new()), |d| DroneChoice::Existing(d.id.clone()))
}

/// Faut-il demander pour un log qu'aucun drone ne reconnaît (ni empreinte connue, ni nom de fiche) ? Toujours sans
/// nom de machine ; avec un nom, seulement si la bibliothèque a déjà des drones — sinon il n'y a rien d'autre à
/// proposer qu'un nouveau drone de ce nom, rangé sans question comme avant.
pub fn must_ask(lib: &Library, craft: &str) -> bool {
    craft.trim().is_empty() || !lib.drones().is_empty()
}

/// Ce que décide l'ouverture d'un log depuis une fiche du Guide (D-090).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuideOpening {
    /// Le log est déjà rangé sous le drone de la fiche : rien à demander.
    Keep,
    /// Demander, le drone de la fiche proposé d'office (son identifiant).
    Ask(String),
}

/// Un log ouvert par « Ouvrir les logs des filtres… » ou « …de la série… » d'une fiche du Guide (`card`) : on le
/// rattache **toujours sur demande** (D-090), avec la fiche proposée d'office, même si son nom de machine désigne
/// cette fiche ou un autre drone, ou s'il est déjà rangé ailleurs (`known`, identifiant de ce drone). Seule
/// exception : déjà rangé sous le drone de la fiche, il n'y a rien à décider. `None` : la fiche n'est pas un drone
/// de la bibliothèque, la règle ordinaire s'applique (D-068, D-084).
pub fn guide_opening(lib: &Library, card: &str, known: Option<&str>) -> Option<GuideOpening> {
    let drone = lib.drone_by_name(card).or_else(|| lib.drone(card))?;
    Some(if known == Some(drone.id.as_str()) {
        GuideOpening::Keep
    } else {
        GuideOpening::Ask(drone.id.clone())
    })
}

/// Pourquoi la fenêtre demande, en une phrase sous le nom du fichier.
pub fn reason(lib: &Library, p: &Pending) -> String {
    let craft = p.craft.trim();
    let named = (!craft.is_empty()).then(|| lib.drone_by_name(craft)).flatten();
    let mut text = if let Some(k) = p.known.as_deref().and_then(|id| lib.drone(id)) {
        format!(
            "Déjà rangé sous « {} ». « Rattacher » le déplace sous le drone choisi, avec ses fragments.",
            k.name
        )
    } else if craft.is_empty() {
        "Pas de nom de machine dans l'en-tête.".to_string()
    } else if let Some(d) = named {
        format!("Nom de machine « {craft} » : c'est celui du drone « {} ».", d.name)
    } else {
        format!("Nom de machine « {craft} » : aucune fiche ne porte ce nom.")
    };
    if let Some(card) = &p.card {
        text.push_str(&format!(" Ouvert depuis la fiche « {card} » du Guide."));
    }
    text
}

impl AttachDialog {
    /// Ajoute un fichier à la fenêtre (une seule ligne par fichier).
    pub fn ask(&mut self, p: Pending) {
        if !self.pending.iter().any(|q| q.path == p.path) {
            self.pending.push(p);
        }
    }

    pub fn is_open(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Retire la ligne d'un fichier.
    pub fn forget(&mut self, path: &std::path::Path) {
        self.pending.retain(|p| p.path != path);
    }

    /// Dessine la fenêtre et rend les choix du cadre. Fermer la fenêtre (Échap, clic à côté) vaut « Ne pas ranger »
    /// pour chaque ligne.
    pub fn ui(&mut self, ctx: &egui::Context, lib: &Library, th: &Theme) -> Vec<AttachAction> {
        let mut actions = Vec::new();
        if self.pending.is_empty() {
            return actions;
        }
        let m = egui::Modal::new(egui::Id::unique("rattacher-log")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(RichText::new("Rattacher à un drone").size(17.0).strong());
            ui.label(
                RichText::new(
                    "Choisissez le drone de chacun de ces logs : ses fragments seront enregistrés dans la \
                     bibliothèque sous ce drone, et le Guide les comptera dans sa fiche.",
                )
                .color(th.ui.text),
            );
            for (i, p) in self.pending.iter_mut().enumerate() {
                ui.separator();
                let file = p
                    .path
                    .file_name()
                    .map_or_else(|| p.path.display().to_string(), |n| n.to_string_lossy().into_owned());
                ui.label(RichText::new(safe_text(&file)).strong());
                let why = reason(lib, p);
                ui.label(RichText::new(safe_text(&why)).size(12.0).color(th.ui.muted));
                let known = p.known.as_deref().and_then(|id| lib.drone(id));
                ui.horizontal_wrapped(|ui| {
                    crate::usb_page::drone_combo(
                        ui,
                        lib,
                        &mut p.choice,
                        p.craft.trim(),
                        &format!("rattacher-drone-{i}"),
                        &format!("Drone de {file}"),
                    );
                });
                ui.horizontal(|ui| {
                    let nom_vide = matches!(&p.choice, DroneChoice::New(n) if n.trim().is_empty());
                    let rattacher = egui::Button::new(
                        RichText::new("Rattacher")
                            .size(13.0)
                            .strong()
                            .color(crate::theme::plot::ON_ACCENT),
                    )
                    .fill(th.ui.highlight)
                    .stroke(egui::Stroke::new(1.0, th.ui.highlight))
                    .corner_radius(6);
                    if ui
                        .add_enabled(!nom_vide, rattacher)
                        .on_disabled_hover_text("Donnez un nom au nouveau drone.")
                        .clicked()
                    {
                        actions.push(AttachAction::Attach(p.path.clone(), p.choice.clone()));
                    }
                    // un log déjà rangé n'a pas de « Ne pas ranger » : il l'est déjà, on le laisse où il est
                    let (second, hover) = match known {
                        Some(k) => (
                            format!("Laisser sous « {} »", k.name),
                            "Le log reste rangé sous ce drone, comme avant.",
                        ),
                        None => (
                            "Ne pas ranger".to_string(),
                            "Le log reste ouvert pour cette séance ; la bibliothèque ne garde rien.",
                        ),
                    };
                    let passer = egui::Button::new(RichText::new(safe_text(&second)).size(13.0).color(th.ui.text))
                        .fill(th.ui.base)
                        .stroke(egui::Stroke::new(1.0, th.ui.mid))
                        .corner_radius(6);
                    if ui.add(passer).on_hover_text(hover).clicked() {
                        actions.push(left_alone(lib, p));
                    }
                });
                if let Some(e) = &p.error {
                    ui.label(RichText::new(safe_text(e)).color(th.accent("red")));
                }
            }
            ui.separator();
            ui.label(
                RichText::new(
                    "Un log non rangé se rattache plus tard depuis l'écran d'extraction (« Rattacher à un drone… »).",
                )
                .size(12.0)
                .color(th.ui.muted),
            );
        });
        if m.should_close() && actions.is_empty() {
            actions.extend(self.pending.iter().map(|p| left_alone(lib, p)));
        }
        actions
    }
}

/// Le second bouton, ou la fenêtre fermée : un log déjà rangé reste sous son drone (ses fragments reviennent dans
/// la liste d'analyse) ; un autre n'est pas rangé.
pub fn left_alone(lib: &Library, p: &Pending) -> AttachAction {
    match &p.known {
        Some(id) if lib.drone(id).is_some() => AttachAction::Attach(p.path.clone(), DroneChoice::Existing(id.clone())),
        _ => AttachAction::Skip(p.path.clone()),
    }
}
