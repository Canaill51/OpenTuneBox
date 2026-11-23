//! Les fragments vus du côté du cœur : leur période dans un log, ce qui se passe quand le fichier a changé, et la
//! liste d'analyse (ordre, couleur fixe, actif ou non, fragment de travail) — lot 29, première partie.
//!
//! **Contre-preuves** (L-071) notées sous les tests concernés.

use otb_store::Library;
use otb_store::link::{added_fragment_id, is_added, length, overlaps, range_of, session_fragment_id};
use otb_store::model::{Fragment, LogEntry, Measure};
use std::path::{Path, PathBuf};

fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-frag-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dossier d'essai");
    d
}

fn fragment(id: &str, bornes: (u64, u64)) -> Fragment {
    Fragment {
        id: id.to_string(),
        log_id: "emp-aaa".to_string(),
        session: 1,
        start: bornes.0,
        end: bornes.1,
        name: id.to_string(),
        role: "filters".to_string(),
        date: "2026-11-13".to_string(),
        chosen: true,
    }
}

fn log(id: &str) -> LogEntry {
    LogEntry {
        id: id.to_string(),
        drone_id: "uid-1".to_string(),
        name: "FINAL".to_string(),
        path: "/logs/FINAL.bbl".to_string(),
        copied: false,
        bytes: 1024,
        added: "2026-11-18".to_string(),
    }
}

#[test]
fn la_periode_dun_fragment_est_celle_de_ses_bornes() {
    let f = fragment("fr-1", (1_000, 4_000));
    assert_eq!(range_of(&f, 10_000).unwrap(), 1_000..4_000);
    assert_eq!(length(&f), 3_000);
    // la fin peut toucher le dernier échantillon
    assert_eq!(range_of(&fragment("fr-2", (0, 10_000)), 10_000).unwrap(), 0..10_000);
}

/// ⚠️ Le critère d'arrêt du lot 29 : un fragment qui ne retrouve pas son log doit le **dire**, pas rendre une
/// période fausse. Le message nomme le fragment et les deux nombres, parce qu'il finit dans la barre d'état.
///
/// **Contre-preuve** : en remplaçant les deux contrôles de `range_of` par un simple
/// `Ok(start.min(samples)..end.min(samples))`, le fragment rend 0..900 au lieu d'une erreur et les deux
/// assertions rougissent.
#[test]
fn un_fragment_plus_long_que_son_log_le_dit_clairement() {
    let f = fragment("fr-1", (0, 4_000));
    let e = range_of(&f, 900).unwrap_err();
    assert!(e.contains("fr-1") && e.contains("4000") && e.contains("900"), "{e}");
    assert!(e.contains("le fichier a changé"), "{e}");

    let tardif = fragment("fr-2", (5_000, 6_000));
    let e = range_of(&tardif, 900).unwrap_err();
    assert!(e.contains("il commence à l'échantillon 5000"), "{e}");
}

#[test]
fn des_bornes_impossibles_sont_refusees() {
    assert!(range_of(&fragment("fr-1", (500, 500)), 10_000).is_err(), "vide");
    assert!(range_of(&fragment("fr-2", (900, 500)), 10_000).is_err(), "à l'envers");
    // le message reste lisible même sans nom
    let mut sans_nom = fragment("fr-3", (900, 500));
    sans_nom.name = String::new();
    assert!(range_of(&sans_nom, 10_000).unwrap_err().contains("fr-3"));
}

/// Deux fragments du même passage se savent chevauchants (pour prévenir, jamais pour refuser).
#[test]
fn le_chevauchement_se_voit_dans_le_meme_log_et_la_meme_session() {
    let a = fragment("fr-a", (0, 1_000));
    assert!(overlaps(&a, &fragment("fr-b", (500, 1_500))));
    assert!(!overlaps(&a, &fragment("fr-c", (1_000, 2_000))), "bout à bout");
    let mut autre_session = fragment("fr-d", (500, 1_500));
    autre_session.session = 2;
    assert!(!overlaps(&a, &autre_session));
    let mut autre_log = fragment("fr-e", (500, 1_500));
    autre_log.log_id = "emp-bbb".into();
    assert!(!overlaps(&a, &autre_log));
}

