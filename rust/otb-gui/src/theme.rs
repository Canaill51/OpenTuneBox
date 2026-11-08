//! Couleurs et thèmes de l'interface (port de `gui/theme.py`, palette inspirée de PIDtoolbox).
//!
//! ⚠️ Toute couleur de l'interface vit dans ce fichier : une couleur en dur ailleurs casse l'un des deux thèmes
//! (L-016). `tests/contracts.rs` refuse tout code couleur dans les autres fichiers de `src/`.

use egui::{Color32, Context, Visuals};

/// `0xrrggbb` → couleur opaque.
const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Couleurs des fichiers comparés (jusqu'à 10), dans l'ordre de PIDtoolbox.
pub const FILE_COLORS: [Color32; 10] = [
    hex(0xe6194b),
    hex(0xf58231),
    hex(0xe6c619),
    hex(0x7ccd3a),
    hex(0x2ec4c4),
    hex(0x2f6fd6),
    hex(0x1c2f8c),
    hex(0x9b4fd1),
    hex(0xd65fa6),
    hex(0x7f7f7f),
];

/// Moteurs 1/5, 2/6, 3/7, 4/8.
pub const MOTOR_COLORS: [Color32; 4] = [hex(0xe6194b), hex(0xf58231), hex(0x2f6fd6), hex(0x2ec4c4)];

pub const AXIS_NAMES: [&str; 3] = ["Roll", "Pitch", "Yaw"];

/// La même couleur avec une opacité de `alpha` (0 à 255) : remplissages translucides posés sur un graphique.
pub fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

pub fn file_color(i: usize) -> Color32 {
    FILE_COLORS[i % FILE_COLORS.len()]
}

pub fn motor_color(i: usize) -> Color32 {
    MOTOR_COLORS[i % MOTOR_COLORS.len()]
}

/// Couleurs des panneaux (clés de `ui` dans `theme.py`).
#[derive(Clone, Copy, Debug)]
pub struct Ui {
    pub window: Color32,
    pub base: Color32,
    pub alt: Color32,
    pub button: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub disabled: Color32,
    pub highlight: Color32,
    pub tooltip: Color32,
    pub link: Color32,
    pub hover: Color32,
    pub light: Color32,
    pub mid: Color32,
    pub dark: Color32,
}

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub name: &'static str,
    pub dark: bool,
    /// Fond des graphiques.
    pub bg: Color32,
    /// Texte et axes des graphiques.
    pub fg: Color32,
    /// Opacité de la grille (0 à 1).
    pub grid: f32,
    /// Zones hors période.
    pub shade: Color32,
    /// Fond translucide des légendes posées sur un graphique.
    pub legend_fill: Color32,
    pub ui: Ui,
}

pub const CLAIR: Theme = Theme {
    name: "clair",
    dark: false,
    bg: hex(0xffffff),
    fg: hex(0x1b1b1b),
    grid: 0.22,
    shade: Color32::from_rgba_unmultiplied_const(128, 128, 128, 90),
    legend_fill: Color32::from_rgba_unmultiplied_const(255, 255, 255, 190),
    ui: Ui {
        window: hex(0xf3f3f3),
        base: hex(0xffffff),
        alt: hex(0xf5f7fa),
        button: hex(0xfbfbfb),
        text: hex(0x1b1b1b),
        muted: hex(0x5c5c5c),
        disabled: hex(0x9a9a9a),
        highlight: hex(0x1f57c3),
        tooltip: hex(0xffffe8),
        link: hex(0x2b6be0),
        hover: hex(0xe4e7ec),
        light: hex(0xffffff),
        mid: hex(0xc4c4c4),
        dark: hex(0xa0a0a0),
    },
};

pub const SOMBRE: Theme = Theme {
    name: "sombre",
    dark: true,
    bg: hex(0x1e1f22),
    fg: hex(0xe2e2e2),
    grid: 0.18,
    shade: Color32::from_rgba_unmultiplied_const(0, 0, 0, 110),
    legend_fill: Color32::from_rgba_unmultiplied_const(30, 31, 34, 190),
    ui: Ui {
        window: hex(0x2b2d31),
        base: hex(0x1e1f22),
        alt: hex(0x26282c),
        button: hex(0x3a3d43),
        text: hex(0xe8e8e8),
        muted: hex(0xa8a8a8),
        disabled: hex(0x6e7177),
        highlight: hex(0x3d7be0),
        tooltip: hex(0x3a3d43),
        link: hex(0x5a92f0),
        hover: hex(0x3a3d43),
        light: hex(0x4a4d54),
        mid: hex(0x55585f),
        dark: hex(0x18191c),
    },
};

