//! `otb-dev corpus rates` : taux de déclenchement des constats de l'assistant sur tout le corpus (remplace `scripts/advisor_rates.py`,
//! docs/assistant.md, « Calibration »).
//!
//! ```text
//! otb-dev corpus rates SORTIE.json [--limit N] [-j FILS]   passage complet (dossier : OTB_LOGS), puis le tableau
//! otb-dev corpus rates --resume SORTIE.json                tableau seul, sans recalcul
//! otb-dev corpus rates --compare AVANT.json APRÈS.json [N] constats différents, N sessions détaillées (code 1 si écart)
//! ```
//!
//! Mêmes sessions que la calibration (au moins 8 s, période analysée d'au moins 5 s). Chaque session garde le texte complet de ses
//! constats (`details`) et la durée de l'analyse : `--compare` prouve qu'une modification ne change aucun conseil, chiffres
//! affichés compris. Le JSON a les mêmes clés que celui de `advisor_rates.py` : les deux outils se lisent l'un l'autre.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::time::Instant;

use otb_advisor::rules::{ATTENTION, CONSEIL, analyze};
use otb_log::flightlog::load;
use serde_json::{Value, json};

use super::{Args, Output, analysed, log_files, logs_root, map_files, rel};

/// Les lignes (une par session analysée) d'un fichier du corpus.
pub fn analyze_file(f: &Path, root: &Path) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for lg in load(f).map_err(|e| e.to_string())? {
        let Some(sel) = analysed(&lg) else { continue };
        let t0 = Instant::now();
        let rep = analyze(&lg, Some(sel), None)?;
        let t_analyze = t0.elapsed().as_secs_f64();
        let moving = rep.measures.delays.as_ref().is_some_and(|d| d.moving());
        let findings: Vec<Value> = rep
            .findings
            .iter()
            .map(|x| json!([x.key, x.title, x.severity, !x.actions.is_empty()]))
            .collect();
        let details: Vec<Value> = rep
            .findings
            .iter()
            .map(|x| json!([x.key, x.axis, x.severity, x.title, x.detail, x.actions]))
            .collect();
        out.push(json!({
            "file": rel(f, root), "sess": lg.session, "profile": rep.profile.key, "moving": moving,
            "t_analyze": t_analyze, "findings": findings, "details": details,
        }));
    }
    Ok(out)
}

fn read_rows(path: &str) -> Result<Vec<Value>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path} : {e}"))?;
    match serde_json::from_str(&text).map_err(|e| format!("{path} : {e}"))? {
        Value::Array(rows) => Ok(rows),
        _ => Err(format!("{path} : un tableau JSON était attendu")),
    }
}

fn str_of(v: &Value, i: usize) -> &str {
    v.get(i).and_then(Value::as_str).unwrap_or("")
}

