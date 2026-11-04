//! Calculs du Simulateur de filtres (port de `gui/tools/filter_sim.py`, partie sans Qt) : spectres « et si », commandes CLI,
//! éditeur de réglages, courbes de Bode et retards des filtres. Aucune dépendance à egui : tout est testable sans fenêtre.

use std::collections::BTreeMap;
use std::ops::Range;

use otb_analysis::bf_filters::{
    Chain, Target, WhatIfCache, chain_delay_ms, chain_response, dterm_chain, dyn_lpf_cutoff, gyro_chain,
    whatif_spectrum,
};
use otb_analysis::npstat::unwrap;
use otb_analysis::spectral::{Mode, spectrum, to_ptb_db};
use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::{Lowpass, Settings};

use crate::tools::common::wide;

/// Types de passe-bas proposés (indice = code de la liste déroulante).
pub const LPF_TYPES: [&str; 4] = ["PT1", "BIQUAD", "PT2", "PT3"];
/// Modes d'un LPF1 (indice = code de la liste déroulante).
pub const LPF_MODES: [&str; 3] = ["désactivé", "statique", "dynamique"];
/// Fréquences (Hz) où l'on donne les retards.
pub const DELAY_FREQS: [f64; 3] = [20.0, 50.0, 100.0];
/// Clés des spectres de `FilterSimResult`.
pub const SPECTRA_KEYS: [&str; 8] = [
    "gyro_pf",
    "gyro",
    "gyro_sim",
    "gyro_sim_cur",
    "d_pf",
    "d",
    "d_sim",
    "d_sim_cur",
];

// ---------------------------------------------------------------- bornes et défauts de l'interface Python

/// Coupures, centres et largeurs de notch (Hz) : `QSpinBox` de 0 à 1000.
pub const HZ_RANGE: (i64, i64) = (0, 1000);
pub const EXPO_RANGE: (i64, i64) = (0, 10);
/// Nombre de notchs dynamiques : 0 à 7 (5 au plus en 4.4).
pub const DYN_COUNT_MAX: i64 = 7;
pub const DYN_COUNT_MAX_44: i64 = 5;
pub const DYN_Q_RANGE: (i64, i64) = (1, 1000);
pub const DYN_MIN_RANGE: (i64, i64) = (20, 250);
pub const DYN_MAX_RANGE: (i64, i64) = (200, 1000);
/// Centres du notch dynamique quand l'en-tête n'en donne pas (`or 100`, `or 600`).
pub const DYN_MIN_DEFAULT: i64 = 100;
pub const DYN_MAX_DEFAULT: i64 = 600;
pub const RPM_HARM_RANGE: (i64, i64) = (0, 3);
pub const RPM_Q_RANGE: (i64, i64) = (250, 3000);
pub const RPM_MIN_RANGE: (i64, i64) = (30, 200);
pub const RPM_WEIGHT_RANGE: (i64, i64) = (0, 100);
/// Gaz du Bode (%) : curseur de 0 à 100, 50 au départ.
pub const THROTTLE_RANGE: (i64, i64) = (0, 100);
pub const THROTTLE_DEFAULT: i64 = 50;
/// Fréquence moteur du Bode (Hz) : de 20 à 3000, 200 sans mesure.
pub const MOTOR_HZ_RANGE: (i64, i64) = (20, 3000);
pub const MOTOR_HZ_DEFAULT: i64 = 200;

// ---------------------------------------------------------------- spectres

/// Spectres enregistrés et « et si » (dB, échelle PIDtoolbox) d'un log.
#[derive(Debug, Clone)]
pub struct FilterSimResult {
    /// Identifiant du log calculé (`LoadedLog::uid`, L-072), mis par l'outil ; `simulate_filters` laisse 0.
    pub log_id: u64,
    pub fs: f64,
    pub base: Settings,
    pub modf: Settings,
    pub freqs: Vec<f64>,
    /// Par clé de `SPECTRA_KEYS` : trois axes, `None` quand le spectre n'existe pas.
    pub spectra: BTreeMap<&'static str, [Option<Vec<f64>>; 3]>,
    /// 1,0 par axe, NaN quand le terme D de l'axe est nul.
    pub d_scale: Vec<f64>,
    pub has_unfilt: bool,
}

