#!/usr/bin/env python3
"""Cas de test des chaînes de filtres de `analysis/bf_filters.py` pour `rust/otb-analysis/src/bf_filters.rs` (D-035, 4b-3).

Usage : bf_filters_cases.py SORTIE.json

Couvre `coefficients`, `freq_response`, `chain_response`, `chain_delay_ms`, `dyn_lpf_cutoff`, `scale_lowpass`,
`lowpass_off`, `rpm_notch_freq_weight`, `dyn_notch_enabled`, `dyn_notch_initial_centres`, `gyro_chain` et `dterm_chain`
(la simulation temporelle et `whatif_spectrum` viennent en 4b-4). Réglages tirés au hasard (graine fixe) et réglages à la
main sur les cas limites : notch statique au-dessus de Nyquist ou à coupure ≥ centre, filtre à 1 Hz ou 1000 Hz, poids RPM
absents, cadence PID autour de 2 kHz, `looptime_us` ou `pid_denom` nuls (Python lève une division par zéro).

Un flottant s'écrit en hexadécimal (16 caractères, bits du float64) ; `"error": true` = Python a levé une exception. Le
test `rust/otb-analysis/tests/bf_filters.rs` exige des chaînes identiques au bit près (arithmétique pure, sans libm) et
des réponses, coefficients et retards à la tolérance mesurée (libm, BLAS, atan2 : L-048).
"""
import json
import struct
import sys

import numpy as np

from opentunebox.analysis import bf_filters as bf
from opentunebox.log.settings import LowpassSetting, Settings


def hx(x) -> str:
    return struct.pack("<d", float(x)).hex()


def hexs(a) -> str:
    return np.ascontiguousarray(a, dtype="<f8").tobytes().hex()


def call(fn, *args, **kw):
    """(résultat, False) ou (None, True) si Python lève une exception."""
    try:
        return fn(*args, **kw), False
    except Exception:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        return None, True


INPUTS: dict[str, str] = {}
_IDS: dict[str, str] = {}


def ref(a) -> str:
    """Identifiant d'une entrée dans la table `inputs` du fichier : une entrée identique n'y est écrite qu'une fois."""
    h = hexs(a)
    if h not in _IDS:
        _IDS[h] = f"i{len(_IDS)}"
        INPUTS[_IDS[h]] = h
    return _IDS[h]


def enc_spec(sp) -> dict:
    return {"k": sp.kind, "f": hx(sp.f_hz), "q": hx(sp.q), "w": hx(sp.weight)}


def enc_chain(chain) -> list:
    return [[enc_spec(sp), hx(rate)] for sp, rate in chain]


def enc_lp(lp) -> dict:
    return {"type": lp.type, "static_hz": lp.static_hz, "dyn_min_hz": lp.dyn_min_hz, "dyn_max_hz": lp.dyn_max_hz,
            "expo": lp.expo}


def enc_settings(s: Settings) -> dict:
    """Seuls les champs que lisent les chaînes de filtres."""
    return {"looptime_us": s.looptime_us, "pid_denom": s.pid_denom, "gyro_lpf1": enc_lp(s.gyro_lpf1),
            "gyro_lpf2": enc_lp(s.gyro_lpf2), "dterm_lpf1": enc_lp(s.dterm_lpf1), "dterm_lpf2": enc_lp(s.dterm_lpf2),
            "gyro_notches": [list(n) for n in s.gyro_notches], "dterm_notch": list(s.dterm_notch),
            "dyn_notch_count": s.dyn_notch_count, "dyn_notch_q": s.dyn_notch_q, "dyn_notch_min_hz": s.dyn_notch_min_hz,
            "dyn_notch_max_hz": s.dyn_notch_max_hz, "rpm_harmonics": s.rpm_harmonics, "rpm_weights": list(s.rpm_weights),
            "rpm_q": s.rpm_q, "rpm_min_hz": s.rpm_min_hz, "rpm_fade_hz": s.rpm_fade_hz}


def lp(kind="PT1", st=0, mn=0, mx=0, ex=5) -> LowpassSetting:
    return LowpassSetting(kind, st, mn, mx, ex)


