//! Binaire `opentunebox` : `opentunebox info LOG…`, `opentunebox report LOG… [--profil P]`.

fn main() {
    // `args_os` : un chemin qui n'est pas de l'UTF-8 ne doit pas faire paniquer le programme.
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().to_string())
        .collect();
    std::process::exit(otb_cli::run_stdio(&args));
}