impl FilterSimResult {
    pub fn spectrum(&self, key: &str, axis: usize) -> Option<&[f64]> {
        self.spectra.get(key)?.get(axis)?.as_deref()
    }
}

/// `simulate_filters` : spectres enregistrés et spectres « et si » avec les filtres `modf`, sur la période `sel`.
///
/// Méthode relative (`whatif_spectrum`) : spectre réellement enregistré × rapport des réponses des chaînes de filtres (modifiée /
/// actuelle), évaluées à la vraie cadence PID fenêtre par fenêtre. Réglages inchangés : on retombe exactement sur le log.
pub fn simulate_filters(
    log: &FlightLog,
    sel: Range<usize>,
    base: &Settings,
    modf: &Settings,
    with_current: bool,
) -> Result<FilterSimResult, String> {
    let fs = log.fs;
    let has_unfilt = log.gyro_unfilt.is_some();
    let mut spectra: BTreeMap<&'static str, [Option<Vec<f64>>; 3]> =
        SPECTRA_KEYS.iter().map(|&k| (k, [None, None, None])).collect();
    let br: Vec<i64> = log.breaks(sel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let psd = |row: &[f32]| -> Result<Vec<f64>, String> {
        let (a, b) = (sel.start.min(row.len()), sel.end.min(row.len()));
        let x = wide(&row[a..a.max(b)]);
        Ok(to_ptb_db(
            &spectrum(&x, fs, Mode::Psd, 1000.0 / 300.0, 0, Some(&br))?.values,
        ))
    };
    // réponses des filtres par fenêtre, communes aux trois axes et au gyro / D
    let mut cache = WhatIfCache::default();
    let mut freqs = Vec::new();
    let mut d_scale = Vec::new();
    let mut prefilter: Option<Rows> = None;
    for ax in 0..3 {
        let unfilt = log.gyro_unfilt.as_ref().map(|u| wide(u.row(ax)));
        let w = whatif_spectrum(
            &wide(log.gyro.row(ax)),
            unfilt.as_deref(),
            log,
            sel.clone(),
            base,
            modf,
            Target::Gyro,
            300.0,
            &mut cache,
        )?;
        freqs = w.freqs;
        spectra.get_mut("gyro").unwrap()[ax] = Some(to_ptb_db(&w.current_db));
        spectra.get_mut("gyro_sim").unwrap()[ax] = Some(to_ptb_db(&w.modified_db));
        if with_current {
            spectra.get_mut("gyro_sim_cur").unwrap()[ax] = Some(to_ptb_db(&w.current_db));
        }
        if let Some(u) = &log.gyro_unfilt {
            spectra.get_mut("gyro_pf").unwrap()[ax] = Some(psd(u.row(ax))?);
        }
        let d = log.d_term.row(ax);
        if !d.iter().any(|&v| v != 0.0) {
            d_scale.push(f64::NAN);
            continue;
        }
        d_scale.push(1.0);
        if prefilter.is_none() {
            prefilter = Some(log.d_term_prefilter()?);
        }
        let pf = prefilter.as_ref().unwrap().row(ax);
        let wd = whatif_spectrum(
            &wide(d),
            Some(&wide(pf)),
            log,
            sel.clone(),
            base,
            modf,
            Target::Dterm,
            300.0,
            &mut cache,
        )?;
        spectra.get_mut("d").unwrap()[ax] = Some(to_ptb_db(&wd.current_db));
        spectra.get_mut("d_sim").unwrap()[ax] = Some(to_ptb_db(&wd.modified_db));
        if with_current {
            spectra.get_mut("d_sim_cur").unwrap()[ax] = Some(to_ptb_db(&wd.current_db));
        }
        spectra.get_mut("d_pf").unwrap()[ax] = Some(psd(pf)?);
    }
    Ok(FilterSimResult {
        log_id: 0,
        fs,
        base: base.clone(),
        modf: modf.clone(),
        freqs,
        spectra,
        d_scale,
        has_unfilt,
    })
}

// ---------------------------------------------------------------- commandes CLI

fn type_code(t: &str) -> usize {
    LPF_TYPES.iter().position(|&x| x == t).unwrap_or(0)
}

/// `cli_values` : valeurs CLI Betaflight des réglages de filtres, dans l'ordre du dictionnaire de Python.
pub fn cli_values(s: &Settings) -> Vec<(String, String)> {
    let notch = |i: usize| s.gyro_notches.get(i).copied().unwrap_or((0, 0));
    let weights: Vec<String> = if s.rpm_weights.is_empty() {
        vec!["100".to_string()]
    } else {
        s.rpm_weights.iter().take(3).map(i64::to_string).collect()
    };
    let (g1, g2, d1, d2) = (&s.gyro_lpf1, &s.gyro_lpf2, &s.dterm_lpf1, &s.dterm_lpf2);
    let out: Vec<(&str, String)> = vec![
        ("gyro_lpf1_type", g1.kind.to_string()),
        ("gyro_lpf1_static_hz", g1.static_hz.to_string()),
        ("gyro_lpf1_dyn_min_hz", g1.dyn_min_hz.to_string()),
        ("gyro_lpf1_dyn_max_hz", g1.dyn_max_hz.to_string()),
        ("gyro_lpf1_dyn_expo", g1.expo.to_string()),
        ("gyro_lpf2_type", g2.kind.to_string()),
        ("gyro_lpf2_static_hz", g2.static_hz.to_string()),
        ("gyro_notch1_hz", notch(0).0.to_string()),
        ("gyro_notch1_cutoff", notch(0).1.to_string()),
        ("gyro_notch2_hz", notch(1).0.to_string()),
        ("gyro_notch2_cutoff", notch(1).1.to_string()),
        ("dterm_lpf1_type", d1.kind.to_string()),
        ("dterm_lpf1_static_hz", d1.static_hz.to_string()),
        ("dterm_lpf1_dyn_min_hz", d1.dyn_min_hz.to_string()),
        ("dterm_lpf1_dyn_max_hz", d1.dyn_max_hz.to_string()),
        ("dterm_lpf1_dyn_expo", d1.expo.to_string()),
        ("dterm_lpf2_type", d2.kind.to_string()),
        ("dterm_lpf2_static_hz", d2.static_hz.to_string()),
        ("dterm_notch_hz", s.dterm_notch.0.to_string()),
        ("dterm_notch_cutoff", s.dterm_notch.1.to_string()),
        ("dyn_notch_count", s.dyn_notch_count.to_string()),
        ("dyn_notch_q", s.dyn_notch_q.to_string()),
        ("dyn_notch_min_hz", s.dyn_notch_min_hz.to_string()),
        ("dyn_notch_max_hz", s.dyn_notch_max_hz.to_string()),
        ("rpm_filter_harmonics", s.rpm_harmonics.to_string()),
        ("rpm_filter_q", s.rpm_q.to_string()),
        ("rpm_filter_min_hz", s.rpm_min_hz.to_string()),
        ("rpm_filter_weights", weights.join(",")),
    ];
    out.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// `filter_cli` : lignes de commandes CLI pour passer des filtres de `base` à ceux de `modf` (seulement ce qui change).
pub fn filter_cli_lines(base: &Settings, modf: &Settings) -> Vec<String> {
    let (a, b) = (cli_values(base), cli_values(modf));
    let fam = base.family();
    let mut lines = vec![format!("# OpenTuneBox — filtres modifiés (Betaflight {fam})")];
    let changed: Vec<usize> = (0..b.len()).filter(|&i| a[i].1 != b[i].1).collect();
    if changed.is_empty() {
        lines.push("# aucun changement par rapport au log".to_string());
        return lines;
    }
    let sl = base.sliders();
    let slider = |name: &str| sl.iter().find(|(n, _)| *n == name).map_or(0, |(_, v)| *v);
    let any = |prefix: &str| changed.iter().any(|&i| b[i].0.starts_with(prefix));
    if any("gyro_lpf") && slider("gyro_filter") == 1 {
        lines.push("# réglage manuel : on désactive le curseur de filtre gyro (comme le configurateur)".to_string());
        lines.push("set simplified_gyro_filter = OFF".to_string());
    }
    if any("dterm_lpf") && slider("dterm_filter") == 1 {
        lines.push("# réglage manuel : on désactive le curseur de filtre D (comme le configurateur)".to_string());
        lines.push("set simplified_dterm_filter = OFF".to_string());
    }
    for &i in &changed {
        let (k, v) = &b[i];
        if k == "rpm_filter_weights" && fam == "4.4" {
            lines.push("# rpm_filter_weights n'existe qu'à partir de Betaflight 4.5 (ignoré)".to_string());
            continue;
        }
        lines.push(format!("set {k} = {v}"));
    }
    if fam == "4.4" && modf.dyn_notch_count > 5 {
        lines.push("# attention : 5 notchs dynamiques au plus en 4.4".to_string());
    }
    lines.push("save".to_string());
    lines
}

/// Texte affiché dans le cadre CLI (lignes séparées par des retours à la ligne).
pub fn filter_cli(base: &Settings, modf: &Settings) -> String {
    filter_cli_lines(base, modf).join("\n")
}

// ---------------------------------------------------------------- éditeur

/// Les six contrôles d'un LPF1 (gyro `g1` ou D `d1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lpf1Edit {
    /// Indice dans `LPF_TYPES`.
    pub kind: usize,
    /// Indice dans `LPF_MODES`.
    pub mode: usize,
    pub static_hz: i64,
    pub min_hz: i64,
    pub max_hz: i64,
    pub expo: i64,
}

fn clamp_to(v: i64, (lo, hi): (i64, i64)) -> i64 {
    v.clamp(lo, hi)
}

impl Lpf1Edit {
    /// `lpf(...)` de `_load_editor`. Les bornes des champs sont appliquées par `Editor::clamp`.
    fn from_lowpass(lp: &Lowpass) -> Self {
        Lpf1Edit {
            kind: type_code(lp.kind),
            mode: if lp.dynamic() {
                2
            } else if lp.static_hz > 0 {
                1
            } else {
                0
            },
            static_hz: lp.static_hz,
            min_hz: lp.dyn_min_hz,
            max_hz: lp.dyn_max_hz,
            expo: lp.expo,
        }
    }

    fn clamp(&mut self) {
        self.kind = self.kind.min(LPF_TYPES.len() - 1);
        self.mode = self.mode.min(LPF_MODES.len() - 1);
        self.static_hz = clamp_to(self.static_hz, HZ_RANGE);
        self.min_hz = clamp_to(self.min_hz, HZ_RANGE);
        self.max_hz = clamp_to(self.max_hz, HZ_RANGE);
        self.expo = clamp_to(self.expo, EXPO_RANGE);
    }

    /// `lpf(p, lp)` de `modified_settings`.
    fn apply(&self, lp: &mut Lowpass) {
        lp.kind = LPF_TYPES[self.kind.min(LPF_TYPES.len() - 1)];
        lp.expo = self.expo;
        lp.dyn_max_hz = self.max_hz;
        match self.mode {
            0 => {
                lp.static_hz = 0;
                lp.dyn_min_hz = 0;
            }
            1 => {
                lp.static_hz = self.static_hz.max(1);
                lp.dyn_min_hz = 0;
            }
            _ => {
                lp.static_hz = self.static_hz;
                lp.dyn_min_hz = self.min_hz.max(1);
                lp.dyn_max_hz = lp.dyn_min_hz.max(lp.dyn_max_hz);
            }
        }
    }
}

/// Valeurs des champs du panneau (une par contrôle de Python, mêmes noms : `g1` = `g1_*`, `d1` = `d1_*`, `rpm_w` = `rpm_w0..2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Editor {
    pub g1: Lpf1Edit,
    /// Type du LPF2 gyro (indice dans `LPF_TYPES`) et coupure.
    pub g2_type: usize,
    pub g2_hz: i64,
    /// Notchs statiques gyro : centre et coupure de chacun.
    pub n1_hz: i64,
    pub n1_cut: i64,
    pub n2_hz: i64,
    pub n2_cut: i64,
    pub d1: Lpf1Edit,
    pub d2_type: usize,
    pub d2_hz: i64,
    /// Notch D.
    pub dn_hz: i64,
    pub dn_cut: i64,
    pub dyn_count: i64,
    pub dyn_q: i64,
    pub dyn_min: i64,
    pub dyn_max: i64,
    pub rpm_harm: i64,
    pub rpm_q: i64,
    pub rpm_min: i64,
    pub rpm_w: [i64; 3],
}

