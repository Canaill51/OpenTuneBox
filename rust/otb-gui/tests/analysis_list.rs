//! La liste d'analyse à l'écran (lot 29c, D-066) : la barre de pastilles du haut, le « + » d'un fragment au
//! tableau de bord, et la liste retrouvée au lancement suivant.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Pos2, Rect, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use otb_gui::analysis_bar::{drop_index, master_label, remove_label};
use otb_gui::app::{App, Screen};
use otb_store::Library;

fn banc(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1400.0, 700.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app)
}

fn dossier_temp(nom: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("otb-liste-{nom}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un fichier à **deux sessions** : le vrai log de 5 s mis bout à bout deux fois (c'est ainsi qu'une carte
/// enregistre plusieurs vols dans un même fichier).
fn deux_sessions(dir: &Path) -> PathBuf {
    let un = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")).unwrap();
    let p = dir.join("deux.bbl");
    std::fs::write(&p, [un.clone(), un].concat()).unwrap();
    p
}

/// ⚠️ Au pas, jamais `run()` pendant une ouverture (L-077).
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
    // laisser finir le paquet en cours (un fichier de trop arriverait après)
    for _ in 0..40 {
        h.step();
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(3);
}

fn noms(h: &Harness<'static, App>) -> Vec<String> {
    h.state().chip_infos().into_iter().map(|c| c.name).collect()
}

fn toggled(h: &Harness<'static, App>, label: &str) -> bool {
    h.get_by_role_and_label(egui::accesskit::Role::CheckBox, label)
        .accesskit_node()
        .toggled()
        == Some(egui::accesskit::Toggled::True)
}

/// Les gestes de la barre : désactiver et réactiver une pastille, choisir le maître, retirer un fragment.
///
/// **Contre-preuves vues en échec** : `BarAction::Toggle` sans effet (la pastille reste active), `BarAction::Master`
/// qui ne change pas le log courant, `BarAction::Remove` sans effet.
#[test]
fn la_barre_montre_une_pastille_par_fragment_et_ses_gestes() {
    let d = dossier_temp("gestes");
    let mut h = banc(App::new(otb_gui::tools::all_pages()));
    h.state_mut().screen = Screen::Dashboard;
    h.state_mut().open(vec![deux_sessions(&d)]);
    attendre_logs(&mut h, 2);
    let n = noms(&h);
    assert_eq!(n.len(), 2);
    assert_ne!(n[0], n[1], "une pastille par session, chacune son nom");
    // le drone est écrit sur la pastille
    assert_eq!(h.state().chip_infos()[0].drone, "JeNo Ced");
    assert!(toggled(&h, &n[0]) && toggled(&h, &n[1]), "toutes actives à l'ouverture");
    // désactiver puis réactiver, sans fermer
    h.get_by_role_and_label(egui::accesskit::Role::CheckBox, &n[0]).click();
    h.run_steps(2);
    assert!(!h.state().state.logs[0].active);
    assert!(!toggled(&h, &n[0]));
    assert_eq!(h.state().state.logs.len(), 2, "désactiver ne ferme pas");
    h.get_by_role_and_label(egui::accesskit::Role::CheckBox, &n[0]).click();
    h.run_steps(2);
    assert!(h.state().state.logs[0].active);
    // le maître : le dernier ouvert ; un clic sur le rond du premier le désigne
    assert_eq!(h.state().state.master(), Some(1));
    h.get_by_label(&master_label(&n[0], false)).click();
    h.run_steps(2);
    assert_eq!(h.state().state.master(), Some(0));
    h.get_by_label(&master_label(&n[0], true));
    // retirer le second : il est fermé
    h.get_by_label(&remove_label(&n[1])).click();
    h.run_steps(2);
    assert_eq!(h.state().state.logs.len(), 1);
    assert_eq!(noms(&h), vec![n[0].clone()]);
}

#[test]
fn sans_fragment_la_barre_invite_a_en_ajouter() {
    let mut h = banc(App::new(otb_gui::tools::all_pages()));
    h.state_mut().screen = Screen::Dashboard;
    h.run();
    h.get_by_label("Aucun fragment — ajoutez-en avec le rond + d'un drone");
    // les deux boutons gardés à droite de la barre (écarts écrits, D-066) et le thème de la maquette
    h.get_by_label("Ouvrir…");
    h.get_by_label("Exporter la fenêtre en image PNG");
    h.get_by_label("Thème sombre").click();
    h.run_steps(2);
    assert!(h.state().theme.dark);
    h.get_by_label("Thème clair");
}

/// Le « + » d'un fragment au tableau de bord n'ouvre **que sa session**, le même rond le retire ensuite.
///
/// **Contre-preuve vue en échec** : sans le filtre des sessions voulues (`App::accepte`), les deux sessions du
/// fichier s'ouvrent et le test compte 2 logs au lieu d'un.
#[test]
fn le_rond_plus_d_un_fragment_n_ouvre_que_sa_session_puis_la_retire() {
    let d = dossier_temp("rond");
    let fichier = deux_sessions(&d);
    let mut h = banc(App::new(otb_gui::tools::all_pages()));
    h.state_mut().screen = Screen::Dashboard;
    // une première ouverture range le fichier et ses deux fragments dans la bibliothèque
    h.state_mut().open(vec![fichier]);
    attendre_logs(&mut h, 2);
    h.state_mut().state.clear();
    h.run_steps(2);
    let second = h.state().library.fragments()[1].clone();
    h.get_by_label("JeNo Ced").click();
    h.run_steps(2);
    h.get_by_label(&format!("Ajouter {} à l'analyse", second.name)).click();
    attendre_logs(&mut h, 1);
    assert_eq!(h.state().state.logs.len(), 1, "seulement la session du fragment");
    assert_eq!(h.state().state.logs[0].fragment.as_deref(), Some(second.id.as_str()));
    // le même rond, devenu coche à la couleur de la pastille, le retire ; le « × » de la pastille porte le même
    // libellé (même geste) : le rond du tableau de bord est dessiné après la barre, donc le dernier
    let retirer = format!("Retirer {} de l'analyse", second.name);
    assert_eq!(
        h.query_all_by_label(&retirer).count(),
        2,
        "le « × » de la pastille et le rond"
    );
    h.query_all_by_label(&retirer).last().unwrap().click();
    h.run_steps(2);
    assert!(h.state().state.logs.is_empty());
}

/// La liste se retrouve au lancement suivant : même ordre, mêmes couleurs, même état des pastilles, même maître.
///
/// **Contre-preuve vue en échec** : sans `finish_restore`, les logs rouverts reviennent dans l'ordre du fichier et
/// l'ordre comparé diffère.
#[test]
fn la_liste_d_analyse_est_retrouvee_au_lancement() {
    let d = dossier_temp("relance");
    let fichier = deux_sessions(&d);
    let lib = d.join("bibliotheque");
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library = Library::open(lib.clone());
    let mut h = banc(app);
    h.state_mut().open(vec![fichier]);
    attendre_logs(&mut h, 2);
    // la seconde session en tête, la première désactivée, la seconde maître
    h.state_mut().state.move_log(1, 0);
    h.state_mut().state.set_active(1, false);
    h.state_mut().state.set_current(Some(0));
    h.run_steps(2);
    let avant: Vec<_> = h
        .state()
        .state
        .logs
        .iter()
        .map(|l| (l.fragment.clone(), l.color, l.active))
        .collect();
    let maitre = h.state().state.current_log().unwrap().fragment.clone();
    h.state_mut().library.save().unwrap();
    drop(h);

    let mut app = App::new(otb_gui::tools::all_pages());
    app.library = Library::open(lib);
    app.restore_analysis();
    let mut h = banc(app);
    attendre_logs(&mut h, 2);
    let apres: Vec<_> = h
        .state()
        .state
        .logs
        .iter()
        .map(|l| (l.fragment.clone(), l.color, l.active))
        .collect();
    assert_eq!(apres, avant, "ordre, couleurs et pastilles désactivées retrouvés");
    assert_eq!(h.state().state.current_log().unwrap().fragment, maitre);
}

/// Un fragment de la liste dont le fichier a disparu est signalé, pas ouvert.
#[test]
fn un_fragment_introuvable_au_lancement_est_signale() {
    use otb_store::model::{Drone, Fragment, LogEntry};
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library.put_drone(Drone::new("JeNo Ced", "JeNo Ced"));
    app.library.add_log(LogEntry {
        id: "emp".into(),
        drone_id: "JeNo Ced".into(),
        name: "parti.bbl".into(),
        path: "/nulle/part/parti.bbl".into(),
        copied: false,
        bytes: 1,
        added: "2026-11-23".into(),
    });
    app.library.put_fragment(Fragment {
        id: "emp-1".into(),
        log_id: "emp".into(),
        session: 1,
        start: 0,
        end: 1,
        name: "PARTI".into(),
        role: String::new(),
        date: "2026-11-23".into(),
        chosen: false,
    });
    app.library.add_to_analysis("emp-1");
    app.restore_analysis();
    let mut h = banc(app);
    h.run_steps(3);
    assert!(h.state().status.contains("parti.bbl"), "{}", h.state().status);
    assert!(h.state().state.logs.is_empty());
}

/// Où tombe une pastille lâchée : fonction pure, le glisser à la souris simulée se teste mal.
#[test]
fn une_pastille_lachee_tombe_a_sa_place() {
    // deux lignes : trois pastilles de 100 points, puis une
    let r = |x: f32, y: f32| Rect::from_min_size(Pos2::new(x, y), Vec2::new(100.0, 30.0));
    let rects = [r(0.0, 0.0), r(108.0, 0.0), r(216.0, 0.0), r(0.0, 38.0)];
    assert_eq!(
        drop_index(&rects, 0, Pos2::new(300.0, 15.0)),
        2,
        "au bout de la 1re ligne"
    );
    assert_eq!(drop_index(&rects, 2, Pos2::new(5.0, 15.0)), 0, "en tête");
    assert_eq!(
        drop_index(&rects, 0, Pos2::new(160.0, 15.0)),
        1,
        "après le milieu de la 2e"
    );
    assert_eq!(
        drop_index(&rects, 0, Pos2::new(150.0, 50.0)),
        3,
        "sur la 2e ligne, après la dernière"
    );
    assert_eq!(
        drop_index(&rects, 3, Pos2::new(150.0, 15.0)),
        1,
        "remontée sur la 1re ligne"
    );
}

/// Un même fichier ouvert deux fois : deux pastilles, mais un seul fragment dans la liste enregistrée.
///
/// **Contre-preuve vue en échec** : sans le filtre des doublons de `App::sync_analysis`, la liste en garde deux.
#[test]
fn un_meme_fragment_ouvert_deux_fois_n_est_garde_qu_une_fois() {
    let un = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    let mut h = banc(App::new(otb_gui::tools::all_pages()));
    h.state_mut().open(vec![un.clone(), un]);
    attendre_logs(&mut h, 2);
    assert_eq!(h.state().state.logs.len(), 2);
    assert_eq!(h.state().library.analysis().items.len(), 1);
}

// ---------------------------------------------------------------- second fragment d'une session (lot 29, D-067)

/// Le vrai log de 5 s ouvert, puis l'écran d'extraction sur son fragment, brouillon prêt.
fn extraction_du_vrai_log(app: App) -> Harness<'static, App> {
    let un = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    let mut h = banc(app);
    h.state_mut().screen = Screen::Dashboard;
    h.state_mut().open(vec![un]);
    attendre_logs(&mut h, 1);
    h.state_mut().state.set_current(Some(0));
    h.state_mut().enter_extraction();
    h.run_steps(3);
    h
}

/// Règle le brouillon puis clique « Enregistrer comme nouveau fragment ».
fn nouveau_fragment(h: &mut Harness<'static, App>, bornes: (usize, usize), nom: &str) {
    let log = h.state().state.current_log().unwrap().log.clone();
    let brouillon = h.state_mut().viewer.draft.as_mut().expect("brouillon");
    brouillon.period = (log.t[bornes.0], log.t[bornes.1]);
    brouillon.name = nom.into();
    h.get_by_label("Enregistrer comme nouveau fragment").click();
    h.run_steps(3);
}

/// « Enregistrer comme nouveau fragment » laisse le fragment d'origine tel quel et en range un second dans la même
/// session : pastille juste après l'origine, sur les **mêmes échantillons** (pas de relecture), bornes choisies,
/// devenu maître ; un second nom identique est distingué.
///
/// **Contre-preuves vues en échec** : `save_new_fragment` sans le nom distinct (deux « Rafale ») ; sans
/// `move_log` après la pose (la pastille du second ajout n'est pas juste après son origine).
#[test]
fn enregistrer_comme_nouveau_fragment_garde_l_origine_et_ajoute_une_pastille() {
    let mut h = extraction_du_vrai_log(App::new(otb_gui::tools::all_pages()));
    let origine = h.state().state.logs[0]
        .fragment
        .clone()
        .expect("rangé sous « JeNo Ced »");
    let avant = h.state().library.fragment(&origine).unwrap().clone();
    let periode = h.state().state.logs[0].period;
    nouveau_fragment(&mut h, (400, 6000), "Rafale");

    assert_eq!(h.state().screen, Screen::Dashboard, "retour à l'écran d'où l'on venait");
    let logs = &h.state().state.logs;
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].period, periode, "la période de l'origine n'a pas bougé");
    assert_eq!(
        h.state().library.fragment(&origine),
        Some(&avant),
        "l'origine n'a pas bougé"
    );
    assert!(
        std::sync::Arc::ptr_eq(&logs[0].log, &logs[1].log),
        "mêmes échantillons, fichier non relu"
    );
    assert_ne!(logs[0].color, logs[1].color);
    assert_eq!(logs[1].period, (logs[1].log.t[400], logs[1].log.t[6000]));
    assert_eq!(h.state().state.current, Some(1), "le nouveau fragment est le maître");
    let id = logs[1].fragment.clone().unwrap();
    assert_eq!(id, format!("{origine}-2"));
    let f = h.state().library.fragment(&id).unwrap();
    assert!(f.chosen);
    assert_eq!((f.start, f.end, f.session), (400, 6001, avant.session));
    assert_eq!(f.name, "Rafale");
    assert_eq!(noms(&h), [avant.name.clone(), "Rafale".to_string()]);

    // depuis l'origine encore : le troisième se pose juste après elle, avec un nom distinct
    h.state_mut().state.set_current(Some(0));
    h.state_mut().enter_extraction();
    h.run_steps(3);
    nouveau_fragment(&mut h, (100, 2000), "Rafale");
    assert_eq!(
        noms(&h),
        [avant.name.clone(), "Rafale (2)".to_string(), "Rafale".to_string()]
    );
    assert_eq!(h.state().state.logs[1].fragment, Some(format!("{origine}-3")));
    assert_eq!(
        h.state().library.analysis().items.len(),
        3,
        "trois fragments dans la liste enregistrée"
    );
}

