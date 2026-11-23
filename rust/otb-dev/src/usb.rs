//! `otb-dev usb` : essai à la main de la récupération par USB sur une vraie carte (lot 32a), avant l'écran du
//! lot 32b. Sert aussi aux mesures de `docs/spec-usb.md` (« Procédure d'essai à la main ») : débit de la lecture
//! MSP et durée d'un effacement.
//!
//! L'effacement passe par les mêmes contrôles que l'application (`otb_usb::erase`) et demande de taper `effacer`.

use std::io::{BufRead, Write};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use otb_usb::erase::{ErasePolling, erase_flash, verify_and_save};
use otb_usb::{Msp, SerialLink};

fn open(port: &str) -> Result<Msp<SerialLink>, String> {
    Ok(Msp::new(SerialLink::open(Path::new(port)).map_err(|e| e.to_string())?))
}

/// Les ports série, et pour chacun la carte Betaflight qui répond, s'il y en a une.
pub fn ports() -> Result<String, String> {
    let ports = otb_usb::link::list_ports().map_err(|e| format!("liste des ports impossible : {e}"))?;
    let ports = otb_usb::link::board_candidates(ports, cfg!(target_os = "linux"));
    if ports.is_empty() {
        return Ok("Aucun port série. Carte branchée ? Sous Windows, pilote du port USB virtuel installé ?".into());
    }
    let mut out = String::new();
    for p in ports {
        let state = match SerialLink::open(&p) {
            Err(e) => e.to_string(),
            Ok(link) => {
                let mut msp = Msp::new(link);
                msp.set_timeout(std::time::Duration::from_millis(300));
                match msp.api_version() {
                    Ok((a, b)) => format!("répond en MSP (API {a}.{b})"),
                    Err(e) => format!("ne répond pas en MSP ({e})"),
                }
            }
        };
        out.push_str(&format!("{} : {state}\n", p.display()));
    }
    Ok(out)
}

/// L'identifiant de la carte, sans l'afficher en entier : c'est un numéro de série fixe du matériel, et la sortie de
/// cet outil se recopie volontiers (issue, documentation publique). Une empreinte courte suffit à vérifier qu'il ne
/// change pas après un nouveau firmware (`docs/spec-usb.md`, procédure d'essai). Fonction pure.
pub fn chip_id_summary(chip_id: &otb_usb::ChipId) -> String {
    use sha2::{Digest, Sha256};
    if !chip_id.is_known() {
        return "nul".into();
    }
    let digest = Sha256::digest(chip_id.0.map(u32::to_le_bytes).concat());
    let short: String = digest[..4].iter().map(|b| format!("{b:02x}")).collect();
    format!("non nul, empreinte {short}")
}

/// Identité, état armé et mémoire.
pub fn info(port: &str) -> Result<String, String> {
    let mut msp = open(port)?;
    let id = msp.identify().map_err(|e| e.to_string())?;
    let armed = msp.armed().map_err(|e| e.to_string())?;
    let s = msp.flash_summary().map_err(|e| e.to_string())?;
    let sd = msp.sd_summary().map_err(|e| e.to_string())?;
    Ok(format!(
        "Firmware   : {} {}.{}.{} (API MSP {}.{})\nCarte      : {} ({})\nNom        : {}\nIdentifiant: {}{}\nArmé       : {}\n\
         Mémoire    : {}\nCarte SD   : {}\n",
        id.variant,
        id.version.0,
        id.version.1,
        id.version.2,
        id.api.0,
        id.api.1,
        id.board,
        id.target,
        if id.name.is_empty() { "(aucun)" } else { &id.name },
        chip_id_summary(&id.chip_id),
        if id.chip_id.is_known() {
            ""
        } else {
            " — ne sert pas de clé"
        },
        if armed { "OUI" } else { "non" },
        if s.supported {
            format!(
                "{} octets occupés sur {} ({} secteurs){}",
                s.used,
                s.total,
                s.sectors,
                if s.ready { "" } else { ", pas prête" }
            )
        } else {
            "aucune lisible en MSP (carte SD ?)".into()
        },
        match (sd.supported, sd.state) {
            (false, _) => "aucune".to_string(),
            (true, 4) => format!("prête, {} Mo libres sur {}", sd.free_kb / 1024, sd.total_kb / 1024),
            (true, 0) => "absente".into(),
            (true, 1) => "en panne ou illisible".into(),
            (true, _) => "en préparation".into(),
        },
    ))
}

