//! Les contournements d'egui_plot trouvés par le prototype E1 (D-035), écrits une seule fois pour tous les outils :
//! axe logarithmique, barres d'erreur, carte avec barre de couleurs, lignes et régions déplaçables, second axe Y,
//! et la décimation min/max des longues séries.

use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use egui_plot::{GridInput, GridMark};

use crate::theme::Palette;

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
/// `hi`. Les NaN prennent la couleur `empty` (transparent, ou le gris de fond des cartes de fréquence). L'image est retournée : sa première ligne est le haut de l'écran.
pub fn heatmap_pixels(
    values: &[f32],
    nf: usize,
    nt: usize,
    lo: f32,
    hi: f32,
    palette: Palette,
    empty: Color32,
) -> Vec<Color32> {
    assert_eq!(
        values.len(),
        nf * nt,
        "carte de {} valeurs pour {nf} × {nt}",
        values.len()
    );
    let span = if hi > lo { hi - lo } else { 1.0 };
    let mut pixels = vec![empty; nf * nt];
    for fi in 0..nf {
        for ti in 0..nt {
            let v = values[fi * nt + ti];
            if !v.is_nan() {
                pixels[(nf - 1 - fi) * nt + ti] = palette.color(((v - lo) / span).clamp(0.0, 1.0));
            }
        }
    }
    pixels
}

/// Texture d'une carte (voir `heatmap_pixels`), à afficher avec `PlotImage` : `Heatmap` d'egui_plot coûte 30 ms par
/// cadre à 10⁵ cases (mesuré en 6-0, docs/migration-rust.md).
// Fonction de dessin : chaque argument est une grandeur de l'écran (bornes, échelle, couleurs). Les réunir en
// structure n'ajouterait qu'un type intermédiaire, utilisé une seule fois.
#[allow(clippy::too_many_arguments)]
pub fn heatmap_texture(
    ctx: &Context,
    name: &str,
    values: &[f32],
    nf: usize,
    nt: usize,
    lo: f32,
    hi: f32,
    palette: Palette,
    empty: Color32,
) -> TextureHandle {
    let pixels = heatmap_pixels(values, nf, nt, lo, hi, palette, empty);
    let image = ColorImage {
        size: [nt, nf],
        source_size: egui::vec2(nt as f32, nf as f32),
        pixels,
    };
    ctx.load_texture(name, image, TextureOptions::NEAREST)
}

/// Graduations « rondes » de `lo` à `hi` : `n` intervalles au plus, pas de 1, 2 ou 5 × 10^k (barre de couleurs).
pub fn colorbar_ticks(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    if !lo.is_finite() || !hi.is_finite() || hi <= lo || n == 0 {
        return Vec::new();
    }
    let raw = (hi - lo) / n as f64;
    let e = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * e)
        .find(|s| *s >= raw * (1.0 - 1e-9))
        .unwrap_or(10.0 * e);
    let first = (lo / step - 1e-9).ceil();
    (0..)
        .map(|k| (first + k as f64) * step)
        .take_while(|v| *v <= hi * (1.0 + 1e-12) + 1e-12 * step)
        .collect()
}

/// Barre de couleurs graduée entre `lo` et `hi`, avec un titre. Verticale (haut = valeur la plus forte, `length` points
/// de haut) ou horizontale (`length` points de large).
pub fn colorbar(ui: &mut egui::Ui, palette: Palette, lo: f64, hi: f64, length: f32, vertical: bool, title: &str) {
    const STEPS: usize = 64;
    const THICK: f32 = 14.0;
    let text = ui.visuals().text_color();
    let font = egui::FontId::proportional(11.0);
    let label_room = if vertical { 44.0 } else { 16.0 };
    let size = if vertical {
        egui::vec2(THICK + label_room, length)
    } else {
        egui::vec2(length, THICK + label_room)
    };
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    // marge aux extrémités : les étiquettes des bornes sont centrées sur elles et ne doivent pas être coupées
    let pad = 14.0;
    let bar = if vertical {
        egui::Rect::from_min_size(
            rect.min + egui::vec2(0.0, pad / 2.0),
            egui::vec2(THICK, (length - pad).max(1.0)),
        )
    } else {
        egui::Rect::from_min_size(
            rect.min + egui::vec2(pad, 0.0),
            egui::vec2((length - 2.0 * pad).max(1.0), THICK),
        )
    };
    for i in 0..STEPS {
        let t = i as f32 / (STEPS - 1) as f32;
        let (a, b) = (i as f32 / STEPS as f32, (i + 1) as f32 / STEPS as f32);
        let band = if vertical {
            egui::Rect::from_min_max(
                egui::pos2(bar.left(), bar.bottom() - bar.height() * b),
                egui::pos2(bar.right(), bar.bottom() - bar.height() * a + 1.0),
            )
        } else {
            egui::Rect::from_min_max(
                egui::pos2(bar.left() + bar.width() * a, bar.top()),
                egui::pos2(bar.left() + bar.width() * b + 1.0, bar.bottom()),
            )
        };
        painter.rect_filled(band, 0.0, palette.color(t));
    }
    let span = hi - lo;
    for v in colorbar_ticks(lo, hi, if length > 160.0 { 5 } else { 3 }) {
        let f = ((v - lo) / span) as f32;
        let label = otb_format::general(v, 6).replace('.', ",").replace('-', "−");
        if vertical {
            let y = bar.bottom() - bar.height() * f;
            painter.text(
                egui::pos2(bar.right() + 4.0, y),
                egui::Align2::LEFT_CENTER,
                label,
                font.clone(),
                text,
            );
        } else {
            let x = bar.left() + bar.width() * f;
            painter.text(
                egui::pos2(x, bar.bottom() + 2.0),
                egui::Align2::CENTER_TOP,
                label,
                font.clone(),
                text,
            );
        }
    }
    if !title.is_empty() {
        ui.label(title);
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

// La décimation de la fenêtre visible a déménagé dans `otb_dsp::minmax` (D-048) : `window_points` y répond par un
// index multi-résolution, en O(points rendus) au lieu de O(échantillons visibles), et ses tests comparent sa sortie,
// point pour point, à celle de l'ancienne `visible_points` (`rust/otb-dsp/tests/minmax.rs`).

/// Petit trait de couleur devant un libellé (légende des traces), dessiné : les polices d'egui n'ont pas « ━ ».
pub fn swatch(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 12.0), egui::Sense::hover());
    let bar = egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 4.0));
    ui.painter().rect_filled(bar, 1.0, color);
}

