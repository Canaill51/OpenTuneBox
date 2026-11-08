//! `pt1_gain` et `notch_q` : les deux formules du firmware Betaflight qui n'avaient **aucun test**.
//!
//! Elles étaient vérifiées uniquement en rejouant les références figées (`$OTB_GOLDEN`, `otb-parity/tests/golden.rs`),
//! donc jamais par la CI, qui n'a pas le corpus. Or elles décident du gain des passe-bas et de la largeur des notchs :
//! une erreur ici fausse tous les conseils de filtrage (W-054).
//!
//! Les valeurs attendues ne sont pas reprises du code testé : elles viennent du calcul direct de la formule du
//! firmware (`docs/spec-betaflight.md`), et les propriétés exactes ci-dessous tiennent par construction.

use otb_dsp::{notch_q, pt1_gain};

/// Les deux valeurs sont égales au bit près.
fn bit_pres(got: f64, want: f64, quoi: &str) {
    assert_eq!(got.to_bits(), want.to_bits(), "{quoi} : {got:.17e} ≠ {want:.17e}");
}

#[test]
fn pt1_gain_suit_la_formule_du_firmware() {
    // k = ω/(ω+1), ω = 2π·fc/fs — calculés hors de ce dépôt, à 17 chiffres.
    for (fc, fs, want) in [
        (100.0, 8000.0, 0.072_820_507_087_338_2),
        (90.0, 3200.0, 0.150_176_252_382_773_85),
        (250.0, 8000.0, 0.164_123_890_338_906_7),
        (1.0, 8000.0, 0.000_784_781_797_215_247_7),
    ] {
        bit_pres(pt1_gain(fc, fs), want, &format!("pt1_gain({fc}, {fs})"));
    }
}

/// Propriétés vraies par construction, indépendantes des chiffres ci-dessus.
#[test]
fn pt1_gain_a_les_proprietes_d_un_passe_bas() {
    let fs = 8000.0;
    // ω = 1 exactement quand fc = fs/2π : le gain vaut alors 0,5 pile.
    bit_pres(pt1_gain(fs / (2.0 * std::f64::consts::PI), fs), 0.5, "gain à ω = 1");
    // Coupure nulle : le filtre ne laisse rien passer d'un pas à l'autre.
    bit_pres(pt1_gain(0.0, fs), 0.0, "pt1_gain(0)");
    // Le gain croît avec la coupure, et reste dans ]0, 1[.
    let mut precedent = 0.0;
    for i in 1..200 {
        let k = pt1_gain(f64::from(i) * 10.0, fs);
        assert!(k > precedent, "le gain doit croître avec la coupure (fc = {})", i * 10);
        assert!(k > 0.0 && k < 1.0, "gain hors de ]0,1[ : {k}");
        precedent = k;
    }
}

#[test]
fn notch_q_suit_la_formule_du_firmware() {
    // Q = centre·coupure / (centre² − coupure²), formule de `filterGetNotchQ`.
    for (centre, coupure, want) in [(200.0, 150.0, 1.714_285_714_285_714_2), (150.0, 100.0, 1.2)] {
        bit_pres(notch_q(centre, coupure), want, &format!("notch_q({centre}, {coupure})"));
    }
}

/// Le Q ne dépend que du **rapport** entre le centre et la coupure : doubler les deux ne le change pas.
/// C'est ce qui permet au même notch de servir à toutes les fréquences moteur.
#[test]
fn notch_q_ne_depend_que_du_rapport_des_frequences() {
    for (centre, coupure) in [(200.0, 150.0), (150.0, 100.0), (333.0, 250.0)] {
        let base = notch_q(centre, coupure);
        for facteur in [2.0, 4.0, 10.0] {
            let mis_a_l_echelle = notch_q(centre * facteur, coupure * facteur);
            assert!(
                (mis_a_l_echelle - base).abs() <= base.abs() * 1e-12,
                "notch_q doit être invariant par mise à l'échelle : {base} contre {mis_a_l_echelle}"
            );
        }
    }
    // Une coupure plus large que le centre donne un Q négatif : le firmware ne le produit jamais, mais la
    // fonction ne doit ni paniquer ni rendre NaN.
    assert!(notch_q(100.0, 200.0) < 0.0, "Q d'une coupure au-dessus du centre");
    assert!(
        notch_q(100.0, 100.0).is_infinite(),
        "centre = coupure : division par zéro"
    );
}
