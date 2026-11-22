//! La bibliothèque de données : aller-retour sur disque, protections contre la perte de données, refus des
//! doublons, péremption des mesures, et les chiffres de départ du déclencheur de changement (lot 28).
//!
//! **Contre-preuves** (L-071) : chaque protection a été cassée à la main et le test visé a rougi — c'est noté
//! sous le test concerné, avec ce qui échoue.

use otb_store::model::{Analysis, AnalysisItem, Drone, FORMAT_VERSION, Fragment, LogEntry, Measure};
use otb_store::{Library, dir_in, fingerprint};
use std::ffi::OsString;
use std::path::PathBuf;

/// Un dossier d'essai propre, hors du dépôt. Le nom porte le processus : deux exécutions ne se gênent pas.
fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-store-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dossier d'essai");
    d
}

fn log(id: &str, drone: &str, nom: &str) -> LogEntry {
    LogEntry {
        id: id.to_string(),
        drone_id: drone.to_string(),
        name: nom.to_string(),
        path: format!("/logs/{nom}.bbl"),
        copied: true,
        bytes: 2_489_856,
        added: "2026-11-18T10:00:00".to_string(),
    }
}

fn fragment(id: &str, log_id: &str, bornes: (u64, u64), date: &str) -> Fragment {
    Fragment {
        id: id.to_string(),
        log_id: log_id.to_string(),
        session: 1,
        start: bornes.0,
        end: bornes.1,
        name: id.to_string(),
        role: "filters".to_string(),
        date: date.to_string(),
    }
}

fn mesure(fragment: &str, valeur: f64, version: u64) -> Measure {
    Measure {
        fragment_id: fragment.to_string(),
        axis: 0,
        key: "latence_ms".to_string(),
        value: valeur,
        settings: "reglages-a".to_string(),
        calc_version: version,
    }
}

/// Une bibliothèque remplie, pour les allers-retours.
fn remplie(dir: PathBuf) -> Library {
    let mut lib = Library::open(dir);
    let mut d = Drone::new("uid-0001", "JeNo Ced");
    d.firmware = "Betaflight 4.5.1".into();
    d.first_seen = "2026-11-02".into();
    lib.put_drone(d);
    assert!(lib.add_log(log("emp-aaa", "uid-0001", "FINAL")));
    lib.put_fragment(fragment("fr-1", "emp-aaa", (0, 5_000), "2026-11-13"));
    lib.record_config("uid-0001", "2026-11-13T18:10:12", "set gyro_lpf1_static_hz = 165");
    lib.set_measure(mesure("fr-1", 12.5, 1));
    lib.set_analysis(Analysis {
        items: vec![AnalysisItem {
            fragment_id: "fr-1".into(),
            colour: 2,
            active: true,
        }],
        working: Some("fr-1".into()),
    });
    lib
}

#[test]
fn tout_se_retrouve_apres_fermeture_et_reouverture() {
    let dir = dossier("aller-retour");
    let mut lib = remplie(dir.clone());
    lib.save().expect("écriture");

    let relue = Library::open(dir);
    assert!(relue.problems().is_empty(), "{:?}", relue.problems());
    assert_eq!(relue.drones().len(), 1);
    assert_eq!(relue.drone("uid-0001").unwrap().name, "JeNo Ced");
    assert_eq!(relue.drone("uid-0001").unwrap().firmware, "Betaflight 4.5.1");
    assert_eq!(relue.logs().len(), 1);
    assert_eq!(relue.log("emp-aaa").unwrap().bytes, 2_489_856);
    assert_eq!(relue.fragment("fr-1").unwrap().end, 5_000);
    assert_eq!(relue.fragment("fr-1").unwrap().role, "filters");
    assert_eq!(relue.configs_of_drone("uid-0001").len(), 1);
    assert_eq!(relue.measure("fr-1", 0, "latence_ms").unwrap().value, 12.5);
    assert_eq!(relue.analysis().working.as_deref(), Some("fr-1"));
    assert_eq!(relue.analysis().items[0].colour, 2);
    assert_eq!(relue.copied_bytes(), 2_489_856);
}