/// Copie la mémoire dans `dir` (créé s'il manque), vérifie, mesure le débit ; avec `erase`, propose l'effacement.
pub fn copy(port: &str, dir: &Path, erase: bool) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    let mut msp = open(port)?;
    let start = Instant::now();
    let mut last = Instant::now();
    let copy = msp
        .copy_flash(|n, total| {
            if last.elapsed().as_millis() >= 500 || n == total {
                last = Instant::now();
                eprint!(
                    "\rCopie : {n} / {total} octets ({:.0} %)   ",
                    100.0 * n as f64 / total.max(1) as f64
                );
            }
            true
        })
        .map_err(|e| e.to_string())?;
    eprintln!();
    let secs = start.elapsed().as_secs_f64();
    let n = copy.bytes().len();
    // Mémoire vide : pas de débit à donner, `verify_and_save` dit pourquoi rien n'est copié.
    let mut out = if n == 0 {
        String::new()
    } else {
        format!(
            "Copié : {n} octets en {secs:.1} s, soit {:.2} Mo par minute.\n",
            n as f64 / 1_048_576.0 / (secs / 60.0).max(1e-9)
        )
    };
    let name = if copy.identity().name.is_empty() {
        "carte".to_string()
    } else {
        copy.identity().name.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    };
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let path = dir.join(format!("{name}-{stamp}.bbl"));
    let verified = match verify_and_save(copy, &path) {
        Ok(v) => v,
        Err(e) => return Err(format!("{out}{e}")),
    };
    let r = verified.report();
    out.push_str(&format!(
        "Vérifié : {} ({} logs, {} fragments de vol).\n",
        r.path.display(),
        r.logs,
        r.fragments
    ));
    if !erase {
        return Ok(out);
    }
    print!("{out}Retirer les hélices. Taper « effacer » pour effacer la mémoire de la carte : ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    if answer.trim() != "effacer" {
        return Ok("Effacement abandonné : rien n'a été effacé.".into());
    }
    let start = Instant::now();
    erase_flash(&mut msp, verified.confirm(), ErasePolling::default(), |t| {
        eprint!("\rEffacement en cours : {} s   ", t.as_secs());
    })
    .map_err(|e| e.to_string())?;
    eprintln!();
    let used = msp.flash_summary().map_err(|e| e.to_string())?.used;
    Ok(format!(
        "Effacé en {:.1} s ; place occupée relue : {used} octets.",
        start.elapsed().as_secs_f64()
    ))
}

/// Sauvegarde `diff all` dans `file` (jamais par-dessus un fichier existant). La carte redémarre ensuite.
pub fn diff(port: &str, file: &Path) -> Result<String, String> {
    let mut msp = open(port)?;
    msp.identify().map_err(|e| e.to_string())?;
    let text = msp.diff_all().map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file)
        .map_err(|e| format!("{} : {e}", file.display()))?;
    f.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok(format!(
        "{} lignes écrites dans {} ; la carte redémarre.",
        text.lines().count(),
        file.display()
    ))
}

/// Demande de taper « effacer » ; `false` pour toute autre réponse.
fn ask_erase(prompt: &str) -> Result<bool, String> {
    print!("{prompt} Taper « effacer » pour continuer : ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    Ok(answer.trim() == "effacer")
}

fn stamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Progression d'une copie depuis un disque, au plus deux fois par seconde.
fn disk_progress(what: &str) -> impl FnMut(u64, u64) -> bool + '_ {
    let mut last = Instant::now();
    move |n, total| {
        if last.elapsed().as_millis() >= 500 || n == total {
            last = Instant::now();
            eprint!(
                "\r{what} : {n} / {total} octets ({:.0} %)   ",
                100.0 * n as f64 / total.max(1) as f64
            );
        }
        true
    }
}

