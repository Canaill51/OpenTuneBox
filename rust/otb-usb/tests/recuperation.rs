//! Parcours complet sur une carte simulée : copier, vérifier, confirmer, effacer — et chaque refus d'effacer.
//! ⚠️ Chaque refus est un test : c'est la promesse du lot 32 (« un log perdu est un vol à refaire »).

use std::path::PathBuf;
use std::time::Duration;

use otb_usb::client::{Command, Msp, MspError};
use otb_usb::erase::{CheckFailure, EraseError, ErasePolling, erase_flash, erase_limit, verify_and_save};
use otb_usb::frame::Version;
use otb_usb::sim::{Fault, SimBoard};

fn jeno() -> Vec<u8> {
    std::fs::read(format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"))).expect("log d'essai")
}

/// Deux vols à la suite, comme la mémoire d'une carte après deux batteries.
fn deux_vols() -> Vec<u8> {
    let mut f = jeno();
    f.extend(jeno());
    f
}

/// L'en-tête seul d'un log : un début de log sans aucune trame.
fn en_tete_seul() -> Vec<u8> {
    let d = jeno();
    let mut fin = 0;
    for line in d.split_inclusive(|&b| b == b'\n') {
        if !line.starts_with(b"H ") {
            break;
        }
        fin += line.len();
    }
    d[..fin].to_vec()
}

fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-usb-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn vite() -> ErasePolling {
    ErasePolling {
        interval: Duration::from_millis(1),
        limit: Some(Duration::from_secs(5)),
    }
}

/// Copie et vérifie ; rend la connexion et la copie vérifiée.
fn copie_verifiee(board: SimBoard, nom: &str) -> (Msp<SimBoard>, otb_usb::VerifiedCopy) {
    let mut msp = Msp::new(board);
    let copy = msp.copy_flash(|_, _| true).expect("copie");
    let v = verify_and_save(copy, &dossier(nom).join("copie.bbl")).expect("contrôles");
    (msp, v)
}

#[test]
fn parcours_complet() {
    let flash = deux_vols();
    let mut msp = Msp::new(SimBoard::new(flash.clone()));
    let id = msp.identify().unwrap();
    assert_eq!(
        (id.variant.as_str(), id.board.as_str(), id.target.as_str()),
        ("BTFL", "S7X2", "STM32F7X2")
    );
    assert_eq!((id.name.as_str(), id.version, id.api), ("jeno5", (25, 12, 0), (1, 47)));
    assert!(id.uid.is_known());
    assert_eq!(id.uid.to_hex().len(), 24);

    let mut pas = Vec::new();
    let copy = msp
        .copy_flash(|n, total| {
            pas.push((n, total));
            true
        })
        .unwrap();
    assert_eq!(copy.bytes(), &flash[..]);
    assert_eq!(pas.last(), Some(&(flash.len() as u64, flash.len() as u64)));
    // Blocs de 4 096 − 16 au plus : le firmware rend moins que demandé, la copie suit.
    assert_eq!(pas.len(), flash.len().div_ceil(4080));

    let path = dossier("complet").join("copie.bbl");
    let verified = verify_and_save(copy, &path).unwrap();
    let r = verified.report();
    assert_eq!((r.logs, r.bytes), (2, flash.len() as u64));
    assert!(r.fragments >= 2);
    assert_eq!(std::fs::read(&path).unwrap(), flash);

    let mut attentes = 0;
    erase_flash(&mut msp, verified.confirm(), vite(), |_| attentes += 1).unwrap();
    let board = msp.into_link();
    assert!(board.flash.is_empty());
    assert_eq!(board.count(Command::DataflashErase), 1);
    assert_eq!(attentes, 4, "3 relectures « pas prête », puis « prête et vide »");
    // ⚠️ L'état armé est relu en dernier, juste avant l'effacement.
    let i = board
        .received
        .iter()
        .position(|&c| c == Command::DataflashErase.code())
        .unwrap();
    assert_eq!(board.received[i - 1], Command::StatusEx.code());
}

#[test]
fn drone_arme_rien_n_est_efface() {
    let (mut msp, v) = copie_verifiee(SimBoard::new(jeno()), "arme");
    msp.link_mut().armed = true;
    let e = erase_flash(&mut msp, v.confirm(), vite(), |_| {}).unwrap_err();
    assert!(matches!(e, EraseError::Armed), "{e:?}");
    assert_eq!(msp.link().count(Command::DataflashErase), 0);
    assert_eq!(msp.link().flash, jeno());
}

#[test]
fn vol_enregistre_apres_la_copie() {
    let (mut msp, v) = copie_verifiee(SimBoard::new(jeno()), "apres");
    msp.link_mut().flash.extend(jeno());
    let e = erase_flash(&mut msp, v.confirm(), vite(), |_| {}).unwrap_err();
    assert!(matches!(e, EraseError::Changed { .. }), "{e:?}");
    assert_eq!(msp.link().count(Command::DataflashErase), 0);
}

#[test]
fn vol_enregistre_pendant_la_copie() {
    let mut board = SimBoard::new(jeno());
    board.record_during_read = Some((0, jeno()));
    let (mut msp, v) = copie_verifiee(board, "pendant");
    let e = erase_flash(&mut msp, v.confirm(), vite(), |_| {}).unwrap_err();
    assert!(matches!(e, EraseError::Changed { .. }), "{e:?}");
    assert_eq!(msp.link().count(Command::DataflashErase), 0);
}

#[test]
fn autre_carte_branchee() {
    let (mut msp, v) = copie_verifiee(SimBoard::new(jeno()), "autre");
    msp.link_mut().uid[2] ^= 1;
    let e = erase_flash(&mut msp, v.confirm(), vite(), |_| {}).unwrap_err();
    assert!(matches!(e, EraseError::OtherBoard), "{e:?}");
    assert_eq!(msp.link().count(Command::DataflashErase), 0);
}

#[test]
fn identifiant_nul_seulement_sur_la_meme_connexion() {
    let mut board = SimBoard::new(jeno());
    board.uid = [0; 3];
    let (msp, v) = copie_verifiee(board, "uid-nul");
    // Nouvelle connexion : rien ne prouve que c'est la même carte.
    let mut autre = Msp::new(msp.into_link());
    let e = erase_flash(&mut autre, v.confirm(), vite(), |_| {}).unwrap_err();
    assert!(matches!(e, EraseError::OtherBoard), "{e:?}");
    assert_eq!(autre.link().count(Command::DataflashErase), 0);

    let mut board = SimBoard::new(jeno());
    board.uid = [0; 3];
    let (mut msp, v) = copie_verifiee(board, "uid-nul-meme");
    erase_flash(&mut msp, v.confirm(), vite(), |_| {}).unwrap();
    assert!(msp.link().flash.is_empty());
}

#[test]
fn copie_incomplete_fichier_garde_effacement_refuse() {
    let flash = jeno();
    let mut board = SimBoard::new(flash.clone());
    // La mémoire s'arrête avant la place annoncée : la lecture rend un bloc vide.
    board.total = (flash.len() - 1000) as u32;
    let mut msp = Msp::new(board);
    let copy = msp.copy_flash(|_, _| true).unwrap();
    let path = dossier("incomplete").join("copie.bbl");
    let e = verify_and_save(copy, &path).err().expect("refus");
    assert!(matches!(e.reason, CheckFailure::SizeMismatch { .. }), "{e:?}");
    assert_eq!(e.saved.as_deref(), Some(path.as_path()));
    assert_eq!(std::fs::read(&path).unwrap(), &flash[..flash.len() - 1000]);
}

#[test]
fn log_sans_vol_refuse() {
    let mut flash = jeno();
    flash.extend(en_tete_seul());
    let mut msp = Msp::new(SimBoard::new(flash));
    let copy = msp.copy_flash(|_, _| true).unwrap();
    let e = verify_and_save(copy, &dossier("sans-vol").join("copie.bbl"))
        .err()
        .expect("refus");
    assert!(
        matches!(e.reason, CheckFailure::LogWithoutFragment { log: 2, logs: 2 }),
        "{e:?}"
    );
}

#[test]
fn memoire_sans_log_refusee() {
    let bruit: Vec<u8> = (0..20_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let mut msp = Msp::new(SimBoard::new(bruit));
    let copy = msp.copy_flash(|_, _| true).unwrap();
    let e = verify_and_save(copy, &dossier("bruit").join("copie.bbl"))
        .err()
        .expect("refus");
    assert!(matches!(e.reason, CheckFailure::NoLog), "{e:?}");
}

#[test]
fn memoire_vide() {
    let mut msp = Msp::new(SimBoard::new(Vec::new()));
    let copy = msp.copy_flash(|_, _| true).unwrap();
    let path = dossier("vide").join("copie.bbl");
    let e = verify_and_save(copy, &path).err().expect("refus");
    assert!(matches!(e.reason, CheckFailure::Empty), "{e:?}");
    assert!(!path.exists());
}

#[test]
fn fichier_existant_jamais_ecrase() {
    let path = dossier("existant").join("copie.bbl");
    std::fs::write(&path, b"ancien log").unwrap();
    let mut msp = Msp::new(SimBoard::new(jeno()));
    let copy = msp.copy_flash(|_, _| true).unwrap();
    let e = verify_and_save(copy, &path).err().expect("refus");
    assert!(matches!(e.reason, CheckFailure::AlreadyExists), "{e:?}");
    assert_eq!(std::fs::read(&path).unwrap(), b"ancien log");
}

#[test]
fn autre_firmware_rien_n_est_lu() {
    let mut board = SimBoard::new(jeno());
    board.variant = *b"INAV";
    let mut msp = Msp::new(board);
    let e = msp.copy_flash(|_, _| true).err().expect("refus");
    assert!(
        matches!(&e, MspError::NotBetaflight { variant } if variant == "INAV"),
        "{e:?}"
    );
    assert_eq!(
        msp.link().received,
        vec![Command::ApiVersion.code(), Command::FcVariant.code()]
    );
}

#[test]
fn carte_sans_memoire() {
    let mut board = SimBoard::new(Vec::new());
    board.has_flash = false;
    let mut msp = Msp::new(board);
    assert!(matches!(msp.copy_flash(|_, _| true), Err(MspError::NoFlash)));
    assert_eq!(msp.link().count(Command::DataflashRead), 0);
}

#[test]
fn copie_arretee_par_le_pilote() {
    let mut msp = Msp::new(SimBoard::new(jeno()));
    let mut blocs = 0;
    let r = msp.copy_flash(|_, _| {
        blocs += 1;
        blocs < 3
    });
    assert!(matches!(r, Err(MspError::Cancelled)));
    assert_eq!(msp.link().count(Command::DataflashRead), 3);
}

#[test]
fn reponses_perdues_ou_alterees_redemandees() {
    let flash = deux_vols();
    let mut board = SimBoard::new(flash.clone());
    // Réponses 0 à 6 : identité et résumé ; les lectures commencent à la 7e.
    board.faults = vec![
        (9, Fault::Drop),
        (10, Fault::Corrupt),
        (30, Fault::Drop),
        (50, Fault::Corrupt),
        (51, Fault::Corrupt),
        (60, Fault::ShortData),
        (70, Fault::WrongAddress),
    ];
    let mut msp = Msp::new(board);
    msp.set_timeout(Duration::from_millis(20));
    let copy = msp.copy_flash(|_, _| true).unwrap();
    assert_eq!(copy.bytes(), &flash[..]);
}

#[test]
fn carte_muette_abandon() {
    let mut board = SimBoard::new(jeno());
    board.faults = (0..10).map(|k| (k, Fault::Drop)).collect();
    let mut msp = Msp::new(board);
    msp.set_timeout(Duration::from_millis(5));
    assert!(matches!(msp.identify(), Err(MspError::Timeout { cmd: 1 })));
    assert_eq!(msp.link().count(Command::ApiVersion), 3);
}

#[test]
fn msp_v2() {
    let flash = jeno();
    let mut msp = Msp::with_version(SimBoard::new(flash.clone()), Version::V2);
    let copy = msp.copy_flash(|_, _| true).unwrap();
    assert_eq!(copy.bytes(), &flash[..]);
}

#[test]
fn effacement_trop_long() {
    let mut board = SimBoard::new(jeno());
    board.erase_polls = u32::MAX;
    let (mut msp, v) = copie_verifiee(board, "long");
    let polling = ErasePolling {
        interval: Duration::from_millis(1),
        limit: Some(Duration::from_millis(20)),
    };
    let e = erase_flash(&mut msp, v.confirm(), polling, |_| {}).unwrap_err();
    assert!(matches!(e, EraseError::Timeout { .. }), "{e:?}");
}

#[test]
fn effacement_sans_effet() {
    let mut board = SimBoard::new(jeno());
    board.erase_ignored = true;
    let (mut msp, v) = copie_verifiee(board, "sans-effet");
    let polling = ErasePolling {
        interval: Duration::from_millis(1),
        limit: Some(Duration::from_millis(20)),
    };
    // « Prête » ne suffit pas : la mémoire doit aussi être vide.
    let e = erase_flash(&mut msp, v.confirm(), polling, |_| {}).unwrap_err();
    assert!(matches!(e, EraseError::Timeout { .. }), "{e:?}");
}

#[test]
fn delai_d_effacement_du_configurateur() {
    assert_eq!(erase_limit(0), Duration::from_secs(300));
    assert_eq!(erase_limit(16 * 1024 * 1024), Duration::from_secs(300));
    assert_eq!(erase_limit(16 * 1024 * 1024 + 1), Duration::from_secs(500));
    assert_eq!(erase_limit(128 * 1024 * 1024), Duration::from_secs(1700));
}

#[test]
fn diff_all_recu_par_morceaux() {
    // ⚠️ `# version` commence par l'invite « \r\n# » : reçue octet par octet, la sortie s'arrêterait là si la fin
    // se reconnaissait à l'invite seule, sans le silence qui la suit.
    let mut board = SimBoard::new(jeno());
    board.recv_chunk = 1;
    let attendu = board.diff.clone();
    assert_eq!(Msp::new(board).diff_all().unwrap(), attendu);
}

#[test]
fn diff_all_en_dernier() {
    let board = SimBoard::new(jeno());
    let attendu = board.diff.clone();
    let mut msp = Msp::new(board);
    msp.identify().unwrap();
    // `diff_all` consomme la connexion : la carte redémarre à la sortie du CLI, plus rien ne passe après.
    let text = msp.diff_all().unwrap();
    assert_eq!(text, attendu);
    assert!(text.starts_with("# version\n"));
}
