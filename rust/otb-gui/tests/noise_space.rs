//! Choix communs de l'espace « Bruit et filtres » (D-069) : le signal (« Gyro | Terme D ») et les axes, choisis
//! dans une vue, valent pour les quatre.

mod support;
use support::Cliquer;

use egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use otb_gui::app::{App, Screen};
use otb_gui::state::{AppState, NoiseSignal};
use otb_gui::tool::Tool;
use otb_gui::tools::freq_throttle::{FreqThrottleTool, PRESETS};
use otb_gui::tools::freq_time::FreqTimeTool;
use otb_gui::tools::spectral::SpectralTool;

fn banc() -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), App::new(otb_gui::tools::all_pages()));
    h.run();
    h.cliquer_label("Bruit et filtres");
    h.run();
    h
}

fn vue(h: &Harness<'static, App>) -> &'static str {
    match h.state().screen {
        Screen::Tool(i) => h.state().pages[i].title(),
        autre => panic!("écran inattendu : {autre:?}"),
    }
}

/// Bouton du choix du signal allumé ? (`Button::selected` le met à l'état « coché » pour un lecteur d'écran.)
fn allume(h: &Harness<'static, App>, signal: NoiseSignal) -> bool {
    h.get_by_role_and_label(Role::Button, signal.label())
        .accesskit_node()
        .toggled()
        == Some(Toggled::True)
}

fn case(h: &Harness<'static, App>, label: &str) -> bool {
    h.get_by_role_and_label(Role::CheckBox, label)
        .accesskit_node()
        .toggled()
        == Some(Toggled::True)
}

const ONGLETS: [(&str, &str); 4] = [
    ("Spectre", "Analyse spectrale"),
    ("Fréq. × gaz", "Fréquence × gaz"),
    ("Fréq. × temps", "Fréquence × temps"),
    ("Et si", "Simulateur de filtres"),
];

/// Les onglets de l'espace se reconnaissent comme des boutons : chacun est un bouton, celui de la vue affichée est
/// allumé, et la souris prend la main au survol d'un autre (on ne voyait pas que c'étaient des boutons, 2026-11-25).
///
/// **Contre-preuve** : sans `on_hover_cursor` dans `space_tabs`, le curseur reste la flèche et l'assertion rougit.
#[test]
fn les_onglets_de_lespace_sont_des_boutons() {
    let mut h = banc();
    for (onglet, titre) in ONGLETS {
        let allume = h.get_by_role_and_label(Role::Button, onglet).accesskit_node().toggled() == Some(Toggled::True);
        assert_eq!(allume, titre == "Analyse spectrale", "« {onglet} »");
    }
    h.get_by_role_and_label(Role::Button, "Fréq. × gaz").hover();
    h.run();
    assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::PointingHand);
}

/// « Terme D » choisi dans le Spectre allume « Terme D » dans les trois autres vues, et « Gyro » choisi dans
/// Fréq. × temps revient au gyro partout.
///
/// **Contre-preuve** : sans l'appel à `follow_space` au début de `ToolPage::ui`, les autres vues restent sur le
/// gyro et l'assertion rougit à la deuxième vue.
#[test]
fn le_choix_du_signal_vaut_pour_les_quatre_vues() {
    let mut h = banc();
    assert_eq!(vue(&h), "Analyse spectrale");
    assert!(allume(&h, NoiseSignal::Gyro), "l'espace s'ouvre sur le gyro");
    h.cliquer_role(Role::Button, "Terme D");
    h.run();
    assert_eq!(h.state().state.space.signal, NoiseSignal::DTerm);
    for (onglet, titre) in ONGLETS {
        h.cliquer_label(onglet);
        h.run();
        assert_eq!(vue(&h), titre);
        assert!(
            allume(&h, NoiseSignal::DTerm) && !allume(&h, NoiseSignal::Gyro),
            "« {onglet} » montre le terme D"
        );
    }
    h.cliquer_label("Fréq. × temps");
    h.run();
    h.cliquer_role(Role::Button, "Gyro");
    h.run();
    for (onglet, _) in ONGLETS {
        h.cliquer_label(onglet);
        h.run();
        assert!(allume(&h, NoiseSignal::Gyro), "« {onglet} » revient au gyro");
    }
}

/// Un axe décoché dans une vue l'est dans les autres.
///
/// **Contre-preuve** : sans la recopie des axes dans `publish_space`, Fréq. × gaz garde son lacet et l'assertion
/// rougit.
#[test]
fn les_axes_sont_partages_par_les_vues() {
    let mut h = banc();
    assert!(case(&h, "Y"), "les trois axes au départ");
    h.cliquer_role(Role::CheckBox, "Y");
    h.run();
    assert_eq!(h.state().state.space.axes, [true, true, false]);
    for (onglet, _) in ONGLETS {
        h.cliquer_label(onglet);
        h.run();
        assert!(
            case(&h, "R") && case(&h, "P") && !case(&h, "Y"),
            "« {onglet} » sans le lacet"
        );
    }
}

