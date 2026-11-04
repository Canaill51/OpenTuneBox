//! `packaging/linux/install_linux.sh` : la mise à jour d'un programme déjà installé qui tourne (relecture de la PR #93).
//! `cp` directement sur un programme en cours d'exécution échoue (« Fichier texte occupé »), le script copie à côté puis fait un `mv`.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn racine() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn dossier_jetable(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-install-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn installe(home: &Path, programme: &Path) -> std::process::Output {
    Command::new("sh")
        .arg(racine().join("packaging/linux/install_linux.sh"))
        .arg(programme)
        .env("HOME", home)
        .output()
        .unwrap()
}

fn nouveau_programme(dossier: &Path, contenu: &str) -> PathBuf {
    let p = dossier.join("nouveau");
    std::fs::write(&p, contenu).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// Lance un programme qui vient d'être copié, en réessayant tant que Linux le dit « Text file busy » (`ETXTBSY`).
/// ⚠️ Sans ce réessai, le test échoue par intermittence (L-082) : les tests de ce fichier tournent en parallèle dans le même
/// processus et chacun lance des processus (le script d'installation) ; un enfant hérite du descripteur d'écriture de la copie
/// le temps du `fork`, et le noyau refuse alors d'exécuter le fichier. Le réessai ne touche à aucune assertion.
fn lance_malgre_etxtbsy(programme: &Path, args: &[&str]) -> std::process::Child {
    for _ in 0..100 {
        match Command::new(programme).args(args).stdout(Stdio::null()).spawn() {
            Ok(enfant) => return enfant,
            Err(e) if e.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => panic!("lancement impossible : {e}"),
        }
    }
    panic!(
        "programme toujours « Text file busy » après 2 s : {}",
        programme.display()
    );
}

#[test]
fn la_mise_a_jour_marche_meme_si_le_programme_installe_tourne() {
    let d = dossier_jetable("maj");
    let home = d.join("home");
    let bin = home.join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    // un vrai exécutable (pas un script ni un binaire « multi-appel ») qui reste en cours d'exécution
    let installe_deja = bin.join("opentunebox-gui");
    std::fs::copy(std::fs::canonicalize("/bin/sh").unwrap(), &installe_deja).unwrap();
    let mut en_cours = lance_malgre_etxtbsy(&installe_deja, &["-c", "sleep 20; echo fin"]);
    std::thread::sleep(std::time::Duration::from_millis(500));

    let nouveau = nouveau_programme(&d, "#!/bin/sh\necho nouvelle version\n");
    let sortie = installe(&home, &nouveau);
    let _ = en_cours.kill();
    let _ = en_cours.wait();

    assert!(
        sortie.status.success(),
        "mise à jour refusée : {}",
        String::from_utf8_lossy(&sortie.stderr)
    );
    assert_eq!(
        std::fs::read(&installe_deja).unwrap(),
        std::fs::read(&nouveau).unwrap(),
        "programme non remplacé"
    );
    assert!(
        home.join(".local/share/applications/OpenTuneBox.desktop").exists(),
        "le .desktop n'a pas été écrit"
    );
    assert!(!bin.join(".opentunebox-gui.new").exists(), "fichier temporaire oublié");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn les_anciens_png_de_l_ancien_script_sont_retires() {
    let d = dossier_jetable("png");
    let home = d.join("home");
    let vieux = home.join(".local/share/icons/hicolor/64x64/apps");
    std::fs::create_dir_all(&vieux).unwrap();
    std::fs::write(vieux.join("opentunebox.png"), "vieux").unwrap();
    let nouveau = nouveau_programme(&d, "#!/bin/sh\n");
    assert!(installe(&home, &nouveau).status.success());
    assert!(
        !vieux.join("opentunebox.png").exists(),
        "l'ancien PNG de 64 px est resté"
    );
    assert!(
        home.join(".local/share/icons/hicolor/256x256/apps/opentunebox.png")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// La licence du projet (PolyForm Noncommercial 1.0.0, D-045) oblige, dans sa section « Notices », à remettre ses termes et la ligne
/// `Required Notice:` à quiconque reçoit une copie du programme : l'installateur pose donc `LICENSE` à côté des mentions tierces.
#[test]
fn la_licence_du_projet_est_installee_avec_les_mentions_tierces() {
    let d = dossier_jetable("licence");
    let home = d.join("home");
    let sortie = installe(&home, &nouveau_programme(&d, "#!/bin/sh\n"));
    assert!(
        sortie.status.success(),
        "installation échouée : {}",
        String::from_utf8_lossy(&sortie.stderr)
    );

    let doc = home.join(".local/share/doc/opentunebox");
    for fichier in ["LICENSE", "THIRD-PARTY-LICENSES.txt"] {
        assert!(
            doc.join(fichier).exists(),
            "{fichier} n'est pas installé dans ~/.local/share/doc/opentunebox"
        );
    }
    let licence = std::fs::read_to_string(doc.join("LICENSE")).unwrap();
    assert!(
        licence.contains("PolyForm Noncommercial License 1.0.0") && licence.contains("Required Notice:"),
        "la licence installée n'a pas son titre ou sa ligne « Required Notice: »"
    );
    let _ = std::fs::remove_dir_all(&d);
}
