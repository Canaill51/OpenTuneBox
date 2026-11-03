//! Fenêtre principale : thème conservé quand le système change de thème, ouverture de fichiers fichier par fichier.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use otb_gui::app::{App, Screen};
use otb_gui::theme;

fn banc(app: App) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(900.0, 500.0))
        .with_context_setup(otb_gui::app::setup)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app)
}

#[test]
fn le_theme_choisi_survit_a_un_changement_de_theme_du_systeme() {
    let mut app = App::new(Vec::new());
    app.theme = theme::SOMBRE;
    let mut h = banc(app);
    h.run();
    assert_eq!(h.ctx.global_style().visuals.panel_fill, theme::SOMBRE.ui.window);
    // le bureau passe en clair puis en sombre : notre palette doit rester
    for sys in [egui::Theme::Light, egui::Theme::Dark] {
        h.input_mut().system_theme = Some(sys);
        h.run();
        assert_eq!(
            h.ctx.global_style().visuals.panel_fill,
            theme::SOMBRE.ui.window,
            "système {sys:?}"
        );
    }
}

fn donnees() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")
}

/// Les fichiers du double-clic (association `.bbl` : `OpenTuneBox.exe "%1"`, AppImage : `%F`) : tout sauf les options.
#[test]
fn les_arguments_de_lancement_sont_les_fichiers_sans_les_options() {
    use std::ffi::OsString;
    let args = |v: &[&str]| v.iter().map(OsString::from).collect::<Vec<_>>();
    assert!(otb_gui::app::files_from_args(args(&[])).is_empty());
    assert_eq!(
        otb_gui::app::files_from_args(args(&["a.bbl", "--option", "dossier/b bis.BFL", "-x"])),
        vec![PathBuf::from("a.bbl"), PathBuf::from("dossier/b bis.BFL")]
    );
}

/// Double-clic sur un log : directement les courbes, même si le Guide était sur une autre page que l'accueil (comme la version Python).
#[test]
fn un_log_donne_au_lancement_ouvre_les_courbes_meme_depuis_une_page_du_guide() {
    let mut a = App::new(Vec::new());
    a.screen = Screen::Guide;
    a.open_at_launch(Vec::new());
    assert_eq!(a.screen, Screen::Guide, "aucun fichier : l'écran ne change pas");
    a.open_at_launch(vec![PathBuf::from("/inexistant/absent.bbl")]);
    assert_eq!(a.screen, Screen::Viewer);
}