LOWPASSES = [lp(), lp("PT1", 150), lp("BIQUAD", 200), lp("PT2", 90), lp("PT3", 120), lp("PT1", 0, 75, 150, 5),
             lp("PT2", 0, 200, 500, 0), lp("BIQUAD", 0, 100, 300, 3), lp("PT1", 0, 300, 200, 5),  # min > max
             lp("PT1", 1000), lp("PT1", 1), lp("PT1", 0, 1, 1, 5), lp("PT3", 0, 50, 1000, 10)]
NOTCHES = [[], [(250, 200)], [(300, 250), (450, 400), (600, 500)], [(5000, 100)], [(400, 500)], [(100, 0)], [(-5, 3)],
           [(0, 0), (260, 160)]]


def random_settings(rng, n):
    out = []
    for _ in range(n):
        pick = lambda seq: seq[int(rng.integers(len(seq)))]  # noqa: E731
        out.append(Settings(
            raw={}, looptime_us=int(pick([125, 250, 500, 1000, 62, 31])), pid_denom=int(pick([1, 2, 4, 8, 3])),
            gyro_lpf1=pick(LOWPASSES), gyro_lpf2=pick(LOWPASSES), dterm_lpf1=pick(LOWPASSES), dterm_lpf2=pick(LOWPASSES),
            gyro_notches=list(pick(NOTCHES)), dterm_notch=tuple(pick([(0, 0), (260, 160), (400, 300), (100, 100)])),
            dyn_notch_count=int(pick([0, 1, 3, 5, 7])), dyn_notch_q=int(pick([0, 250, 500, 800])),
            dyn_notch_min_hz=int(pick([0, 100, 150])), dyn_notch_max_hz=int(pick([0, 600, 800])),
            rpm_harmonics=int(pick([0, 1, 3])), rpm_weights=list(pick([[], [100], [100, 50, 25], [0, 100], [250, -10]])),
            rpm_q=int(pick([0, 300, 500])), rpm_min_hz=int(pick([100, 80, 5000, 0])),
            rpm_fade_hz=int(pick([0, 50, 100000]))))
    return out


