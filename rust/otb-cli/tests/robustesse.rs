//! Un fichier abîmé ou piégé passe par tout le programme sans l'emporter : décodage, conversion en unités physiques, toutes
//! les analyses et l'assistant (`report`), c'est-à-dire ce que subit l'interface quand un pilote ouvre le fichier (D-044).
//! Les mutations sont celles de `otb-decoder/tests/support/mutations.rs`. 6 cas en CI (un de chaque sorte) ;
//! `OTB_MUTATIONS=N` en lance N : essai prolongé, en `--release` et seul (docs/validation.md, « Robustesse »).
//!
//! ⚠️ Une allocation ratée n'est pas une panique : `catch_unwind` ne la rattrape pas, le processus est abandonné (L-080).

use std::path::PathBuf;
use std::time::{Duration, Instant};

#[path = "../../otb-decoder/tests/support/mutations.rs"]
mod mutations;
use mutations::{muter, nombre_de_mutations};

const LIMITE: Duration = Duration::from_secs(60);

#[test]
fn fichiers_abimes_traversent_analyses_et_assistant() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/jeno5_5s.bbl");
    let o = std::fs::read(source).expect("tests/data/jeno5_5s.bbl");
    let total = nombre_de_mutations(6);
    let dir = std::env::temp_dir().join(format!("otb-cli-robustesse-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let chemin = dir.join("abime.bbl");
    let fichiers = [chemin.to_string_lossy().to_string()];
    for m in 0..total {
        let (nom, d) = muter(&o, m);
        std::fs::write(&chemin, &d).unwrap();
        let t = Instant::now();
        let r = std::panic::catch_unwind(|| otb_cli::report(&fichiers, None));
        let dt = t.elapsed();
        assert!(r.is_ok(), "{nom} : panique");
        assert!(dt < LIMITE, "{nom} : {dt:?} (plus de {LIMITE:?})");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
