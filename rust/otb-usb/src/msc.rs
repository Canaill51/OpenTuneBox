//! Le mode stockage de masse (lot 32c, D-075) : la carte redémarre en disque USB, le système le monte, et OpenTuneBox
//! y lit les logs — bien plus vite qu'en MSP, et seule voie pour une carte SD.
//!
//! - [`reboot_to_disk`] : lit ce qui désignera la carte (identité, mémoire ou carte SD), puis la fait redémarrer en
//!   disque. ⚠️ Le seul redémarrage envoyé est celui du mode stockage de masse (jamais le chargeur de firmware).
//! - [`mounted_volumes`], [`inspect`], [`new_disks`] : trouver le disque monté par le système.
//! - [`copy_flash_disk`] : lire `BTFL_ALL.BBL`, toute la place occupée de la mémoire, octet pour octet comme la copie
//!   MSP ; elle passe ensuite par les mêmes contrôles ([`crate::erase::verify_and_save`]).
//!
//! ⚠️ Le disque de la mémoire interne est **en lecture seule** : l'effacer demande de débrancher puis rebrancher la
//! carte, et de passer par [`crate::erase::erase_flash`], qui vérifie l'identifiant gravé (`docs/spec-usb.md` §1).
//! Les logs d'une carte SD s'effacent par le système : [`crate::sd`].

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::client::{Command, FlashSummary, Identity, Msp, MspError, new_session};
use crate::erase::FlashCopy;
use crate::link::Link;
use crate::sd::{SdFile, sd_logs};

/// Mode « stockage de masse » de `MSP_REBOOT` (`src/main/msp/msp.c`, `MSP_REBOOT_MSC`). Le mode 3 (heure UTC) ne
/// dit pas si le disque est prêt : on garde le 2.
const REBOOT_MSC: u8 = 2;

/// Première version de l'API MSP dont `MSP_REBOOT` connaît le mode stockage de masse (Betaflight 3.5, `df55fc99e`,
/// 2018-07-15). ⚠️ Plus ancien, le firmware ignore le mode et **redémarre normalement** : ne jamais le lui demander
/// (`tests/stockage_de_masse.rs`).
pub const DISK_REBOOT_API: (u8, u8) = (1, 40);

/// La carte sait-elle redémarrer en disque sur demande ? Faux si son API est plus ancienne que [`DISK_REBOOT_API`].
pub fn knows_disk_reboot(api: (u8, u8)) -> bool {
    api >= DISK_REBOOT_API
}

/// Nom du fichier de toute la mémoire sur le disque de la carte (`src/main/msc/emfat_file.c`).
pub const FLASH_ALL: &str = "BTFL_ALL.BBL";

/// Morceau de lecture d'un fichier du disque : la progression se donne entre deux.
pub(crate) const CHUNK: usize = 1 << 20;

/// État de la carte SD (`MSP_SDCARD_SUMMARY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SdSummary {
    /// La carte est faite pour une carte SD (`sdcard_mode` réglé).
    pub supported: bool,
    /// 0 absente, 1 en panne, 2 et 3 en préparation, 4 prête (`MSP_SDCARD_STATE_*`).
    pub state: u8,
    pub free_kb: u32,
    pub total_kb: u32,
}

impl SdSummary {
    pub fn ready(&self) -> bool {
        self.supported && self.state == 4
    }
}

/// Ce que la carte enregistre, lu avant de devenir un disque.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// Mémoire interne : sa place occupée est la taille attendue de `BTFL_ALL.BBL`.
    Flash(FlashSummary),
    SdCard(SdSummary),
}

/// Ce qu'on sait de la carte avant qu'elle devienne un disque : c'est tout ce qui la désignera ensuite (le disque, lui,
/// ne porte pas son identifiant).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeforeDisk {
    pub identity: Identity,
    pub storage: Storage,
}

