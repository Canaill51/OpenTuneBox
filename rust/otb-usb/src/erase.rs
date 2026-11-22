//! Les contrôles avant effacement, et le seul chemin qui efface (lot 32, D-063).
//!
//! ⚠️ **Un log perdu est un vol à refaire ; un effacement reporté ne coûte rien.** L'effacement exige un
//! [`ErasePermit`], qu'on n'obtient qu'en passant, dans cet ordre :
//!
//! 1. [`FlashCopy`] — la copie, rendue par [`crate::client::Msp::copy_flash`] ;
//! 2. [`verify_and_save`] — fichier écrit sur le disque et relu à l'identique, taille copiée = place annoncée, le
//!    décodeur ouvre chaque log du fichier relu et en tire au moins un fragment ;
//! 3. [`VerifiedCopy::confirm`] — le pilote a vu le résultat et accepte ;
//! 4. [`erase_flash`] — relit, **juste avant** d'envoyer l'effacement : même carte, place occupée inchangée (aucun
//!    vol enregistré depuis la copie), drone **désarmé** (le firmware ne le vérifie pas, `docs/spec-usb.md`).
//!
//! Aucun de ces types ne se fabrique hors de ce module, et aucun ne se copie : un permis sert une fois.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::client::{Command, FlashSummary, Identity, Msp, MspError, Uid};
use crate::link::Link;

/// Le contenu de la mémoire, tel que copié, avec ce que la carte annonçait au début de la copie.
pub struct FlashCopy {
    identity: Identity,
    summary: FlashSummary,
    bytes: Vec<u8>,
    session: u64,
}

impl FlashCopy {
    pub(crate) fn new(identity: Identity, summary: FlashSummary, bytes: Vec<u8>, session: u64) -> Self {
        FlashCopy {
            identity,
            summary,
            bytes,
            session,
        }
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Place occupée annoncée par la carte au début de la copie.
    pub fn announced(&self) -> u32 {
        self.summary.used
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Pourquoi une copie n'ouvre pas l'effacement.
#[derive(Debug)]
pub enum CheckFailure {
    /// Mémoire vide : rien à copier, rien à effacer.
    Empty,
    /// Le fichier existe déjà : OpenTuneBox n'écrase jamais un log.
    AlreadyExists,
    Write(std::io::Error),
    /// Le fichier relu ne contient pas ce qui a été copié.
    DiskMismatch,
    /// Moins (ou plus) d'octets copiés que la place annoncée.
    SizeMismatch {
        copied: u64,
        announced: u64,
    },
    /// Le décodeur refuse le fichier.
    Decode(String),
    /// Le début d'un log n'a donné aucun fragment de vol.
    LogWithoutFragment {
        log: usize,
        logs: usize,
    },
    /// Aucun début de log dans le fichier.
    NoLog,
}

impl std::fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckFailure::Empty => write!(f, "La mémoire est vide : rien à copier."),
            CheckFailure::AlreadyExists => write!(f, "Un fichier porte déjà ce nom : OpenTuneBox ne l'écrase pas."),
            CheckFailure::Write(e) => write!(f, "Écriture du fichier impossible : {e}."),
            CheckFailure::DiskMismatch => write!(f, "Le fichier relu sur le disque ne correspond pas à la copie."),
            CheckFailure::SizeMismatch { copied, announced } => write!(
                f,
                "{copied} octets copiés pour {announced} annoncés par la carte : copie incomplète."
            ),
            CheckFailure::Decode(e) => write!(f, "Le décodeur ne lit pas la copie : {e}."),
            CheckFailure::LogWithoutFragment { log, logs } => {
                write!(f, "Le log {log} sur {logs} ne contient aucun vol lisible.")
            }
            CheckFailure::NoLog => write!(f, "Aucun log Blackbox dans la copie."),
        }
    }
}

/// Une copie qui n'a pas passé les contrôles. Le fichier, s'il a été écrit, **reste** sur le disque : un log
/// incomplet vaut mieux que pas de log. Seul l'effacement est refusé.
#[derive(Debug)]
pub struct NotVerified {
    pub saved: Option<PathBuf>,
    pub reason: CheckFailure,
}

impl std::fmt::Display for NotVerified {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} Effacement refusé.", self.reason)?;
        if let Some(p) = &self.saved {
            write!(f, " La copie est gardée dans {}.", p.display())?;
        }
        Ok(())
    }
}

