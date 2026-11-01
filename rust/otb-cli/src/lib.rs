//! Ligne de commande d'OpenTuneBox (port de `cli.py`). Le texte est produit ici, session par session, et remis à un « puits »
//! (`&mut dyn FnMut(&str)`) : le binaire l'écrit au fil de l'eau comme Python, `otb-parity check-cli` le rassemble et le compare
//! caractère par caractère à celui de Python.

use otb_advisor::profiles::{self, CraftProfile};
use otb_advisor::rules::analyze;
use otb_analysis::period::auto_period;
use otb_format::fixed;
use otb_log::flightlog::{FlightLog, load};
use std::path::Path;

const AXES: [&str; 3] = ["Roll", "Pitch", "Yaw"];
const PROFILS: [&str; 5] = ["whoop", "micro", "five", "seven", "large"];

/// Puits du texte : le binaire l'écrit, les tests le rassemblent.
pub type Sink<'a> = &'a mut dyn FnMut(&str);

/// `FlightLog.summary()` de Python.
pub fn summary(lg: &FlightLog) -> String {
    let s = &lg.settings;
    let pid = AXES
        .iter()
        .zip(&s.pids)
        .map(|(a, p)| format!("{} {},{},{},{},{}", &a[..1], p.p, p.i, p.d, p.dmax, p.f))
        .collect::<Vec<_>>()
        .join(" | ");
    let oui = |b: bool| if b { "oui" } else { "non" };
    format!(
        "{} — {}\n  durée {} s, {} trames, enregistrement {} Hz (PID {} Hz, gyro {} Hz)\n  debug {}, {} moteurs, eRPM {}, gyro non filtré {}\n  PID (P,I,D,Dmax,F) : {}",
        lg.name(),
        s.firmware,
        fixed(lg.duration(), 1, false),
        lg.n(),
        fixed(lg.fs, 0, false),
        fixed(s.pid_rate_hz(), 0, false),
        fixed(s.gyro_rate_hz(), 0, false),
        s.debug_mode,
        lg.n_motors(),
        oui(lg.motor_hz.is_some()),
        oui(lg.gyro_unfilt.is_some()),
        pid,
    )
}

/// Sessions du fichier ; un fichier illisible ou vide est signalé en une ligne, sans trace d'erreur.
fn load_or_say(f: &str, out: Sink) -> Vec<FlightLog> {
    match load(Path::new(f)) {
        Err(e) => {
            out(&format!("{f} : illisible ({e})\n"));
            Vec::new()
        }
        Ok(logs) => {
            if logs.is_empty() {
                out(&format!("{f} : aucune session exploitable\n"));
            }
            logs
        }
    }
}

/// `opentunebox info FICHIER…`.
pub fn info_to(files: &[String], out: Sink) {
    for f in files {
        for lg in load_or_say(f, out) {
            out(&format!("{}\n\n", summary(&lg)));
        }
    }
}

/// `opentunebox report FICHIER… [--profil P]`. `Err` si l'analyse échoue (Python : trace d'erreur) ; le texte des sessions
/// précédentes est alors déjà remis au puits, comme Python l'a déjà écrit.
pub fn report_to(files: &[String], profil: Option<&str>, out: Sink) -> Result<(), String> {
    let forced: Option<&CraftProfile> = profil.and_then(profiles::profile);
    for f in files {
        for lg in load_or_say(f, out) {
            let (t0, t1) = auto_period(&lg, 8.0, 0.5)?;
            let rep = analyze(&lg, Some(lg.slice_time(t0, t1)), forced)?;
            out(&format!("=== {} — {}\n", lg.name(), lg.settings.firmware));
            out(&format!(
                "    {} ({}) ; période {}–{} s\n",
                rep.profile.label,
                rep.profile_reason,
                fixed(t0, 1, false),
                fixed(t1, 1, false)
            ));
            for fd in rep.sorted() {
                out(&format!(
                    "  [{:<9}] {} — {}\n",
                    fd.severity.to_uppercase(),
                    fd.category,
                    fd.title
                ));
                out(&format!("      {}\n", fd.detail));
                for a in &fd.actions {
                    out(&format!("      → {a}\n"));
                }
            }
            out("\n");
        }
    }
    Ok(())
}

/// Texte entier de `info` (pour les tests et la comparaison au corpus).
pub fn info(files: &[String]) -> String {
    let mut text = String::new();
    info_to(files, &mut |s| text.push_str(s));
    text
}

/// Texte de `report` et son erreur éventuelle : le texte écrit avant l'erreur est gardé.
pub fn report(files: &[String], profil: Option<&str>) -> (String, Option<String>) {
    let mut text = String::new();
    let err = report_to(files, profil, &mut |s| text.push_str(s)).err();
    (text, err)
}

const USAGE: &str =
    "usage : opentunebox info FICHIER… | opentunebox report FICHIER… [--profil whoop|micro|five|seven|large]";

/// Ce que demande la ligne de commande.
#[derive(Debug, PartialEq)]
pub enum Parsed {
    Help,
    Info(Vec<String>),
    Report(Vec<String>, Option<String>),
}

/// `option` désigne `cible` comme `argparse` : nom entier, ou préfixe d'au moins `--x`.
fn is_prefix(option: &str, target: &str) -> bool {
    option.len() > 2 && target.starts_with(option)
}

