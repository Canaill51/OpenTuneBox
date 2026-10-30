#!/usr/bin/env python3
"""Cas de test de `log/settings.py` pour `rust/otb-log` (D-035, phase 4a-4).

Usage : settings_cases.py SORTIE.json

Chaque cas : l'en-tête brut (`raw`) et les `Settings` que Python en tire (`golden_settings.settings_json`), ou `error` si Python
lève une exception (non comparé). Cas écrits à la main (versions limites, D/D max, filtres, RPM, moteurs, curseurs, valeurs
absurdes) puis cas tirés au hasard (graine fixe) dans un vocabulaire de clés et de valeurs ; plus chaque caractère latin-1
comme premier caractère du nom du firmware et comme séparateur (le motif `\\w+\\s+\\d+.\\d+.\\d+`).
"""
import json
import random
import sys

from golden_settings import settings_json

from opentunebox.log.settings import parse_settings

FW = ["Betaflight 4.4.0 (abc) STM32F7X2", "Betaflight 4.4.3 (abc) STM32H743", "Betaflight 4.5.1 (abc) STM32G47X",
      "Betaflight 2025.12.0 (abc) STM32H743", "Betaflight 2026.6.0-beta (abc) AT32F435", "Betaflight 2026.6.2 (def)",
      "EmuFlight 0.4.1 (xyz) STM32F4", "Betaflight 4.3.0 STM32F4", "Betaflight 4.5", "Betaflight 4.5.1 (abc STM32X", "Betaflight 4.5.1 abc) STM32Y", "  Betaflight 4.5.1 (a) B  ", "",
      "Betaflight\t4.5.1\t(a)\tBOARD", "Betaflight\x1c4.5.1 (a) X", "Betaflight\xa04.5.1 (a) X", "Betaflight 4.5.1x (a) X",
      "_x 1.2.3", "9 1.2.3", "²x 1.2.3", "Beta flight 4.5.1", "4.5.1"]