/// ⚠️ Une courbe doit garder sa couleur d'un écran à l'autre : l'indice est attribué à l'entrée dans la liste et
/// ne bouge plus. Un indice libéré est repris par le suivant, pour ne pas s'éloigner indéfiniment de la palette.
///
/// **Contre-preuve** : en attribuant `items.len()` comme indice au lieu du plus petit libre, le quatrième
/// fragment prend 3 au lieu de 1 et l'assertion rougit.
#[test]
fn la_couleur_dun_fragment_ne_change_pas_tant_quil_reste_dans_la_liste() {
    let mut lib = Library::in_memory();
    for id in ["fr-1", "fr-2", "fr-3"] {
        assert!(lib.add_to_analysis(id));
    }
    assert!(!lib.add_to_analysis("fr-2"), "déjà dans la liste");
    let couleur = |lib: &Library, id: &str| {
        lib.analysis()
            .items
            .iter()
            .find(|i| i.fragment_id == id)
            .map(|i| i.colour)
    };
    assert_eq!(couleur(&lib, "fr-1"), Some(0));
    assert_eq!(couleur(&lib, "fr-2"), Some(1));
    assert_eq!(couleur(&lib, "fr-3"), Some(2));

    lib.remove_from_analysis("fr-2");
    assert_eq!(couleur(&lib, "fr-1"), Some(0), "les autres gardent la leur");
    assert_eq!(couleur(&lib, "fr-3"), Some(2));
    lib.add_to_analysis("fr-4");
    assert_eq!(couleur(&lib, "fr-4"), Some(1), "l'indice libéré est repris");
}

#[test]
fn deplacer_un_fragment_change_lordre_des_courbes() {
    let mut lib = Library::in_memory();
    for id in ["fr-1", "fr-2", "fr-3"] {
        lib.add_to_analysis(id);
    }
    assert!(lib.move_in_analysis("fr-3", 0));
    let ordre: Vec<&str> = lib.analysis().items.iter().map(|i| i.fragment_id.as_str()).collect();
    assert_eq!(ordre, vec!["fr-3", "fr-1", "fr-2"]);
    assert_eq!(lib.analysis().items[0].colour, 2, "déplacer ne change pas la couleur");
    // une place au-delà de la fin met à la fin, sans paniquer
    assert!(lib.move_in_analysis("fr-3", 99));
    let ordre: Vec<&str> = lib.analysis().items.iter().map(|i| i.fragment_id.as_str()).collect();
    assert_eq!(ordre, vec!["fr-1", "fr-2", "fr-3"]);
    assert!(!lib.move_in_analysis("fr-inconnu", 0));
}

#[test]
fn cacher_un_fragment_lui_garde_sa_place_et_sa_couleur() {
    let mut lib = Library::in_memory();
    lib.add_to_analysis("fr-1");
    lib.add_to_analysis("fr-2");
    assert!(lib.set_active("fr-1", false));
    assert!(!lib.analysis().items[0].active);
    assert_eq!(lib.analysis().items[0].fragment_id, "fr-1", "il reste en place");
    assert_eq!(lib.analysis().items[0].colour, 0);
    assert!(!lib.set_active("fr-inconnu", false));
}

/// ⚠️ Le fragment de travail désigne celui dont l'interface montre les réglages : hors de la liste d'analyse, il
/// montrerait des réglages qui ne correspondent à aucune courbe.
///
/// **Contre-preuve** : en acceptant n'importe quel identifiant dans `set_working`, les deux dernières assertions
/// rougissent.
#[test]
fn le_fragment_de_travail_reste_dans_la_liste() {
    let mut lib = Library::in_memory();
    lib.add_to_analysis("fr-1");
    assert!(lib.set_working(Some("fr-1")));
    assert_eq!(lib.analysis().working.as_deref(), Some("fr-1"));
    // le retirer de la liste l'oublie
    lib.remove_from_analysis("fr-1");
    assert_eq!(lib.analysis().working, None);
    // et on ne peut pas désigner un fragment absent
    assert!(!lib.set_working(Some("fr-9")));
    assert_eq!(lib.analysis().working, None);
    assert!(
        lib.set_working(None),
        "oublier le fragment de travail est toujours permis"
    );
}

