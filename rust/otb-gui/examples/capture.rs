//! Capture d'écran sans fenêtre (rendu wgpu de egui_kittest) pour regarder l'interface : jamais dans la CI.
//!
//! `cargo run --release -p otb-gui --example capture -- LOG.bbl DOSSIER_DE_SORTIE [AUTRE_LOG…]`
//! Les logs supplémentaires sont ouverts aussi et cochés dans chaque outil (séries de vols : Réponse indicielle).
//! Écrit `courbes-clair.png` et `courbes-sombre.png`, puis une capture par outil : `<outil>-<thème>.png`
//! (`spectre`, `freq-gaz`, `freq-temps`…, dans l'ordre de `tools::all_pages`).

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_gui::app::{App, Screen};
use otb_gui::theme;

fn main() {
    let mut args = std::env::args().skip(1);
    let log = args.next().expect("chemin du log");
    let out = std::path::PathBuf::from(args.next().expect("dossier de sortie"));
    let others: Vec<String> = args.collect();
    for th in [theme::CLAIR, theme::SOMBRE] {
        let mut app = App::new(otb_gui::tools::all_pages());
        app.theme = th;
        let mut h = Harness::builder()
            .with_size(egui::vec2(1500.0, 900.0))
            .with_context_setup(otb_gui::app::setup)
            .wgpu()
            .build_ui_state(|ui, a: &mut App| a.ui(ui), app);
        let mut all: Vec<std::path::PathBuf> = vec![log.clone().into()];
        all.extend(others.iter().map(std::path::PathBuf::from));
        let wanted = all.len();
        h.state_mut().open(all);
        for _ in 0..400 {
            h.run();
            if h.state().state.logs.len() >= wanted {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        for _ in 0..4 {
            h.run();
        }
        assert_eq!(h.state().screen, Screen::Viewer);
        // une mesure et un curseur au milieu du vol, pour voir le lecteur et les repères
        let (lo, hi) = {
            let l = &h.state().state.logs[0].log;
            (l.t[0], *l.t.last().unwrap())
        };
        h.state_mut().state.set_cursor(lo + (hi - lo) * 0.45);
        h.state_mut().viewer.measure_on = true;
        h.state_mut().viewer.measure = vec![lo + (hi - lo) * 0.30, lo + (hi - lo) * 0.33];
        h.run_steps(4);
        let img = h.render().expect("rendu");
        img.save(out.join(format!("courbes-{}.png", th.name))).unwrap();
        // le guide : l'accueil, puis la phase des filtres avec la fiche du drone de ce log (fiche en mémoire, jamais le vrai fichier)
        let nom = h.state().state.logs[0].log.settings.craft_name.clone();
        let mut fiche = otb_advisor::drone_card::DroneCard::new(&nom);
        fiche.pid_khz = 3.2;
        h.state_mut().guide.cards.list.push(fiche);
        h.state_mut().guide.select_by_name(&nom);
        h.state_mut().screen = Screen::Guide;
        for phase in [otb_gui::guide::HOME, "prep", "filters", "pid"] {
            h.state_mut().guide.phase = phase;
            for _ in 0..600 {
                h.run_steps(2);
                if !h.state().guide.busy() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            h.run_steps(6);
            let img = h.render().expect("rendu");
            img.save(out.join(format!("guide-{phase}-{}.png", th.name))).unwrap();
        }
        // un outil après l'autre : on attend la fin de son calcul avant de regarder
        for i in 0..h.state().pages.len() {
            h.state_mut().screen = Screen::Tool(i);
            for _ in 0..600 {
                h.run_steps(2);
                if !h.state().pages[i].busy() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            h.run_steps(6);
            if wanted > 1 {
                // coche les autres fichiers (le log courant est déjà pris) puis attend le nouveau calcul
                let labels: Vec<String> = h.state().state.logs.iter().map(|l| l.label.clone()).collect();
                let current = h.state().state.current;
                for (k, l) in labels.iter().enumerate() {
                    if Some(k) != current
                        && let Some(n) = h.query_all_by_label(l.as_str()).next()
                    {
                        n.click();
                    }
                }
                for _ in 0..600 {
                    h.run_steps(2);
                    if !h.state().pages[i].busy() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                h.run_steps(6);
            }
            let title = h.state().pages[i].title();
            let name: String = title
                .to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect();
            let img = h.render().expect("rendu");
            img.save(out.join(format!("{name}-{}.png", th.name))).unwrap();
            if title == "Réponse indicielle" {
                // aussi avec les réponses brutes et la bande ± écart-type
                for l in ["Brutes (toutes les fenêtres)", "± écart-type"] {
                    h.get_by_label(l).click();
                }
                let t0 = std::time::Instant::now();
                h.run_steps(8);
                // temps CPU moyen d'un cadre avec les réponses brutes (mesure, jamais sous profileur)
                eprintln!(
                    "{name} brutes ({}) : {:.1} ms par cadre",
                    th.name,
                    t0.elapsed().as_secs_f64() * 1000.0 / 8.0
                );
                let img = h.render().expect("rendu");
                img.save(out.join(format!("{name}-brutes-{}.png", th.name))).unwrap();
            }
        }
    }
}
