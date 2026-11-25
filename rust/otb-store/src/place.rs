//! Où vit la bibliothèque, et son déplacement sans rien perdre (lot 34, D-081).
//!
//! Le dossier par défaut est calculé par `default_dir` ; l'utilisateur peut en choisir un autre (disque plus grand,
//! dossier sauvegardé, même bibliothèque d'un PC à l'autre). Un **pointeur** (`emplacement-bibliotheque.json`), rangé
//! à côté du dossier par défaut, dit alors où elle est : sans lui, l'application ne saurait pas où chercher au
//! lancement. Absent, rien ne change pour qui ne déplace rien.
//!
//! **Déplacement** : tout copier dans le nouveau dossier, **relire chaque fichier** et le comparer à l'original,
//! basculer, puis seulement effacer l'ancien dossier — même règle que l'USB : un effacement reporté ne coûte rien.
//! Pendant la copie, le nouveau dossier porte une marque (`deplacement-en-cours`) : tant qu'elle est là, il n'est pas
//! une bibliothèque, et une copie interrompue n'est jamais prise pour une bibliothèque complète.
//!
//! **Ce qui ne se fait jamais en silence** : recréer une bibliothèque vide à la place d'un dossier introuvable
//! (disque débranché), fusionner deux bibliothèques, écraser une bibliothèque trouvée dans le dossier choisi.

use serde_json::{Map, Value};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{FOLDER, TABLES};

/// Le pointeur, dans le dossier **parent** du dossier par défaut : il survit au déplacement de la bibliothèque.
pub const POINTER: &str = "emplacement-bibliotheque.json";

/// La marque d'un dossier en cours de remplissage : il n'est pas (encore) une bibliothèque.
pub const MOVING: &str = "deplacement-en-cours";

/// Le témoin d'ouverture : quel poste a écrit la bibliothèque, et quand (D-081, dossier synchronisé sur deux PC).
pub const WITNESS: &str = "ouverte-par.json";

/// Nom du sous-dossier créé dans un dossier choisi qui n'est pas vide (« D:\Drones » reçoit « D:\Drones\OpenTuneBox »).
pub const SUBFOLDER: &str = "OpenTuneBox";

/// Version du format du pointeur et du témoin.
const VERSION: u64 = 1;

/// Délai au-delà duquel un témoin d'un autre poste est jugé ancien (un plantage ne bloque rien) : 10 minutes, soit
/// vingt enregistrements automatiques de l'interface (toutes les 30 s).
pub const WITNESS_FRESH_S: u64 = 600;

/// Où chercher la bibliothèque au lancement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// Le dossier à ouvrir. `chosen` : désigné par le pointeur (sinon c'est le dossier par défaut, créé à la
    /// première écriture s'il manque).
    Ready { dir: PathBuf, chosen: bool },
    /// Le pointeur désigne un dossier absent ou sans bibliothèque, ou il est illisible : **ne rien ouvrir, ne rien
    /// écrire** (sinon une bibliothèque vide serait recréée sans le dire). `why` est à montrer à l'utilisateur.
    Missing { dir: Option<PathBuf>, why: String },
}

fn pointer_path(default_dir: &Path) -> PathBuf {
    default_dir
        .parent()
        .map_or_else(|| PathBuf::from(POINTER), |p| p.join(POINTER))
}

/// Le dossier contient une bibliothèque : au moins un de ses fichiers de table, et pas de copie en cours.
pub fn is_library(dir: &Path) -> bool {
    !dir.join(MOVING).exists() && TABLES.iter().any(|t| dir.join(t).is_file())
}