impl Editor {
    /// `_load_editor` (suivi du `setMaximum` de `_ensure_base`) : champs initiaux d'un jeu de réglages. Comme les `QSpinBox`, chaque
    /// valeur est ramenée dans les bornes de son champ.
    pub fn from_settings(s: &Settings) -> Editor {
        let notch = |i: usize| s.gyro_notches.get(i).copied().unwrap_or((0, 0));
        let w = |i: usize| s.rpm_weights.get(i).copied().unwrap_or(100);
        let mut e = Editor {
            g1: Lpf1Edit::from_lowpass(&s.gyro_lpf1),
            g2_type: type_code(s.gyro_lpf2.kind),
            g2_hz: s.gyro_lpf2.static_hz,
            n1_hz: notch(0).0,
            n1_cut: notch(0).1,
            n2_hz: notch(1).0,
            n2_cut: notch(1).1,
            d1: Lpf1Edit::from_lowpass(&s.dterm_lpf1),
            d2_type: type_code(s.dterm_lpf2.kind),
            d2_hz: s.dterm_lpf2.static_hz,
            dn_hz: s.dterm_notch.0,
            dn_cut: s.dterm_notch.1,
            dyn_count: s.dyn_notch_count,
            dyn_q: s.dyn_notch_q.max(1),
            dyn_min: if s.dyn_notch_min_hz != 0 {
                s.dyn_notch_min_hz
            } else {
                DYN_MIN_DEFAULT
            },
            dyn_max: if s.dyn_notch_max_hz != 0 {
                s.dyn_notch_max_hz
            } else {
                DYN_MAX_DEFAULT
            },
            rpm_harm: s.rpm_harmonics,
            rpm_q: s.rpm_q,
            rpm_min: s.rpm_min_hz,
            rpm_w: [w(0), w(1), w(2)],
        };
        e.clamp(s.family());
        e
    }

