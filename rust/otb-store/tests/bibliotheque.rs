//! La bibliothèque de données : aller-retour sur disque, protections contre la perte de données, refus des
//! doublons, péremption des mesures, et les chiffres de départ du déclencheur de changement (lot 28).
//!
//! **Contre-preuves** (L-071) : chaque protection a été cassée à la main et le test visé a rougi — c'est noté
//! sous le test concerné, avec ce qui échoue.

use otb_store::model::{
    Analysis, AnalysisItem, Drone, DroneSheet, FORMAT_VERSION, Fragment, LogEntry, Measure, config_changes,
};
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
        chosen: true,
    }
}

/// Une mesure de latence roll calculée sur les bornes `bornes` du fragment.
fn mesure(fragment: &str, bornes: (u64, u64), valeur: f64, version: u64) -> Measure {
    Measure {
        fragment_id: fragment.to_string(),
        axis: 0,
        key: "latence_ms".to_string(),
        value: valeur,
        start: bornes.0,
        end: bornes.1,
        settings: "reglages-a".to_string(),
        calc_version: version,
    }
}

/// Une bibliothèque remplie, pour les allers-retours.
fn remplie(dir: PathBuf) -> Library {
    let mut lib = Library::open(dir);
    lib.load_measures();
    let mut d = Drone::new("uid-0001", "JeNo Ced");
    d.firmware = "Betaflight 4.5.1".into();
    d.first_seen = "2026-11-02".into();
    lib.put_drone(d);
    assert!(lib.add_log(log("emp-aaa", "uid-0001", "FINAL")));
    lib.put_fragment(fragment("fr-1", "emp-aaa", (0, 5_000), "2026-11-13"));
    lib.record_config("uid-0001", "2026-11-13T18:10:12", "set gyro_lpf1_static_hz = 165");
    lib.set_measure(mesure("fr-1", (0, 5_000), 12.5, 1));
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

    let mut relue = Library::open(dir);
    relue.load_measures();
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
    lib.set_measure(mesure("fr-1", (0, 5_000), 12.5, 1));
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
    lib.set_measure(mesure("fr-1", (0, 10), 12.5, 1));
    lib.set_measure(mesure("fr-2", (0, 10), 13.0, 2));
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
        lib.set_measure(mesure(f, (0, 10), v, 1));
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
    lib.load_measures();
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
                        start: 0,
                        end: 60_000,
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
    let mut relue = Library::open(dir.clone());
    let ouverture = depart.elapsed();
    assert_eq!(
        relue.measures_len(),
        0,
        "la table des mesures ne se lit pas à l'ouverture (D-064)"
    );
    let depart = std::time::Instant::now();
    relue.load_measures();
    let lecture_mesures = depart.elapsed();

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
        "[OTB-MESURE] bibliothèque à volume réel : {} Mo au total (mesures {} Mo), écriture {} ms, ouverture {} ms, \
         lecture des mesures {} ms",
        total / 1_000_000,
        taille("mesures.json") / 1_000_000,
        ecriture.as_millis(),
        ouverture.as_millis(),
        lecture_mesures.as_millis()
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

// ---------------------------------------------------------------- fiche du drone (lot 30d, D-068)

fn fiche() -> DroneSheet {
    DroneSheet {
        size: "5″".into(),
        cells: "6S".into(),
        weight: "680".into(),
        kv: "1950".into(),
        poles: "14".into(),
        props: "HQ 5.1×3.5×3".into(),
        esc: "BLHeli_32".into(),
        betaflight: "2026.6".into(),
        pid_khz: "8".into(),
        done: vec!["recording".into(), "rpm".into()],
        verified: vec!["blackbox".into()],
        target: None,
    }
}

/// La fiche et la carte de vol font l'aller-retour ; un drone sans fiche reste **sans** fiche (pas de valeurs
/// inventées à la relecture).
#[test]
fn la_fiche_du_drone_se_retrouve_et_un_drone_sans_fiche_le_reste() {
    let dir = dossier("fiche");
    let mut lib = Library::open(dir.clone());
    let mut d = Drone::new("uid-1", "JeNo Ced");
    d.board = "STM32F405".into();
    d.sheet = Some(fiche());
    lib.put_drone(d);
    lib.put_drone(Drone::new("Whoop", "Whoop"));
    lib.save().expect("écriture");

    let relue = Library::open(dir);
    assert!(relue.problems().is_empty(), "{:?}", relue.problems());
    assert_eq!(relue.drone("uid-1").unwrap().board, "STM32F405");
    assert_eq!(relue.drone("uid-1").unwrap().sheet, Some(fiche()));
    assert_eq!(relue.drone("Whoop").unwrap().sheet, None);
}

/// La fiche cible (lot 31b, D-072) fait l'aller-retour, cases vides comprises et dans l'ordre ; une fiche dont le
/// pilote n'a pas changé la cible n'écrit **pas** de clé `cible` (elle suit alors les valeurs proposées), et un
/// fichier d'avant le lot 31b se lit sans cible.
#[test]
fn la_fiche_cible_se_retrouve_et_n_est_ecrite_que_si_elle_a_change() {
    let mut f = fiche();
    assert!(
        f.to_value().get("cible").is_none(),
        "cible écrite sans avoir été changée"
    );
    let cible: Vec<(String, String)> = [("latency_ms", "18"), ("peak_low", ""), ("peak_high", "1,15")]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    f.target = Some(cible.clone());

    let dir = dossier("fiche-cible");
    let mut lib = Library::open(dir.clone());
    let mut d = Drone::new("uid-1", "JeNo Ced");
    d.sheet = Some(f.clone());
    lib.put_drone(d);
    lib.save().expect("écriture");
    let relue = Library::open(dir);
    let s = relue.drone("uid-1").unwrap().sheet.clone().unwrap();
    assert_eq!(s.target, Some(cible));
    assert_eq!(s, f);

    let avant = DroneSheet::from_value(&serde_json::json!({"taille": "5″", "cases": []}));
    assert_eq!(avant.target, None);
}

/// Un fichier de drones écrit avant la fiche (lot 28) se lit sans problème, et sans fiche.
#[test]
fn un_fichier_de_drones_d_avant_la_fiche_se_lit() {
    let dir = dossier("fiche-avant");
    std::fs::write(
        dir.join("drones.json"),
        r#"{"version":1,"items":[{"id":"JeNo Ced","nom":"JeNo Ced","firmware":"Betaflight 2026.6.0","vu_le":"2026-11-17","revu_le":"2026-11-17"}]}"#,
    )
    .unwrap();
    let lib = Library::open(dir);
    assert!(lib.problems().is_empty(), "{:?}", lib.problems());
    let d = lib.drone("JeNo Ced").unwrap();
    assert_eq!((d.board.as_str(), d.sheet.as_ref()), ("", None));
}

/// Supprimer un drone retire tout ce qui le désigne, et **rien d'un autre drone**.
///
/// Contre-preuve vue en échec (2026-11-23) : sans la boucle sur `remove_fragment`, le fragment restait dans la
/// liste d'analyse et la troisième assertion rougissait.
#[test]
fn supprimer_un_drone_retire_ses_logs_fragments_mesures_et_configurations() {
    let mut lib = remplie(dossier("suppr-drone"));
    lib.put_drone(Drone::new("uid-0002", "Autre"));
    assert!(lib.add_log(log("emp-bbb", "uid-0002", "AUTRE")));
    lib.put_fragment(fragment("fr-2", "emp-bbb", (0, 10), "2026-11-14"));
    lib.add_to_analysis("fr-2");
    lib.record_config("uid-0002", "2026-11-14T10:00:00", "set p_roll = 50");
    let avant = lib.drones_revision();

    let retires = lib.remove_drone("uid-0001");
    assert_eq!(retires, vec!["fr-1".to_string()]);
    assert!(lib.drone("uid-0001").is_none() && lib.log("emp-aaa").is_none());
    assert!(lib.analysis().items.iter().all(|i| i.fragment_id != "fr-1"));
    assert_eq!(lib.analysis().working, None, "le fragment de travail était fr-1");
    assert!(lib.measure("fr-1", 0, "latence_ms").is_none());
    assert!(lib.configs_of_drone("uid-0001").is_empty());
    assert!(lib.drones_revision() > avant);
    // l'autre drone est intact
    assert!(lib.drone("uid-0002").is_some() && lib.fragment("fr-2").is_some());
    assert_eq!(lib.configs_of_drone("uid-0002").len(), 1);
    assert!(lib.analysis().items.iter().any(|i| i.fragment_id == "fr-2"));
}

/// C'est le nom qui relie un log à sa fiche : un drone renommé se retrouve par son nouveau nom.
#[test]
fn un_drone_se_retrouve_par_son_nom() {
    let mut lib = Library::in_memory();
    let mut d = Drone::new("JeNo", "JeNo");
    d.name = "JeNo Ced".into();
    lib.put_drone(d);
    assert_eq!(lib.drone_by_name("JeNo Ced").unwrap().id, "JeNo");
    assert!(lib.drone_by_name("JeNo").is_none());
}

/// Les changements entre deux `diff all` : réglages modifiés, ajoutés (défaut → valeur) et revenus au défaut,
/// chacun dans sa section — le même nom dans deux profils fait deux réglages.
#[test]
fn deux_configurations_se_comparent_reglage_par_reglage() {
    let avant = "# diff all\nset gyro_lpf1_static_hz = 150\nset dshot_idle_value = 550\nprofile 0\nset p_roll = 31\n\
                 profile 1\nset p_roll = 40\nrateprofile 0\nset roll_rc_rate = 7\n";
    let apres = "# diff all\nset gyro_lpf1_static_hz = 165\nprofile 0\nset p_roll = 72\nset d_roll = 48\n\
                 profile 1\nset p_roll = 40\nrateprofile 0\nset roll_rc_rate = 7\n";
    let c = config_changes(avant, apres);
    let attendu: Vec<(String, String, String)> = [
        ("gyro_lpf1_static_hz", "150", "165"),
        ("profile 0 · p_roll", "31", "72"),
        ("profile 0 · d_roll", "", "48"),
        ("dshot_idle_value", "550", ""),
    ]
    .iter()
    .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
    .collect();
    assert_eq!(c, attendu);
    assert!(config_changes(apres, apres).is_empty());
}

// ---------------------------------------------------------------- table des mesures lue à la demande (D-064, lot 31)

/// Une bibliothèque enregistrée avec une mesure sur « fr-1 » (bornes 0 à 5 000).
fn avec_une_mesure(nom: &str) -> PathBuf {
    let dir = dossier(nom);
    let mut lib = remplie(dir.clone());
    lib.save().expect("écriture");
    dir
}

/// ⚠️ La table des mesures ne se lit pas à l'ouverture, se lit sur un autre fil, et **un enregistrement fait avant
/// sa lecture ne vide pas son fichier**.
///
/// Contre-preuves vues en échec (2026-11-23) : la lecture remise dans `Library::open` fait rougir la première
/// assertion ; sans le `continue` de `Library::save`, `mesures.json` est réécrit vide et la comparaison des octets
/// rougit.
#[test]
fn la_table_des_mesures_se_lit_a_la_demande_et_nest_jamais_videe_avant() {
    let dir = avec_une_mesure("mesures-a-la-demande");
    let avant = std::fs::read(dir.join("mesures.json")).expect("mesures.json écrit");

    let mut lib = Library::open(dir.clone());
    assert!(!lib.measures_loaded());
    assert_eq!(lib.measures_len(), 0, "rien n'est lu à l'ouverture");
    assert!(lib.measure("fr-1", 0, "latence_ms").is_none());
    // un enregistrement avant la lecture (le Guide, la liste d'analyse…) laisse le fichier intact
    lib.set_working(None);
    lib.save().expect("écriture");
    assert_eq!(
        std::fs::read(dir.join("mesures.json")).unwrap(),
        avant,
        "mesures.json intact"
    );

    // la lecture sur un autre fil, puis le rattachement
    let source = lib.measures_source();
    let lue = std::thread::spawn(move || source.read())
        .join()
        .expect("fil de lecture");
    lib.attach_measures(lue);
    assert!(lib.measures_loaded());
    assert_eq!(lib.measure("fr-1", 0, "latence_ms").unwrap().value, 12.5);
}

/// ⚠️ Un fragment déplacé **avant** la lecture de la table perd quand même ses mesures : `put_fragment` ne pouvait
/// pas les jeter, elles n'étaient pas encore lues.
///
/// Contre-preuve vue en échec (2026-11-23) : sans le `retain` d'`attach_measures`, la mesure calculée sur les
/// anciennes bornes ressort.
#[test]
fn un_fragment_deplace_avant_la_lecture_perd_ses_mesures() {
    let dir = avec_une_mesure("deplace-avant-lecture");
    let mut lib = Library::open(dir.clone());
    lib.put_fragment(fragment("fr-1", "emp-aaa", (0, 4_000), "2026-11-13"));
    lib.load_measures();
    assert!(
        lib.measure("fr-1", 0, "latence_ms").is_none(),
        "bornes changées : jetée"
    );
    assert!(!lib.is_measured(lib.fragment("fr-1").unwrap(), 1));
    assert!(lib.history("uid-0001", "latence_ms", 0).is_empty());

    // même chose pour un fragment supprimé avant la lecture
    let mut lib = Library::open(dir);
    assert!(lib.remove_fragment("fr-1"));
    lib.load_measures();
    assert_eq!(lib.measures_len(), 0, "fragment disparu : jetée");
}

/// Une mesure posée avant la lecture est plus récente que le fichier : elle l'emporte. Une seconde lecture ne change
/// plus rien.
#[test]
fn une_mesure_posee_avant_la_lecture_lemporte_sur_le_fichier() {
    let dir = avec_une_mesure("posee-avant-lecture");
    let mut lib = Library::open(dir);
    lib.set_measure(mesure("fr-1", (0, 5_000), 9.0, 1));
    let source = lib.measures_source();
    lib.attach_measures(source.read());
    assert_eq!(lib.measure("fr-1", 0, "latence_ms").unwrap().value, 9.0);
    lib.set_measure(mesure("fr-1", (0, 5_000), 8.0, 1));
    lib.attach_measures(source.read());
    assert_eq!(
        lib.measure("fr-1", 0, "latence_ms").unwrap().value,
        8.0,
        "déjà rattachée : rien relu"
    );
}

/// Un fragment est « mesuré » s'il a une mesure de la version demandée (ou plus récente) sur ses bornes actuelles —
/// y compris une grandeur qui n'a pas pu se mesurer (NaN, écrite `null`) : sans quoi il se recalculerait à chaque
/// ouverture.
#[test]
fn un_fragment_mesure_lest_a_sa_version_et_sur_ses_bornes() {
    let dir = dossier("est-mesure");
    let mut lib = remplie(dir.clone());
    let fr = lib.fragment("fr-1").unwrap().clone();
    assert!(lib.is_measured(&fr, 1));
    assert!(!lib.is_measured(&fr, 2), "version plus récente demandée");
    let mut ailleurs = fr.clone();
    ailleurs.end = 4_000;
    assert!(!lib.is_measured(&ailleurs, 1), "autres bornes");

    lib.set_measure(mesure("fr-1", (0, 5_000), f64::NAN, 2));
    lib.save().expect("écriture");
    let texte = std::fs::read_to_string(dir.join("mesures.json")).unwrap();
    assert!(texte.contains("\"valeur\":null"), "{texte}");
    let mut relue = Library::open(dir);
    relue.load_measures();
    assert!(relue.problems().is_empty(), "{:?}", relue.problems());
    assert!(relue.measure("fr-1", 0, "latence_ms").unwrap().value.is_nan());
    assert!(relue.is_measured(&fr, 2));
}

/// Un fichier de mesures abîmé se découvre à la lecture, pas à l'ouverture, et il est mis de côté comme les autres.
#[test]
fn un_fichier_de_mesures_illisible_est_mis_de_cote_a_la_lecture() {
    let dir = avec_une_mesure("mesures-abimees");
    std::fs::write(dir.join("mesures.json"), "{ pas du JSON").unwrap();
    let mut lib = Library::open(dir.clone());
    assert!(lib.problems().is_empty(), "rien n'est lu à l'ouverture");
    lib.load_measures();
    assert!(
        lib.problems().iter().any(|p| p.contains("mesures.json")),
        "{:?}",
        lib.problems()
    );
    lib.save().expect("écriture");
    let copies: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("mesures.json."))
        .collect();
    assert_eq!(copies.len(), 1, "copie mise de côté : {copies:?}");
}

