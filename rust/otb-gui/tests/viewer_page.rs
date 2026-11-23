//! L'écran d'extraction (le visualiseur, lot 29b) sur un vrai log de 5 s (`tests/data/jeno5_5s.bbl`) : bornes du
//! fragment déplaçables depuis n'importe quel graphique et appliquées seulement à l'enregistrement, curseur, barre de
//! navigation, bascule vers l'écran à l'ouverture (port de `test_gui_viewer.py`).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::Pos2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_gui::app::{App, Screen};

fn banc() -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1400.0, 800.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), App::new(Vec::new()));
    let log = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    h.state_mut().open(vec![log]);
    let debut = Instant::now();
    while h.state().state.logs.is_empty() {
        h.run();
        assert!(debut.elapsed() < Duration::from_secs(30), "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(5));
    }
    for _ in 0..4 {
        h.run();
    }
    h
}

fn px(h: &Harness<'static, App>, ax: usize, t: f64) -> f32 {
    h.state().viewer.transforms[ax]
        .expect("graphique dessiné")
        .position_from_point_x(t)
}

fn centre_y(h: &Harness<'static, App>, ax: usize) -> f32 {
    h.state().viewer.transforms[ax].unwrap().frame().center().y
}

#[test]
fn le_premier_log_ouvert_fait_passer_aux_courbes() {
    let h = banc();
    assert_eq!(h.state().screen, Screen::Viewer);
    assert!(
        h.state().viewer.transforms.iter().all(Option::is_some),
        "les quatre graphiques sont dessinés"
    );
}

/// ⚠️ Les bornes du fragment se tirent depuis n'importe quel graphique, mais rien n'atteint les outils avant
/// « Enregistrer le fragment » : ni pendant le glissement, ni au relâchement (lot 29b).
///
/// **Contre-preuve** : appliquer le brouillon à l'état au relâchement (`state.set_period` dans `plots`) rougit
/// l'assertion « rien d'appliqué au relâchement ».
#[test]
fn les_bornes_se_tirent_depuis_n_importe_quel_graphique_sans_rien_appliquer() {
    for ax in [0usize, 1, 2, 3] {
        let mut h = banc();
        let periode = h.state().state.logs[0].period;
        let (p0, p1) = h.state().viewer.draft.as_ref().expect("brouillon").period;
        // le curseur (prioritaire) est écarté de la borne
        h.state_mut().state.set_cursor(p0 + (p1 - p0) * 0.8);
        h.run();
        let rev = h.state().state.revision();
        let y = centre_y(&h, ax);
        let x_depart = px(&h, ax, p0);
        let cible = p0 + (p1 - p0) * 0.3;
        h.hover_at(Pos2::new(x_depart, y));
        h.run();
        h.drag_at(Pos2::new(x_depart, y));
        h.run();
        h.hover_at(Pos2::new(px(&h, ax, cible), y));
        h.run();
        let (e0, e1) = h.state().viewer.draft.as_ref().unwrap().period;
        assert!((e0 - cible).abs() < 0.2, "graphique {ax} : borne à {e0}, cible {cible}");
        assert_eq!(e1, p1, "graphique {ax} : la fin n'a pas bougé");
        h.drop_at(Pos2::new(px(&h, ax, cible), y));
        h.run();
        h.run();
        assert_eq!(
            h.state().state.logs[0].period,
            periode,
            "graphique {ax} : rien d'appliqué au relâchement"
        );
        assert_eq!(h.state().state.revision(), rev, "graphique {ax} : aucun calcul relancé");
        let log = h.state().state.logs[0].log.clone();
        let (d0, _) = h.state().viewer.draft.as_ref().unwrap().period;
        assert_eq!(
            log.t[log.index_of(d0)],
            d0,
            "graphique {ax} : la borne est posée sur un échantillon"
        );
    }
}

