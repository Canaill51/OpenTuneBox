//! Capture d'écran sans fenêtre (rendu wgpu de egui_kittest) pour regarder l'interface : jamais dans la CI.
//!
//! `cargo run --release -p otb-gui --example capture -- LOG.bbl DOSSIER_DE_SORTIE [AUTRE_LOG…]`
//! Les logs supplémentaires sont ouverts aussi et cochés dans chaque outil (séries de vols : Réponse indicielle).
//! Écrit `courbes-clair.png`, `courbes-sombre.png` (l'écran d'extraction, lot 29b), `fragment-ajoute-<thème>.png` (un second fragment de la session, D-067) et `tableau-de-bord-<thème>.png`, `bibliotheque-<thème>.png` et `bibliotheque-introuvable-<thème>.png` (lot 34, D-081), `tableau-de-bord-releve-<thème>.png` (la carte après le passage du Guide, D-071), `fiche-drone-<thème>.png`, `fiche-drone-suivi-<thème>.png` (les graphiques du suivi, lot 31c), `fiche-drone-bas-<thème>.png`, `barre-desactivee-<thème>.png`, puis une capture par outil : `<outil>-<thème>.png`
//! (`spectre`, `freq-gaz`, `freq-temps`…, dans l'ordre de `tools::all_pages`).

#[path = "../tests/support/clic.rs"]
mod clic;

