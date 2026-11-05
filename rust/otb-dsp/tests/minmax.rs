//! Décimation min/max et index multi-résolution (`otb_dsp::minmax`, D-048, W-049).
//!
//! Contrat vérifié ici :
//! 1. **référence historique** : sur une courbe sans trou, la décimation rend exactement ce que rendait
//!    `otb-gui::plots::visible_points` (copiée ci-dessous, figée) — point pour point, pas seulement l'enveloppe ;
//! 2. **index exact** : `window_points` (par la pyramide) rend exactement ce que rend `window_points_scan` (par
//!    parcours), sur des courbes et des fenêtres variées, trous compris ;
//! 3. **requête d'intervalle** : `MinMaxPyramid::extremes` rend ce que rend un parcours naïf du même intervalle ;
//! 4. **mémoire** : l'index tient dans le budget annoncé par la documentation du module.

use otb_dsp::minmax::{Extremes, FACTOR, MinMaxPyramid, window_points, window_points_scan};

// ------------------------------------------------------------------ référence historique

/// Copie **figée** de `otb-gui::plots::visible_points` d'avant D-048 (lot 10 du plan des chantiers), gardée comme
/// témoin : le nouveau calcul doit lui être identique sur toute courbe sans `NaN`.
///
/// ⚠️ Ne pas « corriger » cette fonction : son intérêt est d'être l'ancien comportement, trous compris (un `NaN` en
/// tête de paquet y gagne toutes les comparaisons et efface le min/max réel du paquet).
fn visible_points_avant(t: &[f64], y: &[f64], x0: f64, x1: f64, max_points: usize) -> Vec<[f64; 2]> {
    let n = t.len().min(y.len());
    if n == 0 || x0.is_nan() || x1.is_nan() || x0 > x1 {
        return Vec::new();
    }
    let i0 = t[..n].partition_point(|&v| v < x0).saturating_sub(1);
    let i1 = (t[..n].partition_point(|&v| v <= x1) + 1).min(n);
    let count = i1.saturating_sub(i0);
    if count <= max_points.max(4) {
        return (i0..i1).map(|i| [t[i], y[i]]).collect();
    }
    let chunk = (2 * count).div_ceil(max_points.max(4));
    let mut out = Vec::with_capacity(max_points + 2);
    for start in (i0..i1).step_by(chunk) {
        let end = (start + chunk).min(i1);
        let (mut lo, mut hi) = (start, start);
        for i in start..end {
            if y[i] < y[lo] {
                lo = i;
            }
            if y[i] > y[hi] {
                hi = i;
            }
        }
        let (a, b) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        out.push([t[a], y[a]]);
        if b != a {
            out.push([t[b], y[b]]);
        }
    }
    out
}

// ------------------------------------------------------------------ courbes d'essai

/// Générateur reproductible (congruence linéaire), pour ne dépendre d'aucune bibliothèque.
struct Lcg(u64);