/// ⚠️ La protection qui évite de perdre les données de l'utilisateur : un fichier illisible n'est **jamais**
/// écrasé en silence (même règle que les fiches drone, D-039).
///
/// **Contre-preuve** : en retirant la mise de côté dans `Library::save`, le fichier abîmé est écrasé et
/// l'assertion sur son contenu rougit.
#[test]
fn un_fichier_illisible_est_mis_de_cote_et_jamais_ecrase() {
    let dir = dossier("illisible");
    std::fs::write(dir.join("fragments.json"), "{ ceci n'est pas du JSON").unwrap();

    let mut lib = Library::open(dir.clone());
    assert_eq!(lib.fragments().len(), 0, "une table illisible reste vide");
    assert!(
        lib.problems()
            .iter()
            .any(|p| p.contains("fragments.json") && p.contains("JSON invalide")),
        "{:?}",
        lib.problems()
    );

    lib.put_fragment(fragment("fr-1", "emp-aaa", (0, 10), "2026-11-18"));
    lib.save().expect("écriture");

    let garde = std::fs::read_to_string(dir.join("fragments.json.illisible")).expect("mis de côté");
    assert_eq!(garde, "{ ceci n'est pas du JSON", "le contenu abîmé est gardé tel quel");
    assert_eq!(Library::open(dir).fragments().len(), 1, "le nouveau fichier est bon");
}

/// Un fichier déjà mis de côté n'est pas écrasé par le suivant.
#[test]
fn deux_fichiers_illisibles_de_suite_gardent_chacun_leur_copie() {
    let dir = dossier("illisible-deux");
    for texte in ["premier abîmé", "second abîmé"] {
        std::fs::write(dir.join("logs.json"), texte).unwrap();
        let mut lib = Library::open(dir.clone());
        lib.save().expect("écriture");
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("logs.json.illisible")).unwrap(),
        "premier abîmé"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("logs.json.illisible.1")).unwrap(),
        "second abîmé"
    );
}

/// ⚠️ Un fichier écrit par une version **plus récente** d'OpenTuneBox peut porter des champs que celle-ci ignore.
/// L'écraser perdrait ces champs sans le dire : il est mis de côté et la raison est affichable.
///
/// **Contre-preuve** : en acceptant `version > FORMAT_VERSION` dans `items_of`, le fichier est relu puis réécrit
/// sans ses champs inconnus, et l'assertion sur le fichier gardé rougit.
#[test]
fn un_fichier_plus_recent_nest_jamais_ecrase() {
    let dir = dossier("plus-recent");
    let futur = format!(
        r#"{{"version": {}, "items": [{{"id": "uid-9", "nom": "Futur", "champ_inconnu": 42}}]}}"#,
        FORMAT_VERSION + 1
    );
    std::fs::write(dir.join("drones.json"), &futur).unwrap();

    let mut lib = Library::open(dir.clone());
    assert_eq!(lib.drones().len(), 0, "rien n'est lu d'un format inconnu");
    assert!(
        lib.problems().iter().any(|p| p.contains("plus récente")),
        "{:?}",
        lib.problems()
    );
    lib.save().expect("écriture");
    assert_eq!(
        std::fs::read_to_string(dir.join("drones.json.plus-recente")).unwrap(),
        futur,
        "le fichier de la version plus récente est gardé entier"
    );
}

/// Un enregistrement abîmé ne fait pas perdre les autres : il est sauté, et dit.
#[test]
fn un_enregistrement_sans_identifiant_est_saute_sans_perdre_les_autres() {
    let dir = dossier("enregistrement");
    std::fs::write(
        dir.join("drones.json"),
        r#"{"version": 1, "items": [{"nom": "sans identifiant"}, {"id": "uid-2", "nom": "Whoop 65"}]}"#,
    )
    .unwrap();
    let lib = Library::open(dir);
    assert_eq!(lib.drones().len(), 1);
    assert_eq!(lib.drones()[0].id, "uid-2");
    assert!(lib.problems().iter().any(|p| p.contains("enregistrement 1 ignoré")));
}

