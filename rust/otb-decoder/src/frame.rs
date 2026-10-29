//! Prédicteurs, lecture d'une trame et d'un événement (miroir de `decoder/core.py`).
//!
//! Les numéros de prédicteur et d'encodage sont ceux de `decoder/header.py` ; `tests/test_native_decoder.py`
//! vérifie qu'ils sont identiques des deux côtés.

use crate::reader::*;

pub const ENC_SIGNED_VB: i64 = 0;
pub const ENC_UNSIGNED_VB: i64 = 1;
pub const ENC_NEG_14BIT: i64 = 3;
pub const ENC_TAG8_8SVB: i64 = 6;
pub const ENC_TAG2_3S32: i64 = 7;
pub const ENC_TAG8_4S16: i64 = 8;
pub const ENC_NULL: i64 = 9;
pub const ENC_TAG2_3SVARIABLE: i64 = 10;

pub const PRED_PREVIOUS: i64 = 1;
pub const PRED_STRAIGHT_LINE: i64 = 2;
pub const PRED_AVERAGE_2: i64 = 3;
pub const PRED_MINTHROTTLE: i64 = 4;
pub const PRED_MOTOR_0: i64 = 5;
pub const PRED_INC: i64 = 6;
pub const PRED_HOME_COORD: i64 = 7;
pub const PRED_1500: i64 = 8;
pub const PRED_VBATREF: i64 = 9;
pub const PRED_LAST_MAIN_FRAME_TIME: i64 = 10;
pub const PRED_MINMOTOR: i64 = 11;
pub const PRED_HOME_COORD_1: i64 = 256;

// Indices du contexte `ctx` (mêmes numéros que `CTX_*` de core.py)
pub const CTX_MINTHROTTLE: usize = 0;
pub const CTX_MOTOR0_IDX: usize = 1;
pub const CTX_VBATREF: usize = 2;
pub const CTX_MOTOR_MIN: usize = 3;
pub const CTX_HOME0_IDX: usize = 4;
pub const CTX_HOME1_IDX: usize = 5;
pub const CTX_HAS_LAST_TIME: usize = 6;
pub const CTX_LAST_TIME: usize = 7;
pub const CTX_DATA_VERSION: usize = 8;
pub const CTX_SIZE: usize = 9;

pub const EVENT_SYNC_BEEP: i64 = 0;
pub const EVENT_AUTOTUNE_CYCLE_START: i64 = 10;
pub const EVENT_AUTOTUNE_CYCLE_RESULT: i64 = 11;
pub const EVENT_AUTOTUNE_TARGETS: i64 = 12;
pub const EVENT_INFLIGHT_ADJUSTMENT: i64 = 13;
pub const EVENT_LOGGING_RESUME: i64 = 14;
pub const EVENT_DISARM: i64 = 15;
pub const EVENT_GTUNE_CYCLE_RESULT: i64 = 20;
pub const EVENT_FLIGHT_MODE: i64 = 30;
pub const EVENT_TWITCH_TEST: i64 = 40;
pub const EVENT_LOG_END: i64 = 255;

/// Lecture tolérante : numba lit hors tableau sans erreur (comportement indéfini) ; ici, 0. ⚠️ (L-032)
fn at(a: &[i64], i: i64) -> i64 {
    usize::try_from(i).ok().and_then(|i| a.get(i)).copied().unwrap_or(0)
}

/// Valeur prédite ; `prev` / `prev2` ne servent que si `has_prev`.
#[allow(clippy::too_many_arguments)]
fn predict(
    i: usize,
    p: i64,
    v: i64,
    cur: &[i64],
    prev: &[i64],
    prev2: &[i64],
    has_prev: bool,
    ctx: &[i64],
    home: &[i64],
) -> i64 {
    match p {
        PRED_PREVIOUS if has_prev => v.wrapping_add(prev[i]),
        PRED_STRAIGHT_LINE if has_prev => v.wrapping_add(prev[i].wrapping_mul(2)).wrapping_sub(prev2[i]),
        PRED_AVERAGE_2 if has_prev => {
            let s = prev[i].wrapping_add(prev2[i]);
            // division entière tronquée vers zéro, comme en C (la valeur absolue se divise par « // » de numba)
            let q = s.wrapping_abs().div_euclid(2);
            v.wrapping_add(if s >= 0 { q } else { q.wrapping_neg() })
        }
        PRED_MINTHROTTLE => to_int32(v).wrapping_add(ctx[CTX_MINTHROTTLE]),
        PRED_MINMOTOR => to_int32(v).wrapping_add(ctx[CTX_MOTOR_MIN]),
        PRED_MOTOR_0 => v.wrapping_add(at(cur, ctx[CTX_MOTOR0_IDX])),
        PRED_1500 => v.wrapping_add(1500),
        PRED_VBATREF => v.wrapping_add(ctx[CTX_VBATREF]),
        PRED_HOME_COORD => v.wrapping_add(at(home, ctx[CTX_HOME0_IDX])),
        PRED_HOME_COORD_1 => v.wrapping_add(at(home, ctx[CTX_HOME1_IDX])),
        PRED_LAST_MAIN_FRAME_TIME if ctx[CTX_HAS_LAST_TIME] != 0 => v.wrapping_add(ctx[CTX_LAST_TIME]),
        _ => v,
    }
}

