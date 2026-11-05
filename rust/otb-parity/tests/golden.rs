//! Sur les vraies références (`OTB_GOLDEN`, ex. `$OTB_LOGS/golden`) : tout fichier se lit et chaque fonction portée rend ce
//! que Python a rendu, au bit près ou dans la tolérance mesurée de `registry::tolerance` (celle de `otb-parity check`).
//! Ignoré (avec message) quand le dossier manque : la CI n'a pas le corpus.

use otb_parity::{compare, registry, value};

#[test]
fn les_references_se_lisent_et_les_fonctions_portees_concordent() {
    let Ok(dir) = std::env::var("OTB_GOLDEN") else {
        println!("[OTB-SAUT] OTB_GOLDEN absent : concordance avec les références ignorée");
        return;
    };
    let (mut lus, mut rejoues) = (0, 0);
    let mut ecarts = Vec::new();
    for log in std::fs::read_dir(std::path::Path::new(&dir).join("modules")).unwrap() {
        for f in std::fs::read_dir(log.unwrap().path()).unwrap() {
            let path = f.unwrap().path();
            let case = value::load_case(&path).unwrap_or_else(|e| panic!("{e}"));
            lus += 1;
            if let Some(port) = registry::lookup(&case.function) {
                rejoues += 1;
                let got = port(&case).unwrap();
                // ⚠️ jamais `rtol: 0.0` pour toutes : FFT, sin/cos, exp et SVD s'écartent de numpy au dernier bit (mesuré,
                // registry::tolerance) ; une fonction sans tolérance déclarée reste comparée au bit près (L-083)
                let tol = compare::Tol {
                    rtol: registry::tolerance(&case.function, &case),
                };
                if let Err(e) = compare::equal(case.result.as_ref().unwrap(), &got, tol) {
                    ecarts.push(format!("{} : {e}", path.display()));
                }
            }
        }
    }
    assert!(lus > 100 && rejoues > 0, "{lus} lus, {rejoues} rejoués");
    assert!(
        ecarts.is_empty(),
        "{} écart(s) sur {rejoues} appels rejoués :\n{}",
        ecarts.len(),
        ecarts.join("\n")
    );
}
