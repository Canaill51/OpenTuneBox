//! Trames MSP, telles qu'elles passent sur le fil (`docs/spec-usb.md`, lu dans `src/main/msp/msp_serial.c` de Betaflight).
//!
//! - **v1** : `$M<` (demande), `$M>` (réponse) ou `$M!` (erreur), taille (u8), commande (u8), données, puis un octet de
//!   contrôle : le OU exclusif de la taille, de la commande et des données.
//! - **v1 « jumbo »** : quand les données font **255 octets ou plus**, la taille vaut 255 et la vraie taille suit sur
//!   16 bits (petit-boutiste), avant les données ; elle entre dans le OU exclusif. C'est la forme des blocs de mémoire
//!   (jusqu'à 4 096 octets).
//! - **v2** : `$X<`, `$X>` ou `$X!`, drapeau (u8), commande (u16), taille (u16), données, puis un CRC-8 DVB-S2 calculé du
//!   drapeau à la fin des données.
//!
//! Le firmware répond dans la version de la demande. La forme « v2 dans v1 » (commande 255) n'est jamais demandée par
//! OpenTuneBox ; une telle réponse serait une trame v1 ordinaire de commande 255, que le client refuse.

/// Version du protocole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    V1,
    V2,
}

/// Sens d'une trame : `<`, `>` ou `!`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Request,
    Reply,
    /// Le firmware ne connaît pas la commande, ou l'a refusée.
    Error,
}

impl Direction {
    fn byte(self) -> u8 {
        match self {
            Direction::Request => b'<',
            Direction::Reply => b'>',
            Direction::Error => b'!',
        }
    }

    fn from_byte(b: u8) -> Option<Self> {
        match b {
            b'<' => Some(Direction::Request),
            b'>' => Some(Direction::Reply),
            b'!' => Some(Direction::Error),
            _ => None,
        }
    }
}

/// Une trame complète, contrôle vérifié.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub version: Version,
    pub direction: Direction,
    pub cmd: u16,
    pub payload: Vec<u8>,
}

/// Taille à partir de laquelle une trame v1 passe en « jumbo » (`JUMBO_FRAME_SIZE_LIMIT` du firmware).
pub const JUMBO_LIMIT: usize = 255;

/// Plus grosses données acceptées en lecture. Le firmware n'envoie jamais plus de 4 096 octets
/// (`MSP_PORT_OUTBUF_SIZE`) ; au-delà, c'est du bruit pris pour un début de trame : ne pas réserver 64 Ko pour lui.
pub const MAX_PAYLOAD: usize = 8 * 1024;

/// CRC-8 DVB-S2 (polynôme 0xD5), celui des trames v2.
pub fn crc8_dvb_s2(mut crc: u8, data: &[u8]) -> u8 {
    for &b in data {
        crc ^= b;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ 0xD5 } else { crc << 1 };
        }
    }
    crc
}

fn xor(data: &[u8]) -> u8 {
    data.iter().fold(0, |a, &b| a ^ b)
}

/// Écrit une trame. En v1, la commande doit tenir sur un octet (le client n'en envoie pas d'autre) ; les données de
/// 255 octets ou plus passent en jumbo — le firmware n'en accepte pas en entrée, seul le port simulé s'en sert pour
/// répondre.
pub fn encode(version: Version, direction: Direction, cmd: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 12);
    match version {
        Version::V1 => {
            assert!(cmd <= 0xFF, "commande {cmd} impossible en MSP v1");
            out.extend_from_slice(&[b'$', b'M', direction.byte()]);
            let start = out.len();
            if payload.len() >= JUMBO_LIMIT {
                let n = u16::try_from(payload.len()).expect("trame de plus de 65 535 octets");
                out.extend_from_slice(&[JUMBO_LIMIT as u8, cmd as u8]);
                out.extend_from_slice(&n.to_le_bytes());
            } else {
                out.extend_from_slice(&[payload.len() as u8, cmd as u8]);
            }
            out.extend_from_slice(payload);
            let c = xor(&out[start..]);
            out.push(c);
        }
        Version::V2 => {
            let n = u16::try_from(payload.len()).expect("trame de plus de 65 535 octets");
            out.extend_from_slice(&[b'$', b'X', direction.byte()]);
            let start = out.len();
            out.push(0); // drapeau : rien de demandé
            out.extend_from_slice(&cmd.to_le_bytes());
            out.extend_from_slice(&n.to_le_bytes());
            out.extend_from_slice(payload);
            let c = crc8_dvb_s2(0, &out[start..]);
            out.push(c);
        }
    }
    out
}

