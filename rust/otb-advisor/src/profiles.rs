//! Profils de taille de drone et détection automatique (`advisor/profiles.py`).
//!
//! La taille se devine surtout par la fréquence de rotation des moteurs en vol stationnaire : plus l'hélice est petite, plus le
//! moteur tourne vite. Le nombre de cellules départage les petits drones (2 à 3,5") qui tournent aussi vite qu'un 5".
//!
//! Les deux expressions régulières de Python ont un `(?<!\d)` : la crate `regex` ne le connaît pas, elles sont écrites à la main
//! avec la même sémantique (chaque position de départ dans l'ordre, alternatives gloutonnes dans l'ordre de Python).

use crate::pyclasses::{DIGITS, WORDS};
use otb_analysis::motors::hover_motor_hz;
use otb_format::fixed;
use otb_log::flightlog::FlightLog;

#[derive(Debug, Clone, PartialEq)]
pub struct CraftProfile {
    pub key: &'static str,
    pub label: &'static str,
    /// Fréquence moteur typique en stationnaire.
    pub hover_hz: (f64, f64),
    /// Zone habituelle des résonances de châssis.
    pub resonance_hz: (f64, f64),
    /// Latence (50 %) de réponse indicielle « normale ».
    pub latency_ms: (f64, f64),
    pub gyro_delay_max_ms: f64,
    pub dterm_delay_max_ms: f64,
    pub gyro_noise_high_db: f64,
    pub dterm_noise_high_db: f64,
    pub low_band_high_db: f64,
    pub motor_left_high_db: f64,
    pub imbalance_high_pct: f64,
    pub iterm_offset_high: f64,
    pub saturation_high_pct: f64,
    pub roll_pitch_gap_high: f64,
    pub resonance_high_db: f64,
    /// Trop peu de drones dans le corpus : seuils peu fiables.
    pub provisional: bool,
    /// Taille d'hélice représentative, pour les objectifs de retard des filtres.
    pub nominal_inch: Option<f64>,
}

/// Valeurs par défaut de `CraftProfile` (Python) : seuils de bruit et seuils mécaniques communs.
const BASE: CraftProfile = CraftProfile {
    key: "",
    label: "",
    hover_hz: (0.0, 0.0),
    resonance_hz: (0.0, 0.0),
    latency_ms: (0.0, 0.0),
    gyro_delay_max_ms: 0.0,
    dterm_delay_max_ms: 0.0,
    gyro_noise_high_db: -28.5,
    dterm_noise_high_db: -13.5,
    low_band_high_db: -15.5,
    motor_left_high_db: -29.5,
    imbalance_high_pct: 13.5,
    iterm_offset_high: 33.0,
    saturation_high_pct: 1.5,
    roll_pitch_gap_high: 0.12,
    resonance_high_db: 7.9,
    provisional: false,
    nominal_inch: None,
};

/// Les 5 profils, dans l'ordre de `PROFILES` (Python) ; seuils : recalibration du 2026-10-05 (docs/assistant.md).
pub const PROFILES: [CraftProfile; 5] = [
    CraftProfile {
        key: "whoop",
        label: "Tiny whoop / micro (≤ 75 mm, 1–2S)",
        hover_hz: (345.0, 900.0),
        resonance_hz: (250.0, 600.0),
        latency_ms: (13.0, 20.0),
        gyro_delay_max_ms: 1.5,
        dterm_delay_max_ms: 2.5,
        gyro_noise_high_db: -28.0,
        dterm_noise_high_db: -14.7,
        low_band_high_db: -13.9,
        motor_left_high_db: -32.2,
        provisional: true,
        ..BASE
    },
    CraftProfile {
        key: "micro",
        label: "2 à 3,5 pouces",
        hover_hz: (200.0, 345.0),
        resonance_hz: (180.0, 450.0),
        nominal_inch: Some(3.0),
        latency_ms: (10.0, 21.0),
        gyro_delay_max_ms: 1.8,
        dterm_delay_max_ms: 2.6,
        gyro_noise_high_db: -29.4,
        dterm_noise_high_db: -14.8,
        low_band_high_db: -13.9,
        motor_left_high_db: -32.5,
        provisional: true,
        ..BASE
    },
    CraftProfile {
        key: "five",
        label: "4 à 5 pouces",
        hover_hz: (160.0, 290.0),
        resonance_hz: (120.0, 350.0),
        nominal_inch: Some(5.0),
        latency_ms: (10.0, 22.0),
        gyro_delay_max_ms: 1.9,
        dterm_delay_max_ms: 2.7,
        gyro_noise_high_db: -28.3,
        dterm_noise_high_db: -13.1,
        low_band_high_db: -15.7,
        motor_left_high_db: -28.4,
        imbalance_high_pct: 14.0,
        iterm_offset_high: 32.0,
        roll_pitch_gap_high: 0.11,
        resonance_high_db: 6.8,
        ..BASE
    },
    CraftProfile {
        key: "seven",
        label: "6 à 7 pouces",
        hover_hz: (115.0, 160.0),
        resonance_hz: (80.0, 220.0),
        nominal_inch: Some(7.0),
        latency_ms: (15.0, 24.0),
        gyro_delay_max_ms: 2.6,
        dterm_delay_max_ms: 2.8,
        gyro_noise_high_db: -31.4,
        dterm_noise_high_db: -17.5,
        low_band_high_db: -21.7,
        motor_left_high_db: -31.8,
        imbalance_high_pct: 11.2,
        iterm_offset_high: 31.0,
        roll_pitch_gap_high: 0.08,
        resonance_high_db: 8.2,
        ..BASE
    },
    CraftProfile {
        key: "large",
        label: "8 pouces et plus",
        hover_hz: (50.0, 125.0),
        resonance_hz: (40.0, 160.0),
        nominal_inch: Some(8.0),
        latency_ms: (18.0, 30.0),
        gyro_delay_max_ms: 3.5,
        dterm_delay_max_ms: 3.1,
        gyro_noise_high_db: -31.0,
        dterm_noise_high_db: -17.0,
        low_band_high_db: -22.0,
        motor_left_high_db: -32.0,
        provisional: true,
        ..BASE
    },
];

