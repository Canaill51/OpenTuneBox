//! État partagé : logs, couleurs, libellés, période, révision (port de `gui/state.py`).

mod support;

use otb_gui::state::{AppState, Famille, MAX_LOGS};
use otb_gui::theme::file_color;
use support::synthetic_log;

#[test]
fn un_log_ajoute_devient_le_courant_avec_la_premiere_couleur_libre() {
    let mut s = AppState::new();
    assert!(s.current_log().is_none());
    assert!(s.add(synthetic_log("a", 1000, 1000.0)));
    assert!(s.add(synthetic_log("b", 1000, 1000.0)));
    assert_eq!(s.current, Some(1));
    assert_eq!((s.logs[0].color, s.logs[1].color), (file_color(0), file_color(1)));
    // une couleur libérée est reprise avant les suivantes
    s.remove(0);
    s.add(synthetic_log("c", 1000, 1000.0));
    assert_eq!(s.logs[1].color, file_color(0));
}

#[test]
fn deux_logs_du_meme_nom_ont_des_libelles_distincts() {
    let mut s = AppState::new();
    for _ in 0..3 {
        s.add(synthetic_log("vol", 100, 1000.0));
    }
    let labels: Vec<_> = s.logs.iter().map(|l| l.label.as_str()).collect();
    assert_eq!(labels, ["vol", "vol (2)", "vol (3)"]);
}

#[test]
fn pas_plus_de_dix_logs() {
    let mut s = AppState::new();
    for i in 0..MAX_LOGS {
        assert!(s.add(synthetic_log(&format!("l{i}"), 100, 1000.0)));
    }
    let rev = s.revision();
    assert!(!s.add(synthetic_log("de_trop", 100, 1000.0)));
    assert_eq!((s.logs.len(), s.revision()), (MAX_LOGS, rev));
}

#[test]
fn retirer_garde_un_courant_valide() {
    let mut s = AppState::new();
    for n in ["a", "b", "c"] {
        s.add(synthetic_log(n, 100, 1000.0));
    }
    s.remove(2);
    assert_eq!(s.current, Some(1));
    s.remove(7); // hors limites : rien
    assert_eq!(s.logs.len(), 2);
    s.remove(0);
    s.remove(0);
    assert_eq!((s.logs.len(), s.current), (0, None));
    s.add(synthetic_log("d", 100, 1000.0));
    s.clear();
    assert_eq!((s.logs.len(), s.current), (0, None));
}

#[test]
fn la_periode_est_bornee_ordonnee_et_vide_les_caches() {
    let mut s = AppState::new();
    s.add(synthetic_log("a", 10_000, 1000.0)); // 0 à 9,999 s
    let rev = s.revision();
    s.logs[0].extras.put(Famille::Step, "spectre", std::sync::Arc::new(1u8));
    s.set_period(0, 8.0, 2.0);
    assert_eq!(s.logs[0].period, (2.0, 8.0));
    assert!(s.logs[0].extras.is_empty());
    assert_eq!(s.revision(), rev + 1);
    s.set_period(0, -5.0, 50.0);
    assert_eq!(s.logs[0].period, (0.0, 9.999));
    // même période : ni révision ni cache vidé
    let rev = s.revision();
    s.logs[0].extras.put(Famille::Step, "spectre", std::sync::Arc::new(1u8));
    s.set_period(0, 0.0, 9.999);
    assert_eq!(s.revision(), rev);
    assert!(!s.logs[0].extras.is_empty());
    s.set_period(3, 0.0, 1.0); // log inexistant : rien
    s.reset_period(0);
    assert_eq!(s.logs[0].period, (0.0, 9.999));
}

#[test]
fn le_curseur_et_le_log_courant_ne_changent_pas_la_revision() {
    let mut s = AppState::new();
    s.add(synthetic_log("a", 100, 1000.0));
    s.add(synthetic_log("b", 100, 1000.0));
    let rev = s.revision();
    s.set_cursor(0.05);
    s.set_current(Some(0));
    assert_eq!((s.cursor, s.current, s.revision()), (0.05, Some(0), rev));
    s.set_current(Some(9)); // hors limites : aucun
    assert_eq!(s.current, None);
}