/// Ce que le lecteur a rejeté.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// Contrôle faux : un octet a changé en route.
    Checksum { cmd: u16 },
    /// Taille annoncée au-delà de [`MAX_PAYLOAD`].
    TooLarge { size: usize },
}

#[derive(Clone, Copy, Debug)]
enum State {
    Idle,
    Magic,        // reçu `$`
    Dir(Version), // reçu `$M` ou `$X`
    Head(Version, Direction),
    Payload,
    Check,
}

/// Lecteur de trames octet par octet. Les octets hors trame (bruit, texte du CLI) sont comptés et ignorés.
#[derive(Debug)]
pub struct Parser {
    state: State,
    version: Version,
    direction: Direction,
    head: Vec<u8>,
    cmd: u16,
    size: usize,
    payload: Vec<u8>,
    /// Octets ignorés hors trame depuis la création du lecteur.
    pub skipped: u64,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            state: State::Idle,
            version: Version::V1,
            direction: Direction::Reply,
            head: Vec::with_capacity(5),
            cmd: 0,
            size: 0,
            payload: Vec::new(),
            skipped: 0,
        }
    }

    /// Aucune trame commencée : c'est là qu'un `#` fait entrer le firmware dans son CLI.
    pub fn is_idle(&self) -> bool {
        matches!(self.state, State::Idle)
    }

    /// Oublie une trame commencée (après un délai dépassé, par exemple).
    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.head.clear();
        self.payload.clear();
    }

    /// Avale un octet ; rend une trame quand elle est complète.
    pub fn push(&mut self, b: u8) -> Option<Result<Frame, FrameError>> {
        match self.state {
            State::Idle => {
                if b == b'$' {
                    self.state = State::Magic;
                } else {
                    self.skipped += 1;
                }
            }
            State::Magic => {
                self.state = match b {
                    b'M' => State::Dir(Version::V1),
                    b'X' => State::Dir(Version::V2),
                    b'$' => State::Magic,
                    _ => {
                        self.skipped += 2;
                        State::Idle
                    }
                };
            }
            State::Dir(v) => match Direction::from_byte(b) {
                Some(d) => {
                    self.head.clear();
                    self.state = State::Head(v, d);
                }
                None => {
                    self.skipped += 3;
                    self.state = if b == b'$' { State::Magic } else { State::Idle };
                }
            },
            State::Head(v, d) => {
                self.head.push(b);
                let h = &self.head;
                let done = match v {
                    // taille, commande, et 2 octets de plus si la taille vaut 255
                    Version::V1 => h.len() == 2 && h[0] as usize != JUMBO_LIMIT || h.len() == 4,
                    // drapeau, commande (2), taille (2)
                    Version::V2 => h.len() == 5,
                };
                if done {
                    let (cmd, size) = match v {
                        Version::V1 if h.len() == 4 => (u16::from(h[1]), usize::from(u16::from_le_bytes([h[2], h[3]]))),
                        Version::V1 => (u16::from(h[1]), usize::from(h[0])),
                        Version::V2 => (
                            u16::from_le_bytes([h[1], h[2]]),
                            usize::from(u16::from_le_bytes([h[3], h[4]])),
                        ),
                    };
                    if size > MAX_PAYLOAD {
                        self.reset();
                        return Some(Err(FrameError::TooLarge { size }));
                    }
                    self.version = v;
                    self.direction = d;
                    self.cmd = cmd;
                    self.size = size;
                    self.payload.clear();
                    self.payload.reserve(size);
                    self.state = if size == 0 { State::Check } else { State::Payload };
                }
            }
            State::Payload => {
                self.payload.push(b);
                if self.payload.len() == self.size {
                    self.state = State::Check;
                }
            }
            State::Check => {
                let expected = match self.version {
                    Version::V1 => xor(&self.head) ^ xor(&self.payload),
                    Version::V2 => crc8_dvb_s2(crc8_dvb_s2(0, &self.head), &self.payload),
                };
                let cmd = self.cmd;
                self.state = State::Idle;
                self.head.clear();
                if b != expected {
                    self.payload.clear();
                    return Some(Err(FrameError::Checksum { cmd }));
                }
                return Some(Ok(Frame {
                    version: self.version,
                    direction: self.direction,
                    cmd,
                    payload: std::mem::take(&mut self.payload),
                }));
            }
        }
        None
    }
}
