//! Vérification des logs ouverts pour le guide intégré (`advisor/log_check.py`) : à quoi sert chaque log, et quoi corriger avant le
//! prochain vol.
//!
//! Textes caractère par caractère, décisions identiques. Une exception de Python devient une `Err` (comme dans `rules.rs`).

use crate::profiles::filter_delay_target_ms;
use crate::rules::{
    DTERM_PTB_MAX_DB, GYRO_PTB_MAX_DB, PID_LOG_RATE_FLOOR_HZ, PID_LOG_RATE_MIN_HZ, band_peak_ptb_db, gaps_text,
    log_rate_needed, pid_log_rate_needed, sample_rate_cli,
};
use crate::step_tuning::{I_SLIDER_RANGE, MIN_WINDOWS, SURE_WINDOWS};
use otb_analysis::delay::{MOVING_SETPOINT_DPS, compute_delays, stick_p95_dps, xcorr_delay_ms};
use otb_analysis::motors::hover_motor_hz;
use otb_analysis::npstat::std_f32;
use otb_analysis::period::{crash_time, flight_parts};
use otb_analysis::sliders::{PTB_CATEGORIES, slider_view};
use otb_analysis::step::{LoopInput, StepOptions, input_windows, step_response};
use otb_analysis::step_series::{
    FF_KEY, FlightConditions, FlightStep, Sig, filter_signature, find_series, flight_conditions,
};
use otb_format::{fixed, percent, round_digits};
use otb_log::flightlog::{FlightLog, clamp_range};
use std::collections::BTreeMap;
use std::ops::Range;

pub const OK: &str = "ok";
pub const WARN: &str = "à revoir";
pub const BLOCK: &str = "bloquant";
pub const INFO: &str = "info";

/// Tables de débogage et réglages normalisés à partir de la 4.4 (`log/debug_tables.py`).
pub const MIN_VERSION: [i64; 2] = [4, 4];
/// Vols de test conseillés : 20 à 40 s (guide) ; en dessous de 15 s, trop peu de fenêtres.
pub const MIN_DURATION_S: f64 = 15.0;

// Protocole des vols avec mouvements (choix de Cédric du 2026-10-12, d'après [S24, S33])
/// `rates_type` : BETAFLIGHT 0, RACEFLIGHT 1, KISS 2, ACTUAL 3, QUICK 4 (firmware).
pub const RATES_TYPE_ACTUAL: i64 = 3;
/// Rates linéaires : sensibilité au centre = maximum = 200 °/s, expo 0, sur les trois axes.
pub const LOG_RATE_DPS: i64 = 200;
/// Limite d'angle conseillée pour le wobble [S33] (degrés).
pub const LOG_ANGLE_LIMIT: i64 = 30;
/// Part du temps en mouvement où roll et pitch bougent ensemble (corpus, 2026-10-12).
pub const AXES_MIXED_MIN: f64 = 0.4;
/// Curseur Dynamic Damping « à 0 » pendant les séries [S34].
pub const DYN_DAMPING_MAX: f64 = 0.05;
/// PIDtoolbox mesure le retard du D ~0,5 ms plus court que nous (`profiles.py`).
pub const D_DELAY_PTB_OFFSET_MS: f64 = 0.5;

/// Curseurs de l'onglet « Réglages de PID » du configurateur : catégorie PTB → libellé français (anglais).
pub const CONFIGURATOR_SLIDERS: [(&str, &str); 8] = [
    ("Damping", "Atténuation (Damping)"),
    ("Tracking", "Suivi (Tracking)"),
    ("Stick Resp.", "Réponse des sticks (Stick Response)"),
    ("Dynamic D.", "Atténuation dynamique (Dynamic Damping)"),
    ("Drift-Wobble", "Dérive - Oscillations (Drift - Wobble)"),
    ("Pitch D.", "Atténuation du Pitch (Pitch Damping)"),
    ("Pitch T.", "Suivi du Pitch (Pitch Tracking)"),
    ("Master", "Multiplicateur Maître (Master Multiplier)"),
];

pub fn configurator_slider(key: &str) -> &'static str {
    CONFIGURATOR_SLIDERS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .unwrap_or_else(|| panic!("curseur inconnu : {key}"))
}

/// Rôle d'un log : vol de réglage des filtres ou vol d'une série PID.
pub const FILTERS: &str = "filters";
pub const PID: &str = "pid";
pub const ROLE_NAMES: [(&str, &str); 2] = [(FILTERS, "filtres"), (PID, "séries PID")];
const ROLE_ITEMS_FILTERS: [&str; 4] = ["log_rate", "gyro_unfilt", "debug_mode", "rpm"];
const ROLE_ITEMS_PID: [&str; 5] = ["pid_log_rate", "windows", "axes_mixed", "i_slider", "dyn_damping"];

fn role_items(role: &str) -> &'static [&'static str] {
    if role == FILTERS {
        &ROLE_ITEMS_FILTERS
    } else {
        &ROLE_ITEMS_PID
    }
}

fn usage_role(key: &str) -> Option<&'static str> {
    match key {
        "noise" | "delay" => Some(FILTERS),
        "series" | "ff_series" => Some(PID),
        _ => None,
    }
}

/// Une correction : où la faire dans le configurateur, et les commandes CLI équivalentes (repli).
#[derive(Debug, Clone, PartialEq)]
pub struct Fix {
    pub where_: String,
    /// Une ou plusieurs commandes, séparées par des retours à la ligne.
    pub cli: String,
}