use clic::Cliquer;
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
        // une mesure et un curseur au milieu du vol, pour voir le lecteur et les repères — sur le log **affiché**
        // (le dernier ouvert), pas sur la première session du fichier
        let (lo, hi) = {
            let l = &h.state().state.current_log().expect("un log affiché").log;
            (l.t[0], *l.t.last().unwrap())
        };
        h.state_mut().state.set_cursor(lo + (hi - lo) * 0.45);
        h.state_mut().viewer.measure_on = true;
        h.state_mut().viewer.measure = vec![lo + (hi - lo) * 0.30, lo + (hi - lo) * 0.33];
        h.run_steps(4);
        let img = h.render().expect("rendu");
        img.save(out.join(format!("courbes-{}.png", th.name))).unwrap();
        // un second fragment de la même session (D-067) : l'écran d'extraction sur lui, avec sa zone de suppression ;
        // puis supprimé, pour que la suite des captures reste celle d'avant
        let deja = h.state().state.logs.len();
        h.state_mut().viewer.draft.as_mut().expect("brouillon").name = "Rafale".into();
        h.cliquer_label("Enregistrer comme nouveau fragment");
        h.run_steps(4);
        assert_eq!(h.state().state.logs.len(), deja + 1, "second fragment posé");
        h.state_mut().enter_extraction();
        h.run_steps(4);
        h.render()
            .expect("rendu")
            .save(out.join(format!("fragment-ajoute-{}.png", th.name)))
            .unwrap();
        h.cliquer_label("Supprimer ce fragment…");
        h.run_steps(2);
        h.cliquer_label("Supprimer définitivement");
        h.run_steps(4);
        assert_eq!(h.state().state.logs.len(), deja, "second fragment supprimé");
        // le tableau de bord : les drones que la bibliothèque a retenus en ouvrant ces logs (lot 30c)
        h.state_mut().screen = Screen::Dashboard;
        h.run_steps(4);
        let drone = h.state().library.drones().first().map(|d| d.id.clone());
        h.state_mut().dash.selected = drone;
        h.run_steps(4);
        h.render()
            .expect("rendu")
            .save(out.join(format!("tableau-de-bord-{}.png", th.name)))
            .unwrap();
        // l'écran de la bibliothèque (lot 34, D-081) : une bibliothèque d'essai dans le dossier temporaire, le
        // déplacement vers un dossier vide qui attend sa confirmation ; puis la même, introuvable au lancement. La
        // bibliothèque des logs ouverts est remise ensuite, pour la suite des captures.
        {
            let essai = std::env::temp_dir().join(format!("otb-capture-bibliotheque-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&essai);
            let (dir, vers) = (essai.join("bibliotheque"), essai.join("Disque externe"));
            std::fs::create_dir_all(&vers).unwrap();
            let mut lib = otb_store::Library::open(dir.clone());
            for d in h.state().library.drones() {
                lib.put_drone(d.clone());
            }
            lib.save().unwrap();
            let garde = std::mem::replace(&mut h.state_mut().library, lib);
            h.state_mut().libpage.default_dir = Some(dir);
            let a = h.state_mut();
            a.libpage.choose(&a.library, &vers);
            a.open_library_screen();
            h.run_steps(4);
            h.render()
                .expect("rendu")
                .save(out.join(format!("bibliotheque-{}.png", th.name)))
                .unwrap();
            h.state_mut().libpage.cancel();
            h.state_mut().libpage.missing = Some(format!(
                "le dossier de la bibliothèque est introuvable : {} (disque débranché ?)",
                vers.join("OpenTuneBox").display()
            ));
            h.run_steps(4);
            h.render()
                .expect("rendu")
                .save(out.join(format!("bibliotheque-introuvable-{}.png", th.name)))
                .unwrap();
            h.state_mut().libpage.missing = None;
            h.state_mut().library = garde;
            h.state_mut().screen = Screen::Dashboard;
            h.run_steps(4);
            let _ = std::fs::remove_dir_all(&essai);
        }
        // la fiche du drone (lot 30d, D-068) : deux « diff all » d'exemple, en mémoire, pour voir la liste des
        // configurations ; la seconde dépliée par « Comparer »
        if let Some(id) = h.state().library.drones().first().map(|d| d.id.clone()) {
            let avant = "profile 0\nset p_roll = 45\nset i_roll = 31\nset d_roll = 39\n";
            let apres = "profile 0\nset p_roll = 45\nset i_roll = 72\nset d_roll = 48\n";
            h.state_mut().library.record_config(&id, "2026-11-02T11:15:00", avant);
            h.state_mut().library.record_config(&id, "2026-11-13T18:15:00", apres);
            let drone = h.state().library.drone(&id).cloned().unwrap();
            let mut form = otb_gui::drone_sheet::SheetForm::edit(&drone);
            let derniere = h
                .state()
                .library
                .configs_of_drone(&id)
                .last()
                .map(|c| c.id.clone())
                .unwrap();
            form.shown = Some((derniere, true));
            h.state_mut().open_sheet(form);
            h.run_steps(4);
            // temps CPU moyen d'un cadre de la fiche (le budget de 60 images par seconde : 16,7 ms)
            let t0 = std::time::Instant::now();
            h.run_steps(8);
            eprintln!(
                "fiche du drone ({}) : {:.1} ms par cadre",
                th.name,
                t0.elapsed().as_secs_f64() * 1000.0 / 8.0
            );
            h.render()
                .expect("rendu")
                .save(out.join(format!("fiche-drone-{}.png", th.name)))
                .unwrap();
            // le bas de l'écran : la configuration comparée, le suivi des mesures une fois le calcul en arrière-plan
            // fini (lot 31a), et la suppression qui attend sa confirmation
            let debut = std::time::Instant::now();
            while h.state().tracking.busy() && debut.elapsed().as_secs() < 120 {
                h.step();
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            // les graphiques du suivi (lot 31c), défilés jusqu'au dernier
            if let Some(dernier) = h.query_by_label_contains("Retard des filtres D (ms)") {
                dernier.scroll_to_me();
                h.run_steps(4);
                h.render()
                    .expect("rendu")
                    .save(out.join(format!("fiche-drone-suivi-{}.png", th.name)))
                    .unwrap();
            }
            h.cliquer_label("Supprimer…");
            h.run_steps(4);
            h.render()
                .expect("rendu")
                .save(out.join(format!("fiche-drone-bas-{}.png", th.name)))
                .unwrap();
            h.state_mut().apply_sheet(otb_gui::drone_sheet::SheetAction::Leave);
            h.run_steps(2);
        }
        // la barre de fragments avec une pastille désactivée (pointillés, barrée), puis réactivée (lot 29c)
        h.state_mut().state.set_active(0, false);
        h.run_steps(4);
        h.render()
            .expect("rendu")
            .save(out.join(format!("barre-desactivee-{}.png", th.name)))
            .unwrap();
        h.state_mut().state.set_active(0, true);
        // le guide : l'accueil, puis la phase des filtres avec la fiche du drone de ce log (fiche en mémoire, jamais le vrai fichier)
        let nom = h.state().state.logs[0].log.settings.craft_name.clone();
        let mut fiche = otb_advisor::drone_card::DroneCard::new(&nom);
        fiche.pid_khz = 3.2;
        h.state_mut().put_card(fiche);
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
        // la carte du drone après le passage du Guide : son étape et sa barre comptent le relevé (lot 30g, D-071)
        h.state_mut().screen = Screen::Dashboard;
        h.run_steps(4);
        h.render()
            .expect("rendu")
            .save(out.join(format!("tableau-de-bord-releve-{}.png", th.name)))
            .unwrap();
        h.state_mut().screen = Screen::Guide;
        // barre d'état vide pour les pages (les messages de l'essai des fragments n'ont rien à y faire) : `open` sans fichier
        // ne fait que l'effacer
        h.state_mut().open(Vec::new());
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
            // tous les fragments ouverts sont actifs dans la barre : rien à cocher page par page (D-066)
            let title = h.state().pages[i].title();
            let name: String = title
                .to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect();
            // temps CPU moyen d'un cadre de la page, calcul terminé (budget de 60 images par seconde : 16,7 ms)
            let t0 = std::time::Instant::now();
            h.run_steps(8);
            eprintln!(
                "{name} ({}) : {:.1} ms par cadre",
                th.name,
                t0.elapsed().as_secs_f64() * 1000.0 / 8.0
            );
            let img = h.render().expect("rendu");
            img.save(out.join(format!("{name}-{}.png", th.name))).unwrap();
            if title == "Réponse indicielle" {
                // aussi avec les réponses brutes et la bande ± écart-type
                for l in ["Brutes (toutes les fenêtres)", "± écart-type"] {
                    h.cliquer_label(l);
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