/// `PROFILES[key]` de Python : `None` si la clé n'existe pas (Python lève `KeyError`).
pub fn profile(key: &str) -> Option<&'static CraftProfile> {
    PROFILES.iter().find(|p| p.key == key)
}

/// Objectifs de retard des filtres (gyro, D) selon la taille : droites de 1 ms à 2 pouces jusqu'à 4 ms (gyro) et 3 ms (D)
/// à 14 pouces. `None` si la taille est inconnue ou sous 2 pouces (`inch < 2` est faux pour NaN : Python rend alors NaN).
pub fn filter_delay_target_ms(inch: Option<f64>) -> Option<(f64, f64)> {
    let inch = inch?;
    if inch < 2.0 {
        return None;
    }
    Some((1.0 + (inch - 2.0) * 3.0 / 12.0, 1.0 + (inch - 2.0) * 2.0 / 12.0))
}

// Limites de taille, vérifiées sur 41 drones de taille connue (docs/assistant.md, « Taille du drone »)
const WHOOP_HZ: f64 = 345.0;
const MICRO_HZ: f64 = 290.0;
const FIVE_HZ: f64 = 160.0;
const SEVEN_HZ: f64 = 115.0;
/// 2 à 4S au-dessus de 200 Hz : petit drone (un 5" en 4S tourne plus lentement).
const SMALL_PACK_HZ: f64 = 200.0;

/// Taille d'après la fréquence moteur médiane en vol et, si connu, le nombre de cellules (NaN : `large`, comme Python).
pub fn profile_key(hz: f64, cells: Option<i64>) -> &'static str {
    if cells == Some(1) {
        return "whoop";
    }
    if hz >= WHOOP_HZ {
        return "whoop";
    }
    if hz >= MICRO_HZ {
        return "micro";
    }
    if hz >= FIVE_HZ {
        if cells.is_some_and(|c| c <= 4) && hz >= SMALL_PACK_HZ {
            return "micro";
        }
        return "five";
    }
    if hz >= SEVEN_HZ {
        return "seven";
    }
    "large"
}

/// Valeur d'un chiffre décimal Unicode (`\d` de Python), `None` si `c` n'en est pas un.
pub fn digit_value(c: char) -> Option<u32> {
    let c = c as u32;
    let i = DIGITS.partition_point(|&(_, b)| b < c);
    DIGITS.get(i).filter(|&&(a, _)| a <= c).map(|&(a, _)| (c - a) % 10)
}

/// `\s` de Python (`str.isspace`) : les blancs d'Unicode plus U+001C à U+001F.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `\w` de Python : lettres, chiffres et `_` (table de `pyclasses.rs`, pas `is_alphanumeric` de Rust qui compte aussi des signes combinants).
pub fn is_word(c: char) -> bool {
    let c = c as u32;
    let i = WORDS.partition_point(|&(_, b)| b < c);
    WORDS.get(i).is_some_and(|&(a, _)| a <= c)
}

/// `\b` entre `s[i-1]` et `s[i]` (début et fin de chaîne comptent comme non-mot).
fn boundary(s: &[char], i: usize) -> bool {
    let before = i > 0 && is_word(s[i - 1]);
    let after = i < s.len() && is_word(s[i]);
    before != after
}

fn starts_with(s: &[char], i: usize, lit: &str) -> bool {
    let n = lit.chars().count();
    i + n <= s.len() && s[i..i + n].iter().copied().eq(lit.chars())
}

