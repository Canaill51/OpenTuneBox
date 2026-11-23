//! Le client MSP : identité de la carte, état armé, résumé et lecture de la mémoire, sauvegarde `diff all`.
//!
//! ⚠️ **Aucune commande d'écriture de réglage** (D-063) : [`Command`] est la liste fermée de ce que le client peut
//! envoyer, et `tests/contrats.rs` la fige. L'effacement y figure, mais un seul chemin l'envoie :
//! [`crate::erase::erase_flash`], qui exige un [`crate::erase::ErasePermit`].

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::frame::{self, Direction, Frame, FrameError, Parser, Version};
use crate::link::Link;

/// Les seules commandes MSP qu'OpenTuneBox envoie (numéros de `src/main/msp/msp_protocol.h`, `docs/spec-usb.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    ApiVersion,
    FcVariant,
    FcVersion,
    BoardInfo,
    Name,
    /// Envoyée par [`crate::msc::reboot_to_disk`] seulement, et seulement pour le mode stockage de masse (lot 32c).
    Reboot,
    DataflashSummary,
    DataflashRead,
    /// Envoyée par [`crate::erase::erase_flash`] seulement.
    DataflashErase,
    SdcardSummary,
    StatusEx,
    ChipId,
}

impl Command {
    pub const ALL: [Command; 12] = [
        Command::ApiVersion,
        Command::FcVariant,
        Command::FcVersion,
        Command::BoardInfo,
        Command::Name,
        Command::Reboot,
        Command::DataflashSummary,
        Command::DataflashRead,
        Command::DataflashErase,
        Command::SdcardSummary,
        Command::StatusEx,
        Command::ChipId,
    ];

    pub fn code(self) -> u16 {
        match self {
            Command::ApiVersion => 1,
            Command::FcVariant => 2,
            Command::FcVersion => 3,
            Command::BoardInfo => 4,
            Command::Name => 10,
            Command::Reboot => 68,
            Command::DataflashSummary => 70,
            Command::DataflashRead => 71,
            Command::DataflashErase => 72,
            Command::SdcardSummary => 79,
            Command::StatusEx => 150,
            Command::ChipId => 160,
        }
    }

    pub fn from_code(code: u16) -> Option<Command> {
        Command::ALL.into_iter().find(|c| c.code() == code)
    }
}

/// Plus gros bloc demandé à la fois : celui du configurateur (`BLOCK_SIZE`). Le firmware rend moins s'il manque de
/// place dans son tampon ou en fin de mémoire (`docs/spec-usb.md` §2).
pub const READ_BLOCK: u16 = 4096;

/// Délai d'une réponse ordinaire. Un bloc de 4 Ko passe en quelques millisecondes sur USB ; une seconde laisse de la
/// marge à une carte occupée.
const REPLY_TIMEOUT: Duration = Duration::from_secs(1);

/// Essais d'une même demande avant d'abandonner (contrôle faux, réponse perdue).
const ATTEMPTS: usize = 3;

/// Ce qui peut mal tourner en parlant à la carte.
#[derive(Debug)]
pub enum MspError {
    Io(std::io::Error),
    /// Pas de réponse dans le délai, après [`ATTEMPTS`] essais.
    Timeout {
        cmd: u16,
    },
    /// La carte a répondu `!` : commande inconnue ou refusée.
    Refused {
        cmd: u16,
    },
    /// Réponse trop courte ou incohérente.
    Malformed {
        cmd: u16,
        why: String,
    },
    /// Ce n'est pas Betaflight (D-062) : rien n'est lu ni effacé.
    NotBetaflight {
        variant: String,
    },
    /// Pas de mémoire interne (la carte enregistre sur carte SD, ou n'a pas de mémoire) : voie du stockage de masse.
    NoFlash,
    /// L'utilisateur a arrêté la copie.
    Cancelled,
}

