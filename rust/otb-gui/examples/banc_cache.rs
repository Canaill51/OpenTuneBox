//! Banc de mémoire du **cache des outils** (`state.rs:Cache`, point 9 de W-035, lot 15 de `docs/plan.md`).
//!
//! ```text
//! cargo run --release -p otb-gui --example banc_cache -- SORTIE.json [--limit N] [--part i/K]
//! ```
//!
//! Ce que le banc mesure, session par session, sur le corpus (`OTB_LOGS`) : ce que **pèse une entrée** de chacun des
//! quatre types rangés dans le cache — simulateur de filtres (`FilterSimResult`), réponse indicielle (`FileData`),
//! assistant (`Report`), guide (`CheckResult`, rapide et complète). Le simulateur est mesuré **deux fois**, avec deux
//! jeux de réglages différents : c'est lui dont la clé contient tous les réglages modifiés, donc le seul qui peut
//! accumuler sans fin ; la deuxième mesure montre que la deuxième entrée coûte bien un deuxième bloc de mémoire
//! (rien n'est partagé entre deux entrées).
//!
//! **Comment** : un allocateur qui compte (`Compteur`). Les octets *vivants* sont relevés avant et après le calcul ;
//! les tableaux de travail du calcul sont rendus avant qu'il ne finisse, donc la différence est exactement ce que la
//! valeur gardée en cache retient. Aucun poids écrit à la main, donc rien à tenir à jour quand un type change.
//!
//! **Pourquoi un seul fil** (`map_files` avec `-j 1`) : le compteur est global au processus ; deux logs calculés en
//! même temps mêleraient leurs octets. Pour aller plus vite, lancer **plusieurs processus** avec `--part i/K`
//! (chacun compte les siens, chacun écrit son JSON) et dépouiller les fichiers ensemble.
//!
//! **Témoin** : au démarrage, le banc mesure un `Vec<f64>` de taille connue et **panique** si le compteur ne retrouve
//! pas ses octets. Une mesure dont l'instrument ment ne vaut rien (L-071 : un contrôle qui ne peut pas échouer ne
//! prouve rien).
//!
//! Ce qui n'est **pas** mesuré : la mémoire des logs eux-mêmes (affaire d'`otb-dev bench-open`), ni les courbes du
//! viewer (`banc_cadre`).

// Seule exception à `unsafe_code = "deny"` (rust/Cargo.toml) : l'allocateur qui compte les octets ne s'écrit qu'en `unsafe`.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use otb_advisor::rules::analyze;
use otb_dev::corpus::json::{dump, num};
use otb_dev::corpus::{Args, analysed, log_files, logs_root, map_files, rel};
use otb_gui::guide_page::run_check;
use otb_gui::state::{Cache, Famille};
use otb_gui::tools::filter_sim_calc::{Editor, simulate_filters};
use otb_gui::tools::step_common::{StepOpts, compute_file_data};
use otb_log::flightlog::{FlightLog, load};
use serde_json::{Map, Value, json};

/// Octets vivants : chaque allocation ajoute sa taille, chaque libération la retire.
static VIVANT: AtomicUsize = AtomicUsize::new(0);

struct Compteur;

// ⚠️ Compter dans `realloc` aussi : sans lui, un `Vec` qui grandit ferait apparaître des octets venus de nulle part (L-071).
unsafe impl GlobalAlloc for Compteur {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            VIVANT.fetch_add(l.size(), Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        VIVANT.fetch_sub(l.size(), Ordering::Relaxed);
        unsafe { System.dealloc(p, l) };
    }

    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            VIVANT.fetch_add(l.size(), Ordering::Relaxed);
        }
        p
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, taille: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, taille) };
        if !q.is_null() {
            VIVANT.fetch_add(taille, Ordering::Relaxed);
            VIVANT.fetch_sub(l.size(), Ordering::Relaxed);
        }
        q
    }
}

#[global_allocator]
static ALLOC: Compteur = Compteur;

fn vivant() -> usize {
    VIVANT.load(Ordering::Relaxed)
}

