//! Tri des fenêtres de la réponse indicielle (D-082) : bande sur le régime établi, distance à la courbe médiane, et tri
//! historique de Python gardé pour les références. Vols fabriqués : la CI n'a pas le corpus (L-023).

use otb_analysis::step::{
    LoopInput, MEDIAN_MIN_WINDOWS, Rejection, RobustRejection, StepOptions, StepResult, median_keep, step_response,
};

const FS: f64 = 1000.0;

/// Consigne : bruit blanc reproductible passé deux fois par un passe-bas (≈ 8 Hz), comme des mouvements de manche (°/s).
/// Une somme de quelques sinus ne convient pas : la déconvolution n'y voit que quelques fréquences et le palier n'a plus de sens.
fn setpoint(n: usize) -> Vec<f64> {
    let mut s: u64 = 12345;
    let a = 1.0 - (-2.0 * std::f64::consts::PI * 8.0 / FS).exp();
    let (mut y1, mut y2) = (0.0, 0.0);
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let w = ((s >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0;
            y1 += a * (w - y1);
            y2 += a * (y1 - y2);
            4000.0 * y2
        })
        .collect()
}

/// Gyro : la consigne passée par un retard du premier ordre (≈ 10 ms), multipliée par `gain(i)`.
fn gyro(sp: &[f64], gain: impl Fn(usize) -> f64) -> Vec<f64> {
    let a = 1.0 - (-1.0 / (0.010 * FS)).exp();
    let mut y = 0.0;
    sp.iter()
        .enumerate()
        .map(|(i, &x)| {
            y += a * (x - y);
            gain(i) * y
        })
        .collect()
}

fn run(sp: &[f64], gy: &[f64], o: &StepOptions) -> StepResult {
    step_response(sp, gy, FS, o, &LoopInput::Absent, None).unwrap()
}

/// Moyenne de la courbe moyenne entre 200 et 500 ms.
fn plateau(r: &StepResult) -> f64 {
    let v: Vec<f64> = r
        .t_ms
        .iter()
        .zip(&r.mean)
        .filter(|(t, _)| **t >= 200.0)
        .map(|(_, m)| *m)
        .collect();
    v.iter().sum::<f64>() / v.len() as f64
}

/// Une moitié du vol répond 1,8 fois trop fort : le tri de Python (bande 0,5–3) garde ses fenêtres et la moyenne monte ;
/// la bande 0,5–1,5 les écarte et la moyenne revient près de 1.
#[test]
fn la_bande_ecarte_les_fenetres_au_palier_aberrant() {
    let n = (40.0 * FS) as usize;
    let sp = setpoint(n);
    let gy = gyro(&sp, |i| if i < n / 2 { 1.0 } else { 1.8 });
    let py = run(&sp, &gy, &StepOptions::python());
    let rb = run(&sp, &gy, &StepOptions::default());
    assert_eq!(py.windows, rb.windows, "mêmes fenêtres candidates");
    assert!(
        py.n * 100 >= py.windows * 95,
        "Python : {} fenêtres gardées sur {}",
        py.n,
        py.windows
    );
    assert!(rb.n < py.n * 7 / 10, "bande : {} fenêtres gardées sur {}", rb.n, py.n);
    assert!(plateau(&py) > 1.25, "Python : palier {}", plateau(&py));
    assert!((plateau(&rb) - 1.0).abs() < 0.05, "robuste : palier {}", plateau(&rb));
    assert_eq!(rb.responses.len(), rb.n * rb.t_ms.len());
}

/// Le tri par la médiane écarte une minorité de fenêtres qui restent dans la bande mais s'éloignent de la courbe médiane.
#[test]
fn la_mediane_ecarte_les_fenetres_eloignees_que_la_bande_garde() {
    let n = (40.0 * FS) as usize;
    let sp = setpoint(n);
    // 4 s sur 40 répondent 1,35 fois : dans la bande (< 1,5), loin de la médiane
    let gy = gyro(&sp, |i| if (20_000..24_000).contains(&i) { 1.35 } else { 1.0 });
    let band_only = StepOptions {
        rejection: Rejection::Robust(RobustRejection {
            k: f64::INFINITY,
            ..RobustRejection::default()
        }),
        ..StepOptions::default()
    };
    let b = run(&sp, &gy, &band_only);
    let m = run(&sp, &gy, &StepOptions::default());
    assert!(
        b.n * 100 >= b.windows * 95,
        "bande seule : {} fenêtres gardées sur {}",
        b.n,
        b.windows
    );
    assert!(m.n < b.n, "médiane : {} fenêtres sur {}", m.n, b.n);
    assert!(m.n > b.n * 3 / 4, "médiane : seulement {} fenêtres sur {}", m.n, b.n);
    assert!(
        plateau(&m) < plateau(&b),
        "palier {} contre {}",
        plateau(&m),
        plateau(&b)
    );
    // la boucle fermée est mesurée sur les seules fenêtres gardées (second passage) : elle diffère de celle de la bande seule
    assert_ne!(m.loop_.bandwidth_hz.to_bits(), b.loop_.bandwidth_hz.to_bits());
}

/// Ligne `i` de `rows` : constante `v`.
fn rows_of(values: &[f64], len: usize) -> Vec<f64> {
    values.iter().flat_map(|&v| std::iter::repeat_n(v, len)).collect()
}

#[test]
fn median_keep_ecarte_la_ligne_aberrante() {
    let mut v = vec![1.0, 1.01, 0.99, 1.02, 0.98, 1.0, 1.01, 0.99, 1.0];
    v.push(1.4);
    let keep = median_keep(&rows_of(&v, 5), v.len(), 5, 3.0);
    assert_eq!(keep, (0..v.len() - 1).collect::<Vec<_>>());
}

#[test]
fn median_keep_garde_tout_sous_le_minimum_ou_sans_k() {
    let v = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 9.0];
    assert!(v.len() < MEDIAN_MIN_WINDOWS);
    assert_eq!(median_keep(&rows_of(&v, 3), v.len(), 3, 3.0).len(), v.len());
    let w = [1.0, 1.01, 0.99, 1.0, 1.0, 1.02, 0.98, 1.0, 9.0];
    assert_eq!(median_keep(&rows_of(&w, 3), w.len(), 3, f64::INFINITY).len(), w.len());
}

/// Toutes les lignes identiques : écart absolu médian nul, rien n'est écarté (seuil = médiane, comparaison large).
#[test]
fn median_keep_lignes_identiques() {
    let v = [1.0; 12];
    assert_eq!(median_keep(&rows_of(&v, 4), v.len(), 4, 3.0).len(), v.len());
}
