//! `ToolPage` : recalcul quand les réglages, la sélection ou les logs changent, file d'attente, erreur affichée
//! (port de `test_tool_rerun_when_busy_and_compute_error`).

mod support;

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use egui::Ui;
use otb_gui::state::AppState;
use otb_gui::theme::Theme;
use otb_gui::tool::{LogInput, Tool, ToolPage};
use support::synthetic_log;

static APPELS: AtomicUsize = AtomicUsize::new(0);
/// Tant que le test tient ce verrou, `compute` reste bloqué.
static PORTE: Mutex<()> = Mutex::new(());

#[derive(Clone, PartialEq)]
struct Options {
    facteur: u32,
    panne: bool,
}

struct Essai {
    facteur: u32,
    panne: bool,
}

impl Tool for Essai {
    type Options = Options;
    type Output = (u32, usize);
    const TITLE: &'static str = "Essai";

    fn options(&self) -> Options {
        Options {
            facteur: self.facteur,
            panne: self.panne,
        }
    }

    fn selected(&self, state: &AppState) -> Vec<usize> {
        (0..state.logs.len()).collect()
    }

    fn compute(o: Options, logs: Vec<LogInput>) -> Result<(u32, usize), String> {
        drop(PORTE.lock().unwrap());
        APPELS.fetch_add(1, Ordering::SeqCst);
        if o.panne {
            return Err("panne simulée".into());
        }
        Ok((o.facteur, logs.len()))
    }

    fn controls(&mut self, _: &mut Ui, _: &AppState, _: &Theme) {}
    fn render(&mut self, _: &mut Ui, _: &mut AppState, _: &(u32, usize), _: &Theme) {}
}

