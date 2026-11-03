//! Icônes d'application refaites depuis `opentunebox.svg` (remplace `packaging/make_icons.py`, phase 7-2).
//!
//! - `packaging/linux/opentunebox.png` : 256 × 256, pour l'AppImage et l'icône de fenêtre (`include_bytes!` de `otb-gui`).
//! - `packaging/opentunebox.ico` : plusieurs tailles, pour l'installateur et l'exécutable Windows (`build.rs` de `otb-gui`).
//!
//! Plus de `.icns` : macOS est abandonné (D-042). Les fichiers produits restent versionnés ; relancer `otb-dev icons` si le SVG change.

use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use resvg::{tiny_skia, usvg};
use std::fs;
use std::path::{Path, PathBuf};

/// Tailles du fichier `.ico` (celles que Windows demande : barre des tâches, explorateur, écrans à forte densité).
pub const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
/// Taille du PNG Linux.
pub const LINUX_PNG_SIZE: u32 = 256;

/// Le SVG rendu à `size` × `size` pixels, fond transparent, couleurs non prémultipliées (RGBA).
pub fn render(svg: &[u8], size: u32) -> Result<IconImage, String> {
    // 256 est la taille maximale d'une entrée de `.ico` : au-delà, `IconImage` panique.
    if !(1..=256).contains(&size) {
        return Err(format!("taille invalide : {size} (de 1 à 256)"));
    }
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).map_err(|e| format!("SVG illisible : {e}"))?;
    let (w, h) = (tree.size().width(), tree.size().height());
    if (w - h).abs() > f32::EPSILON || w <= 0.0 {
        return Err(format!("l'icône doit être carrée ({w} × {h})"));
    }
    let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or_else(|| format!("taille invalide : {size}"))?;
    let scale = size as f32 / w;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(IconImage::from_rgba_data(size, size, pixmap.take_demultiplied()))
}

/// Le PNG d'une taille.
pub fn png_bytes(svg: &[u8], size: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    render(svg, size)?
        .write_png(&mut out)
        .map_err(|e| format!("encodage PNG : {e}"))?;
    Ok(out)
}

/// Le fichier `.ico` de toutes les tailles de `ICO_SIZES`.
pub fn ico_bytes(svg: &[u8]) -> Result<Vec<u8>, String> {
    let mut dir = IconDir::new(ResourceType::Icon);
    for size in ICO_SIZES {
        let entry = IconDirEntry::encode(&render(svg, size)?).map_err(|e| format!("encodage de {size} px : {e}"))?;
        dir.add_entry(entry);
    }
    let mut out = Vec::new();
    dir.write(&mut out).map_err(|e| format!("écriture du .ico : {e}"))?;
    Ok(out)
}

/// Le SVG de l'icône : celui qu'on donne, sinon le premier qui existe parmi les emplacements connus.
pub fn find_svg(root: &Path, given: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = given {
        return Ok(p.to_path_buf());
    }
    [
        "rust/otb-gui/assets/opentunebox.svg",
        "src/opentunebox/gui/assets/opentunebox.svg",
    ]
    .iter()
    .map(|p| root.join(p))
    .find(|p| p.exists())
    .ok_or_else(|| "opentunebox.svg introuvable (donner --svg CHEMIN)".into())
}

/// Écrit `packaging/linux/opentunebox.png` et `packaging/opentunebox.ico` ; rend le message à afficher.
pub fn generate(root: &Path, svg: Option<&Path>) -> Result<String, String> {
    let svg_path = find_svg(root, svg)?;
    let data = fs::read(&svg_path).map_err(|e| format!("lecture de {} impossible : {e}", svg_path.display()))?;
    // Tout est calculé avant la première écriture.
    let png = png_bytes(&data, LINUX_PNG_SIZE)?;
    let ico = ico_bytes(&data)?;
    let (png_path, ico_path) = (
        root.join("packaging/linux/opentunebox.png"),
        root.join("packaging/opentunebox.ico"),
    );
    for (path, bytes) in [(&png_path, &png), (&ico_path, &ico)] {
        fs::write(path, bytes).map_err(|e| format!("écriture de {} impossible : {e}", path.display()))?;
    }
    Ok(format!(
        "Icônes écrites depuis {} : packaging/linux/opentunebox.png ({} octets), packaging/opentunebox.ico ({} octets, tailles {:?}).",
        svg_path.display(),
        png.len(),
        ico.len(),
        ICO_SIZES
    ))
}