// ------------------------------------------------------------------ texte ancré dans un coin

/// Rectangle et position de chaque ligne d'un bloc de texte ancré dans un coin de `frame` (`CornerText` de Python :
/// indépendant du zoom). `sizes` : taille de chaque ligne ; les lignes sont alignées du côté du coin.
pub fn corner_layout(
    frame: egui::Rect,
    corner: egui::Align2,
    sizes: &[egui::Vec2],
    margin: f32,
) -> (egui::Rect, Vec<egui::Pos2>) {
    let w = sizes.iter().map(|s| s.x).fold(0.0, f32::max);
    let h: f32 = sizes.iter().map(|s| s.y).sum();
    let size = egui::vec2(w, h);
    let min = egui::pos2(
        match corner.x() {
            egui::Align::Min => frame.left() + margin,
            egui::Align::Center => frame.center().x - w / 2.0,
            egui::Align::Max => frame.right() - margin - w,
        },
        match corner.y() {
            egui::Align::Min => frame.top() + margin,
            egui::Align::Center => frame.center().y - h / 2.0,
            egui::Align::Max => frame.bottom() - margin - h,
        },
    );
    let mut y = min.y;
    let mut pos = Vec::with_capacity(sizes.len());
    for s in sizes {
        let x = match corner.x() {
            egui::Align::Min => min.x,
            egui::Align::Center => min.x + (w - s.x) / 2.0,
            egui::Align::Max => min.x + w - s.x,
        };
        pos.push(egui::pos2(x, y));
        y += s.y;
    }
    (egui::Rect::from_min_size(min, size), pos)
}

/// Dessine des lignes de texte colorées dans un coin du cadre d'un tracé, sur un fond translucide `fill`.
pub fn corner_text(ui: &egui::Ui, frame: egui::Rect, corner: egui::Align2, lines: &[(String, Color32)], fill: Color32) {
    if lines.is_empty() {
        return;
    }
    let painter = ui.painter().with_clip_rect(frame);
    let font = egui::FontId::proportional(12.5);
    let galleys: Vec<_> = lines
        .iter()
        .map(|(t, c)| painter.layout_no_wrap(t.clone(), font.clone(), *c))
        .collect();
    let sizes: Vec<egui::Vec2> = galleys.iter().map(|g| g.size()).collect();
    let (rect, pos) = corner_layout(frame, corner, &sizes, 6.0);
    painter.rect_filled(rect.expand(3.0), 3.0, fill);
    for ((g, p), (_, c)) in galleys.into_iter().zip(pos).zip(lines) {
        painter.galley(p, g, *c);
    }
}

// ------------------------------------------------------------------ grille linéaire

fn nice_up(x: f64) -> f64 {
    let e = 10f64.powf(x.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * e)
        .find(|v| *v >= x * (1.0 - 1e-9))
        .unwrap_or(10.0 * e)
}

/// Grille linéaire à repères ronds (1, 2 ou 5 × 10^k). La grille par défaut d'egui_plot ne marque que les puissances de
/// 10 : sur 0..100 elle ne donne que 0 et 100, et les axes restent sans graduation (egui_plot n'étiquette un repère que
/// si deux repères de même rang sont écartés de 60 px sur X, 20 px sur Y). Ici, un repère fin tous les ≥ 8 px et un repère
/// étiqueté tous les ≥ `8 × label_factor` px : `label_factor` = 8 sur un axe X (≥ 64 px), 3 sur un axe Y (≥ 24 px).
pub fn nice_grid(input: GridInput, label_factor: f64) -> Vec<GridMark> {
    let (lo, hi) = input.bounds;
    let base = input.base_step_size;
    if !lo.is_finite() || !hi.is_finite() || !base.is_finite() || base <= 0.0 || hi <= lo {
        return Vec::new();
    }
    let fine = nice_up(base);
    let coarse = nice_up(base * label_factor).max(fine);
    let (k0, k1) = ((lo / fine - 1e-9).ceil(), (hi / fine + 1e-9).floor());
    if k1 - k0 > 4000.0 {
        return Vec::new();
    }
    (k0 as i64..=k1 as i64)
        .map(|k| {
            let v = k as f64 * fine;
            let on_coarse = ((v / coarse).round() * coarse - v).abs() < 1e-9 * coarse;
            GridMark {
                value: v,
                step_size: if on_coarse { coarse } else { fine },
            }
        })
        .collect()
}

/// Un tracé avec la grille à repères ronds sur les deux axes (`nice_grid`).
pub fn nice_axes(plot: egui_plot::Plot<'_>) -> egui_plot::Plot<'_> {
    plot.x_grid_spacer(|i| nice_grid(i, 8.0))
        .y_grid_spacer(|i| nice_grid(i, 3.0))
}