    /// Ramène chaque champ dans ses bornes (`QSpinBox.setValue` / `setMaximum`) ; `family` fixe le maximum des notchs dynamiques
    /// (5 en 4.4, 7 sinon).
    pub fn clamp(&mut self, family: &str) {
        self.g1.clamp();
        self.d1.clamp();
        let last = LPF_TYPES.len() - 1;
        self.g2_type = self.g2_type.min(last);
        self.d2_type = self.d2_type.min(last);
        for v in [
            &mut self.g2_hz,
            &mut self.n1_hz,
            &mut self.n1_cut,
            &mut self.n2_hz,
            &mut self.n2_cut,
            &mut self.d2_hz,
            &mut self.dn_hz,
            &mut self.dn_cut,
        ] {
            *v = clamp_to(*v, HZ_RANGE);
        }
        let max = if family == "4.4" {
            DYN_COUNT_MAX_44
        } else {
            DYN_COUNT_MAX
        };
        self.dyn_count = clamp_to(self.dyn_count, (0, max));
        self.dyn_q = clamp_to(self.dyn_q, DYN_Q_RANGE);
        self.dyn_min = clamp_to(self.dyn_min, DYN_MIN_RANGE);
        self.dyn_max = clamp_to(self.dyn_max, DYN_MAX_RANGE);
        self.rpm_harm = clamp_to(self.rpm_harm, RPM_HARM_RANGE);
        self.rpm_q = clamp_to(self.rpm_q, RPM_Q_RANGE);
        self.rpm_min = clamp_to(self.rpm_min, RPM_MIN_RANGE);
        for w in &mut self.rpm_w {
            *w = clamp_to(*w, RPM_WEIGHT_RANGE);
        }
    }

