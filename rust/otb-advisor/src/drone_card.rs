//! Fiche drone du guide intégré (`advisor/drone_card.py`) : ce que l'utilisateur dit de son drone, et les réglages qui en découlent.
//!
//! La lecture et l'écriture du fichier des fiches ne sont pas ici (elles étaient dans `gui/guide.py`). Les valeurs conseillées
//! viennent des mêmes fonctions que l'assistant (`rules::log_rate_needed`, `rules::pid_log_rate_needed`, profils de taille).

use crate::profiles::{detect_profile, is_space, profile};
use crate::rules::{log_rate_needed, pid_log_rate_needed, sample_rate_cli};
use otb_format::round_digits;
use otb_log::flightlog::FlightLog;
use serde_json::{Map, Value, json};

/// Choix proposés dans la fiche : (valeur enregistrée, libellé).
pub const BF_FAMILIES: [(&str, &str); 3] = [("4.4", "4.4"), ("4.5", "4.5"), ("2025.12", "2025.12 et plus récent")];
pub const PID_RATES_KHZ: [(f64, &str); 5] = [
    (8.0, "8 kHz"),
    (4.0, "4 kHz"),
    (3.2, "3,2 kHz (gyro BMI270)"),
    (2.0, "2 kHz"),
    (1.6, "1,6 kHz"),
];
pub const ESCS: [(&str, &str); 4] = [
    ("BLHeli_32", "BLHeli_32"),
    ("Bluejay", "BLHeli_S avec Bluejay"),
    ("AM32", "AM32"),
    ("BLHeli_S", "BLHeli_S d'origine (sans Bluejay)"),
];
/// DShot bidirectionnel possible.
pub const BIDIR_ESCS: [&str; 3] = ["BLHeli_32", "Bluejay", "AM32"];
/// 6–7" et plus : autres listes et plages ([S34]).
/// Taille d'une fiche neuve, et repli de toute taille inconnue.
pub const DEFAULT_SIZE: &str = "five";

pub const LARGE_SIZES: [&str; 2] = ["seven", "large"];

/// Séries de vols (méthode des séries, choix de Cédric du 2026-10-12) : valeurs du curseur, vol par vol.
pub const DAMPING_SERIES_SMALL: [f64; 6] = [0.6, 0.8, 1.0, 1.2, 1.4, 1.6];
pub const DAMPING_SERIES_LARGE: [f64; 4] = [1.0, 1.2, 1.4, 1.6];
/// Arrêt au premier vol « trop haut » [S34].
pub const MASTER_SERIES: [f64; 6] = [1.0, 1.2, 1.4, 1.6, 1.8, 2.0];
/// Réponse des sticks, en Acro, pas de 0,5 (choix de Cédric, 2026-10-12).
pub const FF_SERIES: [f64; 3] = [0.0, 0.5, 1.0];
/// Pitch Damping / Pitch Tracking : 3 vols dans le sens donné par le diagnostic.
pub const PITCH_SERIES_LEN: usize = 3;

/// Filtres RPM par défaut du firmware (identiques en 4.4, 4.5 et 2025.12+ : `pg/rpm_filter.c`) ; « Réinitialiser ce profil » du
/// configurateur ne les remet pas.
pub struct RpmDefaults {
    pub harmonics: i64,
    pub q: i64,
    pub min_hz: i64,
    pub fade_hz: i64,
}
/// Notch dynamique par défaut du firmware (`pg/dyn_notch.c`).
pub struct DynNotchDefaults {
    pub count: i64,
    pub q: i64,
    pub min_hz: i64,
    pub max_hz: i64,
}
pub const RPM_DEFAULTS: RpmDefaults = RpmDefaults {
    harmonics: 3,
    q: 500,
    min_hz: 100,
    fade_hz: 50,
};
pub const DYN_NOTCH_DEFAULTS: DynNotchDefaults = DynNotchDefaults {
    count: 3,
    q: 300,
    min_hz: 100,
    max_hz: 600,
};

/// Finitions : plage de départ du I ([S34, S36], arrondie et choisie par Cédric).
pub const I_RANGE_SMALL: (f64, f64) = (0.8, 1.0);
pub const I_RANGE_LARGE: (f64, f64) = (0.6, 0.8);

