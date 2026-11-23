//! Graphiques du suivi (lot 31c, D-072) : un graphique par grandeur de la fiche cible, au fil des vols du drone, la
//! zone visée lue dans les cases de la fiche. Les mesures sont posées à la main dans une bibliothèque en mémoire : ces
//! tests ne calculent rien, ils vérifient ce qui se trace.
//!
//! **Contre-preuves** (L-071) : chaque protection a été cassée à la main et le test visé a rougi — c'est noté sous
//! le test concerné.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_advisor::rules::{STEP_MIN_WINDOWS, STEP_SURE_WINDOWS};
use otb_advisor::target::ROWS;
use otb_advisor::tracked::{
    ALL_AXES, AXIS_KEYS, GAIN_MARGIN_DB, GYRO_NOISE_DB, LATENCY_MS, PEAK, PHASE_MARGIN_DEG, RISE_MS, STEP_WINDOWS,
    Tracked, WHOLE_KEYS,
};
use otb_gui::app::App;
use otb_gui::drone_sheet::{SheetForm, band_text, hover_text, target_values};
use otb_gui::tracking::{Chart, Charts, charts_of, measures_of};
use otb_store::Library;
use otb_store::model::{Drone, DroneSheet, Fragment, LogEntry};

const DRONE: &str = "uid-1";

/// Un log et son fragment, daté `date`.
fn fragment(lib: &mut Library, id: &str, date: &str) -> Fragment {
    lib.add_log(LogEntry {
        id: format!("log-{id}"),
        drone_id: DRONE.into(),
        name: format!("LOG-{id}"),
        path: format!("/nulle/part/{id}.bbl"),
        copied: false,
        bytes: 1,
        added: date.into(),
    });
    let f = Fragment {
        id: id.into(),
        log_id: format!("log-{id}"),
        session: 1,
        start: 100,
        end: 9000,
        name: format!("Vol {id}"),
        role: String::new(),
        date: date.into(),
        chosen: true,
    };
    lib.put_fragment(f.clone());
    f
}

/// Range les 22 grandeurs d'un fragment : `latency` sur les trois axes, `windows` fenêtres de réponse indicielle,
/// `noise` pour le bruit du gyro, et une valeur quelconque ailleurs.
fn mesurer(lib: &mut Library, f: &Fragment, latency: f64, windows: f64, noise: f64) {
    let mut v = Vec::new();
    for axis in 0..3 {
        for key in AXIS_KEYS {
            let value = match key {
                LATENCY_MS => latency,
                STEP_WINDOWS => windows,
                _ => 40.0,
            };
            v.push(Tracked { axis, key, value });
        }
    }
    for key in WHOLE_KEYS {
        let value = if key == GYRO_NOISE_DB { noise } else { 1.0 };
        v.push(Tracked {
            axis: ALL_AXES,
            key,
            value,
        });
    }
    for m in measures_of(f, "profil=five;filtres=rejeu", &v) {
        lib.set_measure(m);
    }
}

fn bibliotheque() -> Library {
    let mut lib = Library::in_memory();
    lib.put_drone(Drone::new(DRONE, "Jeno"));
    lib
}

fn chart<'a>(c: &'a Charts, key: &str) -> &'a Chart {
    c.charts.iter().find(|c| c.key == key).unwrap()
}

/// Les valeurs tracées d'un axe, par numéro de vol.
fn points(c: &Charts, key: &str, axis: u64) -> Vec<(usize, f64, bool)> {
    chart(c, key)
        .lines
        .iter()
        .find(|l| l.axis == axis)
        .map(|l| l.points.iter().map(|p| (p.flight, p.value, p.sure)).collect())
        .unwrap_or_default()
}

const SANS_CIBLE: [Option<f64>; 10] = [None; 10];

// ---------------------------------------------------------------- sans écran