/// Valeur de `f` et octets qu'elle **retient** (différence des octets vivants : le travail intermédiaire est déjà rendu).
fn poids<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let avant = vivant();
    let v = f();
    (v, vivant().saturating_sub(avant))
}

/// Vérifie l'instrument avant de s'en servir : un tableau de taille connue doit peser ses octets, à la place du `Vec` près.
fn temoin() {
    const N: usize = 100_000;
    let attendu = N * std::mem::size_of::<f64>();
    let (v, mesure) = poids(|| vec![0.0f64; N]);
    assert_eq!(v.len(), N);
    assert!(
        mesure >= attendu && mesure <= attendu + 4096,
        "témoin : {mesure} octets mesurés pour {attendu} attendus — le compteur ne dit pas la vérité, aucune mesure de ce banc n'est utilisable"
    );
    drop(v);
}

/// Deuxième jeu de réglages du simulateur : un coup de curseur sur la coupure du LPF1 gyro, comme en fait l'utilisateur.
fn autre_reglage(e: &Editor) -> Editor {
    let mut e = e.clone();
    e.g1.static_hz += 10;
    e.g1.min_hz += 10;
    e.g1.max_hz += 10;
    e
}

/// Les poids d'une session, ou `None` si l'assistant l'écarte (moins de 8 s).
fn mesure_session(lg: &FlightLog, f: &Path, root: &Path) -> Result<Option<Value>, String> {
    let Some(sel) = analysed(lg) else { return Ok(None) };
    let base = &lg.settings;
    let cache = Cache::default();
    let mut r = Map::new();
    r.insert("file".into(), json!(rel(f, root)));
    r.insert("sess".into(), json!(lg.session));
    r.insert("fs".into(), num(lg.fs));
    r.insert("n".into(), json!(sel.len()));
    r.insert("duree_s".into(), num(sel.len() as f64 / lg.fs));
    let mut total = 0usize;
    // Chaque type est mesuré **deux fois**, la deuxième avec des réglages différents : c'est bien une deuxième entrée
    // du cache, et c'est elle qui donne le coût d'une entrée de plus. Un premier poids nettement supérieur au second
    // dénonce une allocation faite une fois pour toutes par le calcul (plan FFT, tampon global), qui n'est pas le
    // poids du cache : le dépouillement se fie au second.
    let mut poids_mesures: Vec<(String, usize)> = Vec::new();
    let mut mesure = |nom: &str, un: usize, deux: usize| {
        poids_mesures.push((format!("{nom}_1"), un));
        poids_mesures.push((format!("{nom}_2"), deux));
        total += un + deux;
    };

    // assistant : une entrée par profil choisi (deuxième mesure : un profil imposé)
    let (rep, o1) = poids(|| analyze(lg, Some(sel.clone()), None));
    let rep = rep?;
    // part des fenêtres individuelles (`StepResult::responses`) : la page Assistant ne trace que `mean`, `std` et
    // `t_ms`, jamais ces fenêtres-là — savoir ce qu'elles pèsent dit si le poids du cache vient d'un vrai besoin
    let brutes: usize = rep
        .steps
        .iter()
        .map(|st| st.result.responses.len() * std::mem::size_of::<f64>())
        .sum();
    cache.put(Famille::Assistant, "advisor:1", std::sync::Arc::new(rep));
    let autre_profil = otb_advisor::profiles::profile("five");
    let (rep2, o2) = poids(|| analyze(lg, Some(sel.clone()), autre_profil));
    cache.put(Famille::Assistant, "advisor:2", std::sync::Arc::new(rep2?));
    mesure("advisor", o1, o2);

    // réponse indicielle : une entrée par jeu de réglages de calcul
    let (fd, o1) = poids(|| compute_file_data(lg, sel.clone(), &StepOpts::default()));
    cache.put(Famille::Step, "step:1", std::sync::Arc::new(fd?));
    let autres = StepOpts {
        seg_s: 1.5,
        ..StepOpts::default()
    };
    let (fd2, o2) = poids(|| compute_file_data(lg, sel.clone(), &autres));
    cache.put(Famille::Step, "step:2", std::sync::Arc::new(fd2?));
    mesure("step", o1, o2);

    // guide : une rapide et une complète par période (deux entrées distinctes, pas deux fois la même)
    let (gq, o1) = poids(|| run_check(lg, sel.clone(), "banc", false, None));
    cache.put(Famille::Guide, "guide-quick", std::sync::Arc::new(gq));
    let (gf, o2) = poids(|| run_check(lg, sel.clone(), "banc", true, None));
    cache.put(Famille::Guide, "guide-full", std::sync::Arc::new(gf));
    mesure("guide", o1, o2);

    // simulateur de filtres : la clé contient tous les réglages modifiés, donc une entrée par coup de curseur
    let e = Editor::from_settings(base);
    let modf = e.modified_settings(base);
    let (fs1, o1) = poids(|| simulate_filters(lg, sel.clone(), base, &modf, true));
    cache.put(Famille::Filtres, "filtres:1", std::sync::Arc::new(fs1?));
    let modf2 = autre_reglage(&e).modified_settings(base);
    let (fs2, o2) = poids(|| simulate_filters(lg, sel.clone(), base, &modf2, true));
    cache.put(Famille::Filtres, "filtres:2", std::sync::Arc::new(fs2?));
    mesure("filtres", o1, o2);

    for (k, v) in poids_mesures {
        r.insert(k, json!(v));
    }
    r.insert("advisor_brutes".into(), json!(brutes));
    r.insert("entrees".into(), json!(cache.len()));
    r.insert("total".into(), json!(total));
    Ok(Some(Value::Object(r)))
}