#[derive(Debug, Clone, PartialEq)]
pub struct DroneCard {
    /// = nom du drone dans Betaflight (`craft_name`) : relie les logs à la fiche.
    pub name: String,
    pub bf: String,
    /// Clé de `profiles::PROFILES`.
    pub size: String,
    /// Fréquence de la boucle PID (onglet Configuration).
    pub pid_khz: f64,
    /// La valeur lue dans le fichier était un entier (`4`) : on la réécrit telle quelle, comme Python (qui écrit ce qu'il a lu).
    /// Une fiche faite par la fenêtre de saisie garde un flottant (`4.0`), comme `float(...)` de `CardDialog.card`.
    pub pid_khz_int: bool,
    pub esc: String,
    /// Cases cochées à la main (dans l'ordre du fichier, comme le dictionnaire de Python).
    pub done: Vec<(String, bool)>,
}

impl DroneCard {
    /// `DroneCard(name)` de Python : les valeurs par défaut des autres champs.
    pub fn new(name: &str) -> DroneCard {
        DroneCard {
            name: name.to_string(),
            bf: "4.5".to_string(),
            size: DEFAULT_SIZE.to_string(),
            pid_khz: 4.0,
            pid_khz_int: false,
            esc: "BLHeli_32".to_string(),
            done: Vec::new(),
        }
    }

    /// `to_dict` de Python (`asdict`). `pid_khz` s'écrit en entier (`4`) seulement s'il a été lu ainsi (`pid_khz_int`), sinon en
    /// flottant (`4.0`) : Python écrit ce qu'il a lu.
    pub fn to_value(&self) -> Value {
        let done: Map<String, Value> = self.done.iter().map(|(k, v)| (k.clone(), Value::Bool(*v))).collect();
        let khz = if self.pid_khz_int && self.pid_khz.fract() == 0.0 && self.pid_khz.abs() < 1e15 {
            json!(self.pid_khz as i64)
        } else {
            json!(self.pid_khz)
        };
        json!({"name": self.name, "bf": self.bf, "size": self.size, "pid_khz": khz, "esc": self.esc, "done": done})
    }

    /// `from_dict` de Python, en plus tolérant : les clés inconnues sont ignorées ; une clé absente **ou d'un type faux** garde son
    /// défaut (Python ne vérifie rien et laisse la valeur fausse planter plus loin) ; une taille inconnue devient `five` ; une case
    /// de `done` vaut sa valeur de vérité (comme `bool(done.get(clé))`). Seuls restent des erreurs : une fiche qui n'est pas un
    /// objet, et un `name` absent ou qui n'est pas un texte (Python : `TypeError` pour l'absent, nom inutilisable sinon). W-029.
    pub fn from_value(d: &Value) -> Result<DroneCard, String> {
        let obj = d.as_object().ok_or("la fiche n'est pas un objet")?;
        let text = |k: &str| match obj.get(k) {
            Some(Value::String(s)) => Some(s.clone()),
            _ => None,
        };
        let name = match obj.get("name") {
            Some(Value::String(s)) => s.clone(),
            Some(_) => return Err("« name » n'est pas un texte".to_string()),
            None => return Err("fiche sans nom".to_string()),
        };
        let mut card = DroneCard::new(&name);
        if let Some(s) = text("bf") {
            card.bf = s;
        }
        if let Some(s) = text("size") {
            card.size = s;
        }
        if let Some(s) = text("esc") {
            card.esc = s;
        }
        if let Some(Value::Number(n)) = obj.get("pid_khz")
            && let Some(x) = n.as_f64()
        {
            card.pid_khz = x;
            card.pid_khz_int = n.is_i64() || n.is_u64();
        }
        if let Some(Value::Object(m)) = obj.get("done") {
            card.done = m.iter().map(|(k, v)| (k.clone(), truthy(v))).collect();
        }
        if profile(&card.size).is_none() {
            card.size = DEFAULT_SIZE.to_string();
        }
        Ok(card)
    }

    /// DShot bidirectionnel possible avec cet ESC.
    pub fn bidir(&self) -> bool {
        BIDIR_ESCS.contains(&self.esc.as_str())
    }

    /// Fréquence d'enregistrement conseillée (Hz) et valeur `blackbox_sample_rate` (1/N), d'après la boucle PID et la vitesse
    /// typique des moteurs de cette taille (milieu de `hover_hz`).
    pub fn log_rate(&self) -> (f64, String) {
        let pid_hz = self.pid_khz * 1000.0;
        let (lo, hi) = self.profile().hover_hz;
        let hz = log_rate_needed(pid_hz, Some((lo + hi) / 2.0));
        (hz, sample_rate_cli(pid_hz, hz))
    }

    /// Fréquence d'enregistrement conseillée pour les vols de série PID (Hz) et `blackbox_sample_rate`.
    pub fn pid_log_rate(&self) -> (f64, String) {
        let pid_hz = self.pid_khz * 1000.0;
        let hz = pid_log_rate_needed(pid_hz);
        (hz, sample_rate_cli(pid_hz, hz))
    }

