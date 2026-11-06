//! Les contournements d'egui_plot de `plots.rs` : fonctions pures, puis glisser d'un repère à la souris
//! (kittest, sans écran), le plus fragile des cinq (E1).

use egui::{Color32, Pos2, vec2};
use egui_kittest::Harness;
use egui_plot::{GridInput, Line, Plot, PlotPoints, VLine};
use otb_gui::plots::{
    VDrag, decimate_minmax, error_bars, format_hz, heatmap_pixels, log_grid, log_points, right_axis_scale,
};
use otb_gui::theme::Palette;

fn marques(lo: f64, hi: f64) -> Vec<f64> {
    log_grid(GridInput {
        bounds: (lo, hi),
        base_step_size: 0.01,
    })
    .into_iter()
    .map(|m| m.value)
    .collect()
}

#[test]
fn la_grille_logarithmique_donne_1_2_5_par_decade() {
    // 1 Hz à 100 Hz : 1, 2, 5, 10, 20, 50, 100
    let m = marques(0.0, 2.0);
    let attendu: Vec<f64> = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]
        .iter()
        .map(|f: &f64| f.log10())
        .collect();
    assert_eq!(m.len(), attendu.len(), "{m:?}");
    for a in &attendu {
        assert!(
            m.iter().any(|v| (v - a).abs() < 1e-12),
            "repère manquant {a} dans {m:?}"
        );
    }
    // jamais hors de la fenêtre visible
    assert!(marques(1.2, 1.9).iter().all(|v| (1.2..=1.9).contains(v)));
    // 10 Hz à 1 kHz : 10, 20, 50, 100, 200, 500, 1000
    assert_eq!(marques(0.9, 3.1).len(), 7);
}

#[test]
fn la_grille_supporte_les_bornes_folles() {
    assert!(marques(f64::NAN, 2.0).is_empty());
    assert!(marques(0.0, f64::INFINITY).is_empty());
    assert!(
        marques(-3e9, 3e9).len() < 200,
        "bornes énormes : nombre de repères limité"
    );
    assert!(marques(-1e7, 1e7).len() < 200);
}

#[test]
fn les_libelles_de_frequence() {
    assert_eq!(format_hz(500.0), "500");
    assert_eq!(format_hz(1000.0), "1 k");
    assert_eq!(format_hz(1500.0), "1,5 k");
    assert_eq!(format_hz(2000.0), "2 k");
    // l'axe donne log10(f) et le libellé refait 10^v : pas de « 4.999999999999999 k »
    for f in [
        1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0,
    ] {
        let lu = format_hz(10f64.powf(f64::log10(f)));
        assert_eq!(lu, format_hz(f), "{f} Hz lu {lu}");
    }
    assert_eq!(format_hz(10f64.powf(200f64.log10())), "200");
}

#[test]
fn les_points_logarithmiques_ecartent_moins_de_1_hz() {
    let p = log_points(&[0.0, 0.5, 1.0, 10.0, 1000.0], &[9.0, 9.0, 1.0, 2.0, 3.0]);
    assert_eq!(p, vec![[0.0, 1.0], [1.0, 2.0], [3.0, 3.0]]);
}

#[test]
fn les_barres_d_erreur_sont_verticales_et_espacees() {
    let x = [1.0, 2.0, 3.0, 4.0, 5.0];
    let (m, e) = ([10.0; 5], [1.0, 2.0, 3.0, 4.0, 5.0]);
    let b = error_bars(&x, &m, &e, 2);
    assert_eq!(
        b,
        vec![
            vec![[1.0, 9.0], [1.0, 11.0]],
            vec![[3.0, 7.0], [3.0, 13.0]],
            vec![[5.0, 5.0], [5.0, 15.0]]
        ]
    );
    assert_eq!(error_bars(&x, &m, &e, 0).len(), 5, "un pas nul vaut 1");
    assert_eq!(
        error_bars(&x, &m, &[1.0], 1).len(),
        1,
        "s'arrête à la plus courte des entrées"
    );
}

#[test]
fn la_carte_est_retournee_et_garde_les_trous() {
    // 2 fréquences × 3 instants ; ligne 0 = fréquence basse, qui doit finir EN BAS de l'image
    let v = [0.0, 10.0, f32::NAN, 5.0, 5.0, 5.0];
    let px = heatmap_pixels(&v, 2, 3, 0.0, 10.0, Palette::Turbo, Color32::TRANSPARENT);
    assert_eq!(px.len(), 6);
    // image : ligne 0 (haut) = fréquence haute = valeurs [5, 5, 5] ; ligne 1 (bas) = [0, 10, NaN]
    assert_eq!(&px[..3], &[Palette::Turbo.color(0.5); 3]);
    assert_eq!(px[3], Palette::Turbo.color(0.0));
    assert_eq!(px[4], Palette::Turbo.color(1.0));
    assert_eq!(px[5], Color32::TRANSPARENT);
    // couleur des cases vides au choix
    let gris = Color32::from_rgb(9, 9, 9);
    assert_eq!(
        heatmap_pixels(&[f32::NAN], 1, 1, 0.0, 1.0, Palette::Hot, gris),
        vec![gris]
    );
    // hors échelle : bornes de la palette ; échelle nulle : pas de division par zéro
    let px = heatmap_pixels(&[-5.0, 99.0], 1, 2, 0.0, 10.0, Palette::Turbo, Color32::TRANSPARENT);
    assert_eq!(px, vec![Palette::Turbo.color(0.0), Palette::Turbo.color(1.0)]);
    assert!(
        heatmap_pixels(&[1.0], 1, 1, 3.0, 3.0, Palette::Turbo, Color32::TRANSPARENT)
            .iter()
            .all(|c| *c != Color32::TRANSPARENT)
    );
}

