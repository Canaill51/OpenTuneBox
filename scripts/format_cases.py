#!/usr/bin/env python3
"""Cas de test des formats de nombres de Python, pour `rust/otb-format` (D-035, étape E2).

Usage : format_cases.py SORTIE.tsv [N_ALEATOIRES]   (défaut 1 000 000)

Une ligne par cas : spécification, bits du flottant en hexadécimal (IEEE 754, 16 chiffres), texte rendu par Python
(`ERR` quand Python lève une exception). Les spécifications sont celles qu'emploie le code de l'application :
.0f .1f .2f .3f +.0f +.3f .0% g .3g repr round0 round2. Les valeurs mêlent des motifs de bits au hasard, des nombres
ordinaires, des moitiés exactes (là où `round` et les arrondis diffèrent) et les valeurs spéciales.
"""
import math
import random
import struct
import sys

SPECS = [".0f", ".1f", ".2f", ".3f", "+.0f", "+.3f", ".0%", "g", ".3g", "repr", "round0", "round2"]


def render(spec: str, x: float) -> str:
    try:
        if spec == "repr":
            return repr(x)
        if spec == "round0":
            return str(round(x))
        if spec == "round2":
            return repr(round(x, 2))
        return format(x, spec)
    except (ValueError, OverflowError):
        return "ERR"


def values(n: int, rng: random.Random):
    special = [0.0, -0.0, math.nan, math.inf, -math.inf, 5e-324, 1.7976931348623157e308, 1e16, 1e-5, 9.5, 0.5, 1.5, 2.5,
               -0.5, -2.5, 0.05, 0.15, 0.25, 0.35, 2.675, 1.005, 999999.5, 1e15, 123456789.0, 0.0001, 0.00001234]
    yield from special
    for _ in range(n):
        k = rng.randrange(6)
        if k == 0:
            yield struct.unpack("<d", rng.getrandbits(64).to_bytes(8, "little"))[0]
        elif k == 1:
            yield rng.uniform(-1000, 1000)
        elif k == 2:
            yield rng.randint(-100000, 100000) / 2 ** rng.randint(0, 12)  # moitiés et quarts exacts
        elif k == 3:
            yield rng.randint(-10**6, 10**6) / 10 ** rng.randint(0, 5)  # décimales : 0,15, 2,675…
        elif k == 4:
            yield math.copysign(10 ** rng.uniform(-9, 18), rng.choice((-1, 1)))
        else:
            yield round(rng.uniform(-50, 50), rng.randint(0, 4)) + rng.choice((0, 0.5, 0.05, 0.005))


def main():
    out = sys.argv[1]
    n = int(sys.argv[2]) if len(sys.argv) > 2 else 1_000_000
    rng = random.Random(20261030)
    count = 0
    with open(out, "w", encoding="utf-8") as f:
        for x in values(n // len(SPECS) + 1, rng):
            bits = struct.pack("<d", x)[::-1].hex()
            for spec in SPECS:
                f.write(f"{spec}\t{bits}\t{render(spec, x)}\n")
                count += 1
    print(count, "cas écrits")


if __name__ == "__main__":
    main()