pub const THEMES: [Theme; 2] = [CLAIR, SOMBRE];

/// Thème de nom donné (le clair si le nom est inconnu, comme `THEMES.get(name, clair)`).
pub fn by_name(name: &str) -> Theme {
    THEMES.iter().copied().find(|t| t.name == name).unwrap_or(CLAIR)
}

/// Couleurs des textes mis en valeur (boutons d'outils…), choisies pour rester lisibles sur chaque fond
/// (contraste ≥ 4,5:1) : (thème clair, thème sombre).
pub const ACCENTS: [(&str, Color32, Color32); 7] = [
    ("green", hex(0x137a2c), hex(0x5fd47c)),
    ("red", hex(0xb3261e), hex(0xff7b70)),
    ("blue", hex(0x1f57c3), hex(0x82acff)),
    ("orange", hex(0x9a5300), hex(0xffb54d)),
    ("purple", hex(0x7432a0), hex(0xcf9df5)),
    ("teal", hex(0x00756f), hex(0x4fd1c5)),
    ("pink", hex(0xb0306a), hex(0xff8cc6)),
];

impl Theme {
    /// Couleur d'accent de ce thème. Panique sur un nom inconnu (faute de frappe du code, pas de l'utilisateur).
    pub fn accent(&self, name: &str) -> Color32 {
        let (_, light, dark) = ACCENTS
            .iter()
            .find(|(n, ..)| *n == name)
            .unwrap_or_else(|| panic!("accent inconnu : {name}"));
        if self.dark { *dark } else { *light }
    }

    /// Fond translucide de la période d'analyse (zone claire déplaçable des graphiques).
    pub fn region_fill(&self) -> Color32 {
        self.ui.highlight.gamma_multiply(0.12)
    }

    /// Couleur d'un trait de la grille (le texte, adouci par `grid`).
    pub fn grid_color(&self) -> Color32 {
        self.fg.gamma_multiply(self.grid)
    }

    /// Couleur d'une trace du visualiseur ; le noir devient quasi blanc sur le thème sombre.
    pub fn trace_color(&self, key: &str) -> Color32 {
        let c = TRACE_COLORS
            .iter()
            .find(|(k, _)| *k == key)
            .map_or(hex(0x444444), |(_, c)| *c);
        if self.dark && c == hex(0x111111) {
            hex(0xf0f0f0)
        } else {
            c
        }
    }
}

/// Couleurs des traces du visualiseur.
pub const TRACE_COLORS: [(&str, Color32); 12] = [
    ("gyro_prefilt", hex(0x9a9a9a)),
    ("gyro", hex(0x111111)),
    ("pterm", hex(0x2ca02c)),
    ("iterm", hex(0x1f5fd6)),
    ("dterm_prefilt", hex(0x8c8c1a)),
    ("dterm", hex(0xe0b000)),
    ("fterm", hex(0xa0522d)),
    ("setpoint", hex(0xe6194b)),
    ("pid_sum", hex(0x8e44ad)),
    ("pid_error", hex(0x17becf)),
    ("throttle", hex(0x111111)),
    ("debug", hex(0xff7f0e)),
];

/// Couleurs fixes des graphiques : courbes, repères et éléments posés sur une image ou un fond coloré, lisibles
/// sur les deux fonds de tracé (blanc et #1e1f22). Jamais pour un texte sur le fond du panneau : là, `accent`
/// ou `Theme::ui`.
pub mod plot {
    use super::{Color32, hex};

