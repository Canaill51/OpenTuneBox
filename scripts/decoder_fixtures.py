#!/usr/bin/env python3
"""Fichiers Blackbox de test du décodeur Rust (D-035, phase 4a-3), avec les empreintes que rend le décodeur d'origine.

Usage : OTB_BACKEND=python decoder_fixtures.py DOSSIER

Écrit `DOSSIER/NOM.bbl` et `DOSSIER/expected.json` (`{nom: [session, …]}`, mêmes empreintes que `golden.py` : SHA-256 des
tableaux `main main_kind slow gps gps_at events`, noms de champs, statistiques ; sans l'empreinte d'en-tête, propre à
Python, couverte par `golden.py record-header`). Les cas reprennent `tests/test_bbl_decode.py` (fabrique `tests/bbl_factory.py`),
plus des copies abîmées d'un vrai log (octets changés, bits inversés, octets retirés ou insérés, fichier coupé ; graine
fixe). `OTB_BACKEND=python` impose le décodeur numba (l'étalon) : sans lui, ce serait le Rust qui s'attesterait lui-même.
"""
import json
import os
import random
import sys
from pathlib import Path

os.environ["OTB_BACKEND"] = "python"
HERE = Path(__file__).resolve().parent
sys.path[:0] = [str(HERE), str(HERE.parent / "tests")]

import bbl_factory as bf  # noqa: E402
from golden import decode_digest  # noqa: E402

FIELDS = bf.gyro_fields()
REAL = HERE.parent / "tests" / "data" / "jeno5_5s.bbl"
EVENT_LOGGING_RESUME, EVENT_FLIGHT_MODE = 14, 30


def scenarios() -> dict[str, bytes]:
    out = {}
    out["aller_retour"] = bf.session(FIELDS, bf.rows_2khz(100, gyro=1000))
    for p_num, p_denom, txt in [(1, 2, "2"), (1, 2, "1/2"), (1, 4, "1/4"), (3, 4, "3/4"), (2, 3, "2/3")]:
        data = bf.session(FIELDS, bf.rows_2khz(64, p_num=p_num, p_denom=p_denom), p_num=p_num, p_denom=p_denom)
        out[f"p_interval_{txt.replace('/', 'sur')}"] = data.replace(
            f"H P interval:{p_num}/{p_denom}".encode(), f"H P interval:{txt}".encode(), 1)
    w = bf.Writer(FIELDS)
    body = bytearray(bf.header_bytes(FIELDS))
    for row in bf.rows_2khz(64):
        if row[0] == 20:
            body += b"\x00\x00\x00"
        body += w.iframe(row) if row[0] % 32 == 0 else w.pframe(row)
    out["trame_abimee"] = bytes(body + bf.event_log_end())
    big_fields = bf.gyro_fields([f"x[{i}]" for i in range(60)])
    wb = bf.Writer(big_fields)
    out["trame_de_plus_de_256_octets"] = (bf.header_bytes(big_fields) + wb.iframe([0, 1_000_000] + [0] * 3 + [2**30] * 60)
                                          + wb.iframe([0, 1_000_000] + [0] * 63) + bf.event_log_end())
    w = bf.Writer(FIELDS)
    out["saut_de_trame_i"] = (bf.header_bytes(FIELDS) + w.iframe([0, 1_000_000, 0, 0, 0]) + w.pframe([1, 1_000_125, 0, 0, 0])
                              + w.iframe([32000, 1_000_000 + 32000 * 125, 0, 0, 0])
                              + w.pframe([32001, 1_000_000 + 32001 * 125, 0, 0, 0]) + bf.event_log_end())
    w = bf.Writer(FIELDS)
    it, tm = 32000, 1_000_000 + 32000 * 125
    out["reprise_de_journalisation"] = (bf.header_bytes(FIELDS) + w.iframe([0, 1_000_000, 0, 0, 0])
                                        + bf.event(EVENT_LOGGING_RESUME, it, tm) + w.iframe([it, tm, 5, 6, 7])
                                        + w.pframe([it + 1, tm + 125, 5, 6, 7]) + bf.event_log_end())
    w = bf.Writer(FIELDS)
    out["evenement_mode_et_faux_log_end"] = (bf.header_bytes(FIELDS) + w.iframe([0, 1_000_000, 0, 0, 0])
                                             + bf.event(EVENT_FLIGHT_MODE, 0b11, 0b1) + w.pframe([1, 1_000_125, 0, 0, 0])
                                             + b"E\xffEnd of lag\x00" + w.pframe([2, 1_000_250, 0, 0, 0]) + bf.event_log_end())
    w = bf.Writer(FIELDS)
    body = b"".join(w.iframe(r) if r[0] % 32 == 0 else w.pframe(r) for r in bf.rows_2khz(10))
    out["derniere_trame_coupee"] = bf.header_bytes(FIELDS) + body[:-2]
    out["trames_lentes_remplies"] = bf.session(FIELDS, bf.rows_2khz(8), slow=["flightModeFlags", "stateFlags"],
                                               slow_rows={0: [1, 0], 5: [3, 4]})
    out["trame_lente_apres_la_premiere_ligne"] = bf.session(FIELDS, bf.rows_2khz(8), slow=["a"], slow_rows={3: [9], 3 + 1: [8]})
    a = bf.session(FIELDS, bf.rows_2khz(5), raw={"Craft name": "A"})
    b = bf.session(FIELDS, bf.rows_2khz(7), raw={"Craft name": "B"})
    out["deux_sessions"] = a + b
    out["session_sans_definition_i_complete"] = bf.session(FIELDS, bf.rows_2khz(5)).replace(
        b"H Field I encoding:", b"H Field I encodage:", 1)
    out["session_vide_puis_session"] = b"junk" + bf.session(FIELDS, []) + bf.session(FIELDS, bf.rows_2khz(6))
    out["firmware_et_reglages"] = bf.session(FIELDS, bf.rows_2khz(40), raw={
        "Firmware revision": "Betaflight 4.5.1 (abc) STM32F7X2", "minthrottle": "1070", "vbatref": "1680",
        "motorOutput": "48,2047"})
    one = bf.header_bytes(FIELDS[:1])  # un seul champ I : l'itération sans le temps est écartée (D-031)
    out["un_seul_champ_i"] = one + bf.Writer(FIELDS[:1]).iframe([0]) + bf.event_log_end()
    # trames GPS : home (H) avec les champs dans l'ordre inverse (indices GPS_home[1] = 1, GPS_home[0] = 2 : aucun n'est 0), puis G
    # dont les deux coordonnées sont prédites par HOME_COORD
    w = bf.Writer(FIELDS)
    gps_head = (b"H Field H name:pad,GPS_home[1],GPS_home[0]\nH Field H signed:1,1,1\nH Field H predictor:0,0,0\n"
                b"H Field H encoding:0,0,0\nH Field G name:GPS_time,GPS_numSat,GPS_coord[0],GPS_coord[1]\n"
                b"H Field G signed:0,0,1,1\nH Field G predictor:0,0,7,7\nH Field G encoding:1,1,0,0\n")
    rows = bf.rows_2khz(6)
    out["gps_et_maison"] = (bf.header_bytes(FIELDS) + gps_head + w.iframe(rows[0])
                            + b"H" + bf.enc_svb(5) + bf.enc_svb(-120000) + bf.enc_svb(450000) + b"G" + bf.enc_uvb(1000) + bf.enc_uvb(7)
                            + bf.enc_svb(15) + bf.enc_svb(-4) + w.pframe(rows[1]) + b"G" + bf.enc_uvb(40) + bf.enc_uvb(8)
                            + bf.enc_svb(-3) + bf.enc_svb(9) + w.pframe(rows[2]) + bf.event_log_end())
    out["aucune_trame"] = bf.header_bytes(FIELDS)
    out["fichier_sans_session"] = b"pas un fichier blackbox"
    return out


