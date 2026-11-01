//! Les contournements d'egui_plot trouvés par le prototype E1 (D-035), écrits une seule fois pour tous les outils :
//! axe logarithmique, barres d'erreur, carte avec barre de couleurs, lignes et régions déplaçables, second axe Y,
//! et la décimation min/max des longues séries.

use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use egui_plot::{GridInput, GridMark};

use crate::theme;

// ------------------------------------------------------------------ axe logarithmique

/// Graduations 1-2-5 × 10^k d'un axe tracé en log10(f) (egui_plot n'a pas d'axe logarithmique).
pub fn log_grid(input: GridInput) -> Vec<GridMark> {
    let (lo, hi) = input.bounds;
    // bornes folles (dézoom extrême, NaN) : ni débordement d'entier ni milliers de repères par cadre
    if !lo.is_finite() || !hi.is_finite() {
        return Vec::new();
    }
    let (lo, hi) = (lo.max(-30.0), hi.min(30.0));
    let mut out = Vec::new();
    for dec in (lo.floor() as i32 - 1)..=(hi.ceil() as i32) {
        for (m, step) in [(1.0, 1.0), (2.0, 0.3), (5.0, 0.3)] {
            let v = (m * 10f64.powi(dec)).log10();
            if v >= lo && v <= hi {
                out.push(GridMark {
                    value: v,
                    step_size: step,
                });
            }
        }
    }
    out
}