VOCAB = {
    "Firmware revision": FW, "Craft name": ["", "Mon drone", " espace ", "é\xa0"],
    "looptime": ["125", "250", "0", "-5", "x", "100.7", "0x7d", "", " 125 "], "pid_process_denom": ["1", "2", "0", "x", "-1"],
    "P interval": ["1", "2", "1/2", "3/4", "0/0", "x", "", "/"], "debug_mode": ["0", "6", "17", "16", "96", "999", "x", "-1", ""],
    "debug_mode_name": ["", "chirp", " fft_freq ", "GYRO_SCALED", "\x1fchirp\x1f", "\x85gyro_scaled\xa0", "\x1f"], "rollPID": ["45,80,40", "45,80", "45", "x,y", "", "45,80,40,5",
    "0x2d,80,40"], "pitchPID": ["47,84,46", "1,2,3"], "yawPID": ["45,80,0", "30,80,0"],
    "ff_weight": ["120,125,120", "120", "", "x", "0,0,0"], "d_min": ["30,34,0", "0,0,0", "50,50,50", "x", "30"],
    "d_max": ["40,46,0", "30,30,30", "60", "x"], "gyro_lpf1_type": ["0", "1", "2", "3", "4", "-1", "x", ""],
    "gyro_lpf1_static_hz": ["0", "250", "x", "-1"], "gyro_lpf1_dyn_hz": ["0,0", "150,400", "150", "x,200", "", "1,2,3"],
    "gyro_lpf1_dyn_expo": ["5", "0", "x", ""], "gyro_lpf2_type": ["0", "2"], "gyro_lpf2_static_hz": ["500", "0"],
    "gyro_lpf2_dyn_hz": ["0,0", "100,300"], "gyro_notch_hz": ["0,0", "300,0", "300,400,0", "x,200", "", "0,350"],
    "gyro_notch_cutoff": ["0,0", "250,0", "250", "", "x"], "dterm_lpf1_type": ["0", "1"], "dterm_lpf1_static_hz": ["100", "0"],
    "dterm_lpf1_dyn_hz": ["0,0", "75,150"], "dterm_lpf2_type": ["0", "1"], "dterm_lpf2_static_hz": ["150"],
    "dterm_notch_hz": ["0", "260", "x"], "dterm_notch_cutoff": ["0", "160"], "yaw_lowpass_hz": ["0", "100"],
    "dyn_notch_count": ["0", "3", "x"], "dyn_notch_q": ["300", "x"], "dyn_notch_min_hz": ["100"], "dyn_notch_max_hz": ["600"],
    "rpm_filter_harmonics": ["0", "3", "x", "1", "5"], "rpm_filter_weights": ["100,100,100", "", "x", "100,50"],
    "rpm_filter_q": ["500", "x", "0"], "rpm_filter_min_hz": ["100", "x"], "rpm_filter_fade_range_hz": ["50", "x"],
    "rpm_filter_lpf_hz": ["150", "x"], "motor_poles": ["14", "12", "0", "1", "x", "-4"], "dshot_bidir": ["0", "1", "2", "x"],
    "motor_pwm_protocol": ["0", "5", "6", "9", "x", ""], "motorOutput": ["48,2047", "158,2047", "100", "", "x,1", "1,2,3"],
    "minthrottle": ["1000", "1070", "x"], "maxthrottle": ["2000", "x"], "blackbox_high_resolution": ["0", "1", "x"],
    "acc_1G": ["2048", "0", "-3", "x", "4096"], "motor_kv": ["0", "2300", "x"], "simplified_pids_mode": ["0", "1", "2", "x"],
    "simplified_master_multiplier": ["100", "x"], "simplified_i_gain": ["100"], "simplified_d_gain": ["100", "x"],
    "simplified_pi_gain": ["100"], "simplified_feedforward_gain": ["100"], "simplified_pitch_d_gain": ["100"],
    "simplified_pitch_pi_gain": ["100"], "simplified_d_max_gain": ["100", "x"], "simplified_dmax_gain": ["90"],
    "simplified_dterm_filter": ["1", "0"], "simplified_dterm_filter_multiplier": ["100"], "simplified_gyro_filter": ["1"],
    "simplified_gyro_filter_multiplier": ["100", ""],
}
NUMERIC_FORMS = ["0x10", "0X1f", "1_0", " 7 ", "1e2", "+5", "-5", "5.9", "-5.9", "nan", "inf", "", "x", "0x", "0x_1", "1__0"]


def run(raw: dict) -> dict:
    case = {"raw": raw}
    try:
        case["settings"] = settings_json(parse_settings(dict(raw)))
    except Exception as e:  # noqa: BLE001 (Python plante : noté, non comparé)
        case["error"] = f"{type(e).__name__}: {e}"
    return case


def main():
    rng = random.Random(20261030)
    cases = [run({})]
    for fw in FW:
        cases.append(run({"Firmware revision": fw}))
    for key, values in VOCAB.items():
        for v in values:
            cases.append(run({"Firmware revision": rng.choice(FW[:6]), key: v}))
    for key in VOCAB:
        for v in ['3,4', 'x,6,7', *rng.sample(NUMERIC_FORMS, 3)]:
            cases.append(run({key: v}))
    for c in range(256):  # chaque caractère latin-1 : début du nom du firmware, puis séparateur
        cases.append(run({"Firmware revision": f"{chr(c)}x 4.5.1 (a) B"}))
        cases.append(run({"Firmware revision": f"Betaflight{chr(c)}4.5.1 (a) B"}))
    for _ in range(300):
        raw = {k: rng.choice(v) for k, v in VOCAB.items() if rng.random() < 0.35}
        if rng.random() < 0.8:
            raw["Firmware revision"] = rng.choice(FW)
        cases.append(run(raw))
    json.dump(cases, open(sys.argv[1], "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
    print(len(cases), "cas dont", sum("error" in c for c in cases), "où Python lève une exception")


if __name__ == "__main__":
    main()