/// « Enregistrer le fragment » : les bornes deviennent la période d'analyse (une seule fois), la bibliothèque les
/// garde comme choisies, et l'écran se referme. « Annuler » ne change rien.
///
/// **Contre-preuve** : sans `f.chosen = true` dans `App::save_fragment`, l'assertion sur la bibliothèque rougit ;
/// sans `self.state.set_period` dans la même fonction, celle sur la période rougit.
#[test]
fn enregistrer_applique_les_bornes_et_annuler_ne_change_rien() {
    let mut h = banc();
    h.state_mut().return_to = Screen::Dashboard;
    let periode = h.state().state.logs[0].period;
    let rev = h.state().state.revision();
    let log = h.state().state.logs[0].log.clone();
    let cible = (log.t[400], log.t[6000]);
    // Annuler : le brouillon est jeté
    h.state_mut().viewer.draft.as_mut().unwrap().period = cible;
    h.get_by_label("Annuler").click();
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Dashboard, "retour à l'écran d'où l'on venait");
    assert_eq!(h.state().state.logs[0].period, periode);
    assert_eq!(h.state().state.revision(), rev);
    // Enregistrer
    h.state_mut().enter_extraction();
    h.run_steps(2);
    let brouillon = h.state_mut().viewer.draft.as_mut().unwrap();
    brouillon.period = cible;
    brouillon.name = "Vol de réglage".into();
    brouillon.role = otb_advisor::log_check::FILTERS.into();
    h.get_by_label("Enregistrer le fragment").click();
    h.run_steps(2);
    assert_eq!(h.state().screen, Screen::Dashboard);
    assert_eq!(
        h.state().state.logs[0].period,
        cible,
        "la période d'analyse est celle du fragment"
    );
    assert_eq!(h.state().state.revision(), rev + 1, "appliquée en une seule fois");
    let id = h.state().state.logs[0]
        .fragment
        .clone()
        .expect("log rangé sous « JeNo Ced »");
    let f = h.state().library.fragment(&id).expect("fragment rangé").clone();
    assert!(f.chosen, "bornes choisies");
    assert_eq!((f.start, f.end), (400, 6001), "bornes en échantillons, fin exclue");
    assert_eq!(f.name, "Vol de réglage");
    assert_eq!(f.role, otb_advisor::log_check::FILTERS);
    let chemin = log.path.to_string_lossy().into_owned();
    assert_eq!(
        h.state().guide.roles.get(&chemin).map(String::as_str),
        Some(otb_advisor::log_check::FILTERS),
        "le Guide connaît le rôle du fichier"
    );
}

#[test]
fn le_curseur_se_deplace_sans_changer_la_periode() {
    let mut h = banc();
    let log = h.state().state.logs[0].log.clone();
    let c0 = h.state().state.cursor;
    assert_eq!(c0, log.t[0]);
    let rev = h.state().state.revision();
    let y = centre_y(&h, 1);
    let depart = px(&h, 1, c0);
    let cible = log.t[0] + 2.5;
    h.hover_at(Pos2::new(depart, y));
    h.run();
    h.drag_at(Pos2::new(depart, y));
    h.run();
    h.hover_at(Pos2::new(px(&h, 1, cible), y));
    h.run();
    h.drop_at(Pos2::new(px(&h, 1, cible), y));
    h.run();
    h.run();
    assert!(
        (h.state().state.cursor - cible).abs() < 0.2,
        "curseur à {}",
        h.state().state.cursor
    );
    assert_eq!(h.state().state.revision(), rev, "le curseur ne relance aucun calcul");
}

#[test]
fn un_seul_axe_en_vue_focalisee() {
    let mut h = banc();
    h.state_mut().viewer.focus = 2;
    h.state_mut().viewer.transforms = [None; 4]; // on ne garde que ce qui est dessiné après le changement
    h.run();
    h.run();
    h.run();
    // Pitch occupe toute la hauteur au-dessus des moteurs ; les deux autres axes ne sont plus dessinés
    let (pitch, moteurs) = (
        h.state().viewer.transforms[1].unwrap(),
        h.state().viewer.transforms[3].unwrap(),
    );
    assert!(pitch.frame().height() > 2.0 * moteurs.frame().height());
    let t = &h.state().viewer.transforms;
    assert!(t[0].is_none() && t[2].is_none(), "Roll et Yaw ne sont plus dessinés");
}