/// Un fragment dont le log n'est plus connu est **signalé**, pas affiché comme analysable.
#[test]
fn un_fragment_dont_le_log_est_inconnu_est_signale() {
    let mut lib = Library::in_memory();
    lib.add_log(log("emp-aaa"));
    lib.put_fragment(fragment("fr-1", (0, 100)));
    let mut perdu = fragment("fr-2", (0, 100));
    perdu.log_id = "emp-disparu".into();
    lib.put_fragment(perdu);
    let orphelins = lib.orphan_fragments();
    assert_eq!(orphelins.len(), 1);
    assert_eq!(orphelins[0].id, "fr-2");
}

/// La liste d'analyse survit au redémarrage, avec ses couleurs, son ordre et son fragment de travail.
#[test]
fn la_liste_danalyse_se_retrouve_apres_redemarrage() {
    let dir = dossier("liste");
    let mut lib = Library::open(dir.clone());
    for id in ["fr-1", "fr-2", "fr-3"] {
        lib.add_to_analysis(id);
    }
    lib.set_active("fr-2", false);
    lib.move_in_analysis("fr-3", 0);
    lib.set_working(Some("fr-1"));
    lib.save().expect("écriture");

    let relue = Library::open(dir);
    assert!(relue.problems().is_empty(), "{:?}", relue.problems());
    let items = &relue.analysis().items;
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].fragment_id, "fr-3");
    assert_eq!(items[0].colour, 2);
    assert!(!items.iter().find(|i| i.fragment_id == "fr-2").unwrap().active);
    assert_eq!(relue.analysis().working.as_deref(), Some("fr-1"));
}

/// ⚠️ **La preuve demandée par le plan**, sur un vrai log : un fragment relu après redémarrage rend exactement
/// les mêmes échantillons. La contre-preuve est dans le test : une borne décalée d'**un seul échantillon** donne
/// une tranche différente, donc ce test verrait une dérive d'un échantillon.
#[test]
fn un_fragment_dun_vrai_log_rend_les_memes_echantillons_apres_redemarrage() {
    let chemin = format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"));
    let logs = otb_log::flightlog::load(Path::new(&chemin)).expect("vrai log");
    let lg = &logs[0];
    let n = lg.n();
    assert!(n > 3_000, "le log d'essai doit être assez long ({n})");
    let (a, b) = (n / 3, 2 * n / 3);

    let dir = dossier("vrai-log");
    let mut lib = Library::open(dir.clone());
    lib.add_log(log("emp-jeno"));
    let mut f = fragment("fr-milieu", (a as u64, b as u64));
    f.log_id = "emp-jeno".into();
    lib.put_fragment(f);
    lib.save().expect("écriture");

    let relue = Library::open(dir);
    let periode = range_of(relue.fragment("fr-milieu").unwrap(), n).expect("période");
    assert_eq!(periode, a..b, "les bornes relues sont celles enregistrées");

    let attendu: Vec<f32> = lg.gyro.row(0)[a..b].to_vec();
    assert_eq!(lg.gyro.row(0)[periode.clone()].to_vec(), attendu, "mêmes échantillons");
    // contre-preuve, dans le test : un échantillon de décalage se verrait
    assert_ne!(lg.gyro.row(0)[a..b - 1].to_vec(), attendu);
}