impl Fix {
    fn new(where_: impl Into<String>, cli: impl Into<String>) -> Fix {
        Fix {
            where_: where_.into(),
            cli: cli.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckItem {
    /// Clé stable (cases du guide).
    pub key: String,
    pub title: String,
    pub status: &'static str,
    pub detail: String,
    pub fixes: Vec<Fix>,
}

/// Ce que le log permet de faire (régler les filtres, lire une série PID…).
#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    pub key: String,
    pub label: String,
    pub ok: bool,
    pub reason: String,
    /// Affiché dans le panneau des logs (sinon seulement lu par les cases du guide).
    pub shown: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogCheck {
    pub label: String,
    /// Le log contient une partie avec mouvements.
    pub moving: bool,
    pub hover: Option<(f64, f64)>,
    pub moving_part: Option<(f64, f64)>,
    pub items: Vec<CheckItem>,
    pub usages: Vec<Usage>,
    /// Fenêtres roll, pitch de la partie avec mouvements.
    pub windows: Vec<usize>,
    /// `windows` = vrai N de la réponse indicielle (sinon estimation rapide).
    pub exact: bool,
    /// Curseurs (catégorie PTB → fraction).
    pub sliders: Vec<(&'static str, f64)>,
    /// `FILTERS` ou `PID` : deviné par `check_log`, fixé par le guide.
    pub role: &'static str,
    /// Retard consigne → gyro roll, pitch (partie avec mouvements).
    pub sp_delay_ms: Vec<f64>,
}

impl LogCheck {
    pub fn item(&self, key: &str) -> Option<&CheckItem> {
        self.items.iter().find(|it| it.key == key)
    }

    pub fn usage(&self, key: &str) -> Option<&Usage> {
        self.usages.iter().find(|u| u.key == key)
    }

    /// Points utiles pour ce rôle (par défaut, celui du log) : sans ceux propres à l'autre rôle.
    /// Points utiles pour ce rôle (par défaut, celui du log ; un rôle vide vaut « par défaut », comme `role or self.role`) : sans ceux
    /// propres aux autres rôles (un rôle inconnu écarte ceux des deux).
    pub fn items_for(&self, role: Option<&str>) -> Vec<&CheckItem> {
        let role = self.role_or(role);
        let other: Vec<&str> = [FILTERS, PID]
            .into_iter()
            .filter(|r| *r != role)
            .flat_map(|r| role_items(r).iter().copied())
            .collect();
        self.items
            .iter()
            .filter(|it| !other.contains(&it.key.as_str()))
            .collect()
    }

    fn role_or<'a>(&'a self, role: Option<&'a str>) -> &'a str {
        match role {
            Some(r) if !r.is_empty() => r,
            _ => self.role,
        }
    }

    pub fn usages_for(&self, role: Option<&str>) -> Vec<&Usage> {
        let role = self.role_or(role);
        self.usages
            .iter()
            .filter(|u| u.shown && usage_role(&u.key).unwrap_or(role) == role)
            .collect()
    }

    pub fn cli_commands(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for it in self.items_for(None) {
            if it.status == WARN || it.status == BLOCK {
                for f in &it.fixes {
                    // `out += [c for c in … if c and c not in out]` : la liste entière est évaluée avant d'être ajoutée
                    let fresh: Vec<String> = f
                        .cli
                        .lines()
                        .filter(|c| !c.is_empty() && !out.iter().any(|o| o == c))
                        .map(str::to_string)
                        .collect();
                    out.extend(fresh);
                }
            }
        }
        out
    }
}

fn nn(x: f64, d: usize) -> String {
    fixed(x, d, false).replace('.', ",")
}

fn n1(x: f64) -> String {
    nn(x, 1)
}

fn db(x: f64) -> String {
    format!("{} dB", fixed(x, 0, false)).replace('-', "−")
}

fn span(p: (f64, f64)) -> String {
    format!("{} à {} s", n1(p.0), n1(p.1))
}

/// `str.capitalize()` de Python : première lettre en majuscule, le reste en minuscules.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().chain(c.flat_map(|x| x.to_lowercase())).collect(),
    }
}

/// Rates ACTUAL du profil enregistré : (sensibilité au centre °/s, maximum °/s, expo %) par axe ; `None` pour un autre type de
/// rates. En-tête : `rates_type`, `rc_rates` (centre ÷ 10), `rates` (maximum ÷ 10), `rc_expo` (%).
pub fn log_rates(s: &otb_log::settings::Settings) -> Option<(Vec<i64>, Vec<i64>, Vec<i64>)> {
    if s.get_int("rates_type", -1) != RATES_TYPE_ACTUAL {
        return None;
    }
    let (rc, mx, ex) = (s.get_ints("rc_rates"), s.get_ints("rates"), s.get_ints("rc_expo"));
    if rc.len().min(mx.len()).min(ex.len()) < 3 {
        return None;
    }
    Some((
        rc[..3].iter().map(|v| 10 * v).collect(),
        mx[..3].iter().map(|v| 10 * v).collect(),
        ex[..3].to_vec(),
    ))
}

/// Part du temps en mouvement (roll ou pitch au-delà de `min_dps`) où les deux axes bougent ensemble.
pub fn axes_mixed_share(log: &FlightLog, sel: Range<usize>, min_dps: f64) -> f64 {
    let sel = clamp_range(sel.start, sel.end, log.n());
    let (r, p) = (&log.setpoint.row(0)[sel.clone()], &log.setpoint.row(1)[sel]);
    let thr = min_dps as f32;
    let (mut anym, mut both) = (0usize, 0usize);
    for (a, b) in r.iter().zip(p) {
        let (ma, mb) = (a.abs() > thr, b.abs() > thr);
        anym += (ma || mb) as usize;
        both += (ma && mb) as usize;
    }
    if anym > 0 { both as f64 / anym as f64 } else { 0.0 }
}

/// Mise en place du profil de rates « LOG » des vols de réglage.
pub fn log_rates_fix() -> Fix {
    let mut cli = vec!["set rates_type = ACTUAL".to_string()];
    for ax in ["roll", "pitch", "yaw"] {
        cli.push(format!("set {ax}_rc_rate = {}", LOG_RATE_DPS / 10));
        cli.push(format!("set {ax}_srate = {}", LOG_RATE_DPS / 10));
        cli.push(format!("set {ax}_expo = 0"));
    }
    Fix::new(
        format!(
            "Onglet « Réglages de PID », profil de rates « LOG » : « Type de rates » ACTUAL, « Sensibilité au centre » {LOG_RATE_DPS}, « Max Vel » {LOG_RATE_DPS}, Expo 0, sur les trois axes."
        ),
        cli.join("\n"),
    )
}

/// État partagé par les parties de `check_log` (`_Check` de Python).
struct Check<'a> {
    log: &'a FlightLog,
    sel: Range<usize>,
    t0: f64,
    t1: f64,
    items: Vec<CheckItem>,
    version_ok: bool,
    rate_ok: bool,
    harm_ok: bool,
    pid_rate_ok: bool,
    unfilt: bool,
    hover: Option<(f64, f64)>,
    moving: Option<(f64, f64)>,
    cond: Option<FlightConditions>,
    sliders: Vec<(&'static str, f64)>,
    sv_from_header: bool,
    windows: Vec<usize>,
    sp_delay: Vec<f64>,
    i_ok: bool,
    ff_ok: bool,
    enough: bool,
}

impl Check<'_> {
    fn dur(&self) -> f64 {
        self.t1 - self.t0
    }

    fn add(&mut self, key: &str, title: &str, status: &'static str, detail: String, fixes: Vec<Fix>) {
        self.items.push(CheckItem {
            key: key.into(),
            title: title.into(),
            status,
            detail,
            fixes,
        });
    }
}

pub fn check_log(log: &FlightLog, sel: Option<Range<usize>>, label: &str, exact: bool) -> Result<LogCheck, String> {
    let n = log.n();
    let sel = sel.unwrap_or(0..n);
    // Python : `sel.start or 0`, `min(sel.stop or log.n, log.n)` (un stop à 0 vaut « tout »)
    let (i0, i1) = (sel.start, if sel.end == 0 { n } else { sel.end.min(n) });
    let (t0, t1) = if i1 > i0 {
        (log.t[i0], log.t[i1 - 1])
    } else {
        (0.0, 0.0)
    };
    let mut c = Check {
        log,
        sel,
        t0,
        t1,
        items: Vec::new(),
        version_ok: false,
        rate_ok: false,
        harm_ok: true,
        pid_rate_ok: false,
        unfilt: false,
        hover: None,
        moving: None,
        cond: None,
        sliders: Vec::new(),
        sv_from_header: false,
        windows: Vec::new(),
        sp_delay: Vec::new(),
        i_ok: true,
        ff_ok: true,
        enough: false,
    };
    recording_items(&mut c);
    flight_parts_item(&mut c)?;
    protocol_items(&mut c, exact)?;
    general_items(&mut c)?;
    let usages = usages(&c);
    Ok(LogCheck {
        label: label.to_string(),
        moving: c.moving.is_some(),
        hover: c.hover,
        moving_part: c.moving,
        items: c.items,
        usages,
        windows: c.windows,
        exact,
        sliders: c.sliders,
        role: if c.hover.is_some() { FILTERS } else { PID },
        sp_delay_ms: c.sp_delay,
    })
}

/// Enregistrement : version, durée, fréquence du log (filtres, PID), gyro non filtré, débogage, eRPM.
fn recording_items(c: &mut Check) {
    let (log, fs) = (c.log, c.log.fs);
    let s = &log.settings;
    let ver2 = &s.version[..s.version.len().min(2)];
    c.version_ok = s.firmware_name.to_lowercase().starts_with("betaflight") && ver2 >= &MIN_VERSION[..];
    let vtxt = if s.version.is_empty() {
        "inconnue".to_string()
    } else {
        s.version.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(".")
    };
    let ok = c.version_ok;
    c.add(
        "version",
        "Version du firmware",
        if ok { OK } else { BLOCK },
        format!("{} {vtxt}.", s.firmware_name)
            + if ok {
                ""
            } else {
                " OpenTuneBox lit Betaflight 4.4 et plus récent."
            },
        if ok {
            vec![]
        } else {
            vec![Fix::new(
                "Mettre à jour le firmware (onglet « Flasher le Firmware »), après une sauvegarde `diff all`.",
                "",
            )]
        },
    );

    let dur = c.dur();
    c.add(
        "duration",
        "Durée analysée",
        if dur >= MIN_DURATION_S { OK } else { WARN },
        format!("{} s de vol dans la période d'analyse", n1(dur))
            + if dur >= MIN_DURATION_S {
                "."
            } else {
                " : trop court, visez 20 à 40 s pour chaque partie du vol."
            },
        vec![],
    );

    let hz = hover_motor_hz(log);
    let pid_hz = s.pid_rate_hz();
    let base_hz = log_rate_needed(pid_hz, None);
    let harm_hz = log_rate_needed(pid_hz, hz);
    // max(base_hz, harm_hz) de Python : le premier, sauf si le second est strictement plus grand
    let need = if harm_hz > base_hz { harm_hz } else { base_hz };
    c.rate_ok = fs >= 0.95 * base_hz;
    c.harm_ok = hz.is_none_or(|h| 3.0 * h <= fs / 2.0) || harm_hz <= fs * 1.05;
    let loops = format!(
        "gyro {} kHz, boucle PID {} kHz",
        n1(s.gyro_rate_hz() / 1000.0),
        n1(pid_hz / 1000.0)
    );
    let frac = sample_rate_cli(pid_hz, need);
    let rate_fix = vec![Fix::new(
        format!(
            "Onglet « Blackbox » → « Fréquence d'enregistrement de la Blackbox » : {} Hz ({frac}).",
            fixed(need, 0, false)
        ),
        format!("set blackbox_sample_rate = {frac}"),
    )];
    if !c.rate_ok {
        c.add(
            "log_rate",
            "Fréquence d'enregistrement (filtres)",
            BLOCK,
            format!(
                "{} Hz ({loops}) : il faut au moins {} Hz. Le bruit au-dessus de {} Hz se replie dans le spectre (fausses raies) : les filtres ne peuvent pas être jugés.",
                fixed(fs, 0, false),
                fixed(base_hz, 0, false),
                fixed(fs / 2.0, 0, false)
            ),
            rate_fix,
        );
    } else if !c.harm_ok {
        let h = hz.unwrap();
        c.add(
            "log_rate",
            "Fréquence d'enregistrement (filtres)",
            WARN,
            format!(
                "{} Hz : les moteurs tournent à ~{} Hz, leur 3e harmonique ({} Hz) se replie dans le spectre (petit drone, moteurs rapides).",
                fixed(fs, 0, false),
                fixed(h, 0, false),
                fixed(3.0 * h, 0, false)
            ),
            rate_fix,
        );
    } else {
        c.add(
            "log_rate",
            "Fréquence d'enregistrement (filtres)",
            OK,
            format!("{} Hz ({loops}).", fixed(fs, 0, false)),
            vec![],
        );
    }
    let pid_need = pid_log_rate_needed(pid_hz);
    let pid_frac = sample_rate_cli(pid_hz, pid_need);
    c.pid_rate_ok = fs >= 0.95 * PID_LOG_RATE_FLOOR_HZ;
    let pid_fix = vec![Fix::new(
        format!(
            "Onglet « Blackbox » → « Fréquence d'enregistrement de la Blackbox » : {} Hz ({pid_frac}).",
            fixed(pid_need, 0, false)
        ),
        format!("set blackbox_sample_rate = {pid_frac}"),
    )];
    if fs >= 0.95 * PID_LOG_RATE_MIN_HZ {
        c.add(
            "pid_log_rate",
            "Fréquence d'enregistrement (PID)",
            OK,
            format!("{} Hz ({loops}).", fixed(fs, 0, false)),
            vec![],
        );
    } else {
        let ok = c.pid_rate_ok;
        c.add(
            "pid_log_rate",
            "Fréquence d'enregistrement (PID)",
            if ok { WARN } else { BLOCK },
            format!(
                "{} Hz : il faut au moins {} Hz pour mesurer la réponse à la milliseconde ",
                fixed(fs, 0, false),
                fixed(PID_LOG_RATE_MIN_HZ, 0, false)
            ) + &if ok {
                "près.".to_string()
            } else {
                format!("près, et {} Hz pour la lire.", fixed(PID_LOG_RATE_FLOOR_HZ, 0, false))
            },
            pid_fix,
        );
    }

    c.unfilt = log.gyro_unfilt.is_some();
    if c.unfilt {
        c.add(
            "gyro_unfilt",
            "Gyro non filtré",
            OK,
            "Présent : l'effet des filtres et leur retard se mesurent.".into(),
            vec![],
        );
    } else if s.family() == "4.4" {
        c.add(
            "gyro_unfilt",
            "Gyro non filtré",
            BLOCK,
            "Absent : en 4.4, seul le mode de débogage GYRO_SCALED l'enregistre.".into(),
            vec![Fix::new(
                "Onglet « Blackbox » → « Mode débug de la Blackbox » : GYRO_SCALED.",
                "set debug_mode = GYRO_SCALED",
            )],
        );
    } else {
        c.add(
            "gyro_unfilt",
            "Gyro non filtré",
            BLOCK,
            "Absent : le champ a été désactivé dans l'enregistrement.".into(),
            vec![Fix::new(
                "Onglet « Blackbox » → « Champs de débogage inclus » : cocher « Gyro (Unfiltered) ».",
                "set blackbox_disable_gyrounfilt = OFF",
            )],
        );
    }

    if s.family() != "4.4" && s.dyn_notch_count > 0 && s.debug_mode != "FFT_FREQ" {
        c.add(
            "debug_mode",
            "Mode de débogage",
            WARN,
            format!(
                "{} : avec FFT_FREQ, le log contient aussi les fréquences suivies par le notch dynamique (courbes du Spectre et diagnostic du bruit plus complets).",
                s.debug_mode
            ),
            vec![Fix::new("Onglet « Blackbox » → « Mode débug de la Blackbox » : FFT_FREQ.", "set debug_mode = FFT_FREQ")],
        );
    }

    if log.motor_hz.is_none() {
        c.add(
            "rpm",
            "Télémétrie RPM",
            WARN,
            "Absente : le filtre RPM est inactif et les harmoniques moteur ne sont qu'estimées.".into(),
            vec![Fix::new(
                "Onglet « Moteurs » → « DShot bi-directionnel » activé (ESC BLHeli_32, Bluejay ou AM32), et « Pôles moteur » juste (aimants de la cloche, souvent 14).",
                "set dshot_bidir = ON",
            )],
        );
    } else {
        c.add(
            "rpm",
            "Télémétrie RPM",
            OK,
            format!("Présente ({} pôles moteur dans les réglages).", s.motor_poles),
            vec![],
        );
    }
}

/// Parties du vol : stationnaire et mouvements (un vol trop court pour être découpé est l'une ou l'autre).
fn flight_parts_item(c: &mut Check) -> Result<(), String> {
    let (log, dur) = (c.log, c.dur());
    let (mut hover, mut moving) = (None, None);
    if dur > 0.0 {
        let p = flight_parts(log, c.t0, c.t1, 1.0, MOVING_SETPOINT_DPS)?;
        (hover, moving) = (p.hover, p.moving);
    }
    if hover.is_none() && moving.is_none() && dur > 0.0 {
        // vol trop court pour être découpé
        if stick_p95_dps(log, c.sel.clone())? >= MOVING_SETPOINT_DPS {
            moving = Some((c.t0, c.t1));
        } else {
            hover = Some((c.t0, c.t1));
        }
    }
    c.hover = hover;
    c.moving = moving;
    let desc = [
        hover.map_or("pas de partie stationnaire".to_string(), |h| {
            format!("stationnaire de {}", span(h))
        }),
        moving.map_or("pas de partie avec mouvements".to_string(), |m| {
            format!("mouvements de {}", span(m))
        }),
    ];
    c.add(
        "parts",
        "Parties du vol",
        INFO,
        capitalize(&desc.join(" ; ")) + ".",
        vec![],
    );
    Ok(())
}

fn up(v: &[f32]) -> Vec<f64> {
    v.iter().map(|&x| x as f64).collect()
}

/// Protocole de la partie avec mouvements : fenêtres de mesure, rates, limite d'angle, mode de vol, axes mêlés, curseur I,
/// atténuation dynamique.
fn protocol_items(c: &mut Check, exact: bool) -> Result<(), String> {
    let (log, fs) = (c.log, c.log.fs);
    let s = &log.settings;
    c.cond = Some(flight_conditions(log, c.sel.clone())?);
    let sv = slider_view(s)?;
    c.sliders = sv.values.clone();
    c.sv_from_header = sv.from_header;
    let Some(mv) = c.moving else {
        return Ok(());
    };
    let msel = log.slice_time(mv.0, mv.1);
    let cond = flight_conditions(log, msel.clone())?;
    c.cond = Some(cond);
    let part = |rows: &otb_log::flightlog::Rows, a: usize| -> Vec<f32> {
        rows.row(a)[clamp_range(msel.start, msel.end, log.n())].to_vec()
    };
    let mut sp_delay = Vec::new();
    let mut sps = Vec::new();
    let mut gys = Vec::new();
    for a in 0..2 {
        let (sp32, gy32) = (part(&log.setpoint, a), part(&log.gyro, a));
        let (sp, gy) = (up(&sp32), up(&gy32));
        sp_delay.push(if std_f32(&sp32) >= 5.0f32 {
            xcorr_delay_ms(&sp, &gy, fs, 100.0, 0)?
        } else {
            f64::NAN
        });
        sps.push(sp);
        gys.push(gy);
    }
    c.sp_delay = sp_delay;
    let mbr: Vec<i64> = log.breaks(msel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let mut windows = Vec::new();
    for a in 0..2 {
        windows.push(if exact {
            step_response(
                &sps[a],
                &gys[a],
                fs,
                &StepOptions::default(),
                &LoopInput::Absent,
                Some(&mbr),
            )?
            .n
        } else {
            input_windows(&sps[a], fs, 2.0, 20.0, 62.5, Some(&mbr))?
        });
    }
    c.windows = windows.clone();
    let n = windows[0].min(windows[1]);
    c.enough = n >= MIN_WINDOWS;
    let approx = if exact {
        ""
    } else {
        " (estimation rapide ; le vrai nombre est un peu plus faible)"
    };
    if n >= SURE_WINDOWS {
        c.add(
            "windows",
            "Fenêtres de mesure",
            OK,
            format!("N = {} en roll, {} en pitch{approx}.", windows[0], windows[1]),
            vec![],
        );
    } else {
        let enough = c.enough;
        c.add(
            "windows",
            "Fenêtres de mesure",
            if enough { WARN } else { BLOCK },
            format!(
                "N = {} en roll, {} en pitch{approx} : il en faut au moins {SURE_WINDOWS} pour une mesure sûre",
                windows[0], windows[1]
            ) + &if enough {
                ".".to_string()
            } else {
                format!(", et {MIN_WINDOWS} pour être lu.")
            },
            vec![Fix::new(
                "Au moins 20 s de wobbles roll et pitch mêlés (environ 12 fenêtres par seconde de mouvement).",
                "",
            )],
        );
    }

    match log_rates(s) {
        None => c.add(
            "rates",
            "Rates de log",
            WARN,
            "Le profil de rates n'est pas de type ACTUAL.".into(),
            vec![log_rates_fix()],
        ),
        Some((center, mx, expo)) => {
            let good = center.iter().all(|&x| x == LOG_RATE_DPS)
                && mx.iter().all(|&m| m == LOG_RATE_DPS)
                && !expo.iter().any(|&e| e != 0);
            let join = |v: &[i64]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("/");
            let txt = format!(
                "ACTUAL, centre {}, maximum {} °/s, expo {} (roll/pitch/yaw)",
                join(&center),
                join(&mx),
                join(&expo)
            );
            c.add(
                "rates",
                "Rates de log",
                if good { OK } else { WARN },
                txt + &if good {
                    ".".to_string()
                } else {
                    format!(" : le protocole demande {LOG_RATE_DPS} / {LOG_RATE_DPS} / 0 sur les trois axes.")
                },
                if good { vec![] } else { vec![log_rates_fix()] },
            );
        }
    }

    // mode Angle (le firmware y coupe le feedforward) ou Acro avec le feedforward à 0 : même réponse lue
    let share = cond.angle_share;
    let ff_zero = cond.ff.0.max(cond.ff.1) == 0;
    let acro = !share.is_nan() && !cond.angle(); // modes enregistrés, pas en mode Angle : la limite d'angle ne joue pas
    let lim = s.get_int("angle_limit", -1);
    let lim_fix = vec![Fix::new(
        format!("Onglet « Réglages de PID » → « Limite de l'Angle » : {LOG_ANGLE_LIMIT}."),
        format!(
            "set {} = {LOG_ANGLE_LIMIT}",
            if s.family() == "4.4" {
                "level_limit"
            } else {
                "angle_limit"
            }
        ),
    )];
    if acro {
    } else if lim < 0 {
        c.add(
            "angle_limit",
            "Limite d'angle",
            INFO,
            format!("Non enregistrée dans le log (Betaflight 4.4) : vérifiez qu'elle vaut {LOG_ANGLE_LIMIT}°."),
            lim_fix,
        );
    } else {
        c.add(
            "angle_limit",
            "Limite d'angle",
            if lim == LOG_ANGLE_LIMIT { OK } else { WARN },
            format!("{lim}°")
                + &if lim == LOG_ANGLE_LIMIT {
                    ".".to_string()
                } else {
                    format!(" : le protocole demande {LOG_ANGLE_LIMIT}°.")
                },
            if lim == LOG_ANGLE_LIMIT { vec![] } else { lim_fix },
        );
    }

    let mode_title = "Mode Angle ou feedforward à 0";
    if cond.angle() {
        c.add(
            "flight_mode",
            mode_title,
            OK,
            format!(
                "{} de la partie avec mouvements en mode Angle (le feedforward y est coupé).",
                percent(share, 0)
            ),
            vec![],
        );
    } else if ff_zero {
        c.add(
            "flight_mode",
            mode_title,
            OK,
            "Acro avec le feedforward à 0 en roll et en pitch.".into(),
            vec![],
        );
    } else {
        // Acro avec le feedforward : vol d'une série Feedforward (finitions), pas d'une série PID ; simple info,
        // l'usage « série PID » dit pourquoi il n'est pas lu comme tel
        let nan = share.is_nan();
        c.add(
            "flight_mode",
            mode_title,
            if nan { WARN } else { INFO },
            format!(
                "{}, feedforward actif (F roll {}, pitch {}) : vol d'une série Feedforward. Pour une série PID, volez en mode Angle ou mettez le feedforward à 0 (il fausse la réponse).",
                if nan { "Modes de vol absents du log" } else { "Acro" },
                cond.ff.0,
                cond.ff.1
            ),
            vec![
                Fix::new("Série PID : onglet « Modes », mode ANGLE sur un interrupteur, activé pendant les mouvements.", ""),
                Fix::new(format!("Ou en Acro : curseur « {} » à 0.", configurator_slider("Stick Resp.")), "set simplified_feedforward_gain = 0"),
            ],
        );
    }

    let mixed = axes_mixed_share(log, msel, 20.0);
    c.add(
        "axes_mixed",
        "Roll et pitch mêlés",
        if mixed >= AXES_MIXED_MIN { OK } else { WARN },
        format!(
            "Les deux axes bougent ensemble {} du temps en mouvement",
            percent(mixed, 0)
        ) + &if mixed >= AXES_MIXED_MIN {
            ".".to_string()
        } else {
            format!(
                " (repère {}) : bougez le roll et le pitch en même temps, au hasard, plutôt qu'un axe puis l'autre.",
                percent(AXES_MIXED_MIN, 0)
            )
        },
        vec![],
    );

    c.ff_ok = !cond.ff_active();
    let (lo, hi) = I_SLIDER_RANGE;
    c.i_ok = lo <= cond.i_slider && cond.i_slider <= hi;
    if !c.i_ok {
        c.add(
            "i_slider",
            "Curseur I",
            WARN,
            format!(
                "{} : pendant les séries PID, le terme I change la forme de la réponse ; le garder entre 0,2 et 0,3, puis le remonter en finitions.",
                nn(cond.i_slider, 2)
            ),
            vec![Fix::new(
                format!("Curseur « {} » : 0,25 (Mode Expert).", configurator_slider("Drift-Wobble")),
                "set simplified_i_gain = 25",
            )],
        );
    }
    let dd = sv
        .values
        .iter()
        .find(|(k, _)| *k == "Dynamic D.")
        .map(|(_, v)| *v)
        .ok_or("KeyError : 'Dynamic D.'")?;
    if sv.from_header && dd > DYN_DAMPING_MAX {
        let dd_key = if s.raw.contains_key("simplified_d_max_gain") {
            "simplified_d_max_gain"
        } else {
            "simplified_dmax_gain"
        };
        c.add(
            "dyn_damping",
            "Atténuation dynamique",
            WARN,
            format!(
                "{} : pendant les séries, le D max change la réponse selon la vitesse du mouvement ; le mettre à 0, puis le régler en finitions.",
                nn(dd, 2)
            ),
            vec![Fix::new(format!("Curseur « {} » : 0 (Mode Expert).", configurator_slider("Dynamic D.")), format!("set {dd_key} = 0"))],
        );
    }
    Ok(())
}

/// Quel que soit le vol : réglage simplifié, crash, trous dans l'enregistrement.
fn general_items(c: &mut Check) -> Result<(), String> {
    let log = c.log;
    if !c.sv_from_header {
        c.add(
            "simplified",
            "Réglage simplifié",
            WARN,
            "Désactivé : les curseurs du configurateur ne pilotent pas les PID ; OpenTuneBox estime leurs positions à partir des PID.".into(),
            vec![Fix::new(
                "Onglet « Réglages de PID » → curseurs, « Mode » : RPY. Attention : les PID sont alors recalculés à partir des curseurs (notez-les avant).",
                "set simplified_pids_mode = RPY",
            )],
        );
    }

    if let Some(crash) = crash_time(log, 300.0, 60.0)? {
        c.add(
            "crash",
            "Crash",
            INFO,
            format!("Crash vers {} s : la période d'analyse s'arrête avant.", n1(crash)),
            vec![],
        );
    }
    let (n_gaps, lost_s) = log.recording_gaps(c.sel.clone(), 4.0);
    if n_gaps > 0 {
        // fenêtres qui chevauchent un trou écartées par les analyses (D-020)
        let dur = c.dur();
        c.add(
            "gaps",
            "Trous dans l'enregistrement",
            INFO,
            format!(
                "{} : les fenêtres qui chevauchent un trou sont écartées, il reste moins de données à analyser.",
                gaps_text(n_gaps as i64, lost_s, dur)
            ),
            vec![Fix::new(
                "Si c'est fréquent : carte SD plus rapide, ou fréquence d'enregistrement plus basse.",
                "",
            )],
        );
    }
    Ok(())
}

fn first_problem(items: &[CheckItem], keys: &[&str]) -> String {
    for it in items {
        if keys.contains(&it.key.as_str()) && (it.status == WARN || it.status == BLOCK) {
            return format!("« {} » à corriger (voir ci-dessous).", it.title);
        }
    }
    String::new()
}

/// Verdict par usage : identifier le bruit, juger le retard, vol d'une série PID, vol d'une série Feedforward.
fn usages(c: &Check) -> Vec<Usage> {
    let cond = c.cond.expect("flight_conditions calculé");
    let items = &c.items;
    let base_ok = c.version_ok && c.rate_ok && c.unfilt;
    let base_why = first_problem(items, &["version", "log_rate", "gyro_unfilt"]);
    let aliased = if c.harm_ok {
        ""
    } else {
        " Attention : 3e harmonique moteur repliée (voir la fréquence d'enregistrement)."
    };
    let (hover, moving) = (c.hover, c.moving);
    let mut usages = vec![
        Usage {
            key: "noise".into(),
            label: "Identifier le bruit (filtres)".into(),
            ok: base_ok && hover.is_some(),
            reason: if !base_ok {
                base_why.clone()
            } else if let Some(h) = hover {
                format!("partie stationnaire de {}.", span(h)) + aliased
            } else {
                "pas de partie stationnaire : commencez le vol par 20 s de stationnaire avec des montées de gaz.".into()
            },
            shown: true,
        },
        Usage {
            key: "delay".into(),
            label: "Juger le retard des filtres".into(),
            ok: base_ok && moving.is_some(),
            reason: if !base_ok {
                base_why
            } else if let Some(m) = moving {
                format!("partie avec mouvements de {}.", span(m))
            } else {
                "pas de partie avec mouvements : terminez le vol par 20 s de wobbles en mode Angle.".into()
            },
            shown: true,
        },
    ];
    let series_ok = c.version_ok && c.pid_rate_ok && moving.is_some() && c.enough && c.ff_ok && c.i_ok;
    let protocol: Vec<String> = items
        .iter()
        .filter(|it| {
            [
                "pid_log_rate",
                "windows",
                "rates",
                "angle_limit",
                "axes_mixed",
                "dyn_damping",
            ]
            .contains(&it.key.as_str())
                && it.status == WARN
        })
        .map(|it| it.title.to_lowercase())
        .collect();
    let reason = if series_ok {
        if protocol.is_empty() {
            "conditions respectées.".to_string()
        } else {
            format!("lisible, mais à améliorer : {}.", protocol.join(", "))
        }
    } else if moving.is_none() {
        "pas de partie avec mouvements.".to_string()
    } else {
        let blocking: Vec<String> = [
            ("Version du firmware", !c.version_ok),
            ("Fréquence d'enregistrement (PID)", !c.pid_rate_ok),
            ("Fenêtres de mesure", !c.enough),
            ("Curseur I", !c.i_ok),
        ]
        .iter()
        .filter(|(_, bad)| *bad)
        .map(|(t, _)| format!("« {t} »"))
        .collect();
        let mut reason = if blocking.is_empty() {
            String::new()
        } else {
            format!("à corriger : {} (voir ci-dessous).", blocking.join(", "))
        };
        if !c.ff_ok {
            // `("… " + reason).strip()` de Python
            reason = format!(
                "feedforward actif en Acro (vol d'une série Feedforward) ; pour une série PID, mode Angle ou feedforward à 0. {reason}"
            )
            .trim_matches(crate::profiles::is_space)
            .to_string();
        }
        reason
    };
    usages.push(Usage {
        key: "series".into(),
        label: "Vol d'une série PID".into(),
        ok: series_ok,
        reason,
        shown: true,
    });
    // série Feedforward (finitions) : en Acro, feedforward voulu ; le retard consigne → gyro se lit sur la partie avec mouvements
    let ffs_ok = c.version_ok && c.pid_rate_ok && moving.is_some() && c.enough && !cond.angle();
    let ff_why = if ffs_ok {
        "conditions respectées."
    } else if moving.is_none() {
        "pas de partie avec mouvements."
    } else if cond.angle() {
        "en mode Angle : le firmware y coupe le feedforward, volez en Acro."
    } else {
        "pas assez de fenêtres de mesure ou fréquence d'enregistrement trop basse (voir ci-dessous)."
    };
    // affiché seulement avec le feedforward actif : un vol à 0 sert aussi de 1er vol de la série, sans le redire
    usages.push(Usage {
        key: "ff_series".into(),
        label: "Vol d'une série Feedforward".into(),
        ok: ffs_ok,
        reason: ff_why.into(),
        shown: cond.ff.0.max(cond.ff.1) > 0,
    });
    usages
}

/// Repères de fin de la phase Filtres : bruit sur la partie stationnaire (échelle de PIDtoolbox), retard des filtres sur la partie avec
/// mouvements, objectif de retard de référence pour la taille.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterTargets {
    pub gyro_ptb_db: f64,
    pub dterm_ptb_db: f64,
    pub gyro_delay_ms: f64,
    pub dterm_delay_ms: f64,
    /// (gyro, D) tels que lus par PIDtoolbox.
    pub delay_goal_ms: Option<(f64, f64)>,
}

impl FilterTargets {
    /// (repère, atteint ? (`None` : non mesurable), texte).
    pub fn checks(&self) -> Vec<(String, Option<bool>, String)> {
        let fin = |x: f64| x.is_finite();
        let mut out = vec![
            (
                "Gyro filtré".to_string(),
                if fin(self.gyro_ptb_db) {
                    Some(self.gyro_ptb_db <= GYRO_PTB_MAX_DB)
                } else {
                    None
                },
                format!("{} (repère : sous {})", db(self.gyro_ptb_db), db(GYRO_PTB_MAX_DB)),
            ),
            (
                "Terme D".to_string(),
                if fin(self.dterm_ptb_db) {
                    Some(self.dterm_ptb_db <= DTERM_PTB_MAX_DB)
                } else {
                    None
                },
                format!("{} (repère : au plus {})", db(self.dterm_ptb_db), db(DTERM_PTB_MAX_DB)),
            ),
        ];
        if let Some((g, d)) = self.delay_goal_ms {
            out.push((
                "Retard gyro".to_string(),
                if fin(self.gyro_delay_ms) {
                    Some(self.gyro_delay_ms <= g)
                } else {
                    None
                },
                format!("{} ms (objectif : {} ms)", nn(self.gyro_delay_ms, 2), nn(g, 2)),
            ));
            out.push((
                "Retard D".to_string(),
                if fin(self.dterm_delay_ms) {
                    Some(self.dterm_delay_ms <= d + D_DELAY_PTB_OFFSET_MS)
                } else {
                    None
                },
                format!(
                    "{} ms (objectif : {} ms lus par PIDtoolbox, soit {} ms ici)",
                    nn(self.dterm_delay_ms, 2),
                    nn(d, 2),
                    nn(d + D_DELAY_PTB_OFFSET_MS, 2)
                ),
            ));
        }
        out
    }

