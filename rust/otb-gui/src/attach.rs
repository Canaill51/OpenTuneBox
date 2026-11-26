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
                    "Ces logs ne disent pas à quelle fiche ils appartiennent. Choisissez un drone pour chacun : ses \
                     fragments seront enregistrés dans la bibliothèque.",
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
                let why = if p.craft.trim().is_empty() {
                    "Pas de nom de machine dans l'en-tête.".to_string()
                } else {
                    format!("Nom de machine « {} » : aucune fiche ne porte ce nom.", p.craft.trim())
                };
                ui.label(RichText::new(safe_text(&why)).size(12.0).color(th.ui.muted));
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
                    let passer = egui::Button::new(RichText::new("Ne pas ranger").size(13.0).color(th.ui.text))
                        .fill(th.ui.base)
                        .stroke(egui::Stroke::new(1.0, th.ui.mid))
                        .corner_radius(6);
                    if ui
                        .add(passer)
                        .on_hover_text("Le log reste ouvert pour cette séance ; la bibliothèque ne garde rien.")
                        .clicked()
                    {
                        actions.push(AttachAction::Skip(p.path.clone()));
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
            actions.extend(self.pending.iter().map(|p| AttachAction::Skip(p.path.clone())));
        }
        actions
    }
}
