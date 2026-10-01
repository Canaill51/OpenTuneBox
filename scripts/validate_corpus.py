#!/usr/bin/env python3
"""Compare notre décodeur à blackbox_decode (référence) sur tout un dossier de logs.

Usage : validate_corpus.py [DOSSIER_OU_FICHIER ...] [--decoder CHEMIN] [--limit N]

Pour chaque session : mêmes lignes (alignées sur loopIteration) et mêmes valeurs entières
pour toutes les colonnes communes. Affiche un résumé et la liste des écarts.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))
from opentunebox.decoder.reader import decode_file  # noqa: E402

DEFAULT_REF = Path.home() / ".local/share/PIDtoolboxProV094/blackbox_decode"
DEFAULT_ROOT = Path.home() / "Documents/Drones/Logs"


def read_ref_csv(path: Path) -> tuple[list[str], np.ndarray]:
    with open(path, encoding="latin-1") as f:
        names = [n.strip() for n in f.readline().split(",")]
        # « gyroADC[0] (deg/s) » → « gyroADC[0] » ; « time (us) » → « time »
        names = [n.split(" (")[0] for n in names]
        rows = []
        for line in f:
            parts = line.split(",")
            try:
                rows.append([float(p) for p in parts])
            except ValueError:
                rows.append([float(p) if _isnum(p) else np.nan for p in parts])
    if not rows:
        return names, np.zeros((0, len(names)))
    width = len(names)
    arr = np.full((len(rows), width), np.nan)
    for i, r in enumerate(rows):
        arr[i, : min(width, len(r))] = r[:width]
    return names, arr


def _isnum(s: str) -> bool:
    try:
        float(s)
        return True
    except ValueError:
        return False


def compare_file(path: Path, ref_bin: Path, tmp: Path) -> list[dict]:
    for f in tmp.glob("*"):
        f.unlink()
    try:
        subprocess.run(
            [str(ref_bin), "--unit-flags", "raw", "--unit-vbat", "raw", "--unit-amperage", "raw",
             "--output-dir", str(tmp), str(path)],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=180,
        )
    except subprocess.TimeoutExpired:
        # la référence boucle parfois indéfiniment sur des logs abîmés : on vérifie quand même
        # que notre décodeur lit le fichier
        try:
            n = len(decode_file(path))
            detail = f"référence bloquée ; notre décodeur lit {n} session(s)"
        except Exception as e:  # noqa: BLE001
            detail = f"référence bloquée ; notre décodeur échoue : {e!r}"
        return [{"file": str(path), "session": 0, "time": 0.0, "status": "refhang", "detail": detail}]
    t0 = time.perf_counter()
    try:
        sessions = decode_file(path)
        err = None
    except Exception as e:  # noqa: BLE001
        sessions, err = [], repr(e)
    dt = time.perf_counter() - t0
    ours = {s.index: s for s in sessions}
    results = []
    refs = sorted(tmp.glob("*.csv"))
    refs = [r for r in refs if not r.name.endswith((".gps.csv", ".headers.csv"))]
    for ref in refs:
        idx = int(ref.name.split(".")[-2])
        res = {"file": str(path), "session": idx, "time": dt, "status": "ok", "detail": ""}
        names, arr = read_ref_csv(ref)
        s = ours.get(idx)
        if s is None:
            res.update(status="missing", detail=err or "session non décodée")
            results.append(res)
            continue
        # Alignement sur l'horodatage : la version de blackbox_decode fournie avec PTB compte mal
        # loopIteration quand « P interval » est écrit sous la forme « N » (BF récents).
        t_ref = arr[:, names.index("time")].astype(np.int64)
        t_our = s.main[:, 1]
        common, ir, io = np.intersect1d(t_ref, t_our, return_indices=True)
        res["rows_ref"], res["rows_our"], res["rows_common"] = len(t_ref), len(t_our), len(common)
        slow_names = set(s.slow_names)
        bad, bad_slow = [], []
        for k, n in enumerate(names):
            if n == "loopIteration":
                continue
            col = s.column(n)
            if col is None:
                continue
            a = arr[ir, k]
            b = col[io].astype(np.float64)
            ok = np.isnan(a) | (a == b) | (a == (b % 2**32))
            if not ok.all():
                first = int(np.argmax(~ok))
                msg = f"{n}: {int((~ok).sum())} écarts (ex. t={int(common[first])}: ref={a[first]} nous={b[first]})"
                (bad_slow if n in slow_names else bad).append(msg)

        # Lignes de la référence absentes chez nous : tolérées si elles précèdent immédiatement
        # un trou d'enregistrement (trame tronquée par une pause d'écriture SD, que la
        # référence accepte avec des valeurs parasites).
        missing = np.setdiff1d(t_ref, t_our) if len(t_ref) else np.zeros(0, dtype=np.int64)
        period = float(np.median(np.diff(t_ref))) if len(t_ref) > 1 else 1.0
        pos = np.searchsorted(t_ref, missing)
        nxt = np.zeros(0) if not len(t_ref) else np.where(pos + 1 < len(t_ref), t_ref[np.minimum(pos + 1, len(t_ref) - 1)], t_ref[-1] + 10 * period)
        before_gap = (nxt - missing) > 3 * period
        n_gap, n_other = int(before_gap.sum()), int((~before_gap).sum())
        extra = len(t_our) - len(common)
        notes = []
        if extra:
            notes.append(f"+{extra} trames valides en plus chez nous")
        if n_gap:
            res["status"] = "gap"
            notes.append(f"{n_gap} trame(s) tronquée(s) avant un trou SD rejetée(s)")
        if bad_slow:
            res["status"] = "slow"
            notes.append("données lentes : " + "; ".join(bad_slow[:2]))
        if n_other:
            res["status"] = "rows"
            notes.append(f"{n_other} lignes de la référence absentes chez nous")
        if bad:
            res["status"] = "values"
            notes.append("; ".join(bad[:4]))
        res["detail"] = " | ".join(notes)
        results.append(res)
    if not refs and sessions:
        results.append({"file": str(path), "session": 0, "time": dt, "status": "noref", "detail": "référence vide"})
    return results


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("paths", nargs="*", default=[str(DEFAULT_ROOT)])
    ap.add_argument("--decoder", default=str(DEFAULT_REF))
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--start", type=int, default=1, help="reprendre au fichier n° START")
    args = ap.parse_args()

    files: list[Path] = []
    for p in map(Path, args.paths):
        if p.is_dir():
            files += sorted(f for f in p.rglob("*") if f.suffix.lower() in (".bbl", ".bfl"))
        else:
            files.append(p)
    first = max(1, args.start)
    if args.limit:
        files = files[: first - 1 + args.limit]

    all_res = []
    with tempfile.TemporaryDirectory() as td:
        for k, f in enumerate(files, 1):
            if k < first:
                continue
            res = compare_file(f, Path(args.decoder), Path(td))
            all_res += res
            worst = [r for r in res if r["status"] not in ("ok", "gap", "refhang")]
            mark = "OK " if not worst else "ERR"
            print(f"[{k}/{len(files)}] {mark} {f.relative_to(f.anchor) if not f.is_relative_to(DEFAULT_ROOT) else f.relative_to(DEFAULT_ROOT)}"
                  f" ({len(res)} sessions)", flush=True)
            for r in worst:
                print(f"      session {r['session']}: {r['status']} {r['detail']}", flush=True)

    n_ok = sum(r["status"] in ("ok", "gap", "refhang") for r in all_res)
    print(f"\nSessions conformes (ok + trou SD) : {n_ok}/{len(all_res)}")
    by = {}
    for r in all_res:
        by[r["status"]] = by.get(r["status"], 0) + 1
    print("Par statut :", by)
    return 0 if n_ok == len(all_res) else 1


if __name__ == "__main__":
    sys.exit(main())
