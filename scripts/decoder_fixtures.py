#!/usr/bin/env python3
"""Fichiers Blackbox de test du décodeur Rust (D-035, phase 4a-3), avec les empreintes que rend le décodeur d'origine.

Usage : OTB_BACKEND=python decoder_fixtures.py DOSSIER

Écrit `DOSSIER/NOM.bbl` et `DOSSIER/expected.json` (`{nom: [session, …]}`, mêmes empreintes que `golden.py` : SHA-256 des
tableaux `main main_kind slow gps gps_at events`, noms de champs, statistiques ; sans l'empreinte d'en-tête, propre à
Python, couverte par `golden.py record-header`). Les cas reprennent `tests/test_bbl_decode.py` (fabrique `tests/bbl_factory.py`),
plus des copies abîmées d'un vrai log (octets changés, bits inversés, octets retirés ou insérés, fichier coupé ; graine
fixe). `OTB_BACKEND=python` impose le décodeur numba (l'étalon) : sans lui, ce serait le Rust qui s'attesterait lui-même.

Chaque entrée de `expected.json` est `{"sessions": [empreintes du décodeur], "flightlog": [enregistrements de
`golden_flightlog.flightlog_record`]}` : les mêmes fichiers servent à prouver `decode_file` (4a-3) et `FlightLog` (4a-5, phase
de `from_session`). Les scénarios « fl_… » existent pour `FlightLog` : haute résolution, DShot ou PWM, eRPM, repli du debug,
chirp, voies absentes, trames aberrantes, entiers 32 bits, trous, noms de champs répétés.
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
from golden_flightlog import flightlog_file  # noqa: E402

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


FULL = (["setpoint[0]", "setpoint[1]", "setpoint[2]", "setpoint[3]", "rcCommand[0]", "rcCommand[1]", "rcCommand[2]",
         "rcCommand[3]", "axisP[0]", "axisP[1]", "axisP[2]", "axisI[0]", "axisI[1]", "axisI[2]", "axisD[0]", "axisD[1]",
         "axisD[2]", "axisF[0]", "axisF[1]", "axisF[2]", "motor[0]", "motor[1]", "motor[2]", "motor[3]", "eRPM[0]", "eRPM[1]",
         "eRPM[2]", "eRPM[3]", "gyroUnfilt[0]", "gyroUnfilt[1]", "gyroUnfilt[2]", "debug[0]", "debug[1]", "debug[2]",
         "debug[3]", "vbatLatest", "amperageLatest", "accSmooth[0]", "accSmooth[1]", "accSmooth[2]"])
BASE_RAW = {"Firmware revision": "Betaflight 4.5.1 (abc) STM32F7X2", "motor_poles": "14", "motor_pwm_protocol": "6",
            "acc_1G": "2048", "vbatref": "1680", "rollPID": "45,80,40", "pitchPID": "47,84,46", "yawPID": "45,80,0",
            "ff_weight": "120,125,120", "debug_mode": "0"}


def fl_session(extra, n=300, raw=None, gain=1, slow=(), slow_rows=None, tweak=None, dt=125, seed=1):
    """Session aux valeurs déterministes pour tous les champs de `gyro_fields(extra)` ; `tweak(k, name, v)` les retouche."""
    fields = bf.gyro_fields(extra)
    rng = random.Random(seed)
    rows = []
    for k in range(n):
        row = [k, 1_000_000 + k * dt]
        for i, f in enumerate(fields[2:]):
            v = int(80 * gain * __import__("math").sin(k / 9 + i)) + rng.randint(-3, 3)
            if f.name.startswith(("motor[", "eRPM[")):
                v = abs(v) * 10 + 100
            if tweak:
                v = tweak(k, f.name, v)
            row.append(v)
        rows.append(row)
    return bf.session(fields, rows, raw={**BASE_RAW, **(raw or {})}, slow=list(slow), slow_rows=slow_rows)


def flightlog_scenarios() -> dict[str, bytes]:
    out = {}
    flags = {0: [0, 0], 40: [1, 1], 120: [3, 1], 200: [7, 1], 260: [1, 1]}
    out["fl_complet"] = fl_session(FULL, slow=["flightModeFlags", "stateFlags"], slow_rows=flags)
    out["fl_haute_resolution"] = fl_session(FULL, raw={"blackbox_high_resolution": "1"}, gain=10)
    out["fl_pwm"] = fl_session(FULL, raw={"motor_pwm_protocol": "0", "motorOutput": "1000,2000"})
    out["fl_pwm_sortie_haute"] = fl_session(FULL, raw={"motor_pwm_protocol": "0", "motorOutput": "1000,2500"})
    gyro_only = ["debug[0]", "debug[1]", "debug[2]"]
    out["fl_gyro_scaled"] = fl_session(gyro_only, raw={"Firmware revision": "Betaflight 4.4.3 (abc) STM32F7X2",
                                                       "debug_mode": "6"})
    out["fl_dshot_telemetrie_debug"] = fl_session(["motor[0]", "motor[1]", "motor[2]", "motor[3]", "debug[0]", "debug[1]",
                                                   "debug[2]", "debug[3]"], raw={"debug_mode_name": "DSHOT_RPM_TELEMETRY"})
    chirp = ["debug[0]", "debug[1]", "debug[2]", "debug[3]", "rcCommand[0]", "rcCommand[1]", "rcCommand[2]", "rcCommand[3]"]
    out["fl_chirp_en_mode_angle"] = fl_session(chirp, raw={"debug_mode_name": "CHIRP", "chirp_amplitude_roll": "50",
                                                           "chirp_amplitude_pitch": "20.5"},
                                               slow=["flightModeFlags"], slow_rows={0: [3]},
                                               tweak=lambda k, n, v: (k // 20) % 4 - 1 if n == "debug[1]" else v)
    out["fl_sans_gaz_du_mixeur"] = fl_session(["setpoint[0]", "setpoint[1]", "setpoint[2]", "rcCommand[0]", "rcCommand[1]",
                                               "rcCommand[2]", "rcCommand[3]"], tweak=lambda k, n, v: 1500 + v if n == "rcCommand[3]" else v)
    out["fl_gaz_de_repli_hors_bornes"] = fl_session(
        ["rcCommand[0]", "rcCommand[1]", "rcCommand[2]", "rcCommand[3]"],
        tweak=lambda k, n, v: (900, 2100, 1500, 1995, 1004)[k % 5] if n == "rcCommand[3]" else v)
    # sessions plus courtes que le noyau de lissage de `d_term_prefilter` (133 points à 8 kHz) : `np.convolve` échange alors
    # ses deux arguments et rend `max(n, w)` valeurs (relecture indépendante de la phase 4a)
    for n in (2, 3, 4, 12, 60, 132, 133, 140):
        out[f"fl_courte_avec_terme_d_{n}"] = fl_session(["axisD[0]", "axisD[1]", "axisD[2]"], n=n)
    out["fl_voies_absentes"] = fl_session([])
    out["fl_un_axe_pid"] = fl_session(["axisP[1]", "axisD[0]", "axisD[1]", "axisF[2]"])
    out["fl_une_voie_de_chaque_groupe"] = fl_session(["setpoint[2]", "rcCommand[1]", "debug[2]", "motor[0]", "accSmooth[1]",
                                                      "gyroUnfilt[2]", "eRPM[0]"])
    out["fl_erpm_incomplet"] = fl_session(["motor[0]", "motor[1]", "motor[2]", "eRPM[0]", "eRPM[1]"])
    out["fl_aberrant_minoritaire"] = fl_session(FULL[:12], tweak=lambda k, n, v: 5000 if n == "gyroUnfilt[0]" or (
        k % 7 == 0 and n == "setpoint[1]") else v)
    out["fl_aberrant_avec_trames_lentes"] = fl_session(
        ["setpoint[0]", "setpoint[1]", "setpoint[2]", "setpoint[3]"], slow=["flightModeFlags", "stateFlags"],
        slow_rows={0: [1, 0], 30: [3, 1], 31: [5, 1], 90: [2, 2], 200: [7, 3]},
        tweak=lambda k, n, v: 5000 if n == "setpoint[0]" and k in (10, 31, 32, 150) else v)
    out["fl_aberrant_pid"] = fl_session(FULL[:20], tweak=lambda k, n, v: 70000 if k % 11 == 0 and n == "axisP[0]" else v)
    out["fl_corrompu_majoritaire"] = fl_session(FULL[:4], tweak=lambda k, n, v: 9000 if n == "setpoint[0]" and k % 3 else v)
    out["fl_moitie_exacte"] = fl_session(FULL[:4], n=100, tweak=lambda k, n, v: 9000 if n == "setpoint[0]" and k % 2 else v)
    out["fl_entiers_32_bits"] = fl_session(["motor[0]", "eRPM[0]", "debug[0]", "accSmooth[0]"],
                                           tweak=lambda k, n, v: (2**31 + 5 + v if k % 4 else -(2**31) - 9 + v) if n == "debug[0]" else v)
    out["fl_trou_denregistrement"] = fl_session(FULL[:8], n=260, tweak=None).replace(b"", b"")
    gap = bf.session(bf.gyro_fields(FULL[:8]), [[k, 1_000_000 + k * 125 + (60_000 if k >= 120 else 0), *([k % 17] * 3), 1, 2, 3, 4,
                                                  5, 6, 7, 8] for k in range(260)], raw=BASE_RAW)
    out["fl_trou_denregistrement"] = gap
    out["fl_une_ligne"] = fl_session(FULL[:4], n=1)
    out["fl_deux_lignes_meme_temps"] = bf.session(bf.gyro_fields(FULL[:4]), [[0, 1_000_000, 1, 2, 3, 4, 5, 6, 7],
                                                                           [1, 1_000_000, 1, 2, 3, 4, 5, 6, 7],
                                                                           [2, 1_000_125, 1, 2, 3, 4, 5, 6, 7]], raw=BASE_RAW)
    out["fl_noms_repetes"] = fl_session(["gyroADC[0]", "setpoint[0]", "setpoint[0]", "setpoint[1]", "setpoint[2]"])
    out["fl_sans_vbatref_tension_lue"] = fl_session(["vbatLatest", "amperageLatest"], raw={"vbatref": "0"},
                                                    tweak=lambda k, n, v: 1260 + (v // 40 if n == "vbatLatest" else 0) if n == "vbatLatest" else v)
    out["fl_batterie_6s_personnalisee"] = fl_session(["vbatLatest"], raw={"vbatref": "2520", "vbatcellvoltage": "330,350,440"})
    out["fl_mode_horizon"] = fl_session(["rcCommand[0]", "rcCommand[1]", "rcCommand[2]", "rcCommand[3]"],
                                        slow=["flightModeFlags"], slow_rows={0: [1], 10: [5], 250: [1]})
    out["fl_indicateurs_negatifs"] = fl_session(["debug[0]"], slow=["flightModeFlags"], slow_rows={0: [2**32 - 1], 60: [2**31 + 1],
                                                                                           150: [0]})
    out["fl_deux_sessions"] = fl_session(FULL[:12], n=120) + fl_session(FULL[:12], n=150, seed=2)
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
    for name, data in {**scenarios(), **flightlog_scenarios(), **damaged()}.items():
        path = target / f"{name}.bbl"
        path.write_bytes(data)
        digests = decode_digest(str(path))
        for d in digests:
            d.pop("header")
        expected[name] = {"sessions": digests, "flightlog": flightlog_file(str(path))}
    # le vrai log entier, resté dans tests/data/ (non recopié)
    expected["@tests/data/jeno5_5s.bbl"] = {
        "sessions": [{k: v for k, v in d.items() if k != "header"} for d in decode_digest(str(REAL))],
        "flightlog": flightlog_file(str(REAL))}
    json.dump(expected, open(target / "expected.json", "w", encoding="utf-8"), indent=0, sort_keys=True)
    print(len(expected), "fichiers,", sum(len(v["sessions"]) for v in expected.values()), "sessions décodées,",
          sum(len(v["flightlog"]) for v in expected.values()), "FlightLog")


if __name__ == "__main__":
    main()
