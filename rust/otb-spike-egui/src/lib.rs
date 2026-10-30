//! Prototype egui (E1) : un onglet par point à établir (D-035), sur un vrai log exporté
//! (`OTB_SPIKE_DIR`, fichiers écrits par le script d'export du répertoire de travail).

use std::path::{Path, PathBuf};
use std::time::Instant;

use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_plot::{
    AxisHints, GridInput, GridMark, HLine, HPlacement, HoverPosition, Line, Plot, PlotImage, PlotPoint, PlotPoints,
    Points, Span, VLine,
};

fn read_f32(p: &Path) -> Vec<f32> {
    let b = std::fs::read(p).unwrap_or_else(|e| panic!("{} : {e}", p.display()));
    b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

fn read_f64(p: &Path) -> Vec<f64> {
    let b = std::fs::read(p).unwrap_or_else(|e| panic!("{} : {e}", p.display()));
    b.as_chunks::<8>().0.iter().map(|c| f64::from_le_bytes(*c)).collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Spectre,
    Carte,
    Serie,
    Reperes,
    DeuxAxes,
    Texte,
}

const TABS: [(Tab, &str); 6] = [
    (Tab::Spectre, "Spectre (log + barres d'erreur)"),
    (Tab::Carte, "Carte fréquence × temps"),
    (Tab::Serie, "Série 1,2 M points"),
    (Tab::Reperes, "Repères déplaçables"),
    (Tab::DeuxAxes, "Second axe Y"),
    (Tab::Texte, "Texte riche + fichiers"),
];

pub struct Spike {
    pub tab: Tab,
    pub dark: bool,
    applied_dark: Option<bool>,
    n: usize,
    fs: f64,
    gyro: Vec<f32>,
    t: Vec<f64>,
    motor: Vec<f32>,
    spec: (Vec<f32>, Vec<f32>),
    err: (Vec<f32>, Vec<f32>, Vec<f32>),
    sg: Vec<f32>,
    sg_dims: (usize, usize, f64, f64, f64, f64),
    texture: Option<TextureHandle>,
    // série
    pub decimate: bool,
    pub last_ms: f64,
    // repères
    pub cutoff_hz: f64,
    pub region: (f64, f64),
    dragging: Option<u8>,
    hot: bool,
    // texte / fichiers
    md_cache: CommonMarkCache,
    picked: String,
    hovered_info: String,
}

fn meta_num(json: &str, key: &str) -> f64 {
    let k = format!("\"{key}\": ");
    let i = json.find(&k).unwrap_or_else(|| panic!("clé {key}")) + k.len();
    let j = json[i..].find([',', '}']).unwrap() + i;
    json[i..j].trim().parse().unwrap()
}

impl Spike {
    pub fn load(dir: PathBuf) -> Self {
        let meta = std::fs::read_to_string(dir.join("meta.json")).expect("meta.json");
        let m = |k| meta_num(&meta, k);
        let gyro = read_f32(&dir.join("gyro.f32"));
        let t = read_f64(&dir.join("t.f64"));
        let motor = read_f32(&dir.join("motor.f32"));
        let s = read_f32(&dir.join("spec.f32"));
        let ns = s.len() / 2;
        let e = read_f32(&dir.join("spec_err.f32"));
        let ne = e.len() / 3;
        Self {
            tab: Tab::Spectre,
            dark: true,
            applied_dark: None,
            n: gyro.len(),
            fs: m("fs"),
            gyro,
            t,
            motor,
            spec: (s[..ns].to_vec(), s[ns..].to_vec()),
            err: (e[..ne].to_vec(), e[ne..2 * ne].to_vec(), e[2 * ne..].to_vec()),
            sg: read_f32(&dir.join("sg.f32")),
            sg_dims: (m("nf") as usize, m("nt") as usize, m("t0"), m("t1"), m("f0"), m("f1")),
            texture: None,
            decimate: false,
            last_ms: 0.0,
            cutoff_hz: 150.0,
            region: (80.0, 200.0),
            dragging: None,
            hot: false,
            md_cache: CommonMarkCache::default(),
            picked: String::new(),
            hovered_info: String::new(),
        }
    }

    pub fn ui(&mut self, root: &mut egui::Ui) {
        if self.applied_dark != Some(self.dark) {
            root.ctx().set_visuals(if self.dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            root.ctx().request_repaint(); // le thème ne vaut qu'au cadre suivant
            self.applied_dark = Some(self.dark);
        }
        egui::Panel::top("tabs").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (tab, name) in TABS {
                    ui.selectable_value(&mut self.tab, tab, name);
                }
                ui.separator();
                ui.checkbox(&mut self.dark, "Thème sombre");
            });
        });
        egui::CentralPanel::default().show(root, |ui| match self.tab {
            Tab::Spectre => self.spectre(ui),
            Tab::Carte => self.carte(ui),
            Tab::Serie => self.serie(ui),
            Tab::Reperes => self.reperes(ui),
            Tab::DeuxAxes => self.deux_axes(ui),
            Tab::Texte => self.texte(ui),
        });
    }

    // ------------------------------------------------------------------ 1. axe log + barres d'erreur
    fn spectre(&mut self, ui: &mut egui::Ui) {
        ui.label(
            "Axe X logarithmique : tracé en log10(f), graduations 1-2-5 × 10^k et libellés en Hz refaits à la main.",
        );
        let (f, v) = &self.spec;
        let pts: Vec<[f64; 2]> = f
            .iter()
            .zip(v)
            .filter(|(f, _)| **f >= 1.0)
            .map(|(f, v)| [(*f as f64).log10(), *v as f64])
            .collect();
        let (ef, em, es) = &self.err;
        let mut bars: Vec<Vec<[f64; 2]>> = Vec::new();
        for i in (0..ef.len()).step_by(8).filter(|i| ef[*i] >= 1.0) {
            let x = (ef[i] as f64).log10();
            bars.push(vec![[x, (em[i] - es[i]) as f64], [x, (em[i] + es[i]) as f64]]);
        }
        let mean: Vec<[f64; 2]> = ef
            .iter()
            .zip(em)
            .filter(|(f, _)| **f >= 1.0)
            .map(|(f, m)| [(*f as f64).log10(), *m as f64])
            .collect();
        Plot::new("spectre")
            .x_axis_label("Fréquence (Hz)")
            .y_axis_label("DSP (dB)")
            .x_grid_spacer(log_grid)
            .x_axis_formatter(|m, _| format_hz(10f64.powf(m.value)))
            .label_formatter(|h| {
                let (n, p) = match h {
                    HoverPosition::NearDataPoint {
                        plot_name, position, ..
                    } => (*plot_name, position),
                    HoverPosition::Elsewhere { position } => ("", position),
                };
                Some(format!("{n}\n{:.1} Hz · {:.1} dB", 10f64.powf(p.x), p.y))
            })
            .show(ui, |pu| {
                pu.line(Line::new("Roll (Welch)", PlotPoints::from(pts)));
                pu.line(Line::new("moyenne 4 morceaux", PlotPoints::from(mean)).color(Color32::LIGHT_RED));
                for (i, b) in bars.into_iter().enumerate() {
                    pu.line(
                        Line::new(format!("±1σ {i}"), PlotPoints::from(b))
                            .color(Color32::LIGHT_RED)
                            .allow_hover(false),
                    );
                }
            });
    }

    // ------------------------------------------------------------------ 2. carte + barre de couleurs
    fn carte(&mut self, ui: &mut egui::Ui) {
        let (nf, nt, t0, t1, f0, f1) = self.sg_dims;
        if self.texture.is_none() {
            let (lo, hi) = range_pct(&self.sg, 0.02, 0.995);
            let mut px = vec![Color32::BLACK; nf * nt];
            // image dont la ligne 0 est la fréquence la plus haute (le haut de l'écran)
            for fi in 0..nf {
                for ti in 0..nt {
                    let v = self.sg[fi * nt + ti];
                    px[(nf - 1 - fi) * nt + ti] = if v.is_nan() {
                        Color32::TRANSPARENT
                    } else {
                        turbo(((v - lo) / (hi - lo)).clamp(0.0, 1.0))
                    };
                }
            }
            self.texture = Some(ui.ctx().load_texture(
                "carte",
                ColorImage {
                    size: [nt, nf],
                    source_size: egui::vec2(nt as f32, nf as f32),
                    pixels: px,
                },
                TextureOptions::NEAREST,
            ));
            self.hovered_info = format!("échelle {lo:.0} à {hi:.0} dB");
        }
        ui.label(format!(
            "PlotImage + barre de couleurs dessinée à côté ({}).",
            self.hovered_info
        ));
        let h = ui.available_height();
        ui.horizontal(|ui| {
            let tex = self.texture.as_ref().unwrap();
            let w = ui.available_width() - 70.0;
            Plot::new("carte")
                .width(w)
                .height(h)
                .x_axis_label("Temps (s)")
                .y_axis_label("Fréquence (Hz)")
                .show(ui, |pu| {
                    let c = PlotPoint::new((t0 + t1) / 2.0, (f0 + f1) / 2.0);
                    pu.image(PlotImage::new("DSP", tex, c, [(t1 - t0) as f32, (f1 - f0) as f32]));
                });
            let (r, _) = ui.allocate_exact_size(egui::vec2(24.0, h.min(400.0)), egui::Sense::hover());
            let p = ui.painter_at(r);
            for i in 0..64 {
                let y0 = r.top() + r.height() * i as f32 / 64.0;
                let c = turbo(1.0 - i as f32 / 63.0);
                p.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(r.left(), y0),
                        egui::pos2(r.right(), y0 + r.height() / 64.0 + 1.0),
                    ),
                    0.0,
                    c,
                );
            }
            ui.label("dB");
        });
    }

    // ------------------------------------------------------------------ 3. série de 10^6 points
    fn serie(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.decimate, "Décimation min/max (contournement)");
            ui.label(format!(
                "{} points ×3 = {} · tracé de ce cadre : {:.1} ms",
                self.n,
                self.n * 3,
                self.last_ms
            ));
        });
        let dur = self.t.last().copied().unwrap_or(0.0);
        let t0 = Instant::now();
        let reps = 3;
        let decimate = self.decimate;
        let (t, g) = (&self.t, &self.gyro);
        Plot::new("serie")
            .x_axis_label("Temps (s)")
            .y_axis_label("Gyro (°/s)")
            .show(ui, |pu| {
                let build = |lo: usize, hi: usize| -> Vec<[f64; 2]> {
                    let mut v = Vec::with_capacity((hi - lo) / if decimate { 100 } else { 1 });
                    if decimate {
                        for ch in (lo..hi).step_by(200) {
                            let e = (ch + 200).min(hi);
                            let (mut imin, mut imax) = (ch, ch);
                            for i in ch..e {
                                if g[i % g.len()] < g[imin % g.len()] {
                                    imin = i
                                }
                                if g[i % g.len()] > g[imax % g.len()] {
                                    imax = i
                                }
                            }
                            let (a, b) = if imin < imax { (imin, imax) } else { (imax, imin) };
                            v.push([t[a % t.len()] + dur * (a / t.len()) as f64, g[a % g.len()] as f64]);
                            v.push([t[b % t.len()] + dur * (b / t.len()) as f64, g[b % g.len()] as f64]);
                        }
                    } else {
                        for i in lo..hi {
                            v.push([t[i % t.len()] + dur * (i / t.len()) as f64, g[i % g.len()] as f64]);
                        }
                    }
                    v
                };
                pu.line(Line::new("Roll ×3", PlotPoints::from(build(0, g.len() * reps))));
            });
        self.last_ms = t0.elapsed().as_secs_f64() * 1000.0;
    }

    // ------------------------------------------------------------------ 4. repères et régions déplaçables
    fn reperes(&mut self, ui: &mut egui::Ui) {
        ui.label(format!(
            "Glisser la ligne verticale (fréquence de coupure) ou les bords de la région : coupure {:.0} Hz, région {:.0}–{:.0} Hz.",
            self.cutoff_hz, self.region.0, self.region.1
        ));
        let pts: Vec<[f64; 2]> = self
            .spec
            .0
            .iter()
            .zip(&self.spec.1)
            .map(|(f, v)| [*f as f64, *v as f64])
            .collect();
        let (mut cutoff, mut region, mut dragging) = (self.cutoff_hz, self.region, self.dragging);
        let busy = self.hot || dragging.is_some();
        let mut hot = false;
        let origin = ui.input(|i| i.pointer.press_origin()).map(|p| p.x);
        Plot::new("reperes")
            .allow_drag(!busy)
            .x_axis_label("Fréquence (Hz)")
            .show(ui, |pu| {
                pu.span(
                    Span::new("région", region.0..=region.1).fill(Color32::from_rgba_unmultiplied(80, 160, 255, 40)),
                );
                pu.line(Line::new("Roll", PlotPoints::from(pts)));
                pu.vline(VLine::new("coupure", cutoff).color(Color32::YELLOW).width(2.0));
                pu.vline(VLine::new("bord bas", region.0).color(Color32::LIGHT_BLUE));
                pu.vline(VLine::new("bord haut", region.1).color(Color32::LIGHT_BLUE));
                pu.hline(HLine::new("seuil", -20.0).color(Color32::LIGHT_GREEN));
                let (Some(p), tr) = (pu.pointer_coordinate(), *pu.transform()) else {
                    return;
                };
                let px = |x: f64| tr.position_from_point_x(x);
                let here = pu.response().hover_pos().map(|h| h.x);
                let find = |at: Option<f32>| {
                    [(0u8, cutoff), (1, region.0), (2, region.1)]
                        .into_iter()
                        .find(|(_, x)| at.is_some_and(|h| (h - px(*x)).abs() < 6.0))
                        .map(|(i, _)| i)
                };
                hot = find(here).is_some();
                if pu.response().drag_started() {
                    // egui annonce le début du glisser après un déplacement : la ligne se cherche là où le bouton a été pressé
                    dragging = find(origin);
                }
                if !pu.response().dragged() {
                    dragging = None;
                }
                match dragging {
                    Some(0) => cutoff = p.x.max(0.0),
                    Some(1) => region.0 = p.x.clamp(0.0, region.1),
                    Some(2) => region.1 = p.x.max(region.0),
                    _ => {}
                }
            });
        (self.cutoff_hz, self.region, self.dragging, self.hot) = (cutoff, region, dragging, hot);
    }

    // ------------------------------------------------------------------ 5. second axe Y
    fn deux_axes(&mut self, ui: &mut egui::Ui) {
        ui.label("Gyro sur l'axe de gauche (°/s), moteur 1 sur l'axe de droite (%), mis à l'échelle du premier.");
        let n = (self.n).min(200_000);
        let gmax = self.gyro[..n].iter().fold(1.0f32, |a, b| a.max(b.abs())) as f64;
        let k = gmax / 100.0; // 100 % <-> gmax
        let g: Vec<[f64; 2]> = (0..n).map(|i| [self.t[i], self.gyro[i] as f64]).collect();
        let m: Vec<[f64; 2]> = (0..n).map(|i| [self.t[i], self.motor[i] as f64 * k]).collect();
        let right = AxisHints::new_y()
            .label("Moteur 1 (%)")
            .placement(HPlacement::Right)
            .formatter(move |mk, _| format!("{:.0}", mk.value / k));
        Plot::new("axes")
            .custom_y_axes(vec![AxisHints::new_y().label("Gyro (°/s)"), right])
            .x_axis_label("Temps (s)")
            .show(ui, |pu| {
                pu.line(Line::new("Gyro Roll", PlotPoints::from(g)));
                pu.line(Line::new("Moteur 1", PlotPoints::from(m)).color(Color32::ORANGE));
                pu.points(Points::new("", PlotPoints::from(vec![[0.0, 0.0]])).radius(0.0));
            });
    }

    // ------------------------------------------------------------------ 6. texte riche + fenêtres de fichiers
    fn texte(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Ouvrir un log…").clicked() {
                self.picked = match rfd::FileDialog::new()
                    .add_filter("Blackbox", &["bbl", "bfl"])
                    .pick_file()
                {
                    Some(p) => p.display().to_string(),
                    None => "(annulé)".into(),
                };
            }
            ui.label(&self.picked);
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            CommonMarkViewer::new().show(
                ui,
                &mut self.md_cache,
                "# Guide intégré\n\nUn **titre**, du *gras*, du `code` et une liste :\n\n\
                 - étape 1 : régler le filtre gyro à **250 Hz**\n- étape 2 : voir [le configurateur](https://app.betaflight.com)\n\n\
                 > Citation : en vol stationnaire, on identifie le bruit.\n\n\
                 | axe | P | D |\n|---|---|---|\n| Roll | 48 | 36 |\n| Pitch | 52 | 40 |\n\n1. un\n2. deux\n",
            );
        });
        let _ = self.fs;
    }
}

