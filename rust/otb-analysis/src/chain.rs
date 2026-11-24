//! Chaîne de filtres d'un firmware décrite en **données** (D-062, lot 23 du plan) : la seule description de l'ordre des
//! filtres, lue par la réponse en fréquence (`bf_filters::gyro_chain`, `dterm_chain`) et par la simulation dans le temps
//! (étages `bf_filters::GyroStage`, rejeu de la page Filtres, contrôle par étage, suivi du notch dynamique, terme D).
//!
//! Le fichier `chains/betaflight.json` est intégré au programme à la compilation : rien n'est lu au lancement, et changer
//! de chaîne demande de recompiler (D-062). Il donne :
//! - `gyro`, `dterm` : les étapes **dans l'ordre du firmware** ; chacune nomme sa brique (`lowpass`, `static_notch`,
//!   `rpm_notch`, `dyn_notch`), sa cadence (`gyro` ou `pid`) et, pour un passe-bas ou un notch statique, le réglage
//!   qu'elle lit. Une étape est en service ou non selon ces réglages, jamais selon la description ;
//! - `taps` (les **prises**) : où le log enregistre chaque signal, `before:<étape>`, `after:<étape>` ou `end:<chaîne>`.
//!   Un étage de la simulation est l'intervalle entre deux prises ; la prise du début de la chaîne gyro dit quel signal
//!   le rejeu prend en entrée (sans lui, la page Filtres ne simule rien) ;
//! - `derivative` : où la chaîne D dérive (`after` : la dernière étape filtrée avant la dérivée) et le signe.
//!
//! Ce que la description ne dit pas, les briques le portent (`bf_filters`) : formules des coefficients, centres du notch
//! dynamique, poids des notchs RPM. ⚠️ L'ordre et la cadence sont **lus ici** : les écrire en dur ailleurs refait deux
//! descriptions de la même chaîne, qui divergent sans qu'aucun test ne le voie (lot 23).

use otb_log::flightlog::{FlightLog, Rows};
use otb_log::settings::{Lowpass, Settings};
use serde_json::{Map, Value};
use std::ops::Range;
use std::sync::OnceLock;

/// Cadence d'une étape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rate {
    /// Cadence d'échantillonnage du gyro (`looptime`).
    Gyro,
    /// Cadence de la boucle PID (`looptime × pid_process_denom`).
    Pid,
}

impl Rate {
    /// Fréquence (Hz) de cette cadence d'après l'en-tête.
    pub fn hz(self, s: &Settings) -> f64 {
        match self {
            Rate::Gyro => s.gyro_rate_hz(),
            Rate::Pid => s.pid_rate_hz(),
        }
    }
}

/// Passe-bas lu par une étape `lowpass`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LowpassSetting {
    GyroLpf1,
    GyroLpf2,
    DtermLpf1,
    DtermLpf2,
}

impl LowpassSetting {
    pub fn get(self, s: &Settings) -> &Lowpass {
        match self {
            LowpassSetting::GyroLpf1 => &s.gyro_lpf1,
            LowpassSetting::GyroLpf2 => &s.gyro_lpf2,
            LowpassSetting::DtermLpf1 => &s.dterm_lpf1,
            LowpassSetting::DtermLpf2 => &s.dterm_lpf2,
        }
    }
}

/// Notch statique lu par une étape `static_notch` : (centre, coupure) en Hz, `None` s'il n'est pas dans l'en-tête.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotchSetting {
    /// `gyro_notch1_hz` / `gyro_notch1_cutoff` (indice 0) ou `gyro_notch2_*` (indice 1).
    Gyro(usize),
    Dterm,
}

impl NotchSetting {
    pub fn get(self, s: &Settings) -> Option<(i64, i64)> {
        match self {
            NotchSetting::Gyro(i) => s.gyro_notches.get(i).copied(),
            NotchSetting::Dterm => Some(s.dterm_notch),
        }
    }
}

/// Limite au-dessus de laquelle un notch statique est ramené (ou coupé), comme le firmware à l'initialisation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotchLimit {
    /// `0,95 × cadence PID / 2`, calculé comme l'étalon Python (`1e6 / 2 / looptime / pid_denom × 0,95`) ; refusé si
    /// `looptime` ou `pid_denom` vaut 0. Notchs gyro.
    Pid95,
    /// `cadence PID / 2`. Notch D.
    Pid,
}

