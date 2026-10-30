#!/usr/bin/env python3
"""Références par module pour la migration vers Rust (D-035, étape E2) : `golden.py record-modules DOSSIER`.

Pendant que les 8 outils et l'assistant tournent sur les logs de l'échantillon (comme `golden.py record`), chaque
fonction publique de `analysis/` et de `log/` (et chaque méthode de `FlightLog`) est enveloppée : ses arguments et son
résultat sont écrits dans `DOSSIER/modules/NNN/MODULE.FONCTION.K.npz` (NNN = rang du log dans `modules.json`,
K = rang de l'appel, `MAX_CALLS` appels par fonction et par log). Le futur comparateur Rust `otb-parity` relit ces
fichiers et rejoue chaque module seul.

Format d'un fichier : tableaux numpy sous les clés `a0`, `a1`… ; clé `__meta__` = JSON UTF-8 (octets) décrivant
`{"fn": …, "args": …, "kwargs": …, "result": …}`. Les valeurs du JSON :
  - flottant       → {"__f64__": "bits hexadécimaux"} (NaN et ±inf sans perte) ; entier, texte, booléen, null → tels quels ;
  - tableau numpy  → {"__npy__": "aK"} ; scalaire numpy → {"__np__": "dtype", "v": …} ;
  - liste / tuple / dict / slice → {"__list__": […]}, {"__tuple__": […]}, {"__dict__": [[clé, valeur], …]}, {"__slice__": [a, b, c]} ;
  - dataclass / NamedTuple → {"__obj__": "Classe", "fields": {…}} ;
  - FlightLog      → {"__flightlog__": {"path": relatif au corpus, "session": n}} (à rouvrir par le décodeur) ;
  - autre objet    → {"__repr__": "…"} (sans valeur de comparaison ; à repérer à la lecture) ;
  - exception levée → "raised": "Classe: message" à la place de "result".
Un appel dont les tableaux dépassent `MAX_BYTES` est ignoré (compté dans `modules.json`).
"""
from __future__ import annotations

import dataclasses
import hashlib
import inspect
import io
import json
import os
import struct
import sys
import types

import numpy as np

MAX_CALLS = 2  # appels gardés par fonction et par log
MAX_BYTES = 16 * 2**20
SKIP = {"flightlog.from_session", "flightlog.load"}  # entrée = sortie du décodeur (empreintes de `golden.py record`)
PACKAGES = ("opentunebox.analysis", "opentunebox.log")
CLASSES = (("opentunebox.log.flightlog", "FlightLog"),)


def _f64(x: float) -> dict:
    return {"__f64__": struct.pack(">d", x).hex()}


class _Encoder:
    def __init__(self, root: str):
        self.root, self.arrays, self.nbytes, self.opaque = root, {}, 0, 0

    def enc(self, o):
        from opentunebox.log.flightlog import FlightLog
        if o is None or isinstance(o, (bool, str)):
            return o
        if isinstance(o, np.generic):  # avant `float` : np.float64 en est une sous-classe
            v = o.item()
            return {"__np__": str(o.dtype), "v": _f64(v) if isinstance(v, float) else v}
        if isinstance(o, int):
            return int(o)
        if isinstance(o, float):
            return _f64(o)
        if isinstance(o, np.ndarray) and o.dtype.kind == "O":  # np.savez les écrirait en pickle
            return {"__list__": [self.enc(v) for v in o.tolist()]}
        if isinstance(o, np.ndarray):
            key = f"a{len(self.arrays)}"
            self.arrays[key] = o
            self.nbytes += o.nbytes
            return {"__npy__": key}
        if isinstance(o, FlightLog):
            return {"__flightlog__": {"path": os.path.relpath(str(o.path), self.root), "session": o.session}}
        if isinstance(o, slice):
            return {"__slice__": [self.enc(o.start), self.enc(o.stop), self.enc(o.step)]}
        if isinstance(o, tuple) and hasattr(o, "_fields"):
            return {"__obj__": type(o).__name__, "fields": {k: self.enc(v) for k, v in o._asdict().items()}}
        if isinstance(o, tuple):
            return {"__tuple__": [self.enc(v) for v in o]}
        if isinstance(o, (list, frozenset, set)):
            return {"__list__": [self.enc(v) for v in o]}
        if isinstance(o, dict):
            return {"__dict__": [[self.enc(k), self.enc(v)] for k, v in o.items()]}
        if dataclasses.is_dataclass(o) and not isinstance(o, type):
            return {"__obj__": type(o).__name__,
                    "fields": {f.name: self.enc(getattr(o, f.name)) for f in dataclasses.fields(o)}}
        self.opaque += 1
        return {"__repr__": repr(o)[:200]}