impl std::error::Error for NotVerified {}

/// Ce que les contrôles ont trouvé, à montrer au pilote avant qu'il confirme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyReport {
    pub path: PathBuf,
    pub bytes: u64,
    /// Débuts de log (`H Product:Blackbox`) trouvés dans le fichier.
    pub logs: usize,
    /// Fragments de vol tirés par le décodeur.
    pub fragments: usize,
}

/// Une copie écrite, relue et décodée : il ne lui manque que l'accord du pilote.
pub struct VerifiedCopy {
    uid: Uid,
    used: u32,
    session: u64,
    report: CopyReport,
}

impl VerifiedCopy {
    pub fn report(&self) -> &CopyReport {
        &self.report
    }

    /// Le pilote a lu le résultat et accepte d'effacer. À n'appeler qu'après sa réponse.
    pub fn confirm(self) -> ErasePermit {
        ErasePermit {
            uid: self.uid,
            used: self.used,
            session: self.session,
        }
    }
}

/// Le droit d'effacer, une fois, la mémoire d'où vient une copie vérifiée.
#[derive(Debug)]
pub struct ErasePermit {
    uid: Uid,
    used: u32,
    session: u64,
}

/// Écrit la copie dans `path` (jamais par-dessus un fichier existant), la relit, puis la fait lire au décodeur.
pub fn verify_and_save(copy: FlashCopy, path: &Path) -> Result<VerifiedCopy, NotVerified> {
    let refuse = |saved: bool, reason| NotVerified {
        saved: saved.then(|| path.to_path_buf()),
        reason,
    };
    if copy.bytes.is_empty() {
        return Err(refuse(false, CheckFailure::Empty));
    }
    // 1. Sur le disque : écrit, forcé jusqu'au support, relu à l'identique.
    write_new(path, &copy.bytes).map_err(|e| {
        let already = e.kind() == std::io::ErrorKind::AlreadyExists;
        refuse(
            false,
            if already {
                CheckFailure::AlreadyExists
            } else {
                CheckFailure::Write(e)
            },
        )
    })?;
    let on_disk = std::fs::read(path).map_err(|e| refuse(true, CheckFailure::Write(e)))?;
    if on_disk != copy.bytes {
        return Err(refuse(true, CheckFailure::DiskMismatch));
    }
    drop(on_disk);
    // 2. Taille copiée = place annoncée.
    let copied = copy.bytes.len() as u64;
    let announced = u64::from(copy.summary.used);
    if copied != announced {
        return Err(refuse(true, CheckFailure::SizeMismatch { copied, announced }));
    }
    let data = copy.bytes;
    // 3. Le décodeur lit le fichier **relu** et tire au moins un fragment de chaque log.
    let logs = otb_decoder::header::find_log_starts(&data).len();
    drop(data);
    if logs == 0 {
        return Err(refuse(true, CheckFailure::NoLog));
    }
    let flights = otb_log::flightlog::load(path).map_err(|e| refuse(true, CheckFailure::Decode(e.to_string())))?;
    if let Some(log) = (1..=logs).find(|k| !flights.iter().any(|f| f.session == *k)) {
        return Err(refuse(true, CheckFailure::LogWithoutFragment { log, logs }));
    }
    Ok(VerifiedCopy {
        uid: copy.identity.uid,
        used: copy.summary.used,
        session: copy.session,
        report: CopyReport {
            path: path.to_path_buf(),
            bytes: copied,
            logs,
            fragments: flights.len(),
        },
    })
}

/// Écrit un fichier neuf et le force sur le support. ⚠️ `create_new` : un log existant n'est jamais écrasé.
fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}

/// Pourquoi l'effacement n'est pas parti, ou ne s'est pas terminé.
#[derive(Debug)]
pub enum EraseError {
    Msp(MspError),
    /// Le drone est armé : aucune opération.
    Armed,
    /// Ce n'est pas la carte qui a été copiée.
    OtherBoard,
    /// La place occupée a changé depuis la copie : un vol a été enregistré (ou la mémoire effacée ailleurs).
    Changed {
        copied: u32,
        now: u32,
    },
    /// La mémoire n'est pas prête (un effacement est déjà en cours ?).
    NotReady,
    /// Effacement envoyé, mais pas fini dans le délai : la mémoire est peut-être à moitié effacée.
    Timeout {
        waited: Duration,
    },
}

