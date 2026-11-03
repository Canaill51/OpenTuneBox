//! Contrats de l'interface vérifiables sans écran (port de `test_gui_contracts.py`) : aucune couleur hors de
//! `theme.rs` (L-016), icônes et licence, contrastes des accents, deux thèmes rendus.

use std::path::PathBuf;

use egui::Color32;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use otb_gui::app::App;
use otb_gui::icons::{ICONS, source};
use otb_gui::theme::{self, ACCENTS, THEMES};

fn racine() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Tous les `.rs` de `src/`, sous-dossiers compris (les outils de 6-1 à 6-4 iront dans `src/tools/`).
fn fichiers_src() -> Vec<PathBuf> {
    fn parcourt(d: &std::path::Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                parcourt(&p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    let mut v = Vec::new();
    parcourt(&racine().join("src"), &mut v);
    v.sort();
    v
}

/// Lignes de `texte` qui contiennent une couleur en dur.
fn couleurs_en_dur(texte: &str) -> Vec<(usize, String)> {
    let mut mauvais = Vec::new();
    for (i, ligne) in texte.lines().enumerate() {
        if ligne.trim_start().starts_with("//") {
            continue;
        }
        let code = ligne.split("//").next().unwrap();
        let hex = code
            .as_bytes()
            .windows(7)
            .any(|w| w[0] == b'#' && w[1..].iter().all(u8::is_ascii_hexdigit));
        let mut nommee = false;
        for (pos, _) in code.match_indices("Color32::") {
            let suite = &code[pos + "Color32::".len()..];
            nommee |= !suite.starts_with("TRANSPARENT");
        }
        if hex || nommee || code.contains("Rgba::") || code.contains("Hsva") || code.contains("gamma_multiply") {
            mauvais.push((i + 1, ligne.trim().to_string()));
        }
    }
    mauvais
}

#[test]
fn le_detecteur_de_couleurs_voit_chaque_forme_et_laisse_passer_le_reste() {
    for ko in [
        "let c = Color32::from_rgb(1, 2, 3);",
        "ui.colored_label(Color32::RED, x);",
        "let c = \"#1e1f22\";",
        "let c = egui::Color32::WHITE;",
        "let c = Rgba::from_rgb(1.0, 0.0, 0.0);",
    ] {
        assert_eq!(couleurs_en_dur(ko).len(), 1, "non vu : {ko}");
    }
    for ok in [
        "pub color: Color32,",
        "let c = Color32::TRANSPARENT;",
        "// Color32::RED dans un commentaire",
        "ui.colored_label(theme.accent(\"red\"), x);",
        "fn f(c: Color32) -> Color32 { c }",
    ] {
        assert!(couleurs_en_dur(ok).is_empty(), "faux positif : {ok}");
    }
}

#[test]
fn le_parcours_de_src_lit_les_sous_dossiers() {
    let d = std::env::temp_dir().join(format!("otb-gui-parcours-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("tools")).unwrap();
    std::fs::write(d.join("tools/a.rs"), "").unwrap();
    // même parcours que `fichiers_src`, sur un dossier fabriqué
    fn parcourt(d: &std::path::Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() { parcourt(&p, out) } else { out.push(p) }
        }
    }
    let mut v = Vec::new();
    parcourt(&d, &mut v);
    std::fs::remove_dir_all(&d).unwrap();
    assert_eq!(v.len(), 1);
}

#[test]
fn toute_couleur_vit_dans_theme_rs() {
    let mut mauvais = Vec::new();
    for p in fichiers_src() {
        if p.file_name().unwrap() == "theme.rs" {
            continue;
        }
        for (n, l) in couleurs_en_dur(&std::fs::read_to_string(&p).unwrap()) {
            mauvais.push(format!("{}:{n} {l}", p.file_name().unwrap().to_string_lossy()));
        }
    }
    assert!(
        mauvais.is_empty(),
        "couleur en dur hors de theme.rs (L-016) :\n{}",
        mauvais.join("\n")
    );
}

#[test]
fn icones_et_licence() {
    let dossier = racine().join("assets/icons");
    assert!(
        dossier.join("LICENSE-lucide.txt").exists(),
        "licence ISC de Lucide absente"
    );
    let sur_disque: std::collections::BTreeSet<_> = std::fs::read_dir(&dossier)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "svg"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().to_string())
        .collect();
    let embarquees: std::collections::BTreeSet<_> = ICONS.iter().map(|(n, _)| n.to_string()).collect();
    assert!(!sur_disque.is_empty());
    assert_eq!(
        sur_disque, embarquees,
        "chaque SVG du dossier est embarqué, et inversement"
    );
    // les icônes citées par le code existent : noms littéraux passés à `icon("…", …)`
    let mut citees = Vec::new();
    for p in fichiers_src() {
        let t = std::fs::read_to_string(&p).unwrap();
        for (pos, _) in t.match_indices("icon(\"") {
            let nom: String = t[pos + 6..].chars().take_while(|c| *c != '"').collect();
            citees.push(nom);
        }
    }
    for nom in &citees {
        assert!(source(nom).is_some(), "icône citée mais absente : {nom}");
    }
    assert!(!citees.is_empty(), "le détecteur d'icônes ne trouve rien");
    // Lucide dessine en currentColor, que `icon` remplace pour pouvoir teinter
    assert!(ICONS.iter().all(|(_, s)| s.contains("currentColor")));
}

fn luminance(c: Color32) -> f64 {
    let lin = |v: u8| {
        let v = v as f64 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

fn contraste(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

#[test]
fn les_accents_restent_lisibles_sur_les_fonds_de_leur_theme() {
    // `theme.py` annonce un contraste ≥ 4,5:1 : on le mesure au lieu de le croire
    for t in THEMES {
        for (nom, ..) in ACCENTS {
            for (fond, c) in [("window", t.ui.window), ("base", t.ui.base)] {
                let r = contraste(t.accent(nom), c);
                assert!(r >= 4.5, "accent {nom} sur {fond} du thème {} : {r:.2}:1", t.name);
            }
        }
        for (nom, c) in [("text", t.ui.text), ("muted", t.ui.muted)] {
            let r = contraste(c, t.ui.window);
            assert!(r >= 4.5, "{nom} sur window du thème {} : {r:.2}:1", t.name);
        }
    }
}

#[test]
fn les_themes_se_retrouvent_par_nom() {
    assert_eq!(theme::by_name("sombre").name, "sombre");
    assert_eq!(theme::by_name("clair").name, "clair");
    assert_eq!(theme::by_name("inconnu").name, "clair");
    assert_ne!(theme::CLAIR.bg, theme::SOMBRE.bg);
    assert_ne!(theme::CLAIR.accent("green"), theme::SOMBRE.accent("green"));
    assert_ne!(
        theme::CLAIR.trace_color("gyro"),
        theme::SOMBRE.trace_color("gyro"),
        "noir sur noir en thème sombre"
    );
}

#[test]
fn la_fenetre_se_dessine_dans_les_deux_themes_sans_log() {
    for t in [theme::CLAIR, theme::SOMBRE] {
        let mut app = App::new(Vec::new());
        app.theme = t;
        let mut h = Harness::builder()
            .with_size(egui::vec2(900.0, 500.0))
            .with_context_setup(otb_gui::app::setup)
            .build_ui_state(|ui, a: &mut App| a.ui(ui), app);
        h.run();
        h.run();
        let fond = h.ctx.global_style().visuals.panel_fill;
        assert_eq!(fond, t.ui.window, "thème {}", t.name);
        h.get_by_label("Ouvrir…");
        assert_eq!(h.get_all_by_label("Guide").count(), 1, "le bouton du guide");
        h.get_by_label("Régler son drone avec OpenTuneBox"); // l'accueil du guide est affiché
    }
}

#[test]
fn chaque_onglet_de_la_barre_a_son_icone_et_sa_couleur_comme_python() {
    use otb_gui::app::tab_icon;
    // `gui/main_window.py:TOOLS` : (titre Rust, icône, couleur) ; « Courbes » = le visualiseur, sans couleur
    for (titre, icone, couleur) in [
        ("Courbes", "activity", None),
        ("Analyse spectrale", "audio-waveform", Some("blue")),
        ("Réponse indicielle", "chart-line", Some("green")),
        ("Curseurs PID", "sliders-horizontal", Some("pink")),
        ("Simulateur de filtres", "funnel", Some("teal")),
        ("Assistant", "lightbulb", Some("orange")),
    ] {
        assert_eq!(tab_icon(titre), Some((icone, couleur)), "{titre}");
        assert!(source(icone).is_some(), "icône {icone} absente");
        if let Some(c) = couleur {
            assert!(otb_gui::theme::ACCENTS.iter().any(|(n, ..)| *n == c), "accent {c}");
        }
    }
    assert_eq!(
        tab_icon("Configuration"),
        None,
        "page ouverte depuis un autre bouton : pas d'onglet à elle"
    );
}

/// L-076 : sous Linux (Wayland), le bureau trouve l'icône de la fenêtre par un fichier `.desktop` dont le **nom** est l'`app_id`
/// de la fenêtre, majuscules comprises (`OpenTuneBox.desktop` : bonne icône ; `opentunebox.desktop` avec `StartupWMClass` : icône
/// générique, essayé sur KDE Plasma le 2026-11-03). Les deux scripts de `packaging/linux/` doivent donc écrire ce nom exact.
#[test]
fn le_fichier_desktop_porte_le_nom_de_l_app_id() {
    let main = std::fs::read_to_string(racine().join("src/main.rs")).unwrap();
    let debut = main.find("with_app_id(\"").expect("with_app_id dans main.rs") + "with_app_id(\"".len();
    let app_id = &main[debut..debut + main[debut..].find('"').unwrap()];
    assert!(!app_id.is_empty());
    for (script, dossier) in [("install_linux.sh", "$APPS"), ("build_appimage.sh", "$APPDIR")] {
        let texte = std::fs::read_to_string(racine().join("../../packaging/linux").join(script)).unwrap();
        let attendu = format!("\"{dossier}/{app_id}.desktop\"");
        assert!(
            texte.contains(&attendu),
            "{script} doit écrire {attendu} (nom du fichier .desktop = app_id de la fenêtre, L-076)"
        );
        assert!(
            texte.contains(&format!("StartupWMClass={app_id}")),
            "{script} : StartupWMClass doit valoir l'app_id (X11)"
        );
    }
}
