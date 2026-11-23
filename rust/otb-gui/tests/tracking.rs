//! Mesures suivies dans le temps (lot 31a) : la fiche d'un drone fait lire la table des mesures hors du fil de
//! l'écran, puis mesurer en arrière-plan les fragments qui n'ont pas de mesure valable ; un fragment en échec est dit
//! et pas relancé ; un fragment déplacé pendant le calcul ne reçoit pas une mesure de ses anciennes bornes.
//!
//! **Contre-preuves** (L-071) : chaque protection a été cassée à la main et le test visé a rougi — c'est noté sous
//! le test concerné.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_advisor::tracked::{self, CALC_VERSION, LATENCY_MS};
use otb_gui::app::App;
use otb_gui::drone_sheet::{SheetForm, tracking_lines};
use otb_gui::tracking::{Progress, Tracking, TrackingView};
use otb_store::Library;
use otb_store::model::{Drone, Fragment, LogEntry, Measure};

fn banc(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1300.0, 900.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app)
}

fn donnees() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")
}

fn echantillons() -> usize {
    otb_log::flightlog::load(&donnees()).unwrap()[0].n()
}

/// Le numéro de la session du log d'essai, tel que le décodeur le donne.
fn session() -> u64 {
    otb_log::flightlog::load(&donnees()).unwrap()[0].session as u64
}

/// Un drone, son log (au chemin `chemin`) et un fragment de la session du log d'essai sur `bornes`.
fn bibliotheque(lib: &mut Library, chemin: &str, bornes: (u64, u64)) -> Fragment {
    lib.put_drone(Drone::new("uid-1", "Jeno"));
    lib.add_log(LogEntry {
        id: "emp-1".into(),
        drone_id: "uid-1".into(),
        name: "jeno".into(),
        path: chemin.into(),
        copied: false,
        bytes: 1,
        added: "2026-11-23T10:00:00".into(),
    });
    let f = Fragment {
        id: "emp-1-s".into(),
        log_id: "emp-1".into(),
        session: session(),
        start: bornes.0,
        end: bornes.1,
        name: "Série PID 1".into(),
        role: String::new(),
        date: "2026-11-23".into(),
        chosen: true,
    };
    lib.put_fragment(f.clone());
    f
}