/// Libellé d'une graduation de fréquence : « 500 », « 2 k », « 1,5 k ». `f` vient de `10^log10(f)` : arrondi à
/// 3 chiffres significatifs, virgule décimale française.
pub fn format_hz(f: f64) -> String {
    let (v, unite) = if f >= 1000.0 { (f / 1000.0, " k") } else { (f, "") };
    let d = if v == 0.0 {
        0
    } else {
        (2 - v.abs().log10().floor() as i32).max(0) as usize
    };
    let mut s = format!("{v:.d$}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    format!("{}{unite}", s.replace('.', ","))
}

/// Points `[log10(f), y]` d'une courbe sur axe logarithmique (les fréquences < 1 Hz sont écartées).
pub fn log_points(f: &[f32], y: &[f32]) -> Vec<[f64; 2]> {
    f.iter()
        .zip(y)
        .filter(|(f, _)| **f >= 1.0)
        .map(|(f, y)| [(*f as f64).log10(), *y as f64])
        .collect()
}

// ------------------------------------------------------------------ barres d'erreur

/// Segments verticaux `[x, mean − err]` → `[x, mean + err]`, un toutes les `step` abscisses
/// (egui_plot n'a pas de barres d'erreur : chaque barre est une ligne de deux points).
pub fn error_bars(x: &[f64], mean: &[f64], err: &[f64], step: usize) -> Vec<Vec<[f64; 2]>> {
    (0..x.len().min(mean.len()).min(err.len()))
        .step_by(step.max(1))
        .map(|i| vec![[x[i], mean[i] - err[i]], [x[i], mean[i] + err[i]]])
        .collect()
}

// ------------------------------------------------------------------ carte et barre de couleurs

/// Pixels d'une carte `valeurs[ligne * nt + colonne]` (ligne 0 = fréquence la plus basse), colorée entre `lo` et
/// `hi`. Les NaN restent transparents. L'image est retournée : sa première ligne est le haut de l'écran.
pub fn heatmap_pixels(values: &[f32], nf: usize, nt: usize, lo: f32, hi: f32) -> Vec<Color32> {
    assert_eq!(
        values.len(),
        nf * nt,
        "carte de {} valeurs pour {nf} × {nt}",
        values.len()
    );
    let span = if hi > lo { hi - lo } else { 1.0 };
    let mut pixels = vec![Color32::TRANSPARENT; nf * nt];
    for fi in 0..nf {
        for ti in 0..nt {
            let v = values[fi * nt + ti];
            if !v.is_nan() {
                pixels[(nf - 1 - fi) * nt + ti] = theme::turbo(((v - lo) / span).clamp(0.0, 1.0));
            }
        }
    }
    pixels
}

/// Texture d'une carte (voir `heatmap_pixels`), à afficher avec `PlotImage` : `Heatmap` d'egui_plot coûte 30 ms par
/// cadre à 10⁵ cases (mesuré en 6-0, docs/migration-rust.md).
pub fn heatmap_texture(
    ctx: &Context,
    name: &str,
    values: &[f32],
    nf: usize,
    nt: usize,
    lo: f32,
    hi: f32,
) -> TextureHandle {
    let pixels = heatmap_pixels(values, nf, nt, lo, hi);
    let image = ColorImage {
        size: [nt, nf],
        source_size: egui::vec2(nt as f32, nf as f32),
        pixels,
    };
    ctx.load_texture(name, image, TextureOptions::NEAREST)
}

/// Barre de couleurs verticale (haut = valeur la plus forte), `height` points de haut.
pub fn colorbar(ui: &mut egui::Ui, height: f32) {
    const STEPS: usize = 64;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    for i in 0..STEPS {
        let y0 = rect.top() + rect.height() * i as f32 / STEPS as f32;
        let band = egui::Rect::from_min_max(
            egui::pos2(rect.left(), y0),
            egui::pos2(rect.right(), y0 + rect.height() / STEPS as f32 + 1.0),
        );
        painter.rect_filled(band, 0.0, theme::turbo(1.0 - i as f32 / (STEPS - 1) as f32));
    }
}

// ------------------------------------------------------------------ lignes et régions déplaçables

/// Distance (en pixels) à laquelle on « attrape » un repère.
pub const GRAB_PX: f32 = 6.0;

/// État d'un glisser de repères verticaux. À garder entre deux cadres.
#[derive(Clone, Copy, Debug, Default)]
pub struct VDrag {
    /// Repère en cours de déplacement.
    pub active: Option<usize>,
    /// Le pointeur survole un repère (le plot ne doit pas se déplacer).
    pub hot: bool,
}

impl VDrag {
    /// Le plot doit refuser son propre glisser (`Plot::allow_drag(!busy)`).
    pub fn busy(&self) -> bool {
        self.hot || self.active.is_some()
    }

    /// À appeler dans la fermeture de `Plot::show` après avoir dessiné les repères. `xs` : abscisses des
    /// repères ; `limits` : bornes de chacun. Déplace le repère attrapé vers le pointeur.
    ///
    /// egui annonce le début du glisser après un déplacement : le repère se cherche là où le bouton a été
    /// pressé (`press_origin`), pas là où est le pointeur.
    pub fn update(&mut self, pu: &egui_plot::PlotUi, press_x: Option<f32>, xs: &mut [f64], limits: &[(f64, f64)]) {
        let Some(p) = pu.pointer_coordinate() else { return };
        let tr = *pu.transform();
        let near = |at: Option<f32>, xs: &[f64]| {
            xs.iter()
                .position(|x| at.is_some_and(|a| (a - tr.position_from_point_x(*x)).abs() < GRAB_PX))
        };
        let resp = pu.response();
        self.hot = near(resp.hover_pos().map(|h| h.x), xs).is_some();
        if resp.drag_started() {
            self.active = near(press_x, xs);
        }
        if !resp.dragged() {
            self.active = None;
        }
        if let Some(i) = self.active
            && let Some(&(lo, hi)) = limits.get(i)
        {
            xs[i] = p.x.max(lo).min(hi.max(lo));
        }
    }
}

// ------------------------------------------------------------------ second axe Y

/// Échelle qui ramène une courbe de plage `[0, right_max]` sur l'axe de gauche de plage `[0, left_max]`
/// (egui_plot n'a pas de second axe Y : la 2e courbe est tracée multipliée par `k`, l'axe de droite divise).
pub fn right_axis_scale(left_max: f64, right_max: f64) -> f64 {
    if right_max > 0.0 { left_max / right_max } else { 1.0 }
}

// ------------------------------------------------------------------ décimation

/// Décimation min/max : pour chaque paquet de `chunk` points, garde le plus bas et le plus haut dans l'ordre
/// du temps. L'allure (pics compris) est conservée avec `2 / chunk` des points.
pub fn decimate_minmax(t: &[f64], y: &[f32], chunk: usize) -> Vec<[f64; 2]> {
    let n = t.len().min(y.len());
    let chunk = chunk.max(2);
    let mut out = Vec::with_capacity(2 * n.div_ceil(chunk));
    for start in (0..n).step_by(chunk) {
        let end = (start + chunk).min(n);
        let (mut imin, mut imax) = (start, start);
        for i in start..end {
            if y[i] < y[imin] {
                imin = i;
            }
            if y[i] > y[imax] {
                imax = i;
            }
        }
        let (a, b) = if imin <= imax { (imin, imax) } else { (imax, imin) };
        out.push([t[a], y[a] as f64]);
        if b != a {
            out.push([t[b], y[b] as f64]);
        }
    }
    out
}