impl BeforeDisk {
    /// Ce disque est-il bien celui de cette carte ? Mémoire interne : `BTFL_ALL.BBL` a la taille de la place occupée.
    pub fn matches(&self, disk: &Disk) -> bool {
        match (&self.storage, &disk.kind) {
            (Storage::Flash(s), DiskKind::Flash { len, .. }) => *len == u64::from(s.used),
            (Storage::SdCard(_), DiskKind::SdCard { .. }) => true,
            _ => false,
        }
    }
}

/// Pourquoi la carte ne devient pas un disque. Dans tous les cas, elle n'a **pas** redémarré.
#[derive(Debug)]
pub enum DiskRebootError {
    Msp(MspError),
    /// Le drone est armé : aucune opération.
    Armed,
    /// Ni mémoire interne ni carte SD.
    NoStorage,
    /// Mémoire interne vide : rien à copier.
    Empty,
    /// Carte SD absente ou pas prête (`state` de [`SdSummary`]).
    SdNotReady {
        state: u8,
    },
    /// Le firmware n'a pas le mode stockage de masse (compilé sans lui, ou API plus ancienne que 1.40) : rester en MSP.
    NotSupported,
    /// Le firmware dit son disque pas prêt : il ne redémarre pas.
    NotReady,
}

impl std::fmt::Display for DiskRebootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiskRebootError::Msp(e) => write!(f, "{e}"),
            DiskRebootError::Armed => write!(f, "Le drone est armé : rien n'est lu. Le désarmer, puis réessayer."),
            DiskRebootError::NoStorage => write!(f, "Cette carte n'a ni mémoire de logs ni carte SD."),
            DiskRebootError::Empty => write!(f, "La mémoire du drone est vide : rien à copier."),
            DiskRebootError::SdNotReady { state } => write!(
                f,
                "La carte SD n'est pas prête ({}) : la vérifier, puis réessayer.",
                match state {
                    0 => "absente",
                    1 => "en panne ou illisible",
                    _ => "en préparation",
                }
            ),
            DiskRebootError::NotSupported => write!(
                f,
                "Ce firmware n'a pas le mode stockage de masse : copier par USB série (MSP)."
            ),
            DiskRebootError::NotReady => write!(
                f,
                "La carte dit son disque pas prêt : elle n'a pas redémarré. Réessayer, ou copier par USB série (MSP)."
            ),
        }
    }
}

impl std::error::Error for DiskRebootError {}

impl From<MspError> for DiskRebootError {
    fn from(e: MspError) -> Self {
        DiskRebootError::Msp(e)
    }
}

impl<L: Link> Msp<L> {
    /// État de la carte SD. Un firmware qui refuse la commande n'a pas de carte SD.
    pub fn sd_summary(&mut self) -> Result<SdSummary, MspError> {
        let p = match self.request(Command::SdcardSummary, &[]) {
            Err(MspError::Refused { .. }) => vec![0; 11],
            r => r?,
        };
        if p.len() < 11 {
            return Err(MspError::Malformed {
                cmd: Command::SdcardSummary.code(),
                why: format!("{} octets reçus, 11 attendus", p.len()),
            });
        }
        let le = |at: usize| u32::from_le_bytes([p[at], p[at + 1], p[at + 2], p[at + 3]]);
        Ok(SdSummary {
            supported: p[0] & 1 != 0,
            state: p[1],
            free_kb: le(3),
            total_kb: le(7),
        })
    }
}

