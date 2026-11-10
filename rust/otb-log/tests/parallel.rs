//! Plafond de fichiers décodés en même temps et ordre des résultats (D-051, lot 11).
//!
//! Le plafond se prouve par un **compteur d'appels en cours**, pas par une mesure de mémoire (D-051) : `map_bounded`
//! est donc testée avec une fonction qui compte, sans décoder de vrai fichier.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use otb_log::parallel::{MAX_PARALLEL_FILES, load_many, map_bounded};

/// Compte les appels en cours et retient le maximum atteint.
struct Compteur {
    en_cours: AtomicUsize,
    maximum: AtomicUsize,
}

impl Compteur {
    fn new() -> Self {
        Self {
            en_cours: AtomicUsize::new(0),
            maximum: AtomicUsize::new(0),
        }
    }

    fn entrer(&self) {
        let n = self.en_cours.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(n, Ordering::SeqCst);
    }

    fn sortir(&self) {
        self.en_cours.fetch_sub(1, Ordering::SeqCst);
    }
}

/// ⚠️ Sans plafond, ouvrir dix logs d'un coup demanderait près de 2 Go (180 Mo par fichier décodé, D-051).
#[test]
fn jamais_plus_de_trois_fichiers_en_meme_temps() {
    let c = Compteur::new();
    let items: Vec<usize> = (0..12).collect();
    let out = map_bounded(&items, MAX_PARALLEL_FILES, |i| {
        c.entrer();
        std::thread::sleep(Duration::from_millis(20)); // assez long pour que les fils se chevauchent
        c.sortir();
        *i
    });
    assert_eq!(out, items, "l'ordre des résultats est celui des éléments");
    let max = c.maximum.load(Ordering::SeqCst);
    assert!(
        max <= MAX_PARALLEL_FILES,
        "{max} fichiers en même temps, plafond {MAX_PARALLEL_FILES}"
    );
    assert!(
        max > 1,
        "rien ne s'est passé en parallèle : le test ne prouverait pas le plafond"
    );
}

/// L'ordre ne doit rien devoir à la durée de chaque fichier : le premier élément est le plus lent.
#[test]
fn l_ordre_est_garde_quelles_que_soient_les_durees() {
    let durees = [120u64, 1, 1, 1, 60, 1, 1, 30];
    let out = map_bounded(&durees, MAX_PARALLEL_FILES, |d| {
        std::thread::sleep(Duration::from_millis(*d));
        *d
    });
    assert_eq!(out, durees);
}

/// Plafond de 1 : tout séquentiel, aucun fil lancé (c'est le mode du banc « avant »).
#[test]
fn un_plafond_de_un_reste_sequentiel() {
    let c = Compteur::new();
    let ordre = Mutex::new(Vec::new());
    let items: Vec<usize> = (0..6).collect();
    map_bounded(&items, 1, |i| {
        c.entrer();
        ordre.lock().unwrap().push(*i);
        c.sortir();
    });
    assert_eq!(c.maximum.load(Ordering::SeqCst), 1);
    assert_eq!(*ordre.lock().unwrap(), items, "les éléments sont traités dans l'ordre");
}

/// Un plafond plus grand que le nombre d'éléments ne lance pas de fils inutiles.
#[test]
fn le_plafond_ne_depasse_pas_le_nombre_d_elements() {
    let c = Compteur::new();
    map_bounded(&[7usize], MAX_PARALLEL_FILES, |_| {
        c.entrer();
        c.sortir();
    });
    assert_eq!(c.maximum.load(Ordering::SeqCst), 1);
}

fn donnees() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl")
}

/// Un fichier illisible n'empêche pas les autres, et chaque résultat garde son chemin (message de l'interface).
#[test]
fn un_fichier_illisible_n_empeche_pas_les_autres() {
    let absent = PathBuf::from("/inexistant/absent.bbl");
    let paths = vec![absent.clone(), donnees(), absent.clone(), donnees()];
    let out = load_many(&paths);
    assert_eq!(out.len(), 4);
    let chemins: Vec<&PathBuf> = out.iter().map(|(p, _)| p).collect();
    assert_eq!(
        chemins,
        paths.iter().collect::<Vec<_>>(),
        "chaque résultat garde son chemin, dans l'ordre"
    );
    assert!(out[0].1.is_err() && out[2].1.is_err());
    assert_eq!(out[1].1.as_ref().expect("log lu").len(), 1);
    assert_eq!(out[3].1.as_ref().expect("log lu").len(), 1);
}
