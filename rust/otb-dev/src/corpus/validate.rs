//! `otb-dev corpus validate` : compare notre décodeur à `blackbox_decode` (la référence) sur des logs (remplace
//! `scripts/validate_corpus.py`, docs/validation.md).
//!
//! ```text
//! otb-dev corpus validate [DOSSIER_OU_FICHIER …] [--decoder CHEMIN] [--limit N] [--start N] [-j FILS]
//! ```
//!
//! Pour chaque session : mêmes lignes (alignées sur l'horodatage `time`) et mêmes valeurs entières pour toutes les colonnes
//! communes. Statuts : `ok`, `gap` (trame tronquée avant un trou SD, rejetée chez nous), `slow` (écart sur une donnée lente),
//! `rows` (lignes de la référence absentes chez nous), `values` (écart de valeur), `missing` (session non décodée), `noref`
//! (référence vide), `refhang` (la référence boucle : on vérifie seulement que nous lisons le fichier). Seuls `ok`, `gap` et
//! `refhang` sont conformes ; le code de sortie est 0 si toutes les sessions le sont.
//!
//! Différences assumées avec Python : une référence introuvable, un CSV sans colonne `time` ou au nom inattendu donnent une erreur
//! claire (Python plante) ; le texte d'une erreur de décodage n'est pas celui de `repr(e)` ; les fichiers sont traités en parallèle
//! (`-j 1` : un à la fois) et la sortie est écrite à la fin, dans l'ordre des fichiers.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use otb_decoder::file::{DecodedSession, decode_file};

use super::{Args, Output, logs_root, map_files};

/// Délai laissé à la référence avant de la déclarer bloquée (elle boucle parfois sur des logs abîmés).
const REF_TIMEOUT: Duration = Duration::from_secs(180);
/// Fils par défaut : la plus grosse session du corpus (620 000 trames) pèse ≈ 300 Mo en tableau, donc pas « tous les cœurs ».
const DEFAULT_JOBS: usize = 4;

pub const USAGE: &str =
    "Usage :\n  otb-dev corpus validate [DOSSIER_OU_FICHIER …] [--decoder CHEMIN] [--limit N] [--start N] [-j FILS]";

/// Résultat d'une session.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionResult {
    pub session: usize,
    pub status: &'static str,
    pub detail: String,
}

/// Statuts qui comptent comme conformes.
pub fn conforming(status: &str) -> bool {
    matches!(status, "ok" | "gap" | "refhang")
}

/// CSV de la référence : colonnes nommées (unité retirée), valeurs rangées ligne après ligne (illisible = NaN).
#[derive(Debug, Default)]
pub struct RefCsv {
    pub names: Vec<String>,
    pub rows: usize,
    pub data: Vec<f64>,
}

impl RefCsv {
    fn col(&self, k: usize) -> impl Iterator<Item = f64> + '_ {
        (0..self.rows).map(move |r| self.data[r * self.names.len() + k])
    }
}