impl std::fmt::Display for EraseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EraseError::Msp(e) => write!(f, "{e} Rien n'a été effacé."),
            EraseError::Armed => write!(f, "Le drone est armé : effacement refusé. Le désarmer, puis réessayer."),
            EraseError::OtherBoard => write!(
                f,
                "Ce n'est plus la carte qui a été copiée : effacement refusé. Recommencer la copie."
            ),
            EraseError::Changed { copied, now } => write!(
                f,
                "La mémoire a changé depuis la copie ({copied} octets copiés, {now} occupés maintenant) : \
                 effacement refusé. Recommencer la copie."
            ),
            EraseError::NotReady => write!(f, "La mémoire de la carte n'est pas prête : effacement refusé."),
            EraseError::Timeout { waited } => write!(
                f,
                "Effacement envoyé, mais pas terminé après {} s. Débrancher puis rebrancher la carte, et relire la mémoire.",
                waited.as_secs()
            ),
        }
    }
}

impl std::error::Error for EraseError {}

impl From<MspError> for EraseError {
    fn from(e: MspError) -> Self {
        EraseError::Msp(e)
    }
}

/// Cadence de l'attente de fin d'effacement.
#[derive(Clone, Copy, Debug)]
pub struct ErasePolling {
    /// Entre deux relectures du résumé (500 ms comme le configurateur).
    pub interval: Duration,
    /// Délai maximal ; `None` : celui du configurateur, 200 s par tranche de 16 Mo plus 100 s (`docs/spec-usb.md` §3).
    pub limit: Option<Duration>,
}

impl Default for ErasePolling {
    fn default() -> Self {
        ErasePolling {
            interval: Duration::from_millis(500),
            limit: None,
        }
    }
}

/// Délai du configurateur pour une mémoire de `total` octets.
pub fn erase_limit(total: u32) -> Duration {
    const SLICE: u64 = 16 * 1024 * 1024;
    let slices = u64::from(total).div_ceil(SLICE).max(1);
    Duration::from_secs(200 * slices + 100)
}

/// ⚠️ **Le seul chemin qui efface** (`tests/contrats.rs` le vérifie). Relit tout juste avant d'envoyer
/// l'effacement, puis attend que la carte dise la mémoire prête et vide. `waiting(écoulé)` est appelé à chaque
/// relecture, pour l'écran.
pub fn erase_flash<L: Link>(
    msp: &mut Msp<L>,
    permit: ErasePermit,
    polling: ErasePolling,
    mut waiting: impl FnMut(Duration),
) -> Result<(), EraseError> {
    // Même carte. Un identifiant nul ne prouve rien : il faut alors la même connexion que la copie.
    let uid = msp.uid()?;
    let same = if permit.uid.is_known() {
        uid == permit.uid
    } else {
        !uid.is_known() && msp.session() == permit.session
    };
    if !same {
        return Err(EraseError::OtherBoard);
    }
    // Rien d'enregistré depuis la copie : sinon on effacerait un vol jamais copié.
    let summary = msp.flash_summary()?;
    if !summary.supported || !summary.ready {
        return Err(EraseError::NotReady);
    }
    if summary.used != permit.used {
        return Err(EraseError::Changed {
            copied: permit.used,
            now: summary.used,
        });
    }
    // ⚠️ L'état armé en dernier, au plus près de l'envoi : le firmware efface même armé.
    if msp.armed()? {
        return Err(EraseError::Armed);
    }
    msp.request(Command::DataflashErase, &[])?;
    // La réponse arrive avant la fin : attendre « prête » et « vide ».
    let limit = polling.limit.unwrap_or_else(|| erase_limit(summary.total));
    let start = Instant::now();
    loop {
        std::thread::sleep(polling.interval);
        let waited = start.elapsed();
        waiting(waited);
        // Une relecture perdue n'arrête pas l'attente : l'effacement continue sur la carte.
        if let Ok(s) = msp.flash_summary()
            && s.ready
            && s.used == 0
        {
            return Ok(());
        }
        if waited >= limit {
            return Err(EraseError::Timeout { waited });
        }
    }
}