/// Un fragment ajouté se retrouve comme les autres : par son rond « + » (sur la session déjà ouverte, sans relire le
/// fichier ; ou en n'ouvrant que lui), par son crayon, et dans la liste relue au lancement.
///
/// **Contre-preuves vues en échec** : `accepte` qui ne cherche que le fragment de la session (le « + » d'un fragment
/// ajouté n'ouvre rien) ; `toggle_fragment` sans `loaded_session` (le fichier est relu, échantillons non partagés) ;
/// `edit_fragment` sans sa branche « fragment ajouté » (le crayon ouvre la session et n'entre pas dans l'écran).
#[test]
fn un_fragment_ajoute_se_rouvre_par_son_rond_son_crayon_et_au_lancement() {
    let d = dossier_temp("ajoute");
    let lib = d.join("bibliotheque");
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library = Library::open(lib.clone());
    let mut h = extraction_du_vrai_log(app);
    nouveau_fragment(&mut h, (400, 6000), "Rafale");
    let id = h.state().state.logs[1].fragment.clone().unwrap();
    let periode = h.state().state.logs[1].period;

    // rond « + » : session déjà ouverte sous l'origine → posé tout de suite, mêmes échantillons
    h.state_mut().state.remove(1);
    h.run_steps(2);
    h.get_by_label("JeNo Ced").click();
    h.run_steps(2);
    h.get_by_label("Ajouter Rafale à l'analyse").click();
    h.run_steps(2);
    assert_eq!(h.state().state.logs.len(), 2, "posé sans attendre une ouverture");
    assert!(std::sync::Arc::ptr_eq(
        &h.state().state.logs[0].log,
        &h.state().state.logs[1].log
    ));
    assert_eq!(h.state().state.logs[1].fragment.as_deref(), Some(id.as_str()));
    assert_eq!(h.state().state.logs[1].period, periode, "ses bornes choisies");

    // rond « + » sans rien d'ouvert : seulement lui
    h.state_mut().state.clear();
    h.run_steps(2);
    h.get_by_label("Ajouter Rafale à l'analyse").click();
    attendre_logs(&mut h, 1);
    assert_eq!(h.state().state.logs.len(), 1, "seulement le fragment demandé");
    assert_eq!(h.state().state.logs[0].fragment.as_deref(), Some(id.as_str()));
    assert_eq!(h.state().state.logs[0].period, periode);

    // crayon sans rien d'ouvert : l'écran d'extraction sur lui
    h.state_mut().state.clear();
    h.run_steps(2);
    h.get_by_label("Modifier les limites de Rafale").click();
    attendre_logs(&mut h, 1);
    assert_eq!(h.state().screen, Screen::Viewer);
    assert_eq!(
        h.state().state.current_log().unwrap().fragment.as_deref(),
        Some(id.as_str())
    );
    h.get_by_label("Annuler").click();
    h.run_steps(2);

    // liste enregistrée, relue au lancement : l'origine et lui
    let origine = otb_gui::app::fragment_id(
        &h.state().library.fragment(&id).unwrap().log_id.clone(),
        &h.state().state.logs[0].log,
    );
    let nom = h.state().library.fragment(&origine).unwrap().name.clone();
    h.get_by_label(&format!("Ajouter {nom} à l'analyse")).click();
    attendre_logs(&mut h, 2);
    h.state_mut().library.save().unwrap();
    let avant: Vec<_> = h
        .state()
        .state
        .logs
        .iter()
        .map(|l| (l.fragment.clone(), l.period))
        .collect();
    assert_eq!(avant.len(), 2);
    assert!(avant.iter().any(|(f, _)| f.as_deref() == Some(origine.as_str())));
    drop(h);
    let mut app = App::new(otb_gui::tools::all_pages());
    app.library = Library::open(lib);
    app.restore_analysis();
    let mut h = banc(app);
    attendre_logs(&mut h, 2);
    let apres: Vec<_> = h
        .state()
        .state
        .logs
        .iter()
        .map(|l| (l.fragment.clone(), l.period))
        .collect();
    assert_eq!(apres, avant, "les deux fragments de la session, avec leurs bornes");
}

