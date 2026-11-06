//! Décimation min/max d'une courbe et index multi-résolution qui la rend indépendante du nombre d'échantillons
//! (D-048, W-049).
//!
//! Tracer une courbe de plusieurs centaines de milliers d'échantillons sur quelques milliers de pixels se fait par
//! décimation min/max : la fenêtre visible est coupée en paquets d'échantillons, et chaque paquet ne donne que son
//! point le plus bas et son point le plus haut (l'allure, pics compris, est conservée). Le parcours des paquets coûte
//! O(nombre d'échantillons visibles) : [`window_points_scan`]. La [`MinMaxPyramid`] donne le même résultat en
//! O(nombre de points rendus) : [`window_points`].
//!
//! Les deux fonctions rendent **exactement** la même chose, point pour point : c'est le contrat, vérifié par les tests
//! (`rust/otb-dsp/tests/minmax.rs`).
//!
//! ## Trous (`NaN`)
//!
//! Un paquet qui contient au moins un `NaN` est rendu comme **un seul point `NaN`** : la ligne est coupée, le trou du
//! log reste visible. Cette règle est *associative* (combiner des paquets déjà combinés donne le même résultat que
//! parcourir les échantillons un par un), ce qui est la condition pour qu'un index multi-résolution soit exact.
//!
//! ⚠️ Ne jamais revenir à une comparaison nue (`y[i] < y[lo]`) pour choisir le minimum : un `NaN` rencontré en tête
//! de paquet y gagne toutes les comparaisons suivantes, le min et le max réels du paquet disparaissent, et le résultat
//! dépend de la position du trou dans le découpage — donc d'une pyramide à l'autre (L-085, D-048).

/// Nombre de blocs du niveau inférieur résumés par un bloc (facteur entre deux niveaux de la pyramide).
///
/// Compromis : la mémoire vaut `24 / (FACTOR - 1)` octets par échantillon (43 % d'une courbe `f64` pour 8), et
/// le coût d'une requête croît avec `FACTOR` (les bords d'un paquet, qui ne tombent pas sur un bloc, sont parcourus
/// échantillon par échantillon : au plus `2 · (FACTOR - 1)`).
pub const FACTOR: usize = 8;

/// Résultat de la combinaison d'un intervalle : les indices du point le plus bas et du plus haut, ou un trou.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extremes {
    /// Aucun `NaN` : indice du minimum et indice du maximum (le premier atteint en cas d'égalité).
    Clean { lo: u32, hi: u32 },
    /// Au moins un `NaN` dans l'intervalle : indice du premier rencontré.
    Hole { at: u32 },
}

impl Extremes {
    /// Points à tracer pour cet intervalle, dans l'ordre du temps : deux points (le plus bas et le plus haut, dans
    /// l'ordre où ils arrivent), un seul s'ils sont confondus, un `NaN` si l'intervalle contient un trou.
    fn push_points(self, t: &[f64], y: &[f64], out: &mut Vec<[f64; 2]>) {
        match self {
            Self::Hole { at } => out.push([t[at as usize], f64::NAN]),
            Self::Clean { lo, hi } => {
                let (a, b) = if lo <= hi {
                    (lo as usize, hi as usize)
                } else {
                    (hi as usize, lo as usize)
                };
                out.push([t[a], y[a]]);
                if b != a {
                    out.push([t[b], y[b]]);
                }
            }
        }
    }
}

/// Un bloc résumé : indices **et valeurs** du minimum et du maximum, ou un trou (`lo_i == HOLE`, `hi_i` portant
/// l'indice du premier `NaN`).
///
/// ⚠️ Les valeurs sont rangées à côté des indices **exprès** : sans elles, chaque comparaison de deux blocs relit la
/// courbe (`y[lo]`) à deux adresses éloignées, et le temps d'un cadre redevient proportionnel au nombre
/// d'échantillons visibles par défaut de cache — mesuré : 5,7 ms contre 1,9 ms sur la même courbe, à nombre de points
/// rendus égal (L-086, D-048).
#[derive(Clone, Copy, Debug)]
struct Block {
    lo_i: u32,
    hi_i: u32,
    lo_v: f64,
    hi_v: f64,
}

impl Block {
    const HOLE: u32 = u32::MAX;