def main():
    rng = np.random.default_rng(20261101)
    hand = [
        Settings(raw={}),  # valeurs par défaut du dataclass
        Settings(raw={}, looptime_us=0), Settings(raw={}, pid_denom=0),
        Settings(raw={}, looptime_us=125, pid_denom=4, dyn_notch_count=3, dyn_notch_q=500, dyn_notch_min_hz=100,
                 dyn_notch_max_hz=600),  # cadence PID = 2000 Hz exactement : seuil du notch dynamique
        Settings(raw={}, looptime_us=125, pid_denom=5, dyn_notch_count=3, dyn_notch_q=500),  # 1600 Hz : désactivé
        Settings(raw={}, looptime_us=125, pid_denom=2, dyn_notch_count=7, dyn_notch_q=300, dyn_notch_min_hz=90,
                 dyn_notch_max_hz=620, rpm_harmonics=3, rpm_weights=[100, 50, 25], rpm_q=300, rpm_min_hz=90,
                 rpm_fade_hz=40, gyro_lpf1=lp("PT1", 0, 75, 160, 5), gyro_lpf2=lp("PT2", 400),
                 gyro_notches=[(250, 200), (300, 270)], dterm_lpf1=lp("PT1", 0, 60, 120, 7), dterm_lpf2=lp("PT3", 80),
                 dterm_notch=(260, 190)),
    ]
    all_settings = hand + random_settings(rng, 70)
    sidx = {i: enc_settings(s) for i, s in enumerate(all_settings)}
    out = {"settings": [sidx[i] for i in range(len(all_settings))]}
    for k in ("coefficients", "chain_response", "chain_delay_ms", "dyn_lpf_cutoff", "scale_lowpass", "lowpass_off",
              "rpm_notch_freq_weight", "dyn_notch_enabled", "dyn_notch_initial_centres", "complete_dyn_centres", "gyro_chain", "dterm_chain"):
        out[k] = []

    # ---- coefficients : chaque type, cadences, Q et poids ; centres aux limites
    kinds = bf.FILTER_KINDS
    freqs_hz = (0.0, 1.0, 37.5, 100.0, 250.0, 500.0, 999.9, 1500.0, 3999.0)
    for kind in kinds:
        for f in freqs_hz:
            for q in (bf.BUTTERWORTH_Q, 0.3, 1.0, 5.0):
                for w in (1.0, 0.5, 0.0):
                    for fs in (1000.0, 2000.0, 8000.0, 3200.0):
                        if rng.random() < 0.18 or (f in (0.0, 3999.0)):
                            spec = bf.FilterSpec(kind, f, q, w)
                            res, err = call(bf.coefficients, spec, fs)
                            out["coefficients"].append({
                                "spec": enc_spec(spec), "fs": hx(fs), "error": err,
                                "b": None if err else hexs(res[0]), "a": None if err else hexs(res[1])})

    # ---- chain_response / freq_response : chaînes réelles (gyro et D) et listes à cadence unique
    def freq_sets(fs):
        ny = fs / 2.0
        return [np.linspace(1.0, ny, 40), np.array([0.0, ny, ny * 1.5, 1e5]), np.array([]), np.linspace(1.0, ny, 12).reshape(3, 4)]

    for i in range(len(all_settings)):
        s = all_settings[i]
        for thr in (0.2, 0.9):
            for chain_fn, name in ((lambda s=s, thr=thr: bf.gyro_chain(s, thr, motor_hz=(180.0, 210.0, 195.5, 160.0)), "gyro"),
                                   (lambda s=s, thr=thr: bf.dterm_chain(s, thr), "dterm")):
                chain, err = call(chain_fn)
                if err:
                    continue
                for fr in freq_sets(float(s.pid_rate_hz))[: 2 if i % 3 else 4]:
                    res, err = call(bf.chain_response, chain, fr)
                    out["chain_response"].append({"chain": enc_chain(chain), "freqs": ref(fr), "shape": list(fr.shape),
                                                  "error": err, "out": None if err else hexs(res.view(np.float64))})
    # freq_response (liste de FilterSpec à cadence unique) : codé comme une chaîne de même cadence
    for fs in (1000.0, 4000.0):
        specs = [bf.FilterSpec("notch", 120.0, 4.0), bf.FilterSpec("pt2", 90.0), bf.FilterSpec("biquad_lpf", 200.0, 0.8),
                 bf.FilterSpec("notch", 300.0, 5.0, 0.4), bf.FilterSpec("pt3", 150.0), bf.FilterSpec("pt1", 70.0)]
        fr = np.linspace(1.0, fs / 2, 60)
        res, err = call(bf.freq_response, specs, fr, fs)
        out["chain_response"].append({"chain": enc_chain([(sp, fs) for sp in specs]), "freqs": ref(fr), "shape": [60],
                                      "error": err, "out": hexs(res.view(np.float64))})

    # ---- chain_delay_ms : listes de FilterSpec (avec fs) et chaînes ; fréquences de mesure variées
    for i in range(0, len(all_settings), 2):
        s = all_settings[i]
        chain, err = call(bf.gyro_chain, s, 0.5, motor_hz=(200.0, 200.0, 190.0, 205.0))
        if err:
            continue
        for f_hz in (10.0, 5.0, 50.0, 100.0, 0.0, 400.0):
            res, err = call(bf.chain_delay_ms, chain, None, f_hz)
            out["chain_delay_ms"].append({"chain": enc_chain(chain), "f_hz": hx(f_hz), "error": err,
                                          "out": None if err else [hx(res.group_ms), hx(res.phase_ms)]})
    out["chain_delay_ms"].append({"chain": [], "f_hz": hx(10.0), "error": False,
                                  "out": [hx(v) for v in bf.chain_delay_ms([], 1000.0, 10.0)]})
    out["chain_delay_ms"].append({"chain": enc_chain([(bf.FilterSpec("pt1", 150.0), 4000.0)]), "f_hz": hx(1900.0),
                                  "error": False, "out": [hx(v) for v in bf.chain_delay_ms([(bf.FilterSpec("pt1", 150.0), 4000.0)],
                                                                                       None, 1900.0)]})  # saut de phase
    _, err = call(bf.chain_delay_ms, [bf.FilterSpec("pt1", 150.0)], None, 10.0)  # FilterSpec sans cadence : ValueError
    out["chain_delay_ms"].append({"chain_no_rate": [enc_spec(bf.FilterSpec("pt1", 150.0))], "f_hz": hx(10.0),
                                  "error": err, "out": None})

    # ---- dyn_lpf_cutoff : gaz au centième, demi-centièmes (arrondi au pair), hors bornes, NaN
    thr = [0.0, 0.001, 0.005, 0.125, 0.135, 0.145, 0.2, 0.5, 0.505, 0.995, 1.0, 1.7, -0.4, float("nan"), float("inf")]
    for mn, mx, ex in ((75, 150, 5), (75, 150, 0), (200, 500, 10), (100, 100, 5), (300, 200, 3), (0, 0, 5), (1, 1000, 1)):
        for t in thr:
            res, err = call(bf.dyn_lpf_cutoff, mn, mx, ex, t)
            out["dyn_lpf_cutoff"].append({"min": mn, "max": mx, "expo": ex, "t": hx(t), "arr": False, "error": err,
                                          "out": None if err else hx(res)})
        arr = np.array(thr)
        res, err = call(bf.dyn_lpf_cutoff, mn, mx, ex, arr)
        out["dyn_lpf_cutoff"].append({"min": mn, "max": mx, "expo": ex, "t": ref(arr), "arr": True, "error": err,
                                      "out": None if err else hexs(res)})

    # ---- scale_lowpass / lowpass_off
    factors = [0.0, 0.5, 1.0, 1.5, 2.0, 0.333, 1.005, 10.0, -1.0, 1e-9, float("nan"), float("inf")]
    for l_ in LOWPASSES:
        for fct in factors:
            res, err = call(bf.scale_lowpass, l_, fct)
            out["scale_lowpass"].append({"lp": enc_lp(l_), "factor": hx(fct), "error": err,
                                         "out": None if err else enc_lp(res)})
        out["lowpass_off"].append({"lp": enc_lp(l_), "out": enc_lp(bf.lowpass_off(l_))})
    for v in (1, 3, 7, 201, 333, 999):  # v × 0,5 = x,5 : arrondi au pair
        l_ = lp("PT1", v, v, v)
        for fct in (0.5, 1.5, 2.5):
            res, err = call(bf.scale_lowpass, l_, fct)
            out["scale_lowpass"].append({"lp": enc_lp(l_), "factor": hx(fct), "error": err, "out": enc_lp(res)})

    # ---- rpm_notch_freq_weight
    motors = [np.array(5.0), np.array([100.0, 250.5, 400.0, 0.0, -3.0, float("nan"), 1e5]),
              (rng.uniform(80, 700, (4, 12))), (rng.uniform(80, 700, (4, 12))).astype(np.float32), np.zeros((4, 3))]
    for i, s in enumerate(all_settings):
        for h in (0, 1, 2, 4):
            for mi, m in enumerate(motors):
                if (i + h + mi) % 3 and i > 5:
                    continue
                rate = float(s.pid_rate_hz) if (i % 2) else 4000.0
                res, err = call(bf.rpm_notch_freq_weight, m, h, s, rate)
                out["rpm_notch_freq_weight"].append({
                    "motor": ref(m.astype(np.float64)), "shape": list(m.shape), "harmonic": h, "s": i, "rate": hx(rate),
                    "error": err, "f": None if err else hexs(res[0]), "w": None if err else hexs(res[1])})

    # ---- dyn_notch_enabled / dyn_notch_initial_centres
    for i, s in enumerate(all_settings):
        out["dyn_notch_enabled"].append({"s": i, "out": bool(bf.dyn_notch_enabled(s))})
        out["dyn_notch_initial_centres"].append({"s": i, "out": [hx(v) for v in bf.dyn_notch_initial_centres(s)]})

    # ---- complete_dyn_centres (D-041) : centres enregistrés hors plage, pics trop proches (tolérance 27,75 Hz exactement), hors
    # plage, en double, plus de pics que de notchs libres, nombre de notchs nul ou négatif
    hand_known = [[], [200.0], [50.0, 700.0], [200.0, 300.0, 400.0, 500.0], [183.25]]
    hand_peaks = [[], [407.0], [210.0, 407.0], [650.0, 90.0, 407.0], [300.0, 407.0, 480.0], [227.75, 228.0, 139.0],
                  [183.25 + 27.75, 183.25 - 27.75, 183.25 + 27.76], [350.0, 350.0, 350.0], [100.0, 600.0, 99.9, 600.1]]
    crng = np.random.default_rng(20261103)  # propre à ce bloc : ne décale pas les tirages des cas suivants
    for i, s in enumerate(all_settings):
        cases = [(k, pk) for k in hand_known[:3] for pk in hand_peaks[:5]]
        cases += [(hand_known[int(crng.integers(len(hand_known)))], hand_peaks[int(crng.integers(len(hand_peaks)))]) for _ in range(6)]
        for known, peaks in cases:
            out["complete_dyn_centres"].append({"s": i, "known": [hx(v) for v in known], "peaks": [hx(v) for v in peaks],
                                                "out": [hx(v) for v in bf.complete_dyn_centres(list(known), s, list(peaks))]})
    for s in (Settings(raw={}, dyn_notch_count=3, dyn_notch_min_hz=100, dyn_notch_max_hz=600),
              Settings(raw={}, dyn_notch_count=-1, dyn_notch_min_hz=100, dyn_notch_max_hz=600),
              Settings(raw={}, dyn_notch_count=2, dyn_notch_min_hz=600, dyn_notch_max_hz=100)):  # plage inversée
        out["settings"].append(enc_settings(s))
        si = len(out["settings"]) - 1
        for known, peaks in ((hand_known[1], hand_peaks[2]), (hand_known[3], hand_peaks[4]), (hand_known[2], hand_peaks[3])):
            out["complete_dyn_centres"].append({"s": si, "known": [hx(v) for v in known], "peaks": [hx(v) for v in peaks],
                                                "out": [hx(v) for v in bf.complete_dyn_centres(list(known), s, list(peaks))]})

    # ---- gyro_chain / dterm_chain
    motor_opts = [None, (180.0, 210.0, 195.5, 160.0), [300.0], 250.0, (0.0, 0.0), (float("nan"), 220.0),
                  (50.0, 4000.0, 1000.0)]
    dyn_opts = [None, [], [120.0, 340.0], [0.0, 150.0, -5.0], [float("nan"), 200.0], [100.0] * 9]
    for i, s in enumerate(all_settings):
        for t in (0.5, 0.0, 0.123, 0.995, 1.0, 1.5, -0.2, float("nan")):
            if i > 5 and rng.random() > 0.35:
                continue
            for mi, m in enumerate(motor_opts):
                di = int(rng.integers(len(dyn_opts)))
                d = dyn_opts[di]
                res, err = call(bf.gyro_chain, s, t, m, d)
                out["gyro_chain"].append({"s": i, "t": hx(t), "motor": None if m is None else [hx(v) for v in np.atleast_1d(m)],
                                          "motor_scalar": isinstance(m, float), "dyn": None if d is None else [hx(v) for v in d],
                                          "error": err, "out": None if err else enc_chain(res)})
            res, err = call(bf.dterm_chain, s, t)
            out["dterm_chain"].append({"s": i, "t": hx(t), "error": err, "out": None if err else enc_chain(res)})

    out["inputs"] = INPUTS
    json.dump(out, open(sys.argv[1], "w", encoding="utf-8"), separators=(",", ":"))
    print({k: len(v) for k, v in out.items() if k not in ("inputs",)}, len(INPUTS), "entrées distinctes")


if __name__ == "__main__":
    main()
