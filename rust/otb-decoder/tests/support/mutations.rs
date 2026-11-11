//! Fichiers abîmés fabriqués en mémoire à partir de `tests/data/jeno5_5s.bbl`, pour les essais de robustesse (D-044) :
//! partagé par `otb-decoder/tests/robustness.rs` (décodeur seul) et `otb-cli/tests/robustesse.rs` (décodage, analyses et
//! assistant), qui l'incluent par `#[path]`. Chaque mutation se refait à l'identique d'après son numéro.
/// Générateur pseudo-aléatoire xorshift64 : graine fixe, donc chaque cas se refait à l'identique d'après son numéro.
struct Alea(u64);

impl Alea {
    fn new(graine: u64) -> Alea {
        Alea(graine.max(1)) // xorshift reste bloqué à 0
    }
    fn suivant(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Entier de 0 à n − 1 (0 si n = 0).
    fn jusqua(&mut self, n: usize) -> usize {
        (self.suivant() % n.max(1) as u64) as usize
    }
}

/// Longueur de l'en-tête de la première session (lignes « H … » du début du fichier).
fn longueur_en_tete(d: &[u8]) -> usize {
    let mut i = 0;
    while d[i..].starts_with(b"H ") {
        match d[i..].iter().position(|&b| b == b'\n') {
            Some(n) => i += n + 1,
            None => return d.len(),
        }
    }
    i
}

/// Mutation numéro `m` du fichier source, d'une des six sortes ; rend son nom (pour la refaire) et le fichier abîmé.
pub fn muter(o: &[u8], m: u64) -> (String, Vec<u8>) {
    let mut a = Alea::new(0x9E37_79B9_7F4A_7C15 ^ m.wrapping_mul(0x2545_F491_4F6C_DD1D));
    let mut d = o.to_vec();
    let n = o.len();
    let sorte = match m % 6 {
        0 => {
            for _ in 0..1 + a.jusqua(8) {
                let i = a.jusqua(n);
                d[i] = a.suivant() as u8;
            }
            "quelques octets"
        }
        1 => {
            let h = longueur_en_tete(o);
            for _ in 0..1 + a.jusqua(4) {
                let i = a.jusqua(h);
                d[i] = a.suivant() as u8;
            }
            "en-tête"
        }
        2 => {
            d.truncate(a.jusqua(n));
            "coupé"
        }
        3 => {
            let i = a.jusqua(n);
            let fin = (i + 1 + a.jusqua(4096)).min(n);
            d.drain(i..fin);
            "trou"
        }
        4 => {
            let i = a.jusqua(n);
            let fin = (i + 1 + a.jusqua(4096)).min(n);
            let j = a.jusqua(n);
            d.splice(j..j, o[i..fin].iter().copied());
            "morceau doublé"
        }
        _ => {
            for _ in 0..n / 100 {
                let i = a.jusqua(n);
                d[i] = a.suivant() as u8;
            }
            "1 %"
        }
    };
    (format!("mutation {m} ({sorte})"), d)
}

/// Nombre de mutations à essayer : `OTB_MUTATIONS` s'il est donné (essai prolongé), sinon `defaut` (CI). Jamais zéro par
/// défaut : sans la variable, le test fait ses cas, il ne se saute pas.
pub fn nombre_de_mutations(defaut: u64) -> u64 {
    std::env::var("OTB_MUTATIONS")
        .map(|v| v.parse().expect("OTB_MUTATIONS : nombre entier de mutations"))
        .unwrap_or(defaut)
}
