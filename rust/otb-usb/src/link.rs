//! Le fil : un vrai port série (`serial2`, D-063) ou le port simulé des tests ([`crate::sim`]).

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Ce dont le client a besoin d'un port : envoyer, et recevoir ce qui est arrivé dans un délai.
pub trait Link {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()>;
    /// Lit ce qui est arrivé, en attendant au plus `timeout`. `Ok(0)` : rien dans le délai.
    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize>;
}

/// Un port série ouvert, réservé à OpenTuneBox.
#[derive(Debug)]
pub struct SerialLink {
    port: serial2::SerialPort,
    path: PathBuf,
}

/// Les ports série présents sur la machine. `serial2` ne donne pas l'identifiant USB du fabricant : une carte se
/// reconnaît en lui demandant `MSP_API_VERSION` (`docs/spec-usb.md` §6).
pub fn list_ports() -> io::Result<Vec<PathBuf>> {
    serial2::SerialPort::available_ports()
}

/// Les ports où une carte de vol peut se trouver. Sous Linux, `serial2` liste aussi les dizaines de ports série
/// historiques (`/dev/ttyS*`) qui n'ont souvent aucun matériel derrière : une carte branchée en USB apparaît en
/// `/dev/ttyACM*` (port virtuel) ou `/dev/ttyUSB*` (adaptateur). Ailleurs, rien n'est filtré. Fonction pure.
pub fn board_candidates(ports: Vec<PathBuf>, linux: bool) -> Vec<PathBuf> {
    if !linux {
        return ports;
    }
    ports
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("ttyACM") || n.starts_with("ttyUSB"))
        })
        .collect()
}

/// Pourquoi un port n'a pas pu s'ouvrir, avec ce que le pilote peut y faire.
#[derive(Debug)]
pub enum PortError {
    /// Port absent : carte débranchée, ou (Windows) pilote du port virtuel manquant.
    NotFound(PathBuf),
    /// Linux : l'utilisateur n'est pas dans le groupe qui a le droit d'ouvrir le port.
    /// Windows : le port est déjà ouvert par un autre programme (Windows le dit « accès refusé »).
    Denied(PathBuf),
    /// Un autre programme (configurateur Betaflight, autre OpenTuneBox) tient déjà le port.
    Busy(PathBuf),
    Other(PathBuf, io::Error),
}

impl std::fmt::Display for PortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortError::NotFound(p) => write!(
                f,
                "Le port {} n'existe pas : la carte est peut-être débranchée{}.",
                p.display(),
                if cfg!(windows) {
                    ", ou le pilote du port USB virtuel n'est pas installé (Betaflight le fournit)"
                } else {
                    ""
                }
            ),
            PortError::Denied(p) if cfg!(windows) => write!(
                f,
                "Le port {} est déjà ouvert par un autre programme : fermer le configurateur Betaflight, puis réessayer.",
                p.display()
            ),
            PortError::Denied(p) => write!(
                f,
                "Droits insuffisants pour ouvrir {}. Sous Linux, ajouter votre compte au groupe « dialout » \
                 (sudo usermod -aG dialout $USER), puis fermer la session et la rouvrir.",
                p.display()
            ),
            PortError::Busy(p) => write!(
                f,
                "Le port {} est déjà utilisé, sans doute par le configurateur Betaflight ou un autre OpenTuneBox : \
                 le fermer, puis réessayer.",
                p.display()
            ),
            PortError::Other(p, e) => write!(f, "Impossible d'ouvrir {} : {e}.", p.display()),
        }
    }
}

impl std::error::Error for PortError {}

/// Classe une erreur d'ouverture. Fonction pure : testée sans port (L-071).
pub fn classify_open_error(path: &Path, e: io::Error) -> PortError {
    let path = path.to_path_buf();
    match e.kind() {
        io::ErrorKind::NotFound => PortError::NotFound(path),
        io::ErrorKind::PermissionDenied => PortError::Denied(path),
        io::ErrorKind::ResourceBusy | io::ErrorKind::WouldBlock => PortError::Busy(path),
        _ => PortError::Other(path, e),
    }
}

impl SerialLink {
    /// Ouvre le port et le réserve. Le débit est sans effet sur un port USB virtuel ; 115 200 est celui du configurateur.
    pub fn open(path: &Path) -> Result<Self, PortError> {
        let port = serial2::SerialPort::open(path, 115_200).map_err(|e| classify_open_error(path, e))?;
        let port = reserve(port).map_err(|e| classify_open_error(path, e))?;
        // Ce qui traînait dans le tampon vient d'avant nous.
        let _ = port.discard_buffers();
        Ok(SerialLink {
            port,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// ⚠️ Sous Linux, deux programmes peuvent ouvrir le même port série et se voler les réponses : verrou exclusif
/// (`flock`), sans code `unsafe`. Un second OpenTuneBox est refusé ; un programme qui ne prend pas ce verrou n'est
/// pas arrêté par lui (procédure d'essai de `docs/spec-usb.md`). Sous Windows, un port COM n'a qu'un seul
/// propriétaire : rien à faire.
#[cfg(unix)]
fn reserve(port: serial2::SerialPort) -> io::Result<serial2::SerialPort> {
    use std::os::fd::OwnedFd;
    let file = std::fs::File::from(OwnedFd::from(port));
    match file.try_lock() {
        Ok(()) => Ok(serial2::SerialPort::from(OwnedFd::from(file))),
        Err(std::fs::TryLockError::WouldBlock) => Err(io::Error::from(io::ErrorKind::ResourceBusy)),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

#[cfg(not(unix))]
fn reserve(port: serial2::SerialPort) -> io::Result<serial2::SerialPort> {
    Ok(port)
}

impl Link for SerialLink {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.port.write_all(bytes)?;
        self.port.flush()
    }

    fn recv(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        self.port.set_read_timeout(timeout)?;
        match self.port.read(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == io::ErrorKind::TimedOut || e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(e),
        }
    }
}