/// `whoop|meteor|(?<!\d)(65|75)(?!\d)` cherché dans `s`.
fn looks_like_whoop(s: &[char]) -> bool {
    (0..s.len()).any(|i| {
        starts_with(s, i, "whoop")
            || starts_with(s, i, "meteor")
            || ((starts_with(s, i, "65") || starts_with(s, i, "75"))
                && !(i > 0 && digit_value(s[i - 1]).is_some())
                && !(i + 2 < s.len() && digit_value(s[i + 2]).is_some()))
    })
}

/// Unité de taille après le nombre : `"` ou `in(ch(es)?)?\b` ou `pouces?\b` (la suite de la chaîne n'est pas lue au-delà).
fn unit_at(s: &[char], i: usize) -> bool {
    if i < s.len() && s[i] == '"' {
        return true;
    }
    // alternatives de `in(?:ch(?:es)?)?\b`, de la plus longue à la plus courte, puis `pouces?\b`
    for lit in ["inches", "inch", "in"] {
        if starts_with(s, i, lit) && boundary(s, i + lit.chars().count()) {
            return true;
        }
    }
    for lit in ["pouces", "pouce"] {
        if starts_with(s, i, lit) && boundary(s, i + lit.chars().count()) {
            return true;
        }
    }
    false
}

/// `(?<!\d)(\d{1,2}(?:[.,]\d)?)\s*(?:"|in(?:ch(?:es)?)?\b|pouces?\b)` : premier texte capturé (position la plus à gauche, puis
/// alternatives gloutonnes : deux chiffres avant un, décimale avant sans).
fn size_in_name(s: &[char]) -> Option<String> {
    for i in 0..s.len() {
        if i > 0 && digit_value(s[i - 1]).is_some() {
            continue;
        }
        let mut digits = 0;
        while digits < 2 && i + digits < s.len() && digit_value(s[i + digits]).is_some() {
            digits += 1;
        }
        for nd in (1..=digits).rev() {
            let j = i + nd;
            let has_dec = j + 1 < s.len() && (s[j] == '.' || s[j] == ',') && digit_value(s[j + 1]).is_some();
            for dec in [true, false] {
                if dec && !has_dec {
                    continue;
                }
                let end = if dec { j + 2 } else { j };
                let mut k = end;
                while k < s.len() && is_space(s[k]) {
                    k += 1;
                }
                if unit_at(s, k) {
                    return Some(s[i..end].iter().collect());
                }
            }
        }
    }
    None
}

/// `float(texte.replace(",", "."))` d'un nombre capturé : chiffres Unicode lus comme `float()` de Python.
fn parse_size(text: &str) -> f64 {
    let ascii: String = text
        .chars()
        .map(|c| match digit_value(c) {
            Some(d) => char::from_digit(d, 10).unwrap(),
            None => '.',
        })
        .collect();
    ascii.parse().unwrap()
}

/// Profil d'après la fréquence moteur, les cellules et le nom du drone : `(profil, explication)`.
/// Séparé de `detect_profile` pour être essayé sans journal ; `craft_name` est le nom tel qu'écrit dans l'en-tête.
pub fn detect_profile_from(hz: Option<f64>, cells: Option<i64>, craft_name: &str) -> (&'static CraftProfile, String) {
    let get = |k: &str| profile(k).unwrap();
    if let Some(hz) = hz {
        let batt = match cells {
            Some(c) if c != 0 => format!(", batterie {c}S"),
            _ => String::new(),
        };
        let why = format!(
            "moteurs à {} Hz en vol (≈ {} tr/min)",
            fixed(hz, 0, false),
            fixed(hz * 60.0, 0, false)
        ) + &batt;
        return (get(profile_key(hz, cells)), why);
    }
    if cells == Some(1) {
        return (get("whoop"), "batterie 1S".to_string());
    }
    let name: Vec<char> = craft_name.to_lowercase().chars().collect();
    // 65 / 75 (mm) seulement comme nombre entier : « 1750 » (une capacité) n'est pas un whoop
    if looks_like_whoop(&name) {
        return (get("whoop"), format!("nom du drone « {craft_name} »"));
    }
    // taille en pouces : 1 ou 2 chiffres (« 10in »), décimale facultative (« 3.5" »)
    if let Some(text) = size_in_name(&name) {
        let size = parse_size(&text);
        let key = if size < 3.8 {
            "micro"
        } else if size < 5.8 {
            "five"
        } else if size < 7.8 {
            "seven"
        } else {
            "large"
        };
        return (get(key), format!("nom du drone « {craft_name} »"));
    }
    (get("five"), "taille inconnue : profil 5 pouces par défaut".to_string())
}

/// Profil du journal : fréquence moteur en vol et nombre de cellules en priorité, puis nom du drone.
pub fn detect_profile(log: &FlightLog) -> (&'static CraftProfile, String) {
    detect_profile_from(hover_motor_hz(log), log.cells(), &log.settings.craft_name)
}
