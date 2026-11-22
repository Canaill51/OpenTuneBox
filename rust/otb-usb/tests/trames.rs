//! Trames MSP : valeurs connues, aller-retour, « jumbo », v2, bruit et octet altéré.

use otb_usb::frame::{Direction, Frame, FrameError, JUMBO_LIMIT, Parser, Version, crc8_dvb_s2, encode};

fn parse_all(bytes: &[u8]) -> Vec<Result<Frame, FrameError>> {
    let mut p = Parser::new();
    bytes.iter().filter_map(|&b| p.push(b)).collect()
}

#[test]
fn demande_v1_connue() {
    // MSP_API_VERSION : `$M<`, taille 0, commande 1, contrôle 0 ^ 1.
    assert_eq!(encode(Version::V1, Direction::Request, 1, &[]), b"$M<\x00\x01\x01");
}

#[test]
fn crc_dvb_s2_valeur_de_controle() {
    // Valeur de contrôle publiée du CRC-8/DVB-S2 sur « 123456789 ».
    assert_eq!(crc8_dvb_s2(0, b"123456789"), 0xBC);
}

#[test]
fn demande_v2_connue() {
    let f = encode(Version::V2, Direction::Request, 1, &[]);
    assert_eq!(&f[..8], b"$X<\x00\x01\x00\x00\x00");
    assert_eq!(f[8], crc8_dvb_s2(0, &[0, 1, 0, 0, 0]));
}

#[test]
fn aller_retour_toutes_tailles() {
    for version in [Version::V1, Version::V2] {
        for n in [0usize, 1, 253, 254, 255, 256, 4080, 4096] {
            let payload: Vec<u8> = (0..n).map(|i| (i * 7 + 3) as u8).collect();
            let bytes = encode(version, Direction::Reply, 71, &payload);
            let got = parse_all(&bytes);
            assert_eq!(got.len(), 1, "{version:?} {n}");
            let f = got[0].as_ref().expect("trame valide");
            assert_eq!((f.version, f.direction, f.cmd), (version, Direction::Reply, 71));
            assert_eq!(f.payload, payload, "{version:?} {n}");
        }
    }
}

#[test]
fn jumbo_a_partir_de_255_octets() {
    // Firmware : `dataLen >= JUMBO_FRAME_SIZE_LIMIT` (255) → taille 255, puis la vraie sur 16 bits.
    let court = encode(Version::V1, Direction::Reply, 71, &[0; 254]);
    assert_eq!(court[3], 254);
    assert_eq!(court.len(), 3 + 2 + 254 + 1);
    let jumbo = encode(Version::V1, Direction::Reply, 71, &[0; JUMBO_LIMIT]);
    assert_eq!(&jumbo[3..7], &[255, 71, 255, 0]);
    assert_eq!(jumbo.len(), 3 + 4 + 255 + 1);
    // La vraie taille entre dans le OU exclusif : 255 ^ 71 ^ 255 ^ 0 = 71.
    assert_eq!(*jumbo.last().unwrap(), 71);
}

#[test]
fn reponse_d_erreur() {
    let got = parse_all(&encode(Version::V1, Direction::Error, 72, &[]));
    assert_eq!(got[0].as_ref().unwrap().direction, Direction::Error);
}

#[test]
fn bruit_ignore_et_trames_collees() {
    let mut bytes = b"\r\n# Entering CLI$$M".to_vec();
    bytes.extend(encode(Version::V1, Direction::Reply, 1, &[0, 1, 47]));
    bytes.extend(encode(Version::V2, Direction::Reply, 160, &[9; 12]));
    let got = parse_all(&bytes);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].as_ref().unwrap().payload, vec![0, 1, 47]);
    assert_eq!(got[1].as_ref().unwrap().cmd, 160);
}

#[test]
fn octet_altere_detecte() {
    for version in [Version::V1, Version::V2] {
        let mut bytes = encode(version, Direction::Reply, 71, &[1, 2, 3, 4]);
        let k = bytes.len() - 2;
        bytes[k] ^= 0x10;
        assert_eq!(
            parse_all(&bytes),
            vec![Err(FrameError::Checksum { cmd: 71 })],
            "{version:?}"
        );
    }
}

#[test]
fn taille_absurde_refusee() {
    // `$X>` drapeau 0, commande 71, taille 60 000 : du bruit pris pour une trame.
    let got = parse_all(b"$X>\x00\x47\x00\x60\xea");
    assert_eq!(got, vec![Err(FrameError::TooLarge { size: 60_000 })]);
}