#[test]
fn un_fichier_illisible_n_empeche_pas_les_suivants() {
    assert!(donnees().exists(), "{}", donnees().display());
    let mut h = banc(App::new(Vec::new()));
    h.state_mut()
        .open(vec![PathBuf::from("/inexistant/absent.bbl"), donnees(), donnees()]);
    let debut = Instant::now();
    // ⚠️ `run()` panique si le fil de calcul redemande une repeinte pendant qu'il tourne
    // (« exceeded max_steps », vu sur Windows en CI) : on avance au pas, sans exiger le repos.
    while h.state().state.logs.len() < 2 {
        h.step();
        assert!(debut.elapsed() < Duration::from_secs(30), "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(3);
    let a = h.state();
    assert_eq!(a.state.logs.len(), 2, "les deux bons fichiers sont ouverts");
    assert!(
        a.status.contains("absent.bbl") && a.status.contains("illisible"),
        "{}",
        a.status
    );
    assert_eq!(a.state.logs[1].label, format!("{} (2)", a.state.logs[0].label));
}

// ---- export PNG de la fenêtre et des outils (6-6a) ----

use std::cell::RefCell;
use std::rc::Rc;

use egui_kittest::kittest::{NodeT, Queryable};

/// App avec les vraies pages, un log ouvert, et une boîte d'enregistrement qui rend `cible`
/// (ou `None` : annulation) en notant le nom proposé.
fn banc_export(cible: Option<PathBuf>, propose: Rc<RefCell<Vec<PathBuf>>>) -> Harness<'static, App> {
    banc_export_avec(cible, propose, Degrade)
}

/// Moteur de rendu qui échoue toujours : la demande de capture n'est jamais honorée (surface de taille nulle…).
struct Muet;

impl egui_kittest::TestRenderer for Muet {
    fn handle_delta(&mut self, delta: &mut egui::TexturesDelta) {
        delta.clear();
    }

    fn render(&mut self, _: &egui::Context, _: &egui::FullOutput) -> Result<image::RgbaImage, String> {
        Err("pas de rendu".into())
    }
}

fn banc_export_avec(
    cible: Option<PathBuf>,
    propose: Rc<RefCell<Vec<PathBuf>>>,
    renderer: impl egui_kittest::TestRenderer + 'static,
) -> Harness<'static, App> {
    let mut app = App::new(otb_gui::tools::all_pages());
    // jamais le vrai dossier personnel : le nom proposé dépend des fichiers déjà là (un essai à la main y laisse des images)
    app.export_dir = Some(dossier_temp("banc"));
    app.save_dialog = Box::new(move |nom| {
        propose.borrow_mut().push(nom.to_path_buf());
        cible.clone()
    });
    let mut h = Harness::builder()
        .with_size(egui::vec2(900.0, 500.0))
        .with_context_setup(otb_gui::app::setup)
        .renderer(renderer)
        .build_ui_state(|ui, a: &mut App| a.ui(ui), app);
    h.state_mut().open(vec![donnees()]);
    let debut = Instant::now();
    while h.state().state.logs.is_empty() {
        h.run();
        assert!(debut.elapsed() < Duration::from_secs(30), "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(5));
    }
    h
}

fn dossier_temp(nom: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("otb-{nom}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Moteur de rendu de test (la CI n'a pas de carte graphique) : une image de la taille de la fenêtre dont chaque pixel
/// dit où il est (rouge = x, vert = y, modulo 256, opaque).
struct Degrade;

impl egui_kittest::TestRenderer for Degrade {
    fn handle_delta(&mut self, delta: &mut egui::TexturesDelta) {
        delta.clear(); // rien à dessiner : les textures n'ont pas besoin d'exister
    }

    fn render(&mut self, ctx: &egui::Context, _: &egui::FullOutput) -> Result<image::RgbaImage, String> {
        let (w, h) = (
            ctx.content_rect().width() * ctx.pixels_per_point(),
            ctx.content_rect().height() * ctx.pixels_per_point(),
        );
        Ok(image::RgbaImage::from_fn(w.round() as u32, h.round() as u32, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 9, 255])
        }))
    }
}

/// Laisse l'application demander la capture (le banc y répond), puis écrire le fichier.
fn attendre_export(h: &mut Harness<'static, App>) {
    for _ in 0..6 {
        h.step();
    }
}

