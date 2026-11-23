//! Le seul clic permis dans les tests et les exemples de l'interface (L-114) : faire défiler jusqu'au widget, laisser
//! l'écran se redessiner, vérifier qu'il est dans la fenêtre, puis cliquer.
//!
//! ⚠️ `Node::click` d'egui_kittest envoie un clic de **pointeur** au centre du nœud : un widget sous le bas d'une zone
//! qui défile est dans l'arbre d'accessibilité (la recherche le trouve), mais le clic ne touche rien et l'échec arrive
//! plus loin, sur l'assertion suivante (L-114, deux récidives). `rust/otb-gui/tests/contracts.rs` refuse donc tout
//! `.click()` nu hors de ce fichier.
#![allow(dead_code)]

use egui::accesskit::{NodeId, Role, TreeId};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};

/// Images dessinées entre le défilement et le clic : la première fait défiler, les suivantes posent la mise en page.
pub const FRAMES_AFTER_SCROLL: usize = 3;

/// Ce qui désigne un nœud d'une image à l'autre (`accesskit_node().locate()`) : il ne dépend pas du texte cherché.
pub type NodeKey = (NodeId, TreeId);

pub trait Cliquer {
    /// Clique le nœud que `trouve` désigne par sa clé (`|h| h.get_by_label(x).accesskit_node().locate()`), après
    /// l'avoir fait venir dans la zone visible. La clé, et non le nœud : un nœud de kittest emprunte le texte
    /// cherché, et il faut le retrouver après le défilement.
    fn cliquer<F>(&mut self, trouve: F)
    where
        F: FnOnce(&Self) -> NodeKey;

    /// Clique le seul nœud de ce nom.
    fn cliquer_label(&mut self, label: &str);

    /// Clique le seul nœud de ce rôle et de ce nom.
    fn cliquer_role(&mut self, role: Role, label: &str);

    /// Clique le seul nœud de cette valeur (un menu déroulant par le choix qu'il montre).
    fn cliquer_value(&mut self, value: &str);
}

impl<S> Cliquer for Harness<'_, S> {
    fn cliquer<F>(&mut self, trouve: F)
    where
        F: FnOnce(&Self) -> NodeKey,
    {
        let id = trouve(self);
        let par_id = || By::new().predicate(move |n| n.locate() == id);
        // sans effet sur un widget déjà entièrement visible : egui ne fait défiler que ce qui dépasse
        self.get(par_id()).scroll_to_me();
        self.run_steps(FRAMES_AFTER_SCROLL);
        let node = self.get(par_id());
        let centre = node.rect().center();
        let fenetre = self.ctx.content_rect();
        assert!(
            fenetre.contains(centre),
            "clic hors de la fenêtre même après défilement : centre {centre:?}, fenêtre {fenetre:?} (L-114)"
        );
        node.click();
    }

    fn cliquer_label(&mut self, label: &str) {
        self.cliquer(|h| h.get_by_label(label).accesskit_node().locate());
    }

    fn cliquer_role(&mut self, role: Role, label: &str) {
        self.cliquer(|h| h.get_by_role_and_label(role, label).accesskit_node().locate());
    }

    fn cliquer_value(&mut self, value: &str) {
        self.cliquer(|h| h.get_by_value(value).accesskit_node().locate());
    }
}
