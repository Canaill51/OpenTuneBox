//! Le mode stockage de masse et les cartes SD (lot 32c, D-075), sans matériel : carte simulée, et disques fabriqués
//! dans un dossier temporaire comme le système les monterait.
//! ⚠️ Chaque refus est un test : c'est la promesse du lot 32 (« un log perdu est un vol à refaire »).

use std::path::{Path, PathBuf};
use std::time::Duration;

use otb_usb::client::Msp;
use otb_usb::erase::{CheckFailure, EraseError, ErasePolling, erase_flash, verify_and_save};
use otb_usb::msc::{
    DiskError, DiskKind, DiskRebootError, FLASH_ALL, Storage, copy_flash_disk, find_disks, inspect, parse_mountinfo,
    reboot_to_disk,
};
use otb_usb::sd::{confirm_sd, copy_sd_log, delete_sd_logs};
use otb_usb::sim::SimBoard;

fn jeno() -> Vec<u8> {
    std::fs::read(format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"))).expect("log d'essai")
}

fn deux_vols() -> Vec<u8> {
    let mut f = jeno();
    f.extend(jeno());
    f
}

/// L'en-tête seul d'un log : un vol coupé avant sa première trame.
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
    let d = std::env::temp_dir().join(format!("otb-usb-msc-{nom}-{}", std::process::id()));
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

/// Le disque d'une carte à mémoire interne en mode stockage de masse : `BTFL_ALL.BBL` et un fichier par log.
fn disque_memoire(nom: &str, flash: &[u8]) -> PathBuf {
    let root = dossier(nom).join("BETAFLT");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(FLASH_ALL), flash).unwrap();
    std::fs::write(root.join("BTFL_001.BBL"), &flash[..flash.len() / 2]).unwrap();
    root
}

/// Une carte SD : `LOGS/LOG00001.BFL`… et un fichier qui n'est pas un log.
fn carte_sd(nom: &str, logs: &[Vec<u8>]) -> PathBuf {
    let root = dossier(nom).join("SD");
    std::fs::create_dir_all(root.join("LOGS")).unwrap();
    for (k, l) in logs.iter().enumerate() {
        std::fs::write(root.join("LOGS").join(format!("LOG{:05}.BFL", k + 1)), l).unwrap();
    }
    std::fs::write(root.join("LOGS").join("NOTES.TXT"), b"pas un log").unwrap();
    root
}

// ---------------------------------------------------------------- redémarrage en disque

#[test]
fn redemarre_en_disque_apres_avoir_lu_la_carte() {
    let flash = deux_vols();
    let mut msp = Msp::new(SimBoard::new(flash.clone()));
    let before = reboot_to_disk(&mut msp).expect("redémarrage");
    assert_eq!(before.identity.name, "jeno5");
    assert!(before.identity.chip_id.is_known());
    match before.storage {
        Storage::Flash(s) => assert_eq!(s.used as usize, flash.len()),
        other => panic!("{other:?}"),
    }
    let board = msp.into_link();
    assert_eq!(board.reboots, vec![2], "un seul redémarrage, en stockage de masse");
    assert!(board.rebooted);
}

#[test]
fn refus_de_redemarrer() {
    let cas: Vec<(&str, SimBoard)> = vec![
        ("armé", {
            let mut b = SimBoard::new(jeno());
            b.armed = true;
            b
        }),
        ("mémoire vide", SimBoard::new(Vec::new())),
        ("sans stockage de masse", {
            let mut b = SimBoard::new(jeno());
            b.msc = false;
            b
        }),
        // ⚠️ Betaflight 3.4 et plus ancien : la demande ferait redémarrer la carte normalement.
        ("firmware sans redémarrage en disque", {
            let mut b = SimBoard::new(jeno());
            b.api = (1, 39);
            b
        }),
        ("disque pas prêt", {
            let mut b = SimBoard::new(jeno());
            b.msc_ready = false;
            b
        }),
        ("carte SD absente", {
            let mut b = SimBoard::new(Vec::new());
            b.has_flash = false;
            b.sd_state = Some(0);
            b
        }),
        ("ni mémoire ni SD", {
            let mut b = SimBoard::new(Vec::new());
            b.has_flash = false;
            b
        }),
    ];
    for (nom, board) in cas {
        let mut msp = Msp::new(board);
        let e = reboot_to_disk(&mut msp).expect_err(nom);
        let attendu = match nom {
            "armé" => matches!(e, DiskRebootError::Armed),
            "mémoire vide" => matches!(e, DiskRebootError::Empty),
            "sans stockage de masse" | "firmware sans redémarrage en disque" => {
                matches!(e, DiskRebootError::NotSupported)
            }
            "disque pas prêt" => matches!(e, DiskRebootError::NotReady),
            "carte SD absente" => matches!(e, DiskRebootError::SdNotReady { state: 0 }),
            _ => matches!(e, DiskRebootError::NoStorage),
        };
        assert!(attendu, "{nom} : {e:?}");
        let board = msp.into_link();
        assert!(!board.rebooted, "{nom} : la carte a redémarré");
        // Armé, vide, sans stockage : rien n'est même demandé.
        if matches!(
            nom,
            "armé" | "mémoire vide" | "carte SD absente" | "ni mémoire ni SD" | "firmware sans redémarrage en disque"
        ) {
            assert!(board.reboots.is_empty(), "{nom} : {:?}", board.reboots);
        }
    }
}