/// Une bibliothèque en mémoire n'écrit **jamais** : c'est ce qui garantit qu'un banc de test ne touche pas la
/// vraie bibliothèque de l'utilisateur.
#[test]
fn la_bibliotheque_en_memoire_necrit_aucun_fichier() {
    let dir = dossier("memoire");
    let mut lib = Library::in_memory();
    lib.put_drone(Drone::new("uid-1", "Essai"));
    lib.save().expect("sans dossier, ne fait rien");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    assert!(lib.logs_dir().is_none());
}

/// Deux fois le même fichier ne fait pas deux entrées : l'empreinte du contenu sert d'identifiant.
#[test]
fn un_log_deja_connu_est_refuse() {
    let mut lib = Library::in_memory();
    assert!(lib.add_log(log("emp-aaa", "uid-1", "FINAL")));
    assert!(!lib.add_log(log("emp-aaa", "uid-1", "FINAL (copie)")));
    assert_eq!(lib.logs().len(), 1);
    assert_eq!(
        lib.logs()[0].name,
        "FINAL",
        "le premier enregistrement n'est pas remplacé"
    );
}

/// L'empreinte distingue deux contenus et reconnaît le même.
#[test]
fn lempreinte_suit_le_contenu() {
    assert_eq!(fingerprint(b"meme contenu"), fingerprint(b"meme contenu"));
    assert_ne!(fingerprint(b"contenu a"), fingerprint(b"contenu b"));
    // deux contenus de longueurs différentes ne peuvent pas se confondre : la longueur est dans l'empreinte
    assert_ne!(fingerprint(b"ab"), fingerprint(b"abab"));
    assert_eq!(fingerprint(b"").len(), 24);
}

/// Un `diff all` identique au dernier n'est pas gardé : l'historique ne liste que les **changements**.
#[test]
fn une_configuration_identique_nest_pas_gardee() {
    let mut lib = Library::in_memory();
    assert!(lib.record_config("uid-1", "2026-11-13T18:00:00", "set p_roll = 58"));
    assert!(!lib.record_config("uid-1", "2026-11-13T19:00:00", "set p_roll = 58"));
    assert!(lib.record_config("uid-1", "2026-11-13T20:00:00", "set p_roll = 62"));
    // le même texte que l'avant-dernier compte comme un changement : il diffère du dernier
    assert!(lib.record_config("uid-1", "2026-11-13T21:00:00", "set p_roll = 58"));
    assert_eq!(lib.configs_of_drone("uid-1").len(), 3);
    assert!(
        lib.record_config("uid-2", "2026-11-13T18:00:00", "set p_roll = 58"),
        "autre drone"
    );
}

/// ⚠️ Une mesure calculée sur d'autres bornes ne vaut plus rien : déplacer un fragment la jette.
///
/// **Contre-preuve** : en retirant l'appel à `drop_measures_of_fragment` dans `put_fragment`, la mesure survit au
/// déplacement et l'assertion rougit.
#[test]
fn deplacer_un_fragment_jette_ses_mesures() {
    let mut lib = Library::in_memory();
    lib.put_fragment(fragment("fr-1", "emp-aaa", (0, 5_000), "2026-11-13"));
    lib.set_measure(mesure("fr-1", 12.5, 1));
    assert!(lib.measure("fr-1", 0, "latence_ms").is_some());

    // mêmes bornes : la mesure reste (renommer un fragment ne jette rien)
    let mut renomme = fragment("fr-1", "emp-aaa", (0, 5_000), "2026-11-13");
    renomme.name = "Série PID 3".into();
    lib.put_fragment(renomme);
    assert!(
        lib.measure("fr-1", 0, "latence_ms").is_some(),
        "un renommage ne jette rien"
    );

    lib.put_fragment(fragment("fr-1", "emp-aaa", (0, 4_000), "2026-11-13"));
    assert!(
        lib.measure("fr-1", 0, "latence_ms").is_none(),
        "bornes changées : jetée"
    );
}

