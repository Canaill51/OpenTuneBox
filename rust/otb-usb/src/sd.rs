//! Les logs d'une carte SD (lot 32c, D-075) : copiés un par un depuis le disque (carte dans le drone en mode stockage
//! de masse, ou dans un lecteur de cartes), vérifiés comme une copie de la mémoire interne, puis supprimés de la carte
//! **par le système** — Betaflight n'efface pas une carte SD en MSP (`docs/spec-usb.md` §5).
//!
//! ⚠️ **Un log perdu est un vol à refaire.** La suppression exige un [`SdErasePermit`], qu'on n'obtient qu'en passant,
//! dans cet ordre :
//!
//! 1. [`copy_sd_log`] — le fichier de la carte est lu, écrit sur le disque et relu à l'identique (ou comparé à la copie
//!    déjà rangée), puis le décodeur en tire au moins un fragment de chaque log : [`SdCopy`] ;
//! 2. [`confirm_sd`] — le pilote a vu le résultat et accepte ;
//! 3. [`delete_sd_logs`] — **juste avant** chaque suppression, le fichier de la carte est relu et comparé octet pour
//!    octet à sa copie : un fichier changé ou illisible reste sur la carte.
//!
//! Seuls les fichiers copiés et vérifiés partent ; rien d'autre de la carte n'est touché.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::erase::{CheckFailure, CopyReport, NotVerified, decode_check, save_and_reread};
use crate::msc::read_with_progress;

/// Un log sur la carte SD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdFile {
    pub path: PathBuf,
    pub len: u64,
}

/// Les logs du dossier `LOGS` d'une carte SD (`LOG00001.BFL`… écrits par `src/main/blackbox/blackbox_io.c`), par nom.
/// Les autres fichiers et les sous-dossiers sont ignorés.
pub fn sd_logs(logs_dir: &Path) -> std::io::Result<Vec<SdFile>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(logs_dir)? {
        let e = e?;
        let path = e.path();
        let ext_ok = path
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("bfl") || x.eq_ignore_ascii_case("bbl"));
        let m = e.metadata()?;
        if ext_ok && m.is_file() {
            out.push(SdFile { path, len: m.len() });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Un log de la carte SD copié et vérifié : il ne lui manque que l'accord du pilote.
#[derive(Debug)]
pub struct SdCopy {
    source: PathBuf,
    len: u64,
    report: CopyReport,
}

impl SdCopy {
    /// Le fichier sur la carte.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// La copie vérifiée (son chemin est celui de la copie, neuve ou déjà rangée).
    pub fn report(&self) -> &CopyReport {
        &self.report
    }
}

/// Copie un log de la carte dans `dest` (fichier neuf) et le vérifie. `existing(contenu)` rend la copie déjà rangée de
/// ce contenu, s'il y en a une (même empreinte dans la bibliothèque) : elle est alors comparée au fichier de la carte
/// au lieu d'en écrire une seconde. `progress(lus, total)` : `false` arrête, rien n'est gardé.
pub fn copy_sd_log(
    file: &SdFile,
    dest: &Path,
    existing: impl FnOnce(&[u8]) -> Option<PathBuf>,
    progress: impl FnMut(u64, u64) -> bool,
) -> Result<SdCopy, NotVerified> {
    let refuse = |saved: Option<&Path>, reason| NotVerified {
        saved: saved.map(Path::to_path_buf),
        reason,
    };
    let bytes = read_with_progress(&file.path, progress).map_err(|e| {
        refuse(
            None,
            if e.kind() == std::io::ErrorKind::Interrupted {
                CheckFailure::Cancelled
            } else {
                CheckFailure::Read(e)
            },
        )
    })?;
    if bytes.is_empty() {
        return Err(refuse(None, CheckFailure::Empty));
    }
    // Taille lue = taille annoncée par le système au relevé de la carte.
    let copied = bytes.len() as u64;
    if copied != file.len {
        return Err(refuse(
            None,
            CheckFailure::SizeMismatch {
                copied,
                announced: file.len,
            },
        ));
    }
    let path = match existing(&bytes) {
        Some(p) => {
            let on_disk = std::fs::read(&p).map_err(|e| refuse(None, CheckFailure::Write(e)))?;
            if on_disk != bytes {
                return Err(refuse(None, CheckFailure::DiskMismatch));
            }
            p
        }
        None => {
            save_and_reread(dest, &bytes).map_err(|(saved, reason)| refuse(saved.then_some(dest), reason))?;
            dest.to_path_buf()
        }
    };
    let (logs, fragments) = decode_check(&path, bytes).map_err(|reason| refuse(Some(&path), reason))?;
    Ok(SdCopy {
        source: file.path.clone(),
        len: file.len,
        report: CopyReport {
            path,
            bytes: copied,
            logs,
            fragments,
        },
    })
}

/// Le droit de supprimer de la carte, une fois, les logs copiés et vérifiés.
#[derive(Debug)]
pub struct SdErasePermit {
    files: Vec<(PathBuf, u64, PathBuf)>,
}

/// Le pilote a lu le résultat des copies et accepte de les supprimer de la carte. À n'appeler qu'après sa réponse.
pub fn confirm_sd(copies: Vec<SdCopy>) -> SdErasePermit {
    SdErasePermit {
        files: copies.into_iter().map(|c| (c.source, c.len, c.report.path)).collect(),
    }
}

/// Ce que la suppression a fait.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SdErased {
    pub deleted: Vec<PathBuf>,
    /// Restés sur la carte, et pourquoi.
    pub kept: Vec<(PathBuf, String)>,
}

