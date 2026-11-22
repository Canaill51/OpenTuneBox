//! `otb-dev usb` : l'identifiant de la carte n'est jamais affiché en entier.

use otb_usb::Uid;

#[test]
fn identifiant_resume_sans_le_numero() {
    let uid = Uid([0x0033_0021, 0x3438_5107, 0x3436_3432]);
    let s = otb_dev::usb::uid_summary(&uid);
    assert!(s.starts_with("non nul, empreinte "), "{s}");
    assert_eq!(s.len(), "non nul, empreinte ".len() + 8);
    assert!(
        !s.contains(&uid.to_hex()[..8]) && !s.contains(&uid.to_hex()[16..]),
        "{s}"
    );
    // Stable d'un lancement à l'autre, différente pour une autre carte.
    assert_eq!(s, otb_dev::usb::uid_summary(&uid));
    assert_ne!(
        s,
        otb_dev::usb::uid_summary(&Uid([0x0033_0021, 0x3438_5107, 0x3436_3433]))
    );
    assert_eq!(otb_dev::usb::uid_summary(&Uid([0; 3])), "nul");
}