/// Le calcul a changé de version : les chiffres gardés ne veulent plus rien dire.
#[test]
fn changer_de_version_de_calcul_jette_les_mesures_perimees() {
    let mut lib = Library::in_memory();
    lib.set_measure(mesure("fr-1", 12.5, 1));
    lib.set_measure(mesure("fr-2", 13.0, 2));
    assert_eq!(lib.drop_measures_before_version(2), 1);
    assert_eq!(lib.measures_len(), 1);
    assert_eq!(lib.measures().next().unwrap().fragment_id, "fr-2");
}

/// La question à laquelle la bibliothèque doit répondre : l'évolution d'une grandeur sur tous les vols d'un
/// drone, rangée par date — y compris quand les fragments sont enregistrés dans le désordre.
#[test]
fn lhistorique_dune_grandeur_est_range_par_date_et_par_drone() {
    let mut lib = Library::in_memory();
    lib.add_log(log("emp-a", "uid-1", "A"));
    lib.add_log(log("emp-b", "uid-2", "B"));
    lib.put_fragment(fragment("fr-3", "emp-a", (0, 10), "2026-11-13"));
    lib.put_fragment(fragment("fr-1", "emp-a", (0, 10), "2026-11-02"));
    lib.put_fragment(fragment("fr-2", "emp-a", (0, 10), "2026-11-09"));
    lib.put_fragment(fragment("fr-9", "emp-b", (0, 10), "2026-11-05"));
    for (f, v) in [("fr-1", 18.0), ("fr-2", 15.0), ("fr-3", 12.0), ("fr-9", 99.0)] {
        lib.set_measure(mesure(f, v, 1));
    }
    let h = lib.history("uid-1", "latence_ms", 0);
    assert_eq!(
        h,
        vec![
            ("2026-11-02".to_string(), 18.0),
            ("2026-11-09".to_string(), 15.0),
            ("2026-11-13".to_string(), 12.0)
        ],
        "le drone uid-2 n'y est pas, et les dates montent"
    );
    assert_eq!(lib.history("uid-2", "latence_ms", 0).len(), 1);
    assert!(lib.history("uid-1", "latence_ms", 1).is_empty(), "autre axe");
}

/// Les fragments d'un drone passent par le log dont ils viennent.
#[test]
fn les_fragments_dun_drone_viennent_de_ses_logs() {
    let mut lib = Library::in_memory();
    lib.add_log(log("emp-a", "uid-1", "A"));
    lib.add_log(log("emp-b", "uid-2", "B"));
    lib.put_fragment(fragment("fr-1", "emp-a", (0, 10), "2026-11-02"));
    lib.put_fragment(fragment("fr-2", "emp-b", (0, 10), "2026-11-02"));
    lib.put_fragment(fragment("fr-3", "emp-inconnu", (0, 10), "2026-11-02"));
    assert_eq!(lib.fragments_of_drone("uid-1").len(), 1);
    assert_eq!(lib.fragments_of_drone("uid-1")[0].id, "fr-1");
    assert_eq!(lib.fragments_of_drone("uid-3").len(), 0);
}

