//! Noms des modes (boîtes) enregistrés dans `flightModeFlags` (port de `log/modes.py`).

use crate::debug_tables::family;

const M44: &[&str] = &[
    "ARM",
    "ANGLE",
    "HORIZON",
    "MAG",
    "HEADFREE",
    "PASSTHRU",
    "FAILSAFE",
    "GPSRESCUE",
    "ANTIGRAVITY",
    "HEADADJ",
    "CAMSTAB",
    "BEEPERON",
    "LEDLOW",
    "CALIB",
    "OSD",
    "TELEMETRY",
    "SERVO1",
    "SERVO2",
    "SERVO3",
    "BLACKBOX",
    "AIRMODE",
    "3D",
    "FPVANGLEMIX",
    "BLACKBOXERASE",
    "CAMERA1",
    "CAMERA2",
    "CAMERA3",
    "FLIPOVERAFTERCRASH",
    "PREARM",
    "BEEPGPSCOUNT",
    "VTXPITMODE",
    "PARALYZE",
    "USER1",
    "USER2",
    "USER3",
    "USER4",
    "PIDAUDIO",
    "ACROTRAINER",
    "VTXCONTROLDISABLE",
    "LAUNCHCONTROL",
    "MSPOVERRIDE",
    "STICKCOMMANDDISABLE",
    "BEEPERMUTE",
    "READY",
];
const M45_EXTRA: &str = "LAPTIMERRESET";
const M2025: &[&str] = &[
    "ARM",
    "ANGLE",
    "HORIZON",
    "MAG",
    "ALTHOLD",
    "HEADFREE",
    "CHIRP",
    "PASSTHRU",
    "FAILSAFE",
    "POSHOLD",
    "GPSRESCUE",
    "ANTIGRAVITY",
    "HEADADJ",
    "CAMSTAB",
    "BEEPERON",
    "LEDLOW",
    "CALIB",
    "OSD",
    "TELEMETRY",
    "SERVO1",
    "SERVO2",
    "SERVO3",
    "BLACKBOX",
    "AIRMODE",
    "3D",
    "FPVANGLEMIX",
    "BLACKBOXERASE",
    "CAMERA1",
    "CAMERA2",
    "CAMERA3",
    "CRASHFLIP",
    "PREARM",
    "BEEPGPSCOUNT",
    "VTXPITMODE",
    "PARALYZE",
    "USER1",
    "USER2",
    "USER3",
    "USER4",
    "PIDAUDIO",
    "ACROTRAINER",
    "VTXCONTROLDISABLE",
    "LAUNCHCONTROL",
    "MSPOVERRIDE",
    "STICKCOMMANDDISABLE",
    "BEEPERMUTE",
    "READY",
    "LAPTIMERRESET",
];

/// Modes affichés sous la ligne de temps (les autres sont des interrupteurs sans intérêt pour le réglage).
pub const SHOWN: &[&str] = &[
    "ARM",
    "ANGLE",
    "HORIZON",
    "ALTHOLD",
    "POSHOLD",
    "HEADFREE",
    "CHIRP",
    "PASSTHRU",
    "FAILSAFE",
    "GPSRESCUE",
    "AUTOPILOT",
    "AIRMODE",
    "3D",
    "FLIPOVERAFTERCRASH",
    "CRASHFLIP",
    "ACROTRAINER",
    "LAUNCHCONTROL",
    "PIDAUDIO",
];

static TABLES: std::sync::LazyLock<[(&'static str, Vec<&'static str>); 4]> = std::sync::LazyLock::new(|| {
    let mut m45 = M44.to_vec();
    m45.push(M45_EXTRA);
    let mut m2026 = M2025[..11].to_vec();
    m2026.push("AUTOPILOT");
    m2026.extend_from_slice(&M2025[11..]);
    m2026.push("WPCAPTURE");
    [
        ("4.4", M44.to_vec()),
        ("4.5", m45),
        ("2025.12", M2025.to_vec()),
        ("2026.6", m2026),
    ]
});

/// Table d'une famille de version (voir `debug_tables::family`).
pub fn table(family: &str) -> &'static [&'static str] {
    TABLES
        .iter()
        .find(|(f, _)| *f == family)
        .map(|(_, t)| t.as_slice())
        .unwrap_or_else(|| panic!("famille inconnue : {family}"))
}

/// Noms des modes actifs dans `mask`. Le masque est lu comme un entier signé à extension de signe, comme en Python
/// (`mask >> i & 1` : un masque négatif a tous ses bits hauts à 1).
pub fn mode_names(mask: i64, version: &[i64], shown_only: bool) -> Vec<&'static str> {
    table(family(version))
        .iter()
        .enumerate()
        .filter(|(i, name)| (mask >> i) & 1 == 1 && (!shown_only || SHOWN.contains(name)))
        .map(|(_, name)| *name)
        .collect()
}

/// Liste (instant, « |ARM|ANGLE ») à chaque changement des modes affichés.
pub fn mode_changes(flags: Option<&[i32]>, t: &[f64], version: &[i64]) -> Vec<(f64, String)> {
    let Some(flags) = flags.filter(|f| !f.is_empty()) else {
        return vec![];
    };
    let mut out = Vec::new();
    let mut last: Option<String> = None;
    for i in 0..flags.len() {
        if i > 0 && flags[i] == flags[i - 1] {
            continue;
        }
        let names = mode_names(i64::from(flags[i]), version, true);
        let label = if names.is_empty() {
            "|—".to_string()
        } else {
            format!("|{}", names.join("|"))
        };
        if last.as_deref() != Some(label.as_str()) {
            out.push((t[i], label.clone()));
            last = Some(label);
        }
    }
    out
}
