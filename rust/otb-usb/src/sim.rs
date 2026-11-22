//! Une carte Betaflight simulée, pour les tests : **aucun matériel en CI** (lot 32). Elle répond aux commandes du
//! client comme le firmware (`docs/spec-usb.md`) et se laisse abîmer à la demande : réponse perdue ou altérée, vol
//! enregistré pendant la copie, drone armé, carte remplacée.
//!
//! Elle répond aussitôt, sans attendre : un délai dépassé se simule par une réponse perdue.

use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use crate::client::Command;
use crate::frame::{self, Direction, Parser};
use crate::link::Link;

/// Place réservée par le firmware dans un bloc lu (`MSP_PORT_DATAFLASH_INFO_SIZE`) : il rend au plus 4 096 − 16.
const DATAFLASH_INFO: usize = 16;

/// Une panne sur une réponse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Réponse perdue.
    Drop,
    /// Un octet altéré en route : contrôle faux.
    Corrupt,
    /// Contrôle juste, mais des données manquent par rapport à la longueur annoncée.
    ShortData,
    /// Contrôle juste, mais la lecture rendue est celle d'une autre adresse (réponse en retard d'une autre demande).
    WrongAddress,
}

#[derive(Debug)]
pub struct SimBoard {
    /// Contenu de la mémoire : sa longueur est la place occupée.
    pub flash: Vec<u8>,
    /// Taille de la mémoire.
    pub total: u32,
    /// `false` : pas de mémoire interne (logs sur carte SD).
    pub has_flash: bool,
    pub chip_id: [u32; 3],
    pub variant: [u8; 4],
    pub version: (u8, u8, u8),
    pub board: [u8; 4],
    pub target: String,
    pub name: String,
    pub armed: bool,
    /// Relectures du résumé encore « pas prête » après un effacement.
    pub erase_polls: u32,
    /// Plus gros bloc rendu (tampon du firmware).
    pub max_block: usize,
    /// Pannes à appliquer, par numéro de réponse (à partir de 0).
    pub faults: Vec<(usize, Fault)>,
    /// L'effacement est accepté mais ne fait rien (la mémoire reste pleine).
    pub erase_ignored: bool,
    /// Texte rendu par `diff all`, lignes en `\n`.
    pub diff: String,
    /// Commandes MSP reçues, dans l'ordre.
    pub received: Vec<u16>,
    /// Après une lecture de la mémoire à cette adresse, ajoute ces octets (vol enregistré pendant la copie).
    pub record_during_read: Option<(u32, Vec<u8>)>,
    erasing: u32,
    replies: usize,
    parser: Parser,
    out: VecDeque<u8>,
    cli: Option<Vec<u8>>,
    /// Octets rendus au plus par lecture du port (un vrai port livre par morceaux).
    pub recv_chunk: usize,
    /// Le port a été fermé par un redémarrage.
    pub rebooted: bool,
}

impl SimBoard {
    /// Une carte Betaflight 2025.12 dont la mémoire de 16 Mo contient `flash`.
    pub fn new(flash: Vec<u8>) -> Self {
        SimBoard {
            flash,
            total: 16 * 1024 * 1024,
            has_flash: true,
            chip_id: [0x0033_0021, 0x3438_5107, 0x3436_3432],
            variant: *b"BTFL",
            version: (25, 12, 0),
            board: *b"S7X2",
            target: "STM32F7X2".into(),
            name: "jeno5".into(),
            armed: false,
            erase_polls: 3,
            max_block: 4096 - DATAFLASH_INFO,
            faults: Vec::new(),
            erase_ignored: false,
            diff: "# version\n# Betaflight / STM32F7X2 (S7X2) 2025.12.0\n\n# master\nset gyro_lpf1_static_hz = 0\n"
                .into(),
            received: Vec::new(),
            record_during_read: None,
            erasing: 0,
            replies: 0,
            parser: Parser::new(),
            out: VecDeque::new(),
            cli: None,
            recv_chunk: usize::MAX,
            rebooted: false,
        }
    }

    /// Nombre de fois qu'une commande a été reçue.
    pub fn count(&self, cmd: Command) -> usize {
        self.received.iter().filter(|&&c| c == cmd.code()).count()
    }

    fn reply(&mut self, version: frame::Version, cmd: u16, payload: Option<Vec<u8>>) {
        let n = self.replies;
        self.replies += 1;
        let fault = self.faults.iter().find(|(k, _)| *k == n).map(|(_, f)| *f);
        let (dir, mut payload) = match payload {
            Some(p) => (Direction::Reply, p),
            None => (Direction::Error, Vec::new()),
        };
        match fault {
            Some(Fault::Drop) => return,
            // Une réponse de lecture dont il manque un octet au milieu, longueur annoncée inchangée.
            Some(Fault::ShortData) if payload.len() > 8 => {
                payload.remove(8);
            }
            _ => {}
        }
        let mut bytes = frame::encode(version, dir, cmd, &payload);
        if fault == Some(Fault::Corrupt) {
            let k = bytes.len() - 2; // dernier octet de données (ou d'en-tête), pas le contrôle
            bytes[k] ^= 0x5A;
        }
        self.out.extend(bytes);
    }