/// ⚠️ Un fragment écrit avant le lot 29b n'a pas de champ « bornes_choisies » : il se lit comme **non choisi**, et ses
/// bornes suivront donc les limites automatiques à la prochaine ouverture. Un fragment choisi le reste après relecture.
///
/// **Contre-preuve** : lire le champ manquant comme `true` (`flag(v, "bornes_choisies", true)`) rougit la première
/// assertion.
#[test]
fn des_bornes_choisies_se_relisent_et_un_ancien_fragment_ne_lest_pas() {
    let ancien = serde_json::json!({
        "id": "emp-aaa-1", "log": "emp-aaa", "session": 1, "debut": 0, "fin": 5000,
        "nom": "FINAL", "role": "", "date": "2026-11-19"
    });
    let lu = Fragment::from_value(&ancien).expect("fragment lisible");
    assert!(!lu.chosen, "un fragment d'avant le lot 29b n'a pas de bornes choisies");

    let choisi = fragment("FINAL.02", (120, 4_000));
    assert!(choisi.chosen);
    assert_eq!(Fragment::from_value(&choisi.to_value()).expect("relu"), choisi);
}

/// Lot 29, second fragment d'une session : un fragment ajouté prend le premier numéro libre, et seul lui est
/// « ajouté » (le fragment de la session revient à chaque ouverture du fichier, il ne se supprime pas).
///
/// Contre-preuve (L-071) : `added_fragment_id` sans le filtre des numéros pris rend `-2` deux fois, et le second
/// fragment écraserait le premier au rangement — l'assertion `-3` échoue.
#[test]
fn un_fragment_ajoute_prend_un_numero_libre_et_se_distingue_de_celui_de_la_session() {
    let principal = Fragment {
        id: session_fragment_id("emp-aaa", 1),
        ..fragment("x", (0, 10))
    };
    assert_eq!(principal.id, "emp-aaa-1");
    assert!(!is_added(&principal));
    let deux = Fragment {
        id: added_fragment_id("emp-aaa", 1, [&principal]),
        ..fragment("x", (2, 8))
    };
    assert_eq!(deux.id, "emp-aaa-1-2");
    assert!(is_added(&deux));
    assert_eq!(added_fragment_id("emp-aaa", 1, [&principal, &deux]), "emp-aaa-1-3");
    // une autre session du même log a sa propre suite
    assert_eq!(added_fragment_id("emp-aaa", 2, [&principal, &deux]), "emp-aaa-2-2");
}

/// Supprimer un fragment emporte ses mesures et sa place dans la liste d'analyse (sinon le lancement suivant
/// rouvrirait un fantôme), sans toucher aux autres.
///
/// Contre-preuve (L-071) : `remove_fragment` sans `remove_from_analysis` laisse « f-2 » dans la liste relue.
#[test]
fn supprimer_un_fragment_emporte_ses_mesures_et_sa_place_dans_la_liste() {
    let dir = dossier("suppression");
    let mut lib = Library::open(dir.clone());
    lib.add_log(log("emp-aaa"));
    lib.put_fragment(fragment("f-1", (0, 10)));
    lib.put_fragment(fragment("f-2", (2, 8)));
    for f in ["f-1", "f-2"] {
        lib.add_to_analysis(f);
        lib.set_measure(Measure {
            fragment_id: f.to_string(),
            axis: 0,
            key: "latence_ms".to_string(),
            value: 4.0,
            settings: "r".to_string(),
            calc_version: 1,
        });
    }
    lib.set_working(Some("f-2"));
    assert!(lib.remove_fragment("f-2"));
    assert!(!lib.remove_fragment("f-2"), "déjà parti");
    lib.save().expect("écriture");

    let relue = Library::open(dir);
    assert!(relue.problems().is_empty(), "{:?}", relue.problems());
    assert!(relue.fragment("f-2").is_none());
    assert!(relue.fragment("f-1").is_some());
    assert!(relue.measure("f-2", 0, "latence_ms").is_none());
    assert!(relue.measure("f-1", 0, "latence_ms").is_some());
    let ids: Vec<&str> = relue.analysis().items.iter().map(|i| i.fragment_id.as_str()).collect();
    assert_eq!(ids, ["f-1"]);
    assert_eq!(relue.analysis().working, None);
}
