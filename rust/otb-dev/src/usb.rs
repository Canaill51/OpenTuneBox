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

/// Identité, état armé et mémoire.
pub fn info(port: &str) -> Result<String, String> {
    let mut msp = open(port)?;
    let id = msp.identify().map_err(|e| e.to_string())?;
    let armed = msp.armed().map_err(|e| e.to_string())?;
    let s = msp.flash_summary().map_err(|e| e.to_string())?;
    Ok(format!(
        "Firmware   : {} {}.{}.{} (API MSP {}.{})\nCarte      : {} ({})\nNom        : {}\nIdentifiant: {}{}\nArmé       : {}\n\
         Mémoire    : {}\n",
        id.variant,
        id.version.0,
        id.version.1,
        id.version.2,
        id.api.0,
        id.api.1,
        id.board,
        id.target,
        if id.name.is_empty() { "(aucun)" } else { &id.name },
        id.uid.to_hex(),
        if id.uid.is_known() {
            ""
        } else {
            " — nul : ne sert pas de clé"
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
    ))
}

/// Copie la mémoire dans `dir`, vérifie, mesure le débit ; avec `erase`, propose l'effacement.
pub fn copy(port: &str, dir: &Path, erase: bool) -> Result<String, String> {
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
    let mut out = format!(
        "Copié : {n} octets en {secs:.1} s, soit {:.2} Mo par minute.\n",
        n as f64 / 1_048_576.0 / (secs / 60.0).max(1e-9)
    );
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