/// Où est la bibliothèque, d'après le pointeur rangé à côté de `default_dir`.
pub fn locate(default_dir: &Path) -> Place {
    let pointer = pointer_path(default_dir);
    let text = match std::fs::read_to_string(&pointer) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Place::Ready {
                dir: default_dir.to_path_buf(),
                chosen: false,
            };
        }
        Err(e) => {
            return Place::Missing {
                dir: None,
                why: format!(
                    "le fichier qui dit où est la bibliothèque ({}) est illisible : {e}",
                    pointer.display()
                ),
            };
        }
    };
    let v: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            return Place::Missing {
                dir: None,
                why: format!(
                    "le fichier qui dit où est la bibliothèque ({}) est abîmé : {e}",
                    pointer.display()
                ),
            };
        }
    };
    if v.get("version").and_then(Value::as_u64).unwrap_or(0) > VERSION {
        return Place::Missing {
            dir: None,
            why: format!(
                "le fichier qui dit où est la bibliothèque ({}) vient d'une version plus récente d'OpenTuneBox",
                pointer.display()
            ),
        };
    }
    let Some(dir) = v.get("dossier").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
        return Place::Missing {
            dir: None,
            why: format!("le fichier {} ne donne aucun dossier", pointer.display()),
        };
    };
    let dir = PathBuf::from(dir);
    if dir == default_dir {
        return Place::Ready { dir, chosen: false };
    }
    if !dir.is_dir() {
        return Place::Missing {
            why: format!(
                "le dossier de la bibliothèque est introuvable : {} (disque débranché ?)",
                dir.display()
            ),
            dir: Some(dir),
        };
    }
    if !is_library(&dir) {
        return Place::Missing {
            why: format!("le dossier {} ne contient pas de bibliothèque", dir.display()),
            dir: Some(dir),
        };
    }
    Place::Ready { dir, chosen: true }
}

/// Fait de `dir` l'emplacement de la bibliothèque. Le dossier par défaut **retire** le pointeur : on revient à
/// la situation de qui n'a jamais rien déplacé. Écriture par fichier temporaire puis renommage (D-039).
pub fn set_pointer(default_dir: &Path, dir: &Path) -> io::Result<()> {
    let pointer = pointer_path(default_dir);
    if dir == default_dir {
        return match std::fs::remove_file(&pointer) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let mut m = Map::new();
    m.insert("version".into(), VERSION.into());
    m.insert("dossier".into(), dir.to_string_lossy().into_owned().into());
    if let Some(parent) = pointer.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = pointer.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_string(&Value::Object(m)).expect("écriture en mémoire"),
    )?;
    std::fs::rename(&tmp, &pointer)
}

// ---------------------------------------------------------------- choix d'un nouveau dossier

/// Ce que devient le dossier choisi par l'utilisateur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Il contient déjà une bibliothèque (autre PC, disque externe) : on propose de **l'utiliser telle quelle**,
    /// jamais de la fusionner ni de l'écraser.
    Existing(PathBuf),
    /// La bibliothèque actuelle y sera copiée : le dossier choisi s'il est vide, sinon son sous-dossier
    /// `OpenTuneBox` (sans rien toucher à ce qu'il contient déjà).
    New(PathBuf),
}

impl Target {
    pub fn dir(&self) -> &Path {
        match self {
            Target::Existing(d) | Target::New(d) => d,
        }
    }
}

/// Le chemin sans `..` ni lien, pour comparer deux dossiers ; un dossier qui n'existe pas encore prend celui de
/// son parent.
fn canon(p: &Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(p) {
        return c;
    }
    match (p.parent(), p.file_name()) {
        (Some(parent), Some(name)) => canon(parent).join(name),
        _ => p.to_path_buf(),
    }
}

/// Le dossier est vide, ou ne contient qu'une copie interrompue de notre fait (sa marque le dit).
fn free(dir: &Path) -> bool {
    !dir.exists() || dir.join(MOVING).exists() || std::fs::read_dir(dir).is_ok_and(|mut it| it.next().is_none())
}

/// Ce que devient `chosen` pour une bibliothèque qui vit dans `current`. `Err` : un message à montrer.
pub fn target_of(current: &Path, chosen: &Path) -> Result<Target, String> {
    if !chosen.is_dir() {
        return Err(format!("{} n'est pas un dossier.", chosen.display()));
    }
    let here = canon(current);
    for candidate in [chosen.to_path_buf(), chosen.join(SUBFOLDER), chosen.join(FOLDER)] {
        if is_library(&candidate) {
            if canon(&candidate) == here {
                return Err("C'est déjà l'emplacement de la bibliothèque.".into());
            }
            return Ok(Target::Existing(candidate));
        }
    }
    let target = if free(chosen) {
        chosen.to_path_buf()
    } else {
        let sub = chosen.join(SUBFOLDER);
        if !free(&sub) {
            return Err(format!(
                "{} existe déjà et ne contient pas de bibliothèque : choisissez un autre dossier.",
                sub.display()
            ));
        }
        sub
    };
    let there = canon(&target);
    if there == here || there.starts_with(&here) {
        return Err("Ce dossier est à l'intérieur de la bibliothèque actuelle.".into());
    }
    if here.starts_with(&there) {
        return Err("Ce dossier contient la bibliothèque actuelle : choisissez-en un autre.".into());
    }
    Ok(Target::New(target))
}