fn attendre(p: &mut ToolPage<Essai>, ctx: &egui::Context, s: &AppState) {
    let debut = Instant::now();
    while p.busy() {
        p.update(ctx, s);
        assert!(debut.elapsed() < Duration::from_secs(10), "calcul jamais terminé");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn recalcul_file_d_attente_et_erreur() {
    let ctx = egui::Context::default();
    let mut state = AppState::new();
    let mut page = ToolPage::new(Essai {
        facteur: 1,
        panne: false,
    });

    // 1. premier cadre : le calcul part tout seul avec les réglages et les logs actuels
    state.add(synthetic_log("a", 100, 1000.0));
    page.update(&ctx, &state);
    attendre(&mut page, &ctx, &state);
    assert_eq!(page.result(), Some(&(1, 1)));
    assert_eq!(APPELS.load(Ordering::SeqCst), 1);

    // 2. rien n'a changé : aucun recalcul
    page.update(&ctx, &state);
    page.update(&ctx, &state);
    assert!(!page.busy());
    assert_eq!(APPELS.load(Ordering::SeqCst), 1);

    // 3. changements pendant un calcul : un seul recalcul à la fin, avec les derniers réglages
    {
        let _verrou = PORTE.lock().unwrap();
        page.tool.facteur = 2;
        page.update(&ctx, &state); // lance (bloqué sur la porte)
        assert!(page.busy());
        page.tool.facteur = 3;
        page.update(&ctx, &state); // en attente
        page.tool.facteur = 4;
        state.add(synthetic_log("b", 100, 1000.0));
        page.update(&ctx, &state); // remplace l'attente
    }
    attendre(&mut page, &ctx, &state);
    assert_eq!(page.result(), Some(&(4, 2)), "le dernier réglage et les deux logs");
    assert_eq!(APPELS.load(Ordering::SeqCst), 3, "2 lancés + 1 rejoué, pas 4");

    // 4. une erreur finit dans la barre d'état, le résultat précédent reste, la page reste utilisable
    page.tool.panne = true;
    page.update(&ctx, &state);
    attendre(&mut page, &ctx, &state);
    assert!(
        page.status.starts_with("Erreur de calcul") && page.status.contains("panne simulée"),
        "{}",
        page.status
    );
    assert_eq!(page.result(), Some(&(4, 2)));
    page.tool.panne = false;
    page.update(&ctx, &state);
    attendre(&mut page, &ctx, &state);
    assert!(page.status.is_empty());
    assert_eq!(page.result(), Some(&(4, 2)));

    // 5. seule la période d'un log change (mêmes réglages, même sélection) : le calcul est refait
    let avant = APPELS.load(Ordering::SeqCst);
    state.set_period(0, 0.02, 0.05);
    page.update(&ctx, &state);
    attendre(&mut page, &ctx, &state);
    assert_eq!(APPELS.load(Ordering::SeqCst), avant + 1);

    // 6. plus aucun log : pas de calcul, l'ancien résultat disparaît, message comme en Python
    let avant = APPELS.load(Ordering::SeqCst);
    state.clear();
    page.update(&ctx, &state);
    assert!(!page.busy());
    assert_eq!(APPELS.load(Ordering::SeqCst), avant);
    assert_eq!(page.result(), None);
    assert_eq!(page.status, "Sélectionnez au moins un log.");
}

/// Tant que le test tient ce verrou, `compute` de `Perime` reste bloqué (verrou à part : `Essai` compte ses appels).
static PORTE_PERIME: Mutex<()> = Mutex::new(());

/// Outil dont le résultat dit combien de logs ont été donnés au calcul.
struct Perime {
    /// Indices sélectionnés.
    choix: Vec<usize>,
}

impl Tool for Perime {
    type Options = ();
    type Output = Vec<String>;
    const TITLE: &'static str = "Perime";

    fn options(&self) {}
    fn selected(&self, state: &AppState) -> Vec<usize> {
        self.choix.iter().copied().filter(|&i| i < state.logs.len()).collect()
    }
    fn compute(_: (), logs: Vec<LogInput>) -> Result<Vec<String>, String> {
        drop(PORTE_PERIME.lock().unwrap());
        Ok(logs.into_iter().map(|l| l.label).collect())
    }
    fn controls(&mut self, _: &mut Ui, _: &AppState, _: &Theme) {}
    fn render(&mut self, _: &mut Ui, _: &mut AppState, _: &Vec<String>, _: &Theme) {}
}

fn finir(p: &mut ToolPage<Perime>, ctx: &egui::Context, s: &AppState) {
    let debut = Instant::now();
    while p.busy() {
        p.update(ctx, s);
        assert!(debut.elapsed() < Duration::from_secs(10), "calcul jamais terminé");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Un calcul lancé avant que l'état soit vidé ne doit pas rendre son résultat à une page sans log (C2).
#[test]
fn un_calcul_termine_apres_le_vidage_n_affiche_rien() {
    let ctx = egui::Context::default();
    let mut state = AppState::new();
    state.add(synthetic_log("a", 100, 1000.0));
    let mut page = ToolPage::new(Perime { choix: vec![0] });
    {
        let _porte = PORTE_PERIME.lock().unwrap();
        page.update(&ctx, &state); // lance, bloqué
        assert!(page.busy());
        state.clear();
        page.update(&ctx, &state); // plus de log : résultat effacé, message
        assert_eq!(page.status, "Sélectionnez au moins un log.");
    }
    finir(&mut page, &ctx, &state);
    page.update(&ctx, &state);
    assert_eq!(page.result(), None, "le résultat de l'ancien état ne doit pas revenir");
    assert_eq!(page.status, "Sélectionnez au moins un log.");
}

/// Changer la sélection pendant un calcul : seul le résultat de la dernière sélection est affiché.
#[test]
fn le_resultat_de_l_ancienne_selection_ne_s_affiche_jamais() {
    let ctx = egui::Context::default();
    let mut state = AppState::new();
    state.add(synthetic_log("a", 100, 1000.0));
    state.add(synthetic_log("b", 100, 1000.0));
    let mut page = ToolPage::new(Perime { choix: vec![0] });
    {
        let _porte = PORTE_PERIME.lock().unwrap();
        page.update(&ctx, &state); // lance le log 0, bloqué
        page.tool.choix = vec![1];
        page.update(&ctx, &state); // la sélection change : demande en attente
    }
    // le premier calcul finit : son résultat (log « a ») ne doit jamais être vu
    let debut = Instant::now();
    while page.busy() {
        page.update(&ctx, &state);
        if let Some(r) = page.result() {
            assert_eq!(r, &vec!["b".to_string()], "résultat de l'ancienne sélection affiché");
        }
        assert!(debut.elapsed() < Duration::from_secs(10), "calcul jamais terminé");
        std::thread::sleep(Duration::from_millis(2));
    }
    page.update(&ctx, &state);
    assert_eq!(page.result(), Some(&vec!["b".to_string()]));
}

#[test]
fn un_message_d_erreur_trop_long_garde_ses_600_derniers_caracteres() {
    use otb_gui::tool::{ERROR_CHARS, error_status};
    assert_eq!(error_status("court"), "Erreur de calcul : court");
    let long = format!("début{}fin é", "x".repeat(2000));
    let s = error_status(&long);
    let corps = s.strip_prefix("Erreur de calcul : ").unwrap();
    assert_eq!(corps.chars().count(), ERROR_CHARS);
    assert!(corps.ends_with("fin é") && !corps.contains("début"));
    // exactement 600 : rien n'est coupé
    let juste = "y".repeat(ERROR_CHARS);
    assert_eq!(error_status(&juste), format!("Erreur de calcul : {juste}"));
}

// ---- export.rs : fonctions pures de l'export PNG (6-6a) ----

mod export {
    use egui::{Color32, ColorImage, Pos2, Rect};
    use otb_gui::export::{
        civil_from_days, crop, date_from_secs, default_export_path, today_utc, with_png_extension, write_png,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn dates_civiles_connues() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(59), (1970, 3, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29)); // année bissextile
        assert_eq!(civil_from_days(20_394), (2025, 11, 2));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }

    #[test]
    fn nom_par_defaut_premier_numero_libre() {
        let dir = Path::new("/d");
        let jour = (2026, 11, 2);
        let libre = default_export_path(dir, jour, &|_| false);
        assert_eq!(libre, PathBuf::from("/d/OTB_FIGS_2026-11-02-1.png"));
        // 1, 2 et 3 pris : le premier libre est 4 (un pas de 2 sauterait de 1 à 3 puis 5)
        let pris: Vec<PathBuf> = ["1", "2", "3"]
            .iter()
            .map(|n| PathBuf::from(format!("/d/OTB_FIGS_2026-11-02-{n}.png")))
            .collect();
        let p = default_export_path(dir, jour, &|f| pris.iter().any(|x| x == f));
        assert_eq!(p, PathBuf::from("/d/OTB_FIGS_2026-11-02-4.png"));
        // un trou : le premier numéro libre, pas le suivant du dernier
        let trou = default_export_path(dir, jour, &|f| f == Path::new("/d/OTB_FIGS_2026-11-02-1.png"));
        assert_eq!(trou, PathBuf::from("/d/OTB_FIGS_2026-11-02-2.png"));
    }

    #[test]
    fn date_d_un_instant_en_secondes() {
        assert_eq!(date_from_secs(0), (1970, 1, 1));
        assert_eq!(date_from_secs(86_399), (1970, 1, 1), "dernière seconde du jour");
        assert_eq!(date_from_secs(86_400), (1970, 1, 2), "première seconde du lendemain");
        assert_eq!(date_from_secs(-1), (1969, 12, 31));
        assert_eq!(date_from_secs(20_394 * 86_400 + 5), (2025, 11, 2));
        // la date du jour est celle du système : au moins 2026, mois et jour valides
        let (y, m, d) = today_utc();
        assert!(
            y >= 2026 && (1..=12).contains(&m) && (1..=31).contains(&d),
            "{y}-{m}-{d}"
        );
    }

    #[test]
    fn mois_et_jour_sur_deux_chiffres() {
        let p = default_export_path(Path::new("/d"), (2026, 1, 5), &|_| false);
        assert_eq!(p, PathBuf::from("/d/OTB_FIGS_2026-01-05-1.png"));
    }

    #[test]
    fn extension_png_ajoutee_seulement_si_absente() {
        assert_eq!(with_png_extension("/d/a".into()), PathBuf::from("/d/a.png"));
        assert_eq!(with_png_extension("/d/a.png".into()), PathBuf::from("/d/a.png"));
        assert_eq!(with_png_extension("/d/a.PNG".into()), PathBuf::from("/d/a.PNG"));
        assert_eq!(with_png_extension("/d/a.v2".into()), PathBuf::from("/d/a.v2.png"));
    }

    fn degrade(w: usize, h: usize) -> ColorImage {
        let mut img = ColorImage::filled([w, h], Color32::BLACK);
        for y in 0..h {
            for x in 0..w {
                img.pixels[y * w + x] = Color32::from_rgb(x as u8, y as u8, 7);
            }
        }
        img
    }

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
    }

    #[test]
    fn recadrage_en_points_avec_echelle() {
        let img = degrade(40, 20);
        // 2 pixels par point : le rectangle (5,2)-(15,8) devient (10,4)-(30,16) en pixels
        let c = crop(&img, rect(5.0, 2.0, 15.0, 8.0), 2.0).unwrap();
        assert_eq!(c.size, [20, 12]);
        assert_eq!(c.pixels[0], Color32::from_rgb(10, 4, 7));
        assert_eq!(c.pixels[c.size[0] * c.size[1] - 1], Color32::from_rgb(29, 15, 7));
    }

    #[test]
    fn recadrage_arrondit_vers_l_exterieur() {
        let img = degrade(40, 20);
        // (5,3 ; 2,6) → (15,2 ; 8,1) : début arrondi vers le bas, fin vers le haut = pixels 5..16 et 2..9
        let c = crop(&img, rect(5.3, 2.6, 15.2, 8.1), 1.0).unwrap();
        assert_eq!(c.size, [11, 7]);
        assert_eq!(c.pixels[0], Color32::from_rgb(5, 2, 7));
        // même chose avec une échelle : (2,6 ; 1,3) → (7,6 ; 4,2) à 2 px/pt = 5,2..15,2 et 2,6..8,4
        let c = crop(&img, rect(2.6, 1.3, 7.6, 4.2), 2.0).unwrap();
        assert_eq!(c.size, [11, 7]);
        assert_eq!(c.pixels[0], Color32::from_rgb(5, 2, 7));
    }

    #[test]
    fn recadrage_borne_a_l_image() {
        let img = degrade(40, 20);
        let c = crop(&img, rect(-5.0, -5.0, 1000.0, 1000.0), 1.0).unwrap();
        assert_eq!(c.size, [40, 20]);
        let c = crop(&img, rect(30.0, 10.0, 100.0, 100.0), 1.0).unwrap();
        assert_eq!(c.size, [10, 10]);
    }

    #[test]
    fn recadrage_vide_ou_hors_image() {
        let img = degrade(40, 20);
        assert!(crop(&img, rect(50.0, 0.0, 60.0, 10.0), 1.0).is_none());
        assert!(crop(&img, rect(10.0, 10.0, 10.0, 15.0), 1.0).is_none());
        assert!(crop(&img, rect(-9.0, -9.0, -1.0, -1.0), 1.0).is_none());
    }

    #[test]
    fn png_ecrit_puis_relu_identique() {
        let img = degrade(13, 9);
        let dir = std::env::temp_dir().join(format!("otb-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("essai.png");
        write_png(&img, &f).unwrap();
        let lu = image::open(&f).unwrap().to_rgba8();
        assert_eq!((lu.width(), lu.height()), (13, 9));
        for (i, p) in lu.pixels().enumerate() {
            assert_eq!(p.0, img.pixels[i].to_array(), "pixel {i}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn png_dossier_inexistant_rend_une_erreur() {
        let img = degrade(2, 2);
        let f = std::env::temp_dir().join("otb-absent-xyz").join("sous").join("a.png");
        assert!(write_png(&img, &f).is_err());
    }
}

// ---- boutons « Calculer » et « Exporter image » d'un outil (6-6a) ----

mod boutons {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};
    use otb_gui::tool::Page;

    static COMPTE: AtomicUsize = AtomicUsize::new(0);
    /// Tant que le test tient ce verrou, le calcul du `Compteur` reste bloqué.
    static BARRE: Mutex<()> = Mutex::new(());

    struct Compteur;

    impl Tool for Compteur {
        type Options = ();
        type Output = usize;
        const TITLE: &'static str = "Compteur";
        fn options(&self) {}
        fn selected(&self, state: &AppState) -> Vec<usize> {
            (0..state.logs.len()).collect()
        }
        fn compute(_: (), _: Vec<LogInput>) -> Result<usize, String> {
            drop(BARRE.lock().unwrap());
            Ok(COMPTE.fetch_add(1, Ordering::SeqCst) + 1)
        }
        fn controls(&mut self, _: &mut Ui, _: &AppState, _: &Theme) {}
        fn render(&mut self, _: &mut Ui, _: &mut AppState, _: &usize, _: &Theme) {}
    }

    fn attendre(h: &mut Harness<'static, (ToolPage<Compteur>, AppState)>) {
        let debut = Instant::now();
        while h.state().0.busy() {
            h.step();
            assert!(debut.elapsed() < Duration::from_secs(10), "calcul jamais terminé");
            std::thread::sleep(Duration::from_millis(2));
        }
        h.step();
    }

    #[test]
    fn calculer_relance_meme_sans_changement_et_se_grise_pendant_le_calcul() {
        let mut state = AppState::new();
        state.add(synthetic_log("a", 100, 1000.0));
        let mut h = Harness::builder().with_size(egui::vec2(700.0, 400.0)).build_ui_state(
            |ui, (page, state): &mut (ToolPage<Compteur>, AppState)| page.ui(ui, state, &otb_gui::theme::CLAIR),
            (ToolPage::new(Compteur), state),
        );
        h.step(); // le premier calcul part tout seul
        attendre(&mut h);
        assert_eq!(COMPTE.load(Ordering::SeqCst), 1);
        h.step();
        assert_eq!(COMPTE.load(Ordering::SeqCst), 1, "rien n'a changé : pas de recalcul");

        // le clic relance avec les mêmes réglages
        h.get_by_label("Calculer").click();
        h.step();
        attendre(&mut h);
        assert_eq!(COMPTE.load(Ordering::SeqCst), 2);
        assert_eq!(h.state().0.result(), Some(&2));

        // pendant un calcul, le bouton est grisé
        {
            let _barre = BARRE.lock().unwrap();
            h.get_by_label("Calculer").click();
            h.step();
            assert!(h.state().0.busy());
            h.step();
            assert!(h.get_by_label("Calculer").accesskit_node().is_disabled());
        }
        attendre(&mut h);
        assert_eq!(COMPTE.load(Ordering::SeqCst), 3);
        assert!(!h.get_by_label("Calculer").accesskit_node().is_disabled());

        // sans log sélectionné : le message de l'état, comme au recalcul automatique
        h.state_mut().1.clear();
        h.step();
        h.get_by_label("Calculer").click();
        h.step();
        assert_eq!(h.state().0.status, "Sélectionnez au moins un log.");
        assert_eq!(h.state().0.result(), None);
        assert_eq!(COMPTE.load(Ordering::SeqCst), 3, "aucun calcul sans log");
    }

    /// Outil minimal avec « Tout voir » : compte les appels de `reset_view`.
    struct AvecVue {
        resets: usize,
    }

    impl Tool for AvecVue {
        type Options = ();
        type Output = ();
        const TITLE: &'static str = "AvecVue";
        const HAS_VIEW: bool = true;
        fn options(&self) {}
        fn selected(&self, _: &AppState) -> Vec<usize> {
            Vec::new()
        }
        fn compute(_: (), _: Vec<LogInput>) -> Result<(), String> {
            Ok(())
        }
        fn controls(&mut self, _: &mut Ui, _: &AppState, _: &Theme) {}
        fn render(&mut self, _: &mut Ui, _: &mut AppState, _: &(), _: &Theme) {}
        fn reset_view(&mut self) {
            self.resets += 1;
        }
    }

    #[test]
    fn tout_voir_n_existe_que_pour_les_outils_a_graphiques_zoomables() {
        let nouveau = |tool| {
            Harness::builder().with_size(egui::vec2(700.0, 400.0)).build_ui_state(
                |ui, (page, state): &mut (ToolPage<AvecVue>, AppState)| page.ui(ui, state, &otb_gui::theme::CLAIR),
                (ToolPage::new(tool), AppState::new()),
            )
        };
        let mut h = nouveau(AvecVue { resets: 0 });
        h.step();
        h.get_by_label("Tout voir").click();
        h.step();
        h.get_by_label("Tout voir").click();
        h.step();
        assert_eq!(h.state().0.tool.resets, 2, "un appel par clic");
        // la Configuration, l'Assistant… (HAS_VIEW par défaut = faux) n'ont pas le bouton
        let mut h = Harness::builder().with_size(egui::vec2(700.0, 400.0)).build_ui_state(
            |ui, (page, state): &mut (ToolPage<Compteur>, AppState)| page.ui(ui, state, &otb_gui::theme::CLAIR),
            (ToolPage::new(Compteur), AppState::new()),
        );
        h.step();
        assert!(h.query_by_label("Tout voir").is_none());
    }

    #[test]
    fn exporter_image_demande_la_page_entiere_une_seule_fois() {
        let mut h = Harness::builder().with_size(egui::vec2(700.0, 400.0)).build_ui_state(
            |ui, (page, state): &mut (ToolPage<Compteur>, AppState)| page.ui(ui, state, &otb_gui::theme::CLAIR),
            (ToolPage::new(Compteur), AppState::new()),
        );
        h.step();
        assert!(!h.state_mut().0.take_export());
        h.get_by_label("Exporter image").click();
        h.step();
        assert!(h.state_mut().0.take_export(), "demande notée");
        assert!(!h.state_mut().0.take_export(), "lue une seule fois");
    }
}

// ---- « Tout voir » de chaque outil à graphiques zoomables : la vue retenue est bien oubliée (6-6a) ----

mod vue {
    use super::*;
    use egui_kittest::Harness;
    use otb_gui::tool::Page;
    use otb_gui::tools::filter_sim::FilterSimTool;
    use otb_gui::tools::freq_throttle::FreqThrottleTool;
    use otb_gui::tools::freq_time::FreqTimeTool;
    use otb_gui::tools::spectral::SpectralTool;
    use otb_gui::tools::step::StepTool;

    /// Affiche l'outil sur le log d'essai, vérifie qu'il retient une vue, puis que `reset_view` l'oublie.
    fn essai<T: Tool + 'static>(tool: T, sans_vue: fn(&T) -> bool) {
        let mut state = AppState::new();
        let log = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
        state.add(otb_log::flightlog::load(&log).unwrap().remove(0));
        let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).build_ui_state(
            |ui, (page, state): &mut (ToolPage<T>, AppState)| page.ui(ui, state, &otb_gui::theme::CLAIR),
            (ToolPage::new(tool), state),
        );
        let debut = Instant::now();
        while h.state().0.result().is_none() {
            h.step();
            assert!(debut.elapsed() < Duration::from_secs(60), "calcul jamais terminé");
            std::thread::sleep(Duration::from_millis(10));
        }
        h.run_steps(2);
        assert!(!sans_vue(&h.state().0.tool), "le graphique a retenu sa vue");
        h.state_mut().0.tool.reset_view();
        assert!(sans_vue(&h.state().0.tool), "« Tout voir » oublie la vue");
        h.run_steps(2);
        assert!(!sans_vue(&h.state().0.tool), "elle est reposée au cadre suivant");
    }

    #[test]
    fn spectre() {
        essai(SpectralTool::new(), |t| t.view.is_unset());
    }

    #[test]
    fn frequence_gaz() {
        essai(FreqThrottleTool::new(), |t| t.view.is_unset());
    }

    #[test]
    fn frequence_temps() {
        essai(FreqTimeTool::new(), |t| t.view.is_unset());
    }

    #[test]
    fn reponse_indicielle() {
        essai(StepTool::new(), |t| t.view.is_unset());
    }

    #[test]
    fn simulateur_de_filtres() {
        essai(FilterSimTool::new(), |t| t.view_is_unset());
    }
}