#[test]
fn carte_sd_prete() {
    let mut b = SimBoard::new(Vec::new());
    b.has_flash = false;
    b.sd_state = Some(4);
    let mut msp = Msp::new(b);
    let before = reboot_to_disk(&mut msp).expect("redémarrage");
    assert!(matches!(before.storage, Storage::SdCard(s) if s.ready() && s.total_kb > 0));
}

// ---------------------------------------------------------------- disques

#[test]
fn reconnait_les_disques_de_carte() {
    let flash = deux_vols();
    let mem = disque_memoire("reconnait-mem", &flash);
    let sd = carte_sd("reconnait-sd", &[jeno()]);
    let autre = dossier("reconnait-autre");
    std::fs::write(autre.join("photo.jpg"), b"x").unwrap();
    let disks = find_disks([mem.clone(), sd.clone(), autre]);
    assert_eq!(disks.len(), 2);
    assert_eq!(
        disks[0].kind,
        DiskKind::Flash {
            all: mem.join(FLASH_ALL),
            len: flash.len() as u64
        }
    );
    let DiskKind::SdCard { logs } = &disks[1].kind else {
        panic!("{:?}", disks[1]);
    };
    assert_eq!(logs.len(), 1, "seul le .BFL est un log : {logs:?}");
    assert_eq!(logs[0].path, sd.join("LOGS").join("LOG00001.BFL"));
}

/// FAT monté en noms courts minuscules (`shortname=lower`) : mêmes fichiers.
#[test]
fn noms_en_minuscules() {
    let root = dossier("minuscules");
    std::fs::write(root.join("btfl_all.bbl"), jeno()).unwrap();
    assert!(matches!(inspect(&root).unwrap().kind, DiskKind::Flash { .. }));
    let root = dossier("minuscules-sd");
    std::fs::create_dir_all(root.join("logs")).unwrap();
    std::fs::write(root.join("logs").join("log00001.bfl"), jeno()).unwrap();
    assert!(matches!(&inspect(&root).unwrap().kind, DiskKind::SdCard { logs } if logs.len() == 1));
}

#[test]
fn points_de_montage_fat() {
    let text = "\
22 1 8:2 / / rw,relatime shared:1 - ext4 /dev/sda2 rw
95 30 8:17 / /run/media/cedric/BETAFLT rw,nosuid,nodev,relatime shared:50 - vfat /dev/sdb1 rw,fmask=0022
96 30 8:33 / /media/cedric/CARTE\\040SD rw,nosuid shared:51 - exfat /dev/sdc1 rw
97 30 0:48 / /run/user/1000/doc rw,nosuid shared:52 - fuse.portal portal rw
98 30 8:49 / /mnt/sd rw shared:53 master:7 - vfat /dev/sdd1 rw";
    assert_eq!(
        parse_mountinfo(text),
        vec![
            PathBuf::from("/run/media/cedric/BETAFLT"),
            PathBuf::from("/media/cedric/CARTE SD"),
            PathBuf::from("/mnt/sd"),
        ]
    );
}

// ---------------------------------------------------------------- mémoire interne par le disque