impl NotchLimit {
    pub fn hz(self, s: &Settings) -> Result<f64, String> {
        match self {
            NotchLimit::Pid95 => {
                if s.looptime_us == 0 || s.pid_denom == 0 {
                    return Err("float division by zero".into());
                }
                Ok(1e6 / 2.0 / s.looptime_us as f64 / s.pid_denom as f64 * 0.95)
            }
            NotchLimit::Pid => Ok(s.pid_rate_hz() / 2.0),
        }
    }
}

/// Brique de calcul d'une étape (son arithmétique est dans `bf_filters`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brick {
    Lowpass(LowpassSetting),
    StaticNotch(NotchSetting, NotchLimit),
    /// Notchs RPM : un par moteur et par harmonique (réglages `rpm_filter_*`).
    RpmNotch,
    /// Notchs dynamiques (réglages `dyn_notch_*`).
    DynNotch,
}

/// Une étape de la chaîne.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub id: String,
    pub brick: Brick,
    pub rate: Rate,
}

/// Les deux chaînes d'une description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Gyro,
    Dterm,
}

/// Signal enregistré par le log à une prise de la chaîne.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Signal {
    /// `gyroUnfilt`.
    GyroUnfilt,
    /// Point `i` du debug `GYRO_SAMPLE` : 0 avant le LPF2, 1 après lui, 2 après les notchs RPM, 3 après le LPF1.
    GyroSample(usize),
    /// Gyro d'avant les notchs dynamiques, dans le debug `FFT_FREQ`.
    FftFreqBeforeDynNotch,
    /// `gyroADC` du log (gyro filtré).
    Gyro,
    /// `axisD` du log (terme D).
    AxisD,
}

impl Signal {
    fn parse(s: &str) -> Result<Signal, String> {
        Ok(match s {
            "gyro_unfilt" => Signal::GyroUnfilt,
            "gyro_sample.before_lpf2" => Signal::GyroSample(0),
            "gyro_sample.after_lpf2" => Signal::GyroSample(1),
            "gyro_sample.after_rpm" => Signal::GyroSample(2),
            "gyro_sample.after_static" => Signal::GyroSample(3),
            "fft_freq.before_dyn_notch" => Signal::FftFreqBeforeDynNotch,
            "gyro" => Signal::Gyro,
            "axis_d" => Signal::AxisD,
            _ => return Err(format!("signal inconnu : {s}")),
        })
    }
}

/// Où le log enregistre un signal : chaîne, et nombre d'étapes de cette chaîne appliquées avant lui (0 = son entrée).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tap {
    pub signal: Signal,
    pub chain: Which,
    pub at: usize,
}

/// Dérivée du terme D : appliquée après les `at` premières étapes de la chaîne D, multipliée par `sign`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Derivative {
    pub at: usize,
    pub sign: f64,
}

/// Description complète d'une chaîne de firmware.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainDesc {
    pub firmware: String,
    pub gyro: Vec<Step>,
    pub dterm: Vec<Step>,
    pub taps: Vec<Tap>,
    pub derivative: Derivative,
}

const BETAFLIGHT_JSON: &str = include_str!("../chains/betaflight.json");

/// Description de la chaîne Betaflight, lue une fois. ⚠️ Le fichier est intégré au programme : un fichier invalide
/// arrête tout calcul de filtres, et le test `chain::la_description_betaflight_se_lit` le refuse avant.
pub fn betaflight() -> &'static ChainDesc {
    static DESC: OnceLock<ChainDesc> = OnceLock::new();
    DESC.get_or_init(|| match ChainDesc::parse(BETAFLIGHT_JSON) {
        Ok(d) => d,
        Err(e) => panic!("chains/betaflight.json invalide : {e}"),
    })
}

fn str_field<'a>(o: &'a Map<String, Value>, key: &str, ctx: &str) -> Result<&'a str, String> {
    o.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{ctx} : champ « {key} » absent ou pas une chaîne"))
}

fn only_keys(o: &Map<String, Value>, allowed: &[&str], ctx: &str) -> Result<(), String> {
    match o.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(k) => Err(format!("{ctx} : champ inconnu « {k} »")),
        None => Ok(()),
    }
}