#[test]
#[should_panic(expected = "carte de 5 valeurs")]
fn une_carte_de_mauvaise_taille_est_refusee() {
    heatmap_pixels(&[0.0; 5], 2, 3, 0.0, 1.0, Palette::Turbo, Color32::TRANSPARENT);
}

#[test]
fn la_decimation_garde_les_pics_dans_l_ordre_du_temps() {
    let t: Vec<f64> = (0..10).map(|i| i as f64).collect();
    let mut y = vec![0.0f32; 10];
    y[3] = 9.0; // pic haut dans le 1er paquet
    y[2] = -4.0; // creux avant lui
    y[7] = -8.0; // creux dans le 2e paquet
    let d = decimate_minmax(&t, &y, 5);
    // paquet 1 : creux en 2, pic en 3 ; paquet 2 : creux en 7, maximum à égalité en 5 (le premier gagne)
    assert_eq!(d, vec![[2.0, -4.0], [3.0, 9.0], [5.0, 0.0], [7.0, -8.0]]);
}

#[test]
fn la_decimation_d_un_signal_plat_ou_vide() {
    let t: Vec<f64> = (0..8).map(f64::from).collect();
    assert_eq!(decimate_minmax(&t, &[1.0; 8], 4), vec![[0.0, 1.0], [4.0, 1.0]]);
    assert!(decimate_minmax(&[], &[], 4).is_empty());
    // un paquet de 1 est relevé à 2 (sinon on ne décimerait rien)
    assert_eq!(decimate_minmax(&t, &[0.0; 8], 1).len(), 4);
}

#[test]
fn le_second_axe_met_les_courbes_a_l_echelle() {
    assert_eq!(right_axis_scale(500.0, 100.0), 5.0);
    assert_eq!(right_axis_scale(500.0, 0.0), 1.0);
}

// ------------------------------------------------------------------ glisser un repère à la souris

struct Essai {
    coupure: f64,
    bord: f64,
    drag: VDrag,
    transform_x: Option<Box<dyn Fn(f64) -> f32>>,
    press_x: Option<f32>,
}

fn dessine(ui: &mut egui::Ui, e: &mut Essai) {
    let press = ui.input(|i| i.pointer.press_origin()).map(|p| p.x);
    let busy = e.drag.busy();
    let (mut xs, mut drag) = ([e.coupure, e.bord], e.drag);
    let mut tr = None;
    Plot::new("p")
        .allow_drag(!busy)
        .include_x(0.0)
        .include_x(1000.0)
        .include_y(0.0)
        .include_y(1.0)
        .show(ui, |pu| {
            pu.line(Line::new("c", PlotPoints::from(vec![[0.0, 0.5], [1000.0, 0.5]])));
            pu.vline(VLine::new("coupure", xs[0]));
            pu.vline(VLine::new("bord", xs[1]));
            drag.update(pu, press, &mut xs, &[(0.0, 1000.0), (0.0, 1000.0)]);
            tr = Some(*pu.transform());
        });
    (e.coupure, e.bord, e.drag) = (xs[0], xs[1], drag);
    e.press_x = press;
    if let Some(tr) = tr {
        e.transform_x = Some(Box::new(move |x| tr.position_from_point_x(x)));
    }
}

#[test]
fn glisser_un_repere_le_deplace_sans_deplacer_le_graphique_ni_l_autre_repere() {
    let essai = Essai {
        coupure: 150.0,
        bord: 600.0,
        drag: VDrag::default(),
        transform_x: None,
        press_x: None,
    };
    let mut h = Harness::builder()
        .with_size(vec2(900.0, 500.0))
        .build_ui_state(dessine, essai);
    h.run();
    h.run();
    let px = h.state().transform_x.as_ref().unwrap()(150.0);
    let y = 250.0;
    h.hover_at(Pos2::new(px, y));
    h.run();
    h.drag_at(Pos2::new(px, y));
    h.run();
    let cible = h.state().transform_x.as_ref().unwrap()(400.0);
    h.hover_at(Pos2::new(cible, y));
    h.run();
    h.drop_at(Pos2::new(cible, y));
    h.run();
    let s = h.state();
    assert!(
        (380.0..420.0).contains(&s.coupure),
        "coupure après glissement : {}",
        s.coupure
    );
    assert_eq!(s.bord, 600.0, "l'autre repère ne doit pas bouger");
    assert!(s.drag.active.is_none(), "le repère est relâché");
    // le graphique n'a pas été déplacé : la position de 0 est restée la même
    let zero = h.state().transform_x.as_ref().unwrap()(0.0);
    assert!(zero < 200.0, "le graphique a glissé : x(0) = {zero}");
}