    /// Profil de la taille de la fiche, ou celui de `five` si la taille est inconnue.
    ///
    /// ⚠️ Repli volontaire, jamais une panique : une fiche construite à la main (hors `from_value`, qui ramène déjà
    /// toute taille inconnue à `five`) ferait sinon mourir l'application sur un simple champ mal rempli (W-029)
    fn profile(&self) -> &'static crate::profiles::CraftProfile {
        profile(&self.size)
            .or_else(|| profile(DEFAULT_SIZE))
            .expect("le profil `five` existe toujours")
    }

    /// « small » (jusqu'à 5") ou « large » (6–7" et plus).
    pub fn size_group(&self) -> &'static str {
        if LARGE_SIZES.contains(&self.size.as_str()) {
            "large"
        } else {
            "small"
        }
    }

    pub fn damping_series(&self) -> &'static [f64] {
        if self.size_group() == "large" {
            &DAMPING_SERIES_LARGE
        } else {
            &DAMPING_SERIES_SMALL
        }
    }

    pub fn i_range(&self) -> (f64, f64) {
        if self.size_group() == "large" {
            I_RANGE_LARGE
        } else {
            I_RANGE_SMALL
        }
    }

    /// Mode de débogage des vols de réglage : GYRO_SCALED en 4.4 (seul gyro non filtré), FFT_FREQ ensuite.
    pub fn debug_mode(&self) -> &'static str {
        if self.bf == "4.4" { "GYRO_SCALED" } else { "FFT_FREQ" }
    }

    /// Nom CLI de la limite d'angle : `level_limit` en 4.4, `angle_limit` ensuite.
    pub fn angle_limit_setting(&self) -> &'static str {
        if self.bf == "4.4" { "level_limit" } else { "angle_limit" }
    }
}

/// Fiche préremplie d'après les valeurs lues dans un log (`card_from_log` de `gui/guide.py`) : séparée de la lecture du journal pour
/// être essayée sans journal. `family` est la famille de version du log, `esc_32` vaut vrai si le log a des eRPM ou le DShot
/// bidirectionnel, `size` la clé du profil détecté.
pub fn card_from_parts(craft_name: &str, family: &str, pid_rate_hz: f64, esc_32: bool, size: &str) -> DroneCard {
    let bf = if matches!(family, "4.4" | "4.5") {
        family
    } else {
        "2025.12"
    };
    // `min(key=…)` de Python garde le premier ex æquo, comme `min_by` de Rust.
    let khz = PID_RATES_KHZ
        .iter()
        .map(|(k, _)| *k)
        .min_by(|a, b| {
            (a * 1000.0 - pid_rate_hz)
                .abs()
                .total_cmp(&(b * 1000.0 - pid_rate_hz).abs())
        })
        .unwrap_or(4.0);
    let mut card = DroneCard::new(if craft_name.is_empty() { "Mon drone" } else { craft_name });
    card.bf = bf.to_string();
    card.size = size.to_string();
    card.pid_khz = khz;
    card.esc = if esc_32 { "BLHeli_32" } else { "BLHeli_S" }.to_string();
    card
}

/// Fiche préremplie d'après un log : nom, version, taille détectée, boucle PID, ESC (DShot bidirectionnel).
pub fn card_from_log(log: &FlightLog) -> DroneCard {
    let s = &log.settings;
    card_from_parts(
        &s.craft_name,
        s.family(),
        s.pid_rate_hz(),
        log.motor_hz.is_some() || s.dshot_bidir,
        detect_profile(log).0.key,
    )
}

/// Valeur de vérité d'une valeur JSON, comme `bool(x)` de Python.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// `str.strip()` de Python.
fn py_strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// Fiche dont le nom est celui du drone dans le log (casse et espaces ignorés).
pub fn card_for_log<'a>(cards: &'a [DroneCard], craft_name: &str) -> Option<&'a DroneCard> {
    let key = py_strip(craft_name).to_lowercase();
    if key.is_empty() {
        return None;
    }
    cards.iter().find(|c| py_strip(&c.name).to_lowercase() == key)
}

/// Pitch Damping / Pitch Tracking : `PITCH_SERIES_LEN` valeurs à partir de la position actuelle, vers le haut (`direction` > 0) ou
/// vers le bas, par pas de 0,2, bornées à 0,2–2,0 (plage du configurateur).
pub fn pitch_series(current: f64, direction: i64, step: f64) -> Vec<f64> {
    let sign = if direction > 0 { 1.0 } else { -1.0 };
    (1..=PITCH_SERIES_LEN)
        .map(|k| round_digits(current + sign * step * k as f64, 2))
        .filter(|v| (0.2..=2.0).contains(v))
        .collect()
}
