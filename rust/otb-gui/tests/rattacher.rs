//! Rattacher un log à une fiche drone quand son nom de machine ne le fait pas (D-084) : log sans nom, ou nom
//! qu'aucune fiche ne porte. La fenêtre demande, une ligne par fichier ; « Ne pas ranger » laisse le log pour la
//! séance, et l'écran d'extraction la rouvre.

mod support;
use support::Cliquer;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_gui::app::{App, Screen};
use otb_gui::attach::{must_ask, proposal};
use otb_gui::usb::DroneChoice;
use otb_store::Library;
use otb_store::model::Drone;

fn banc(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app)
}

fn donnees() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")
}

/// Une copie du petit log de test dont la ligne « Craft name » porte `nom_machine` (vide : log sans nom), dans un
/// dossier propre à ce test.
fn copie(nom_machine: &str, tag: &str) -> PathBuf {
    let src = std::fs::read(donnees()).unwrap();
    let ligne = b"H Craft name:JeNo Ced\n";
    let pos = src
        .windows(ligne.len())
        .position(|w| w == ligne)
        .expect("ligne du nom de machine dans l'en-tête");
    let mut out = src[..pos].to_vec();
    out.extend_from_slice(format!("H Craft name:{nom_machine}\n").as_bytes());
    out.extend_from_slice(&src[pos + ligne.len()..]);
    let dir = std::env::temp_dir().join(format!("otb-rattacher-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{tag}.bbl"));
    std::fs::write(&p, out).unwrap();
    p
}

/// Une application dont la bibliothèque (en mémoire) a une fiche créée à la main.
fn avec_fiche(nom: &str) -> App {
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library.put_drone(Drone::new(nom, nom));
    app
}