    /// `modified_settings` : réglages du log modifiés par l'éditeur (copie, le log n'est pas touché). Les deux positions de notch
    /// gyro sont toujours écrites (centre 0 = notch désactivé, ignoré par `gyro_chain`).
    pub fn modified_settings(&self, base: &Settings) -> Settings {
        let mut s = base.clone();
        self.g1.apply(&mut s.gyro_lpf1);
        self.d1.apply(&mut s.dterm_lpf1);
        s.gyro_lpf2.kind = LPF_TYPES[self.g2_type.min(LPF_TYPES.len() - 1)];
        s.gyro_lpf2.static_hz = self.g2_hz;
        s.dterm_lpf2.kind = LPF_TYPES[self.d2_type.min(LPF_TYPES.len() - 1)];
        s.dterm_lpf2.static_hz = self.d2_hz;
        s.gyro_notches = vec![(self.n1_hz, self.n1_cut), (self.n2_hz, self.n2_cut)];
        s.dterm_notch = (self.dn_hz, self.dn_cut);
        s.dyn_notch_count = self.dyn_count;
        s.dyn_notch_q = self.dyn_q;
        s.dyn_notch_min_hz = self.dyn_min;
        s.dyn_notch_max_hz = self.dyn_max;
        s.rpm_harmonics = self.rpm_harm;
        s.rpm_q = self.rpm_q;
        s.rpm_min_hz = self.rpm_min;
        s.rpm_weights = self.rpm_w.to_vec();
        s
    }
}