impl std::fmt::Display for MspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MspError::Io(e) => write!(f, "Liaison avec la carte coupée : {e}."),
            MspError::Timeout { cmd } => write!(f, "La carte ne répond plus (commande MSP {cmd})."),
            MspError::Refused { cmd } => write!(f, "La carte a refusé la commande MSP {cmd}."),
            MspError::Malformed { cmd, why } => write!(f, "Réponse illisible à la commande MSP {cmd} : {why}."),
            MspError::NotBetaflight { variant } => write!(
                f,
                "Cette carte tourne sous « {variant} », pas sous Betaflight : OpenTuneBox ne lit que Betaflight."
            ),
            MspError::NoFlash => write!(
                f,
                "Cette carte n'a pas de mémoire de logs lisible en MSP (carte SD ?) : passer par le mode stockage de masse."
            ),
            MspError::Cancelled => write!(f, "Copie arrêtée : rien n'est gardé, la prochaine reprendra au début."),
        }
    }
}

impl std::error::Error for MspError {}

impl From<std::io::Error> for MspError {
    fn from(e: std::io::Error) -> Self {
        MspError::Io(e)
    }
}

/// Identifiant unique du microcontrôleur (`MSP_UID`, 96 bits gravés à la fabrication).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChipId(pub [u32; 3]);

impl ChipId {
    /// ⚠️ Nul sur le cœur M4 des H7 double cœur et sur le simulateur SITL : jamais une clé (`docs/spec-usb.md` §4).
    pub fn is_known(&self) -> bool {
        self.0 != [0; 3]
    }

    /// Forme texte, stable, pour la bibliothèque : 24 chiffres hexadécimaux.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|w| format!("{w:08x}")).collect()
    }
}

/// Ce que la carte dit d'elle au branchement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    /// Version de l'API MSP (`majeur.mineur`).
    pub api: (u8, u8),
    /// `BTFL` pour Betaflight.
    pub variant: String,
    /// Trois octets : `4.5.1` avant 2025, puis l'année depuis 2000, le mois et le correctif (`25.12.0`).
    pub version: (u8, u8, u8),
    /// Identifiant de carte sur 4 lettres (`S7X2`…).
    pub board: String,
    /// Nom de la cible (`STM32F7X2`…), vide sur un firmware qui ne le donne pas.
    pub target: String,
    /// `craft_name` : le pilote le change d'une commande, ce n'est pas une clé.
    pub name: String,
    pub chip_id: ChipId,
}

/// Résumé de la mémoire interne (`MSP_DATAFLASH_SUMMARY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlashSummary {
    /// Bit 1 : une mémoire existe.
    pub supported: bool,
    /// Bit 0 : prête (faux pendant un effacement).
    pub ready: bool,
    pub sectors: u32,
    pub total: u32,
    /// Place occupée : ce qu'une copie complète doit rapporter.
    pub used: u32,
}

/// Numéro de connexion : un permis d'effacement ne vaut que pour la connexion qui a copié, quand l'identifiant de la
/// carte est nul (voir [`crate::erase`]).
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Un numéro qu'aucune autre connexion ne porte (une copie lue sur le disque de la carte en reçoit un neuf : elle ne
/// vaut d'effacement que par l'identifiant de la carte).
pub(crate) fn new_session() -> u64 {
    NEXT_SESSION.fetch_add(1, Ordering::Relaxed)
}

/// Une conversation MSP sur un fil.
pub struct Msp<L: Link> {
    pub(crate) link: L,
    parser: Parser,
    version: Version,
    session: u64,
    timeout: Duration,
    /// Octets reçus et pas encore lus (la fin d'un envoi peut porter le début du suivant).
    pending: VecDeque<u8>,
}

fn le_u32(p: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([p[at], p[at + 1], p[at + 2], p[at + 3]])
}

/// Chaîne précédée de sa longueur (`sbufWritePString`).
fn pstring(p: &[u8], at: &mut usize) -> Option<String> {
    let n = usize::from(*p.get(*at)?);
    let s = p.get(*at + 1..*at + 1 + n)?;
    *at += 1 + n;
    Some(String::from_utf8_lossy(s).into_owned())
}

impl<L: Link> Msp<L> {
    /// MSP v1 par défaut : c'est la forme du configurateur, et tout ce qu'OpenTuneBox demande tient dans ses numéros.
    pub fn new(link: L) -> Self {
        Self::with_version(link, Version::V1)
    }

    pub fn with_version(link: L, version: Version) -> Self {
        Msp {
            link,
            parser: Parser::new(),
            version,
            session: new_session(),
            timeout: REPLY_TIMEOUT,
            pending: VecDeque::new(),
        }
    }