class Recorder:
    def __init__(self, out_dir: str, root: str):
        self.out_dir, self.root = out_dir, root
        self.log_dir: str | None = None
        self.counts: dict[str, int] = {}
        self.skipped: dict[str, int] = {}
        self._open: set[str] = set()  # fonctions en cours d'appel : un appel récursif n'est pas enregistré

    def start_log(self, index: int) -> None:
        self.log_dir = os.path.join(self.out_dir, "modules", f"{index:03d}")
        os.makedirs(self.log_dir, exist_ok=True)
        self.counts, self.skipped = {}, {}

    def wrap(self, name: str, fn):
        rec = self

        def wrapper(*args, **kwargs):
            if name in SKIP or rec.log_dir is None or rec.counts.get(name, 0) >= MAX_CALLS or name in rec._open:
                return fn(*args, **kwargs)
            enc = _Encoder(rec.root)
            a, kw = [enc.enc(x) for x in args], {k: enc.enc(v) for k, v in kwargs.items()}
            if enc.nbytes > MAX_BYTES:
                rec.skipped[name] = rec.skipped.get(name, 0) + 1
                return fn(*args, **kwargs)
            k = rec.counts[name] = rec.counts.get(name, 0) + 1
            rec._open.add(name)
            try:
                result, raised = fn(*args, **kwargs), None
            except Exception as e:  # noqa: BLE001 (on enregistre puis on relance)
                result, raised = None, f"{type(e).__name__}: {e}"
                meta = {"fn": name, "args": a, "kwargs": kw, "raised": raised}
                rec._write(name, k, meta, enc)
                raise
            finally:
                rec._open.discard(name)
            enc.opaque = 0
            res = enc.enc(result)
            if enc.nbytes > MAX_BYTES:
                rec.counts[name] = k - 1
                rec.skipped[name] = rec.skipped.get(name, 0) + 1
                return result
            rec._write(name, k, {"fn": name, "args": a, "kwargs": kw, "result": res}, enc)
            return result

        wrapper.__name__, wrapper.__doc__, wrapper.__wrapped__ = fn.__name__, fn.__doc__, fn
        wrapper._otb_recorded = True
        return wrapper

    def _write(self, name, k, meta, enc: _Encoder) -> None:
        buf = io.BytesIO()
        np.savez_compressed(buf, __meta__=np.frombuffer(json.dumps(meta).encode(), dtype=np.uint8), **enc.arrays)
        with open(os.path.join(self.log_dir, f"{name}.{k}.npz"), "wb") as f:
            f.write(buf.getvalue())


def _public_functions():
    import importlib
    import pkgutil
    found = []
    for pkg in PACKAGES:
        p = importlib.import_module(pkg)
        for info in pkgutil.iter_modules(p.__path__):
            mod = importlib.import_module(f"{pkg}.{info.name}")
            for attr, fn in vars(mod).items():
                if (isinstance(fn, types.FunctionType) and fn.__module__ == mod.__name__ and not attr.startswith("_")
                        and not getattr(fn, "_otb_recorded", False)):
                    found.append((mod, attr, fn, f"{info.name}.{attr}"))
    return found


def install(rec: Recorder) -> int:
    """Enveloppe les fonctions publiques ; remplace aussi les références prises par `from x import f` ailleurs."""
    import importlib
    patched = 0
    for mod, attr, fn, name in _public_functions():
        w = rec.wrap(name, fn)
        for m in list(sys.modules.values()):
            if m is not None and getattr(m, "__name__", "").startswith("opentunebox") and getattr(m, attr, None) is fn:
                setattr(m, attr, w)
        patched += 1
    for modname, cname in CLASSES:
        cls = getattr(importlib.import_module(modname), cname)
        for attr, fn in list(vars(cls).items()):
            if isinstance(fn, types.FunctionType) and not attr.startswith("_"):
                setattr(cls, attr, rec.wrap(f"{cname}.{attr}", fn))
                patched += 1
    return patched


def _run_log(path: str) -> None:
    """Fait tourner les 8 outils puis l'assistant sur la première session assez longue (comme `golden.py`)."""
    from golden import tools_snapshot
    from opentunebox.advisor.rules import analyze
    from opentunebox.analysis.period import auto_period
    from opentunebox.log.flightlog import load
    tools_snapshot(path)
    for lg in load(path):
        if lg.duration < 8:
            continue
        sel = lg.slice_time(*auto_period(lg))
        if sel.stop - sel.start >= lg.fs * 5:
            analyze(lg, sel)
            _extra_calls(lg)
            break