fn parse_step(v: &Value, ctx: &str) -> Result<Step, String> {
    let o = v
        .as_object()
        .ok_or_else(|| format!("{ctx} : une étape doit être un objet"))?;
    let id = str_field(o, "id", ctx)?.to_string();
    let ctx = format!("{ctx} « {id} »");
    let rate = match str_field(o, "rate", &ctx)? {
        "gyro" => Rate::Gyro,
        "pid" => Rate::Pid,
        r => return Err(format!("{ctx} : cadence inconnue « {r} »")),
    };
    let setting = || str_field(o, "setting", &ctx);
    let brick = match str_field(o, "brick", &ctx)? {
        "lowpass" => {
            only_keys(o, &["id", "brick", "rate", "setting"], &ctx)?;
            Brick::Lowpass(match setting()? {
                "gyro_lpf1" => LowpassSetting::GyroLpf1,
                "gyro_lpf2" => LowpassSetting::GyroLpf2,
                "dterm_lpf1" => LowpassSetting::DtermLpf1,
                "dterm_lpf2" => LowpassSetting::DtermLpf2,
                r => return Err(format!("{ctx} : passe-bas inconnu « {r} »")),
            })
        }
        "static_notch" => {
            only_keys(o, &["id", "brick", "rate", "setting", "nyquist"], &ctx)?;
            let notch = match setting()? {
                "gyro_notch1" => NotchSetting::Gyro(0),
                "gyro_notch2" => NotchSetting::Gyro(1),
                "dterm_notch" => NotchSetting::Dterm,
                r => return Err(format!("{ctx} : notch inconnu « {r} »")),
            };
            let limit = match str_field(o, "nyquist", &ctx)? {
                "pid_95" => NotchLimit::Pid95,
                "pid" => NotchLimit::Pid,
                r => return Err(format!("{ctx} : limite de notch inconnue « {r} »")),
            };
            Brick::StaticNotch(notch, limit)
        }
        "rpm_notch" => {
            only_keys(o, &["id", "brick", "rate"], &ctx)?;
            Brick::RpmNotch
        }
        "dyn_notch" => {
            only_keys(o, &["id", "brick", "rate"], &ctx)?;
            Brick::DynNotch
        }
        b => return Err(format!("{ctx} : brique inconnue « {b} »")),
    };
    Ok(Step { id, brick, rate })
}

fn parse_steps(root: &Map<String, Value>, key: &str) -> Result<Vec<Step>, String> {
    root.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("champ « {key} » absent ou pas une liste"))?
        .iter()
        .map(|v| parse_step(v, &format!("chaîne {key}, étape")))
        .collect()
}

