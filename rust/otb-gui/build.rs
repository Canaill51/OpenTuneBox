// Icône de l'exécutable Windows (fichier .exe, Explorateur, barre des tâches) : la même que la version Python.
// `CARGO_CFG_TARGET_OS` et non `cfg!` : ce script tourne sur la machine hôte, pas sur la cible.
fn main() {
    println!("cargo:rerun-if-changed=../../packaging/opentunebox.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/opentunebox.ico");
        res.compile().expect("ajout de l'icône à l'exécutable Windows");
    }
}