#[test]
fn l_icone_de_la_barre_exporte_toute_la_fenetre() {
    let dir = dossier_temp("fenetre");
    let cible = dir.join("fenetre.png");
    let propose = Rc::new(RefCell::new(Vec::new()));
    let mut h = banc_export(Some(cible.clone()), propose.clone());
    h.run();
    h.get_by_label("Exporter la fenêtre en image PNG").click();
    attendre_export(&mut h);
    let img = image::open(&cible).expect("PNG écrit").to_rgba8();
    let ppp = h.ctx.pixels_per_point();
    assert_eq!(
        (img.width(), img.height()),
        ((900.0 * ppp).round() as u32, (500.0 * ppp).round() as u32),
        "la fenêtre entière"
    );
    assert!(
        h.state().export_status.starts_with("Image enregistrée"),
        "{}",
        h.state().export_status
    );
    assert_eq!(
        h.state().export_dir.as_deref(),
        Some(dir.as_path()),
        "dossier retenu pour la prochaine fois"
    );
    let nom = propose.borrow()[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(nom.starts_with("OTB_FIGS_") && nom.ends_with("-1.png"), "{nom}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn le_bouton_d_un_outil_exporte_seulement_sa_page() {
    let dir = dossier_temp("outil");
    let cible = dir.join("outil"); // sans extension : « .png » est ajouté
    let propose = Rc::new(RefCell::new(Vec::new()));
    let mut h = banc_export(Some(cible), propose);
    h.state_mut().open_tool("Analyse spectrale");
    // un message connu dans la barre d'état de la fenêtre : on y lit où la page doit s'arrêter
    h.state_mut().export_status = "etat-connu".into();
    h.run();
    let haut_barre = h.get_by_label("etat-connu").rect().min.y;
    h.get_by_label("Exporter image").click();
    attendre_export(&mut h);
    let img = image::open(dir.join("outil.png"))
        .expect("PNG écrit avec l'extension")
        .to_rgba8();
    let ppp = h.ctx.pixels_per_point();
    assert_eq!(img.width(), (900.0 * ppp).round() as u32, "toute la largeur");
    // le dégradé dit d'où vient chaque pixel : rouge = x, vert = y
    let coin = img.get_pixel(0, 0).0;
    assert_eq!(coin[0], 0, "le bord gauche de la fenêtre");
    let haut = coin[1] as f32 / ppp;
    assert!(haut >= 40.0, "la page commence sous la barre du haut : y = {haut}");
    // la page finit juste au-dessus de la barre d'état de la fenêtre (le cadre de la barre est un peu plus haut que son texte)
    let bas = haut + img.height() as f32 / ppp;
    assert!(
        bas <= haut_barre && haut_barre - bas < 16.0,
        "la page doit finir juste au-dessus de la barre d'état : bas = {bas}, texte de la barre = {haut_barre}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn annuler_la_boite_n_ecrit_rien() {
    let propose = Rc::new(RefCell::new(Vec::new()));
    let mut h = banc_export(None, propose.clone());
    let avant = h.state().export_dir.clone();
    h.run();
    h.get_by_label("Exporter la fenêtre en image PNG").click();
    attendre_export(&mut h);
    assert_eq!(propose.borrow().len(), 1, "la boîte s'est ouverte");
    assert!(h.state().export_status.is_empty(), "{}", h.state().export_status);
    assert_eq!(h.state().export_dir, avant, "dossier inchangé");
}

#[test]
fn un_dossier_introuvable_donne_un_message_d_erreur() {
    let cible = std::env::temp_dir().join("otb-absent-xyz").join("sous").join("a.png");
    let mut h = banc_export(Some(cible), Rc::new(RefCell::new(Vec::new())));
    let avant = h.state().export_dir.clone();
    h.run();
    h.get_by_label("Exporter la fenêtre en image PNG").click();
    attendre_export(&mut h);
    assert!(
        h.state().export_status.starts_with("Export impossible"),
        "{}",
        h.state().export_status
    );
    assert_eq!(
        h.state().export_dir,
        avant,
        "le dossier n'est retenu qu'après une écriture réussie"
    );
}

#[test]
fn le_dossier_retenu_sert_au_nom_propose_suivant() {
    let dir = dossier_temp("suivant");
    let propose = Rc::new(RefCell::new(Vec::new()));
    let mut h = banc_export(Some(dir.join("a.png")), propose.clone());
    h.run();
    h.get_by_label("Exporter la fenêtre en image PNG").click();
    attendre_export(&mut h);
    h.get_by_label("Exporter la fenêtre en image PNG").click();
    attendre_export(&mut h);
    let noms = propose.borrow();
    assert_eq!(noms.len(), 2);
    assert_eq!(
        noms[1].parent(),
        Some(dir.as_path()),
        "le 2e nom est proposé dans le dossier du 1er export"
    );
    drop(noms);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Stockage d'eframe en mémoire.
#[derive(Default)]
struct Memoire(std::collections::HashMap<String, String>);

impl eframe::Storage for Memoire {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }
    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }
    fn flush(&mut self) {}
}

#[test]
fn app_new_n_ouvre_jamais_de_boite_d_enregistrement_du_systeme() {
    // la boîte des tests répond « annulé » sans rien afficher (celle du système bloque le test, voir `App::new`)
    let mut a = App::new(Vec::new());
    assert_eq!((a.save_dialog)(std::path::Path::new("/d/a.png")), None);
}

#[test]
fn le_dossier_d_export_survit_entre_deux_lancements() {
    use eframe::App as _;
    let mut a = App::new(Vec::new());
    a.export_dir = Some(PathBuf::from("/un/dossier"));
    let mut stockage = Memoire::default();
    a.save(&mut stockage);
    let mut b = App::new(Vec::new());
    b.restore(&stockage);
    assert_eq!(b.export_dir, Some(PathBuf::from("/un/dossier")));
    // jamais exporté : rien n'est retenu, et un stockage vide ne casse rien
    let mut vide = Memoire::default();
    App::new(Vec::new()).save(&mut vide);
    let mut c = App::new(Vec::new());
    c.restore(&vide);
    assert_eq!(c.export_dir, None);
    c.restore(&Memoire::default());
    assert_eq!(c.export_dir, None);
}

#[test]
fn une_capture_jamais_recue_est_abandonnee_et_l_export_suivant_reste_possible() {
    let dir = dossier_temp("muet");
    let mut h = banc_export_avec(Some(dir.join("a.png")), Rc::new(RefCell::new(Vec::new())), Muet);
    h.state_mut().capture_timeout = Duration::ZERO;
    h.run();
    h.get_by_label("Exporter la fenêtre en image PNG").click();
    attendre_export(&mut h);
    assert!(
        h.state().export_status.starts_with("Export impossible") && h.state().export_status.contains("capture"),
        "{}",
        h.state().export_status
    );
    assert!(!dir.join("a.png").exists());
    // un nouveau clic est de nouveau pris en compte (la demande bloquée ne reste pas jusqu'à la fermeture)
    h.state_mut().export_status.clear();
    h.get_by_label("Exporter la fenêtre en image PNG").click();
    attendre_export(&mut h);
    assert!(
        h.state().export_status.starts_with("Export impossible"),
        "{}",
        h.state().export_status
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn un_nom_sans_extension_ne_remplace_pas_un_fichier_existant() {
    let dir = dossier_temp("ecrase");
    std::fs::write(dir.join("vol1.png"), b"ancien").unwrap();
    let mut h = banc_export(Some(dir.join("vol1")), Rc::new(RefCell::new(Vec::new())));
    h.run();
    h.get_by_label("Exporter la fenêtre en image PNG").click();
    attendre_export(&mut h);
    assert_eq!(
        std::fs::read(dir.join("vol1.png")).unwrap(),
        b"ancien",
        "fichier intact"
    );
    assert!(
        h.state().export_status.contains("existe déjà"),
        "{}",
        h.state().export_status
    );
    assert_eq!(
        h.state().export_dir.as_deref().map(|d| d != dir.as_path()),
        Some(true),
        "dossier non retenu"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn un_nom_complet_choisi_dans_la_boite_est_ecrit_meme_s_il_existe() {
    // la boîte du système a déjà demandé la confirmation pour un nom complet : on écrit
    let dir = dossier_temp("ecrase-ok");
    std::fs::write(dir.join("vol1.png"), b"ancien").unwrap();
    let mut h = banc_export(Some(dir.join("vol1.png")), Rc::new(RefCell::new(Vec::new())));
    h.run();
    h.get_by_label("Exporter la fenêtre en image PNG").click();
    attendre_export(&mut h);
    assert_ne!(std::fs::read(dir.join("vol1.png")).unwrap(), b"ancien");
    assert!(
        h.state().export_status.starts_with("Image enregistrée"),
        "{}",
        h.state().export_status
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn la_barre_d_etat_montre_l_erreur_d_ouverture_et_l_export_ensemble() {
    use otb_gui::app::status_line;
    assert_eq!(status_line("", "", ""), "");
    assert_eq!(status_line("", "", "Ouverture en cours…"), "Ouverture en cours…");
    assert_eq!(status_line("x illisible", "", "Ouverture en cours…"), "x illisible");
    assert_eq!(
        status_line("", "Image enregistrée : a.png", "Ouverture en cours…"),
        "Image enregistrée : a.png"
    );
    assert_eq!(
        status_line("x illisible", "Export impossible : y", ""),
        "x illisible · Export impossible : y",
        "aucun des deux messages ne masque l'autre"
    );
}

#[test]
fn ouvrir_un_fichier_efface_le_resultat_du_dernier_export() {
    let mut a = App::new(Vec::new());
    a.export_status = "Image enregistrée : a.png".into();
    a.open(vec![PathBuf::from("/inexistant/absent.bbl")]);
    assert!(a.export_status.is_empty());
}

#[test]
fn le_delai_de_capture_ne_se_declenche_pas_apres_un_export_reussi() {
    let dir = dossier_temp("delai");
    let mut h = banc_export(Some(dir.join("a.png")), Rc::new(RefCell::new(Vec::new())));
    h.state_mut().capture_timeout = Duration::from_millis(150);
    h.run();
    h.get_by_label("Exporter la fenêtre en image PNG").click();
    attendre_export(&mut h);
    assert!(
        h.state().export_status.starts_with("Image enregistrée"),
        "{}",
        h.state().export_status
    );
    // bien après le délai : le message de réussite n'est pas remplacé par une fausse erreur
    std::thread::sleep(Duration::from_millis(400));
    h.run_steps(3);
    assert!(
        h.state().export_status.starts_with("Image enregistrée"),
        "{}",
        h.state().export_status
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- fermer les logs (W-039) ----

/// Banc avec deux logs ouverts (le second porte le suffixe « (2) »).
fn banc_deux_logs() -> Harness<'static, App> {
    let mut h = banc_export(None, Rc::new(RefCell::new(Vec::new())));
    h.state_mut().open(vec![donnees()]);
    let debut = Instant::now();
    while h.state().state.logs.len() < 2 {
        h.run();
        assert!(debut.elapsed() < Duration::from_secs(30), "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run();
    h
}

#[test]
fn fermer_retire_le_log_affiche_puis_le_suivant() {
    let mut h = banc_deux_logs();
    let deuxieme = h.state().state.logs[1].label.clone();
    assert_eq!(h.state().state.current, Some(1), "le dernier log ouvert est affiché");
    h.get_by_label("Fermer le log affiché").click();
    h.run();
    let a = h.state();
    assert_eq!(a.state.logs.len(), 1);
    assert_ne!(
        a.state.logs[0].label, deuxieme,
        "c'est le log affiché qui part, pas le premier"
    );
    assert_eq!(a.state.current, Some(0));
    h.get_by_label("Fermer le log affiché").click();
    h.run();
    assert!(h.state().state.logs.is_empty());
    assert_eq!(h.state().state.current, None);
}

#[test]
fn tout_fermer_vide_la_liste() {
    let mut h = banc_deux_logs();
    h.get_by_label("Fermer tous les logs").click();
    h.run();
    assert!(h.state().state.logs.is_empty());
    assert_eq!(h.state().state.current, None);
}

#[test]
fn sans_log_les_boutons_de_fermeture_ne_font_rien() {
    let mut h = banc(App::new(Vec::new()));
    h.run();
    assert!(h.get_by_label("Fermer le log affiché").accesskit_node().is_disabled());
    assert!(h.get_by_label("Fermer tous les logs").accesskit_node().is_disabled());
}