/// Un brouillon appartient à un log : changer de log le jette, et il n'est appliqué à aucun.
#[test]
fn changer_de_log_abandonne_le_brouillon() {
    let mut h = banc();
    let log = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    h.state_mut().open(vec![log]);
    let debut = Instant::now();
    while h.state().state.logs.len() < 2 {
        h.run();
        assert!(debut.elapsed() < Duration::from_secs(30), "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run();
    let avant: Vec<_> = h.state().state.logs.iter().map(|l| l.period).collect();
    let uid1 = h.state().state.logs[1].uid;
    assert_eq!(
        h.state().viewer.draft.as_ref().unwrap().uid,
        uid1,
        "le second log est affiché"
    );
    h.state_mut().viewer.draft.as_mut().unwrap().period = (1.0, 2.0);
    h.state_mut().state.set_current(Some(0));
    h.run();
    h.run();
    let d = h.state().viewer.draft.clone().unwrap();
    assert_eq!(
        d.uid,
        h.state().state.logs[0].uid,
        "le brouillon est celui du log affiché"
    );
    assert_ne!(d.period, (1.0, 2.0), "le brouillon d'un autre log ne doit pas survivre");
    let apres: Vec<_> = h.state().state.logs.iter().map(|l| l.period).collect();
    assert_eq!(avant, apres, "et n'est appliqué à aucun log");
}

/// La barre de navigation : tirer le cadre déplace la fenêtre visible des graphiques, sans changer sa largeur.
///
/// **Contre-preuve** : sans `self.window = Some(…)` du mode `Pan`, la fenêtre ne bouge pas et le test rougit.
#[test]
fn la_barre_de_navigation_deplace_la_fenetre_visible() {
    let mut h = banc();
    let log = h.state().state.logs[0].log.clone();
    let (lo, hi) = (log.t[0], *log.t.last().unwrap());
    // fenêtre réduite au premier cinquième du log
    h.state_mut().viewer.window = Some((lo, lo + (hi - lo) / 5.0));
    h.run_steps(3);
    let (w0, w1) = h.state().viewer.visible().expect("graphiques dessinés");
    assert!(
        (w0 - lo).abs() < 1e-6 && (w1 - (lo + (hi - lo) / 5.0)).abs() < 1e-6,
        "{w0}–{w1}"
    );
    let nav = h.state().viewer.nav_rect.expect("barre dessinée");
    let x_of = |t: f64| nav.left() + ((t - lo) / (hi - lo)) as f32 * nav.width();
    let y = nav.center().y + 12.0; // sous la poignée, dans le cadre
    let depart = Pos2::new(x_of((w0 + w1) / 2.0), y);
    let arrivee = Pos2::new(depart.x + nav.width() / 2.0, y);
    h.hover_at(depart);
    h.run();
    h.drag_at(depart);
    h.run();
    h.hover_at(arrivee);
    h.run();
    h.drop_at(arrivee);
    h.run_steps(3);
    let (n0, n1) = h.state().viewer.visible().unwrap();
    assert!(
        (n0 - (w0 + (hi - lo) / 2.0)).abs() < 0.1,
        "fenêtre déplacée de la moitié du log : {n0}"
    );
    assert!(((n1 - n0) - (w1 - w0)).abs() < 1e-6, "même largeur");
}

/// Le centre vertical d'un graphique, et le temps `t` en pixels, pour cliquer dessus.
fn clic(h: &mut Harness<'static, App>, ax: usize, t: f64) {
    let p = Pos2::new(px(h, ax, t), centre_y(h, ax));
    h.hover_at(p);
    h.run();
    h.drag_at(p); // appui puis relâchement au même endroit = un clic
    h.run();
    h.drop_at(p);
    h.run();
    h.run();
}

#[test]
fn la_mesure_entre_deux_points() {
    let mut h = banc();
    let t0 = h.state().state.logs[0].log.t[0];
    // curseur et bords de période écartés des points cliqués
    h.state_mut().state.set_cursor(t0 + 4.5);
    h.run();
    clic(&mut h, 0, t0 + 1.0);
    assert!(
        h.state().viewer.measure.is_empty(),
        "sans le mode mesure, un clic ne fait rien"
    );
    h.get_by_label("Mesurer").click();
    h.run_steps(4); // la mise en page se stabilise avant de viser un pixel
    clic(&mut h, 0, t0 + 1.0);
    clic(&mut h, 2, t0 + 1.5); // 2e point sur un autre axe
    let m = h.state().viewer.measure.clone();
    assert_eq!(m.len(), 2, "{m:?}");
    assert!(((m[1] - m[0]) - 0.5).abs() < 0.1, "points {m:?} (t0 = {t0})");
    // tirer le 2e repère sur Pitch : la mesure suit
    let (y, from, to) = (centre_y(&h, 1), px(&h, 1, m[1]), px(&h, 1, t0 + 2.0));
    h.hover_at(Pos2::new(from, y));
    h.run();
    h.drag_at(Pos2::new(from, y));
    h.run();
    h.hover_at(Pos2::new(to, y));
    h.run();
    h.drop_at(Pos2::new(to, y));
    h.run();
    h.run();
    let m = h.state().viewer.measure.clone();
    assert!((m[1] - (t0 + 2.0)).abs() < 0.1 && m.len() == 2, "{m:?}");
    // 3e clic : nouvelle mesure
    clic(&mut h, 0, t0 + 3.0);
    assert_eq!(h.state().viewer.measure.len(), 1);
    // couper le mode efface la mesure
    h.get_by_label("Mesurer").click();
    h.run_steps(3);
    assert!(!h.state().viewer.measure_on);
    assert!(h.state().viewer.measure.is_empty(), "la mesure survit à la fin du mode");
}

#[test]
fn le_lecteur_avance_s_arrete_a_la_fin_et_se_borne() {
    let mut h = banc();
    let log = h.state().state.logs[0].log.clone();
    let (lo, hi) = (log.t[0], *log.t.last().unwrap());
    let mut v = std::mem::take(&mut h.state_mut().viewer);
    v.seek_rel(&mut h.state_mut().state, Some(-0.1), 0);
    assert_eq!(h.state().state.cursor, lo, "borné au début");
    v.seek_rel(&mut h.state_mut().state, Some(0.1), 0);
    assert!((h.state().state.cursor - (lo + 0.1)).abs() < 1e-9);
    v.seek_rel(&mut h.state_mut().state, Some(1e6), 0);
    assert_eq!(h.state().state.cursor, hi, "borné à la fin");
    // « Début » et « Fin » du lecteur : les bornes du fragment en cours de modification
    let (p0, p1) = v.draft.as_ref().expect("brouillon").period;
    v.seek_rel(&mut h.state_mut().state, None, -1);
    assert_eq!(h.state().state.cursor, p0);
    v.seek_rel(&mut h.state_mut().state, None, 1);
    assert_eq!(h.state().state.cursor, p1);
    // lecture : le curseur avance, puis la lecture s'arrête d'elle-même à la fin
    h.state_mut().state.set_cursor(lo);
    v.playing = true;
    h.state_mut().viewer = v;
    h.run_steps(2);
    assert!(h.state().state.cursor > lo, "le curseur n'avance pas en lecture");
    for _ in 0..200 {
        h.step();
        if !h.state().viewer.playing {
            break;
        }
    }
    assert!(!h.state().viewer.playing, "la lecture ne s'arrête pas");
    assert!(h.state().state.cursor >= hi - 1e-9 && h.state().state.cursor <= hi);
}

/// Les bornes d'un fragment tombent toujours sur un échantillon, dans l'ordre, à une seconde d'écart au moins ;
/// quand on rapproche trop une borne de l'autre, c'est celle qu'on déplace qui s'arrête.
///
/// **Contre-preuve** : sans le `min(b - gap)` de `snap_bounds`, un début tiré au-delà de la fin passe devant elle et
/// l'assertion sur l'écart rougit.
#[test]
fn les_bornes_se_posent_sur_un_echantillon_et_gardent_leur_ecart() {
    use otb_gui::viewer_page::snap_bounds;
    let h = banc();
    let log = h.state().state.logs[0].log.clone();
    let t = &log.t;
    let sur_un_echantillon = |x: f64| t[log.index_of(x)] == x;
    let (a, b) = snap_bounds(&log, (t[100] + 1e-5, t[3000] - 1e-5), 0);
    assert!(sur_un_echantillon(a) && sur_un_echantillon(b), "{a} {b}");
    // début tiré au-delà de la fin : il s'arrête une seconde avant elle, la fin ne bouge pas
    let (a, b) = snap_bounds(&log, (t[3000] + 0.5, t[3000]), 0);
    assert_eq!(b, t[3000]);
    assert!(
        b - a >= 1.0 - 1.0 / log.fs && b - a <= 1.0 + 1.0 / log.fs,
        "écart {}",
        b - a
    );
    // fin tirée avant le début : c'est elle qui s'arrête
    let (a, b) = snap_bounds(&log, (t[100], t[50]), 1);
    assert_eq!(a, t[100]);
    assert!(b > a + 0.99, "écart {}", b - a);
    // hors du log : ramenées à ses extrémités
    let (a, b) = snap_bounds(&log, (-10.0, 1e6), 0);
    assert_eq!((a, b), (t[0], *t.last().unwrap()));
}
