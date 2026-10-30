#!/usr/bin/env python3
"""Cas de test de `decoder/header.py` pour `rust/otb-decoder` (D-035, phase 4a-2).

Usage : header_cases.py SORTIE.json

Chaque cas : octets d'entrée (hexadécimal), début, fin, et le résultat de `find_log_starts` / `parse_header` de Python
(`raw`, définitions de champs, `data_start`, `data_end`, propriétés numériques), ou `error` si Python lève une exception
(par exemple `int(float("inf"))` : Python plante, le Rust rend la valeur par défaut, écart de robustesse assumé, donc
ces cas ne sont pas comparés). Cas écrits à la main (ce que le firmware et les tests de `tests/test_decoder.py` montrent)
puis cas tirés au hasard (graine fixe) à partir d'un vocabulaire de lignes d'en-tête.
"""
import json
import random
import sys

from opentunebox.decoder.header import find_log_starts, parse_header

START = b"H Product:Blackbox flight data recorder by Nicholas Sherlock"

HAND = [
    START + b"\r\nH Data version:2\nH Field I name:loopIteration,time,gyroADC[0]\nH Field I signed:0,0,1\n"
    b"H Field I predictor:0,0,0\nH Field I encoding:1,1,0\nH Field P predictor:6,2,1\nH Field P encoding:9,0,0\n"
    b"H ligne sans deux-points\nH Firmware revision:Betaflight 4.5.1 (abc) STM32F7X2\nH rollPID:45,80,40\n\x00\x00I\x01\x02",
    b"H Field G name:GPS_home[0],GPS_home[1],GPS_numSat\nH Field G predictor:7\nI",
    b"H Field G name:GPS_home[0],GPS_home[1],GPS_numSat\nH Field G predictor:7,7,0\nI",
    b"H Field G name:a,b,c,d\nH Field G predictor:7,7,7,7\nI",
    b"H P interval:3/8\nH I interval:0\nH Data version:3\nH minthrottle:1070\nH maxthrottle:2000\nH vbatref:1680\nI",
    b"H P interval:0/0\nH motorOutput:48,2047\nI", b"H motorOutput:100\nI", b"H motorOutput:\nI", b"H motorOutput: ,x\nI",
    b"H P interval:2\nI", b"H P interval:abc\nI", b"H P interval:1/\nI", b"H P interval:/4\nI",
    b"H minthrottle:0x1F\nH vbatref:0X10\nH Data version: 0x\nH I interval:-5\nI",
    b"H minthrottle: 12.9 \nH vbatref:-3.7\nH maxthrottle:1e3\nH Data version:1_0\nI",
    b"H vbatref:1__0\nH minthrottle:_1\nH maxthrottle:1_\nH Data version:0x1_f\nI",
    b"H vbatref:0x1__f\nH minthrottle:0x_1f\nH maxthrottle:0x1f_\nH Data version:0x_\nI",
    b"H vbatref:0X1_F\nH minthrottle:0x-1\nH maxthrottle:-0x10\nH Data version:0x 1\nI",
    b"H vbatref:nan\nH minthrottle:abc\nI", b"H vbatref:inf\nI", b"H vbatref:-inf\nI", b"H motorOutput:1,inf\nI",
    b"H Field I name: a , b\xa0,\x85c\nH Field I predictor: 0 , 1 ,x, 2\nH Field I encoding:1\nI",
    b"H Field I name:a\x1c\nH Field I signed:\nI", b"H Field I\nH Field I x\nH Field I n\nH Field Ix:1\nI",
    b"H Field Z name:z\nH Field  name:q\nH Field Iname:1\nH Field I foo:1\nI",
    b"H a:b:c\nH :v\nH k:\nH k2:v\r\r\nH k2:w\nI", b"H a:1\nHx\nI", b"H a:1", b"H a:1\n", b"H a:1\nH ", b"H ",
    b"", b"x", b"\x00\x00\x00", b"IPGHSE", b"I", b"H", b"Hx", b"H a:1\r\nE", b"junk" + START + b"\nH Data version:2\nI",
    START + b"\nI" + START + b"\nH vbatref:5\nP",
]
HAND_RANGES = {0: [(0, None)], 5: [(0, None), (3, None)]}

VOCAB = [
    b"H Field I name:loopIteration,time,motor[0],motor[1]", b"H Field I signed:0,0,0,0", b"H Field I predictor:0,0,4,5",
    b"H Field I encoding:1,1,1,0", b"H Field P predictor:6,2,1,1", b"H Field P encoding:9,0,0,0", b"H Field S name:a,b",
    b"H Field S predictor:0,0", b"H Field S encoding:1,1", b"H Field G name:GPS_home[0],GPS_home[1],x",
    b"H Field G predictor:7,7,7", b"H Field H name:GPS_home[0],GPS_home[1]", b"H Field H predictor:7,7",
    b"H Data version:2", b"H Data version:0x2", b"H I interval:2", b"H P interval:1/2", b"H P interval:4",
    b"H minthrottle:1050", b"H maxthrottle:2000", b"H motorOutput:48,2047", b"H motorOutput:1,2,3", b"H vbatref:1680",
    b"H Firmware revision:EmuFlight 0.4.1", b"H rollPID:45,80,40", b"H x", b"H :", b"H =:=", b"H Field", b"H Field I",
    b"H Field I name", b"H Field I name:", b"H Field I name:,", b"H Field I predictor:1,,2", b"H Field I predictor:a",
    b"H vbatref: 99 ", b"H vbatref:9.9e1", b"H vbatref:0xff", b"\x00", b"\xff\xfe", b"junk", b"",
]


def props(h):
    return {"data_version": h.data_version, "i_interval": h.i_interval, "p_interval": list(h.p_interval),
            "minthrottle": h.minthrottle, "motor_output": list(h.motor_output), "vbatref": h.vbatref}


def run(data: bytes, start: int, end: int) -> dict:
    case = {"hex": data.hex(), "start": start, "end": end, "starts": find_log_starts(data)}
    try:
        h = parse_header(data, start, end)
        case["raw"] = h.raw
        case["frames"] = {k: {"names": f.names, "signed": f.signed, "predictor": f.predictor, "encoding": f.encoding}
                          for k, f in h.frames.items()}
        case["data_start"], case["data_end"] = h.data_start, h.data_end
        case["props"] = props(h)
    except Exception as e:  # noqa: BLE001 (Python plante : le cas est noté, pas comparé)
        case["error"] = f"{type(e).__name__}: {e}"
    return case


def main():
    rng = random.Random(20261030)
    cases = []
    for data in HAND:
        cases.append(run(data, 0, len(data)))
        if len(data) > 4:
            cases.append(run(data, 3, len(data) - 1))  # fin avant la fin du tampon, début décalé
    for _ in range(400):
        lines = [rng.choice(VOCAB) for _ in range(rng.randint(1, 14))]
        sep = rng.choice([b"\n", b"\r\n", b"\n", b"\n\n"])
        data = (START + sep if rng.random() < 0.5 else b"") + sep.join(lines) + rng.choice([b"", b"\n", b"\nI", b"\nPxx", b"\x00E"])
        end = len(data) if rng.random() < 0.8 else rng.randint(0, len(data))
        cases.append(run(data, 0, end))
    json.dump(cases, open(sys.argv[1], "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
    print(len(cases), "cas dont", sum("error" in c for c in cases), "où Python lève une exception")


if __name__ == "__main__":
    main()
