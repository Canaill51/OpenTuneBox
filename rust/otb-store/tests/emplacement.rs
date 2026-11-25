//! L'emplacement de la bibliothèque au choix et son déplacement sûr (lot 34, D-081) : pointeur, dossier introuvable,
//! chemins relatifs des logs copiés, copie relue, échec à mi-copie, bibliothèque déjà présente, témoin des deux postes.
//!
//! **Critère d'arrêt du lot** : un seul fichier perdu ou un log copié introuvable après un déplacement.
//!
//! **Contre-preuves** (L-071) : notées sous chaque test, avec ce qui échoue quand on casse la protection visée.

use otb_store::model::{Drone, Fragment, LogEntry};
use otb_store::place::{self, Place, Target, Witness};
use otb_store::{Library, copied_path_in};
use std::path::{Path, PathBuf};

/// Un dossier d'essai propre, hors du dépôt ; le nom porte le processus (L-093 : un dossier par test).
fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-emplacement-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dossier d'essai");
    d
}

/// Une bibliothèque écrite dans `dir` : un drone, deux logs **copiés** (dont un dans un sous-dossier de drone) et
/// un log ouvert sur place, un fragment.
fn bibliotheque(dir: &Path, ailleurs: &Path) -> Library {
    let mut lib = Library::open(dir.to_path_buf());
    lib.put_drone(Drone::new("jeno", "JeNo 5"));
    for (id, stamp, octets) in [
        ("a", "2026-11-24_10-00-00", 300_000usize),
        ("b", "2026-11-24_11-00-00", 70_000),
    ] {
        let path = lib.copy_path("JeNo 5", stamp).expect("dossier").expect("créé");
        std::fs::write(&path, vec![id.as_bytes()[0]; octets]).expect("log copié");
        lib.add_log(LogEntry {
            id: id.into(),
            drone_id: "jeno".into(),
            name: format!("{stamp}.bbl"),
            path: path.to_string_lossy().into_owned(),
            copied: true,
            bytes: octets as u64,
            added: "2026-11-24T10:00:00".into(),
        });
    }
    let sur_place = ailleurs.join("vol.bbl");
    std::fs::write(&sur_place, b"log de l'utilisateur").expect("log sur place");
    lib.add_log(LogEntry {
        id: "c".into(),
        drone_id: "jeno".into(),
        name: "vol.bbl".into(),
        path: sur_place.to_string_lossy().into_owned(),
        copied: false,
        bytes: 20,
        added: "2026-11-24T12:00:00".into(),
    });
    lib.put_fragment(Fragment {
        id: "f".into(),
        log_id: "a".into(),
        session: 1,
        start: 0,
        end: 100,
        name: "vol 1".into(),
        role: String::new(),
        date: "2026-11-24".into(),
        chosen: true,
    });
    lib.save().expect("écrite");
    lib
}