impl Lcg {
    fn next_f64(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn temps(n: usize) -> Vec<f64> {
    (0..n).map(|i| i as f64 * 0.001).collect()
}

/// Les courbes d'essai, avec leur nom : chacune éprouve un cas particulier du découpage.
fn courbes(n: usize) -> Vec<(&'static str, Vec<f64>)> {
    let mut rng = Lcg(0x5EED);
    let mut pic = vec![0.0; n];
    if n > 7 {
        pic[7] = 1e6; // pic isolé, au milieu d'un bloc
        pic[n - 1] = -1e6; // pic sur le tout dernier échantillon
    }
    let mut plateau = vec![4.0; n];
    if n > 3 {
        plateau[1] = 4.0; // égalité : le premier atteint doit gagner
        plateau[2] = 4.0;
    }
    vec![
        ("constante", vec![1.5; n]),
        ("monotone", (0..n).map(|i| i as f64).collect()),
        ("monotone décroissante", (0..n).map(|i| -(i as f64)).collect()),
        ("plateau", plateau),
        ("pic isolé", pic),
        ("bruit", (0..n).map(|_| rng.next_f64() * 2.0 - 1.0).collect()),
        (
            "sinus bruité",
            (0..n)
                .map(|i| (i as f64 * 0.05).sin() * 500.0 + rng.next_f64())
                .collect(),
        ),
    ]
}

/// Courbes à trous (`NaN`), que la référence historique traite au hasard du découpage : comparées seulement au
/// parcours de référence du nouveau calcul.
fn courbes_a_trous(n: usize) -> Vec<(&'static str, Vec<f64>)> {
    let mut un_trou = (0..n).map(|i| (i as f64 * 0.05).sin()).collect::<Vec<_>>();
    un_trou[n / 3] = f64::NAN;
    let mut trou_en_tete = (0..n).map(|i| i as f64).collect::<Vec<_>>();
    trou_en_tete[0] = f64::NAN;
    let mut trou_long = (0..n).map(|i| (i as f64 * 0.01).cos()).collect::<Vec<_>>();
    for v in trou_long.iter_mut().take(n / 2).skip(n / 4) {
        *v = f64::NAN;
    }
    vec![
        ("un trou", un_trou),
        ("trou en tête", trou_en_tete),
        ("trou long", trou_long),
        ("tout en trou", vec![f64::NAN; n]),
    ]
}

/// Fenêtres d'essai `(x0, x1)` pour une courbe de `n` échantillons échantillonnée à 1 ms.
fn fenetres(n: usize) -> Vec<(&'static str, f64, f64)> {
    let fin = (n.saturating_sub(1)) as f64 * 0.001;
    vec![
        ("vue entière", 0.0, fin),
        ("vue entière élargie", -10.0, fin + 10.0),
        ("moitié gauche", 0.0, fin / 2.0),
        ("moitié droite", fin / 2.0, fin),
        ("tranche décalée", fin * 0.137, fin * 0.642),
        ("tranche étroite", fin * 0.5, fin * 0.5 + 0.03),
        ("un seul point", fin * 0.5, fin * 0.5),
        ("hors du log, à droite", fin + 1.0, fin + 2.0),
        ("hors du log, à gauche", -2.0, -1.0),
    ]
}

/// Budgets de points rendus : largeurs de fenêtre réalistes, et des budgets minuscules qui forcent de gros paquets.
const BUDGETS: [usize; 6] = [4, 7, 100, 1280, 3840, 1_000_000];

// ------------------------------------------------------------------ 1. référence historique

#[test]
fn sans_trou_le_parcours_rend_exactement_l_ancienne_sortie() {
    for n in [1usize, 2, 3, 5, 8, 9, 64, 65, 1000, 4096, 5000] {
        let t = temps(n);
        for (nom, y) in courbes(n) {
            for (quoi, x0, x1) in fenetres(n) {
                for budget in BUDGETS {
                    let attendu = visible_points_avant(&t, &y, x0, x1, budget);
                    let obtenu = window_points_scan(&t, &y, x0, x1, budget);
                    assert_eq!(
                        obtenu, attendu,
                        "courbe « {nom} », fenêtre « {quoi} », n = {n}, budget = {budget}"
                    );
                }
            }
        }
    }
}

#[test]
fn sans_trou_la_pyramide_rend_exactement_l_ancienne_sortie() {
    for n in [8usize, 9, 64, 65, 1000, 4096, 5000, 20_000] {
        let t = temps(n);
        for (nom, y) in courbes(n) {
            let p = MinMaxPyramid::build(&y);
            for (quoi, x0, x1) in fenetres(n) {
                for budget in BUDGETS {
                    let attendu = visible_points_avant(&t, &y, x0, x1, budget);
                    let obtenu = window_points(&t, &y, &p, x0, x1, budget);
                    assert_eq!(
                        obtenu, attendu,
                        "courbe « {nom} », fenêtre « {quoi} », n = {n}, budget = {budget}"
                    );
                }
            }
        }
    }
}

#[test]
fn moins_de_points_que_le_budget_rend_tous_les_points() {
    let n = 500;
    let t = temps(n);
    let y: Vec<f64> = (0..n).map(|i| (i as f64).sin()).collect();
    let p = MinMaxPyramid::build(&y);
    let tous: Vec<[f64; 2]> = (0..n).map(|i| [t[i], y[i]]).collect();
    for budget in [n, n + 1, 10_000] {
        assert_eq!(window_points_scan(&t, &y, -1.0, 99.0, budget), tous);
        assert_eq!(window_points(&t, &y, &p, -1.0, 99.0, budget), tous);
    }
}

// ------------------------------------------------------------------ 2. index exact, trous compris

#[test]
fn la_pyramide_rend_le_meme_resultat_que_le_parcours_avec_des_trous() {
    for n in [8usize, 9, 65, 1000, 4096, 9999] {
        let t = temps(n);
        for (nom, y) in courbes_a_trous(n) {
            let p = MinMaxPyramid::build(&y);
            for (quoi, x0, x1) in fenetres(n) {
                for budget in BUDGETS {
                    let attendu = window_points_scan(&t, &y, x0, x1, budget);
                    let obtenu = window_points(&t, &y, &p, x0, x1, budget);
                    assert_eq!(
                        format!("{obtenu:?}"),
                        format!("{attendu:?}"),
                        "courbe « {nom} », fenêtre « {quoi} », n = {n}, budget = {budget}"
                    );
                }
            }
        }
    }
}

#[test]
fn un_paquet_a_trou_coupe_la_ligne_et_garde_le_min_max_des_paquets_voisins() {
    // 40 échantillons, un trou au 20e ; budget 4 → paquets de 20 : le 1er paquet est propre, le 2e a le trou
    let n = 40;
    let t = temps(n);
    let mut y: Vec<f64> = (0..n).map(|i| i as f64).collect();
    y[20] = f64::NAN;
    let p = MinMaxPyramid::build(&y);
    let obtenu = window_points(&t, &y, &p, 0.0, t[n - 1], 4);
    assert_eq!(
        obtenu.len(),
        3,
        "deux points pour le paquet propre, un pour le trou : {obtenu:?}"
    );
    assert_eq!([obtenu[0], obtenu[1]], [[t[0], 0.0], [t[19], 19.0]]);
    assert_eq!(obtenu[2][0], t[20]);
    assert!(obtenu[2][1].is_nan(), "le paquet à trou est rendu comme un trou");
    // l'ancienne sortie, elle, perdait le min et le max du paquet à trou (témoin du défaut corrigé)
    let avant = visible_points_avant(&t, &y, 0.0, t[n - 1], 4);
    assert!(
        avant.len() == 3 && avant[2][1].is_nan() && avant[2][0] == t[20],
        "témoin : l'ancienne sortie remplaçait tout le paquet par son premier NaN ({avant:?})"
    );
}

#[test]
fn un_pic_isole_n_est_jamais_perdu() {
    let n = 100_000;
    let t = temps(n);
    let mut y = vec![0.0; n];
    let (i_haut, i_bas) = (54_321, 7);
    y[i_haut] = 1234.5;
    y[i_bas] = -99.0;
    let p = MinMaxPyramid::build(&y);
    for budget in [4usize, 100, 3840] {
        for (quoi, x0, x1) in fenetres(n) {
            let pts = window_points(&t, &y, &p, x0, x1, budget);
            let visible = |i: usize| t[i] >= x0 && t[i] <= x1;
            if visible(i_haut) {
                assert!(
                    pts.iter().any(|q| q[1] == 1234.5),
                    "pic haut perdu, fenêtre « {quoi} », budget {budget}"
                );
            }
            if visible(i_bas) {
                assert!(
                    pts.iter().any(|q| q[1] == -99.0),
                    "pic bas perdu, fenêtre « {quoi} », budget {budget}"
                );
            }
        }
    }
}

#[test]
fn fenetres_degenerees_ne_rendent_rien() {
    let t = temps(100);
    let y: Vec<f64> = (0..100).map(|i| i as f64).collect();
    let p = MinMaxPyramid::build(&y);
    for (x0, x1) in [(5.0, 1.0), (f64::NAN, 1.0), (0.0, f64::NAN)] {
        assert!(window_points_scan(&t, &y, x0, x1, 100).is_empty());
        assert!(window_points(&t, &y, &p, x0, x1, 100).is_empty());
    }
    let vide = MinMaxPyramid::build(&[]);
    assert!(window_points_scan(&[], &[], 0.0, 1.0, 100).is_empty());
    assert!(window_points(&[], &[], &vide, 0.0, 1.0, 100).is_empty());
    assert!(!vide.usable(), "aucun index sous FACTOR échantillons");
    assert!(vide.is_empty());
}

#[test]
fn une_pyramide_d_une_autre_courbe_est_refusee_et_le_parcours_prend_le_relais() {
    let t = temps(1000);
    let y: Vec<f64> = (0..1000).map(|i| (i as f64).sin()).collect();
    let autre = MinMaxPyramid::build(&vec![0.0; 777]);
    assert_eq!(
        window_points(&t, &y, &autre, 0.0, 1.0, 100),
        window_points_scan(&t, &y, 0.0, 1.0, 100),
        "une pyramide dont la taille ne correspond pas ne doit pas être utilisée"
    );
}

// ------------------------------------------------------------------ 3. requête d'intervalle

#[test]
fn extremes_rend_ce_que_rend_un_parcours_naif() {
    /// Parcours naïf, à la règle du module : un `NaN` dans l'intervalle le rend tout entier « trou ».
    fn naif(y: &[f64], a: usize, b: usize) -> Extremes {
        if let Some(i) = (a..b).find(|&i| y[i].is_nan()) {
            return Extremes::Hole { at: i as u32 };
        }
        let (mut lo, mut hi) = (a, a);
        for i in a..b {
            if y[i] < y[lo] {
                lo = i;
            }
            if y[i] > y[hi] {
                hi = i;
            }
        }
        Extremes::Clean {
            lo: lo as u32,
            hi: hi as u32,
        }
    }

    let n = 3000;
    let mut rng = Lcg(42);
    for (nom, y) in courbes(n).into_iter().chain(courbes_a_trous(n)) {
        let p = MinMaxPyramid::build(&y);
        assert!(p.usable(), "courbe « {nom} » : index attendu");
        let mut cas = vec![
            (0usize, n),
            (0, 1),
            (n - 1, n),
            (0, FACTOR),
            (1, FACTOR + 1),
            (FACTOR, 2 * FACTOR),
        ];
        for _ in 0..400 {
            let a = (rng.next_f64() * n as f64) as usize;
            let b = (rng.next_f64() * n as f64) as usize;
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            cas.push((a, (b + 1).min(n)));
        }
        for (a, b) in cas {
            if a >= b {
                continue;
            }
            assert_eq!(
                p.extremes(&y, a, b),
                Some(naif(&y, a, b)),
                "courbe « {nom} », intervalle {a}..{b}"
            );
        }
        assert_eq!(p.extremes(&y, 5, 5), None, "intervalle vide");
        assert_eq!(p.extremes(&y, 0, n + 1), None, "intervalle hors de la courbe");
    }
}

// ------------------------------------------------------------------ 4. mémoire

#[test]
fn la_memoire_de_l_index_tient_dans_le_budget_annonce() {
    for n in [1000usize, 100_000, 620_079] {
        let y = vec![1.0; n];
        let p = MinMaxPyramid::build(&y);
        let budget = 24 * n / (FACTOR - 1) + 192; // documentation du module : 24 / (FACTOR - 1) octets par échantillon
        assert!(
            p.bytes() <= budget,
            "n = {n} : index de {} octets, budget {budget}",
            p.bytes()
        );
        assert_eq!(p.len(), n);
        // une courbe f64 pèse 8 n : l'index doit rester une petite fraction
        assert!(
            p.bytes() * 2 < 8 * n,
            "n = {n} : index de {} octets contre {} pour la courbe",
            p.bytes(),
            8 * n
        );
    }
}

#[test]
fn les_tailles_frontieres_de_l_index_rendent_le_meme_resultat_que_le_parcours() {
    // frontières de `build` : un niveau d'un seul bloc, index inutilisable, puissances exactes de FACTOR
    let mut tailles = vec![FACTOR - 1, FACTOR, FACTOR + 1];
    let mut taille = FACTOR;
    for _ in 0..4 {
        taille *= FACTOR;
        tailles.extend([taille - 1, taille, taille + 1]);
    }
    for n in tailles {
        let t = temps(n);
        for (nom, y) in courbes(n).into_iter().chain(courbes_a_trous(n)) {
            let p = MinMaxPyramid::build(&y);
            assert_eq!(
                p.usable(),
                n >= FACTOR,
                "n = {n} : index attendu seulement à partir de FACTOR"
            );
            for (quoi, x0, x1) in fenetres(n) {
                for budget in [4usize, 100, 3840] {
                    assert_eq!(
                        format!("{:?}", window_points(&t, &y, &p, x0, x1, budget)),
                        format!("{:?}", window_points_scan(&t, &y, x0, x1, budget)),
                        "courbe « {nom} », fenêtre « {quoi} », n = {n}, budget = {budget}"
                    );
                }
            }
        }
    }
}

#[test]
fn les_valeurs_extremes_et_le_zero_signe_ne_changent_rien() {
    let n = 2000;
    let t = temps(n);
    let mut y: Vec<f64> = (0..n).map(|i| if i % 2 == 0 { 0.0 } else { -0.0 }).collect();
    y[500] = f64::INFINITY;
    y[501] = f64::NEG_INFINITY;
    y[1500] = f64::MAX;
    y[1501] = f64::MIN;
    let p = MinMaxPyramid::build(&y);
    for budget in [4usize, 100, 3840] {
        let obtenu = window_points(&t, &y, &p, 0.0, t[n - 1], budget);
        assert_eq!(
            format!("{obtenu:?}"),
            format!("{:?}", visible_points_avant(&t, &y, 0.0, t[n - 1], budget)),
            "budget = {budget}"
        );
        assert!(
            obtenu.iter().any(|q| q[1] == f64::INFINITY) && obtenu.iter().any(|q| q[1] == f64::NEG_INFINITY),
            "les infinis doivent rester des extrêmes, budget = {budget}"
        );
    }
}

#[test]
fn un_temps_plus_court_que_la_courbe_ne_fausse_pas_le_resultat() {
    // `window_points` ne regarde que `t.len().min(y.len())` échantillons, alors que l'index est bâti sur toute la
    // courbe : la garde `pyramid.len() != n` le fait passer par le parcours. Ce test ne vérifie que la sortie (le
    // chemin pris ne se voit pas d'ici, et l'index donnerait d'ailleurs la même réponse tant que la fenêtre tient
    // dans la partie commune) ; c'est `une_pyramide_d_une_autre_courbe_est_refusee…` qui garde la garde elle-même
    let y: Vec<f64> = (0..1000).map(|i| (i as f64 * 0.1).sin()).collect();
    let t = temps(600);
    let p = MinMaxPyramid::build(&y);
    assert_eq!(p.len(), 1000);
    for budget in [4usize, 100, 3840] {
        assert_eq!(
            window_points(&t, &y, &p, 0.0, 10.0, budget),
            window_points_scan(&t, &y, 0.0, 10.0, budget),
            "budget = {budget}"
        );
    }
}