/// Lit l'identité et ce que la carte enregistre, puis la fait redémarrer en disque USB. Ensuite, le port série
/// disparaît : abandonner la connexion et attendre le disque ([`new_disks`]).
///
/// La mémoire interne passe avant la carte SD quand les deux existent et que la mémoire n'est pas vide (le disque
/// montre celle que `blackbox_device` désigne, que MSP ne lit pas : [`BeforeDisk::matches`] le vérifie).
pub fn reboot_to_disk<L: Link>(msp: &mut Msp<L>) -> Result<BeforeDisk, DiskRebootError> {
    let identity = msp.identify()?;
    if !knows_disk_reboot(identity.api) {
        return Err(DiskRebootError::NotSupported);
    }
    if msp.armed()? {
        return Err(DiskRebootError::Armed);
    }
    let flash = msp.flash_summary()?;
    let storage = if flash.supported && flash.used > 0 {
        Storage::Flash(flash)
    } else {
        let sd = msp.sd_summary()?;
        if sd.supported {
            if !sd.ready() {
                return Err(DiskRebootError::SdNotReady { state: sd.state });
            }
            Storage::SdCard(sd)
        } else if flash.supported {
            return Err(DiskRebootError::Empty);
        } else {
            return Err(DiskRebootError::NoStorage);
        }
    };
    // ⚠️ Un seul mode, le stockage de masse : jamais le chargeur de firmware (`tests/contrats.rs`).
    let reply = match msp.request(Command::Reboot, &[REBOOT_MSC]) {
        Err(MspError::Refused { .. }) => return Err(DiskRebootError::NotSupported),
        r => r?,
    };
    // Réponse : le mode accepté, puis « disque prêt » ; pas prêt, le firmware ne redémarre pas.
    match reply.as_slice() {
        [REBOOT_MSC, 1, ..] => Ok(BeforeDisk { identity, storage }),
        [REBOOT_MSC, 0, ..] => Err(DiskRebootError::NotReady),
        _ => Err(DiskRebootError::Msp(MspError::Malformed {
            cmd: Command::Reboot.code(),
            why: format!("réponse {reply:?} au redémarrage en stockage de masse"),
        })),
    }
}

// ---------------------------------------------------------------- disques montés

/// Ce que contient un disque reconnu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiskKind {
    /// Mémoire interne d'une carte en mode disque : `BTFL_ALL.BBL` et sa taille.
    Flash { all: PathBuf, len: u64 },
    /// Carte SD (dans le drone ou dans un lecteur) : ses logs `LOGS/*.BFL`, peut-être aucun.
    SdCard { logs: Vec<SdFile> },
}

/// Un disque où se trouvent des logs Betaflight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disk {
    pub root: PathBuf,
    pub kind: DiskKind,
}

/// L'entrée de `dir` nommée `name`, sans tenir compte de la casse (FAT : selon les options de montage, `BTFL_ALL.BBL`
/// peut apparaître en minuscules).
pub(crate) fn entry_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
}

/// Reconnaît un disque de carte Betaflight : `BTFL_ALL.BBL` à la racine (mémoire interne), ou un dossier `LOGS`
/// (carte SD). Rend `None` pour tout autre disque.
pub fn inspect(root: &Path) -> Option<Disk> {
    if let Some(all) = entry_ci(root, FLASH_ALL)
        && let Ok(m) = std::fs::metadata(&all)
        && m.is_file()
    {
        return Some(Disk {
            root: root.to_path_buf(),
            kind: DiskKind::Flash { all, len: m.len() },
        });
    }
    let logs = entry_ci(root, "LOGS").filter(|p| p.is_dir())?;
    Some(Disk {
        root: root.to_path_buf(),
        kind: DiskKind::SdCard {
            logs: sd_logs(&logs).ok()?,
        },
    })
}

/// Les disques de carte Betaflight parmi `roots`.
pub fn find_disks(roots: impl IntoIterator<Item = PathBuf>) -> Vec<Disk> {
    roots.into_iter().filter_map(|r| inspect(&r)).collect()
}

/// Les disques de carte apparus depuis `before` (relevé de [`mounted_volumes`] pris avant le redémarrage).
pub fn new_disks(before: &[PathBuf]) -> Vec<Disk> {
    find_disks(mounted_volumes().into_iter().filter(|v| !before.contains(v)))
}

/// Les volumes montés où chercher un disque de carte : systèmes de fichiers FAT sous Linux, lettres de lecteur sous
/// Windows, `/Volumes` sous macOS. Ne lit aucun fichier.
pub fn mounted_volumes() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/mountinfo")
            .map(|t| parse_mountinfo(&t))
            .unwrap_or_default()
    }
    #[cfg(windows)]
    {
        // A: et B: sont des lecteurs de disquettes : les interroger peut bloquer.
        (b'C'..=b'Z')
            .map(|c| PathBuf::from(format!("{}:\\", c as char)))
            .filter(|p| p.exists())
            .collect()
    }
    #[cfg(target_os = "macos")]
    {
        std::fs::read_dir("/Volumes")
            .map(|d| d.filter_map(Result::ok).map(|e| e.path()).collect())
            .unwrap_or_default()
    }
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        Vec::new()
    }
}