def damaged() -> dict[str, bytes]:
    real = REAL.read_bytes()
    start = real.find(b"\nI", real.find(b"H Product")) + 1  # première trame I après l'en-tête
    head, body = real[:start], real[start:start + 30_000]
    rng = random.Random(20261030)
    out = {"vrai_log_30ko": head + body}
    for k in range(10):
        b = bytearray(body)
        kind = k % 5
        for _ in range(rng.randint(1, 4)):
            i = rng.randrange(len(b))
            if kind == 0:
                b[i] = rng.randrange(256)  # octet changé
            elif kind == 1:
                b[i] ^= 1 << rng.randrange(8)  # bit inversé
            elif kind == 2:
                del b[i:i + rng.randint(1, 20)]  # octets retirés
            elif kind == 3:
                b[i:i] = bytes(rng.randrange(256) for _ in range(rng.randint(1, 20)))  # octets insérés
            else:
                del b[i:]  # fichier coupé
                break
        out[f"vrai_log_abime_{k}"] = head + bytes(b)
    return out


def main():
    target = Path(sys.argv[1])
    target.mkdir(parents=True, exist_ok=True)
    expected = {}
    for name, data in {**scenarios(), **damaged()}.items():
        path = target / f"{name}.bbl"
        path.write_bytes(data)
        digests = decode_digest(str(path))
        for d in digests:
            d.pop("header")
        expected[name] = digests
    # le vrai log entier, resté dans tests/data/ (non recopié)
    expected["@tests/data/jeno5_5s.bbl"] = [{k: v for k, v in d.items() if k != "header"} for d in decode_digest(str(REAL))]
    json.dump(expected, open(target / "expected.json", "w", encoding="utf-8"), indent=0, sort_keys=True)
    print(len(expected), "fichiers,", sum(len(v) for v in expected.values()), "sessions décodées")


if __name__ == "__main__":
    main()
