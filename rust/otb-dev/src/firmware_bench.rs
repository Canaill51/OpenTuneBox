//! `otb-dev firmware-bench` : le banc de référence des briques de filtrage (lot 24b du plan, D-062).
//!
//! Pour chaque version de Betaflight de [`VERSIONS`] : télécharge `common/filter.c`, `maths.c` et leurs en-têtes, ainsi que
//! la licence du dépôt (vérifiée : GPL 3) ; compile **hors du dépôt** le programme du projet `rust/otb-dev/bench/betaflight_filters.c`
//! avec ces fichiers ; lui fait jouer chaque cas sur les mêmes entrées ; et ne verse dans le dépôt que des **nombres** :
//! `rust/otb-dsp/tests/data/firmware/betaflight/` (manifeste JSON, entrées et sorties en float32 petit-boutistes). Aucun fichier
//! du firmware n'est gardé (D-062, D-045). Le test `otb-dsp/tests/firmware_vectors.rs` rejoue ces vecteurs avec les briques Rust
//! et exige l'égalité bit à bit ; le banc fait la même comparaison à la fin et l'affiche.
//!
//! Poste Linux (`curl`, `cc`) ; la CI ne lance pas le banc, elle rejoue ses vecteurs.
//! ⚠️ Compilé sur PC sans fusion multiplication-addition (`-ffp-contract=off`) : la carte de vol, elle, fusionne souvent ;
//! l'écart au vol réel est mesuré, jamais promis (D-062).

