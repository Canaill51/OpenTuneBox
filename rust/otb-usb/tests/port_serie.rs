//! Le vrai port série (`serial2`) sans matériel : un pseudo-terminal relie `SerialLink` à la carte simulée.
//! Linux seulement (le chemin du pseudo-terminal se lit dans `/proc`) ; la procédure d'essai à la main de
//! `docs/spec-usb.md` couvre une vraie carte, sous Linux et Windows.
#![cfg(target_os = "linux")]

use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use otb_usb::link::{Link, PortError, SerialLink};
use otb_usb::sim::SimBoard;
use otb_usb::{Msp, erase::verify_and_save};

/// Un pseudo-terminal : le côté « maître » reste au test, le chemin de l'autre côté est ouvert comme un port série.
fn pty() -> (serial2::SerialPort, serial2::SerialPort, PathBuf) {
    let (master, slave) = serial2::SerialPort::pair().expect("pseudo-terminal");
    let path = std::fs::read_link(format!("/proc/self/fd/{}", slave.as_raw_fd())).expect("chemin du pseudo-terminal");
    (master, slave, path)
}

#[test]
fn second_acces_refuse() {
    let (_master, _slave, path) = pty();
    let first = SerialLink::open(&path).expect("premier accès");
    match SerialLink::open(&path) {
        Err(PortError::Busy(p)) => assert_eq!(p, path),
        other => panic!("second accès : {other:?}"),
    }
    drop(first);
    SerialLink::open(&path).expect("accès rendu à la fermeture");
}

#[test]
fn copie_par_le_vrai_port() {
    let (mut master, _slave, path) = pty();
    let flash = std::fs::read(format!("{}/../../tests/data/jeno5_5s.bbl", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let board = Arc::new(Mutex::new(SimBoard::new(flash.clone())));
    let stop = Arc::new(AtomicBool::new(false));
    // La carte simulée de l'autre côté du fil.
    let relay = {
        let (board, stop) = (board.clone(), stop.clone());
        master.set_read_timeout(Duration::from_millis(20)).unwrap();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while !stop.load(Ordering::Relaxed) {
                let n = match master.read(&mut buf) {
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => 0,
                    Err(e) => panic!("{e}"),
                };
                let mut b = board.lock().unwrap();
                b.send(&buf[..n]).unwrap();
                let mut out = [0u8; 8192];
                loop {
                    let k = b.recv(&mut out, Duration::ZERO).unwrap();
                    if k == 0 {
                        break;
                    }
                    master.write_all(&out[..k]).unwrap();
                }
            }
        })
    };
    let mut msp = Msp::new(SerialLink::open(&path).unwrap());
    let copy = msp.copy_flash(|_, _| true).unwrap();
    assert_eq!(copy.bytes(), &flash[..]);
    let dir = std::env::temp_dir().join(format!("otb-usb-pty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let v = verify_and_save(copy, &dir.join("copie.bbl")).unwrap();
    assert_eq!(v.report().logs, 1);
    stop.store(true, Ordering::Relaxed);
    relay.join().unwrap();
}
