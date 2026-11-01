//! Calculs en arrière-plan : un seul à la fois, la dernière demande attendue est rejouée, une panique devient
//! une erreur (port de `test_tool_rerun_when_busy_and_compute_error`).

use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use otb_gui::worker::Runner;

fn attendre(r: &mut Runner<u32>, ctx: &egui::Context) -> Result<u32, String> {
    let debut = Instant::now();
    loop {
        if let Some(res) = r.poll(ctx) {
            return res;
        }
        assert!(debut.elapsed() < Duration::from_secs(10), "calcul jamais terminé");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn une_demande_pendant_un_calcul_attend_puis_la_derniere_gagne() {
    let ctx = egui::Context::default();
    let mut r = Runner::<u32>::default();
    let (ouvre, porte) = channel::<()>();
    r.request(&ctx, move || {
        porte.recv().unwrap(); // bloque jusqu'à ce que le test ouvre la porte
        Ok(1)
    });
    assert!(r.busy() && !r.pending());
    r.request(&ctx, || Ok(2));
    r.request(&ctx, || Ok(3)); // remplace la précédente
    assert!(r.pending());
    assert!(r.poll(&ctx).is_none(), "pas de résultat tant que la porte est fermée");
    ouvre.send(()).unwrap();
    assert_eq!(attendre(&mut r, &ctx), Ok(1));
    assert!(
        r.busy() && !r.pending(),
        "la demande en attente est lancée à la fin du calcul"
    );
    assert_eq!(attendre(&mut r, &ctx), Ok(3));
    assert!(!r.busy());
    assert!(r.poll(&ctx).is_none());
}

#[test]
fn une_erreur_et_une_panique_sont_rendues_sans_planter() {
    let ctx = egui::Context::default();
    let mut r = Runner::<u32>::default();
    r.request(&ctx, || Err("panne simulée".into()));
    assert_eq!(attendre(&mut r, &ctx), Err("panne simulée".into()));
    r.request(&ctx, || -> Result<u32, String> { panic!("boum") });
    let e = attendre(&mut r, &ctx).unwrap_err();
    assert!(e.contains("boum"), "{e}");
    assert!(!r.busy());
    r.request(&ctx, || Ok(7)); // le coureur reste utilisable
    assert_eq!(attendre(&mut r, &ctx), Ok(7));
}
