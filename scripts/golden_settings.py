#!/usr/bin/env python3
"""Couche « réglages » des références (D-035, phase 4a-4) : `golden.py record-settings DOSSIER`.

`settings_json(s)` met un `Settings` en JSON sans rien de propre à Python (flottants en bits : `"f64:<16 chiffres hexa>"`).
`record_settings` écrit `DOSSIER/settings.jsonl` (une ligne par session du corpus : `{"file", "session", "settings"}`) ;
`otb-parity check-settings DOSSIER` relit ce fichier et refait `parse_settings` en Rust à partir de l'en-tête lu en Rust.
"""
from __future__ import annotations

import json
import os
import struct
import sys

from opentunebox.decoder.header import find_log_starts, parse_header
from opentunebox.log.settings import Settings, parse_settings


def f64(x: float) -> str:
    return "f64:" + struct.pack(">d", x).hex()


def _lowpass(lp) -> dict:
    return {"type": lp.type, "static_hz": lp.static_hz, "dyn_min_hz": lp.dyn_min_hz, "dyn_max_hz": lp.dyn_max_hz,
            "expo": lp.expo, "enabled": lp.enabled, "dynamic": lp.dynamic}


def settings_json(s: Settings) -> dict:
    return {
        "firmware": s.firmware, "firmware_name": s.firmware_name, "version": list(s.version), "board": s.board,
        "craft_name": s.craft_name, "looptime_us": s.looptime_us, "pid_denom": s.pid_denom,
        "p_interval": list(s.p_interval), "debug_mode": s.debug_mode, "debug_index": s.debug_index,
        "pids": [[p.P, p.I, p.D, p.Dmax, p.F] for p in s.pids],
        "gyro_lpf1": _lowpass(s.gyro_lpf1), "gyro_lpf2": _lowpass(s.gyro_lpf2),
        "gyro_notches": [list(n) for n in s.gyro_notches],
        "dterm_lpf1": _lowpass(s.dterm_lpf1), "dterm_lpf2": _lowpass(s.dterm_lpf2), "dterm_notch": list(s.dterm_notch),
        "yaw_lowpass_hz": s.yaw_lowpass_hz, "dyn_notch_count": s.dyn_notch_count, "dyn_notch_q": s.dyn_notch_q,
        "dyn_notch_min_hz": s.dyn_notch_min_hz, "dyn_notch_max_hz": s.dyn_notch_max_hz,
        "rpm_harmonics": s.rpm_harmonics, "rpm_weights": list(s.rpm_weights), "rpm_q": s.rpm_q,
        "rpm_min_hz": s.rpm_min_hz, "rpm_fade_hz": s.rpm_fade_hz, "rpm_lpf_hz": s.rpm_lpf_hz,
        "motor_poles": s.motor_poles, "dshot_bidir": s.dshot_bidir, "motor_protocol": s.motor_protocol,
        "motor_output": list(s.motor_output), "minthrottle": s.minthrottle, "maxthrottle": s.maxthrottle,
        "high_resolution": s.high_resolution, "acc_1g": s.acc_1g, "motor_kv": s.motor_kv,
        "gyro_rate_hz": f64(s.gyro_rate_hz), "pid_rate_hz": f64(s.pid_rate_hz), "log_rate_hz": f64(s.log_rate_hz),
        "family": s.family, "is_dshot": s.is_dshot, "sliders": s.sliders(),
    }


def record_settings(files, root, out_dir) -> None:
    os.makedirs(out_dir, exist_ok=True)
    n = 0
    with open(os.path.join(out_dir, "settings.jsonl"), "w", encoding="utf-8") as out:
        for i, f in enumerate(files):
            data = open(f, "rb").read()
            starts = find_log_starts(data)
            for k, s in enumerate(starts):
                rec = {"file": os.path.relpath(f, root), "session": k + 1}
                try:
                    h = parse_header(data, s, starts[k + 1] if k + 1 < len(starts) else len(data))
                    rec["settings"] = settings_json(parse_settings(h.raw))
                except Exception as e:  # noqa: BLE001 (Python plante : noté, non comparé)
                    rec["error"] = f"{type(e).__name__}: {e}"
                out.write(json.dumps(rec, ensure_ascii=False, separators=(",", ":")) + "\n")
                n += 1
            if (i + 1) % 100 == 0:
                print(f"[réglages] {i + 1}/{len(files)} fichiers, {n} sessions", file=sys.stderr)
    print(f"[réglages] {len(files)} fichiers, {n} sessions", file=sys.stderr)