/// Les vols se rangent par date (pas dans l'ordre d'arrivée dans la bibliothèque) ; un fragment pas encore mesuré, ou
/// déplacé depuis sa mesure, n'a pas de numéro.
///
/// Contre-preuves vues en échec (2026-11-23) : sans le tri de `charts_of`, le vol 1 est « Vol b » ; sans le filtre
/// `is_measured`, les fragments `c` et `d` prennent un numéro.
#[test]
fn les_vols_se_rangent_par_date_et_seuls_les_fragments_mesures_comptent() {
    let mut lib = bibliotheque();
    let b = fragment(&mut lib, "b", "2026-11-22");
    let a = fragment(&mut lib, "a", "2026-11-20");
    fragment(&mut lib, "c", "2026-11-21"); // jamais mesuré
    let d = fragment(&mut lib, "d", "2026-11-19");
    mesurer(&mut lib, &b, 12.0, 500.0, -40.0);
    mesurer(&mut lib, &a, 15.0, 500.0, -35.0);
    mesurer(&mut lib, &d, 18.0, 500.0, -30.0);
    // `d` est déplacé dans l'écran d'extraction : sa mesure parle d'un autre morceau de vol
    let mut deplace = d.clone();
    deplace.end -= 50;
    lib.put_fragment(deplace);

    let c = charts_of(&lib, DRONE, &SANS_CIBLE);
    let noms: Vec<&str> = c.flights.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(noms, ["Vol a", "Vol b"]);
    assert_eq!(points(&c, LATENCY_MS, 0), [(1, 15.0, true), (2, 12.0, true)]);
    assert_eq!(
        points(&c, GYRO_NOISE_DB, ALL_AXES),
        [(1, -35.0, true), (2, -40.0, true)]
    );
    assert_eq!(c.charts.len(), 9, "un graphique par grandeur de la fiche cible");
}

/// Sous `STEP_MIN_WINDOWS` fenêtres, pas de point (l'assistant ne juge pas) ; sous `STEP_SURE_WINDOWS`, un point
/// « moins sûr », sauf pour les marges de la boucle ouverte ; une grandeur non mesurée (NaN) n'a pas de point.
///
/// Contre-preuves vues en échec (2026-11-23) : sans le seuil `STEP_MIN_WINDOWS`, le vol 1 a un point de latence ; sans
/// l'exception des marges, la marge de phase du vol 2 est « moins sûre ».
#[test]
fn peu_de_fenetres_pas_de_point_ou_un_point_moins_sur() {
    let mut lib = bibliotheque();
    let peu = fragment(&mut lib, "a", "2026-11-20");
    let moyen = fragment(&mut lib, "b", "2026-11-21");
    let assez = fragment(&mut lib, "c", "2026-11-22");
    let rien = fragment(&mut lib, "d", "2026-11-23");
    mesurer(&mut lib, &peu, 10.0, (STEP_MIN_WINDOWS - 1) as f64, -30.0);
    mesurer(&mut lib, &moyen, 11.0, STEP_MIN_WINDOWS as f64, -31.0);
    mesurer(&mut lib, &assez, 12.0, STEP_SURE_WINDOWS as f64, -32.0);
    mesurer(&mut lib, &rien, f64::NAN, STEP_SURE_WINDOWS as f64, f64::NAN);

    let c = charts_of(&lib, DRONE, &SANS_CIBLE);
    assert_eq!(c.flights.len(), 4, "un fragment aux mesures vides reste un vol mesuré");
    assert_eq!(points(&c, LATENCY_MS, 2), [(2, 11.0, false), (3, 12.0, true)]);
    assert_eq!(
        points(&c, PHASE_MARGIN_DEG, 1),
        [(2, 40.0, true), (3, 40.0, true), (4, 40.0, true)]
    );
    assert!(
        points(&c, GAIN_MARGIN_DB, 0).iter().all(|p| p.2),
        "marges : jamais « moins sûres »"
    );
    assert_eq!(
        points(&c, GYRO_NOISE_DB, ALL_AXES),
        [(1, -30.0, true), (2, -31.0, true), (3, -32.0, true)],
        "le bruit ne dépend pas des fenêtres"
    );
}