// ---------------------------------------------------------------- Bode et retards

/// Nombre de points des courbes de Bode.
pub const BODE_POINTS: usize = 900;

/// Courbe de Bode d'une chaîne.
#[derive(Debug, Clone, PartialEq)]
pub struct Bode {
    pub freqs: Vec<f64>,
    /// `20 log10(max(|h|, 1e-4))`.
    pub gain_db: Vec<f64>,
    /// `degrees(unwrap(angle(h)))`.
    pub phase_deg: Vec<f64>,
}

/// `np.geomspace(start, stop, num)` : extrémités exactes.
pub fn geomspace(start: f64, stop: f64, num: usize) -> Vec<f64> {
    if num == 0 {
        return Vec::new();
    }
    let (ls, le) = (start.log10(), stop.log10());
    let div = (num - 1) as f64;
    let step = (le - ls) / div;
    let mut out: Vec<f64> = (0..num)
        .map(|i| {
            let y = if num > 1 && i == num - 1 {
                le
            } else {
                i as f64 * step + ls
            };
            10f64.powf(y)
        })
        .collect();
    out[0] = start;
    if num > 1 {
        out[num - 1] = stop;
    }
    out
}

/// `_chains` : chaînes gyro et D d'un jeu de réglages, pour des gaz (%) et une fréquence moteur donnés. `dyn_notch_initial` :
/// notchs dynamiques aux centres initiaux (sinon ils sont absents de la chaîne gyro).
pub fn chains(s: &Settings, thr_pct: i64, motor_hz: f64, dyn_notch_initial: bool) -> Result<(Chain, Chain), String> {
    let thr = thr_pct as f64 / 100.0;
    let dynamic: Option<&[f64]> = if dyn_notch_initial { None } else { Some(&[]) };
    let g = gyro_chain(s, thr, Some(&[motor_hz]), dynamic)?;
    Ok((g, dterm_chain(s, thr)))
}

