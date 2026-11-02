//! Export d'une image PNG (bouton « Exporter image » des outils, icône de la barre du haut) : nom proposé,
//! recadrage de la capture d'écran et écriture. Port de `MainWindow.save_figure`, sans egui ni fenêtre de dialogue.

use std::path::{Path, PathBuf};

use egui::{ColorImage, Rect};

/// Jour civil (année, mois, jour) du nombre de jours écoulés depuis le 1er janvier 1970 (calendrier grégorien).
/// Algorithme de H. Hinnant, `civil_from_days`.
pub fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // jour dans l'ère, 0..=146096
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = (yoe + era * 400) as i32 + i32::from(m <= 2);
    (y, m, d)
}

/// Date du jour en UTC. Python prend la date locale : l'écart d'un jour est possible autour de minuit (W-038).
pub fn today_utc() -> (i32, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    civil_from_days(secs.div_euclid(86_400))
}

/// `OTB_FIGS_AAAA-MM-JJ-N.png` dans `dir`, avec N le premier numéro libre à partir de 1.
pub fn default_export_path(dir: &Path, today: (i32, u32, u32), exists: &dyn Fn(&Path) -> bool) -> PathBuf {
    let (y, m, d) = today;
    let mut n = 1u32;
    loop {
        let p = dir.join(format!("OTB_FIGS_{y:04}-{m:02}-{d:02}-{n}.png"));
        if !exists(&p) {
            return p;
        }
        n += 1;
    }
}

/// Ajoute `.png` si le nom n'a pas cette extension (Python laisse le nom tel quel).
pub fn with_png_extension(path: PathBuf) -> PathBuf {
    let est_png = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if est_png {
        return path;
    }
    let mut s = path.into_os_string();
    s.push(".png");
    PathBuf::from(s)
}

/// Partie de la capture (en pixels) qui couvre `rect` (en points, `ppp` pixels par point), bornée à l'image :
/// `ColorImage::region` panique si le rectangle dépasse. `None` si rien ne reste.
pub fn crop(img: &ColorImage, rect: Rect, ppp: f32) -> Option<ColorImage> {
    let [w, h] = img.size;
    let borne = |v: f32, max: usize| (v.max(0.0) as usize).min(max);
    let (x0, y0) = (
        borne((rect.min.x * ppp).floor(), w),
        borne((rect.min.y * ppp).floor(), h),
    );
    let (x1, y1) = (borne((rect.max.x * ppp).ceil(), w), borne((rect.max.y * ppp).ceil(), h));
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(img.region_by_pixels([x0, y0], [x1 - x0, y1 - y0]))
}

/// Écrit `img` en PNG.
pub fn write_png(img: &ColorImage, path: &Path) -> Result<(), String> {
    let [w, h] = img.size;
    let octets: Vec<u8> = img.pixels.iter().flat_map(|p| p.to_array()).collect();
    let rgba = image::RgbaImage::from_raw(w as u32, h as u32, octets)
        .ok_or_else(|| "image de taille incohérente".to_string())?;
    rgba.save(path).map_err(|e| e.to_string())
}
