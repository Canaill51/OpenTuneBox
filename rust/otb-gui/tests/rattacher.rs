//! Rattacher un log à une fiche drone quand son nom de machine ne le fait pas (D-084) : log sans nom, ou nom
//! qu'aucune fiche ne porte. La fenêtre demande, une ligne par fichier ; « Ne pas ranger » laisse le log pour la
//! séance, et l'écran d'extraction la rouvre.

mod support;
use support::Cliquer;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_advisor::drone_card::DroneCard;
use otb_advisor::log_check::{FILTERS, PID};
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

// ------------------------------------------------------------------ D-090 : le Guide suit la bibliothèque

const CHIRP: &str = "Jeno 5 CHIRP";

fn attendre(h: &mut Harness<'static, App>, fin: impl Fn(&App) -> bool) {
    let debut = Instant::now();
    while !fin(h.state()) {
        h.run_steps(2);
        assert!(debut.elapsed() < Duration::from_secs(60), "attente trop longue");
        std::thread::sleep(Duration::from_millis(10));
    }
    h.run_steps(3);
}

/// Le Guide sur la phase des séries PID, la fiche `choisie` sélectionnée ; une fiche par nom de `noms`.
fn guide_sur(noms: &[&str], choisie: &str) -> App {
    let mut app = App::new(otb_gui::tools::all_pages());
    for n in noms {
        app.put_card(DroneCard::new(n));
    }
    app.guide.select_by_name(choisie);
    app.guide.phase = "pid";
    app
}

/// Le Guide a vérifié ce log (le seul ouvert), avec ce rôle.
fn guide_voit(h: &mut Harness<'static, App>, role: &'static str) {
    attendre(h, |a| !a.guide.busy() && !a.guide.seen().is_empty());
    let uid = h.state().state.logs[0].uid;
    assert_eq!(
        h.state().guide.seen(),
        [(uid, role)],
        "le log compte dans la fiche choisie"
    );
}

fn role_du_fragment(h: &Harness<'static, App>) -> String {
    let fid = h.state().state.logs[0].fragment.clone().expect("rangé");
    h.state().library.fragment(&fid).unwrap().role.clone()
}

/// Le cas qui a fait naître D-090 : la fiche « Jeno 5 CHIRP », des logs dont le nom de machine est « JeNo Ced »,
/// ouverts par « Ouvrir les logs de la série… » du Guide. Ils étaient rangés sous la fiche mais le Guide, qui ne lisait
/// que le nom de machine, les montrait dans « Autres logs ouverts (autre drone) » et les séries restaient vides.
/// Maintenant la fenêtre demande (fiche du Guide proposée d'office) et, une fois rattaché, le log compte dans la fiche
/// avec le rôle demandé.
///
/// **Contre-preuve vue en échec** (2026-11-28) : `card_logs` rétabli sur `card_for_log(…, craft_name)`, le Guide ne
/// voit aucun log et `guide_voit` attend jusqu'à l'échec.
#[test]
fn un_log_d_un_autre_nom_ouvert_depuis_la_fiche_compte_dans_ses_series() {
    let mut h = banc(guide_sur(&[CHIRP], CHIRP));
    h.run();
    h.state_mut().open_as(vec![copie("JeNo Ced", "chirp")], Some(PID));
    attendre_logs(&mut h, 1);
    let p = &h.state().attach.pending;
    assert_eq!(p.len(), 1, "la fenêtre demande");
    assert_eq!(
        p[0].choice,
        DroneChoice::Existing(CHIRP.into()),
        "fiche du Guide proposée"
    );
    h.get_by_label(
        "Nom de machine « JeNo Ced » : aucune fiche ne porte ce nom. Ouvert depuis la fiche « Jeno 5 CHIRP » du Guide.",
    );
    h.cliquer_label("Rattacher");
    h.run_steps(3);
    assert!(!h.state().attach.is_open());
    assert_eq!(logs_du_drone(&h.state().library, CHIRP), 1);
    assert_eq!(role_du_fragment(&h), PID, "le rôle du bouton est gardé");
    guide_voit(&mut h, PID);
}

