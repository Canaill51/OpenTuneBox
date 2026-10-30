//! Sur les vraies références (`OTB_GOLDEN`, ex. `$OTB_LOGS/golden`) : tout fichier se lit et les fonctions portées
//! rendent exactement ce que Python a rendu. Ignoré (avec message) quand le dossier manque : la CI n'a pas le corpus.

use otb_parity::{compare, registry, value};

#[test]
fn les_references_se_lisent_et_les_fonctions_portees_concordent() {
    let Ok(dir) = std::env::var("OTB_GOLDEN") else {
        eprintln!("OTB_GOLDEN absent : test ignoré");
        return;
    };
    let (mut lus, mut rejoues) = (0, 0);
    for log in std::fs::read_dir(std::path::Path::new(&dir).join("modules")).unwrap() {
        for f in std::fs::read_dir(log.unwrap().path()).unwrap() {
            let path = f.unwrap().path();
            let case = value::load_case(&path).unwrap_or_else(|e| panic!("{e}"));
            lus += 1;
            if let Some(port) = registry::lookup(&case.function) {
                rejoues += 1;
                let got = port(&case).unwrap();
                compare::equal(case.result.as_ref().unwrap(), &got, compare::Tol { rtol: 0.0 })
                    .unwrap_or_else(|e| panic!("{} : {e}", path.display()));
            }
        }
    }
    assert!(lus > 100 && rejoues > 0, "{lus} lus, {rejoues} rejoués");
}