#[test]
fn le_cache_rend_la_valeur_rangee_seulement_pour_le_bon_type_et_la_bonne_cle() {
    use otb_gui::state::Cache;
    let c = Cache::default();
    assert!(c.is_empty());
    c.put(Famille::Step, "a", std::sync::Arc::new(7u32));
    assert_eq!(c.get::<u32>("a").as_deref(), Some(&7));
    assert!(c.get::<u32>("b").is_none(), "autre clé");
    assert!(c.get::<String>("a").is_none(), "autre type");
    // un clone partage le même contenu (c'est ce qui passe au thread de calcul)
    let d = c.clone();
    d.put(Famille::Step, "b", std::sync::Arc::new(8u32));
    assert_eq!(c.get::<u32>("b").as_deref(), Some(&8));
    d.clear();
    assert!(c.is_empty());
}

/// Plafond du cache par outil (W-035 point 9, L-094) : sans lui, l'outil « Réponse indicielle » peut retenir
/// 72 résultats de 71 Mo pour un seul log, soit 5,1 Go (`docs/validation.md`).
mod plafond {
    use otb_gui::state::{Cache, Famille};
    use std::sync::Arc;

    fn remplir(c: &Cache, famille: Famille, cles: &[&str]) {
        for (i, k) in cles.iter().enumerate() {
            c.put(famille, *k, Arc::new(i as u32));
        }
    }

    #[test]
    fn au_dela_du_plafond_la_moins_recemment_utilisee_est_jetee() {
        let c = Cache::default();
        let n = Famille::Step.plafond();
        let cles: Vec<String> = (0..n + 2).map(|i| format!("step:{i}")).collect();
        remplir(&c, Famille::Step, &cles.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(c.len_of(Famille::Step), n, "le plafond n'est pas tenu");
        assert!(c.get::<u32>(&cles[0]).is_none(), "la plus ancienne devait partir");
        assert!(c.get::<u32>(&cles[1]).is_none(), "la deuxième plus ancienne aussi");
        for k in &cles[2..] {
            assert!(c.get::<u32>(k).is_some(), "{k} : une entrée récente a été jetée");
        }
    }

    #[test]
    fn un_acces_rajeunit_l_entree_donc_ce_qu_on_affiche_n_est_jamais_jete() {
        let c = Cache::default();
        let n = Famille::Step.plafond();
        assert!(n >= 2, "ce test suppose un plafond d'au moins 2");
        let cles: Vec<String> = (0..n).map(|i| format!("step:{i}")).collect();
        remplir(&c, Famille::Step, &cles.iter().map(String::as_str).collect::<Vec<_>>());
        // on se sert de la plus ancienne : c'est elle qu'on affiche, elle ne doit plus être la prochaine jetée
        assert!(c.get::<u32>(&cles[0]).is_some());
        c.put(Famille::Step, "step:neuf", Arc::new(99u32));
        assert!(
            c.get::<u32>(&cles[0]).is_some(),
            "l'entrée dont on vient de se servir a été jetée : elle serait recalculée à chaque cadre"
        );
        assert!(c.get::<u32>(&cles[1]).is_none(), "c'est la suivante qui devait partir");
    }

    #[test]
    fn un_outil_ne_chasse_pas_les_entrees_d_un_autre() {
        let c = Cache::default();
        c.put(Famille::Step, "step:a", Arc::new(1u32));
        c.put(Famille::Assistant, "advisor:auto", Arc::new(2u32));
        c.put(Famille::Guide, "guide-quick", Arc::new(3u32));
        // bien au-delà du plafond du simulateur de filtres
        let cles: Vec<String> = (0..Famille::Filtres.plafond() + 5)
            .map(|i| format!("filtres:{i}"))
            .collect();
        remplir(
            &c,
            Famille::Filtres,
            &cles.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        assert_eq!(c.len_of(Famille::Filtres), Famille::Filtres.plafond());
        assert!(c.get::<u32>("step:a").is_some(), "la réponse indicielle a été chassée");
        assert!(c.get::<u32>("advisor:auto").is_some(), "l'assistant a été chassé");
        assert!(c.get::<u32>("guide-quick").is_some(), "le guide a été chassé");
    }

    #[test]
    fn le_guide_garde_ses_deux_verifications_d_un_meme_cadre() {
        // le Guide demande la rapide ET la complète au même cadre : un plafond de 1 les ferait se chasser
        // l'une l'autre, donc recalculer sans fin
        assert!(Famille::Guide.plafond() >= 2, "plafond du Guide trop bas");
        let c = Cache::default();
        c.put(Famille::Guide, "guide-quick|x", Arc::new(1u32));
        c.put(Famille::Guide, "guide-full|x", Arc::new(2u32));
        assert!(c.get::<u32>("guide-quick|x").is_some());
        assert!(c.get::<u32>("guide-full|x").is_some());
    }
}