/// ⚠️ **Le seul chemin qui supprime un fichier** (`tests/contrats.rs` le vérifie). Pour chaque log du permis : le fichier
/// de la carte est relu et comparé octet pour octet à sa copie, **puis** supprimé ; un fichier changé ou illisible
/// reste. Ensuite, le dossier est forcé sur le support quand le système le permet ; le pilote **éjecte** le disque avant
/// de débrancher.
pub fn delete_sd_logs(permit: SdErasePermit) -> SdErased {
    let mut out = SdErased::default();
    let mut dirs: Vec<PathBuf> = Vec::new();
    for (source, len, copy) in permit.files {
        match same_content(&source, len, &copy) {
            Ok(true) => match std::fs::remove_file(&source) {
                Ok(()) => {
                    if let Some(d) = source.parent()
                        && !dirs.iter().any(|x| x == d)
                    {
                        dirs.push(d.to_path_buf());
                    }
                    out.deleted.push(source);
                }
                Err(e) => out.kept.push((source, format!("suppression impossible : {e}"))),
            },
            Ok(false) => out
                .kept
                .push((source, "le fichier a changé depuis la copie".to_string())),
            Err(e) => out.kept.push((source, format!("relecture impossible : {e}"))),
        }
    }
    for d in dirs {
        sync_dir(&d);
    }
    out
}

/// Le fichier de la carte a-t-il toujours la taille et le contenu de sa copie ?
fn same_content(source: &Path, len: u64, copy: &Path) -> std::io::Result<bool> {
    if std::fs::metadata(source)?.len() != len || std::fs::metadata(copy)?.len() != len {
        return Ok(false);
    }
    let mut a = std::fs::File::open(source)?;
    let mut b = std::fs::File::open(copy)?;
    let mut ba = vec![0u8; crate::msc::CHUNK];
    let mut bb = vec![0u8; crate::msc::CHUNK];
    loop {
        let n = read_full(&mut a, &mut ba)?;
        let m = read_full(&mut b, &mut bb)?;
        if n != m || ba[..n] != bb[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}

/// Remplit `buf` autant que le fichier le permet (une lecture peut rendre moins sans être à la fin).
fn read_full(f: &mut std::fs::File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

/// Force l'écriture d'un dossier (ses entrées supprimées) sur le support. Sous Windows, `std` n'ouvre pas un dossier :
/// l'éjection du disque s'en charge.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}
