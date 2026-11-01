//! Banc jetable (6-0) : `egui_plot::Heatmap` contre `PlotImage` sur une vraie carte fréquence × temps.
//!
//! Usage : `cargo run --release -p otb-gui --example banc_carte -- LOG.bbl`
//! Mesure, par cadre, la partie « construction des formes » (`run_ui`) et la « tessellation » (formes → sommets),
//! qui sont le coût côté processeur ; le nombre de sommets donne la charge envoyée à la carte graphique.
//! Pas de mesure du GPU (pas de fenêtre) : c'est dit dans docs/migration-rust.md.

use std::time::Instant;

use egui::{Pos2, Rect, vec2};
use egui_plot::{Heatmap, Plot, PlotImage, PlotPoint};
use otb_analysis::spectral::freq_time;
use otb_gui::plots::heatmap_texture;
use otb_gui::theme::turbo;

const ESSAIS: usize = 25;

fn mediane(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Un cadre complet : retourne (ms de construction, ms de tessellation, sommets).
fn cadre(ctx: &egui::Context, contenu: &mut dyn FnMut(&mut egui::Ui)) -> (f64, f64, usize) {
    let raw = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 800.0))),
        ..Default::default()
    };
    let t0 = Instant::now();
    let out = ctx.run_ui(raw, |ui| {
        egui::CentralPanel::default().show(ui, |ui| contenu(ui));
    });
    let t1 = Instant::now();
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
    let t2 = Instant::now();
    let sommets: usize = prims
        .iter()
        .map(|p| match &p.primitive {
            egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
            _ => 0,
        })
        .sum();
    ((t1 - t0).as_secs_f64() * 1e3, (t2 - t1).as_secs_f64() * 1e3, sommets)
}

fn mesure(nom: &str, cases: usize, mut contenu: impl FnMut(&mut egui::Ui)) {
    let ctx = egui::Context::default();
    for _ in 0..3 {
        cadre(&ctx, &mut contenu); // chauffe : polices, premier calcul des bornes du graphique
    }
    let (mut b, mut t, mut s) = (Vec::new(), Vec::new(), 0);
    for _ in 0..ESSAIS {
        let (x, y, z) = cadre(&ctx, &mut contenu);
        b.push(x);
        t.push(y);
        s = z;
    }
    let (b, t) = (mediane(b), mediane(t));
    println!(
        "{nom:<34} {cases:>8} cases | construction {b:>8.2} ms | tessellation {t:>8.2} ms | total {:>8.2} ms | {s:>9} sommets",
        b + t
    );
}

fn main() {
    let chemin = std::env::args().nth(1).expect("chemin du log .bbl");
    let log = otb_log::flightlog::load(std::path::Path::new(&chemin))
        .unwrap()
        .remove(0);
    let x: Vec<f64> = log.gyro.row(0).iter().map(|&v| v as f64).collect();
    let sp = freq_time(&x, &log.t, log.fs, 300.0, 2, 0, None).unwrap();
    println!(
        "{chemin} : fs {:.0} Hz, {} points, carte {} fréquences × {} instants = {} cases",
        log.fs,
        x.len(),
        sp.nf,
        sp.nt,
        sp.nf * sp.nt
    );
    let (lo, hi) = (-40.0_f32, 20.0_f32); // échelle quelconque : la mesure ne dépend pas des valeurs
    let (t0, t1, f0, f1) = (sp.times[0], *sp.times.last().unwrap(), 0.0, *sp.freqs.last().unwrap());

    // ré-échantillonnages « une case sur k » pour retrouver 10⁴ et 10⁵ cases
    for (cible, nom) in [
        (10_000usize, "≈ 10⁴"),
        (100_000, "≈ 10⁵"),
        (usize::MAX, "carte entière"),
    ] {
        let k = ((sp.nf * sp.nt) as f64 / cible.min(sp.nf * sp.nt) as f64).sqrt().ceil() as usize;
        let (nf, nt) = (sp.nf.div_ceil(k), sp.nt.div_ceil(k));
        let mut z = Vec::with_capacity(nf * nt);
        for r in 0..nf {
            for c in 0..nt {
                z.push(sp.z[(r * k) * sp.nt + c * k] as f32);
            }
        }
        println!("\n--- {nom} : {nf} × {nt} = {} cases", nf * nt);
        let valeurs: Vec<f64> = z.iter().map(|&v| v as f64).collect();
        mesure("Heatmap (sans libellés, turbo)", nf * nt, |ui| {
            let h = Heatmap::new(valeurs.clone(), nt)
                .show_labels(false)
                .custom_mapping(Box::new(move |v| turbo(((v as f32 - lo) / (hi - lo)).clamp(0.0, 1.0))))
                .at(PlotPoint::new(t0, f0))
                .size((t1 - t0) as f32, (f1 - f0) as f32);
            Plot::new("h").show(ui, |pu| pu.add(h));
        });
        let ctx = egui::Context::default();
        let debut = Instant::now();
        let tex = heatmap_texture(&ctx, "carte", &z, nf, nt, lo, hi);
        let tex_ms = debut.elapsed().as_secs_f64() * 1e3;
        let tex_id = tex.id();
        mesure("PlotImage (texture, autre contexte)", nf * nt, |ui| {
            let img = PlotImage::new(
                "DSP",
                tex_id,
                PlotPoint::new((t0 + t1) / 2.0, (f0 + f1) / 2.0),
                [(t1 - t0) as f32, (f1 - f0) as f32],
            );
            Plot::new("i").show(ui, |pu| pu.image(img));
        });
        println!("  (construction de la texture, une fois à chaque changement de données : {tex_ms:.2} ms)");
    }
}