/// Systèmes de fichiers d'un disque de carte : FAT (mémoire interne émulée, carte SD), exFAT pour une grosse carte SD
/// dans un lecteur (`fuseblk` quand il passe par FUSE).
const FAT_TYPES: [&str; 4] = ["vfat", "msdos", "exfat", "fuseblk"];

/// Points de montage FAT d'un texte `/proc/self/mountinfo` (`man 5 proc` : 5ᵉ champ, type après le « - »). Les
/// espaces des chemins y sont écrits `\040`. Fonction pure.
pub fn parse_mountinfo(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| {
            let (left, right) = line.split_once(" - ")?;
            let fs = right.split(' ').next()?;
            if !FAT_TYPES.contains(&fs) {
                return None;
            }
            let point = left.split(' ').nth(4)?;
            Some(PathBuf::from(unescape_octal(point)))
        })
        .collect()
}

/// `\040` → espace, etc. (échappement du noyau dans `mountinfo`).
fn unescape_octal(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 3 < b.len() && b[i + 1..=i + 3].iter().all(|c| (b'0'..=b'7').contains(c)) {
            out.push((b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0'));
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------- copie depuis le disque

/// Lit un fichier par morceaux ; `progress(lus, total)` après chaque morceau, `false` arrête (rend `Interrupted`).
pub(crate) fn read_with_progress(path: &Path, mut progress: impl FnMut(u64, u64) -> bool) -> std::io::Result<Vec<u8>> {
    let mut f = std::fs::File::open(path)?;
    let total = f.metadata()?.len();
    let mut bytes = Vec::with_capacity(total as usize);
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            return Ok(bytes);
        }
        bytes.extend_from_slice(&buf[..n]);
        if !progress(bytes.len() as u64, total) {
            return Err(std::io::Error::from(std::io::ErrorKind::Interrupted));
        }
    }
}

/// Pourquoi la lecture du disque n'a pas donné de copie.
#[derive(Debug)]
pub enum DiskError {
    /// Ce disque n'est pas la mémoire interne de cette carte.
    NotThisBoard,
    Io(std::io::Error),
    /// L'utilisateur a arrêté la copie : rien n'est gardé.
    Cancelled,
}

impl std::fmt::Display for DiskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiskError::NotThisBoard => write!(
                f,
                "Ce disque n'est pas la mémoire de la carte : sa taille ne correspond pas à la place occupée annoncée."
            ),
            DiskError::Io(e) => write!(f, "Lecture du disque de la carte impossible : {e}."),
            DiskError::Cancelled => write!(f, "Copie arrêtée : rien n'est gardé, la prochaine reprendra au début."),
        }
    }
}

impl std::error::Error for DiskError {}

/// Lit `BTFL_ALL.BBL` : la même copie que [`Msp::copy_flash`], à vérifier par
/// [`crate::erase::verify_and_save`]. Le permis d'effacer qui en sortira ne vaudra que pour une carte d'identifiant
/// connu, après rebranchement (une copie lue sur le disque n'a pas de connexion MSP).
pub fn copy_flash_disk(
    before: &BeforeDisk,
    disk: &Disk,
    progress: impl FnMut(u64, u64) -> bool,
) -> Result<FlashCopy, DiskError> {
    let (Storage::Flash(summary), DiskKind::Flash { all, .. }) = (&before.storage, &disk.kind) else {
        return Err(DiskError::NotThisBoard);
    };
    if !before.matches(disk) {
        return Err(DiskError::NotThisBoard);
    }
    let bytes = read_with_progress(all, progress).map_err(|e| match e.kind() {
        std::io::ErrorKind::Interrupted => DiskError::Cancelled,
        _ => DiskError::Io(e),
    })?;
    Ok(FlashCopy::new(before.identity.clone(), *summary, bytes, new_session()))
}
