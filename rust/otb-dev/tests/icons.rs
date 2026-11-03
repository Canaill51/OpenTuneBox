//! Icônes : rendu du SVG, PNG Linux, `.ico` Windows, fichiers versionnés à jour.
//!
//! Mesure du 2026-11-03 du rendu de `resvg` contre l'ancien PNG de Qt (`make_icons.py`), 256 × 256, composé sur fond blanc : écart moyen 1,49 sur 255,
//! 1,4 % des pixels à plus de 32 d'écart, tous sur le contour des traits (lissage propre à chaque moteur, carte des écarts regardée) ; centre du trait
//! orange (245, 130, 49) et coin transparent identiques.

use ico::{IconDir, IconImage};
use otb_dev::icons::*;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="2" y="2" width="60" height="60" rx="12" fill="#1e2a3a"/><circle cx="32" cy="32" r="10" fill="#f58231"/></svg>"##;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn px(img: &IconImage, x: usize, y: usize) -> [u8; 4] {
    let i = (y * img.width() as usize + x) * 4;
    img.rgba_data()[i..i + 4].try_into().expect("4 octets")
}

#[test]
fn render_size_alpha_and_colours() {
    let img = render(SVG, 64).expect("rendu");
    assert_eq!((img.width(), img.height()), (64, 64));
    assert_eq!(
        px(&img, 0, 0),
        [0, 0, 0, 0],
        "coin transparent (hors du rectangle arrondi)"
    );
    assert_eq!(
        px(&img, 10, 32),
        [0x1e, 0x2a, 0x3a, 255],
        "fond opaque, couleur non prémultipliée"
    );
    assert_eq!(px(&img, 32, 32), [0xf5, 0x82, 0x31, 255], "disque orange au centre");
    // Même image à une autre taille : le dessin suit l'échelle (le centre reste orange, le point (10, 32) devient (20, 64)).
    let big = render(SVG, 128).expect("rendu");
    assert_eq!(px(&big, 64, 64), [0xf5, 0x82, 0x31, 255]);
    assert_eq!(px(&big, 20, 64), [0x1e, 0x2a, 0x3a, 255]);
}

#[test]
fn render_keeps_straight_colours_under_partial_alpha() {
    let half = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" fill="#f58231" fill-opacity="0.5"/></svg>"##;
    let p = px(&render(half, 16).expect("rendu"), 8, 8);
    // Couleur d'origine (pas multipliée par l'alpha, qui donnerait ≈ 122, 65, 24) et alpha d'environ la moitié.
    assert!((0x80..=0x81).contains(&p[3]), "alpha {p:?}");
    assert!(
        p[0] >= 0xf3 && (0x80..=0x84).contains(&p[1]) && (0x2f..=0x33).contains(&p[2]),
        "couleur {p:?}"
    );
}

#[test]
fn render_refuses_bad_input() {
    assert!(
        render(b"pas un svg", 32)
            .err()
            .expect("erreur")
            .contains("SVG illisible")
    );
    assert!(render(SVG, 0).is_err(), "taille nulle");
    assert!(
        render(SVG, 257).err().expect("erreur").contains("de 1 à 256"),
        "au-delà du maximum d'un .ico, sans panique"
    );
    let wide = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 32"><rect width="64" height="32"/></svg>"#;
    assert!(render(wide, 32).err().expect("erreur").contains("carrée"));
}

#[test]
fn png_decodes_to_the_requested_size() {
    for size in [16, 256] {
        let bytes = png_bytes(SVG, size).expect("png");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let back = IconImage::read_png(Cursor::new(bytes)).expect("relecture");
        assert_eq!((back.width(), back.height()), (size, size));
    }
}

#[test]
fn ico_has_every_size_and_each_image_is_the_render() {
    let dir = IconDir::read(Cursor::new(ico_bytes(SVG).expect("ico"))).expect("relecture");
    let mut sizes: Vec<u32> = dir.entries().iter().map(|e| e.width()).collect();
    sizes.sort_unstable();
    assert_eq!(sizes, ICO_SIZES);
    for e in dir.entries() {
        assert_eq!(e.width(), e.height());
        let decoded = e.decode().expect("décodage");
        let fresh = render(SVG, e.width()).expect("rendu");
        // Seul le canal alpha et la couleur des pixels visibles comptent : sous alpha 0, la couleur est quelconque.
        for (a, b) in decoded.rgba_data().chunks(4).zip(fresh.rgba_data().chunks(4)) {
            assert_eq!(a[3], b[3], "alpha à {} px", e.width());
            if b[3] != 0 {
                assert_eq!(a, b, "pixel à {} px", e.width());
            }
        }
    }
}