fn mesure_fichier(f: &Path, root: &Path) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for lg in load(f).map_err(|e| e.to_string())? {
        if let Some(v) = mesure_session(&lg, f, root)? {
            out.push(v);
        }
    }
    Ok(out)
}

/// `--part i/K` : la tranche `i` (à partir de 1) d'un découpage en `K` parts, un fichier sur `K` (les tailles se
/// répartissent ainsi d'elles-mêmes entre les processus).
fn part(spec: &str) -> Result<(usize, usize), String> {
    let (i, k) = spec.split_once('/').ok_or("--part attend i/K")?;
    let (i, k): (usize, usize) = (
        i.parse().map_err(|_| "--part attend i/K")?,
        k.parse().map_err(|_| "--part attend i/K")?,
    );
    if i == 0 || k == 0 || i > k {
        return Err(format!("--part {spec} : i doit aller de 1 à K"));
    }
    Ok((i - 1, k))
}

fn principal() -> Result<String, String> {
    const USAGE: &str = "usage : banc_cache SORTIE.json [--limit N] [--part i/K]";
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let a = Args::parse(&args, &["--limit", "--part"], &[])?;
    let sortie = *a.positional.first().ok_or(USAGE)?;
    let root = logs_root();
    let mut files = log_files(&root).map_err(|e| format!("{} : {e}", root.display()))?;
    if let Some((i, k)) = a.text("--part").map(part).transpose()? {
        files = files
            .into_iter()
            .enumerate()
            .filter(|(j, _)| j % k == i)
            .map(|(_, f)| f)
            .collect();
    }
    if let Some(n) = a.number("--limit")? {
        files.truncate(n);
    }
    temoin();
    // un fil : le compteur est global au processus (voir l'en-tête)
    let lignes: Vec<Value> = map_files(|f| mesure_fichier(f, &root), &files, Some(1), &root)
        .into_iter()
        .flat_map(|(_, v)| v)
        .collect();
    std::fs::write(sortie, dump(&Value::Array(lignes.clone()))?).map_err(|e| format!("{sortie} : {e}"))?;
    Ok(format!("{} sessions mesurées → {sortie}", lignes.len()))
}

fn main() {
    match principal() {
        Ok(msg) => println!("{msg}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}