/// Ce qu'`argparse` (Python 3.14) prend pour un argument et non pour une option, tant qu'aucune option ne ressemble à un nombre
/// négatif : `-5`, `-5x`, `-.5` (un chiffre, ou un point puis un chiffre, après le tiret), ou tout texte qui contient une espace.
/// Mesuré sur 3.14.4 : `-.`, `-.x`, `-x5`, `-é1` restent des options.
fn looks_like_argument(a: &str) -> bool {
    let mut c = a.chars().skip(1);
    match c.next() {
        Some(d) if d.is_numeric() => true,
        Some('.') => c.next().is_some_and(|d| d.is_numeric()),
        _ => false,
    }
}

/// Analyse des arguments, dans l'ordre comme `argparse` : une seule série de fichiers (une seconde, après une option, est refusée),
/// `--` (le reste est fait de fichiers), options abrégées (`--prof`), `-h` / `--help` qui l'emporte sur toute erreur à venir.
/// Une valeur de `--profil` manquante ou invalide est refusée aussitôt ; une option inconnue ou un fichier de trop seulement à la fin.
/// Erreur d'usage : code 2.
pub fn parse(args: &[String]) -> Result<Parsed, (i32, String)> {
    let usage = |msg: String| (2, format!("{msg}\n{USAGE}"));
    // `-h`, ou toute grappe `-hX…` (l'aide passe avant le reste de la grappe) ; `--he`… sans valeur
    let is_short_help = |a: &str| a.starts_with('-') && !a.starts_with("--") && a[1..].starts_with('h');
    let option_name = |a: &str| {
        if a.starts_with("--") {
            a.split_once('=').map_or(a, |(n, _)| n).to_string()
        } else {
            a.to_string()
        }
    };
    // `--profil` n'existe que pour `report` : pour `info`, `--profil=a b` est un fichier comme un autre
    let known = |a: &str, report: bool| {
        is_short_help(a)
            || (a.starts_with("--")
                && (is_prefix(&option_name(a), "--help") || (report && is_prefix(&option_name(a), "--profil"))))
    };
    let is_option = |a: &str, report: bool| {
        a.starts_with('-') && a.len() > 1 && (known(a, report) || !(looks_like_argument(a) || a.contains(' ')))
    };
    let mut deferred: Option<(i32, String)> = None;
    let mut cmd: Option<&str> = None;
    let mut files: Vec<String> = Vec::new();
    let mut profil: Option<String> = None;
    let (mut run_closed, mut after_dashes) = (false, false);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if !after_dashes && a == "--" {
            after_dashes = true;
        } else if !after_dashes && is_option(a, cmd == Some("report")) {
            if is_short_help(a) {
                return Ok(Parsed::Help);
            }
            if a.starts_with("--") && is_prefix(&option_name(a), "--help") {
                return if a.contains('=') {
                    Err(usage(format!("{a} : l'aide n'accepte pas de valeur")))
                } else {
                    Ok(Parsed::Help)
                };
            }
            let (name, inline) = match a.split_once('=') {
                Some((n, v)) if a.starts_with("--") => (n, Some(v.to_string())),
                _ => (a.as_str(), None),
            };
            if cmd != Some("report") || !a.starts_with("--") || !is_prefix(name, "--profil") {
                deferred.get_or_insert(usage(format!("option inconnue : {a}")));
                continue;
            }
            let value = match inline {
                Some(v) => v,
                None => match it.next() {
                    Some(v) if !is_option(v, true) => v.clone(),
                    _ => return Err(usage("--profil : valeur manquante".into())),
                },
            };
            if !PROFILS.contains(&value.as_str()) {
                return Err(usage(format!("--profil : valeur invalide « {value} »")));
            }
            profil = Some(value);
            if !files.is_empty() {
                run_closed = true;
            }
        } else if cmd.is_none() {
            if !matches!(a.as_str(), "info" | "report") {
                return Err(usage(format!("commande inconnue : {a}")));
            }
            cmd = Some(a.as_str());
        } else {
            if run_closed {
                deferred.get_or_insert(usage(format!("argument inattendu : {a}")));
                continue;
            }
            files.push(a.clone());
        }
    }
    if let Some(e) = deferred {
        return Err(e);
    }
    if cmd.is_none() {
        return Err(usage("commande manquante".into()));
    }
    if files.is_empty() {
        return Err(usage("fichier manquant".into()));
    }
    Ok(if cmd == Some("info") {
        Parsed::Info(files)
    } else {
        Parsed::Report(files, profil)
    })
}

/// Exécute la commande en remettant son texte au puits. `Err((code, message))` : usage faux (2) ou analyse en échec (1).
pub fn run(args: &[String], out: Sink) -> Result<(), (i32, String)> {
    match parse(args)? {
        Parsed::Help => out(&format!("{USAGE}\n")),
        Parsed::Info(files) => info_to(&files, out),
        Parsed::Report(files, profil) => {
            report_to(&files, profil.as_deref(), out).map_err(|e| (1, format!("erreur : {e}")))?;
        }
    }
    Ok(())
}

/// Point d'entrée du binaire : le texte va sur la sortie standard au fil de l'eau, les erreurs sur l'erreur standard.
pub fn run_stdio(args: &[String]) -> i32 {
    let mut write = |s: &str| print!("{s}");
    match run(args, &mut write) {
        Ok(()) => 0,
        Err((code, msg)) => {
            eprintln!("{msg}");
            code
        }
    }
}