/// Ouvert depuis le Guide, un log **demande toujours**, même quand son nom de machine est celui de la fiche ; une fois
/// rangé sous ce drone, le rouvrir depuis la même fiche ne redemande rien (il n'y a rien à décider).
///
/// **Contre-preuve vue en échec** (2026-11-28) : `guide_opening` qui rend toujours `Ask`, la réouverture redemande et
/// l'assertion `!is_open` rougit.
#[test]
fn ouvert_depuis_le_guide_un_log_demande_meme_au_nom_de_la_fiche() {
    let mut h = banc(guide_sur(&[CHIRP], CHIRP));
    h.run();
    let chemin = copie(CHIRP, "meme-nom");
    h.state_mut().open_as(vec![chemin.clone()], Some(FILTERS));
    attendre_logs(&mut h, 1);
    assert!(h.state().attach.is_open(), "toujours demander depuis le Guide");
    assert!(h.state().library.logs().is_empty(), "rien de rangé avant le choix");
    h.get_by_label(
        "Nom de machine « Jeno 5 CHIRP » : c'est celui du drone « Jeno 5 CHIRP ». Ouvert depuis la fiche « Jeno 5 CHIRP » du Guide.",
    );
    h.cliquer_label("Rattacher");
    h.run_steps(3);
    guide_voit(&mut h, FILTERS);
    // rouvert depuis la même fiche : déjà rangé sous son drone, rien à demander
    h.state_mut().state.clear();
    h.state_mut().open_as(vec![chemin], Some(PID));
    attendre_logs(&mut h, 1);
    assert!(!h.state().attach.is_open());
    assert_eq!(role_du_fragment(&h), PID, "le nouveau rôle est pris");
}

/// Le nom de machine désigne un **autre** drone qui a sa fiche : la fenêtre propose la fiche du Guide et dit à qui
/// est ce nom ; le pilote tranche. Rattaché à l'autre drone, le log n'est pas compté dans la fiche du Guide.
#[test]
fn un_nom_d_un_autre_drone_laisse_le_pilote_trancher() {
    let mut h = banc(guide_sur(&[CHIRP, "JeNo Ced"], CHIRP));
    h.run();
    h.state_mut().open_as(vec![copie("JeNo Ced", "conflit")], Some(PID));
    attendre_logs(&mut h, 1);
    assert_eq!(h.state().attach.pending[0].choice, DroneChoice::Existing(CHIRP.into()));
    h.get_by_label("Nom de machine « JeNo Ced » : c'est celui du drone « JeNo Ced ». Ouvert depuis la fiche « Jeno 5 CHIRP » du Guide.");
    h.state_mut().attach.pending[0].choice = DroneChoice::Existing("JeNo Ced".into());
    h.cliquer_label("Rattacher");
    h.run_steps(3);
    assert_eq!(logs_du_drone(&h.state().library, "JeNo Ced"), 1);
    assert_eq!(
        h.state().guide.card().unwrap().name,
        CHIRP,
        "la fiche choisie ne change pas"
    );
    assert!(h.state().guide.seen().is_empty(), "pas compté dans la fiche du Guide");
    h.get_by_label("Autres logs ouverts (autre drone) : conflit.");
}