    /// Un seul échantillon (seule lecture de la courbe, et elle est contiguë aux voisines).
    fn single(i: usize, y: &[f64]) -> Self {
        let v = y[i];
        if v.is_nan() {
            Self {
                lo_i: Self::HOLE,
                hi_i: i as u32,
                lo_v: v,
                hi_v: v,
            }
        } else {
            Self {
                lo_i: i as u32,
                hi_i: i as u32,
                lo_v: v,
                hi_v: v,
            }
        }
    }

    fn is_hole(self) -> bool {
        self.lo_i == Self::HOLE
    }

    /// Combine deux blocs **voisins et dans l'ordre** (`a` avant `b`), sans toucher à la courbe.
    fn join(a: Self, b: Self) -> Self {
        if a.is_hole() {
            return a;
        }
        if b.is_hole() {
            return b;
        }
        Self {
            // `<` et `>` strictes : à égalité le point le plus à gauche est gardé, comme un parcours simple
            lo_i: if b.lo_v < a.lo_v { b.lo_i } else { a.lo_i },
            hi_i: if b.hi_v > a.hi_v { b.hi_i } else { a.hi_i },
            lo_v: if b.lo_v < a.lo_v { b.lo_v } else { a.lo_v },
            hi_v: if b.hi_v > a.hi_v { b.hi_v } else { a.hi_v },
        }
    }

    fn extremes(self) -> Extremes {
        if self.is_hole() {
            Extremes::Hole { at: self.hi_i }
        } else {
            Extremes::Clean {
                lo: self.lo_i,
                hi: self.hi_i,
            }
        }
    }
}

/// Index multi-résolution d'une courbe : minimum et maximum de chaque bloc de `FACTOR` échantillons, puis de chaque
/// bloc de `FACTOR` blocs, etc. Construit une fois par courbe (O(n)), il répond ensuite à une requête d'intervalle
/// sans parcourir tout l'intervalle.
///
/// À garder à côté de la courbe : les indices rendus désignent des positions dans **cette** courbe.
#[derive(Clone, Debug)]
pub struct MinMaxPyramid {
    /// `levels[0]` : blocs de `FACTOR` échantillons ; `levels[k]` : blocs de `FACTOR^(k+1)` échantillons.
    /// Le dernier bloc d'un niveau peut être incomplet ; il n'est alors jamais utilisé par une requête (voir
    /// [`Self::extremes`]).
    levels: Vec<Vec<Block>>,
    /// Nombre d'échantillons résumés.
    n: usize,
}

impl MinMaxPyramid {
    /// Construit l'index d'une courbe. Rien n'est construit sous `FACTOR` échantillons (un parcours direct suffit) ni
    /// au-delà de `u32::MAX` (les indices sont rangés en `u32` : [`Self::extremes`] rend alors `None`).
    pub fn build(y: &[f64]) -> Self {
        let n = y.len();
        let mut levels: Vec<Vec<Block>> = Vec::new();
        // `u32::MAX` sert d'étiquette de trou : il ne doit pas être un indice valide
        if n < FACTOR || n >= u32::MAX as usize {
            return Self { levels, n };
        }
        let first: Vec<Block> = (0..n.div_ceil(FACTOR))
            .map(|b| {
                let start = b * FACTOR;
                let end = (start + FACTOR).min(n);
                (start + 1..end).fold(Block::single(start, y), |acc, i| Block::join(acc, Block::single(i, y)))
            })
            .collect();
        levels.push(first);
        while levels.last().is_some_and(|l| l.len() > 1) {
            let below = levels.last().expect("niveau précédent");
            let up: Vec<Block> = (0..below.len().div_ceil(FACTOR))
                .map(|b| {
                    let start = b * FACTOR;
                    let end = (start + FACTOR).min(below.len());
                    below[start + 1..end]
                        .iter()
                        .fold(below[start], |acc, &x| Block::join(acc, x))
                })
                .collect();
            levels.push(up);
        }
        Self { levels, n }
    }

    /// Mémoire occupée par l'index, en octets (hors `Vec` eux-mêmes).
    pub fn bytes(&self) -> usize {
        self.levels.iter().map(|l| l.len() * std::mem::size_of::<Block>()).sum()
    }

    /// Nombre d'échantillons résumés.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Vrai si l'index ne résume aucun échantillon.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Vrai si l'index peut servir (courbe de `FACTOR` échantillons au moins, indices tenant en `u32`).
    pub fn usable(&self) -> bool {
        !self.levels.is_empty()
    }