impl ChainDesc {
    /// Lit et vérifie une description (format de `chains/betaflight.json`, voir l'en-tête du module).
    pub fn parse(text: &str) -> Result<ChainDesc, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| format!("JSON illisible : {e}"))?;
        let root = v.as_object().ok_or("la description doit être un objet")?;
        only_keys(
            root,
            &["firmware", "gyro", "dterm", "taps", "derivative"],
            "description",
        )?;
        let firmware = str_field(root, "firmware", "description")?.to_string();
        let gyro = parse_steps(root, "gyro")?;
        let dterm = parse_steps(root, "dterm")?;
        let mut ids: Vec<&str> = gyro.iter().chain(&dterm).map(|s| s.id.as_str()).collect();
        ids.sort_unstable();
        if let Some(w) = ids.windows(2).find(|w| w[0] == w[1]) {
            return Err(format!("étape « {} » décrite deux fois", w[0]));
        }
        let mut desc = ChainDesc {
            firmware,
            gyro,
            dterm,
            taps: Vec::new(),
            derivative: Derivative { at: 0, sign: 0.0 },
        };
        let taps = root
            .get("taps")
            .and_then(Value::as_object)
            .ok_or("champ « taps » absent ou pas un objet")?;
        for (name, pos) in taps {
            let signal = Signal::parse(name)?;
            let pos = pos
                .as_str()
                .ok_or_else(|| format!("prise « {name} » : position pas une chaîne"))?;
            let (chain, at) = desc.position(pos).map_err(|e| format!("prise « {name} » : {e}"))?;
            desc.taps.push(Tap { signal, chain, at });
        }
        let d = root
            .get("derivative")
            .and_then(Value::as_object)
            .ok_or("champ « derivative » absent ou pas un objet")?;
        only_keys(d, &["after", "sign"], "derivative")?;
        let after = str_field(d, "after", "derivative")?;
        let at = match desc.locate(after) {
            Some((Which::Dterm, i)) => i + 1,
            _ => {
                return Err(format!(
                    "derivative : « {after} » n'est pas une étape de la chaîne dterm"
                ));
            }
        };
        let sign = match d.get("sign").and_then(Value::as_i64) {
            Some(s @ (-1 | 1)) => s as f64,
            _ => return Err("derivative : « sign » doit valoir -1 ou 1".into()),
        };
        desc.derivative = Derivative { at, sign };
        Ok(desc)
    }

    /// Les étapes d'une chaîne.
    pub fn steps(&self, which: Which) -> &[Step] {
        match which {
            Which::Gyro => &self.gyro,
            Which::Dterm => &self.dterm,
        }
    }

    /// Chaîne et rang de l'étape `id`.
    fn locate(&self, id: &str) -> Option<(Which, usize)> {
        [Which::Gyro, Which::Dterm]
            .into_iter()
            .find_map(|w| self.steps(w).iter().position(|s| s.id == id).map(|i| (w, i)))
    }

    /// `before:<étape>`, `after:<étape>`, `end:gyro` ou `end:dterm` → (chaîne, nombre d'étapes appliquées).
    fn position(&self, pos: &str) -> Result<(Which, usize), String> {
        let (kind, name) = pos
            .split_once(':')
            .ok_or_else(|| format!("position « {pos} » sans « : »"))?;
        match kind {
            "end" => match name {
                "gyro" => Ok((Which::Gyro, self.gyro.len())),
                "dterm" => Ok((Which::Dterm, self.dterm.len())),
                _ => Err(format!("chaîne inconnue « {name} »")),
            },
            "before" | "after" => {
                let (w, i) = self.locate(name).ok_or_else(|| format!("étape inconnue « {name} »"))?;
                Ok((w, if kind == "after" { i + 1 } else { i }))
            }
            _ => Err(format!("position « {pos} » : « before », « after » ou « end » attendu")),
        }
    }

    /// Prise du signal `signal`, si la description en a une.
    pub fn tap(&self, signal: Signal) -> Option<&Tap> {
        self.taps.iter().find(|t| t.signal == signal)
    }

    /// Rangs des étapes gyro situées entre les prises `from` et `to`. Erreur si l'une manque ou si elles ne sont pas sur
    /// la chaîne gyro dans cet ordre.
    pub fn gyro_between(&self, from: Signal, to: Signal) -> Result<Range<usize>, String> {
        let at = |sig: Signal| -> Result<usize, String> {
            match self.tap(sig) {
                Some(t) if t.chain == Which::Gyro => Ok(t.at),
                _ => Err(format!("{} : pas de prise {sig:?} sur la chaîne gyro", self.firmware)),
            }
        };
        let (a, b) = (at(from)?, at(to)?);
        if a > b {
            return Err(format!("{} : prise {from:?} après {to:?}", self.firmware));
        }
        Ok(a..b)
    }

    /// Cadence commune des étapes gyro `steps`. Erreur si elles en mêlent deux : un étage se rejoue d'un bloc, à une seule
    /// cadence (L-104).
    pub fn gyro_rate(&self, steps: &Range<usize>) -> Result<Option<Rate>, String> {
        let mut rates = self.gyro[steps.clone()].iter().map(|s| s.rate);
        let first = rates.next();
        match first {
            Some(r) if rates.any(|x| x != r) => Err(format!(
                "{} : étapes {steps:?} de la chaîne gyro à deux cadences dans un même étage",
                self.firmware
            )),
            _ => Ok(first),
        }
    }

    /// Signal du log pris en entrée de la chaîne gyro (sa prise au début de la chaîne), pour le rejeu : `None` si le log
    /// ne l'a pas ou si la description n'en prend aucun.
    pub fn recorded_input<'a>(&self, log: &'a FlightLog) -> Option<&'a Rows> {
        self.taps
            .iter()
            .filter(|t| t.chain == Which::Gyro && t.at == 0)
            .find_map(|t| match t.signal {
                Signal::GyroUnfilt => log.gyro_unfilt.as_ref(),
                _ => None,
            })
    }
}
