//! `otb-parity list|check DOSSIER [--rtol X]` : DOSSIER = `$OTB_LOGS/golden` (contient `modules/NNN/*.npz`).

use otb_parity::{compare, registry, value};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Default)]
struct Stat {
    cases: usize,
    ported: usize,
    failed: usize,
    unreadable: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(mode), Some(dir)) = (args.first(), args.get(1)) else {
        eprintln!("usage : otb-parity list|check|check-header|check-decoder DOSSIER [--rtol X]");
        std::process::exit(2);
    };
    let rtol = args
        .iter()
        .position(|a| a == "--rtol")
        .map_or(0.0, |i| args[i + 1].parse().expect("--rtol"));
    if mode == "check-decoder" {
        let root = std::env::var("OTB_LOGS")
            .unwrap_or_else(|_| format!("{}/Documents/Drones/Logs", std::env::var("HOME").unwrap()));
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get().min(6));
        let rep = otb_parity::layers::check_decoder(&PathBuf::from(dir), &PathBuf::from(&root), threads)
            .unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(2)
            });
        for f in rep.failures.iter().take(15) {
            println!("≠ {f}");
        }
        println!(
            "décodeur : {} sessions comparées, {} fichiers différents",
            rep.checked,
            rep.failures.len()
        );
        std::process::exit(if rep.failures.is_empty() { 0 } else { 1 });
    }
    if mode == "check-header" {
        let root = std::env::var("OTB_LOGS")
            .unwrap_or_else(|_| format!("{}/Documents/Drones/Logs", std::env::var("HOME").unwrap()));
        let rep = otb_parity::layers::check_header(&PathBuf::from(dir), &PathBuf::from(&root)).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(2)
        });
        for f in rep.failures.iter().take(15) {
            println!("≠ {f}");
        }
        println!(
            "en-têtes : {} sessions comparées, {} ignorées (Python plante), {} différentes",
            rep.checked,
            rep.skipped,
            rep.failures.len()
        );
        std::process::exit(if rep.failures.is_empty() { 0 } else { 1 });
    }
    let check = match mode.as_str() {
        "check" => true,
        "list" => false,
        _ => {
            eprintln!("mode inconnu : {mode}");
            std::process::exit(2)
        }
    };
    let mut files: Vec<PathBuf> = Vec::new();
    for log in std::fs::read_dir(PathBuf::from(dir).join("modules")).expect("dossier modules/") {
        for f in std::fs::read_dir(log.unwrap().path()).unwrap() {
            files.push(f.unwrap().path());
        }
    }
    files.sort();
    let mut stats: BTreeMap<String, Stat> = BTreeMap::new();
    let mut shown = 0;
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let function = name.rsplitn(3, '.').nth(2).unwrap_or(&name).to_string();
        let st = stats.entry(function.clone()).or_default();
        st.cases += 1;
        let case = match value::load_case(path) {
            Ok(c) => c,
            Err(e) => {
                st.unreadable += 1;
                eprintln!("illisible : {e}");
                continue;
            }
        };
        let Some(port) = registry::lookup(&function) else {
            continue;
        };
        st.ported += 1;
        if !check {
            continue;
        }
        let verdict = match (&case.result, port(&case)) {
            (Ok(want), Ok(got)) => compare::equal(want, &got, compare::Tol { rtol }),
            (Err(_), Err(_)) => Ok(()),
            (want, got) => Err(format!("Python {want:?} ; Rust {got:?}")),
        };
        if let Err(e) = verdict {
            st.failed += 1;
            if shown < 15 {
                shown += 1;
                println!("≠ {} : {e}", path.display());
            }
        }
    }
    let (mut ported, mut failed, mut unreadable) = (0, 0, 0);
    println!("{:<40} {:>6} {:>7} {:>6}", "fonction", "appels", "portés", "échecs");
    for (f, s) in &stats {
        println!("{f:<40} {:>6} {:>7} {:>6}", s.cases, s.ported, s.failed);
        ported += s.ported;
        failed += s.failed;
        unreadable += s.unreadable;
    }
    let with_port = stats.iter().filter(|(_, s)| s.ported > 0).count();
    println!(
        "{} fichiers, {} fonctions ({with_port} portées), {ported} appels rejoués, {failed} échecs, {unreadable} illisibles (rtol={rtol})",
        files.len(),
        stats.len()
    );
    std::process::exit(if failed + unreadable > 0 { 1 } else { 0 });
}
