#!/usr/bin/env python3
"""Cas de test de `advisor/profiles.py` pour `rust/otb-advisor/src/profiles.rs` (5a-1).

Usage : profiles_cases.py SORTIE.json [TABLES.rs]
        (TABLES.rs : écrit aussi les tables `\\d` et `\\w` de Python pour `rust/otb-advisor/src/pyclasses.rs`)

- `profiles`     : les 5 profils (tous les champs, flottants en hexadécimal) ;
- `delay_target` : `filter_delay_target_ms` (None, < 2 pouces, 2, 5, 14, NaN, négatif) ;
- `profile_key`  : toutes les frontières de fréquence (345, 290, 200 en ≤ 4S, 160, 115) ± le plus petit écart, cellules
  absentes, 0, 1, 2, 4, 5, 6 ;
- `detect`       : `detect_profile` avec une fréquence moteur connue ou non, des cellules et des noms de drone (whoop, meteor,
  65 / 75 seuls ou collés à d'autres chiffres, tailles en pouces à un ou deux chiffres, décimale « . » ou « , », unités
  `"`, `in`, `inch`, `inches`, `pouce(s)` suivies ou non d'une lettre, chiffres Unicode, majuscules, espaces Unicode, noms vides) ;
- `digits`, `spaces`, `words` : les classes `\\d`, `\\s`, `\\w` de Python (code points) : le test Rust y mesure ses propres
  définitions (les chiffres Unicode sont lus par `float()`, les frontières de mot par `\\b`).

Un flottant s'écrit en hexadécimal ; `"error": true` = Python a levé une exception.
"""
import json
import re
import struct
import sys
import unicodedata
from types import SimpleNamespace

from opentunebox.advisor import profiles
from opentunebox.advisor.profiles import PROFILES, detect_profile, filter_delay_target_ms, profile_key


def hx(x) -> str:
    return struct.pack("<d", float(x)).hex()


def call(fn, *args):
    try:
        return fn(*args), False
    except Exception:  # noqa: BLE001
        return None, True


def ranges(codes):
    out, s, p = [], codes[0], codes[0]
    for c in codes[1:]:
        if c != p + 1:
            out.append([s, p])
            s = c
        p = c
    out.append([s, p])
    return out


def prof(p) -> dict:
    d = {}
    for k, v in vars(p).items():
        if isinstance(v, tuple):
            d[k] = [hx(x) for x in v]
        elif isinstance(v, bool):
            d[k] = v
        elif isinstance(v, (int, float)):
            d[k] = hx(v)
        else:
            d[k] = v
    return d


NAMES = [
    "", "Whoop", "tiny whoop", "WHOOP75", "meteor75", "Meteor", "65", "75", "tw65", "bando 65mm", "75mm", "x75x", "165", "751",
    "1750", "2175", "65.5", "6575", "75-65", "75_", "a75", "75a", "ٌ75", "٦٥", "7٥", "7٥5",
    "5\"", "5 \"", "5  \"", "5\t\"", "5in", "5 in", "5inch", "5 inch", "5inches", "5 inches", "5inchx", "5inx", "5in_", "5in5",
    "5pouce", "5pouces", "5 pouces", "5 pouce", "5poucex", "5poucesx", "5.5\"", "5,5\"", "5.5in", "5,5 pouces", "5.\"", "5,\"",
    "10in", "12\"", "7in", "7\"", "3.5\"", "3,5in", "3.8in", "3.79in", "2in", "2.9\"", "5.8\"", "5.79\"", "7.8\"", "7.79in",
    "8in", "9\"", "10.5in", "11in", "123in", "123\"", "1234in", "12.34in", "1.23in", "9.99in", "01in", "00in", "0.5in", "99in",
    "٣in", "٣.٥in", "٣,٥\"", "３in", "５.５in", "5 in", "5 in", "5\u001cin", "5\u001fin", "5\u0085in", "5​in",
    "my 5\" quad", "quad 5in v2", "5inch 6in", "6in whoop", "whoop 5in", "75 5in", "5 in 7in", "x5in", "é5in", "a5in", "_5in",
    "5iné", "5in٠", "5in_", "5in-", "5in.", "5in ", "5in\n", "5in ", "FPV 5\" ULTRA", "Ünïcode 5 POUCES",
    "İstanbul 5in", "ǅ 5in", "ΣΑΣ 5in", "5\"in", "5\"\"", "1.5.5in", "5..5in", "5.5.5in", "5,.5in", "٥٫٥in", "5.٥in",
    "craft", "5", "5 ", "in", "inch", "pouces", "\"", "meteo", "whoo", "whoo p", "who op", "WH00P",
]


