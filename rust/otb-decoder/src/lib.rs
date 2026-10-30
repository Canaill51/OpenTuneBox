//! Décodeur des trames Blackbox : machine à états fidèle à `decoder/core.py` (D-031).
//!
//! - une trame n'est validée que lorsque l'octet qui la suit ressemble au début d'une nouvelle trame
//!   (ou à la fin du fichier) et qu'elle fait au plus 256 octets ; sinon on reprend la lecture un octet
//!   après son début (resynchronisation) ;
//! - une trame I valide resynchronise le flux principal ; les trames P ne sont acceptées que si le flux
//!   est valide et que le temps / l'itération n'ont pas sauté trop loin.

mod frame;
pub mod header;
mod reader;

pub use frame::*;
use reader::Stream;

const MAX_FRAME_LENGTH: i64 = 256;
const MAX_TIME_JUMP: i64 = 10 * 1_000_000;
const MAX_ITERATION_JUMP: i64 = 500 * 10;
const FIELD_ITERATION: usize = 0;
const FIELD_TIME: usize = 1;

/// Paramètres d'une session (ce que `reader.py:_decode_session` tire de l'en-tête).
pub struct Params<'a> {
    pub start: i64,
    pub end: i64,
    pub i_def: FrameDef<'a>,
    pub p_def: FrameDef<'a>,
    pub s_def: FrameDef<'a>,
    pub g_def: FrameDef<'a>,
    pub h_def: FrameDef<'a>,
    pub has_p: bool,
    pub has_s: bool,
    pub has_g: bool,
    pub has_h: bool,
    pub i_interval: i64,
    pub p_num: i64,
    pub p_denom: i64,
    pub ctx: [i64; CTX_SIZE],
    /// Nombre de lignes à réserver d'avance (simple indication de capacité).
    pub est_rows: usize,
}

/// Table à lignes de largeur fixe, en une seule mémoire (ligne après ligne).
#[derive(Default)]
pub struct Table {
    pub cols: usize,
    pub data: Vec<i64>,
}

impl Table {
    fn new(cols: usize, rows: usize) -> Self {
        // réserve d'avance « au mieux » : si la mémoire manque, on démarre vide au lieu d'abandonner le programme
        let mut data = Vec::new();
        let _ = data.try_reserve(rows.saturating_mul(cols));
        Table { cols, data }
    }
    fn push(&mut self, row: &[i64]) {
        self.data.extend_from_slice(&row[..self.cols]);
    }
    pub fn rows(&self) -> usize {
        self.data.len().checked_div(self.cols).unwrap_or(0)
    }
}

/// Résultat : mêmes tableaux que `core.decode_stream` (largeur au moins 1, comme en Python).
pub struct Decoded {
    pub main: Table,
    pub kind: Vec<i8>,
    pub slow: Table,
    pub slow_at: Vec<i64>,
    pub gps: Table,
    pub gps_at: Vec<i64>,
    /// (type, ligne principale, donnée 1, donnée 2) par événement
    pub events: Vec<[i64; 4]>,
    /// trames corrompues, désynchros, itérations sautées volontairement, fin de log vue, octets lus,
    /// trames I, trames P, inutilisé
    pub stats: [i64; 8],
}

fn should_have_frame(idx: i64, i_interval: i64, p_num: i64, p_denom: i64) -> bool {
    // `%` de Python : le résultat suit le signe du diviseur (positif)
    (idx.rem_euclid(i_interval) + p_num - 1).rem_euclid(p_denom) < p_num
}

fn is_marker(c: i64) -> bool {
    // I P G H S E
    matches!(c, 73 | 80 | 71 | 72 | 83 | 69)
}