#[test]
fn copie_par_le_disque_puis_effacement_apres_rebranchement() {
    let flash = deux_vols();
    let mut msp = Msp::new(SimBoard::new(flash.clone()));
    let before = reboot_to_disk(&mut msp).unwrap();
    let disk = inspect(&disque_memoire("parcours", &flash)).unwrap();
    assert!(before.matches(&disk));
    let mut vu = 0;
    let copy = copy_flash_disk(&before, &disk, |n, total| {
        vu = n;
        assert_eq!(total, flash.len() as u64);
        true
    })
    .unwrap();
    assert_eq!(vu, flash.len() as u64);
    let v = verify_and_save(copy, &dossier("parcours-copie").join("copie.bbl")).expect("contrôles");
    assert_eq!((v.report().logs, v.report().bytes), (2, flash.len() as u64));
    // Débranché, rebranché : nouvelle connexion MSP à la même carte.
    let mut msp = Msp::new(SimBoard::new(flash));
    erase_flash(&mut msp, v.confirm(), vite(), |_| {}).expect("effacement");
    assert!(msp.into_link().flash.is_empty());
}

/// ⚠️ Sans identifiant gravé, rien ne prouve que la carte rebranchée est celle du disque : effacement refusé.
#[test]
fn identifiant_nul_pas_d_effacement_apres_rebranchement() {
    let flash = jeno();
    let mut b = SimBoard::new(flash.clone());
    b.chip_id = [0; 3];
    let mut msp = Msp::new(b);
    let before = reboot_to_disk(&mut msp).unwrap();
    let disk = inspect(&disque_memoire("nul", &flash)).unwrap();
    let copy = copy_flash_disk(&before, &disk, |_, _| true).unwrap();
    let v = verify_and_save(copy, &dossier("nul-copie").join("copie.bbl")).unwrap();
    let mut b = SimBoard::new(flash);
    b.chip_id = [0; 3];
    let mut msp = Msp::new(b);
    let e = erase_flash(&mut msp, v.confirm(), vite(), |_| {}).unwrap_err();
    assert!(matches!(e, EraseError::OtherBoard), "{e:?}");
    assert!(!msp.into_link().flash.is_empty());
}

#[test]
fn disque_d_une_autre_carte_refuse() {
    let flash = deux_vols();
    let mut msp = Msp::new(SimBoard::new(flash.clone()));
    let before = reboot_to_disk(&mut msp).unwrap();
    // Taille différente de la place occupée annoncée : pas ce disque.
    let disk = inspect(&disque_memoire("autre", &jeno())).unwrap();
    assert!(!before.matches(&disk));
    assert!(matches!(
        copy_flash_disk(&before, &disk, |_, _| true),
        Err(DiskError::NotThisBoard)
    ));
    // Une carte SD non plus.
    let sd = inspect(&carte_sd("autre-sd", &[jeno()])).unwrap();
    assert!(matches!(
        copy_flash_disk(&before, &sd, |_, _| true),
        Err(DiskError::NotThisBoard)
    ));
}

#[test]
fn copie_du_disque_arretee() {
    let flash = deux_vols();
    let mut msp = Msp::new(SimBoard::new(flash.clone()));
    let before = reboot_to_disk(&mut msp).unwrap();
    let disk = inspect(&disque_memoire("arret", &flash)).unwrap();
    assert!(matches!(
        copy_flash_disk(&before, &disk, |_, _| false),
        Err(DiskError::Cancelled)
    ));
}

// ---------------------------------------------------------------- carte SD

fn logs_de(root: &Path) -> Vec<otb_usb::SdFile> {
    match inspect(root).unwrap().kind {
        DiskKind::SdCard { logs } => logs,
        k => panic!("{k:?}"),
    }
}

#[test]
fn carte_sd_copie_verifiee_puis_suppression() {
    let root = carte_sd("sd-parcours", &[jeno(), en_tete_seul(), deux_vols()]);
    let logs = logs_de(&root);
    assert_eq!(logs.len(), 3);
    let dest = dossier("sd-parcours-copies");
    let mut ok = Vec::new();
    let mut refus = Vec::new();
    for (k, f) in logs.iter().enumerate() {
        match copy_sd_log(f, &dest.join(format!("{k}.bfl")), |_| None, |_, _| true) {
            Ok(c) => ok.push(c),
            Err(e) => refus.push((f.path.clone(), e)),
        }
    }
    assert_eq!(ok.len(), 2);
    assert_eq!((ok[0].report().logs, ok[1].report().logs), (1, 2));
    assert_eq!(refus.len(), 1);
    assert!(
        matches!(refus[0].1.reason, CheckFailure::LogWithoutFragment { .. }),
        "{:?}",
        refus[0].1
    );
    let erased = delete_sd_logs(confirm_sd(ok));
    assert_eq!(erased.deleted.len(), 2);
    assert!(erased.kept.is_empty(), "{:?}", erased.kept);
    // Le log sans vol et le fichier qui n'est pas un log restent.
    assert!(!logs[0].path.exists() && !logs[2].path.exists());
    assert!(logs[1].path.exists());
    assert!(root.join("LOGS").join("NOTES.TXT").exists());
    // Les copies sont toujours là.
    assert!(dest.join("0.bfl").exists() && dest.join("2.bfl").exists());
}