/// Un log **déjà rangé** sous un autre drone (ici par son nom de machine, comme une copie USB l'est sous le drone
/// choisi au branchement), ouvert depuis une autre fiche : la fenêtre le dit et propose de l'y laisser ; « Rattacher »
/// le déplace, fragments compris, et le Guide le compte alors.
///
/// **Contre-preuve vue en échec** (2026-11-28) : sans `move_log` dans `App::store_log`, le log reste sous « JeNo Ced »
/// et l'assertion sur `logs_du_drone(…, CHIRP)` rougit.
#[test]
fn un_log_range_ailleurs_se_laisse_ou_se_deplace() {
    let mut h = banc(guide_sur(&[CHIRP, "JeNo Ced"], CHIRP));
    h.run();
    let chemin = copie("JeNo Ced", "ailleurs");
    h.state_mut().open(vec![chemin.clone()]);
    attendre_logs(&mut h, 1);
    assert!(!h.state().attach.is_open(), "reconnu par son nom, rangé sans question");
    assert_eq!(logs_du_drone(&h.state().library, "JeNo Ced"), 1);
    // depuis la fiche « Jeno 5 CHIRP » : on demande, et « Laisser sous… » le garde où il est
    h.state_mut().state.clear();
    h.state_mut().guide.select_by_name(CHIRP);
    h.state_mut().open_as(vec![chemin.clone()], Some(PID));
    attendre_logs(&mut h, 1);
    assert_eq!(h.state().attach.pending[0].known.as_deref(), Some("JeNo Ced"));
    h.get_by_label(
        "Déjà rangé sous « JeNo Ced ». « Rattacher » le déplace sous le drone choisi, avec ses fragments. Ouvert depuis la fiche « Jeno 5 CHIRP » du Guide.",
    );
    h.cliquer_label("Laisser sous « JeNo Ced »");
    h.run_steps(3);
    assert!(!h.state().attach.is_open());
    assert_eq!(logs_du_drone(&h.state().library, "JeNo Ced"), 1);
    assert!(
        h.state().state.logs[0].fragment.is_some(),
        "toujours rangé, son fragment revient"
    );
    // rouvert depuis la fiche, cette fois déplacé
    h.state_mut().state.clear();
    h.state_mut().guide.select_by_name(CHIRP);
    h.state_mut().open_as(vec![chemin], Some(PID));
    attendre_logs(&mut h, 1);
    h.cliquer_label("Rattacher");
    h.run_steps(3);
    assert_eq!(logs_du_drone(&h.state().library, CHIRP), 1);
    assert_eq!(logs_du_drone(&h.state().library, "JeNo Ced"), 0);
    assert_eq!(
        h.state().library.fragments().len(),
        1,
        "le même fragment, pas un double"
    );
    guide_voit(&mut h, PID);
}

/// Un log ouvert et rangé sous aucun drone n'est pas « d'un autre drone » : le panneau des logs du Guide le dit et
/// son lien « Rattacher à un drone… » rouvre la fenêtre, la fiche du Guide proposée d'office.
#[test]
fn un_log_non_range_se_rattache_depuis_le_panneau_du_guide() {
    let mut h = banc(guide_sur(&[CHIRP], CHIRP));
    h.run();
    h.state_mut().open(vec![copie("JeNo Ced", "non-range")]);
    attendre_logs(&mut h, 1);
    h.cliquer_label("Ne pas ranger");
    h.run_steps(3);
    assert!(!h.state().attach.is_open());
    h.get_by_label("Ouverts, rangés sous aucun drone (le Guide ne les compte pas) :");
    h.cliquer_label("Rattacher à un drone…");
    h.run_steps(2);
    let p = &h.state().attach.pending;
    assert_eq!(p.len(), 1);
    assert_eq!(p[0].choice, DroneChoice::Existing(CHIRP.into()));
    h.cliquer_label("Rattacher");
    h.run_steps(3);
    attendre(&mut h, |a| !a.guide.busy() && a.guide.seen().len() == 1);
    assert!(h.state().attach.pending.is_empty());
}

/// Le nom d'un drone se compare sans la casse ni les espaces autour (D-090), comme le Guide le faisait.
#[test]
fn le_nom_d_un_drone_se_compare_sans_casse_ni_espaces() {
    let mut lib = Library::in_memory();
    lib.put_drone(Drone::new("uid-1", "JeNo Ced"));
    assert_eq!(lib.drone_by_name("  jeno ced ").map(|d| d.id.as_str()), Some("uid-1"));
    assert!(lib.drone_by_name("JeNo").is_none());
}