/// Chaque fichier de `a` existe dans `b` avec le même contenu (chemins relatifs).
fn fichiers(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![root.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).expect("dossier lisible") {
            let p = e.expect("entrée").path();
            if p.is_dir() {
                todo.push(p);
            } else {
                out.push((
                    p.strip_prefix(root).expect("dans root").to_path_buf(),
                    std::fs::read(&p).expect("lu"),
                ));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn sans_pointeur_la_bibliotheque_est_a_sa_place_par_defaut() {
    let base = dossier("defaut");
    let defaut = base.join("OpenTuneBox").join("bibliotheque");
    assert_eq!(
        place::locate(&defaut),
        Place::Ready {
            dir: defaut.clone(),
            chosen: false
        }
    );
    // pointer vers le dossier par défaut revient à n'avoir pas de pointeur
    place::set_pointer(&defaut, &defaut).expect("rien à retirer");
    assert!(!base.join("OpenTuneBox").join(place::POINTER).exists());
}

#[test]
fn un_dossier_introuvable_ne_devient_jamais_une_bibliotheque_vide() {
    let base = dossier("introuvable");
    let defaut = base.join("donnees").join("bibliotheque");
    let choisi = base.join("disque-externe").join("OpenTuneBox");
    std::fs::create_dir_all(&choisi).expect("créé");
    bibliotheque(&choisi, &base);
    place::set_pointer(&defaut, &choisi).expect("pointeur");
    assert_eq!(
        place::locate(&defaut),
        Place::Ready {
            dir: choisi.clone(),
            chosen: true
        }
    );
    // disque débranché
    std::fs::rename(&choisi, base.join("ailleurs")).expect("débranché");
    match place::locate(&defaut) {
        Place::Missing { dir, why } => {
            assert_eq!(dir.as_deref(), Some(choisi.as_path()));
            assert!(why.contains("débranché"), "{why}");
        }
        autre => panic!("dossier absent pris pour une bibliothèque : {autre:?}"),
    }
    // le dossier revient, mais vide : ce n'est pas une bibliothèque
    std::fs::create_dir_all(&choisi).expect("recréé vide");
    assert!(matches!(place::locate(&defaut), Place::Missing { .. }));
    // un pointeur abîmé non plus ne renvoie pas au dossier par défaut en silence
    std::fs::write(base.join("donnees").join(place::POINTER), "{pas du json").expect("abîmé");
    assert!(matches!(place::locate(&defaut), Place::Missing { dir: None, .. }));
    // Contre-preuve : `locate` sans ses deux contrôles (`is_dir`, `is_library`) → « dossier absent pris pour une
    // bibliothèque » ; sans le seul `is_dir` → le message ne parle plus de disque débranché.
}

#[test]
fn les_logs_copies_s_ecrivent_relatifs_a_la_bibliotheque() {
    let base = dossier("relatifs");
    let a = base.join("A");
    bibliotheque(&a, &base);
    let texte = std::fs::read_to_string(a.join("logs.json")).expect("logs.json");
    assert!(texte.contains("\"logs/JeNo_5/2026-11-24_10-00-00.bbl\""), "{texte}");
    assert!(
        !texte.contains(&*a.to_string_lossy().replace('\\', "\\\\")),
        "chemin complet d'un log copié resté dans logs.json : {texte}"
    );
    // le log ouvert sur place garde son chemin complet
    assert!(texte.contains("vol.bbl"));
    // le dossier copié à la main ailleurs : les logs copiés s'y retrouvent
    let b = base.join("B");
    std::fs::rename(&a, &b).expect("déplacé à la main");
    let lib = Library::open(b.clone());
    for l in lib.logs().iter().filter(|l| l.copied) {
        assert!(Path::new(&l.path).starts_with(&b), "{}", l.path);
        assert!(Path::new(&l.path).is_file(), "log copié introuvable : {}", l.path);
    }
    assert_eq!(
        lib.log("c").expect("sur place").path,
        base.join("vol.bbl").to_string_lossy()
    );
    // Contre-preuve : `stored_log` rendu sans changement (chemins complets) → la première assertion échoue.
}

#[test]
fn un_ancien_chemin_complet_se_retrouve_dans_le_nouveau_dossier() {
    let b = Path::new("/media/disque/OpenTuneBox");
    let attendu = b
        .join("logs")
        .join("JeNo_5")
        .join("x.bbl")
        .to_string_lossy()
        .into_owned();
    // écrit sous Windows avant le lot 34
    let windows = r"C:\Users\p\AppData\Roaming\OpenTuneBox\OpenTuneBox\bibliotheque\logs\JeNo_5\x.bbl";
    assert_eq!(copied_path_in(b, windows), attendu);
    // écrit sous Linux, autre dossier
    let linux = "/home/p/.local/share/OpenTuneBox/OpenTuneBox/bibliotheque/logs/JeNo_5/x.bbl";
    assert_eq!(copied_path_in(b, linux), attendu);
    // écrit relatif (lot 34)
    assert_eq!(copied_path_in(b, "logs/JeNo_5/x.bbl"), attendu);
    // déjà dans ce dossier : inchangé
    assert_eq!(copied_path_in(b, &attendu), attendu);
    // une forme inconnue n'est pas inventée
    assert_eq!(copied_path_in(b, "/ailleurs/x.bbl"), "/ailleurs/x.bbl");
}

#[test]
fn un_deplacement_garde_chaque_fichier_et_chaque_log_copie() {
    let base = dossier("deplacement");
    let defaut = base.join("donnees").join("bibliotheque");
    let mut lib = bibliotheque(&defaut, &base);
    let avant = fichiers(&defaut);
    let logs_avant = lib.logs().to_vec();

    let choisi = base.join("disque");
    std::fs::create_dir_all(&choisi).expect("choisi");
    std::fs::write(choisi.join("autre.txt"), "à l'utilisateur").expect("dossier non vide");
    let Target::New(cible) = place::target_of(&defaut, &choisi).expect("cible") else {
        panic!("dossier sans bibliothèque pris pour une bibliothèque");
    };
    assert_eq!(cible, choisi.join(place::SUBFOLDER), "dossier non vide : sous-dossier");

    let mut vu = (0, 0);
    let copie = place::copy_library(&defaut, &cible, &mut |f, t| vu = (f, t)).expect("copie");
    assert_eq!(copie.files, avant.len());
    assert_eq!(vu.0, vu.1, "avancement complet");
    // tant que la marque est là, la copie n'est pas une bibliothèque
    assert!(!place::is_library(&cible));
    lib.relocate(cible.clone());
    lib.save().expect("écrite dans le nouveau dossier");
    place::finish_move(&cible).expect("marque retirée");
    place::set_pointer(&defaut, &cible).expect("pointeur");
    place::remove_old(&defaut).expect("ancien effacé");

    assert!(!defaut.exists());
    assert_eq!(fichiers(&cible), avant, "un fichier perdu ou changé");
    assert_eq!(
        place::locate(&defaut),
        Place::Ready {
            dir: cible.clone(),
            chosen: true
        }
    );
    let relue = Library::open(cible.clone());
    assert_eq!(relue.logs().len(), logs_avant.len());
    for l in relue.logs() {
        assert!(
            Path::new(&l.path).is_file(),
            "log introuvable après déplacement : {}",
            l.path
        );
    }
    assert_eq!(relue.fragments().len(), 1);
    assert_eq!(
        std::fs::read_to_string(choisi.join("autre.txt")).expect("intact"),
        "à l'utilisateur"
    );
    // Contre-preuve : `relocate` sans réécrire les chemins des logs copiés → `logs.json` les écrit complets vers
    // l'ancien dossier, effacé : « log introuvable après déplacement ».
}

#[test]
fn une_copie_qui_echoue_a_mi_chemin_laisse_l_ancienne_bibliotheque_seule_en_service() {
    let base = dossier("echec");
    let a = base.join("A");
    bibliotheque(&a, &base);
    let avant = fichiers(&a);
    let b = base.join("B");
    // un dossier au nom d'un fichier de table fait échouer la copie de ce fichier, sur tous les systèmes
    std::fs::create_dir_all(b.join("logs.json")).expect("obstacle");
    let err = place::copy_library(&a, &b, &mut |_, _| {}).expect_err("la copie doit échouer");
    assert!(err.contains("reste à son emplacement"), "{err}");
    assert_eq!(fichiers(&a), avant, "l'original a changé");
    assert!(!place::is_library(&b));
    assert!(!b.join(place::MOVING).exists(), "marque laissée");
    assert!(!b.join("logs").exists(), "copie partielle laissée");
    assert!(b.join("logs.json").is_dir(), "ce qui était là a été touché");
    // l'ancien dossier refuse toujours d'être effacé s'il n'est pas une bibliothèque
    assert!(place::remove_old(&base.join("rien")).is_err());
}

#[test]
fn une_relecture_differente_fait_echouer_le_deplacement() {
    // la comparaison est faite après la copie : on la prouve en corrompant la source pendant l'avancement
    let base = dossier("relecture");
    let a = base.join("A");
    bibliotheque(&a, &base);
    let b = base.join("B");
    let log = a.join("logs").join("JeNo_5").join("2026-11-24_10-00-00.bbl");
    let mut touche = false;
    let err = place::copy_library(&a, &b, &mut |fait, total| {
        // fin de la copie, avant la relecture : le fichier change sous nos pieds
        if fait * 2 == total && !touche {
            // même taille : seule la comparaison du contenu peut le voir
            std::fs::write(&log, vec![b'z'; 300_000]).expect("changé");
            touche = true;
        }
    })
    .expect_err("une copie différente de l'original doit être refusée");
    assert!(err.contains("diffère"), "{err}");
    assert!(!place::is_library(&b));
    // Contre-preuve : `same_content` qui rend toujours `true` → `expect_err` panique.
}

#[test]
fn une_bibliotheque_deja_presente_est_proposee_telle_quelle() {
    let base = dossier("existante");
    let actuelle = base.join("actuelle");
    bibliotheque(&actuelle, &base);
    let autre = base.join("autre-pc");
    bibliotheque(&autre.join(place::SUBFOLDER), &base);
    assert_eq!(
        place::target_of(&actuelle, &autre),
        Ok(Target::Existing(autre.join(place::SUBFOLDER)))
    );
    assert_eq!(
        place::target_of(&actuelle, &autre.join(place::SUBFOLDER)),
        Ok(Target::Existing(autre.join(place::SUBFOLDER)))
    );
    // l'actuelle elle-même
    assert!(place::target_of(&actuelle, &actuelle).is_err());
    // un dossier vide reçoit la bibliothèque directement
    let vide = base.join("vide");
    std::fs::create_dir_all(&vide).expect("vide");
    assert_eq!(place::target_of(&actuelle, &vide), Ok(Target::New(vide.clone())));
    // une copie interrompue (marque présente) n'est pas une bibliothèque : elle sera refaite
    std::fs::write(vide.join(place::MOVING), "").expect("marque");
    std::fs::write(vide.join("drones.json"), "{}").expect("table partielle");
    assert_eq!(place::target_of(&actuelle, &vide), Ok(Target::New(vide.clone())));
    // un sous-dossier OpenTuneBox occupé par autre chose : refus
    let occupe = base.join("occupe");
    std::fs::create_dir_all(occupe.join(place::SUBFOLDER)).expect("occupé");
    std::fs::write(occupe.join("a.txt"), "x").expect("non vide");
    std::fs::write(occupe.join(place::SUBFOLDER).join("b.txt"), "x").expect("non vide");
    assert!(place::target_of(&actuelle, &occupe).is_err());
    // dans la bibliothèque actuelle : refus
    assert!(place::target_of(&actuelle, &actuelle.join("logs")).is_err());
}

#[test]
fn les_ecritures_suspendues_ne_touchent_pas_au_disque() {
    let base = dossier("pause");
    let a = base.join("A");
    let mut lib = bibliotheque(&a, &base);
    let avant = fichiers(&a);
    lib.pause_writes(true);
    lib.put_drone(Drone::new("autre", "Autre"));
    lib.save().expect("rien à écrire");
    assert_eq!(fichiers(&a), avant);
    lib.pause_writes(false);
    lib.save().expect("écrite");
    assert_ne!(fichiers(&a), avant);
}

#[test]
fn le_temoin_previent_d_une_autre_session_recente() {
    let mien = Witness {
        host: "atelier".into(),
        session: "1".into(),
        seen_s: 10_000,
    };
    let autre_pc = Witness {
        host: "portable".into(),
        session: "2".into(),
        seen_s: 10_000 - 120,
    };
    let message = autre_pc.warning(&mien).expect("avertissement");
    assert!(
        message.contains("« portable »") && message.contains("2 min"),
        "{message}"
    );
    // une autre fenêtre de ce poste
    let fenetre = Witness {
        host: "atelier".into(),
        ..autre_pc.clone()
    };
    assert!(fenetre.warning(&mien).expect("avertissement").contains("autre fenêtre"));
    // ancien : un plantage ne bloque rien
    let ancien = Witness {
        seen_s: 10_000 - place::WITNESS_FRESH_S - 1,
        ..autre_pc.clone()
    };
    assert_eq!(ancien.warning(&mien), None);
    // le sien
    assert_eq!(mien.warning(&mien), None);
}

#[test]
fn le_temoin_s_ecrit_a_l_ouverture_et_se_retire_a_la_fermeture() {
    let base = dossier("temoin");
    let a = base.join("A");
    bibliotheque(&a, &base);
    Witness {
        host: "portable".into(),
        session: "autre".into(),
        seen_s: place::now_s(),
    }
    .write(&a)
    .expect("témoin d'un autre poste");
    let mut lib = Library::open(a.clone());
    assert!(lib.claim().is_some_and(|m| m.contains("portable")));
    let mien = Witness::read(&a).expect("témoin récrit");
    assert_ne!(mien.session, "autre");
    lib.save().expect("écrite");
    assert_eq!(Witness::read(&a).map(|w| w.session), Some(mien.session.clone()));
    lib.release();
    assert!(!a.join(place::WITNESS).exists());
    // une bibliothèque qui n'a rien réclamé (ligne de commande) n'écrit pas de témoin
    let mut cli = Library::open(a.clone());
    cli.save().expect("écrite");
    assert!(!a.join(place::WITNESS).exists());
}