    /// Minimum et maximum de `y[a..b]`, en combinant les plus gros blocs alignés qui tiennent dans l'intervalle.
    /// `None` si l'index n'est pas utilisable ou si l'intervalle est vide.
    ///
    /// Un bloc n'est pris que si l'intervalle qu'il couvre tient **entièrement** dans `a..b` : un bloc incomplet de
    /// fin de niveau dépasse `n`, donc `b`, et n'est jamais pris.
    pub fn extremes(&self, y: &[f64], a: usize, b: usize) -> Option<Extremes> {
        if !self.usable() || a >= b || b > self.n {
            return None;
        }
        let mut acc: Option<Block> = None;
        let mut i = a;
        while i < b {
            // plus haut niveau dont un bloc commence en `i` et finit dans `b`
            let mut taken = None;
            for (k, level) in self.levels.iter().enumerate() {
                let size = FACTOR.pow(k as u32 + 1);
                if i.is_multiple_of(size) && i + size <= b {
                    taken = Some((level[i / size], size));
                } else {
                    break;
                }
            }
            let (part, step) = taken.unwrap_or_else(|| (Block::single(i, y), 1));
            acc = Some(match acc {
                None => part,
                Some(prev) => Block::join(prev, part),
            });
            i += step;
        }
        acc.map(Block::extremes)
    }
}

/// Bornes du tracé : premier et dernier indice retenus pour la fenêtre `x0..x1` (un point de marge de chaque côté),
/// et taille des paquets de décimation (0 : aucune décimation, tous les points sont rendus).
///
/// `t` doit être croissant. Rend `None` si rien n'est visible.
fn window(t: &[f64], y: &[f64], x0: f64, x1: f64, max_points: usize) -> Option<(usize, usize, usize)> {
    let n = t.len().min(y.len());
    if n == 0 || x0.is_nan() || x1.is_nan() || x0 > x1 {
        return None;
    }
    let i0 = t[..n].partition_point(|&v| v < x0).saturating_sub(1);
    let i1 = (t[..n].partition_point(|&v| v <= x1) + 1).min(n);
    let count = i1.saturating_sub(i0);
    if count == 0 {
        return None;
    }
    let budget = max_points.max(4);
    let chunk = if count <= budget {
        0
    } else {
        (2 * count).div_ceil(budget)
    };
    Some((i0, i1, chunk))
}

/// Points d'une courbe visibles entre `x0` et `x1` (un point de marge de chaque côté), au plus `max_points` environ :
/// au-delà, décimation min/max. Parcourt tous les échantillons visibles : O(n). Référence de [`window_points`].
pub fn window_points_scan(t: &[f64], y: &[f64], x0: f64, x1: f64, max_points: usize) -> Vec<[f64; 2]> {
    let Some((i0, i1, chunk)) = window(t, y, x0, x1, max_points) else {
        return Vec::new();
    };
    if chunk == 0 {
        return (i0..i1).map(|i| [t[i], y[i]]).collect();
    }
    let mut out = Vec::with_capacity(max_points + 2);
    for start in (i0..i1).step_by(chunk) {
        let end = (start + chunk).min(i1);
        let part = (start + 1..end).fold(Block::single(start, y), |acc, i| Block::join(acc, Block::single(i, y)));
        part.extremes().push_points(t, y, &mut out);
    }
    out
}

/// Même résultat que [`window_points_scan`], en interrogeant l'index au lieu de parcourir la courbe : le coût ne
/// dépend plus du nombre d'échantillons mais du nombre de points rendus.
///
/// `pyramid` doit avoir été construit sur **cette** courbe `y` ; s'il n'est pas utilisable (courbe trop courte ou
/// trop longue), le parcours direct prend le relais.
pub fn window_points(
    t: &[f64],
    y: &[f64],
    pyramid: &MinMaxPyramid,
    x0: f64,
    x1: f64,
    max_points: usize,
) -> Vec<[f64; 2]> {
    let n = t.len().min(y.len());
    if !pyramid.usable() || pyramid.len() != n {
        return window_points_scan(t, y, x0, x1, max_points);
    }
    let Some((i0, i1, chunk)) = window(t, y, x0, x1, max_points) else {
        return Vec::new();
    };
    if chunk == 0 {
        return (i0..i1).map(|i| [t[i], y[i]]).collect();
    }
    let mut out = Vec::with_capacity(max_points + 2);
    for start in (i0..i1).step_by(chunk) {
        let end = (start + chunk).min(i1);
        let part = pyramid
            .extremes(y, start, end)
            .expect("intervalle non vide dans la courbe résumée");
        part.push_points(t, y, &mut out);
    }
    out
}