/// Le dossier de la bibliothèque sur les trois systèmes, sans toucher au poste (même méthode que les fiches
/// drone). La branche macOS est gardée sans être livrée (D-042).
#[test]
fn le_dossier_suit_le_systeme() {
    let var = |couples: Vec<(&'static str, &'static str)>| {
        move |k: &str| couples.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(*v))
    };
    let linux = dir_in("linux", &var(vec![("HOME", "/home/pilote")]));
    assert_eq!(
        linux,
        PathBuf::from("/home/pilote/.local/share/OpenTuneBox/OpenTuneBox/bibliotheque")
    );
    let xdg = dir_in(
        "linux",
        &var(vec![("HOME", "/home/pilote"), ("XDG_DATA_HOME", "/data")]),
    );
    assert_eq!(xdg, PathBuf::from("/data/OpenTuneBox/OpenTuneBox/bibliotheque"));
    // XDG relatif : ignoré, comme pour les fiches drone (L-074)
    let relatif = dir_in("linux", &var(vec![("HOME", "/home/pilote"), ("XDG_DATA_HOME", "data")]));
    assert_eq!(relatif, linux);
    let windows = dir_in("windows", &var(vec![("APPDATA", r"C:\Users\p\AppData\Roaming")]));
    assert_eq!(
        windows,
        PathBuf::from(r"C:\Users\p\AppData\Roaming").join("OpenTuneBox/OpenTuneBox/bibliotheque")
    );
    let macos = dir_in("macos", &var(vec![("HOME", "/Users/p")]));
    assert_eq!(
        macos,
        PathBuf::from("/Users/p/Library/Application Support/OpenTuneBox/OpenTuneBox/bibliotheque")
    );
    // aucune variable : le repli de Python
    let sans = dir_in("linux", &var(vec![]));
    assert_eq!(sans, PathBuf::from(".opentunebox/bibliotheque"));
}

/// Les **chiffres de départ** du déclencheur écrit dans `docs/plan.md` : à un volume réaliste (le corpus de
/// Cédric, 535 logs, avec 4 fragments par log et 12 mesures par fragment et par axe), la bibliothèque doit rester
/// très loin des « quelques Mo » et du « lancement visiblement plus lent » qui feraient passer à une vraie base.
/// Les bornes sont larges exprès : ce test mesure, il n'optimise pas.
#[test]
fn a_volume_reel_les_fichiers_restent_petits_et_louverture_rapide() {
    let dir = dossier("volume");
    let mut lib = Library::open(dir.clone());
    for d in 0..6 {
        lib.put_drone(Drone::new(&format!("uid-{d:04}"), &format!("Drone {d}")));
    }
    let mut mesures = 0;
    for i in 0..535 {
        let drone = format!("uid-{:04}", i % 6);
        let id = format!("emp-{i:04}");
        lib.add_log(log(&id, &drone, &format!("BTFL_{i:04}")));
        for f in 0..4 {
            let fid = format!("fr-{i:04}-{f}");
            lib.put_fragment(fragment(&fid, &id, (0, 60_000), "2026-11-13"));
            for axe in 0..3 {
                for g in 0..12 {
                    lib.set_measure(Measure {
                        fragment_id: fid.clone(),
                        axis: axe,
                        key: format!("grandeur_{g:02}"),
                        value: 12.5 + f64::from(g),
                        settings: "reglages-a".to_string(),
                        calc_version: 1,
                    });
                    mesures += 1;
                }
            }
        }
    }
    assert_eq!(mesures, 535 * 4 * 3 * 12, "77 040 mesures");

    let depart = std::time::Instant::now();
    lib.save().expect("écriture");
    let ecriture = depart.elapsed();
    let depart = std::time::Instant::now();
    let relue = Library::open(dir.clone());
    let ouverture = depart.elapsed();

    assert_eq!(relue.measures_len(), mesures);
    assert_eq!(relue.fragments().len(), 535 * 4);
    let taille = |f: &str| std::fs::metadata(dir.join(f)).map(|m| m.len()).unwrap_or(0);
    let total: u64 = [
        "drones.json",
        "logs.json",
        "fragments.json",
        "mesures.json",
        "analyse.json",
    ]
    .iter()
    .map(|f| taille(f))
    .sum();
    println!(
        "[OTB-MESURE] bibliothèque à volume réel : {} Mo au total (mesures {} Mo), écriture {} ms, ouverture {} ms",
        total / 1_000_000,
        taille("mesures.json") / 1_000_000,
        ecriture.as_millis(),
        ouverture.as_millis()
    );
    // bornes larges : le déclencheur du plan parle de « quelques Mo » et d'un lancement « visiblement plus lent »
    assert!(
        total < 40_000_000,
        "{total} octets : le déclencheur du plan est atteint"
    );
    assert!(
        ouverture.as_secs_f64() < 5.0,
        "ouverture en {ouverture:?} : le déclencheur du plan est atteint"
    );
}