    pub fn reached(&self) -> Option<bool> {
        let vals: Vec<Option<bool>> = self.checks().into_iter().map(|(_, ok, _)| ok).collect();
        if vals.contains(&Some(false)) {
            return Some(false);
        }
        if vals.contains(&None) { None } else { Some(true) }
    }
}

/// Repères de la phase Filtres (calcul de quelques secondes : à lancer en arrière-plan).
pub fn filter_targets(
    log: &FlightLog,
    hover: Option<(f64, f64)>,
    moving: Option<(f64, f64)>,
    inch: Option<f64>,
) -> Result<Option<FilterTargets>, String> {
    let Some(h) = hover else {
        return Ok(None);
    };
    let hsel = log.slice_time(h.0, h.1);
    let fs = log.fs;
    let hi = (500.0f64).min(0.95 * fs / 2.0);
    let hbr: Vec<i64> = log.breaks(hsel.clone(), 4.0).iter().map(|&b| b as i64).collect();
    let part = |rows: &otb_log::flightlog::Rows, a: usize| -> Vec<f32> {
        rows.row(a)[clamp_range(hsel.start, hsel.end, log.n())].to_vec()
    };
    // max(...) de Python sur des NaN : le premier, sauf si le suivant est strictement plus grand
    let mut g = band_peak_ptb_db(&part(&log.gyro, 0), fs, 80.0, hi, Some(&hbr))?;
    let g1 = band_peak_ptb_db(&part(&log.gyro, 1), fs, 80.0, hi, Some(&hbr))?;
    if g1 > g {
        g = g1;
    }
    let mut d = f64::NAN;
    let mut first = true;
    for a in 0..2 {
        if log.d_term.row(a).iter().any(|&v| v != 0.0) {
            let v = band_peak_ptb_db(&part(&log.d_term, a), fs, 80.0, hi, Some(&hbr))?;
            if first || v > d {
                d = v;
            }
            first = false;
        }
    }
    let (mut gd, mut dd) = (f64::NAN, f64::NAN);
    if let Some(m) = moving
        && log.gyro_unfilt.is_some()
    {
        let dl = compute_delays(log, log.slice_time(m.0, m.1))?;
        (gd, dd) = (dl.gyro_ms, dl.dterm_ms);
    }
    Ok(Some(FilterTargets {
        gyro_ptb_db: g,
        dterm_ptb_db: d,
        gyro_delay_ms: gd,
        dterm_delay_ms: dd,
        delay_goal_ms: filter_delay_target_ms(inch),
    }))
}

/// Contrôle d'un groupe de logs du même drone : peut-on les lire comme une série ?
#[derive(Debug, Clone, PartialEq)]
pub struct GroupCheck {
    pub craft: String,
    pub labels: Vec<String>,
    pub status: &'static str,
    pub detail: String,
}

fn delays_text(d: Option<&[f64]>) -> String {
    match d {
        Some(v) if v.iter().any(|x| x.is_finite()) => {
            v.iter()
                .map(|&x| if x.is_finite() { n1(x) } else { "–".to_string() })
                .collect::<Vec<_>>()
                .join(" / ")
                + " ms"
        }
        _ => "–".to_string(),
    }
}

/// Logs du même drone (nom et carte) : séries qu'ils forment (un seul curseur changé, mêmes filtres, comme la Réponse indicielle :
/// `step_series.find_series`), vols identiques, vols hors série. Série Feedforward : retard consigne → gyro de chaque vol (`sp_delays`,
/// par libellé : `LogCheck::sp_delay_ms`).
pub fn check_logs(
    logs: &[(String, &FlightLog)],
    sp_delays: &BTreeMap<String, Vec<f64>>,
) -> Result<Vec<GroupCheck>, String> {
    // groupes dans l'ordre de première apparition (dictionnaire de Python)
    let mut groups: Vec<((String, String), Vec<usize>)> = Vec::new();
    for (i, (_, lg)) in logs.iter().enumerate() {
        let key = (lg.settings.craft_name.clone(), lg.settings.board.clone());
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, m)) => m.push(i),
            None => groups.push((key, vec![i])),
        }
    }
    let mut out = Vec::new();
    for ((craft, board), members) in &groups {
        if members.len() < 2 {
            continue;
        }
        let name = if !craft.is_empty() {
            craft.clone()
        } else if !board.is_empty() {
            board.clone()
        } else {
            "drone sans nom".to_string()
        };
        let labels: Vec<String> = members.iter().map(|&i| logs[i].0.clone()).collect();
        let mut flights = Vec::new();
        for &i in members {
            let lg = logs[i].1;
            flights.push(FlightStep {
                label: logs[i].0.clone(),
                sliders: slider_view(&lg.settings)?.values,
                filters: filter_signature(&lg.settings),
                // `None` en Python : `find_series` ne lit pas les conditions
                conditions: FlightConditions {
                    ff: (0, 0),
                    angle_share: f64::NAN,
                    i_slider: f64::NAN,
                },
                metrics: Vec::new(),
                n: Vec::new(),
                sp_delay_ms: Vec::new(),
                loops: Vec::new(),
            });
        }
        let mut in_series: Vec<bool> = vec![false; flights.len()];
        for sr in find_series(&flights)? {
            for &i in &sr.flights {
                in_series[i] = true;
            }
            let mut uniq: Vec<f64> = Vec::new();
            for &v in &sr.values {
                if !uniq.contains(&v) {
                    uniq.push(v);
                }
            }
            uniq.sort_by(|a, b| a.total_cmp(b));
            let vals = uniq.iter().map(|&v| nn(v, 2)).collect::<Vec<_>>().join(" → ");
            let who = sr
                .flights
                .iter()
                .map(|&i| labels[i].as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let read = if sr.key == FF_KEY {
                // réponse indicielle faussée par le feedforward : retards ici et dans l'Assistant
                let per: Vec<String> = sr
                    .flights
                    .iter()
                    .zip(&sr.values)
                    .map(|(&i, &v)| {
                        format!(
                            "{} → {}",
                            nn(v, 2),
                            delays_text(sp_delays.get(&labels[i]).map(|d| d.as_slice()))
                        )
                    })
                    .collect();
                format!(
                    "retard consigne → gyro roll / pitch : {} (détail dans l'Assistant).",
                    per.join(" ; ")
                )
            } else {
                "à lire dans « Réponse indicielle » → Diagnostic du réglage.".to_string()
            };
            out.push(GroupCheck {
                craft: name.clone(),
                labels: sr.flights.iter().map(|&i| labels[i].clone()).collect(),
                status: OK,
                detail: format!("Série « {} » : {vals} ({who}) ; {read}", configurator_slider(sr.key)),
            });
        }
        let mut same: Vec<(Sig, Vec<f64>, Vec<usize>)> = Vec::new();
        for (i, f) in flights.iter().enumerate() {
            let sl: Vec<f64> = PTB_CATEGORIES
                .iter()
                .map(|(cat, _)| {
                    f.sliders
                        .iter()
                        .find(|(k, _)| k == cat)
                        .map(|(_, v)| round_digits(*v, 2))
                        .unwrap_or(f64::NAN)
                })
                .collect();
            match same
                .iter_mut()
                .find(|(sig, v, _)| *sig == f.filters && v.len() == sl.len() && v.iter().zip(&sl).all(|(a, b)| a == b))
            {
                Some((_, _, idx)) => idx.push(i),
                None => same.push((f.filters.clone(), sl, vec![i])),
            }
        }
        for (_, _, idx) in &same {
            if idx.len() > 1 {
                for &i in idx {
                    in_series[i] = true;
                }
                out.push(GroupCheck {
                    craft: name.clone(),
                    labels: idx.iter().map(|&i| labels[i].clone()).collect(),
                    status: INFO,
                    detail: format!(
                        "Mêmes curseurs et mêmes filtres : {}.",
                        idx.iter().map(|&i| labels[i].as_str()).collect::<Vec<_>>().join(", ")
                    ),
                });
            }
        }
        let alone: Vec<String> = (0..flights.len())
            .filter(|&i| !in_series[i])
            .map(|i| labels[i].clone())
            .collect();
        if !alone.is_empty() {
            out.push(GroupCheck {
                craft: name.clone(),
                detail: format!(
                    "Hors série : {} (par rapport aux autres vols, plus d'un curseur ou les filtres changent). Une série ne change qu'un curseur (pas de 0,2 ; 0,5 pour le feedforward), filtres identiques.",
                    alone.join(", ")
                ),
                labels: alone,
                status: INFO,
            });
        }
    }
    Ok(out)
}