/// Mode stockage de masse (lot 32c) : la carte redémarre en disque, on attend que le système le monte, on copie et on
/// vérifie ; avec `erase`, la mémoire interne s'efface après rebranchement (en MSP, même carte), une carte SD par la
/// suppression des fichiers vérifiés.
pub fn msc(port: &str, dir: &Path, erase: bool) -> Result<String, String> {
    use otb_usb::msc::{DiskKind, Storage, copy_flash_disk, mounted_volumes, new_disks, reboot_to_disk};
    std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    let volumes = mounted_volumes();
    let mut msp = open(port)?;
    let before = reboot_to_disk(&mut msp).map_err(|e| e.to_string())?;
    drop(msp);
    let what = match before.storage {
        Storage::Flash(s) => format!("mémoire interne, {} octets occupés", s.used),
        Storage::SdCard(s) => format!("carte SD, {} Mo libres sur {}", s.free_kb / 1024, s.total_kb / 1024),
    };
    eprintln!("La carte redémarre en disque ({what}). Attente du disque…");
    let start = Instant::now();
    let mut hinted = false;
    let disk = loop {
        let found: Vec<_> = new_disks(&volumes).into_iter().filter(|d| before.matches(d)).collect();
        if let [d] = found.as_slice() {
            break d.clone();
        }
        if found.len() > 1 {
            return Err(format!(
                "Plusieurs disques de carte apparus : {:?}. N'en garder qu'un.",
                found.iter().map(|d| &d.root).collect::<Vec<_>>()
            ));
        }
        let waited = start.elapsed().as_secs();
        if waited >= 10 && !hinted {
            hinted = true;
            eprintln!("Pas encore monté : l'ouvrir dans le gestionnaire de fichiers (disque « BETAFLT » ou carte SD).");
        }
        if waited >= 120 {
            return Err("Aucun disque de carte monté en 2 minutes.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    };
    let mut out = format!(
        "Disque trouvé en {:.1} s : {}\n",
        start.elapsed().as_secs_f64(),
        disk.root.display()
    );
    match &disk.kind {
        DiskKind::Flash { .. } => {
            let t = Instant::now();
            let copy = copy_flash_disk(&before, &disk, disk_progress("Copie")).map_err(|e| e.to_string())?;
            eprintln!();
            let n = copy.bytes().len();
            let secs = t.elapsed().as_secs_f64();
            out.push_str(&format!(
                "Copié : {n} octets en {secs:.1} s, soit {:.2} Mo par minute.\n",
                n as f64 / 1_048_576.0 / (secs / 60.0).max(1e-9)
            ));
            let path = dir.join(format!("disque-{}.bbl", stamp()));
            let verified = verify_and_save(copy, &path).map_err(|e| format!("{out}{e}"))?;
            let r = verified.report();
            out.push_str(&format!(
                "Vérifié : {} ({} logs, {} fragments de vol).\n",
                r.path.display(),
                r.logs,
                r.fragments
            ));
            if !erase {
                return Ok(out);
            }
            print!("{out}");
            if !ask_erase(
                "Débrancher la carte, la rebrancher (sans batterie, hélices retirées), attendre son port série.",
            )? {
                return Ok("Effacement abandonné : rien n'a été effacé.".into());
            }
            let mut msp = open(port)?;
            let t = Instant::now();
            erase_flash(&mut msp, verified.confirm(), ErasePolling::default(), |w| {
                eprint!("\rEffacement en cours : {} s   ", w.as_secs());
            })
            .map_err(|e| e.to_string())?;
            eprintln!();
            let used = msp.flash_summary().map_err(|e| e.to_string())?.used;
            Ok(format!(
                "Effacé en {:.1} s ; place occupée relue : {used} octets.",
                t.elapsed().as_secs_f64()
            ))
        }
        DiskKind::SdCard { .. } => {
            let done = sd_copy_and_erase(&disk, dir, erase)?;
            Ok(format!("{out}{done}"))
        }
    }
}

/// Carte SD dans un lecteur de cartes (lot 32c) : `root` est la racine du disque monté.
pub fn sd(root: &Path, dir: &Path, erase: bool) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    let disk = otb_usb::msc::inspect(root)
        .filter(|d| matches!(d.kind, otb_usb::msc::DiskKind::SdCard { .. }))
        .ok_or_else(|| {
            format!(
                "{} : pas de dossier LOGS, ce n'est pas une carte SD Betaflight.",
                root.display()
            )
        })?;
    sd_copy_and_erase(&disk, dir, erase)
}

fn sd_copy_and_erase(disk: &otb_usb::msc::Disk, dir: &Path, erase: bool) -> Result<String, String> {
    use otb_usb::sd::{confirm_sd, copy_sd_log, delete_sd_logs};
    let otb_usb::msc::DiskKind::SdCard { logs } = &disk.kind else {
        return Err("pas une carte SD".into());
    };
    if logs.is_empty() {
        return Ok("Aucun log sur la carte SD : rien à copier.\n".into());
    }
    let mut out = String::new();
    let mut ok = Vec::new();
    let stamp = stamp();
    let t = Instant::now();
    let mut bytes = 0;
    for f in logs {
        let name = f
            .path
            .file_stem()
            .map_or("log".into(), |s| s.to_string_lossy().into_owned());
        let dest = dir.join(format!("{name}-{stamp}.bfl"));
        match copy_sd_log(f, &dest, |_| None, disk_progress(&name)) {
            Ok(c) => {
                eprintln!();
                bytes += c.report().bytes;
                out.push_str(&format!(
                    "Vérifié : {} → {} ({} logs, {} fragments de vol).\n",
                    f.path.display(),
                    c.report().path.display(),
                    c.report().logs,
                    c.report().fragments
                ));
                ok.push(c);
            }
            Err(e) => {
                eprintln!();
                out.push_str(&format!("Refusé : {} — {e}\n", f.path.display()));
            }
        }
    }
    let secs = t.elapsed().as_secs_f64();
    out.push_str(&format!(
        "{} logs sur {} vérifiés, {bytes} octets en {secs:.1} s ({:.2} Mo par minute).\n",
        ok.len(),
        logs.len(),
        bytes as f64 / 1_048_576.0 / (secs / 60.0).max(1e-9)
    ));
    if !erase || ok.is_empty() {
        return Ok(out);
    }
    print!("{out}");
    if !ask_erase(&format!("Supprimer de la carte SD les {} logs vérifiés.", ok.len()))? {
        return Ok("Suppression abandonnée : rien n'a été supprimé.".into());
    }
    let r = delete_sd_logs(confirm_sd(ok));
    let mut done = format!("{} logs supprimés de la carte.\n", r.deleted.len());
    for (p, why) in r.kept {
        done.push_str(&format!("Gardé : {} — {why}\n", p.display()));
    }
    done.push_str("Éjecter le disque avant de débrancher.");
    Ok(done)
}