pub fn decode_stream(buf: &[u8], prm: &Params) -> Decoded {
    let mut s = Stream::new(buf, prm.start, prm.end);
    let mut ctx = prm.ctx;
    let n_main = prm.i_def.n;
    let nm = n_main.max(1);
    let (n_slow, n_gps, n_home) = (prm.s_def.n.max(1), prm.g_def.n.max(1), prm.h_def.n.max(1));

    // l'estimation d'après la taille du fichier est environ deux fois trop grande (W-016) : on en réserve la moitié
    let rows_hint = (prm.est_rows / 2).max(16);
    let mut main = Table::new(nm, rows_hint);
    let mut kind: Vec<i8> = Vec::new();
    let _ = kind.try_reserve(rows_hint);
    let mut slow = Table::new(n_slow, 64);
    let mut slow_at: Vec<i64> = Vec::new();
    let mut gps = Table::new(n_gps, 64);
    let mut gps_at: Vec<i64> = Vec::new();
    let mut events: Vec<[i64; 4]> = Vec::new();
    let mut stats = [0i64; 8];

    // au moins 2 cases : l'itération et le temps se lisent toujours (un en-tête à un seul champ ne doit pas paniquer)
    let nb = n_main.max(2);
    let mut cur = vec![0i64; nb];
    let mut prev = vec![0i64; nb];
    let mut prev2 = vec![0i64; nb];
    let mut has_prev = false;
    let mut last_slow = vec![0i64; n_slow];
    let mut last_gps = vec![0i64; n_gps];
    let mut home0 = vec![0i64; n_home];
    let mut home1 = vec![0i64; n_home];
    let mut home_valid = false;
    let dummy = vec![0i64; n_home];
    let mut values = [0i64; 8];

    let mut stream_valid = false;
    let mut last_iter: i64 = -1;
    let mut last_time: i64 = -1;
    let mut last_skipped: i64 = 0;
    let mut last_type: i64 = -1;
    let mut frame_start = prm.start;
    let mut premature_eof = false;

    // événement en cours
    let (mut ev_ok, mut ev_type, mut ev_d1, mut ev_d2) = (false, 0i64, 0i64, 0i64);

    loop {
        let cmd = s.byte();

        if last_type != -1 {
            let size = s.pos - frame_start;
            let looks = is_marker(cmd) || (!premature_eof && cmd == -1);
            if size <= MAX_FRAME_LENGTH && looks {
                let mut accepted = true;
                match last_type {
                    73 => {
                        // I
                        let mut accept = true;
                        if last_iter != -1 {
                            let (it, tm) = (cur[FIELD_ITERATION], cur[FIELD_TIME]);
                            accept = it >= last_iter
                                && it < last_iter.wrapping_add(MAX_ITERATION_JUMP)
                                && tm >= last_time
                                && tm < last_time.wrapping_add(MAX_TIME_JUMP);
                        }
                        if accept {
                            if last_iter != -1 {
                                // itérations volontairement absentes jusqu'à cette trame
                                let mut k = last_iter.wrapping_add(1);
                                while k < cur[FIELD_ITERATION] {
                                    if !should_have_frame(k, prm.i_interval, prm.p_num, prm.p_denom) {
                                        stats[2] += 1;
                                    }
                                    k = k.wrapping_add(1);
                                }
                            }
                            last_iter = cur[FIELD_ITERATION];
                            last_time = cur[FIELD_TIME];
                            stream_valid = true;
                        } else {
                            stream_valid = false;
                        }
                        if stream_valid {
                            main.push(&cur);
                            kind.push(0);
                            stats[5] += 1;
                        } else {
                            accepted = false;
                        }
                        prev.copy_from_slice(&cur);
                        prev2.copy_from_slice(&cur);
                        has_prev = true;
                    }
                    80 => {
                        // P
                        if stream_valid
                            && (cur[FIELD_TIME] > last_time.wrapping_add(MAX_TIME_JUMP)
                                || cur[FIELD_ITERATION] > last_iter.wrapping_add(MAX_ITERATION_JUMP))
                        {
                            stream_valid = false;
                        }
                        if stream_valid {
                            last_iter = cur[FIELD_ITERATION];
                            last_time = cur[FIELD_TIME];
                            stats[2] = stats[2].wrapping_add(last_skipped);
                            main.push(&cur);
                            kind.push(1);
                            stats[6] += 1;
                            prev2.copy_from_slice(&prev);
                            prev.copy_from_slice(&cur);
                        } else {
                            accepted = false;
                        }
                    }
                    83 => {
                        // S
                        slow.push(&last_slow);
                        slow_at.push(main.rows() as i64);
                    }
                    72 => {
                        // H
                        home1.copy_from_slice(&home0);
                        home_valid = true;
                    }
                    71 => {
                        // G
                        if home_valid {
                            gps.push(&last_gps);
                            gps_at.push(main.rows() as i64);
                        } else {
                            accepted = false;
                        }
                    }
                    69 => {
                        // E
                        if ev_ok {
                            if ev_type == EVENT_LOGGING_RESUME {
                                last_iter = ev_d1;
                                last_time = ev_d2;
                            }
                            events.push([ev_type, main.rows() as i64, ev_d1, ev_d2]);
                            if ev_type == EVENT_LOG_END {
                                stats[3] = 1;
                            }
                        } else {
                            accepted = false;
                        }
                    }
                    _ => {}
                }
                if !accepted {
                    stats[1] += 1;
                }
            } else {
                // trame précédente corrompue : on se resynchronise un octet plus loin
                stream_valid = false;
                stats[0] += 1;
                s.pos = frame_start + 1;
                s.eof = false;
                last_type = -1;
                premature_eof = false;
                continue;
            }
        }

        if cmd == -1 {
            break;
        }

        frame_start = s.pos - 1;
        let defined = cmd == 69
            || cmd == 73
            || (cmd == 80 && prm.has_p)
            || (cmd == 83 && prm.has_s)
            || (cmd == 71 && prm.has_g)
            || (cmd == 72 && prm.has_h);
        if is_marker(cmd) && defined {
            last_type = cmd;
            match cmd {
                73 => {
                    ctx[CTX_HAS_LAST_TIME] = has_prev as i64;
                    ctx[CTX_LAST_TIME] = prev[FIELD_TIME];
                    parse_frame(
                        &mut s,
                        &prm.i_def,
                        &mut cur,
                        &prev,
                        &prev2,
                        has_prev,
                        0,
                        &ctx,
                        &dummy,
                        &mut values,
                    );
                }
                80 => {
                    last_skipped = 0;
                    if last_iter != -1 {
                        let mut k = last_iter.wrapping_add(1);
                        while !should_have_frame(k, prm.i_interval, prm.p_num, prm.p_denom) {
                            last_skipped = last_skipped.wrapping_add(1);
                            k = k.wrapping_add(1);
                        }
                    }
                    ctx[CTX_HAS_LAST_TIME] = has_prev as i64;
                    ctx[CTX_LAST_TIME] = prev[FIELD_TIME];
                    parse_frame(
                        &mut s,
                        &prm.p_def,
                        &mut cur,
                        &prev,
                        &prev2,
                        has_prev,
                        last_skipped,
                        &ctx,
                        &dummy,
                        &mut values,
                    );
                }
                // S, H, G : la trame précédente n'existe pas (has_prev faux), `prev` ne sert pas
                83 => parse_frame(
                    &mut s,
                    &prm.s_def,
                    &mut last_slow,
                    &[],
                    &[],
                    false,
                    0,
                    &ctx,
                    &dummy,
                    &mut values,
                ),
                72 => parse_frame(
                    &mut s,
                    &prm.h_def,
                    &mut home0,
                    &[],
                    &[],
                    false,
                    0,
                    &ctx,
                    &dummy,
                    &mut values,
                ),
                71 => {
                    ctx[CTX_HAS_LAST_TIME] = has_prev as i64;
                    ctx[CTX_LAST_TIME] = prev[FIELD_TIME];
                    parse_frame(
                        &mut s,
                        &prm.g_def,
                        &mut last_gps,
                        &[],
                        &[],
                        false,
                        0,
                        &ctx,
                        &home1,
                        &mut values,
                    );
                }
                _ => (ev_ok, ev_type, ev_d1, ev_d2) = parse_event(&mut s),
            }
            if s.eof {
                premature_eof = true;
            }
        } else {
            stream_valid = false;
            last_type = -1;
        }
    }

    stats[4] = s.pos - prm.start;
    // copies à la taille exacte : la réserve d'avance resterait sinon en mémoire tant que la session existe
    main.data.shrink_to_fit();
    kind.shrink_to_fit();
    Decoded {
        main,
        kind,
        slow,
        slow_at,
        gps,
        gps_at,
        events,
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Session minimale : champs [itération, temps], trame I seule, puis fin de fichier.
    fn run(bytes: &[u8]) -> Decoded {
        let (pred, enc) = ([0i64, 0], [ENC_UNSIGNED_VB, ENC_UNSIGNED_VB]);
        let none = FrameDef {
            pred: &[0],
            enc: &[0],
            n: 0,
        };
        let prm = Params {
            start: 0,
            end: bytes.len() as i64,
            i_def: FrameDef {
                pred: &pred,
                enc: &enc,
                n: 2,
            },
            p_def: FrameDef {
                pred: &[0],
                enc: &[0],
                n: 0,
            },
            s_def: FrameDef {
                pred: &[0],
                enc: &[0],
                n: 0,
            },
            g_def: FrameDef {
                pred: &[0],
                enc: &[0],
                n: 0,
            },
            h_def: none,
            has_p: false,
            has_s: false,
            has_g: false,
            has_h: false,
            i_interval: 32,
            p_num: 1,
            p_denom: 1,
            ctx: [0; CTX_SIZE],
            est_rows: 0,
        };
        decode_stream(bytes, &prm)
    }

    #[test]
    fn single_i_frame() {
        let d = run(&[b'I', 5, 10]);
        assert_eq!((d.main.rows(), d.main.cols), (1, 2));
        assert_eq!(d.main.data, [5, 10]);
        assert_eq!(d.kind, [0]);
        assert_eq!((d.stats[0], d.stats[5], d.stats[4]), (0, 1, 3));
    }

    #[test]
    fn garbage_is_resynchronised() {
        // 0x00 ne commence aucune trame ; la trame I qui suit est lue, aucune trame corrompue comptée
        let d = run(&[0, 0, b'I', 5, 10]);
        assert_eq!(d.main.rows(), 1);
        assert_eq!(d.stats[0], 0);
    }

    #[test]
    fn oversized_frame_is_corrupt() {
        let mut bytes = vec![b'I'];
        bytes.extend(std::iter::repeat_n(0x80u8, 300));
        let d = run(&bytes);
        assert_eq!(d.main.rows(), 0);
        assert!(d.stats[0] > 0);
    }

    #[test]
    fn empty_input() {
        let d = run(&[]);
        assert_eq!((d.main.rows(), d.stats[4]), (0, 0));
    }
}
