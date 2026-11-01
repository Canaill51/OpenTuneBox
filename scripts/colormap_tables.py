#!/usr/bin/env python3
"""Écrit les palettes des cartes (hot, inferno, viridis, turbo, jet) pour `rust/otb-gui/src/colormaps.rs` (6-2).

Usage : colormap_tables.py SORTIE.rs

Chaque palette est une table de 256 couleurs [r, g, b], lue telle quelle dans `gui/tools/_spectral_common.colormap`
(donc dans pyqtgraph pour inferno, viridis et turbo) : l'interface egui affiche les mêmes couleurs que l'interface Qt.
Le fichier ne contient que des octets : la conversion en couleur est dans `gui/theme.rs` (aucune couleur hors du thème).
Licences : inferno et viridis CC0 (matplotlib), turbo Apache-2.0 (Google).
"""
import sys

from opentunebox.gui.tools import _spectral_common as sc

N = 256


def main(out: str) -> None:
    lines = [
        "//! Palettes des cartes de fréquence (hot, inferno, viridis, turbo, jet), 256 couleurs [r, g, b] chacune.",
        "//! Fichier généré par `scripts/colormap_tables.py` : ne pas modifier à la main (le test `tests/colormaps.rs` en vérifie des valeurs connues).",
        "//! Licences : inferno et viridis CC0 (matplotlib, Nathaniel J. Smith, Stefan van der Walt, Eric Firing), turbo Apache-2.0 (Google).",
        "",
    ]
    for name in sc.COLORMAP_NAMES:
        lut = sc.colormap(name).getLookupTable(0.0, 1.0, N, alpha=False)
        assert lut.shape == (N, 3), (name, lut.shape)
        lines.append(f"pub const {name.upper()}: [[u8; 3]; {N}] = [")
        for r, g, b in lut:
            lines.append(f"    [{int(r)}, {int(g)}, {int(b)}],")
        lines.append("];")
        lines.append("")
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines))


if __name__ == "__main__":
    main(sys.argv[1])