    /// Courbe de référence : gyro avant filtres, notchs.
    pub const NEUTRAL: Color32 = hex(0x9a9a9a);
    /// Simulateur de filtres : réglages modifiés.
    pub const MODIFIED: Color32 = hex(0x2f6fd6);
    /// Simulateur de filtres : réglages actuels simulés.
    pub const SIM_CURRENT: Color32 = hex(0xf58231);
    /// Seuil d'avertissement (pic de sensibilité 4 dB).
    pub const LIMIT_WARN: Color32 = hex(0xe6c619);
    /// Seuil d'alerte (6 dB).
    pub const LIMIT_ALERT: Color32 = hex(0xe6194b);
    /// Traits et textes posés sur un spectrogramme.
    pub const OVERLAY: Color32 = hex(0xffffff);
    /// Curseur du Fréquence × temps survolé.
    pub const OVERLAY_HOVER: Color32 = hex(0x00e0ff);
    /// Fréquence des notchs suivie sur le Fréquence × temps (rouge translucide).
    pub const NOTCH_TRACE: Color32 = Color32::from_rgba_unmultiplied_const(255, 32, 32, 210);
    /// Fond translucide d'un texte posé sur un spectrogramme.
    pub const OVERLAY_FILL: Color32 = Color32::from_rgba_unmultiplied_const(0, 0, 0, 110);
    /// Surlignage d'une ligne (Configuration : réglage qui diffère).
    pub const ROW_HIGHLIGHT: Color32 = Color32::from_rgba_unmultiplied_const(255, 200, 0, 90);
    /// Fond des cartes de fréquence là où il n'y a pas de données : gris neutre, distinct du noir (faible) et du
    /// blanc (saturé) des palettes (`MAP_BACKGROUND` de Python).
    pub const MAP_BACKGROUND: Color32 = hex(0x808080);
    /// Texte ou icône sur un fond coloré (bouton principal, badges).
    pub const ON_ACCENT: Color32 = hex(0xffffff);
}

/// Badges de gravité de l'assistant (clés = `advisor::rules`) : fond coloré, texte `plot::ON_ACCENT`, mêmes
/// couleurs dans les deux thèmes.
pub const SEVERITY_COLORS: [(&str, Color32); 5] = [
    ("alerte", hex(0xc0392b)),
    ("attention", hex(0xd35400)),
    ("conseil", hex(0x2f6fd6)),
    ("info", hex(0x7f8c8d)),
    ("ok", hex(0x1a9a3a)),
];

pub fn severity_color(key: &str) -> Color32 {
    SEVERITY_COLORS
        .iter()
        .find(|(k, _)| *k == key)
        .map_or(hex(0x7f8c8d), |(_, c)| *c)
}

/// Palette « turbo » (approximation polynomiale de Google, licence Apache-2.0), `t` de 0 à 1.
pub fn turbo(t: f32) -> Color32 {
    let t = t as f64;
    let r = 0.13572138 + t * (4.6153926 + t * (-42.66032 + t * (132.13108 + t * (-152.9424 + t * 59.28638))));
    let g = 0.09140261 + t * (2.19418 + t * (4.84296 + t * (-14.185033 + t * (4.27805 + t * 2.829566))));
    let b = 0.1066733 + t * (12.64194 + t * (-60.58204 + t * (110.36277 + t * (-89.90310 + t * 27.34824))));
    let c = |x: f64| (x.clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgb(c(r), c(g), c(b))
}

/// Palettes des cartes de fréquence (`COLORMAP_NAMES` de Python, « hot » par défaut comme PIDtoolbox). Tables de
/// `colormaps.rs`, générées depuis celles de l'interface Qt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Palette {
    Hot,
    Inferno,
    Viridis,
    Turbo,
    Jet,
}

impl Palette {
    pub const ALL: [Palette; 5] = [
        Palette::Hot,
        Palette::Inferno,
        Palette::Viridis,
        Palette::Turbo,
        Palette::Jet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Palette::Hot => "hot",
            Palette::Inferno => "inferno",
            Palette::Viridis => "viridis",
            Palette::Turbo => "turbo",
            Palette::Jet => "jet",
        }
    }

    fn table(self) -> &'static [[u8; 3]; 256] {
        match self {
            Palette::Hot => &crate::colormaps::HOT,
            Palette::Inferno => &crate::colormaps::INFERNO,
            Palette::Viridis => &crate::colormaps::VIRIDIS,
            Palette::Turbo => &crate::colormaps::TURBO,
            Palette::Jet => &crate::colormaps::JET,
        }
    }

    /// Couleur de la valeur normalisée `t` (bornée à 0..1 ; NaN → début de la palette).
    pub fn color(self, t: f32) -> Color32 {
        let [r, g, b] = self.table()[(t.clamp(0.0, 1.0) * 255.0).round() as usize];
        Color32::from_rgb(r, g, b)
    }
}