#[test]
fn glisser_loin_d_un_repere_ne_le_deplace_pas() {
    let essai = Essai {
        coupure: 150.0,
        bord: 600.0,
        drag: VDrag::default(),
        transform_x: None,
        press_x: None,
    };
    let mut h = Harness::builder()
        .with_size(vec2(900.0, 500.0))
        .build_ui_state(dessine, essai);
    h.run();
    h.run();
    let tr = h.state().transform_x.as_ref().unwrap();
    let (depart, arrivee) = (tr(300.0), tr(450.0));
    h.hover_at(Pos2::new(depart, 250.0));
    h.run();
    h.drag_at(Pos2::new(depart, 250.0));
    h.run();
    h.hover_at(Pos2::new(arrivee, 250.0));
    h.run();
    h.drop_at(Pos2::new(arrivee, 250.0));
    h.run();
    assert_eq!((h.state().coupure, h.state().bord), (150.0, 600.0));
}

// `les_points_visibles_sont_filtres_puis_decimes` a déménagé avec sa fonction dans `rust/otb-dsp/tests/minmax.rs`
// (D-048) : ses cas (marge d'un point, pic conservé, fenêtre hors du log, plage inversée, NaN, courbe vide) y sont
// repris et élargis, la sortie étant comparée point pour point à celle de l'ancienne `visible_points`.

#[test]
fn les_graduations_de_la_barre_de_couleurs_sont_rondes_et_dans_l_intervalle() {
    use otb_gui::plots::colorbar_ticks;
    assert_eq!(colorbar_ticks(0.0, 10.0, 5), vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
    assert_eq!(colorbar_ticks(-30.0, 10.0, 4), vec![-30.0, -20.0, -10.0, 0.0, 10.0]);
    assert_eq!(
        colorbar_ticks(0.0, 0.5, 5),
        vec![0.0, 0.1, 0.2, 0.30000000000000004, 0.4, 0.5]
    );
    // intervalle vide, inversé, non fini ou sans graduation demandée : rien (pas de boucle infinie)
    assert!(colorbar_ticks(1.0, 1.0, 5).is_empty());
    assert!(colorbar_ticks(2.0, 1.0, 5).is_empty());
    assert!(colorbar_ticks(0.0, f64::NAN, 5).is_empty());
    assert!(colorbar_ticks(0.0, f64::INFINITY, 5).is_empty());
    assert!(colorbar_ticks(0.0, 1.0, 0).is_empty());
}

#[test]
fn la_grille_lineaire_donne_des_repetes_ronds_et_etiquetables() {
    use egui_plot::GridInput;
    use otb_gui::plots::nice_grid;
    // 0..100 sur ~250 px : un repère toutes les 8 px = 3,2 unités → pas fin de 5, pas d'étiquette de 50 (≥ 64 px)
    let g = nice_grid(
        GridInput {
            bounds: (0.0, 100.0),
            base_step_size: 3.2,
        },
        8.0,
    );
    let valeurs: Vec<f64> = g.iter().map(|m| m.value).collect();
    assert_eq!(valeurs, (0..=20).map(|k| f64::from(k) * 5.0).collect::<Vec<_>>());
    for m in &g {
        let attendu = if m.value % 50.0 == 0.0 { 50.0 } else { 5.0 };
        assert_eq!(m.step_size, attendu, "repère {}", m.value);
    }
    // bornes négatives : -50..20 avec un repère toutes les 0,5 unité de base
    let g = nice_grid(
        GridInput {
            bounds: (-50.0, 20.0),
            base_step_size: 0.5,
        },
        3.0,
    );
    assert!(g.iter().any(|m| m.value == -50.0) && g.iter().any(|m| m.value == 20.0));
    assert!(g.iter().all(|m| m.value >= -50.0 && m.value <= 20.0));
    // dézoom extrême, NaN, pas nul : rien, et jamais des milliers de repères
    for (b, s) in [
        ((0.0, f64::NAN), 1.0),
        ((0.0, 1.0), 0.0),
        ((0.0, 1.0), f64::NAN),
        ((0.0, 1e12), 1e-3),
    ] {
        assert!(
            nice_grid(
                GridInput {
                    bounds: b,
                    base_step_size: s
                },
                8.0
            )
            .is_empty()
        );
    }
}