/// Un fragment ajouté se supprime, en deux temps ; celui de la session n'en a pas le bouton (il reviendrait à la
/// prochaine ouverture du fichier).
///
/// **Contre-preuve vue en échec** : `FragmentInfo::added` toujours vrai (le fragment de la session montre le
/// bouton).
#[test]
fn seul_un_fragment_ajoute_se_supprime_apres_confirmation() {
    let mut h = extraction_du_vrai_log(App::new(otb_gui::tools::all_pages()));
    assert!(
        h.query_by_label("Supprimer ce fragment…").is_none(),
        "le fragment de la session ne se supprime pas"
    );
    nouveau_fragment(&mut h, (400, 6000), "Rafale");
    let id = h.state().state.logs[1].fragment.clone().unwrap();
    h.state_mut().enter_extraction();
    h.run_steps(3);
    h.get_by_label("Supprimer ce fragment…").click();
    h.run_steps(2);
    assert!(h.state().library.fragment(&id).is_some(), "rien avant la confirmation");
    h.get_by_label("Garder").click();
    h.run_steps(2);
    assert!(h.query_by_label("Supprimer définitivement").is_none());
    h.get_by_label("Supprimer ce fragment…").click();
    h.run_steps(2);
    h.get_by_label("Supprimer définitivement").click();
    h.run_steps(3);
    assert_eq!(h.state().screen, Screen::Dashboard);
    assert!(h.state().library.fragment(&id).is_none());
    assert_eq!(h.state().state.logs.len(), 1, "sa pastille est partie, l'origine reste");
    assert!(h.state().library.analysis().items.iter().all(|i| i.fragment_id != id));
}