def _extra_calls(lg) -> None:
    """Appels explicites des fonctions publiques que ni les outils ni l'assistant n'utilisent, sur de vraies données."""
    from opentunebox.analysis import _numeric, bf_filters, datasets, loop, motors, period, sliders, spectral, step, step_series
    from opentunebox.log import modes
    st = lg.settings
    sel = slice(0, min(lg.n, int(lg.fs * 20)))
    _numeric.gaussian_filter(lg.gyro[0][sel], 3.0)
    chain = bf_filters.gyro_chain(st)
    specs = [spec for spec, _rate in chain]
    if specs:
        freqs = np.linspace(1.0, lg.fs / 2, 400)
        bf_filters.coefficients(specs[0], lg.fs)
        bf_filters.freq_response(specs, freqs, lg.fs)
        bf_filters.chain_delay_ms(specs, lg.fs)
    bf_filters.pt1_gain(100.0, lg.fs)
    bf_filters.notch_q(300.0, 250.0)
    bf_filters.dyn_notch_initial_centres(st)
    for lp in (st.gyro_lpf1, st.dterm_lpf1):
        bf_filters.scale_lowpass(lp, 1.5)
        bf_filters.lowpass_off(lp)
    cut = {}
    for name in ("t", "gyro", "gyro_unfilt", "setpoint", "rc_command", "p_term", "i_term", "d_term", "f_term",
                 "throttle", "motors", "motor_hz", "debug", "vbat", "current", "acc", "mode_flags"):
        v = getattr(lg, name)
        if v is not None:
            cut[name] = v[..., sel]
    short = dataclasses.replace(lg, **cut)  # même log tronqué à 20 s ; `path` et `session` restent ceux du log entier
    try:
        bf_filters.simulate_gyro(short)
        bf_filters.simulate_dterm(short)
    except Exception:  # noqa: BLE001 (log sans filtres simulables : l'exception est elle-même enregistrée)
        pass
    datasets.available(lg)
    loop.dterm_model(st, np.linspace(1.0, lg.fs / 2, 200), 1.0)
    modes.mode_changes(lg.mode_flags, lg.t, st.version)
    modes.mode_names(0b110, st.version)
    modes.mode_names(0b110, st.version, shown_only=False)
    motors.distribution(lg.motors[0][sel], np.linspace(0.0, 100.0, 21))
    t0 = float(lg.t[sel][0])
    period.flight_parts(lg, t0, float(lg.t[sel][-1]))
    fam = st.family
    pids = sliders.default_pids(fam)
    sv = sliders.sliders_from_pids(pids, fam)
    sliders.cli_commands(sv, fam)
    sliders.filters_from_sliders(10, 10, fam, st)
    spectral.smooth_map(np.abs(np.random.default_rng(1).standard_normal((40, 30))), 2)
    step.input_windows(lg.setpoint[0][sel], lg.fs)
    fs_ = step_series.flight_step(lg, sel, "essai")
    step_series.find_series([fs_])


def record_modules(files, root, out_dir, sample) -> None:
    from golden import sample_files
    rec = Recorder(out_dir, root)
    # tous les modules à enrober doivent être importés avant `install`
    import opentunebox.advisor.rules  # noqa: F401
    import opentunebox.gui.tools  # noqa: F401
    n = install(rec)
    print(f"{n} fonctions enveloppées", file=sys.stderr)
    index, names = {}, []
    for i, f in enumerate(sample_files(files, sample)):
        rec.start_log(i)
        _run_log(f)
        names.append(os.path.relpath(f, root))
        index[names[-1]] = {"calls": dict(sorted(rec.counts.items())), "skipped": dict(sorted(rec.skipped.items()))}
        print(f"[modules {i + 1}] {names[-1]} : {sum(rec.counts.values())} appels, {len(rec.counts)} fonctions",
              file=sys.stderr)
    json.dump({"logs": names, "detail": index}, open(os.path.join(out_dir, "modules.json"), "w"), indent=1)


def digest_dir(out_dir: str) -> dict[str, str]:
    """SHA-256 du contenu (tableaux et JSON) de chaque fichier : l'enregistrement est-il reproductible ?"""
    out = {}
    base = os.path.join(out_dir, "modules")
    for d, _, fs in os.walk(base):
        for name in fs:
            z = np.load(os.path.join(d, name))
            h = hashlib.sha256()
            for k in sorted(z.files):
                a = z[k]
                h.update(k.encode() + str(a.dtype).encode() + str(a.shape).encode() + a.tobytes())
            out[os.path.relpath(os.path.join(d, name), base)] = h.hexdigest()
    return out