def rust_tables(path: str, digits, words) -> None:
    """Tables de caractères de Python (Unicode de sa version) : `\\d` et `\\w`, en suites (premier, dernier)."""
    def table(name: str, doc: str, r) -> str:
        rows = "".join(f"    (0x{a:X}, 0x{b:X}),\n" for a, b in r)
        return f"/// {doc}\npub const {name}: [(u32, u32); {len(r)}] = [\n{rows}];\n"
    assert all((b - a + 1) % 10 == 0 for a, b in digits)  # la valeur d'un chiffre est (c − premier) % 10
    head = (f"//! Classes de caractères de `re` (Python {sys.version.split()[0]}, Unicode {unicodedata.unidata_version}).\n"
            "//! Fichier généré par `scripts/profiles_cases.py` : ne pas modifier à la main (le test `tests/profiles.rs` le vérifie).\n\n")
    with open(path, "w", encoding="utf-8") as f:
        f.write(head + table("DIGITS", "`\\d` : suites de chiffres décimaux, par dix.", digits) + "\n"
                + table("WORDS", "`\\w` : lettres, chiffres et `_`.", words))


def main(out: str, tables: str | None = None) -> None:
    cases: dict = {"profiles": {k: prof(p) for k, p in PROFILES.items()}}
    cases["delay_target"] = []
    for x in (None, -5.0, 0.0, 1.0, 1.999, 2.0, 2.5, 3.0, 5.0, 7.0, 8.0, 10.0, 14.0, 20.0, float("nan"), float("inf")):
        r, err = call(filter_delay_target_ms, x)
        cases["delay_target"].append({"inch": None if x is None else hx(x), "error": err,
                                      "out": None if r is None else [hx(r[0]), hx(r[1])]})
    keys = []
    for hz in (0.0, 50.0, 114.99, 115.0, 115.01, 159.999, 160.0, 160.001, 199.999, 200.0, 200.001, 289.999, 290.0, 290.001,
               344.999, 345.0, 345.001, 900.0, -1.0, float("nan"), float("inf")):
        for cells in (None, 0, 1, 2, 3, 4, 5, 6, 7):
            r, err = call(profile_key, hz, cells)
            keys.append({"hz": hx(hz), "cells": cells, "error": err, "out": r})
    cases["profile_key"] = keys
    detect = []
    for hz in (None, 115.0, 161.0, 205.0, 290.0, 347.0, 0.0, 1234.5, 99.5, 160.4999, 160.5):
        for cells in (None, 0, 1, 4, 6):
            for name in ("", "Whoop", "5in"):
                detect.append((hz, cells, name))
    for name in NAMES:
        for cells in (None, 1, 6):
            detect.append((None, cells, name))
        detect.append((200.0, 4, name))
    out_detect = []
    for hz, cells, name in detect:
        log = SimpleNamespace(cells=cells, settings=SimpleNamespace(craft_name=name))
        old = profiles.hover_motor_hz
        profiles.hover_motor_hz = lambda _lg, hz=hz: hz
        try:
            r, err = call(detect_profile, log)
        finally:
            profiles.hover_motor_hz = old
        out_detect.append({"hz": None if hz is None else hx(hz), "cells": cells, "name": name, "error": err,
                           "out": None if r is None else {"key": r[0].key, "reason": r[1]}})
    cases["detect"] = out_detect
    cases["digits"] = ranges([c for c in range(0x110000) if re.match(r"\d", chr(c))])
    cases["spaces"] = [c for c in range(0x110000) if re.match(r"\s", chr(c))]
    cases["words"] = ranges([c for c in range(0x110000) if re.match(r"\w", chr(c))])
    cases["lower"] = {str(c): chr(c).lower() for c in range(0x110000) if chr(c).lower() != chr(c) and not 0x41 <= c <= 0x5A}
    if tables:
        rust_tables(tables, cases["digits"], cases["words"])
    with open(out, "w", encoding="utf-8") as f:
        json.dump(cases, f, ensure_ascii=True, separators=(",", ":"))
    print(f"{len(keys)} profile_key, {len(out_detect)} detect, {len(cases['lower'])} minuscules -> {out}")


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    main(*sys.argv[1:])