    fn answer(&mut self, cmd: u16, req: &[u8]) -> Option<Vec<u8>> {
        let mut p = Vec::new();
        match Command::from_code(cmd)? {
            Command::ApiVersion => p.extend_from_slice(&[0, 1, 47]),
            Command::FcVariant => p.extend_from_slice(&self.variant),
            Command::FcVersion => p.extend_from_slice(&[self.version.0, self.version.1, self.version.2]),
            Command::BoardInfo => {
                p.extend_from_slice(&self.board);
                p.extend_from_slice(&[0, 0, 0, 1]);
                p.push(self.target.len() as u8);
                p.extend_from_slice(self.target.as_bytes());
                p.extend_from_slice(&[0, 0]);
            }
            Command::Name => p.extend_from_slice(self.name.as_bytes()),
            Command::ChipId => self.chip_id.iter().for_each(|w| p.extend_from_slice(&w.to_le_bytes())),
            Command::StatusEx => {
                p.extend_from_slice(&[0; 6]);
                p.extend_from_slice(&u32::from(self.armed).to_le_bytes());
                p.extend_from_slice(&[0; 6]);
            }
            Command::DataflashSummary => {
                if !self.has_flash {
                    p.extend_from_slice(&[0; 13]);
                } else {
                    let ready = if self.erasing > 0 {
                        self.erasing -= 1;
                        false
                    } else {
                        true
                    };
                    p.push(2 | u8::from(ready));
                    p.extend_from_slice(&(self.total / 4096).to_le_bytes());
                    p.extend_from_slice(&self.total.to_le_bytes());
                    p.extend_from_slice(&(self.flash.len() as u32).to_le_bytes());
                }
            }
            Command::DataflashRead => {
                if req.len() < 6 {
                    return None; // ancien format : le client ne doit jamais l'employer
                }
                let addr = u32::from_le_bytes([req[0], req[1], req[2], req[3]]);
                let want = usize::from(u16::from_le_bytes([req[4], req[5]])).min(self.max_block);
                // Comme le firmware : on lit jusqu'à la taille de la mémoire, pas seulement la place occupée.
                let a = addr as usize;
                let end = (a + want).min(self.total as usize);
                let mut data: Vec<u8> = (a..end).map(|i| self.flash.get(i).copied().unwrap_or(0xFF)).collect();
                if a >= self.total as usize {
                    data.clear();
                }
                p.extend_from_slice(&addr.to_le_bytes());
                p.extend_from_slice(&(data.len() as u16).to_le_bytes());
                p.push(0);
                p.extend_from_slice(&data);
                if let Some((_, extra)) = self.record_during_read.take_if(|(at, _)| *at == addr) {
                    self.flash.extend_from_slice(&extra);
                }
            }
            Command::DataflashErase => {
                if !self.erase_ignored {
                    self.flash.clear();
                }
                self.erasing = self.erase_polls;
            }
        }
        Some(p)
    }

    fn cli_byte(&mut self, b: u8) {
        let line = self.cli.as_mut().expect("mode CLI");
        if b == b'\n' || b == b'\r' {
            let cmd = String::from_utf8_lossy(line).trim().to_string();
            line.clear();
            if cmd.is_empty() {
                return;
            }
            self.out.extend(b"\r\n");
            match cmd.as_str() {
                "diff all" => {
                    let text = format!("\r\n{}", self.diff.replace('\n', "\r\n"));
                    self.out.extend(text.as_bytes());
                }
                "exit" => {
                    self.out
                        .extend(b"\r\n# leaving CLI mode, unsaved changes lost\r\nRebooting");
                    self.rebooted = true;
                    return;
                }
                _ => self.out.extend(b"###ERROR: UNKNOWN COMMAND, TRY 'HELP'###"),
            }
            self.out.extend(b"\r\n# ");
        } else if (32..=126).contains(&b) {
            line.push(b);
            self.out.push_back(b); // écho
        }
    }
}

impl Link for SimBoard {
    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.rebooted {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        for &b in bytes {
            if self.cli.is_some() {
                self.cli_byte(b);
                continue;
            }
            if b == b'#' && self.parser.is_idle() {
                self.cli = Some(Vec::new());
                self.out
                    .extend(b"\r\nEntering CLI Mode, type 'exit' to reboot, or 'help'\r\n\r\n# ".iter());
                continue;
            }
            if let Some(Ok(f)) = self.parser.push(b)
                && f.direction == Direction::Request
            {
                self.received.push(f.cmd);
                let mut req = f.payload;
                let n = self.replies;
                if self.faults.contains(&(n, Fault::WrongAddress)) && req.len() >= 4 {
                    req[1] = req[1].wrapping_add(1); // 256 octets plus loin
                }
                let p = self.answer(f.cmd, &req);
                self.reply(f.version, f.cmd, p);
            }
        }
        Ok(())
    }

    fn recv(&mut self, buf: &mut [u8], _timeout: Duration) -> io::Result<usize> {
        if self.rebooted && self.out.is_empty() {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        let n = buf.len().min(self.out.len()).min(self.recv_chunk);
        for (slot, b) in buf.iter_mut().zip(self.out.drain(..n)) {
            *slot = b;
        }
        Ok(n)
    }
}