/// Ouvre la fiche du drone « uid-1 » et attend la fin de la lecture et du calcul.
fn fiche_et_attente(h: &mut Harness<'static, App>) {
    let form = SheetForm::edit(h.state().library.drone("uid-1").unwrap());
    h.state_mut().open_sheet(form);
    h.run_steps(2);
    let debut = Instant::now();
    while h.state().tracking.busy() {
        h.step();
        assert!(debut.elapsed() < Duration::from_secs(60), "calcul jamais fini");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(3);
}

fn texte_present(h: &Harness<'static, App>, texte: &str) -> bool {
    h.query_by_label_contains(texte).is_some()
}

// ---------------------------------------------------------------- sans écran

#[test]
fn les_lignes_du_suivi_disent_ou_en_est_le_calcul() {
    let vue = |progress, measured, total| TrackingView {
        progress,
        measured,
        total,
        failed: Vec::new(),
    };
    assert_eq!(
        tracking_lines(&vue(Progress::Reading, 0, 3)),
        ["Lecture des mesures déjà faites…"]
    );
    assert_eq!(
        tracking_lines(&vue(Progress::Measuring { done: 2, total: 5 }, 0, 5)),
        ["Mesure en arrière-plan : 2 fragments sur 5…"]
    );
    assert_eq!(tracking_lines(&vue(Progress::Idle, 1, 3)), ["1 fragment mesuré sur 3."]);
    assert_eq!(
        tracking_lines(&vue(Progress::Idle, 4, 4)),
        ["4 fragments mesurés sur 4."]
    );
    assert_eq!(
        tracking_lines(&vue(Progress::Idle, 0, 0)),
        ["Aucun fragment : les mesures viendront avec les logs de ce drone."]
    );
    let mut v = vue(Progress::Idle, 0, 1);
    v.failed.push(("Série PID 1".into(), "log illisible (absent)".into()));
    assert_eq!(
        tracking_lines(&v)[1],
        "Non mesuré : « Série PID 1 » — log illisible (absent)"
    );
}

/// ⚠️ Un résultat qui revient du fil de calcul n'est rangé que si le fragment a encore les bornes mesurées.
///
/// Contre-preuve vue en échec (2026-11-23) : sans la comparaison des bornes dans `Tracking::poll` (`same` toujours
/// vrai), les mesures des anciennes bornes sont rangées et la seconde assertion rougit.
#[test]
fn un_fragment_deplace_pendant_le_calcul_ne_recoit_pas_lancienne_mesure() {
    let n = echantillons() as u64;
    let mut lib = Library::in_memory();
    let f = bibliotheque(&mut lib, &donnees().to_string_lossy(), (n / 20, n - n / 20));
    let ctx = egui::Context::default();
    let mut t = Tracking::default();
    t.wish(&ctx, &lib, "uid-1", &[]);
    assert!(t.busy(), "calcul lancé");
    // déplacé dans l'écran d'extraction pendant le calcul
    let mut deplace = f.clone();
    deplace.end -= 100;
    lib.put_fragment(deplace);
    let debut = Instant::now();
    while t.busy() {
        t.poll(&mut lib);
        assert!(debut.elapsed() < Duration::from_secs(60), "calcul jamais fini");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        t.view(&lib, "uid-1").failed.is_empty(),
        "le calcul a réussi : seul le déplacement l'écarte"
    );
    assert!(!lib.is_measured(lib.fragment(&f.id).unwrap(), CALC_VERSION));
    assert_eq!(lib.measures_len(), 0, "rien rangé sous les nouvelles bornes");
}

// ---------------------------------------------------------------- l'écran

/// Le fragment d'un log ouvert se mesure en arrière-plan, et la mesure rangée est celle que le calcul rend.
#[test]
fn la_fiche_mesure_les_fragments_du_drone_en_arriere_plan() {
    let n = echantillons() as u64;
    let mut app = App::new(otb_gui::tools::all_pages());
    let f = bibliotheque(&mut app.library, &donnees().to_string_lossy(), (n / 20, n - n / 20));
    let mut h = banc(app);
    h.run_steps(2);
    fiche_et_attente(&mut h);

    let lib = &h.state().library;
    assert!(lib.is_measured(lib.fragment(&f.id).unwrap(), CALC_VERSION));
    let log = otb_log::flightlog::load(&donnees()).unwrap().remove(0);
    let (_, v) = tracked::measure(&log, f.start as usize..f.end as usize).unwrap();
    let attendu = v.iter().find(|t| t.axis == 1 && t.key == LATENCY_MS).unwrap().value;
    assert_eq!(
        lib.measure(&f.id, 1, LATENCY_MS).unwrap().value.to_bits(),
        attendu.to_bits()
    );
    assert!(texte_present(&h, "1 fragment mesuré sur 1."));
}

/// Un fragment dont le log a disparu est dit, avec la raison, et n'est pas relancé à chaque cadre.
///
/// Contre-preuve vue en échec (2026-11-23) : sans la liste `failed` de `Tracking`, un nouveau calcul part au cadre
/// suivant et l'attente de la fin du calcul dépasse son délai.
#[test]
fn un_log_introuvable_est_dit_et_pas_relance() {
    let mut app = App::new(otb_gui::tools::all_pages());
    bibliotheque(&mut app.library, "/nulle/part/jeno.bbl", (0, 1_000));
    let mut h = banc(app);
    h.run_steps(2);
    fiche_et_attente(&mut h);
    assert!(texte_present(&h, "Non mesuré : « Série PID 1 » — log illisible"));
    assert!(texte_present(&h, "0 fragment mesuré sur 1."));
    h.run_steps(5);
    assert!(!h.state().tracking.busy(), "pas relancé");
}

/// La table des mesures déjà écrite se lit à l'ouverture de la fiche, et un fragment déjà mesuré n'est **pas**
/// recalculé : la valeur rangée (fausse exprès) reste.
#[test]
fn une_mesure_deja_rangee_se_lit_sans_se_recalculer() {
    let n = echantillons() as u64;
    let dir = std::env::temp_dir().join(format!("otb-gui-suivi-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    {
        let mut lib = Library::open(dir.clone());
        lib.load_measures();
        let f = bibliotheque(&mut lib, &donnees().to_string_lossy(), (n / 20, n - n / 20));
        lib.set_measure(Measure {
            fragment_id: f.id.clone(),
            axis: 1,
            key: LATENCY_MS.into(),
            value: 99.0,
            start: f.start,
            end: f.end,
            settings: "essai".into(),
            calc_version: CALC_VERSION,
        });
        lib.save().unwrap();
    }
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library = Library::open(dir.clone());
    assert!(!app.library.measures_loaded());
    let mut h = banc(app);
    h.run_steps(2);
    fiche_et_attente(&mut h);
    assert!(h.state().library.measures_loaded());
    assert_eq!(h.state().library.measure("emp-1-s", 1, LATENCY_MS).unwrap().value, 99.0);
    assert!(texte_present(&h, "1 fragment mesuré sur 1."));
    let _ = std::fs::remove_dir_all(&dir);
}
