//! Contrats du lot 32 (D-063) : ce que le client peut envoyer, et le seul chemin qui efface.

use std::path::Path;

use otb_usb::client::Command;
use std::path::PathBuf;

use otb_usb::link::{PortError, board_candidates, classify_open_error};

/// ⚠️ La liste fermée des commandes envoyées. **Aucune écriture de réglage** : ajouter une commande ici, c'est
/// demander l'accord de Cédric (D-063).
#[test]
fn commandes_figees() {
    let codes: Vec<u16> = Command::ALL.iter().map(|c| c.code()).collect();
    assert_eq!(codes, vec![1, 2, 3, 4, 10, 70, 71, 72, 150, 160]);
    for c in Command::ALL {
        assert_eq!(Command::from_code(c.code()), Some(c));
    }
}

fn sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&p).unwrap())
        })
        .collect();
    out.sort();
    out
}

/// ⚠️ Critère d'arrêt du lot 32 : un seul chemin de code peut envoyer l'effacement, et il exige un permis.
#[test]
fn un_seul_chemin_efface() {
    let mut envois = Vec::new();
    for (name, src) in sources() {
        if name == "sim.rs" {
            continue; // la carte simulée **reçoit** l'effacement
        }
        // Tout envoi passe par `request` ; l'effacement par son nom, jamais par son numéro.
        for (i, line) in src.lines().enumerate() {
            if line.contains("request(Command::DataflashErase") {
                envois.push(format!("{name}:{}", i + 1));
            }
            assert!(
                !line.contains("from_code(72") && !line.contains("code() == 72"),
                "{name}:{} : effacement désigné par son numéro",
                i + 1
            );
        }
        if name != "client.rs" && name != "erase.rs" {
            assert!(!src.contains("DataflashErase"), "{name} parle de l'effacement");
        }
    }
    assert_eq!(envois.len(), 1, "envois de l'effacement : {envois:?}");
    assert!(envois[0].starts_with("erase.rs:"), "{envois:?}");
    let erase = &sources().into_iter().find(|(n, _)| n == "erase.rs").unwrap().1;
    // L'envoi est dans `erase_flash`, qui prend le permis par valeur.
    let f = erase.find("pub fn erase_flash").unwrap();
    let send = erase.find("request(Command::DataflashErase").unwrap();
    assert!(f < send);
    assert!(erase[f..send].contains("permit: ErasePermit"));
    assert!(
        !erase[f..send].contains("\nfn "),
        "l'envoi n'est pas dans `erase_flash`"
    );
    // Personne d'autre ne fabrique un permis : seul `confirm` le construit.
    let constructions: Vec<_> = sources()
        .into_iter()
        .flat_map(|(_, src)| src.lines().map(str::trim).map(String::from).collect::<Vec<_>>())
        .filter(|l| l == "ErasePermit {")
        .collect();
    assert_eq!(constructions.len(), 1, "seul `confirm` construit un permis");
    let confirm = erase.find("pub fn confirm(self) -> ErasePermit").unwrap();
    let construit = erase.find("\n        ErasePermit {").unwrap();
    assert!(confirm < construit && !erase[confirm..construit].contains("\n    pub fn "));
}

/// Le client n'envoie jamais de requête brute : `request` reste interne à la crate.
#[test]
fn requete_brute_interne() {
    let client = sources().into_iter().find(|(n, _)| n == "client.rs").unwrap().1;
    assert!(client.contains("pub(crate) fn request("));
    assert!(!client.contains("pub fn request("));
}

#[test]
fn erreurs_d_ouverture_expliquees() {
    use std::io::{Error, ErrorKind};
    let p = Path::new("/dev/ttyACM0");
    assert!(matches!(
        classify_open_error(p, Error::from(ErrorKind::NotFound)),
        PortError::NotFound(_)
    ));
    assert!(matches!(
        classify_open_error(p, Error::from(ErrorKind::PermissionDenied)),
        PortError::Denied(_)
    ));
    assert!(matches!(
        classify_open_error(p, Error::from(ErrorKind::ResourceBusy)),
        PortError::Busy(_)
    ));
    assert!(matches!(
        classify_open_error(p, Error::from(ErrorKind::Other)),
        PortError::Other(..)
    ));
    let denied = classify_open_error(p, Error::from(ErrorKind::PermissionDenied)).to_string();
    if cfg!(windows) {
        assert!(denied.contains("configurateur"), "{denied}");
    } else {
        assert!(denied.contains("dialout"), "{denied}");
    }
}

#[test]
fn ports_candidats_sous_linux() {
    let ports: Vec<PathBuf> = [
        "/dev/ttyS0",
        "/dev/ttyACM0",
        "/dev/ttyprintk",
        "/dev/ttyUSB1",
        "/dev/ttyS17",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    let linux = board_candidates(ports.clone(), true);
    assert_eq!(
        linux,
        vec![PathBuf::from("/dev/ttyACM0"), PathBuf::from("/dev/ttyUSB1")]
    );
    assert_eq!(board_candidates(ports.clone(), false), ports);
}