// ---------------------------------------------------------------- copie relue

/// Les fichiers de `root` (chemins relatifs), sans les fichiers temporaires d'une écriture interrompue.
fn files_of(root: &Path) -> io::Result<Vec<(PathBuf, u64)>> {
    let mut out = Vec::new();
    let mut todo = vec![PathBuf::new()];
    while let Some(rel) = todo.pop() {
        for entry in std::fs::read_dir(root.join(&rel))? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let name = rel.join(entry.file_name());
            if kind.is_dir() {
                todo.push(name);
            } else if kind.is_file() {
                let skip = name.extension().is_some_and(|e| e == "tmp") || name == Path::new(MOVING);
                if !skip {
                    out.push((name, entry.metadata()?.len()));
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Le nombre de fichiers d'une bibliothèque et leur taille totale, à annoncer avant de la déplacer.
pub fn library_size(dir: &Path) -> io::Result<(usize, u64)> {
    let files = files_of(dir)?;
    Ok((files.len(), files.iter().map(|(_, n)| n).sum()))
}

/// Les deux fichiers ont le même contenu, lus par blocs (un log copié peut peser des centaines de Mo).
fn same_content(a: &Path, b: &Path) -> io::Result<bool> {
    let (mut fa, mut fb) = (std::fs::File::open(a)?, std::fs::File::open(b)?);
    if fa.metadata()?.len() != fb.metadata()?.len() {
        return Ok(false);
    }
    let (mut ba, mut bb) = (vec![0u8; 1 << 20], vec![0u8; 1 << 20]);
    loop {
        let n = fa.read(&mut ba)?;
        if n == 0 {
            return Ok(fb.read(&mut bb[..1])? == 0);
        }
        fb.read_exact(&mut bb[..n])?;
        if ba[..n] != bb[..n] {
            return Ok(false);
        }
    }
}

/// Ce qu'une copie a fait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copied {
    pub files: usize,
    pub bytes: u64,
}

/// Copie toute la bibliothèque `from` dans `to`, puis **relit chaque fichier copié** et le compare à l'original.
/// `progress(fait, total)` reçoit des octets (copie puis relecture : le total compte chaque fichier deux fois).
///
/// `to` porte la marque `MOVING` du début à la fin : c'est à l'appelant de la retirer (`finish_move`) une fois la
/// bibliothèque écrite et basculée. En cas d'échec, ce qui a été copié est retiré et l'ancienne bibliothèque reste
/// la seule en service.
pub fn copy_library(from: &Path, to: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<Copied, String> {
    let files = files_of(from).map_err(|e| format!("lecture de {} impossible : {e}", from.display()))?;
    let total: u64 = files.iter().map(|(_, n)| n).sum::<u64>() * 2;
    let created = !to.exists();
    std::fs::create_dir_all(to).map_err(|e| format!("{} impossible à créer : {e}", to.display()))?;
    let mark = to.join(MOVING);
    std::fs::write(
        &mark,
        format!(
            "Copie de la bibliothèque d'OpenTuneBox en cours depuis {}.\nTant que ce fichier est là, ce dossier n'est pas une bibliothèque.\n",
            from.display()
        ),
    )
    .map_err(|e| format!("écriture impossible dans {} : {e}", to.display()))?;
    let mut written: Vec<PathBuf> = Vec::new();
    let result = (|| {
        let mut done = 0;
        for (rel, len) in &files {
            let dest = to.join(rel);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            written.push(dest.clone());
            std::fs::copy(from.join(rel), &dest)?;
            done += len;
            progress(done, total);
        }
        for (rel, len) in &files {
            if !same_content(&from.join(rel), &to.join(rel))? {
                return Err(io::Error::other(format!(
                    "{} : la copie relue diffère de l'original",
                    rel.display()
                )));
            }
            done += len;
            progress(done, total);
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(Copied {
            files: files.len(),
            bytes: total / 2,
        }),
        Err(e) => {
            remove_partial(to, created, &written);
            Err(format!(
                "copie impossible ({e}) : la bibliothèque reste à son emplacement actuel"
            ))
        }
    }
}

/// Retire une copie interrompue : les fichiers écrits, la marque, puis les dossiers **devenus vides** (rien de ce
/// qui était déjà là n'est touché ; `to` lui-même seulement si la copie l'a créé).
fn remove_partial(to: &Path, created: bool, written: &[PathBuf]) {
    for f in written {
        let _ = std::fs::remove_file(f);
    }
    let _ = std::fs::remove_file(to.join(MOVING));
    let mut dirs: Vec<&Path> = written.iter().filter_map(|f| f.parent()).collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    dirs.dedup();
    for d in dirs {
        let mut d = d;
        while d.starts_with(to) && d != to {
            if std::fs::remove_dir(d).is_err() {
                break;
            }
            match d.parent() {
                Some(p) => d = p,
                None => break,
            }
        }
    }
    if created {
        let _ = std::fs::remove_dir(to);
    }
}

/// La copie est en service : sa marque est retirée, le dossier devient une bibliothèque.
pub fn finish_move(to: &Path) -> io::Result<()> {
    match std::fs::remove_file(to.join(MOVING)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Efface l'ancien dossier, **après** la bascule. Refuse un dossier qui n'est pas une bibliothèque : une erreur
/// d'appel ne doit jamais effacer autre chose.
pub fn remove_old(old: &Path) -> Result<(), String> {
    if !is_library(old) {
        return Err(format!(
            "{} ne ressemble pas à une bibliothèque : laissé en place",
            old.display()
        ));
    }
    std::fs::remove_dir_all(old).map_err(|e| format!("{} pas entièrement effacé : {e}", old.display()))
}

// ---------------------------------------------------------------- témoin des deux postes

/// Le nom de ce poste, tel que le système le donne ; « poste inconnu » sinon.
pub fn host_name() -> String {
    let from_env = |k: &str| {
        std::env::var(k)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    from_env("COMPUTERNAME")
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .or_else(|| from_env("HOSTNAME"))
        .unwrap_or_else(|| "poste inconnu".into())
}

/// Secondes écoulées depuis 1970, l'horloge du poste.
pub fn now_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Le témoin lu dans une bibliothèque : le poste, la session (un lancement) et l'instant de la dernière écriture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Witness {
    pub host: String,
    pub session: String,
    pub seen_s: u64,
}

impl Witness {
    pub fn read(dir: &Path) -> Option<Witness> {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(WITNESS)).ok()?).ok()?;
        Some(Witness {
            host: v.get("poste")?.as_str()?.to_string(),
            session: v.get("session")?.as_str()?.to_string(),
            seen_s: v.get("vu")?.as_u64()?,
        })
    }

    pub fn write(&self, dir: &Path) -> io::Result<()> {
        let mut m = Map::new();
        m.insert("version".into(), VERSION.into());
        m.insert("poste".into(), self.host.clone().into());
        m.insert("session".into(), self.session.clone().into());
        m.insert("vu".into(), self.seen_s.into());
        let path = dir.join(WITNESS);
        let tmp = path.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_string(&Value::Object(m)).expect("écriture en mémoire"),
        )?;
        std::fs::rename(&tmp, &path)
    }

    /// L'avertissement à montrer quand une **autre** session a écrit la bibliothèque il y a moins de
    /// `WITNESS_FRESH_S` secondes ; `None` sinon (témoin ancien, ou le nôtre).
    pub fn warning(&self, mine: &Witness) -> Option<String> {
        if self.session == mine.session || mine.seen_s.saturating_sub(self.seen_s) > WITNESS_FRESH_S {
            return None;
        }
        let min = mine.seen_s.saturating_sub(self.seen_s).div_ceil(60).max(1);
        let ou = if self.host == mine.host {
            "dans une autre fenêtre d'OpenTuneBox de ce poste".to_string()
        } else {
            format!("sur le poste « {} »", self.host)
        };
        Some(format!(
            "Cette bibliothèque est peut-être ouverte {ou} (dernière écriture il y a {min} min). Deux OpenTuneBox qui \
             l'écrivent en même temps perdent les changements de l'un des deux : fermez l'autre avant de continuer ici."
        ))
    }
}