/// ⚠️ Un fichier changé entre la copie et la suppression reste sur la carte.
#[test]
fn carte_sd_fichier_change_garde() {
    let root = carte_sd("sd-change", &[jeno()]);
    let logs = logs_de(&root);
    let dest = dossier("sd-change-copies").join("0.bfl");
    let c = copy_sd_log(&logs[0], &dest, |_| None, |_, _| true).unwrap();
    // Même taille, un octet différent : seule la relecture complète le voit.
    let mut bytes = std::fs::read(&logs[0].path).unwrap();
    let k = bytes.len() / 2;
    bytes[k] ^= 0xFF;
    std::fs::write(&logs[0].path, &bytes).unwrap();
    let erased = delete_sd_logs(confirm_sd(vec![c]));
    assert!(erased.deleted.is_empty());
    assert_eq!(erased.kept.len(), 1);
    assert!(logs[0].path.exists());
}

/// Un log déjà rangé (même contenu) n'est pas copié une seconde fois : on compare à la copie existante.
#[test]
fn carte_sd_log_deja_range() {
    let root = carte_sd("sd-deja", &[jeno()]);
    let logs = logs_de(&root);
    let range = dossier("sd-deja-biblio").join("ancien.bfl");
    std::fs::write(&range, jeno()).unwrap();
    let neuf = dossier("sd-deja-copies").join("0.bfl");
    let c = copy_sd_log(&logs[0], &neuf, |_| Some(range.clone()), |_, _| true).unwrap();
    assert_eq!(c.report().path, range);
    assert!(!neuf.exists(), "pas de seconde copie");
    // Une copie rangée qui ne correspond pas : refus.
    std::fs::write(&range, deux_vols()).unwrap();
    let e = copy_sd_log(&logs[0], &neuf, |_| Some(range.clone()), |_, _| true).unwrap_err();
    assert!(matches!(e.reason, CheckFailure::DiskMismatch), "{e:?}");
}

#[test]
fn carte_sd_copie_arretee_ou_fichier_existant() {
    let root = carte_sd("sd-arret", &[jeno()]);
    let logs = logs_de(&root);
    let dest = dossier("sd-arret-copies").join("0.bfl");
    let e = copy_sd_log(&logs[0], &dest, |_| None, |_, _| false).unwrap_err();
    assert!(matches!(e.reason, CheckFailure::Cancelled) && e.saved.is_none());
    assert!(!dest.exists());
    std::fs::write(&dest, b"autre").unwrap();
    let e = copy_sd_log(&logs[0], &dest, |_| None, |_, _| true).unwrap_err();
    assert!(matches!(e.reason, CheckFailure::AlreadyExists), "{e:?}");
    assert_eq!(std::fs::read(&dest).unwrap(), b"autre", "jamais écrasé");
}

/// ⚠️ La « copie rangée » proposée est le fichier de la carte lui-même (log ouvert depuis la carte) : une vraie copie
/// est écrite, sinon la suppression viserait le seul exemplaire.
#[test]
fn carte_sd_copie_rangee_qui_est_le_fichier_de_la_carte() {
    let root = carte_sd("sd-meme", &[jeno()]);
    let logs = logs_de(&root);
    let source = logs[0].path.clone();
    let neuf = dossier("sd-meme-copies").join("0.bfl");
    let c = copy_sd_log(&logs[0], &neuf, |_| Some(source.clone()), |_, _| true).unwrap();
    assert_eq!(c.report().path, neuf);
    assert!(neuf.exists());
    let erased = delete_sd_logs(confirm_sd(vec![c]));
    assert_eq!(erased.deleted, vec![source]);
    assert_eq!(std::fs::read(&neuf).unwrap(), jeno(), "le vol reste dans la copie");
}