/// La bande suit les cases de la fiche cible : sans rien changer, les valeurs proposées pour la taille (seuils de
/// l'assistant) ; une case vide, pas de bande ; une case changée dans le formulaire, la bande change aussitôt.
///
/// Contre-preuve vue en échec (2026-11-23) : un `target_values` qui ignore les cases du pilote (valeurs proposées
/// seules) laisse la latence à sa valeur proposée.
#[test]
fn la_bande_vient_des_cases_de_la_fiche() {
    let mut lib = bibliotheque();
    let a = fragment(&mut lib, "a", "2026-11-20");
    mesurer(&mut lib, &a, 12.0, 500.0, -30.0);
    let mut sheet = DroneSheet {
        size: "5″".into(),
        ..Default::default()
    };
    let p = otb_advisor::profiles::profile("five").unwrap();

    let c = charts_of(&lib, DRONE, &target_values(&sheet));
    assert_eq!(chart(&c, LATENCY_MS).band, (None, Some(p.latency_ms.1)));
    assert_eq!(chart(&c, RISE_MS).band, (None, None), "pas de seuil : pas de bande");
    assert_eq!(band_text(chart(&c, RISE_MS)), "sans cible");
    assert_eq!(band_text(chart(&c, PEAK)), "cible ≤ 1,20");

    // le pilote remplit le bas de la zone du pic et change la latence
    let mut cases: Vec<(String, String)> = ROWS.iter().map(|r| (r.key.to_string(), String::new())).collect();
    for (k, v) in cases.iter_mut() {
        match k.as_str() {
            "latency_ms" => *v = "9".into(),
            "peak_low" => *v = "1,05".into(),
            "peak_high" => *v = "1,25".into(),
            "phase_margin_deg" => *v = "40".into(),
            _ => {}
        }
    }
    sheet.target = Some(cases);
    let c = charts_of(&lib, DRONE, &target_values(&sheet));
    assert_eq!(chart(&c, LATENCY_MS).band, (None, Some(9.0)));
    assert_eq!(band_text(chart(&c, LATENCY_MS)), "cible ≤ 9 ms");
    assert_eq!(band_text(chart(&c, PEAK)), "cible 1,05 à 1,25");
    assert_eq!(band_text(chart(&c, PHASE_MARGIN_DEG)), "cible ≥ 40 °");
    assert_eq!(chart(&c, GYRO_NOISE_DB).band, (None, None), "case vidée par le pilote");
}

/// La bulle d'un point dit le vol (numéro, nom, date) et la valeur ; rien hors des vols.
#[test]
fn la_bulle_dit_le_vol_et_la_valeur() {
    let mut lib = bibliotheque();
    for (id, date) in [("a", "2026-11-20"), ("b", "2026-11-22")] {
        let f = fragment(&mut lib, id, date);
        mesurer(&mut lib, &f, 12.0, 500.0, -30.0);
    }
    let c = charts_of(&lib, DRONE, &SANS_CIBLE);
    let lat = chart(&c, LATENCY_MS);
    assert_eq!(
        hover_text(lat, &c.flights, "Pitch", 2.1, 12.4).as_deref(),
        Some("Vol 2 : Vol b — 22 nov. 2026\nPitch : 12 ms")
    );
    let bruit = chart(&c, GYRO_NOISE_DB);
    assert_eq!(
        hover_text(bruit, &c.flights, "", 1.0, -30.04).as_deref(),
        Some("Vol 1 : Vol a — 20 nov. 2026\n-30,0 dB")
    );
    assert_eq!(hover_text(lat, &c.flights, "Roll", 0.0, 1.0), None);
    assert_eq!(hover_text(lat, &c.flights, "Roll", 3.0, 1.0), None);
    assert_eq!(hover_text(lat, &c.flights, "Roll", -1.0, 1.0), None);
}

// ---------------------------------------------------------------- l'écran

fn banc(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1300.0, 900.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app)
}

/// La fiche d'un drone mesuré trace ses graphiques avec leur cible ; un drone sans vol mesuré n'en a pas.
///
/// Contre-preuve vue en échec (2026-11-23) : sans l'appel de `tracking_chart` dans la section, « Latence (50 %) (ms) »
/// est introuvable.
#[test]
fn la_fiche_trace_le_suivi_avec_sa_cible() {
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library.put_drone(Drone::new(DRONE, "Jeno"));
    let f = fragment(&mut app.library, "a", "2026-11-20");
    mesurer(&mut app.library, &f, 12.0, 500.0, -30.0);
    app.library.put_drone(Drone::new("uid-2", "Vide"));
    let mut h = banc(app);
    h.run_steps(2);

    let form = SheetForm::edit(h.state().library.drone(DRONE).unwrap());
    h.state_mut().open_sheet(form);
    h.run_steps(3);
    assert!(h.query_by_label_contains("1 fragment mesuré sur 1.").is_some());
    assert!(h.query_by_label_contains("Latence (50 %) (ms)").is_some());
    assert!(h.query_by_label_contains("Retard des filtres D (ms)").is_some());
    assert_eq!(
        h.query_all_by_label_contains("sans cible").count(),
        2,
        "temps de montée et marge de gain : pas de seuil dans l'assistant"
    );
    assert!(h.query_by_label_contains("moins sûre").is_some(), "légende");

    let form = SheetForm::edit(h.state().library.drone("uid-2").unwrap());
    h.state_mut().open_sheet(form);
    h.run_steps(3);
    assert!(
        h.query_by_label_contains("les mesures viendront avec les logs")
            .is_some()
    );
    assert!(h.query_by_label_contains("Latence (50 %) (ms)").is_none());
}