use otb_dsp::firmware::betaflight::{Arith, cases};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Versions jouées : (nom, référence Git téléchargée, version major.minor de l'en-tête des logs). Une par arithmétique au
/// moins, plus la version de développement des logs 2026.12 du corpus (commit figé : pas encore d'étiquette).
pub const VERSIONS: [(&str, &str, (u32, u32)); 6] = [
    ("4.3.2", "4.3.2", (4, 3)),
    ("4.4.3", "4.4.3", (4, 4)),
    ("4.5.5", "4.5.5", (4, 5)),
    ("2025.12.5", "2025.12.5", (2025, 12)),
    ("2026.6.2", "2026.6.2", (2026, 6)),
    ("2026.12-dev", "1b94a3871529bbb68d8dbb7d33a99d1101d3f2d4", (2026, 12)),
];

/// Fichiers du firmware téléchargés (sous `src/main/`), jamais gardés.
const FIRMWARE_FILES: [&str; 6] = [
    "common/filter.c",
    "common/filter.h",
    "common/maths.c",
    "common/maths.h",
    "common/axis.h",
    "common/utils.h",
];

const RAW: &str = "https://raw.githubusercontent.com/betaflight/betaflight";
const HARNESS: &str = include_str!("../bench/betaflight_filters.c");
const CFLAGS: [&str; 5] = [
    "-std=gnu11",
    "-O2",
    "-ffp-contract=off",
    "-fno-fast-math",
    "-fexcess-precision=standard",
];

/// Dossier des vecteurs, depuis la racine du dépôt.
pub const VECTORS_DIR: &str = "rust/otb-dsp/tests/data/firmware/betaflight";

/// Longueur des signaux filtrés.
const N: usize = 3072;

/// Générateur pseudo-aléatoire fixe (xorshift64*) : mêmes entrées à chaque passage.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (v >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

/// Entrées des cas, en float32 : angles, couples (fréquence, pas de temps), signaux et trajectoires de fréquence.
fn inputs() -> BTreeMap<&'static str, Vec<f32>> {
    use std::f64::consts::PI;
    let mut m = BTreeMap::new();

    // Angles : balayage de −36 à 36 rad (la 4.5 rend 0 au-delà de ±32), plus des valeurs remarquables.
    let mut angles: Vec<f32> = (0..1900).map(|i| (-36.0 + 72.0 * i as f64 / 1899.0) as f32).collect();
    for k in (-8..=8).filter(|&k| k != 0) {
        let a = (k as f32) * std::f32::consts::FRAC_PI_2;
        angles.extend([
            a,
            f32::from_bits(a.to_bits() + 1),
            f32::from_bits(a.to_bits().wrapping_sub(1)),
        ]);
    }
    angles.extend([0.0, -0.0, 1e-6, -1e-6, 31.99, 32.0, 32.5, -32.5, 33.0, 0.25, 0.5, 0.75]);
    let mut rng = Rng(0x0B5E_ED00_2024_0001);
    while angles.len() < 2048 {
        angles.push((rng.next() * 3.3) as f32);
    }
    m.insert("angles", angles);

    // Couples (fréquence de coupure, pas de temps) pour les gains, pas de temps = µs × 1e-6f comme le firmware.
    let fcs = [
        1.0, 5.0, 10.0, 33.3, 50.0, 65.0, 80.0, 90.0, 100.0, 120.0, 150.0, 175.0, 200.0, 250.0, 300.0, 333.3, 400.0,
        500.0, 600.0, 750.0, 1000.0,
    ];
    let dts = [62u32, 125, 156, 250, 312, 500, 1000, 2000];
    let (mut fc, mut dt) = (Vec::new(), Vec::new());
    for &f in &fcs {
        for &us in &dts {
            fc.push(f as f32);
            dt.push(us as f32 * 1e-6);
        }
    }
    m.insert("gain_fc", fc);
    m.insert("gain_dt", dt);

    // Couples (centre, coupure) des notchs statiques.
    let (mut c, mut f) = (Vec::new(), Vec::new());
    for &center in &[60.0f32, 100.0, 150.0, 200.0, 260.0, 300.0, 400.0, 500.0, 600.0, 750.0] {
        for &ratio in &[0.3f32, 0.5, 0.62, 0.75, 0.9, 0.99] {
            c.push(center);
            f.push((center * ratio).round());
        }
    }
    m.insert("notch_center", c);
    m.insert("notch_cutoff", f);

    // Signal : balayage de 20 à 3 800 Hz à 8 kHz, puis bruit, puis échelons (une seule suite, sans remise à zéro).
    let third = N / 3;
    let t_total = third as f64 / 8000.0;
    let mut signal = Vec::with_capacity(N);
    let mut rng = Rng(0x5EED_0000_0000_0024);
    for i in 0..third {
        let t = i as f64 / 8000.0;
        let phase = 2.0 * PI * (20.0 * t + (3800.0 - 20.0) * t * t / (2.0 * t_total));
        signal.push((400.0 * phase.sin() + 5.0 * rng.next()) as f32);
    }
    for _ in third..2 * third {
        signal.push((300.0 * rng.next()) as f32);
    }
    for i in 2 * third..N {
        let level = if ((i - 2 * third + 50) / 100).is_multiple_of(2) {
            500.0
        } else {
            -500.0
        };
        signal.push((level + 2.0 * rng.next()) as f32);
    }
    // Deux autres axes pour le notch RPM : bruit de graines différentes, plus un sinus.
    let mut rng2 = Rng(0x0000_0A11_5EED_0002);
    let axis2: Vec<f32> = (0..N)
        .map(|i| (200.0 * (2.0 * PI * 180.0 * i as f64 / 4000.0).sin() + 150.0 * rng2.next()) as f32)
        .collect();
    let mut rng3 = Rng(0x0000_0A11_5EED_0003);
    let axis3: Vec<f32> = (0..N).map(|_| (800.0 * rng3.next()) as f32).collect();
    m.insert("signal", signal);
    m.insert("signal_axis2", axis2);
    m.insert("signal_axis3", axis3);

    // Trajectoires : coupure d'un LPF dynamique (80 → 300 Hz), centre d'un notch (120 → 600 Hz, avec des paliers), poids
    // d'un notch RPM (0 → 1 puis 1).
    let ramp = |i: usize, cycles: f64| 0.5 - 0.5 * (2.0 * PI * cycles * i as f64 / N as f64).cos();
    m.insert(
        "track_lpf",
        (0..N).map(|i| (80.0 + 220.0 * ramp(i, 3.0)) as f32).collect(),
    );
    m.insert(
        "track_notch",
        (0..N)
            .map(|i| {
                if (i / 64) % 5 == 4 {
                    333.0
                } else {
                    (120.0 + 480.0 * ramp(i, 2.0)) as f32
                }
            })
            .collect(),
    );
    m.insert(
        "track_weight",
        (0..N).map(|i| (i as f64 / (N as f64 / 2.0)).min(1.0) as f32).collect(),
    );
    m
}

