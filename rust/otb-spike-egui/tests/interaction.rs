//! Interaction sans écran : glisser un repère, et coût du tracé de 1,2 M points (mesuré, pas estimé).

use egui_kittest::Harness;
use otb_spike_egui::{Spike, Tab};
use std::time::Instant;

fn harness(tab: Tab) -> Harness<'static, Spike> {
    let dir = std::env::var("OTB_SPIKE_DIR").expect("OTB_SPIKE_DIR");
    let mut h = Harness::builder()
        .with_size(egui::vec2(1100.0, 700.0))
        .build_ui_state(|ui, s: &mut Spike| s.ui(ui), Spike::load(dir.into()));
    h.state_mut().tab = tab;
    h.run();
    h
}

#[test]
fn la_ligne_de_coupure_se_deplace_a_la_souris_sans_deplacer_le_graphique() {
    let mut h = harness(Tab::Reperes);
    let avant = h.state().cutoff_hz;
    assert_eq!(avant, 150.0);
    // x = 232 px ↔ 150 Hz (capture reperes-sombre.png : 0 Hz à 89 px, 1000 Hz à 1045 px)
    h.hover_at(egui::pos2(232.0, 350.0));
    h.run();
    h.drag_at(egui::pos2(232.0, 350.0));
    h.run();
    h.hover_at(egui::pos2(400.0, 350.0));
    h.run();
    h.drop_at(egui::pos2(400.0, 350.0));
    h.run();
    let apres = h.state().cutoff_hz;
    assert!((300.0..350.0).contains(&apres), "coupure après glissement : {apres}");
    assert_eq!(h.state().region, (80.0, 200.0), "la région ne doit pas bouger");
}

#[test]
fn cout_de_la_serie_de_1_2_million_de_points() {
    let mut h = harness(Tab::Serie);
    for decimate in [false, true] {
        h.state_mut().decimate = decimate;
        let mut t = Vec::new();
        for _ in 0..7 {
            let t0 = Instant::now();
            h.step();
            t.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        t.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "décimation={decimate} : cadre complet (construction + tessellation) médiane {:.1} ms, min {:.1}, max {:.1}",
            t[3], t[0], t[6]
        );
    }
}
