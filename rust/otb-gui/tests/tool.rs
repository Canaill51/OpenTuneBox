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