/// Définition d'un type de trame : prédicteur et encodage de chaque champ.
pub struct FrameDef<'a> {
    pub pred: &'a [i64],
    pub enc: &'a [i64],
    pub n: usize,
}

/// Lit une trame et écrit ses champs dans `cur`.
#[allow(clippy::too_many_arguments)]
pub fn parse_frame(
    s: &mut Stream,
    def: &FrameDef,
    cur: &mut [i64],
    prev: &[i64],
    prev2: &[i64],
    has_prev: bool,
    skipped: i64,
    ctx: &[i64],
    home: &[i64],
    values: &mut [i64; 8],
) {
    let n = def.n;
    let mut i = 0;
    while i < n {
        let p = def.pred[i];
        if p == PRED_INC {
            cur[i] = skipped.wrapping_add(1);
            if has_prev {
                cur[i] = cur[i].wrapping_add(prev[i]);
            }
            i += 1;
            continue;
        }
        let e = def.enc[i];
        // champs groupés : l'encodage lit plusieurs valeurs d'un coup
        let group = match e {
            ENC_TAG8_4S16 => {
                if ctx[CTX_DATA_VERSION] < 2 {
                    read_tag8_4s16_v1(s, values);
                } else {
                    read_tag8_4s16_v2(s, values);
                }
                Some(4)
            }
            ENC_TAG2_3S32 => {
                read_tag2_3s32(s, values);
                Some(3)
            }
            ENC_TAG2_3SVARIABLE => {
                read_tag2_3svariable(s, values);
                Some(3)
            }
            ENC_TAG8_8SVB => {
                let mut j = i + 1;
                while j < i + 8 && j < n && def.enc[j] == ENC_TAG8_8SVB {
                    j += 1;
                }
                let count = j - i;
                read_tag8_8svb(s, values, count);
                // ce groupe ne dépasse jamais n (j < n) : pas de test « i < n » par valeur
                for &v in values.iter().take(count) {
                    cur[i] = predict(i, def.pred[i], v, cur, prev, prev2, has_prev, ctx, home);
                    i += 1;
                }
                continue;
            }
            _ => None,
        };
        if let Some(count) = group {
            for &v in values.iter().take(count) {
                if i < n {
                    cur[i] = predict(i, def.pred[i], v, cur, prev, prev2, has_prev, ctx, home);
                }
                i += 1;
            }
            continue;
        }
        let v = match e {
            ENC_SIGNED_VB => read_svb(s),
            ENC_UNSIGNED_VB => read_uvb(s),
            ENC_NEG_14BIT => -sign_extend(read_uvb(s), 14),
            // ENC_NULL, ou encodage inconnu : rien à lire (la trame sera probablement rejetée)
            _ => 0,
        };
        cur[i] = predict(i, p, v, cur, prev, prev2, has_prev, ctx, home);
        i += 1;
    }
}

/// Événement : (valide, type, donnée 1, donnée 2). `s.end` est ramené à la position courante après un
/// vrai « End of log ».
pub fn parse_event(s: &mut Stream) -> (bool, i64, i64, i64) {
    let t = s.byte();
    let (mut d1, mut d2) = (0i64, 0i64);
    let le32 = |s: &mut Stream| {
        let (b1, b2, b3, b4) = (s.byte(), s.byte(), s.byte(), s.byte());
        b1 | (b2 << 8) | (b3 << 16) | (b4 << 24)
    };
    match t {
        EVENT_SYNC_BEEP | EVENT_DISARM => d1 = read_uvb(s),
        EVENT_FLIGHT_MODE | EVENT_LOGGING_RESUME => {
            d1 = read_uvb(s);
            d2 = read_uvb(s);
        }
        EVENT_AUTOTUNE_CYCLE_START => (0..5).for_each(|_| {
            s.byte();
        }),
        EVENT_AUTOTUNE_CYCLE_RESULT => (0..4).for_each(|_| {
            s.byte();
        }),
        EVENT_AUTOTUNE_TARGETS => (0..8).for_each(|_| {
            s.byte();
        }),
        EVENT_GTUNE_CYCLE_RESULT => {
            s.byte();
            d1 = read_svb(s);
            s.byte();
            s.byte();
        }
        EVENT_INFLIGHT_ADJUSTMENT => {
            let f = s.byte();
            d1 = f & 127;
            d2 = if f < 128 { read_svb(s) } else { le32(s) };
        }
        EVENT_TWITCH_TEST => {
            d1 = s.byte();
            d2 = le32(s);
        }
        EVENT_LOG_END => {
            // « End of log\0 » : sinon ce n'était pas un vrai événement de fin (les 11 octets sont lus dans tous les cas)
            let mut ok = true;
            for c in b"End of log\0" {
                if s.byte() != *c as i64 {
                    ok = false;
                }
            }
            if ok {
                s.end = s.pos;
            }
            return (ok, t, d1, d2);
        }
        _ => return (false, t, d1, d2),
    }
    (true, t, d1, d2)
}
