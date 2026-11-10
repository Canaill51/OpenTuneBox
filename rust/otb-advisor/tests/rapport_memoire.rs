//! Le rapport de l'assistant ne garde pas les fenêtres individuelles de la réponse indicielle (L-094).
//!
//! ⚠️ Chaque rapport est **rangé dans le cache des outils de l'interface**, une entrée par profil demandé et par
//! période, sans éviction. Les fenêtres individuelles (`StepResult::responses`, `n` × `len(t_ms)` flottants) pesaient
//! 69 % du rapport — jusqu'à 72 Mo sur une session de 174 s à 2 kHz (mesuré sur tout le corpus,
//! `docs/validation.md`, « Mémoire du cache des outils ») — alors que **personne ne les lit** : les règles se servent
//! de `n`, `metrics` et `loop_`, la page Assistant trace `mean`, `std` et `t_ms`.
//!
//! Contre-preuve : remettre `r.responses` dans `rules.rs:step_section` fait rougir ce test.

use otb_advisor::rules;
use otb_log::flightlog::{self, FlightLog};

fn journal() -> FlightLog {
    let p = format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"));
    flightlog::load(std::path::Path::new(&p)).unwrap().remove(0)
}

#[test]
fn le_rapport_ne_garde_pas_les_fenetres_individuelles() {
    let log = journal();
    let rep = rules::analyze(&log, None, None).expect("rapport");
    assert_eq!(rep.steps.len(), 3, "un axe manque dans le rapport");

    // Sans fenêtre mesurée, le test ne prouverait rien : il faut qu'il y en ait à perdre.
    let mesurees: usize = rep.steps.iter().map(|st| st.result.n).sum();
    assert!(
        mesurees > 0,
        "aucune fenêtre mesurée sur ce log : la contre-preuve serait vide"
    );

    for st in &rep.steps {
        assert!(
            st.result.responses.is_empty(),
            "{} : le rapport garde {} flottants de fenêtres individuelles que personne ne lit",
            st.axis,
            st.result.responses.len()
        );
        if st.result.n > 0 {
            // ce que la page Assistant trace, lui, doit rester
            assert!(!st.result.t_ms.is_empty(), "{} : plus d'axe des temps", st.axis);
            assert_eq!(
                st.result.mean.len(),
                st.result.t_ms.len(),
                "{} : la moyenne a été perdue avec les fenêtres",
                st.axis
            );
            assert_eq!(
                st.result.std.len(),
                st.result.t_ms.len(),
                "{} : l'écart-type a été perdu avec les fenêtres",
                st.axis
            );
        }
    }
}