    /// Délai d'attente d'une réponse (une seconde par défaut).
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    pub fn link_mut(&mut self) -> &mut L {
        &mut self.link
    }

    pub(crate) fn session(&self) -> u64 {
        self.session
    }

    pub fn link(&self) -> &L {
        &self.link
    }

    pub fn into_link(self) -> L {
        self.link
    }

    /// Envoie une commande et attend **sa** réponse ; une réponse à une autre commande (restée d'une demande
    /// abandonnée) est jetée. Réessaie après un délai dépassé ou un contrôle faux.
    pub(crate) fn request(&mut self, cmd: Command, payload: &[u8]) -> Result<Vec<u8>, MspError> {
        let code = cmd.code();
        let mut last = MspError::Timeout { cmd: code };
        for _ in 0..ATTEMPTS {
            self.parser.reset();
            self.pending.clear();
            self.link
                .send(&frame::encode(self.version, Direction::Request, code, payload))?;
            match self.wait_reply(code) {
                Ok(f) if f.direction == Direction::Error => return Err(MspError::Refused { cmd: code }),
                Ok(f) => return Ok(f.payload),
                Err(e @ (MspError::Timeout { .. } | MspError::Malformed { .. })) => last = e,
                Err(e) => return Err(e),
            }
        }
        Err(last)
    }

    fn wait_reply(&mut self, code: u16) -> Result<Frame, MspError> {
        let deadline = Instant::now() + self.timeout;
        let mut buf = [0u8; 4096];
        loop {
            // Octets déjà reçus avec la trame précédente.
            while let Some(b) = self.pending.pop_front() {
                if let Some(r) = self.parser.push(b) {
                    match r {
                        Ok(f) if f.cmd == code && f.direction != Direction::Request => return Ok(f),
                        Ok(_) => {}
                        Err(FrameError::Checksum { .. }) | Err(FrameError::TooLarge { .. }) => {
                            return Err(MspError::Malformed {
                                cmd: code,
                                why: "contrôle de trame faux".into(),
                            });
                        }
                    }
                }
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(MspError::Timeout { cmd: code });
            }
            let n = self.link.recv(&mut buf, deadline - now)?;
            self.pending.extend(&buf[..n]);
        }
    }

    fn expect_len(cmd: Command, p: &[u8], n: usize) -> Result<(), MspError> {
        if p.len() < n {
            return Err(MspError::Malformed {
                cmd: cmd.code(),
                why: format!("{} octets reçus, {n} attendus", p.len()),
            });
        }
        Ok(())
    }

    /// Version de l'API MSP : la première question, comme le recommande le firmware. Sert aussi à reconnaître une
    /// carte parmi les ports série.
    pub fn api_version(&mut self) -> Result<(u8, u8), MspError> {
        let p = self.request(Command::ApiVersion, &[])?;
        Self::expect_len(Command::ApiVersion, &p, 3)?;
        Ok((p[1], p[2]))
    }

    /// Identité complète. ⚠️ S'arrête avant toute autre question si ce n'est pas Betaflight (D-062).
    pub fn identify(&mut self) -> Result<Identity, MspError> {
        let api = self.api_version()?;
        let p = self.request(Command::FcVariant, &[])?;
        Self::expect_len(Command::FcVariant, &p, 4)?;
        let variant = String::from_utf8_lossy(&p[..4]).into_owned();
        if variant != "BTFL" {
            return Err(MspError::NotBetaflight { variant });
        }
        let p = self.request(Command::FcVersion, &[])?;
        Self::expect_len(Command::FcVersion, &p, 3)?;
        let version = (p[0], p[1], p[2]);
        let p = self.request(Command::BoardInfo, &[])?;
        Self::expect_len(Command::BoardInfo, &p, 4)?;
        let board = String::from_utf8_lossy(&p[..4]).into_owned();
        // identifiant (4), révision (u16), type (u8), capacités (u8), puis le nom de la cible
        let mut at = 8;
        let target = pstring(&p, &mut at).unwrap_or_default();
        let name = String::from_utf8_lossy(&self.request(Command::Name, &[])?).into_owned();
        let chip_id = self.chip_id()?;
        Ok(Identity {
            api,
            variant,
            version,
            board,
            target,
            name,
            chip_id,
        })
    }