fn attendre_logs(h: &mut Harness<'static, App>, combien: usize) {
    let debut = Instant::now();
    while h.state().state.logs.len() < combien {
        h.step();
        assert!(
            debut.elapsed() < Duration::from_secs(30),
            "{} logs ouverts sur {combien}",
            h.state().state.logs.len()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(3);
}

fn logs_du_drone(lib: &Library, drone: &str) -> usize {
    lib.logs().iter().filter(|l| l.drone_id == drone).count()
}

/// Le cas qui a fait naître D-084 : une fiche « Whisper 25 » créée à la main, un log **sans nom de machine**. Avant,
/// il n'était rangé nulle part et ses fragments se perdaient à la fermeture. La fenêtre demande ; une fois rattaché
/// (choix fait dans la liste déroulante, comme le pilote), le log, ses fragments et la liste d'analyse sont gardés,
/// et le **même fichier rouvert** est reconnu par son empreinte, sans nouvelle question.
///
/// **Contre-preuve** : en rendant `None` dans `App::remember` pour un log sans nom (l'ancien comportement), la
/// fenêtre ne s'ouvre pas et l'assertion sur `pending` rougit ; sans la recherche par empreinte (`known`), la
/// réouverture redemande et l'assertion finale rougit.
#[test]
fn un_log_sans_nom_se_rattache_a_une_fiche_creee_a_la_main() {
    let chemin = copie("", "sans-nom");
    let mut h = banc(avec_fiche("Whisper 25"));
    h.state_mut().open(vec![chemin.clone()]);
    attendre_logs(&mut h, 1);
    assert!(h.state().library.logs().is_empty(), "rien de rangé avant le choix");
    assert!(h.state().state.logs[0].fragment.is_none());
    let pending = &h.state().attach.pending;
    assert_eq!(pending.len(), 1, "la fenêtre demande la fiche");
    assert_eq!(
        pending[0].choice,
        DroneChoice::New(String::new()),
        "aucune fiche choisie dans le Guide"
    );
    h.get_by_label("Pas de nom de machine dans l'en-tête.");
    // le choix dans la liste, puis « Rattacher »
    h.cliquer_label("Drone de sans-nom.bbl");
    h.run_steps(2);
    h.cliquer_label("Whisper 25");
    h.run_steps(2);
    h.cliquer_label("Rattacher");
    h.run_steps(3);
    let app = h.state();
    assert!(!app.attach.is_open(), "la fenêtre se referme");
    assert_eq!(logs_du_drone(&app.library, "Whisper 25"), 1);
    assert_eq!(app.library.drones().len(), 1, "aucun drone inventé");
    let fid = app.state.logs[0].fragment.clone().expect("la session a son fragment");
    assert!(app.library.fragment(&fid).is_some());
    assert!(
        app.library.analysis().items.iter().any(|it| it.fragment_id == fid),
        "le fragment entre dans la liste d'analyse gardée"
    );
    // rouvert plus tard : reconnu par son empreinte
    h.state_mut().state.clear();
    h.state_mut().open(vec![chemin]);
    attendre_logs(&mut h, 1);
    assert!(!h.state().attach.is_open(), "un log déjà rattaché ne redemande rien");
    assert_eq!(h.state().state.logs[0].fragment.as_deref(), Some(fid.as_str()));
}

/// Un nom de machine qu'aucune fiche ne porte, alors que la bibliothèque en a : la fenêtre propose d'office un
/// nouveau drone de ce nom (ce qui se faisait sans question avant D-084) ; le pilote peut choisir sa fiche à la place.
/// Un nouveau drone au nom d'une fiche existante est refusé, la ligne reste avec son message.
///
/// **Contre-preuve** : avec `must_ask` qui rend `false` pour un nom présent, le drone « WHISPER25 » est créé sans
/// question et l'assertion sur `pending` rougit.
#[test]
fn un_nom_inconnu_demande_et_propose_un_nouveau_drone_de_ce_nom() {
    let chemin = copie("WHISPER25", "nom-inconnu");
    let mut h = banc(avec_fiche("Whisper 25"));
    h.state_mut().open(vec![chemin.clone()]);
    attendre_logs(&mut h, 1);
    assert!(
        h.state().library.drone("WHISPER25").is_none(),
        "pas de drone créé d'office"
    );
    assert_eq!(h.state().attach.pending.len(), 1);
    assert_eq!(h.state().attach.pending[0].choice, DroneChoice::New("WHISPER25".into()));
    h.get_by_label("Nom de machine « WHISPER25 » : aucune fiche ne porte ce nom.");
    // nom déjà pris : refusé, la ligne reste
    h.state_mut().attach.pending[0].choice = DroneChoice::New("Whisper 25".into());
    h.cliquer_label("Rattacher");
    h.run_steps(2);
    assert!(h.state().attach.is_open());
    assert!(
        h.state().attach.pending[0]
            .error
            .as_deref()
            .is_some_and(|e| e.contains("déjà")),
        "{:?}",
        h.state().attach.pending[0].error
    );
    assert!(h.state().library.logs().is_empty());
    // la fiche existante
    h.state_mut().attach.pending[0].choice = DroneChoice::Existing("Whisper 25".into());
    h.cliquer_label("Rattacher");
    h.run_steps(2);
    assert!(!h.state().attach.is_open());
    assert_eq!(logs_du_drone(&h.state().library, "Whisper 25"), 1);
    assert_eq!(h.state().library.drones().len(), 1);
}

/// Sans aucune fiche, un log qui a un nom se range sans question sous un nouveau drone de ce nom : il n'y a rien
/// d'autre à proposer. Un log **sans** nom demande quand même (il faut un nom au drone).
#[test]
fn sans_fiche_un_log_nomme_se_range_sans_question() {
    let mut h = banc(App::new(otb_gui::tools::all_pages()));
    h.state_mut().open(vec![donnees()]);
    attendre_logs(&mut h, 1);
    assert!(!h.state().attach.is_open());
    assert_eq!(logs_du_drone(&h.state().library, "JeNo Ced"), 1);
    h.state_mut().open(vec![copie("", "sans-nom-vide")]);
    attendre_logs(&mut h, 2);
    assert!(h.state().attach.is_open(), "sans nom, toujours demander");
}

/// « Ne pas ranger » laisse le log ouvert pour la séance, sans rien écrire. Un fragment **enregistré** ensuite sur ce
/// log (bornes, nom, rôle) rouvre la fenêtre et passe tel quel à la bibliothèque au rattachement : rien de ce que le
/// pilote a réglé n'est perdu. Le bouton « Rattacher à un drone… » de l'écran d'extraction rouvre aussi la fenêtre.
///
/// **Contre-preuve** : sans `self.unsaved.insert` dans `App::save_fragment`, le fragment rangé garde les limites
/// automatiques et son nom de session, et l'assertion sur `chosen` rougit.
#[test]
fn ne_pas_ranger_puis_rattacher_garde_le_fragment_deja_regle() {
    let chemin = copie("", "plus-tard");
    let mut h = banc(avec_fiche("Whisper 25"));
    h.state_mut().open(vec![chemin]);
    attendre_logs(&mut h, 1);
    h.cliquer_label("Ne pas ranger");
    h.run_steps(2);
    assert!(!h.state().attach.is_open());
    assert!(h.state().library.logs().is_empty(), "rien d'écrit");
    // le bouton de l'écran d'extraction rouvre la fenêtre, « Ne pas ranger » la referme
    h.state_mut().state.set_current(Some(0));
    h.state_mut().enter_extraction();
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Viewer);
    h.cliquer_label("Rattacher à un drone…");
    h.run_steps(2);
    assert!(h.state().attach.is_open());
    h.cliquer_label("Ne pas ranger");
    h.run_steps(2);
    // un fragment réglé puis « enregistré » sur le log non rangé
    let log = h.state().state.logs[0].log.clone();
    let cible = (log.t[400], log.t[6000]);
    let brouillon = h.state_mut().viewer.draft.as_mut().unwrap();
    brouillon.period = cible;
    brouillon.name = "Vol de réglage".into();
    brouillon.role = otb_advisor::log_check::FILTERS.into();
    h.cliquer_label("Enregistrer le fragment");
    h.run_steps(2);
    assert_eq!(h.state().state.logs[0].period, cible, "appliqué pour la séance");
    assert!(h.state().attach.is_open(), "la fenêtre demande la fiche pour le garder");
    h.state_mut().attach.pending[0].choice = DroneChoice::Existing("Whisper 25".into());
    h.cliquer_label("Rattacher");
    h.run_steps(2);
    let app = h.state();
    let fid = app.state.logs[0].fragment.clone().expect("rangé");
    let f = app.library.fragment(&fid).unwrap();
    assert!(f.chosen, "bornes choisies");
    assert_eq!((f.start, f.end), (400, 6001));
    assert_eq!(f.name, "Vol de réglage");
    assert_eq!(f.role, otb_advisor::log_check::FILTERS);
}

/// La proposition d'office : le drone du nom de machine, un nouveau drone de ce nom, ou — sans nom — la fiche
/// choisie dans le Guide si elle est dans la bibliothèque.
#[test]
fn la_proposition_suit_le_nom_puis_la_fiche_du_guide() {
    let mut lib = Library::in_memory();
    assert!(
        !must_ask(&lib, "JeNo Ced"),
        "bibliothèque vide, nom présent : rien à demander"
    );
    assert!(must_ask(&lib, "  "), "sans nom : toujours demander");
    lib.put_drone(Drone::new("Whisper 25", "Whisper 25"));
    assert!(must_ask(&lib, "JeNo Ced"));
    assert_eq!(proposal(&lib, "WHISPER25", None), DroneChoice::New("WHISPER25".into()));
    assert_eq!(
        proposal(&lib, "Whisper 25", None),
        DroneChoice::Existing("Whisper 25".into())
    );
    assert_eq!(
        proposal(&lib, "", Some("Whisper 25")),
        DroneChoice::Existing("Whisper 25".into())
    );
    assert_eq!(proposal(&lib, "", Some("Absent")), DroneChoice::New(String::new()));
    assert_eq!(proposal(&lib, "", None), DroneChoice::New(String::new()));
}