/// La table des mesures ne se réécrit que si elle a changé : l'enregistrement automatique de l'interface ne doit pas
/// récrire 8 Mo toutes les 30 s.
///
/// Contre-preuve vue en échec (2026-11-23) : sans le drapeau `measures_dirty` dans `Library::save`, le fichier est
/// réécrit et sa date de modification change.
#[test]
fn une_table_des_mesures_inchangee_nest_pas_reecrite() {
    let dir = avec_une_mesure("mesures-inchangees");
    let fichier = dir.join("mesures.json");
    let marque = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    std::fs::File::options()
        .write(true)
        .open(&fichier)
        .unwrap()
        .set_modified(marque)
        .unwrap();
    let mut lib = Library::open(dir);
    lib.load_measures();
    lib.save().expect("écriture");
    assert_eq!(
        std::fs::metadata(&fichier).unwrap().modified().unwrap(),
        marque,
        "réécrite sans changement"
    );
    lib.set_measure(mesure("fr-1", (0, 5_000), 7.0, 1));
    lib.save().expect("écriture");
    assert_ne!(
        std::fs::metadata(&fichier).unwrap().modified().unwrap(),
        marque,
        "changée : réécrite"
    );
}

/// ⚠️ Une mesure relue est **exactement** celle qui a été écrite, au dernier bit : sans cela, le critère d'arrêt du
/// lot 31 (une mesure rangée se recalcule à l'identique) tombe au hasard des valeurs. L'analyseur de nombres par défaut
/// de `serde_json` n'arrondit pas toujours exactement : `55.732080660999316` (un temps de montée calculé sous Windows,
/// CI de la PR #179) revenait avec un bit d'écart (L-116).
///
/// Contre-preuve vue en échec (2026-11-23) : sans l'option `float_roundtrip` de `serde_json` dans
/// `rust/otb-store/Cargo.toml`, cette valeur revient différente, sous Linux comme sous Windows.
#[test]
fn une_mesure_relue_est_identique_au_dernier_bit() {
    let dir = dossier("dernier-bit");
    let mut lib = remplie(dir.clone());
    let valeurs = [55.732080660999316, 0.1 + 0.2, 1.0 / 3.0, 2.2250738585072014e-308, 12.5];
    for (i, &v) in valeurs.iter().enumerate() {
        let mut m = mesure("fr-1", (0, 5_000), v, 1);
        m.key = format!("grandeur_{i}");
        lib.set_measure(m);
    }
    lib.save().expect("écriture");
    let mut relue = Library::open(dir);
    relue.load_measures();
    for (i, &v) in valeurs.iter().enumerate() {
        let lu = relue.measure("fr-1", 0, &format!("grandeur_{i}")).unwrap().value;
        assert_eq!(lu.to_bits(), v.to_bits(), "{v:e} relue {lu:e}");
    }
}