/// Lignes d'un texte comme le fait Python en mode texte (`\n`, `\r\n` et `\r` finissent une ligne).
fn text_lines(bytes: &[u8]) -> Vec<&[u8]> {
    let (mut out, mut start, mut i) = (Vec::new(), 0, 0);
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                out.push(&bytes[start..i]);
                start = i + 1;
            }
            b'\r' => {
                out.push(&bytes[start..i]);
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < bytes.len() {
        out.push(&bytes[start..]);
    }
    out
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

fn number(field: &[u8]) -> f64 {
    std::str::from_utf8(field)
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(f64::NAN)
}

/// Lit un CSV de `blackbox_decode` (latin-1) : en-tête coupé à « (unité) », champs illisibles = NaN, lignes complétées de NaN
/// ou tronquées à la largeur de l'en-tête.
pub fn parse_ref_csv(bytes: &[u8]) -> RefCsv {
    let lines = text_lines(bytes);
    let Some((head, body)) = lines.split_first() else {
        return RefCsv {
            names: vec![String::new()],
            ..RefCsv::default()
        };
    };
    let names: Vec<String> = latin1(head)
        .split(',')
        .map(|n| n.trim().split(" (").next().unwrap_or("").to_string())
        .collect();
    let width = names.len();
    let mut data = vec![f64::NAN; body.len() * width];
    for (r, line) in body.iter().enumerate() {
        for (k, field) in line.split(|&b| b == b',').take(width).enumerate() {
            data[r * width + k] = number(field);
        }
    }
    RefCsv {
        names,
        rows: body.len(),
        data,
    }
}

/// Valeurs triées avec leur indice, la première apparition d'une valeur répétée en tête : la fusion de `compare_session`
/// passe aux deux listes suivantes dès qu'elle a pris une paire, donc les doublons suivants ne sont jamais appariés.
fn sorted_with_index(v: &[i64]) -> Vec<(i64, usize)> {
    let mut idx: Vec<(i64, usize)> = v.iter().copied().zip(0..).collect();
    idx.sort(); // à valeur égale, l'indice le plus petit d'abord
    idx
}

/// Un flottant écrit comme `repr` de Python : `123.0`, `1e-05`, `1.8446744073709552e+19`.
fn py_float(x: f64) -> String {
    if x.is_nan() || x.is_infinite() || x == 0.0 || (1e-4..1e16).contains(&x.abs()) {
        return if x.is_nan() { "nan".into() } else { format!("{x:?}") };
    }
    let e = format!("{x:e}");
    let (mantissa, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    format!("{mantissa}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs())
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Compare une session de la référence à la nôtre. `t_our` : horodatages de nos trames ; `column` : colonne par nom (principale
/// puis lente) ; `slow` : noms des colonnes lentes.
pub fn compare_session(
    refcsv: &RefCsv,
    t_our: &[i64],
    column: impl Fn(&str) -> Option<Vec<i64>>,
    slow: &HashSet<String>,
) -> Result<(&'static str, String), String> {
    let time_col = refcsv
        .names
        .iter()
        .position(|n| n == "time")
        .ok_or("colonne « time » absente de la référence")?;
    // `as i64` : un NaN donne 0 (Python : valeur indéfinie)
    let t_ref: Vec<i64> = refcsv.col(time_col).map(|x| x as i64).collect();
    let (ur, uo) = (sorted_with_index(&t_ref), sorted_with_index(t_our));
    // intersection des deux listes triées : horodatage commun, indice côté référence, indice côté nous
    let (mut common, mut ir, mut io) = (Vec::new(), Vec::new(), Vec::new());
    let (mut i, mut j) = (0, 0);
    while i < ur.len() && j < uo.len() {
        match ur[i].0.cmp(&uo[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                common.push(ur[i].0);
                ir.push(ur[i].1);
                io.push(uo[j].1);
                i += 1;
                j += 1;
            }
        }
    }
    let width = refcsv.names.len();
    let (mut bad, mut bad_slow) = (Vec::new(), Vec::new());
    for (k, n) in refcsv.names.iter().enumerate() {
        if n == "loopIteration" {
            continue;
        }
        let Some(col) = column(n) else { continue };
        let (mut count, mut first) = (0usize, None);
        for (p, (&r, &o)) in ir.iter().zip(&io).enumerate() {
            let a = refcsv.data[r * width + k];
            let b = col[o] as f64;
            if !(a.is_nan() || a == b || a == b.rem_euclid(4294967296.0)) {
                count += 1;
                first.get_or_insert((p, a, b));
            }
        }
        if let Some((p, a, b)) = first {
            let msg = format!(
                "{n}: {count} écarts (ex. t={}: ref={} nous={})",
                common[p],
                py_float(a),
                py_float(b)
            );
            if slow.contains(n) { &mut bad_slow } else { &mut bad }.push(msg);
        }
    }

    // Lignes de la référence absentes chez nous : tolérées si elles précèdent immédiatement un trou d'enregistrement
    // (trame tronquée par une pause d'écriture SD, que la référence accepte avec des valeurs parasites).
    let ours: HashSet<i64> = t_our.iter().copied().collect();
    let mut missing: Vec<i64> = t_ref.iter().copied().filter(|t| !ours.contains(t)).collect();
    missing.sort_unstable();
    missing.dedup();
    let period = if t_ref.len() > 1 {
        let mut d: Vec<f64> = t_ref.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
        median(&mut d)
    } else {
        1.0
    };
    let (mut n_gap, mut n_other) = (0usize, 0usize);
    for &m in &missing {
        let pos = t_ref.partition_point(|&t| t < m);
        let next = if pos + 1 < t_ref.len() {
            t_ref[pos + 1] as f64
        } else {
            t_ref[t_ref.len() - 1] as f64 + 10.0 * period
        };
        if next - m as f64 > 3.0 * period {
            n_gap += 1;
        } else {
            n_other += 1;
        }
    }
    let extra = t_our.len() - common.len();
    let mut status = "ok";
    let mut notes = Vec::new();
    if extra > 0 {
        notes.push(format!("+{extra} trames valides en plus chez nous"));
    }
    if n_gap > 0 {
        status = "gap";
        notes.push(format!("{n_gap} trame(s) tronquée(s) avant un trou SD rejetée(s)"));
    }
    if !bad_slow.is_empty() {
        status = "slow";
        notes.push(format!(
            "données lentes : {}",
            bad_slow.iter().take(2).cloned().collect::<Vec<_>>().join("; ")
        ));
    }
    if n_other > 0 {
        status = "rows";
        notes.push(format!("{n_other} lignes de la référence absentes chez nous"));
    }
    if !bad.is_empty() {
        status = "values";
        notes.push(bad.iter().take(4).cloned().collect::<Vec<_>>().join("; "));
    }
    Ok((status, notes.join(" | ")))
}

/// Lance la référence sur `path` ; `Ok(false)` si elle n'a pas fini dans le délai (elle est alors arrêtée).
fn run_reference(reference: &Path, path: &Path, out: &Path, timeout: Duration) -> Result<bool, String> {
    let mut child = Command::new(reference)
        .args([
            "--unit-flags",
            "raw",
            "--unit-vbat",
            "raw",
            "--unit-amperage",
            "raw",
            "--output-dir",
        ])
        .arg(out)
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{} : {e}", reference.display()))?;
    let start = Instant::now();
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Ok(true); // le code de sortie est ignoré, comme en Python
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Compare un fichier à la référence : un résultat par session de la référence.
fn compare_file(path: &Path, reference: &Path, tmp: &Path, timeout: Duration) -> Result<Vec<SessionResult>, String> {
    let _ = std::fs::remove_dir_all(tmp);
    std::fs::create_dir_all(tmp).map_err(|e| format!("{} : {e}", tmp.display()))?;
    let res = compare_with_dir(path, reference, tmp, timeout);
    let _ = std::fs::remove_dir_all(tmp);
    res
}

fn compare_with_dir(
    path: &Path,
    reference: &Path,
    tmp: &Path,
    timeout: Duration,
) -> Result<Vec<SessionResult>, String> {
    if !run_reference(reference, path, tmp, timeout)? {
        // la référence boucle parfois indéfiniment sur des logs abîmés : on vérifie quand même que notre décodeur lit le fichier
        let detail = match decode_file(path, None) {
            Ok(s) => format!("référence bloquée ; notre décodeur lit {} session(s)", s.len()),
            Err(e) => format!("référence bloquée ; notre décodeur échoue : {e}"),
        };
        return Ok(vec![SessionResult {
            session: 0,
            status: "refhang",
            detail,
        }]);
    }
    let decoded = std::panic::catch_unwind(|| decode_file(path, None))
        .map_err(|_| "panique du décodeur".to_string())
        .and_then(|r| r.map_err(|e| e.to_string()));
    let (sessions, err): (Vec<DecodedSession>, Option<String>) = match decoded {
        Ok(s) => (s, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let mut refs: Vec<PathBuf> = std::fs::read_dir(tmp)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            n.ends_with(".csv") && !n.ends_with(".gps.csv") && !n.ends_with(".headers.csv")
        })
        .collect();
    refs.sort();
    let mut results = Vec::new();
    for r in &refs {
        let name = r.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let index: usize = name
            .rsplit('.')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| format!("nom de CSV inattendu : {name}"))?;
        let Some(s) = sessions.iter().find(|s| s.index == index) else {
            results.push(SessionResult {
                session: index,
                status: "missing",
                detail: err.clone().unwrap_or_else(|| "session non décodée".into()),
            });
            continue;
        };
        let csv = parse_ref_csv(&std::fs::read(r).map_err(|e| format!("{name} : {e}"))?);
        let t_our: Vec<i64> = (0..s.main.rows).map(|i| s.main.row(i)[1]).collect();
        let slow: HashSet<String> = s.slow_names.iter().cloned().collect();
        let (status, detail) =
            compare_session(&csv, &t_our, |n| s.column(n), &slow).map_err(|e| format!("{name} : {e}"))?;
        results.push(SessionResult {
            session: index,
            status,
            detail,
        });
    }
    if refs.is_empty() && !sessions.is_empty() {
        results.push(SessionResult {
            session: 0,
            status: "noref",
            detail: "référence vide".into(),
        });
    }
    Ok(results)
}

/// Logs d'un dossier (récursif), triés par composantes de chemin, sans dédoublonnage.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            collect(&p, out)?;
        } else if p
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| matches!(x.to_lowercase().as_str(), "bbl" | "bfl"))
        {
            out.push(p);
        }
    }
    Ok(())
}

/// Chemin affiché : relatif au dossier des logs, sinon sans la racine.
fn shown(path: &Path, root: &Path) -> String {
    let r = path
        .strip_prefix(root)
        .or_else(|_| path.strip_prefix("/"))
        .unwrap_or(path);
    r.display().to_string()
}

/// `{'ok': 3, 'gap': 1}` : les statuts dans l'ordre de leur première apparition, comme le `dict` de Python.
fn by_status(all: &[&SessionResult]) -> String {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for r in all {
        match counts.iter_mut().find(|(s, _)| *s == r.status) {
            Some(c) => c.1 += 1,
            None => counts.push((r.status, 1)),
        }
    }
    let items: Vec<String> = counts.iter().map(|(s, n)| format!("'{s}': {n}")).collect();
    format!("{{{}}}", items.join(", "))
}

/// Texte final et code de sortie à partir des résultats de chaque fichier (`files` et `results` sont dans le même ordre).
pub fn report(files: &[PathBuf], first: usize, total: usize, results: &[Vec<SessionResult>], root: &Path) -> Output {
    let mut text = String::new();
    for (n, (path, res)) in files.iter().zip(results).enumerate() {
        let k = first + n;
        let worst: Vec<&SessionResult> = res.iter().filter(|r| !conforming(r.status)).collect();
        let mark = if worst.is_empty() { "OK " } else { "ERR" };
        text += &format!("[{k}/{total}] {mark} {} ({} sessions)\n", shown(path, root), res.len());
        for r in worst {
            text += &format!("      session {}: {} {}\n", r.session, r.status, r.detail);
        }
    }
    let all: Vec<&SessionResult> = results.iter().flatten().collect();
    let n_ok = all.iter().filter(|r| conforming(r.status)).count();
    text += &format!("\nSessions conformes (ok + trou SD) : {n_ok}/{}\n", all.len());
    text += &format!("Par statut : {}", by_status(&all));
    Output {
        text,
        code: u8::from(n_ok != all.len()),
    }
}

pub fn run(args: &[&str]) -> Result<Output, String> {
    let a = Args::parse(args, &["--decoder", "--limit", "--start", "-j"], &[])?;
    let root = logs_root();
    let reference = match (a.text("--decoder"), std::env::var_os("OTB_REF_DECODER")) {
        (Some(p), _) => PathBuf::from(p),
        (None, Some(p)) => PathBuf::from(p),
        (None, None) => PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join(".local/share/PIDtoolboxProV094/blackbox_decode"),
    };
    if !reference.is_file() {
        return Err(format!(
            "blackbox_decode de référence introuvable : {} (option --decoder ou variable OTB_REF_DECODER)",
            reference.display()
        ));
    }
    let mut files: Vec<PathBuf> = Vec::new();
    let paths: Vec<PathBuf> = if a.positional.is_empty() {
        vec![root.clone()]
    } else {
        a.positional.iter().map(PathBuf::from).collect()
    };
    for p in paths {
        if p.is_dir() {
            let mut found = Vec::new();
            collect(&p, &mut found).map_err(|e| format!("{} : {e}", p.display()))?;
            found.sort();
            files.extend(found);
        } else {
            files.push(p);
        }
    }
    let first = a.number("--start")?.unwrap_or(1).max(1);
    if let Some(limit) = a.number("--limit")?.filter(|&l| l > 0) {
        files.truncate(first - 1 + limit);
    }
    let total = files.len();
    let todo: Vec<PathBuf> = files.into_iter().skip(first - 1).collect();
    let jobs = a.number("-j")?.unwrap_or(DEFAULT_JOBS);
    let base = std::env::temp_dir().join(format!("otb-validate-{}", std::process::id()));
    let counter = AtomicUsize::new(0);
    let done = map_files(
        |f| {
            let tmp = base.join(counter.fetch_add(1, Ordering::Relaxed).to_string());
            compare_file(f, &reference, &tmp, REF_TIMEOUT)
        },
        &todo,
        Some(jobs),
        &root,
    );
    let _ = std::fs::remove_dir_all(&base);
    if done.len() != todo.len() {
        return Err("des fichiers n'ont pas pu être comparés (voir « ERR » ci-dessus)".into());
    }
    let results: Vec<Vec<SessionResult>> = done.into_iter().map(|(_, r)| r).collect();
    Ok(report(&todo, first, total, &results, &root))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv(text: &str) -> RefCsv {
        parse_ref_csv(text.as_bytes())
    }

    /// Colonnes de notre côté : nom → valeurs.
    fn ours(cols: &[(&str, Vec<i64>)]) -> impl Fn(&str) -> Option<Vec<i64>> {
        let cols: Vec<(String, Vec<i64>)> = cols.iter().map(|(n, v)| (n.to_string(), v.clone())).collect();
        move |n| cols.iter().find(|(c, _)| c == n).map(|(_, v)| v.clone())
    }

    fn cmp(text: &str, t_our: &[i64], cols: &[(&str, Vec<i64>)], slow: &[&str]) -> (&'static str, String) {
        let slow: HashSet<String> = slow.iter().map(|s| s.to_string()).collect();
        compare_session(&csv(text), t_our, ours(cols), &slow).unwrap()
    }

    #[test]
    fn floats_are_written_like_python_repr() {
        for (x, want) in [
            (123.0, "123.0"),
            (-2147483648.0, "-2147483648.0"),
            (0.5, "0.5"),
            (1.8446744073709552e19, "1.8446744073709552e+19"),
            (1e16, "1e+16"),
            (9999999999999998.0, "9999999999999998.0"),
            (1e-5, "1e-05"),
            (0.0001, "0.0001"),
            (1.5e-7, "1.5e-07"),
            (0.0, "0.0"),
        ] {
            assert_eq!(py_float(x), want);
        }
    }

    #[test]
    fn csv_names_lose_units_and_bad_fields_are_nan() {
        let c = csv("loopIteration, time (us), gyroADC[0] (deg/s)\r\n1,100,5\r\n2,abc\r\n3,300,7,99\r\n");
        assert_eq!(c.names, ["loopIteration", "time", "gyroADC[0]"]);
        assert_eq!(c.rows, 3);
        assert_eq!(c.data[0..3], [1.0, 100.0, 5.0]);
        assert_eq!(c.data[3], 2.0);
        assert!(c.data[4].is_nan() && c.data[5].is_nan()); // champ illisible, champ manquant
        assert_eq!(c.data[6..9], [3.0, 300.0, 7.0]); // le champ en trop est ignoré
    }

    #[test]
    fn csv_lines_follow_python_text_mode() {
        let c = csv("a,b\r1,2\r\n3,4\n\n5,6");
        // lignes : en-tête, « 1,2 », « 3,4 », ligne vide (NaN), « 5,6 » sans saut final
        assert_eq!(c.rows, 4);
        assert!(c.data[4].is_nan() && c.data[5].is_nan());
        assert_eq!(c.data[6..8], [5.0, 6.0]);
        assert_eq!(csv("").names, [""]);
    }

    #[test]
    fn identical_session_is_ok_and_loop_iteration_is_ignored() {
        let text = "loopIteration,time (us),axisP[0]\n1,100,5\n2,200,6\n3,300,7\n";
        // loopIteration chez nous : valeurs différentes, mais la colonne n'est pas comparée
        let cols = [
            ("loopIteration", vec![9, 9, 9]),
            ("time", vec![100, 200, 300]),
            ("axisP[0]", vec![5, 6, 7]),
        ];
        assert_eq!(cmp(text, &[100, 200, 300], &cols, &[]), ("ok", String::new()));
    }

    #[test]
    fn value_mismatch_reports_first_time_and_count() {
        let text = "time (us),axisP[0]\n100,5\n200,6\n300,7\n";
        let cols = [("time", vec![100, 200, 300]), ("axisP[0]", vec![5, 1, 2])];
        let (st, d) = cmp(text, &[100, 200, 300], &cols, &[]);
        assert_eq!(st, "values");
        assert_eq!(d, "axisP[0]: 2 écarts (ex. t=200: ref=6.0 nous=1.0)");
    }

    #[test]
    fn nan_reference_and_modulo_2_pow_32_are_accepted() {
        let text = "time (us),a,b\n100,x,4294967295\n200,3,1\n";
        // a : NaN côté référence ; b : -1 chez nous, 2^32 − 1 dans la référence
        let cols = [("time", vec![100, 200]), ("a", vec![0, 3]), ("b", vec![-1, 1])];
        assert_eq!(cmp(text, &[100, 200], &cols, &[]).0, "ok");
    }

    #[test]
    fn slow_mismatch_gives_slow_and_values_win_over_it() {
        let text = "time (us),flightModeFlags,axisP[0]\n100,1,5\n200,1,6\n";
        let slow_bad = [
            ("time", vec![100, 200]),
            ("flightModeFlags", vec![0, 0]),
            ("axisP[0]", vec![5, 6]),
        ];
        let (st, d) = cmp(text, &[100, 200], &slow_bad, &["flightModeFlags"]);
        assert_eq!(st, "slow");
        assert!(d.starts_with("données lentes : flightModeFlags: 2 écarts"), "{d}");
        let both = [
            ("time", vec![100, 200]),
            ("flightModeFlags", vec![0, 0]),
            ("axisP[0]", vec![0, 0]),
        ];
        assert_eq!(cmp(text, &[100, 200], &both, &["flightModeFlags"]).0, "values");
    }

    #[test]
    fn missing_row_before_a_hole_is_gap_but_inside_a_run_is_rows() {
        // période 100 ; la ligne 400 manque chez nous
        let text = "time (us),a\n100,0\n200,0\n300,0\n400,0\n500,0\n";
        let cols = |t: &[i64]| [("time", t.to_vec()), ("a", vec![0; t.len()])];
        // 400 suivi de 500 : écart 100 ≤ 3 périodes → ligne manquante au milieu des données
        let t = [100, 200, 300, 500];
        let (st, d) = cmp(text, &t, &cols(&t), &[]);
        assert_eq!(
            (st, d.as_str()),
            ("rows", "1 lignes de la référence absentes chez nous")
        );
        // 400 suivi d'un trou de 1000 → trame tronquée avant un trou SD
        let text = "time (us),a\n100,0\n200,0\n300,0\n400,0\n1400,0\n1500,0\n";
        let t = [100, 200, 300, 1400, 1500];
        let (st, d) = cmp(text, &t, &cols(&t), &[]);
        assert_eq!(
            (st, d.as_str()),
            ("gap", "1 trame(s) tronquée(s) avant un trou SD rejetée(s)")
        );
    }

    #[test]
    fn extra_frames_on_our_side_are_noted_and_status_stays_ok() {
        let text = "time (us),a\n100,0\n300,0\n";
        let t = [100, 200, 300];
        let cols = [("time", t.to_vec()), ("a", vec![0, 0, 0])];
        assert_eq!(
            cmp(text, &t, &cols, &[]),
            ("ok", "+1 trames valides en plus chez nous".into())
        );
    }

    #[test]
    fn alignment_uses_the_first_occurrence_of_a_repeated_time() {
        let text = "time (us),a\n100,1\n100,9\n200,2\n";
        let cols = [("time", vec![100, 200]), ("a", vec![1, 2])];
        assert_eq!(cmp(text, &[100, 200], &cols, &[]).0, "ok");
    }

    #[test]
    fn missing_time_column_is_an_error() {
        let slow = HashSet::new();
        assert!(compare_session(&csv("a,b\n1,2\n"), &[1], ours(&[]), &slow).is_err());
    }

    #[test]
    fn report_matches_python_layout_and_exit_code() {
        let r = |session, status, detail: &str| SessionResult {
            session,
            status,
            detail: detail.into(),
        };
        let root = Path::new("/logs");
        let files = [PathBuf::from("/logs/a/x.bbl"), PathBuf::from("/other/y.bbl")];
        let results = [
            vec![r(1, "ok", ""), r(2, "gap", "d")],
            vec![r(1, "values", "axisP[0]: 1 écarts"), r(2, "ok", "")],
        ];
        let out = report(&files, 5, 9, &results, root);
        assert_eq!(
            out.text,
            "[5/9] OK  a/x.bbl (2 sessions)\n[6/9] ERR other/y.bbl (2 sessions)\n      session 1: values axisP[0]: 1 écarts\n\n\
             Sessions conformes (ok + trou SD) : 3/4\nPar statut : {'ok': 2, 'gap': 1, 'values': 1}"
        );
        assert_eq!(out.code, 1);
        let all_ok = report(&files[..1], 1, 1, &results[..1], root);
        assert_eq!(all_ok.code, 0);
        assert_eq!(
            report(&[], 1, 0, &[], root).text,
            "\nSessions conformes (ok + trou SD) : 0/0\nPar statut : {}"
        );
    }

    #[test]
    fn blocked_reference_is_killed_and_reported_as_refhang() {
        let dir = std::env::temp_dir().join(format!("otb-validate-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("fake_decode.sh");
        std::fs::write(&fake, "#!/bin/sh\nsleep 30\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let t0 = Instant::now();
        let res = compare_file(
            &dir.join("absent.bbl"),
            &fake,
            &dir.join("out"),
            Duration::from_millis(300),
        )
        .unwrap();
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "la référence n'a pas été arrêtée"
        );
        assert_eq!(res.len(), 1);
        assert_eq!((res[0].session, res[0].status), (0, "refhang"));
        assert!(
            res[0].detail.starts_with("référence bloquée ; notre décodeur échoue"),
            "{}",
            res[0].detail
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
