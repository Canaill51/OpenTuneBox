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

#[test]
fn la_mise_a_jour_marche_meme_si_le_programme_installe_tourne() {
    let d = dossier_jetable("maj");
    let home = d.join("home");
    let bin = home.join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    // un vrai exécutable (pas un script ni un binaire « multi-appel ») qui reste en cours d'exécution
    let installe_deja = bin.join("opentunebox-gui");
    std::fs::copy(std::fs::canonicalize("/bin/sh").unwrap(), &installe_deja).unwrap();
    let mut en_cours = Command::new(&installe_deja)
        .args(["-c", "sleep 20; echo fin"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
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
