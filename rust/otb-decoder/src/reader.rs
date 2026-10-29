//! Lecture d'octets et décodage des encodages Blackbox (miroir de `decoder/core.py`, section « lecture d'octets »).
//!
//! ⚠️ Toute l'arithmétique est en `i64` avec les mêmes dépassements que numba : un octet manquant (fin de
//! fichier) vaut -1 et traverse les calculs tel quel, comme dans la version Python (D-031).

/// Flux d'octets : position, fin de la session, drapeau « fin de fichier atteinte ».
pub struct Stream<'a> {
    pub buf: &'a [u8],
    pub pos: i64,
    pub end: i64,
    pub eof: bool,
}

impl<'a> Stream<'a> {
    pub fn new(buf: &'a [u8], start: i64, end: i64) -> Self {
        Stream {
            buf,
            pos: start,
            end,
            eof: false,
        }
    }

    /// Octet suivant, ou -1 (et `eof`) à la fin.
    pub fn byte(&mut self) -> i64 {
        if self.pos >= self.end || self.pos < 0 || self.pos as usize >= self.buf.len() {
            self.eof = true;
            return -1;
        }
        let b = self.buf[self.pos as usize] as i64;
        self.pos += 1;
        b
    }
}

pub fn to_int32(v: i64) -> i64 {
    (v & 0xFFFF_FFFF) as u32 as i32 as i64
}

pub fn sign_extend(v: i64, bits: u32) -> i64 {
    let v = v & ((1i64 << bits) - 1);
    if v & (1i64 << (bits - 1)) != 0 {
        v - (1i64 << bits)
    } else {
        v
    }
}

pub fn read_uvb(s: &mut Stream) -> i64 {
    let mut result = 0i64;
    let mut shift = 0;
    for _ in 0..5 {
        let b = s.byte();
        if b < 0 {
            return 0;
        }
        result |= (b & 0x7F) << shift;
        if b < 128 {
            return result & 0xFFFF_FFFF;
        }
        shift += 7;
    }
    0
}

pub fn read_svb(s: &mut Stream) -> i64 {
    let u = read_uvb(s);
    to_int32((u >> 1) ^ (-(u & 1)))
}

fn tag2_wide(s: &mut Stream, values: &mut [i64], mut lead: i64) {
    for v in values.iter_mut().take(3) {
        *v = match lead & 0x03 {
            0 => sign_extend(s.byte(), 8),
            1 => {
                let (b1, b2) = (s.byte(), s.byte());
                sign_extend(b1 | (b2 << 8), 16)
            }
            2 => {
                let (b1, b2, b3) = (s.byte(), s.byte(), s.byte());
                sign_extend(b1 | (b2 << 8) | (b3 << 16), 24)
            }
            _ => {
                let (b1, b2, b3, b4) = (s.byte(), s.byte(), s.byte(), s.byte());
                to_int32(b1 | (b2 << 8) | (b3 << 16) | (b4 << 24))
            }
        };
        lead >>= 2;
    }
}

pub fn read_tag2_3s32(s: &mut Stream, values: &mut [i64]) {
    let mut lead = s.byte();
    match (lead >> 6) & 3 {
        0 => {
            values[0] = sign_extend((lead >> 4) & 0x03, 2);
            values[1] = sign_extend((lead >> 2) & 0x03, 2);
            values[2] = sign_extend(lead & 0x03, 2);
        }
        1 => {
            values[0] = sign_extend(lead & 0x0F, 4);
            lead = s.byte();
            values[1] = sign_extend(lead >> 4, 4);
            values[2] = sign_extend(lead & 0x0F, 4);
        }
        2 => {
            values[0] = sign_extend(lead & 0x3F, 6);
            lead = s.byte();
            values[1] = sign_extend(lead & 0x3F, 6);
            lead = s.byte();
            values[2] = sign_extend(lead & 0x3F, 6);
        }
        _ => tag2_wide(s, values, lead),
    }
}

pub fn read_tag2_3svariable(s: &mut Stream, values: &mut [i64]) {
    let lead = s.byte();
    match (lead >> 6) & 3 {
        0 => {
            values[0] = sign_extend((lead >> 4) & 0x03, 2);
            values[1] = sign_extend((lead >> 2) & 0x03, 2);
            values[2] = sign_extend(lead & 0x03, 2);
        }
        1 => {
            values[0] = sign_extend((lead & 0x3E) >> 1, 5);
            let lead2 = s.byte();
            values[1] = sign_extend(((lead & 0x01) << 4) | ((lead2 & 0xF0) >> 4), 5);
            values[2] = sign_extend(lead2 & 0x0F, 4);
        }
        2 => {
            let lead2 = s.byte();
            values[0] = sign_extend(((lead & 0x3F) << 2) | ((lead2 & 0xC0) >> 6), 8);
            let lead3 = s.byte();
            values[1] = sign_extend(((lead2 & 0x3F) << 1) | ((lead3 & 0x80) >> 7), 7);
            values[2] = sign_extend(lead3 & 0x7F, 7);
        }
        _ => tag2_wide(s, values, lead),
    }
}