#[test]
fn generate_writes_both_files_and_nothing_on_error() {
    let dir = std::env::temp_dir().join(format!("otb-icons-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("packaging/linux")).expect("dossiers");
    let svg = dir.join("icone.svg");
    fs::write(&svg, SVG).expect("svg");
    let msg = generate(&dir, Some(&svg)).expect("génération");
    assert!(
        msg.contains("opentunebox.png") && msg.contains("opentunebox.ico"),
        "{msg}"
    );
    let png = fs::read(dir.join("packaging/linux/opentunebox.png")).expect("png");
    assert_eq!(png, png_bytes(SVG, LINUX_PNG_SIZE).expect("png"));
    assert_eq!(
        fs::read(dir.join("packaging/opentunebox.ico")).expect("ico"),
        ico_bytes(SVG).expect("ico")
    );
    // Un SVG illisible ne touche à aucun fichier existant.
    fs::write(&svg, b"casse").expect("svg cassé");
    assert!(generate(&dir, Some(&svg)).is_err());
    assert_eq!(fs::read(dir.join("packaging/linux/opentunebox.png")).expect("png"), png);
}

#[test]
fn find_svg_prefers_the_given_path_then_known_places() {
    let dir = std::env::temp_dir().join(format!("otb-svg-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    assert!(find_svg(&dir, None).unwrap_err().contains("introuvable"));
    let old = dir.join("src/opentunebox/gui/assets/opentunebox.svg");
    fs::create_dir_all(old.parent().expect("parent")).expect("dossiers");
    fs::write(&old, SVG).expect("svg");
    assert_eq!(find_svg(&dir, None), Ok(old.clone()));
    let new = dir.join("rust/otb-gui/assets/opentunebox.svg");
    fs::create_dir_all(new.parent().expect("parent")).expect("dossiers");
    fs::write(&new, SVG).expect("svg");
    assert_eq!(
        find_svg(&dir, None),
        Ok(new),
        "le nouvel emplacement passe avant l'ancien"
    );
    assert_eq!(find_svg(&dir, Some(Path::new("x.svg"))), Ok(PathBuf::from("x.svg")));
}

/// Les fichiers versionnés sont ceux que produit `otb-dev icons` à partir du SVG du dépôt (à 3 niveaux près par canal : le lissage varie
/// d'un processeur à l'autre).
#[test]
fn versioned_icons_are_up_to_date() {
    let root = root();
    let svg = fs::read(find_svg(&root, None).expect("svg du dépôt")).expect("lecture");
    let kept = IconImage::read_png(Cursor::new(
        fs::read(root.join("packaging/linux/opentunebox.png")).expect("png versionné"),
    ))
    .expect("relecture");
    let fresh = render(&svg, LINUX_PNG_SIZE).expect("rendu");
    assert_eq!((kept.width(), kept.height()), (LINUX_PNG_SIZE, LINUX_PNG_SIZE));
    let worst = kept
        .rgba_data()
        .chunks(4)
        .zip(fresh.rgba_data().chunks(4))
        .map(|(a, b)| (0..4).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0))
        .max();
    assert!(
        worst.is_some_and(|w| w <= 3),
        "packaging/linux/opentunebox.png n'est pas à jour : relancer `otb-dev icons` (écart {worst:?})"
    );
    let dir = IconDir::read(Cursor::new(
        fs::read(root.join("packaging/opentunebox.ico")).expect(".ico versionné"),
    ))
    .expect("relecture");
    let mut sizes: Vec<u32> = dir.entries().iter().map(|e| e.width()).collect();
    sizes.sort_unstable();
    assert_eq!(
        sizes, ICO_SIZES,
        "packaging/opentunebox.ico n'a pas les tailles attendues : relancer `otb-dev icons`"
    );
}
