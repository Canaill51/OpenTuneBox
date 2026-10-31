#!/usr/bin/env python3
"""Sorties de numpy pour `smooth5` et `smooth15` de `rust/otb-advisor/src/rules.rs` (lissages `np.convolve(v, np.ones(K) / K, "same")`).

Usage : smooth_cases.py SORTIE.json    (à exécuter avec `.venv/bin/python`)

`rust/otb-advisor/tests/smooth.rs` rejoue chaque entrée en Rust et compare : (1) à 1e-15 près (tolérance relative, écart maximal affiché en
bits : un autre processeur, une autre bibliothèque de produits scalaires peuvent différer d'un bit, L-059) ; (2) au bit près seulement si
la variable d'environnement `OTB_BITEXACT=1` est définie (attendu sur ce poste : somme croissante, FMA au centre pour 15 valeurs ; centre sans
FMA et deux premières / deux dernières valeurs avec FMA pour 5 valeurs, L-061).

Entrées : valeurs aléatoires de signe quelconque et d'amplitude très variable (écart-type 30 × exp(N(0, 1))), plus un signal constant et un
signal tout positif. `smooth5` : longueurs 1 à 4 (numpy échange les tableaux : seulement la tolérance), 5 à 12, puis 15, 16, 64, 1 001, 10 000 ;
`smooth15` : au moins 15 valeurs (en dessous, Python lève).
"""
import json
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hexs


def vectors(lengths, seed):
    rng = np.random.default_rng(seed)
    out = []
    for n in lengths:
        out.append(rng.normal(0, 30, n) * np.exp(rng.normal(0, 1, n)))
        out.append(np.abs(rng.normal(100, 20, n)))  # tout positif : aucun effet de compensation
    out.append(np.full(max(lengths), 7.3))  # constant
    return out


def main():
    cases = {}
    for name, k, lengths in (("smooth5", 5, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16, 64, 1001, 10000]),
                             ("smooth15", 15, [15, 16, 17, 22, 23, 30, 64, 500, 1001, 10000])):
        ker = np.ones(k) / k
        rows = []
        for v in vectors(lengths, 7 + k):
            rows.append({"x": hexs(v), "y": hexs(np.convolve(v, ker, mode="same"))})
        cases[name] = rows
    text = json.dumps(cases, separators=(",", ":"))
    Path(sys.argv[1]).write_text(text)
    print({k: len(v) for k, v in cases.items()}, f"{len(text) / 1e6:.2f} Mo", file=sys.stderr)


if __name__ == "__main__":
    main()