/// Tableau des taux : par constat d'attention ou de conseil (regroupé par clé, affiché par un de ses titres), la part des sessions
/// signalées : toutes, vols avec mouvements, vols stationnaires.
pub fn summary_of(rows: &[Value]) -> String {
    let moving = |r: &Value| r["moving"].as_bool().unwrap_or(false);
    let groups: [(&str, Vec<&Value>); 3] = [
        ("tous", rows.iter().collect()),
        ("mouvements", rows.iter().filter(|r| moving(r)).collect()),
        ("stationnaires", rows.iter().filter(|r| !moving(r)).collect()),
    ];
    let mut text = format!(
        "{} sessions : {}\n",
        rows.len(),
        groups
            .iter()
            .map(|(k, v)| format!("{k} {}", v.len()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let findings = |r: &Value| r["findings"].as_array().cloned().unwrap_or_default();
    // une fois par session et par clé ; ordre : première rencontre, puis du plus fréquent au moins fréquent (égalités stables)
    let mut counts: Vec<HashMap<String, usize>> = vec![HashMap::new(); groups.len()];
    let mut order: Vec<String> = Vec::new();
    for (g, (_, v)) in groups.iter().enumerate() {
        for r in v {
            let keys: BTreeSet<String> = findings(r)
                .iter()
                .filter(|f| [ATTENTION, CONSEIL].contains(&str_of(f, 2)))
                .map(|f| str_of(f, 0).to_string())
                .collect();
            // ordre de première apparition dans la session (le `set` de Python n'a pas d'ordre défini)
            let mut seen = BTreeSet::new();
            for f in findings(r) {
                let k = str_of(&f, 0).to_string();
                if keys.contains(&k) && seen.insert(k.clone()) {
                    *counts[g].entry(k.clone()).or_insert(0) += 1;
                    if g == 0 && !order.contains(&k) {
                        order.push(k);
                    }
                }
            }
        }
    }
    // un titre par clé, pour la lecture : le dernier rencontré
    let mut titles: HashMap<String, String> = HashMap::new();
    for r in rows {
        for f in findings(r) {
            titles.insert(str_of(&f, 0).to_string(), str_of(&f, 1).to_string());
        }
    }
    text += &format!(
        "{:<70} {}\n",
        "constat",
        groups
            .iter()
            .map(|(k, _)| format!("{k:>13}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    order.sort_by_key(|k| std::cmp::Reverse(counts[0][k])); // tri stable
    for key in &order {
        let label: String = format!("{key} ({})", titles[key]).chars().take(70).collect();
        let cells: Vec<String> = groups
            .iter()
            .enumerate()
            .map(|(g, (_, v))| {
                let n = *counts[g].get(key).unwrap_or(&0) as f64;
                format!("{:12.0}%", 100.0 * n / (v.len().max(1) as f64))
            })
            .collect();
        text += &format!("{label:<70} {}\n", cells.join(" "));
    }
    text.truncate(text.trim_end_matches('\n').len());
    text
}

pub fn summary(path: &str) -> Result<String, String> {
    Ok(summary_of(&read_rows(path)?))
}

/// Sessions dont les constats (texte complet) diffèrent entre deux passages ; rend le texte et leur nombre (avec celles présentes
/// d'un seul côté).
pub fn compare_rows(a: &[Value], b: &[Value], show: usize) -> (String, usize) {
    let index = |rows: &[Value]| -> HashMap<(String, i64), Value> {
        rows.iter()
            .map(|r| {
                (
                    (
                        r["file"].as_str().unwrap_or("").to_string(),
                        r["sess"].as_i64().unwrap_or(-1),
                    ),
                    r.clone(),
                )
            })
            .collect()
    };
    let (a, b) = (index(a), index(b));
    let only_one_side =
        a.keys().filter(|k| !b.contains_key(*k)).count() + b.keys().filter(|k| !a.contains_key(*k)).count();
    let mut text = format!(
        "sessions : {} avant, {} après ; présentes d'un seul côté : {only_one_side}\n",
        a.len(),
        b.len()
    );
    if a.values().chain(b.values()).any(|r| r.get("details").is_none()) {
        text += "passage sans texte complet (ancienne version du script) : comparaison des clés et titres seulement\n";
    }
    let field = |r: &Value| r.get("details").unwrap_or(&r["findings"]).clone();
    let mut diff: Vec<&(String, i64)> = a
        .keys()
        .filter(|k| b.contains_key(*k) && (field(&a[*k]) != field(&b[*k]) || a[*k]["profile"] != b[*k]["profile"]))
        .collect();
    diff.sort();
    text += &format!("sessions aux constats différents : {}\n", diff.len());
    for k in diff.iter().take(show) {
        text += &format!("== {} (session {})\n", k.0, k.1);
        let list = |r: &Value| field(r).as_array().cloned().unwrap_or_default();
        let (fa, fb) = (list(&a[*k]), list(&b[*k]));
        for x in fa.iter().filter(|x| !fb.contains(x)) {
            text += &format!("  - {x}\n");
        }
        for x in fb.iter().filter(|x| !fa.contains(x)) {
            text += &format!("  + {x}\n");
        }
    }
    let timed = |m: &HashMap<(String, i64), Value>| m.values().all(|r| r.get("t_analyze").is_some());
    if timed(&a) && timed(&b) && !a.is_empty() && !b.is_empty() {
        let t = |m: &HashMap<(String, i64), Value>| -> Vec<f64> {
            m.values().map(|r| r["t_analyze"].as_f64().unwrap_or(0.0)).collect()
        };
        let (ta, tb) = (t(&a).iter().sum::<f64>(), t(&b).iter().sum::<f64>());
        let (ma, mb) = (
            t(&a).into_iter().fold(f64::MIN, f64::max),
            t(&b).into_iter().fold(f64::MIN, f64::max),
        );
        text += &format!(
            "durée cumulée de l'assistant : {ta:.0} s → {tb:.0} s ; session la plus lente : {ma:.1} s → {mb:.1} s\n"
        );
    }
    text.truncate(text.trim_end_matches('\n').len());
    (text, diff.len() + only_one_side)
}

const USAGE: &str = "Usage :\n  otb-dev corpus rates SORTIE.json [--limit N] [-j FILS]\n  otb-dev corpus rates --resume SORTIE.json\n  otb-dev corpus rates --compare AVANT.json APRÈS.json [N]";

pub fn run(args: &[&str]) -> Result<Output, String> {
    let a = Args::parse(args, &["--limit", "-j"], &["--resume", "--compare"])?;
    match (a.has("--resume"), a.has("--compare"), a.positional.as_slice()) {
        (true, false, [path]) => Ok(Output::ok(summary(path)?)),
        (false, true, [old, new, rest @ ..]) if rest.len() <= 1 => {
            let show = match rest.first() {
                Some(n) => n.parse().map_err(|_| format!("N attend un entier, reçu « {n} »"))?,
                None => 10,
            };
            let (text, n) = compare_rows(&read_rows(old)?, &read_rows(new)?, show);
            Ok(Output {
                text,
                code: u8::from(n > 0),
            })
        }
        (false, false, [path]) => {
            let (limit, jobs) = (a.number("--limit")?, a.number("-j")?);
            let root = logs_root();
            let mut files = log_files(&root).map_err(|e| format!("{} : {e}", root.display()))?;
            files.truncate(limit.unwrap_or(files.len()));
            let res = map_files(|f| analyze_file(f, &root), &files, jobs, &root);
            let rows: Vec<Value> = res.into_iter().flat_map(|(_, rows)| rows).collect();
            std::fs::write(path, serde_json::to_string(&rows).map_err(|e| e.to_string())?)
                .map_err(|e| format!("{path} : {e}"))?;
            Ok(Output::ok(summary_of(&rows)))
        }
        _ => Err(USAGE.into()),
    }
}