/// Un cas : identifiant, type (voir `cases::run`), paramètres, entrées.
struct Case {
    id: &'static str,
    kind: &'static str,
    params: Vec<(&'static str, f64)>,
    inputs: Vec<&'static str>,
}

fn case(id: &'static str, kind: &'static str, params: &[(&'static str, f64)], inputs: &[&'static str]) -> Case {
    Case {
        id,
        kind,
        params: params.to_vec(),
        inputs: inputs.to_vec(),
    }
}

/// Tous les cas ; une version joue ceux dont le type existe dans sa famille (`cases::kinds`).
fn all_cases() -> Vec<Case> {
    let s = ["signal"];
    let st = ["signal", "track_lpf"];
    let sn = ["signal", "track_notch"];
    vec![
        case("sincos", "sincos", &[], &["angles"]),
        case("sincosf", "sincosf", &[], &["angles"]),
        case("gains", "gains", &[], &["gain_fc", "gain_dt"]),
        case("notch_q", "notch_q", &[], &["notch_center", "notch_cutoff"]),
        case(
            "pt1_100hz_8k",
            "pt",
            &[("order", 1.0), ("hz", 100.0), ("looptime_us", 125.0)],
            &s,
        ),
        case(
            "pt1_250hz_4k",
            "pt",
            &[("order", 1.0), ("hz", 250.0), ("looptime_us", 250.0)],
            &s,
        ),
        case(
            "pt2_100hz_8k",
            "pt",
            &[("order", 2.0), ("hz", 100.0), ("looptime_us", 125.0)],
            &s,
        ),
        case(
            "pt3_100hz_8k",
            "pt",
            &[("order", 3.0), ("hz", 100.0), ("looptime_us", 125.0)],
            &s,
        ),
        case("pt1_dyn_4k", "pt_dyn", &[("order", 1.0), ("looptime_us", 250.0)], &st),
        case("pt2_dyn_4k", "pt_dyn", &[("order", 2.0), ("looptime_us", 250.0)], &st),
        case("pt3_dyn_4k", "pt_dyn", &[("order", 3.0), ("looptime_us", 250.0)], &st),
        case(
            "biquad_lpf_df2_250hz_8k",
            "biquad_lpf",
            &[("hz", 250.0), ("looptime_us", 125.0), ("df1", 0.0)],
            &s,
        ),
        case(
            "biquad_lpf_df1_150hz_4k",
            "biquad_lpf",
            &[("hz", 150.0), ("looptime_us", 250.0), ("df1", 1.0)],
            &s,
        ),
        case("biquad_lpf_dyn_4k", "biquad_lpf_dyn", &[("looptime_us", 250.0)], &st),
        case(
            "biquad_notch_260_160_4k",
            "biquad_notch",
            &[("center_hz", 260.0), ("cutoff_hz", 160.0), ("looptime_us", 250.0)],
            &s,
        ),
        case(
            "biquad_notch_400_300_8k",
            "biquad_notch",
            &[("center_hz", 400.0), ("cutoff_hz", 300.0), ("looptime_us", 125.0)],
            &s,
        ),
        case(
            "biquad_notch_dyn_4k",
            "biquad_notch_dyn",
            &[("q", 3.0), ("looptime_us", 250.0)],
            &sn,
        ),
        case(
            "biquad_notch_dyn_8k",
            "biquad_notch_dyn",
            &[("q", 3.5), ("looptime_us", 125.0)],
            &sn,
        ),
        case(
            "biquad_rpm_4k",
            "biquad_rpm",
            &[("init_hz", 0.0), ("q", 5.0), ("looptime_us", 250.0)],
            &["signal", "track_notch", "track_weight"],
        ),
        case(
            "biquad_rpm_8k",
            "biquad_rpm",
            &[("init_hz", 100.0), ("q", 5.0), ("looptime_us", 125.0)],
            &["signal", "track_notch", "track_weight"],
        ),
        case(
            "svf_lpf_250hz_8k",
            "svf_lpf",
            &[("hz", 250.0), ("looptime_us", 125.0)],
            &s,
        ),
        case(
            "svf_lpf_150hz_4k",
            "svf_lpf",
            &[("hz", 150.0), ("looptime_us", 250.0)],
            &s,
        ),
        case("svf_lpf_dyn_4k", "svf_lpf_dyn", &[("looptime_us", 250.0)], &st),
        case(
            "svf_notch_260_160_4k",
            "svf_notch",
            &[("center_hz", 260.0), ("cutoff_hz", 160.0), ("looptime_us", 250.0)],
            &s,
        ),
        case(
            "svf_notch_400_300_8k",
            "svf_notch",
            &[("center_hz", 400.0), ("cutoff_hz", 300.0), ("looptime_us", 125.0)],
            &s,
        ),
        case(
            "svf_notch_dyn_4k",
            "svf_notch_dyn",
            &[("q", 3.0), ("looptime_us", 250.0)],
            &sn,
        ),
        case(
            "svf_notch_dyn_8k",
            "svf_notch_dyn",
            &[("q", 3.5), ("looptime_us", 125.0)],
            &sn,
        ),
        case(
            "rpm_notch_4k",
            "rpm_notch",
            &[("init_hz", 100.0), ("q", 5.0), ("looptime_us", 250.0)],
            &["signal", "signal_axis2", "signal_axis3", "track_notch", "track_weight"],
        ),
        case(
            "rpm_notch_8k",
            "rpm_notch",
            &[("init_hz", 200.0), ("q", 5.0), ("looptime_us", 125.0)],
            &["signal", "signal_axis2", "signal_axis3", "track_notch", "track_weight"],
        ),
        case(
            "two_rate_lpf2_pt1_denom2",
            "two_rate",
            &[
                ("gyro_us", 125.0),
                ("denom", 2.0),
                ("lpf2_type", 1.0),
                ("lpf2_hz", 500.0),
                ("notch_center_hz", 260.0),
                ("notch_cutoff_hz", 160.0),
                ("lpf1_type", 2.0),
                ("lpf1_hz", 250.0),
            ],
            &s,
        ),
        case(
            "two_rate_average_denom4",
            "two_rate",
            &[
                ("gyro_us", 125.0),
                ("denom", 4.0),
                ("lpf2_type", 0.0),
                ("lpf2_hz", 0.0),
                ("notch_center_hz", 0.0),
                ("notch_cutoff_hz", 0.0),
                ("lpf1_type", 1.0),
                ("lpf1_hz", 150.0),
            ],
            &s,
        ),
        // Moyenne sur 3 échantillons : « ÷ 3 » et « × 1/3 » n'ont pas les mêmes bits, contrairement à 2 ou 4.
        case(
            "two_rate_average_denom3",
            "two_rate",
            &[
                ("gyro_us", 125.0),
                ("denom", 3.0),
                ("lpf2_type", 0.0),
                ("lpf2_hz", 0.0),
                ("notch_center_hz", 200.0),
                ("notch_cutoff_hz", 100.0),
                ("lpf1_type", 4.0),
                ("lpf1_hz", 90.0),
            ],
            &s,
        ),
        case(
            "two_rate_lpf2_biquad_denom2",
            "two_rate",
            &[
                ("gyro_us", 125.0),
                ("denom", 2.0),
                ("lpf2_type", 2.0),
                ("lpf2_hz", 300.0),
                ("notch_center_hz", 0.0),
                ("notch_cutoff_hz", 0.0),
                ("lpf1_type", 3.0),
                ("lpf1_hz", 120.0),
            ],
            &s,
        ),
    ]
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Empreinte du programme C du banc, fins de ligne ramenées à `\n` (une extraction Windows peut les changer).
fn harness_sha256() -> String {
    sha256_hex(HARNESS.replace('\r', "").as_bytes())
}

fn to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_bytes(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

fn run_cmd(cmd: &mut Command, what: &str) -> Result<Vec<u8>, String> {
    let out = cmd
        .output()
        .map_err(|e| format!("{what} : impossible de lancer ({e})"))?;
    if !out.status.success() {
        return Err(format!("{what} : échec\n{}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(out.stdout)
}

fn download(url: &str, dest: &Path) -> Result<Vec<u8>, String> {
    run_cmd(
        Command::new("curl")
            .args(["-sfL", "--retry", "2", "-o"])
            .arg(dest)
            .arg(url),
        &format!("téléchargement de {url}"),
    )?;
    fs::read(dest).map_err(|e| format!("{} : {e}", dest.display()))
}

/// La licence du dépôt du firmware à cette référence doit être la GPL 3 (vérifiée à sa source à chaque passage, D-062).
fn check_license(text: &str) -> Result<(), String> {
    if text.contains("GNU GENERAL PUBLIC LICENSE") && text.contains("Version 3, 29 June 2007") {
        Ok(())
    } else {
        Err(
            "licence du firmware inattendue (GPL 3 attendue) : le banc s'arrête, revoir D-062 avant de continuer"
                .into(),
        )
    }
}

/// En-têtes de remplacement du projet : `platform.h` (cible matérielle) et `build/build_config.h`, vides de sens ici.
fn write_stubs(dir: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{} : {e}", dir.display());
    fs::create_dir_all(dir.join("build")).map_err(io)?;
    fs::write(
        dir.join("platform.h"),
        "#pragma once\n#define FAST_CODE\n#define FAST_CODE_NOINLINE\n#define FAST_DATA\n#define FAST_DATA_ZERO_INIT\n",
    )
    .map_err(io)?;
    fs::write(dir.join("build").join("build_config.h"), "#pragma once\n").map_err(io)
}

fn compiler_version() -> String {
    Command::new("cc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().map(str::to_owned))
        .unwrap_or_else(|| "inconnu".into())
}

/// Lance le banc et écrit les vecteurs. Rend un résumé (cas joués, égalité avec les briques Rust).
pub fn run(repo: &Path) -> Result<String, String> {
    let work = std::env::temp_dir().join(format!("otb-firmware-bench-{}", std::process::id()));
    if work.starts_with(repo) {
        return Err("le dossier temporaire est dans le dépôt : rien du firmware ne doit y entrer".into());
    }
    let result = run_in(repo, &work);
    let _ = fs::remove_dir_all(&work);
    result
}

fn run_in(repo: &Path, work: &Path) -> Result<String, String> {
    let io = |p: &Path, e: std::io::Error| format!("{} : {e}", p.display());
    fs::create_dir_all(work).map_err(|e| io(work, e))?;
    let stubs = work.join("stubs");
    write_stubs(&stubs)?;
    let harness_c = work.join("betaflight_filters.c");
    fs::write(&harness_c, HARNESS).map_err(|e| io(&harness_c, e))?;

    let inputs = inputs();
    let in_dir = work.join("inputs");
    fs::create_dir_all(&in_dir).map_err(|e| io(&in_dir, e))?;
    let mut input_index = Map::new();
    let mut input_blob = Vec::new();
    for (name, v) in &inputs {
        let p = in_dir.join(format!("{name}.f32"));
        fs::write(&p, to_bytes(v)).map_err(|e| io(&p, e))?;
        input_index.insert((*name).into(), json!({"offset": input_blob.len() / 4, "len": v.len()}));
        input_blob.extend(to_bytes(v));
    }

    let cases = all_cases();
    let mut versions_json = Vec::new();
    let mut outputs: Vec<(String, Vec<u8>)> = Vec::new();
    let mut summary = Vec::new();
    for (name, git_ref, (major, minor)) in VERSIONS {
        let arith = Arith::for_version(major, minor).ok_or_else(|| format!("{name} : aucune arithmétique"))?;
        let src = work.join("src").join(name);
        let mut sources = Map::new();
        for f in FIRMWARE_FILES {
            let dest = src.join(f);
            fs::create_dir_all(dest.parent().unwrap_or(&src)).map_err(|e| io(&src, e))?;
            let bytes = download(&format!("{RAW}/{git_ref}/src/main/{f}"), &dest)?;
            sources.insert(format!("src/main/{f}"), sha256_hex(&bytes).into());
        }
        let license = download(
            &format!("{RAW}/{git_ref}/LICENSE"),
            &work.join(format!("LICENSE-{name}")),
        )?;
        check_license(&String::from_utf8_lossy(&license))?;

        let svf = fs::read_to_string(src.join("common/filter.h"))
            .map_err(|e| io(&src, e))?
            .contains("svfLowpassFilterInit");
        if svf != arith.uses_svf() {
            return Err(format!(
                "{name} : famille SVF = {svf} dans filter.h, {} attendu pour {arith:?}",
                arith.uses_svf()
            ));
        }
        let exe = work.join(format!("harness-{name}"));
        let mut cc = Command::new("cc");
        cc.args(CFLAGS);
        if svf {
            cc.arg("-DOTB_SVF");
        }
        cc.arg("-I")
            .arg(&stubs)
            .arg("-I")
            .arg(&src)
            .arg("-I")
            .arg(src.join("common"))
            .arg(&harness_c)
            .arg(src.join("common/filter.c"))
            .arg(src.join("common/maths.c"))
            .arg("-lm")
            .arg("-o")
            .arg(&exe);
        run_cmd(&mut cc, &format!("compilation pour {name}"))?;

        let kinds = cases::kinds(arith);
        let mut blob = Vec::new();
        let mut played = Vec::new();
        let (mut equal, mut total) = (0usize, 0usize);
        for c in cases.iter().filter(|c| kinds.contains(&c.kind)) {
            let out_path = work.join("out.f32");
            let mut cmd = Command::new(&exe);
            cmd.arg(c.kind).arg(&out_path);
            for i in &c.inputs {
                cmd.arg(format!("@{}", in_dir.join(format!("{i}.f32")).display()));
            }
            for (k, v) in &c.params {
                // Valeur float32 écrite en entier ou au plus court qui revient exacte (`strtof` arrondit au plus près).
                cmd.arg(format!("{k}={:?}", *v as f32));
            }
            run_cmd(&mut cmd, &format!("{name} / {}", c.id))?;
            let bytes = fs::read(&out_path).map_err(|e| io(&out_path, e))?;
            let want = from_bytes(&bytes);
            let ins: Vec<&[f32]> = c.inputs.iter().map(|i| inputs[i].as_slice()).collect();
            let params = |n: &str| c.params.iter().find(|(k, _)| *k == n).map(|(_, v)| *v);
            let got = cases::run(arith, c.kind, &params, &ins)?;
            total += 1;
            if got.len() == want.len() && got.iter().zip(&want).all(|(a, b)| a.to_bits() == b.to_bits()) {
                equal += 1;
            } else {
                let first = got.iter().zip(&want).position(|(a, b)| a.to_bits() != b.to_bits());
                summary.push(format!(
                    "  ✗ {name} / {} : {} valeurs, première différence à l'indice {first:?}",
                    c.id,
                    want.len()
                ));
            }
            played.push(json!({"case": c.id, "offset": blob.len() / 4, "len": want.len()}));
            blob.extend(bytes);
        }
        let file = format!("{name}.f32");
        summary.push(format!(
            "{name} ({arith:?}) : {equal}/{total} cas identiques au bit près"
        ));
        versions_json.push(json!({
            "version": name,
            "git_ref": git_ref,
            "header_version": [major, minor],
            "arith": format!("{arith:?}"),
            "license": format!("GPL-3.0, LICENSE sha256 {}", sha256_hex(&license)),
            "sources_sha256": sources,
            "outputs": file,
            "cases": played,
        }));
        outputs.push((file, blob));
    }

    let manifest = json!({
        "firmware": "betaflight",
        "generated_by": "otb-dev firmware-bench (lot 24b, D-062)",
        "compiler": compiler_version(),
        "cflags": CFLAGS.join(" "),
        "harness_sha256": harness_sha256(),
        "inputs_file": "inputs.f32",
        "inputs": input_index,
        "cases": cases.iter().map(|c| json!({
            "id": c.id,
            "kind": c.kind,
            "params": c.params.iter().map(|(k, v)| ((*k).to_owned(), json!(v))).collect::<Map<String, Value>>(),
            "inputs": c.inputs,
        })).collect::<Vec<_>>(),
        "versions": versions_json,
    });

    let dir: PathBuf = repo.join(VECTORS_DIR);
    fs::create_dir_all(&dir).map_err(|e| io(&dir, e))?;
    let write = |name: &str, bytes: &[u8]| fs::write(dir.join(name), bytes).map_err(|e| io(&dir.join(name), e));
    write("inputs.f32", &input_blob)?;
    for (file, blob) in &outputs {
        write(file, blob)?;
    }
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())? + "\n";
    write("manifest.json", text.as_bytes())?;
    summary.push(format!("vecteurs écrits dans {VECTORS_DIR}"));
    Ok(summary.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(VECTORS_DIR)
            .join("manifest.json");
        serde_json::from_str(&fs::read_to_string(&path).expect("manifest.json")).expect("JSON")
    }

    /// Le banc et ses vecteurs vont ensemble : un cas ou le programme C changé sans relancer `otb-dev firmware-bench`
    /// laisserait le test des briques comparer deux expériences différentes.
    #[test]
    fn les_vecteurs_viennent_de_ce_banc() {
        let m = manifest();
        assert_eq!(
            m["harness_sha256"].as_str(),
            Some(harness_sha256().as_str()),
            "programme C changé : relancer le banc"
        );
        let cases = m["cases"].as_array().expect("cases");
        let want = all_cases();
        assert_eq!(cases.len(), want.len(), "liste des cas changée : relancer le banc");
        for (got, c) in cases.iter().zip(&want) {
            assert_eq!(got["id"], c.id);
            assert_eq!(got["kind"], c.kind, "{}", c.id);
            assert_eq!(got["inputs"], json!(c.inputs), "{}", c.id);
            for (k, v) in &c.params {
                assert_eq!(got["params"][*k].as_f64(), Some(*v), "{} : {k}", c.id);
            }
        }
        let versions: Vec<&str> = m["versions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["version"].as_str().unwrap())
            .collect();
        assert_eq!(
            versions,
            VERSIONS.map(|v| v.0).to_vec(),
            "liste des versions changée : relancer le banc"
        );
    }
}