    pub fn chip_id(&mut self) -> Result<ChipId, MspError> {
        let p = self.request(Command::ChipId, &[])?;
        Self::expect_len(Command::ChipId, &p, 12)?;
        Ok(ChipId([le_u32(&p, 0), le_u32(&p, 4), le_u32(&p, 8)]))
    }

    /// Le drone est-il armé ? Bit 0 des 32 premiers bits de modes de `MSP_STATUS_EX` : le mode `ARM` est toujours le
    /// premier (`src/main/msp/msp_box.c:packFlightModeFlags`).
    pub fn armed(&mut self) -> Result<bool, MspError> {
        let p = self.request(Command::StatusEx, &[])?;
        Self::expect_len(Command::StatusEx, &p, 10)?;
        Ok(le_u32(&p, 6) & 1 != 0)
    }

    pub fn flash_summary(&mut self) -> Result<FlashSummary, MspError> {
        let p = self.request(Command::DataflashSummary, &[])?;
        Self::expect_len(Command::DataflashSummary, &p, 13)?;
        Ok(FlashSummary {
            supported: p[0] & 2 != 0,
            ready: p[0] & 1 != 0,
            sectors: le_u32(&p, 1),
            total: le_u32(&p, 5),
            used: le_u32(&p, 9),
        })
    }

    /// Lit un bloc : rend les octets lus, moins que demandé en fin de mémoire. ⚠️ Toujours avec la longueur : sans
    /// elle, le firmware passe à l'ancien format de 128 octets complétés de zéros (`docs/spec-usb.md` §2).
    pub fn read_block(&mut self, address: u32, len: u16) -> Result<Vec<u8>, MspError> {
        let cmd = Command::DataflashRead;
        let mut req = Vec::with_capacity(7);
        req.extend_from_slice(&address.to_le_bytes());
        req.extend_from_slice(&len.to_le_bytes());
        req.push(0); // compression refusée
        let mut last = None;
        for _ in 0..ATTEMPTS {
            let p = self.request(cmd, &req)?;
            Self::expect_len(cmd, &p, 7)?;
            let got_addr = le_u32(&p, 0);
            let n = usize::from(u16::from_le_bytes([p[4], p[5]]));
            let why = if got_addr != address {
                format!("adresse {got_addr} au lieu de {address}")
            } else if p[6] != 0 {
                format!(
                    "données compressées (méthode {}) alors que la compression était refusée",
                    p[6]
                )
            } else if p.len() != 7 + n {
                format!("{} octets de données pour {n} annoncés", p.len() - 7)
            } else if n > usize::from(len) {
                format!("{n} octets rendus pour {len} demandés")
            } else {
                return Ok(p[7..].to_vec());
            };
            last = Some(MspError::Malformed { cmd: cmd.code(), why });
        }
        Err(last.expect("au moins un essai"))
    }

    /// Copie toute la place occupée de la mémoire, du début à la fin, bloc par bloc. `progress(copiés, total)` est
    /// appelé après chaque bloc ; s'il rend `false`, la copie s'arrête et rien n'est gardé (une copie interrompue
    /// reprend au début, lot 32).
    pub fn copy_flash(
        &mut self,
        mut progress: impl FnMut(u64, u64) -> bool,
    ) -> Result<crate::erase::FlashCopy, MspError> {
        let identity = self.identify()?;
        let summary = self.flash_summary()?;
        if !summary.supported {
            return Err(MspError::NoFlash);
        }
        let total = summary.used;
        let mut bytes = Vec::with_capacity(total as usize);
        while (bytes.len() as u64) < u64::from(total) {
            let address = bytes.len() as u32;
            let want = (total - address).min(u32::from(READ_BLOCK)) as u16;
            let block = self.read_block(address, want)?;
            if block.is_empty() {
                // Fin de mémoire avant la place annoncée : le contrôle de taille refusera l'effacement.
                break;
            }
            bytes.extend_from_slice(&block);
            if !progress(bytes.len() as u64, u64::from(total)) {
                return Err(MspError::Cancelled);
            }
        }
        Ok(crate::erase::FlashCopy::new(identity, summary, bytes, self.session))
    }
}