/// Courbe de Bode d'une chaîne, 900 points de 1 Hz à 0,999 × Nyquist.
pub fn bode_of(chain: &Chain, nyq: f64) -> Result<Bode, String> {
    let f = geomspace(1.0, nyq * 0.999, BODE_POINTS);
    let (re, im) = chain_response(chain, &f)?;
    let gain_db = re
        .iter()
        .zip(&im)
        .map(|(r, i)| 20.0 * r.hypot(*i).max(1e-4).log10())
        .collect();
    let angle: Vec<f64> = re.iter().zip(&im).map(|(r, i)| i.atan2(*r)).collect();
    let phase_deg = unwrap(&angle).into_iter().map(f64::to_degrees).collect();
    Ok(Bode {
        freqs: f,
        gain_db,
        phase_deg,
    })
}

/// Bode des chaînes gyro et D d'un jeu de réglages : `(gyro, D)`.
pub fn bode(s: &Settings, thr_pct: i64, motor_hz: f64, dyn_notch_initial: bool) -> Result<(Bode, Bode), String> {
    let (g, d) = chains(s, thr_pct, motor_hz, dyn_notch_initial)?;
    let nyq = s.pid_rate_hz() / 2.0;
    Ok((bode_of(&g, nyq)?, bode_of(&d, nyq)?))
}

/// Une ligne du tableau des retards (ms) : pour chaque fréquence de `DELAY_FREQS`, (actuel, modifié).
#[derive(Debug, Clone, PartialEq)]
pub struct DelayRow {
    pub name: &'static str,
    pub cells: [(f64, f64); 3],
}

/// Retards des filtres et coupures des LPF1 dynamiques.
#[derive(Debug, Clone, PartialEq)]
pub struct Delays {
    /// Gyro, D seul, Gyro + D.
    pub rows: Vec<DelayRow>,
    /// (« gyro » ou « D », coupure en Hz) pour chaque LPF1 dynamique des réglages modifiés, à ces gaz.
    pub dyn_cutoffs: Vec<(&'static str, f64)>,
}

/// Retard de phase (ms) d'une chaîne à `f_hz` ; 0 pour une chaîne vide.
fn phase_ms(chain: &Chain, f_hz: f64) -> Result<f64, String> {
    if chain.is_empty() {
        return Ok(0.0);
    }
    Ok(chain_delay_ms(chain, f_hz)?.phase_ms)
}

/// `_delay_html` sans le HTML : retards de phase à 20 / 50 / 100 Hz (actuel, modifié) et coupures dynamiques.
pub fn delays(
    base: &Settings,
    modf: &Settings,
    thr_pct: i64,
    motor_hz: f64,
    dyn_notch_initial: bool,
) -> Result<Delays, String> {
    let (cg, cd) = chains(base, thr_pct, motor_hz, dyn_notch_initial)?;
    let (mg, md) = chains(modf, thr_pct, motor_hz, dyn_notch_initial)?;
    let both = |g: &Chain, d: &Chain| -> Chain { g.iter().chain(d.iter()).cloned().collect() };
    let lines: [(&'static str, Chain, Chain); 3] = [
        ("Gyro", cg.clone(), mg.clone()),
        ("D seul", cd.clone(), md.clone()),
        ("Gyro + D", both(&cg, &cd), both(&mg, &md)),
    ];
    let mut rows = Vec::new();
    for (name, cur, new) in &lines {
        let mut cells = [(0.0, 0.0); 3];
        for (cell, &fz) in cells.iter_mut().zip(&DELAY_FREQS) {
            *cell = (phase_ms(cur, fz)?, phase_ms(new, fz)?);
        }
        rows.push(DelayRow { name, cells });
    }
    let thr = thr_pct as f64 / 100.0;
    let mut dyn_cutoffs = Vec::new();
    for (name, lp) in [("gyro", &modf.gyro_lpf1), ("D", &modf.dterm_lpf1)] {
        if lp.dynamic() {
            dyn_cutoffs.push((
                name,
                dyn_lpf_cutoff(lp.dyn_min_hz as f64, lp.dyn_max_hz as f64, lp.expo as f64, thr),
            ));
        }
    }
    Ok(Delays { rows, dyn_cutoffs })
}
