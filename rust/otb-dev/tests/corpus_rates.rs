//! `otb-dev corpus rates` : tableau et comparaison, sans vrais logs (portage de `tests/test_scripts.py`).

use otb_dev::corpus::rates::{compare_rows, summary_of};
use serde_json::{Value, json};

fn rows() -> Vec<Value> {
    vec![
        json!({"file": "a.bbl", "sess": 1, "profile": "five", "moving": true, "findings": [
            ["gyro_noisy", "Gyro filtré bruité en roll", "attention", true],
            ["hover_line_residual", "Raie à 330 Hz", "conseil", true],
            ["hover_line_residual", "Raie à 410 Hz", "conseil", true],
            ["hover_noise_summary", "Vol stationnaire : bruit identifié", "info", false]]}),
        json!({"file": "b.bbl", "sess": 1, "profile": "whoop", "moving": false, "findings": [
            ["hover_line_residual", "Raie à 250 Hz", "conseil", true], ["step_ok", "Dépassement en roll", "ok", false]]}),
    ]
}

#[test]
fn le_tableau_regroupe_par_cle_et_ignore_info_et_ok() {
    let out = summary_of(&rows());
    assert!(
        out.starts_with("2 sessions : tous 2, mouvements 1, stationnaires 1"),
        "{out}"
    );
    let lines: Vec<&str> = out.lines().skip(2).collect();
    let find = |t: &str| {
        lines
            .iter()
            .find(|l| l.starts_with(t))
            .copied()
            .unwrap_or_else(|| panic!("{t} absent de {out}"))
    };
    // regroupée par clé, comptée une fois par session, avec le dernier titre rencontré
    assert!(find("hover_line_residual (Raie à 250 Hz)").contains("100%"));
    assert!(find("gyro_noisy (Gyro filtré bruité en roll)").contains("0%"));
    assert!(
        !lines
            .iter()
            .any(|l| l.starts_with("hover_noise_summary") || l.starts_with("step_ok"))
    );
}

#[test]
fn le_tableau_a_la_mise_en_forme_de_python() {
    let out = summary_of(&rows());
    let l: Vec<&str> = out.lines().collect();
    assert_eq!(
        l[1],
        format!(
            "{:<70} {:>13} {:>13} {:>13}",
            "constat", "tous", "mouvements", "stationnaires"
        )
    );
    // le plus fréquent d'abord ; pourcentage sur 12 colonnes
    assert_eq!(
        l[2],
        format!(
            "{:<70} {:>12}% {:>12}% {:>12}%",
            "hover_line_residual (Raie à 250 Hz)", 100, 100, 100
        )
    );
    assert_eq!(
        l[3],
        format!(
            "{:<70} {:>12}% {:>12}% {:>12}%",
            "gyro_noisy (Gyro filtré bruité en roll)", 50, 100, 0
        )
    );
    assert_eq!(l.len(), 4);
}

#[test]
fn le_libelle_est_coupe_a_70_caracteres_et_l_arrondi_est_celui_de_python() {
    // 1 session sur 8 : 12,5 % → « 12 » (arrondi au pair de Python `.0f`) ; 3 sur 8 : 37,5 → « 38 »
    let long = "é".repeat(100);
    let mut v = Vec::new();
    for i in 0..8 {
        let mut f = Vec::new();
        if i == 0 {
            f.push(json!(["rare", long, "conseil", true]));
        }
        if i < 3 {
            f.push(json!(["moins_rare", "t", "conseil", true]));
        }
        v.push(json!({"file": format!("{i}.bbl"), "sess": 1, "profile": "five", "moving": true, "findings": f}));
    }
    let out = summary_of(&v);
    let l: Vec<&str> = out.lines().collect();
    assert!(l[2].starts_with("moins_rare (t)") && l[2].contains(" 38%"), "{}", l[2]);
    assert!(l[3].starts_with("rare (éé") && l[3].contains(" 12%"), "{}", l[3]);
    assert_eq!(l[3].chars().take(70).count(), 70);
    assert_eq!(l[3].chars().nth(70), Some(' '));
}

fn base() -> Value {
    json!({"file": "a.bbl", "sess": 1, "profile": "five", "moving": true, "t_analyze": 2.0,
        "findings": [["rpm_q_raise", "Filtre RPM : Q peut monter", "conseil", true]],
        "details": [["rpm_q_raise", null, "conseil", "Filtre RPM : Q peut monter", "1e -0 dB", ["rpm_filter_q 600"]]]})
}

#[test]
fn la_comparaison_signale_un_chiffre_qui_change_et_mesure_le_temps() {
    let same = {
        let mut s = base();
        s["file"] = json!("b.bbl");
        s["t_analyze"] = json!(1.0);
        s
    };
    let mut changed = base();
    changed["t_analyze"] = json!(1.0);
    changed["details"][0][4] = json!("1e -1 dB");
    let (text, n) = compare_rows(&[base(), same.clone()], &[changed, same.clone()], 10);
    assert_eq!(n, 1, "{text}");
    assert!(
        text.contains("sessions aux constats différents : 1") && text.contains("== a.bbl (session 1)"),
        "{text}"
    );
    assert!(text.contains("1e -0 dB") && text.contains("1e -1 dB"), "{text}");
    assert!(
        text.contains("durée cumulée de l'assistant : 3 s → 2 s ; session la plus lente : 2.0 s → 1.0 s"),
        "{text}"
    );
    let (text, n) = compare_rows(&[base(), same.clone()], &[base(), same], 10);
    assert_eq!(n, 0, "{text}");
    assert!(text.contains("sessions : 2 avant, 2 après ; présentes d'un seul côté : 0"));
}

#[test]
fn une_session_presente_d_un_seul_cote_ou_un_autre_profil_compte() {
    let mut autre_profil = base();
    autre_profil["profile"] = json!("whoop");
    let (_, n) = compare_rows(&[base()], &[autre_profil], 10);
    assert_eq!(n, 1, "même constats, autre taille détectée");
    let mut seule = base();
    seule["sess"] = json!(2);
    let (text, n) = compare_rows(&[base()], &[base(), seule], 10);
    assert_eq!(n, 1);
    assert!(text.contains("présentes d'un seul côté : 1"), "{text}");
}

#[test]
fn un_passage_sans_texte_complet_compare_les_cles_et_titres() {
    let mut vieux = rows();
    let (text, n) = compare_rows(&vieux, &vieux, 10);
    assert_eq!(n, 0);
    assert!(text.contains("passage sans texte complet"), "{text}");
    vieux[0]["findings"][0][1] = json!("autre titre");
    assert_eq!(compare_rows(&rows(), &vieux, 10).1, 1);
}
