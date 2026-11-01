//! Fiche drone du guide intégré (`advisor/drone_card.py`) : ce que l'utilisateur dit de son drone, et les réglages qui en découlent.
//!
//! La lecture et l'écriture du fichier des fiches ne sont pas ici (elles étaient dans `gui/guide.py`). Les valeurs conseillées
//! viennent des mêmes fonctions que l'assistant (`rules::log_rate_needed`, `rules::pid_log_rate_needed`, profils de taille).

use crate::profiles::{is_space, profile};
use crate::rules::{log_rate_needed, pid_log_rate_needed, sample_rate_cli};
use otb_format::round_digits;
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
/// 6–7" et plus : autres listes et plages (Brian White [S34]).
pub const LARGE_SIZES: [&str; 2] = ["seven", "large"];

/// Séries de vols (méthode de Brian White, choix de Cédric du 2026-10-12) : valeurs du curseur, vol par vol.
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

/// Finitions : plage de départ du I (Brian White [S34, S36], arrondie et choisie par Cédric).
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
            size: "five".to_string(),
            pid_khz: 4.0,
            esc: "BLHeli_32".to_string(),
            done: Vec::new(),
        }
    }

    /// `to_dict` de Python (`asdict`). `pid_khz` s'écrit toujours en flottant (4.0) : Python écrit ce qu'il a lu (`4` si le fichier
    /// disait `4`), seule différence connue (les choix de la fiche sont des flottants).
    pub fn to_value(&self) -> Value {
        let done: Map<String, Value> = self.done.iter().map(|(k, v)| (k.clone(), Value::Bool(*v))).collect();
        json!({"name": self.name, "bf": self.bf, "size": self.size, "pid_khz": self.pid_khz, "esc": self.esc, "done": done})
    }

    /// `from_dict` de Python : les clés inconnues sont ignorées, les absentes gardent leur défaut, une taille inconnue devient
    /// `five`. Python ne vérifie pas les types ; ici une valeur du mauvais type, ou l'absence de `name`, est une erreur.
    pub fn from_value(d: &Value) -> Result<DroneCard, String> {
        let obj = d.as_object().ok_or("la fiche n'est pas un objet")?;
        let text = |k: &str| match obj.get(k) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(format!("« {k} » n'est pas un texte")),
        };
        let name = text("name")?.ok_or("fiche sans nom")?;
        let mut card = DroneCard::new(&name);
        if let Some(s) = text("bf")? {
            card.bf = s;
        }
        if let Some(s) = text("size")? {
            card.size = s;
        }
        if let Some(s) = text("esc")? {
            card.esc = s;
        }
        match obj.get("pid_khz") {
            None => {}
            Some(Value::Number(n)) => card.pid_khz = n.as_f64().ok_or("« pid_khz » illisible")?,
            Some(_) => return Err("« pid_khz » n'est pas un nombre".to_string()),
        }
        match obj.get("done") {
            None => {}
            Some(Value::Object(m)) => {
                for (k, v) in m {
                    card.done.push((
                        k.clone(),
                        v.as_bool()
                            .ok_or_else(|| format!("case « {k} » n'est pas un booléen"))?,
                    ));
                }
            }
            Some(_) => return Err("« done » n'est pas un dictionnaire".to_string()),
        }
        if profile(&card.size).is_none() {
            card.size = "five".to_string();
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

    fn profile(&self) -> &'static crate::profiles::CraftProfile {
        // Python : `PROFILES[self.size]` lève `KeyError` pour une taille inconnue ; `from_value` les ramène à `five`.
        profile(&self.size).unwrap_or_else(|| panic!("taille inconnue : {}", self.size))
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