/// Taille du texte un peu plus grande que celle d'egui (`FONT_SCALE` de Python : 1,15).
pub const FONT_SCALE: f32 = 1.15;

/// Nombre de cadres à laisser passer avant d'annoncer le thème à la fenêtre du système : au premier cadre, Windows vient de
/// créer la fenêtre et ne repeint pas son contour (L-091).
pub const WINDOW_THEME_DELAY: u32 = 3;

/// Thème à annoncer à la fenêtre du système (barre de titre et bordures) pour un thème d'OpenTuneBox.
pub fn window_theme(theme: &Theme) -> egui::SystemTheme {
    if theme.dark {
        egui::SystemTheme::Dark
    } else {
        egui::SystemTheme::Light
    }
}

/// Faut-il annoncer le thème à la fenêtre sur ce cadre ? Oui une fois la fenêtre posée, puis à chaque changement de thème.
///
/// ⚠️ Annoncer trop tôt ne sert à rien (L-091) : c'est tout l'intérêt de `WINDOW_THEME_DELAY`.
pub fn should_send_window_theme(frames: u32, sent: Option<&str>, wanted: &str) -> bool {
    frames >= WINDOW_THEME_DELAY && sent != Some(wanted)
}

/// Applique un thème à egui (couleurs des panneaux, des boutons, de la sélection).
pub fn apply(ctx: &Context, theme: &Theme) {
    let u = &theme.ui;
    let mut v = if theme.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = u.window;
    v.window_fill = u.window;
    v.extreme_bg_color = u.base;
    v.faint_bg_color = u.alt;
    v.code_bg_color = u.alt;
    v.hyperlink_color = u.link;
    v.selection.bg_fill = u.highlight;
    v.selection.stroke.color = plot::ON_ACCENT;
    v.window_stroke.color = u.mid;
    v.widgets.noninteractive.bg_fill = u.window;
    v.widgets.noninteractive.bg_stroke.color = u.mid;
    v.widgets.noninteractive.fg_stroke.color = u.text;
    v.widgets.inactive.bg_fill = u.button;
    v.widgets.inactive.weak_bg_fill = u.button;
    // contour des cases à cocher et des boutons radio : assez sombre pour se voir (piège de Fusion sous Qt, L-014)
    v.widgets.inactive.bg_stroke.color = u.muted;
    v.widgets.inactive.bg_stroke.width = 1.0;
    v.widgets.inactive.fg_stroke.color = u.text;
    v.widgets.hovered.bg_fill = u.hover;
    v.widgets.hovered.weak_bg_fill = u.hover;
    v.widgets.hovered.bg_stroke.color = u.highlight;
    v.widgets.hovered.fg_stroke.color = u.text;
    v.widgets.active.bg_fill = u.mid;
    v.widgets.active.weak_bg_fill = u.mid;
    v.widgets.active.bg_stroke.color = u.highlight;
    v.widgets.active.fg_stroke.color = u.text;
    v.widgets.open.bg_fill = u.hover;
    v.widgets.open.weak_bg_fill = u.hover;
    // ⚠️ egui synchronise seul la barre de titre de la fenêtre (`Options::sync_window_theme`) à la fin du cadre où le thème
    // change, donc au tout **premier** cadre : sous Windows la fenêtre vient d'être créée et ne repeint pas son contour, la
    // barre de titre reste sombre et le nom du programme devient illisible. egui retient ce qu'il a envoyé et ne réessaie
    // jamais. On lui retire donc ce travail : `app.rs` envoie `SetTheme` lui-même une fois la fenêtre posée (L-091).
    ctx.options_mut(|o| o.sync_window_theme = false);
    // préférence fixée : sans cela egui suit le thème du système et abandonne notre palette quand il change
    ctx.set_theme(if theme.dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    ctx.set_visuals_of(
        if theme.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        },
        v,
    );
    let defaults = egui::Style::default().text_styles;
    ctx.all_styles_mut(|s| {
        s.text_styles = defaults
            .iter()
            .map(|(k, f)| {
                (
                    k.clone(),
                    egui::FontId {
                        size: f.size * FONT_SCALE,
                        ..f.clone()
                    },
                )
            })
            .collect();
    });
}