pub fn read_tag8_4s16_v1(s: &mut Stream, values: &mut [i64]) {
    let mut selector = s.byte();
    let mut i = 0;
    while i < 4 {
        match selector & 0x03 {
            0 => values[i] = 0,
            1 => {
                let c = s.byte();
                values[i] = sign_extend(c & 0x0F, 4);
                i += 1;
                selector >>= 2;
                if i < 4 {
                    values[i] = sign_extend(c >> 4, 4);
                }
            }
            2 => values[i] = sign_extend(s.byte(), 8),
            _ => {
                let (c1, c2) = (s.byte(), s.byte());
                values[i] = sign_extend(c1 | (c2 << 8), 16);
            }
        }
        selector >>= 2;
        i += 1;
    }
}

pub fn read_tag8_4s16_v2(s: &mut Stream, values: &mut [i64]) {
    let mut selector = s.byte();
    let mut nibble = 0;
    let mut buffer = 0i64;
    for v in values.iter_mut().take(4) {
        match selector & 0x03 {
            0 => *v = 0,
            1 => {
                if nibble == 0 {
                    buffer = s.byte();
                    *v = sign_extend(buffer >> 4, 4);
                    nibble = 1;
                } else {
                    *v = sign_extend(buffer & 0x0F, 4);
                    nibble = 0;
                }
            }
            2 => {
                if nibble == 0 {
                    *v = sign_extend(s.byte(), 8);
                } else {
                    let mut c1 = (buffer & 0x0F) << 4;
                    buffer = s.byte();
                    c1 |= buffer >> 4;
                    *v = sign_extend(c1, 8);
                }
            }
            _ => {
                if nibble == 0 {
                    let (c1, c2) = (s.byte(), s.byte());
                    *v = sign_extend((c1 << 8) | c2, 16);
                } else {
                    let (c1, c2) = (s.byte(), s.byte());
                    *v = sign_extend(((buffer & 0x0F) << 12) | (c1 << 4) | (c2 >> 4), 16);
                    buffer = c2;
                }
            }
        }
        selector >>= 2;
    }
}

pub fn read_tag8_8svb(s: &mut Stream, values: &mut [i64], count: usize) {
    if count == 1 {
        values[0] = read_svb(s);
    } else {
        let mut header = s.byte();
        for v in values.iter_mut().take(count) {
            *v = if header & 0x01 != 0 { read_svb(s) } else { 0 };
            header >>= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_and_signs() {
        // 300 = 0xAC 0x02 en uvb ; zigzag : 1 → -1, 2 → 1
        let mut s = Stream::new(&[0xAC, 0x02, 0x01, 0x02], 0, 4);
        assert_eq!(read_uvb(&mut s), 300);
        assert_eq!(read_svb(&mut s), -1);
        assert_eq!(read_svb(&mut s), 1);
        assert!(!s.eof);
        assert_eq!(s.byte(), -1);
        assert!(s.eof);
    }

    #[test]
    fn truncated_varint_is_zero() {
        let mut s = Stream::new(&[0x80, 0x80], 0, 2);
        assert_eq!(read_uvb(&mut s), 0);
        assert!(s.eof);
    }

    #[test]
    fn int32_and_sign_extension() {
        assert_eq!(to_int32(0xFFFF_FFFF), -1);
        assert_eq!(to_int32(0x7FFF_FFFF), 0x7FFF_FFFF);
        assert_eq!(sign_extend(0b10, 2), -2);
        assert_eq!(sign_extend(0b01, 2), 1);
        assert_eq!(sign_extend(-1, 4), -1); // octet manquant : -1 traverse les calculs
    }

    #[test]
    fn tag2_3s32_small_values() {
        // sélecteur 0 : trois valeurs de 2 bits (1, -2, -1)
        let mut s = Stream::new(&[0b0001_1011], 0, 1);
        let mut v = [0i64; 8];
        read_tag2_3s32(&mut s, &mut v);
        assert_eq!(&v[..3], &[1, -2, -1]);
    }

    #[test]
    fn tag8_8svb_skips_zero_fields() {
        // en-tête 0b101 : champs 0 et 2 présents (svb 2 → 1, svb 1 → -1), champ 1 nul
        let mut s = Stream::new(&[0b101, 2, 1], 0, 3);
        let mut v = [9i64; 8];
        read_tag8_8svb(&mut s, &mut v, 3);
        assert_eq!(&v[..3], &[1, 0, -1]);
    }
}
