//! Contrats du lot 32 (D-063) : ce que le client peut envoyer, et le seul chemin qui efface.

use std::path::Path;

use otb_usb::client::Command;
use std::path::PathBuf;

use otb_usb::link::{PortError, board_candidates, classify_open_error};

/// ⚠️ La liste fermée des commandes envoyées. **Aucune écriture de réglage** : ajouter une commande ici, c'est
/// demander l'accord de Cédric (D-063 ; le redémarrage en stockage de masse et l'état de la carte SD : D-075).
#[test]
fn commandes_figees() {
    let codes: Vec<u16> = Command::ALL.iter().map(|c| c.code()).collect();
    assert_eq!(codes, vec![1, 2, 3, 4, 10, 68, 70, 71, 72, 79, 150, 160]);
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

/// ⚠️ `MSP_REBOOT` ne part que d'un endroit, avec le seul mode « stockage de masse » (2) : jamais le chargeur de
/// firmware (modes 1 et 4), qui laisserait la carte sans firmware lancé (D-075).
#[test]
fn seul_redemarrage_en_disque() {
    let mut envois = Vec::new();
    for (name, src) in sources() {
        if name == "sim.rs" {
            continue;
        }
        for (i, line) in src.lines().enumerate() {
            if line.contains("Command::Reboot") && !line.trim_start().starts_with("Command::Reboot =>") {
                envois.push((format!("{name}:{}", i + 1), line.trim().to_string()));
            }
        }
    }
    let requetes: Vec<_> = envois.iter().filter(|(_, l)| l.contains("request(")).collect();
    assert_eq!(requetes.len(), 1, "envois du redémarrage : {envois:?}");
    assert!(requetes[0].0.starts_with("msc.rs:"), "{requetes:?}");
    assert!(
        requetes[0].1.contains("request(Command::Reboot, &[REBOOT_MSC])"),
        "{requetes:?}"
    );
    let msc = sources().into_iter().find(|(n, _)| n == "msc.rs").unwrap().1;
    assert!(msc.contains("const REBOOT_MSC: u8 = 2;"));
    // Ailleurs que dans `client.rs` (la liste), personne ne parle du redémarrage.
    let ailleurs: Vec<_> = envois
        .iter()
        .filter(|(f, _)| !f.starts_with("msc.rs:") && !f.starts_with("client.rs:"))
        .collect();
    assert!(ailleurs.is_empty(), "redémarrage cité hors de msc.rs : {ailleurs:?}");
}

/// ⚠️ Critère d'arrêt du lot 32c : un seul chemin supprime un fichier (les logs d'une carte SD), et il exige un permis
/// que seul `confirm_sd` fabrique.
#[test]
fn un_seul_chemin_supprime_un_fichier() {
    let mut suppressions = Vec::new();
    for (name, src) in sources() {
        for (i, line) in src.lines().enumerate() {
            for motif in ["remove_file", "remove_dir", "fs::rename", "set_len"] {
                if line.contains(motif) && !line.trim_start().starts_with("//") {
                    suppressions.push(format!("{name}:{} {motif}", i + 1));
                }
            }
        }
    }
    assert_eq!(suppressions.len(), 1, "suppressions : {suppressions:?}");
    assert!(suppressions[0].starts_with("sd.rs:"), "{suppressions:?}");
    let sd = sources().into_iter().find(|(n, _)| n == "sd.rs").unwrap().1;
    let f = sd.find("pub fn delete_sd_logs(permit: SdErasePermit)").unwrap();
    let remove = sd.find("std::fs::remove_file(").unwrap();
    assert!(
        f < remove && !sd[f..remove].contains("\nfn "),
        "la suppression n'est pas dans `delete_sd_logs`"
    );
    // Avant la suppression, la relecture comparée à la copie.
    assert!(sd[f..remove].contains("same_content(&source, len, &copy)"));
    let constructions: Vec<_> = sources()
        .into_iter()
        .flat_map(|(_, src)| src.lines().map(str::trim).map(String::from).collect::<Vec<_>>())
        .filter(|l| l == "SdErasePermit {")
        .collect();
    assert_eq!(constructions.len(), 1, "seul `confirm_sd` construit un permis");
    let confirm = sd
        .find("pub fn confirm_sd(copies: Vec<SdCopy>) -> SdErasePermit")
        .unwrap();
    let construit = sd.find("\n    SdErasePermit {").unwrap();
    assert!(confirm < construit && !sd[confirm..construit].contains("\npub fn "));
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

/// Le motif « identifiant de compte » de CodeQL (`SensitiveDataHeuristics.qll`, `maybeAccountInfo`) :
/// `([uU]|^|_|[a-z](?=U))([uU][iI][dD])` dans un nom. Réécrit à la main (pas de dépendance `regex` ici).
fn looks_like_account_id(name: &str) -> bool {
    let b = name.as_bytes();
    (0..b.len().saturating_sub(2)).any(|i| {
        b[i..i + 3].eq_ignore_ascii_case(b"uid")
            && (i == 0 || matches!(b[i - 1], b'u' | b'U' | b'_') || (b[i - 1].is_ascii_lowercase() && b[i] == b'U'))
    })
}

/// ⚠️ CodeQL prend tout nom en « uid » pour un identifiant d'utilisateur et refuse qu'on l'affiche ou l'écrive
/// (alerte « Cleartext logging », PR #169) ; une empreinte ne change rien, il juge le nom. L'identifiant de la carte
/// s'appelle donc `ChipId` / `chip_id` (L-109). Seul `MSP_UID`, nom du firmware cité en commentaire, est permis.
#[test]
fn aucun_nom_pris_pour_un_identifiant_de_compte() {
    assert!(looks_like_account_id("uid") && looks_like_account_id("Uid") && looks_like_account_id("board_uid"));
    assert!(looks_like_account_id("uid_summary") && looks_like_account_id("boardUid"));
    assert!(!looks_like_account_id("guide") && !looks_like_account_id("chip_id") && !looks_like_account_id("build"));
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<_> = std::fs::read_dir(root.join("src"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.push(root.join("../otb-dev/src/usb.rs"));
    let mut found = Vec::new();
    for f in files {
        let src = std::fs::read_to_string(&f).unwrap();
        for (i, line) in src.lines().enumerate() {
            for name in line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
                if name != "MSP_UID" && looks_like_account_id(name) {
                    found.push(format!("{}:{} {name}", f.display(), i + 1));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "noms pris pour un identifiant de compte par CodeQL : {found:?}"
    );
}