fn format_hz(f: f64) -> String {
    if f >= 1000.0 {
        format!("{} k", f / 1000.0)
    } else {
        format!("{f}")
    }
}

fn log_grid(input: GridInput) -> Vec<GridMark> {
    let (lo, hi) = input.bounds;
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

fn range_pct(v: &[f32], a: f32, b: f32) -> (f32, f32) {
    let mut s: Vec<f32> = v.iter().copied().filter(|x| !x.is_nan()).collect();
    s.sort_by(|x, y| x.partial_cmp(y).unwrap());
    (
        s[(s.len() as f32 * a) as usize],
        s[((s.len() as f32 * b) as usize).min(s.len() - 1)],
    )
}

fn turbo(t: f32) -> Color32 {
    // approximation polynomiale de la palette « turbo » (Google, licence Apache-2.0)
    let t = t as f64;
    let r = 0.13572138 + t * (4.6153926 + t * (-42.66032 + t * (132.13108 + t * (-152.9424 + t * 59.28638))));
    let g = 0.09140261 + t * (2.19418 + t * (4.84296 + t * (-14.185033 + t * (4.27805 + t * 2.829566))));
    let b = 0.1066733 + t * (12.64194 + t * (-60.58204 + t * (110.36277 + t * (-89.90310 + t * 27.34824))));
    let c = |x: f64| (x.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgb(c(r), c(g), c(b))
}

pub struct App(pub Spike);

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.0.ui(ui);
    }
}