/// Un autre signal choisi dans les cases d'une vue (l'erreur PID) n'allume aucun des deux boutons et ne change pas
/// les autres vues ; un clic sur « Gyro » remet la vue sur le gyro.
///
/// **Contre-preuve** : quand `NoiseSignal::of_keys` saute les jeux qui ne sont ni gyro ni D au lieu de rendre `None`,
/// le Spectre se croit sur le gyro, son bouton s'allume et l'assertion rougit.
#[test]
fn un_autre_signal_naffecte_que_sa_vue() {
    let mut h = banc();
    h.cliquer_role(Role::CheckBox, "Erreur PID");
    h.run();
    assert!(
        !allume(&h, NoiseSignal::Gyro) && !allume(&h, NoiseSignal::DTerm),
        "ni gyro ni D seul"
    );
    let avant = h.state().state.space;
    assert_eq!(avant.signal, NoiseSignal::Gyro);
    h.cliquer_label("Fréq. × temps");
    h.run();
    assert!(allume(&h, NoiseSignal::Gyro), "Fréq. × temps n'a pas bougé");
    h.cliquer_label("Spectre");
    h.run();
    h.cliquer_role(Role::Button, "Gyro");
    h.run();
    assert!(
        allume(&h, NoiseSignal::Gyro) && !case(&h, "Erreur PID"),
        "« Gyro » remet la vue sur le gyro"
    );
}

/// ⚠️ Changer de signal garde l'étage choisi dans la vue (le gyro filtré seul devient le terme D filtré seul) ; tous
/// les jeux d'un signal deviennent tous ceux de l'autre (le terme D, qui n'a plus que son étage filtré, D-078, redevient
/// le gyro avant et après filtres) ; un choix mêlé laisse place à tous les jeux du signal.
///
/// **Contre-preuve** : en remplaçant le passage par `counterpart` par les deux jeux du signal, le Spectre coche
/// `dterm_prefilt` en plus et l'assertion rougit.
#[test]
fn changer_de_signal_garde_letage_choisi() {
    let etat = AppState::new();
    let mut s = SpectralTool::new();
    s.set_dataset("gyro_prefilt", false);
    assert_eq!(s.signal(), Some(NoiseSignal::Gyro));
    s.show_signal(NoiseSignal::DTerm, &etat);
    assert_eq!(s.options().keys, ["dterm"]);
    s.set_dataset("pid_error", true);
    assert_eq!(s.signal(), None, "terme D et erreur PID mêlés");
    s.show_signal(NoiseSignal::Gyro, &etat);
    let mut keys = s.options().keys;
    keys.sort();
    assert_eq!(keys, ["gyro", "gyro_prefilt"]);
    s.show_signal(NoiseSignal::DTerm, &etat);
    assert_eq!(
        s.options().keys,
        ["dterm"],
        "gyro avant et après filtres → terme D filtré"
    );
    s.show_signal(NoiseSignal::Gyro, &etat);
    let mut keys = s.options().keys;
    keys.sort();
    assert_eq!(keys, ["gyro", "gyro_prefilt"], "terme D → gyro avant et après filtres");

    let mut t = FreqTimeTool::new();
    t.set_dataset("gyro");
    t.show_signal(NoiseSignal::DTerm, &etat);
    assert_eq!(t.options().key, "dterm");
    t.set_dataset("setpoint");
    t.show_signal(NoiseSignal::Gyro, &etat);
    assert_eq!(t.options().key, "gyro_prefilt");
}

/// Fréquence × gaz part du préréglage 1 (gyro, P et D mêlés) : le choix du gyro le remplace par « A|A|B|B Gyro pf |
/// Gyro » ; passer au terme D donne « A|B|C|D D », le terme D filtré par fragment (D-078) ; du gyro filtré seul, chaque
/// colonne passe au terme D sur son fragment.
///
/// **Contre-preuve** : avec `PRESET_GYRO` sur le préréglage 1, les colonnes restent mêlées et l'assertion rougit.
#[test]
fn frequence_gaz_suit_le_signal_par_ses_colonnes() {
    let etat = AppState::new();
    let mut g = FreqThrottleTool::new();
    assert_eq!(g.signal(), None, "préréglage 1 : gyro et D mêlés");
    g.show_signal(NoiseSignal::Gyro, &etat);
    let cles = |g: &FreqThrottleTool| g.columns().map(|c| c.0);
    assert_eq!(cles(&g)[..2], [Some("gyro_prefilt"), Some("gyro")]);
    assert_eq!(g.signal(), Some(NoiseSignal::Gyro), "plus aucune colonne du terme D");
    assert!(
        PRESETS[otb_gui::tools::freq_throttle::PRESET_GYRO]
            .0
            .contains("Gyro pf | Gyro")
    );
    g.show_signal(NoiseSignal::DTerm, &etat);
    assert!(
        PRESETS[otb_gui::tools::freq_throttle::PRESET_DTERM]
            .0
            .contains("A|B|C|D D")
    );
    assert_eq!(cles(&g)[0], Some("dterm"));
    assert!(
        cles(&g).iter().flatten().all(|k| *k == "dterm"),
        "plus que le terme D filtré"
    );
    assert_eq!(g.signal(), Some(NoiseSignal::DTerm));
    // du gyro filtré seul (préréglage 5 : A|B|C|D Gyro), chaque colonne garde son fragment
    g.apply_preset(4, &[0, 1], 0);
    g.show_signal(NoiseSignal::DTerm, &etat);
    assert_eq!(g.columns().map(|c| c.1)[..2], [Some(0), Some(1)]);
    assert!(g.columns()[..2].iter().all(|c| c.0 == Some("dterm")));
}
