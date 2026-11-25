//! `otb-dev numero` : le prochain numéro d'un registre, vu de tous les worktrees (compétence `chantiers`, L-125).
//!
//! ⚠️ Les numéros des cas fabriqués sont construits à l'exécution (`n`) : écrits en toutes lettres, le contrôle
//! `register_ids_cited_exist` les prendrait pour des renvois vers des entrées qui n'existent pas.

use std::path::{Path, PathBuf};
use std::process::Command;

use otb_dev::numero::{RESERVATIONS, civil_date, highest, list, next, reservation_line, reserve};

/// `n("D", 7)` → « D-007 ».
fn n(prefix: &str, k: u32) -> String {
    format!("{prefix}-{k:03}")
}

#[test]
fn le_plus_grand_numero_cite_est_trouve_partout_dans_le_texte() {
    let t = format!(
        "### {} x\n| {} |\nvoir {}, {}",
        n("D", 12),
        n("D", 3),
        n("D", 40),
        n("L", 99)
    );
    assert_eq!(highest("D", &t), 40);
    assert_eq!(highest("L", &t), 99);
    assert_eq!(highest("W", "rien ici"), 0);
    // collé à d'autres lettres, ou avec quatre chiffres ou deux : pas un numéro
    let faux = format!("X{} {}0 D-07", n("D", 500), n("D", 500));
    assert_eq!(highest("D", &faux), 0);
}

#[test]
fn le_prochain_numero_depasse_toutes_les_sources() {
    let (a, b, c) = (
        format!("### {} a", n("D", 76)),
        format!("+// ({})", n("D", 77)),
        format!("{}\t2026\t/x\tn\n", n("D", 70)),
    );
    assert_eq!(next("D", [a.as_str(), b.as_str(), c.as_str()]), 78);
    assert_eq!(next("W", Vec::<&str>::new()), 1);
}

#[test]
fn une_note_ne_casse_pas_le_fichier_des_reservations() {
    assert_eq!(
        reservation_line("X-1", "2026-11-25", "/a", "fiche\tdrone\nKV"),
        "X-1\t2026-11-25\t/a\tfiche drone KV\n"
    );
}

#[test]
fn la_date_du_jour_suit_le_calendrier() {
    assert_eq!(civil_date(0), "1970-01-01");
    assert_eq!(civil_date(11_016), "2000-02-29");
    assert_eq!(civil_date(20_782), "2026-11-25");
}

fn git(dir: &Path, args: &[&str]) {
    let st = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=essai",
            "-c",
            "user.email=essai@exemple.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .expect("git introuvable");
    assert!(
        st.status.success(),
        "git {args:?} : {}",
        String::from_utf8_lossy(&st.stderr)
    );
}

fn dossier_temp(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("otb-numero-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn registre(nums: &[u32]) -> String {
    nums.iter().map(|&k| format!("### {} x\n", n("D", k))).collect()
}

/// Dépôt fabriqué : `main` avec les décisions 1 à 3 ; un worktree « code » cite la 7 dans son code sans l'écrire au
/// registre, un autre ajoute la 5 à son registre sans l'avoir validée.
#[test]
fn deux_chantiers_ne_recoivent_jamais_le_meme_numero() {
    let base = dossier_temp("depot");
    let main = base.join("principal");
    std::fs::create_dir_all(main.join("docs")).unwrap();
    git(&main, &["init", "-q", "-b", "main"]);
    std::fs::write(main.join("docs/decisions.md"), registre(&[1, 2, 3])).unwrap();
    git(&main, &["add", "."]);
    git(&main, &["commit", "-q", "-m", "départ"]);
    // origin/main : le dépôt se sert de lui-même comme remote, comme un clone de GitHub
    git(&main, &["remote", "add", "origin", main.to_str().unwrap()]);
    git(&main, &["fetch", "-q", "origin"]);

    let code = base.join("code");
    git(
        &main,
        &["worktree", "add", "-q", "-b", "feat/code", code.to_str().unwrap()],
    );
    std::fs::write(code.join("code.rs"), format!("// le choix fait ({})\n", n("D", 7))).unwrap();
    // `git diff origin/main` ne voit pas un fichier jamais ajouté : le chantier « code » l'a ajouté à l'index
    git(&code, &["add", "code.rs"]);
    let reg = base.join("registre");
    git(
        &main,
        &["worktree", "add", "-q", "-b", "docs/reg", reg.to_str().unwrap()],
    );
    std::fs::write(reg.join("docs/decisions.md"), registre(&[1, 2, 3, 5])).unwrap();

    let premier = reserve(&reg, "D", "registre", "2026-11-25").unwrap();
    assert!(premier.starts_with(&format!("{} ", n("D", 8))), "{premier}");
    let second = reserve(&main, "D", "pilote", "2026-11-25").unwrap();
    assert!(second.starts_with(&format!("{} ", n("D", 9))), "{second}");
    // les autres registres ont leur propre suite
    assert!(
        reserve(&code, "L", "", "2026-11-25")
            .unwrap()
            .starts_with(&format!("{} ", n("L", 1)))
    );
    assert!(reserve(&code, "X", "", "2026-11-25").is_err());

    // une seule liste, commune à tous les worktrees, dans le dossier .git du principal
    let liste = list(&code).unwrap();
    assert_eq!(liste.lines().count(), 3, "{liste}");
    assert!(
        liste.contains(&format!("{}\t2026-11-25\t", n("D", 8))) && liste.contains("\tregistre"),
        "{liste}"
    );
    assert!(main.join(".git").join(RESERVATIONS).exists());
    assert!(
        !main.join(".git").join(format!("{RESERVATIONS}.verrou")).exists(),
        "verrou rendu"
    );
    let _ = std::fs::remove_dir_all(&base);
}
