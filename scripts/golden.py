#!/usr/bin/env python3
"""Résultats de référence (« golden masters ») pour la migration vers Rust (D-031, docs/migration-rust.md).

Usage : golden.py record DOSSIER [--limit N] [--sample N] [-j PROCESSUS]
        golden.py check  DOSSIER [--limit N] [--sample N] [-j PROCESSUS] [--rtol X]
        golden.py record-modules DOSSIER [--limit N] [--sample N]   (références par module, D-035 : golden_modules.py)
        golden.py record-header  DOSSIER [--limit N]   (en-têtes de toutes les sessions : golden_header.py)
        golden.py record-settings DOSSIER [--limit N]   (réglages de toutes les sessions : golden_settings.py)
        golden.py record-sliders DOSSIER [--limit N]   (curseurs de toutes les sessions : golden_sliders.py)
        golden.py record-flightlog DOSSIER [--limit N] [-j PROCESSUS]   (journaux de vol : golden_flightlog.py)
        golden.py record-analysis DOSSIER [--limit N] [-j PROCESSUS]   (analyses de vol de toutes les sessions : golden_analysis.py)
        golden.py record-advisor DOSSIER [--limit N] [-j PROCESSUS]   (rapport et diagnostic du bruit de l'assistant : golden_advisor.py)
        golden.py check-modules  DOSSIER [--limit N] [--sample N]   (réenregistre dans DOSSIER.new, compare le contenu)

DOSSIER : où ranger les références, hors du dépôt (par exemple `$OTB_LOGS/golden`). Trois couches :
  - décodeur  : empreinte (SHA-256) de chaque tableau de chaque session du corpus, exacte au bit près ;
  - outils    : sortie de `compute` de chaque outil du registre sur un échantillon de logs (--sample, défaut 20
                logs répartis dans tout le corpus), tous les nombres gardés ; comparés avec la tolérance --rtol
                (défaut 0 = identique ; ≈ 1e-5 pour juger une réécriture de FFT) ;
  - assistant : `advisor_rates.py` sur tout le corpus (comparé par `advisor_rates.py --compare`).

`record` décode chaque fichier jusqu'à ce que deux passages concordent : le poste de Cédric inverse parfois
un bit sous pleine charge (compétence `calculs-corpus`). `check` refait seul (un processus) tout fichier
qui diffère avant de le signaler. Code de sortie 1 si une différence subsiste.
"""
import hashlib, json, os, subprocess, sys
from dataclasses import fields, is_dataclass

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from corpus_labels import log_files, logs_root, map_files  # noqa: E402

ARRAYS = ("main", "main_kind", "slow", "gps", "gps_at", "events")


# ------------------------------------------------------------------ décodeur
def _digest(a) -> str:
    a = np.ascontiguousarray(a)
    return f"{a.dtype}|{a.shape}|{hashlib.sha256(a.tobytes()).hexdigest()}"


def decode_digest(path):
    """Une empreinte par session du fichier : tableaux, noms de champs, en-tête, statistiques."""
    from opentunebox.decoder.reader import decode_file
    out = []
    for s in decode_file(path):
        d = {k: _digest(getattr(s, k)) for k in ARRAYS}
        d["names"] = [s.main_names, s.slow_names, s.gps_names]
        d["header"] = hashlib.sha256(repr(s.header).encode()).hexdigest()
        d["stats"] = json.loads(json.dumps(s.stats, default=str, sort_keys=True))
        out.append(d)
    return out


def _decoder_pass(files, root, jobs):
    return {os.path.relpath(f, root): r for f, r in map_files(decode_digest, files, jobs, root)}


def record_decoder(files, root, out_dir, jobs):
    a = _decoder_pass(files, root, jobs)
    b = _decoder_pass(files, root, jobs)
    for _ in range(3):  # fichiers où les deux passages divergent : refaits seuls jusqu'à concordance
        bad = [f for f in files if a.get(os.path.relpath(f, root)) != b.get(os.path.relpath(f, root))]
        if not bad:
            break
        print(f"{len(bad)} fichier(s) aux passages discordants, refaits seuls", file=sys.stderr)
        for f in bad:
            k = os.path.relpath(f, root)
            a[k], b[k] = decode_digest(f), decode_digest(f)
    json.dump(a, open(os.path.join(out_dir, "decoder.json"), "w"), indent=0, sort_keys=True)
    print(f"décodeur : {len(a)} fichiers, {sum(len(v) for v in a.values())} sessions")


def check_decoder(files, root, out_dir, jobs):
    ref = json.load(open(os.path.join(out_dir, "decoder.json")))
    new = _decoder_pass(files, root, jobs)
    bad = [k for k in ref if k in new and new[k] != ref[k]]
    for k in bad:  # une inversion de bit du poste s'écarte en refaisant le fichier seul
        new[k] = decode_digest(os.path.join(root, k))
    bad = [k for k in bad if new[k] != ref[k]]
    missing = sorted(set(ref) - set(new))
    print(f"décodeur : {len(ref)} fichiers de référence, {len(bad)} différents, {len(missing)} absents")
    for k in bad[:20]:
        print("  ≠", k)
    return not bad and not missing


# ------------------------------------------------------------------ outils
def flatten(obj, path="") -> dict:
    """Tout nombre, texte ou tableau d'un résultat d'outil, sous un nom stable (chemin dans la structure)."""
    out = {}
    if obj is None:
        out[path] = np.array("None")
    elif isinstance(obj, (np.ndarray, np.generic, int, float, bool, str, bytes)):
        out[path] = np.asarray(obj)
    elif isinstance(obj, dict):
        for k in sorted(obj, key=str):
            out |= flatten(obj[k], f"{path}/{k}")
    elif isinstance(obj, (list, tuple, set, frozenset)):
        items = sorted(obj, key=repr) if isinstance(obj, (set, frozenset)) else obj
        for i, v in enumerate(items):
            out |= flatten(v, f"{path}[{i}]")
    elif is_dataclass(obj) and not isinstance(obj, type):
        for f in fields(obj):
            out |= flatten(getattr(obj, f.name), f"{path}.{f.name}")
    elif hasattr(obj, "__dict__"):
        for k in sorted(vars(obj)):
            out |= flatten(vars(obj)[k], f"{path}.{k}")
    else:
        out[path] = np.array(repr(obj))
    return out


def sample_files(files, n):
    step = max(1, len(files) // n)
    return files[::step][:n]


def tools_snapshot(path):
    """{outil/chemin: valeur} de tous les outils sur la première session d'au moins 8 s du log."""
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    from PySide6.QtWidgets import QApplication
    app = QApplication.instance() or QApplication([])  # noqa: F841 (doit rester vivant : les widgets en dépendent)
    from opentunebox.analysis.period import auto_period
    from opentunebox.gui.state import AppState
    from opentunebox.gui.tools import REGISTRY, create_tool
    from opentunebox.log.flightlog import load
    log = next((lg for lg in load(path) if lg.duration >= 8), None)
    if log is None:
        return {}
    out = {"_session": np.array(log.session)}
    for key in sorted(REGISTRY):
        state = AppState()
        state.add(log)
        state.logs[0].period = auto_period(log)
        win = create_tool(key, state, None)
        if win is None or not hasattr(win, "compute"):
            continue
        win.auto_run = False
        win.files.selectAll()
        res = win.compute_sync(win.files.selected_entries())
        out |= flatten(res, key)
        win.deleteLater()
    return out


def _tools_file(out_dir, i):
    return os.path.join(out_dir, "tools", f"{i:03d}.npz")


def record_tools(files, root, out_dir, sample):
    os.makedirs(os.path.join(out_dir, "tools"), exist_ok=True)
    names = []
    for i, f in enumerate(sample_files(files, sample)):
        snap = tools_snapshot(f)
        np.savez_compressed(_tools_file(out_dir, i), **{k.replace("/", "|"): v for k, v in snap.items()})
        names.append(os.path.relpath(f, root))
        print(f"[outils {i + 1}] {names[-1]} : {len(snap)} valeurs", file=sys.stderr)
    json.dump(names, open(os.path.join(out_dir, "tools.json"), "w"), indent=0)


def same(a: np.ndarray, b: np.ndarray, rtol: float) -> bool:
    if a.shape != b.shape:
        return False
    if a.dtype.kind in "fc" and b.dtype.kind in "fc":
        finite = np.abs(a[np.isfinite(a)])
        atol = rtol * float(finite.max()) if finite.size else 0.0  # écart permis près de zéro : relatif au maximum
        return bool(np.allclose(a, b, rtol=rtol, atol=atol, equal_nan=True))
    return bool(np.array_equal(a, b))


def check_tools(files, root, out_dir, sample, rtol):
    names = json.load(open(os.path.join(out_dir, "tools.json")))
    bad = 0
    for i, rel in enumerate(names):
        ref = np.load(_tools_file(out_dir, i))
        new = {k.replace("/", "|"): v for k, v in tools_snapshot(os.path.join(root, rel)).items()}
        diff = [k for k in ref.files if k not in new or not same(ref[k], np.asarray(new[k]), rtol)]
        diff += [k for k in new if k not in ref.files]
        if diff:
            bad += 1
            print(f"  ≠ {rel} : {len(diff)} valeurs, par exemple {diff[:3]}")
    print(f"outils : {len(names)} logs de référence, {bad} différents (rtol={rtol})")
    return bad == 0


# ------------------------------------------------------------------ assistant et commande
def main():
    argv = sys.argv[1:]
    if len(argv) < 2 or argv[0] not in ("record", "check", "record-modules", "check-modules", "record-header", "record-settings", "record-sliders", "record-flightlog",
                                                  "record-analysis", "record-advisor"):
        sys.exit(__doc__)
    mode, out_dir = argv[0], argv[1]
    opt = lambda name, default, cast: cast(argv[argv.index(name) + 1]) if name in argv else default  # noqa: E731
    limit, sample, jobs, rtol = opt("--limit", None, int), opt("--sample", 20, int), opt("-j", None, int), \
        opt("--rtol", 0.0, float)
    root = logs_root()
    files = log_files(root)[:limit]
    os.makedirs(out_dir, exist_ok=True)
    if mode == "record-flightlog":
        from golden_flightlog import record_flightlog
        record_flightlog(files, root, out_dir, jobs)
        from opentunebox.gui.app import hard_exit  # L-019
        hard_exit(0)
    if mode == "record-advisor":
        from golden_advisor import record_advisor
        record_advisor(files, root, out_dir, jobs)
        from opentunebox.gui.app import hard_exit  # L-019
        hard_exit(0)
    if mode == "record-analysis":
        from golden_analysis import record_analysis
        record_analysis(files, root, out_dir, jobs)
        from opentunebox.gui.app import hard_exit  # L-019
        hard_exit(0)
    if mode == "record-settings":
        from golden_settings import record_settings
        record_settings(files, root, out_dir)
        return
    if mode == "record-sliders":
        from golden_sliders import record_sliders
        record_sliders(files, root, out_dir)
        return
    if mode == "record-header":
        from golden_header import record_header
        record_header(files, root, out_dir)
        return
    if mode in ("record-modules", "check-modules"):
        from golden_modules import digest_dir, record_modules
        from opentunebox.gui.app import hard_exit  # L-019
        if mode == "record-modules":
            record_modules(files, root, out_dir, sample)
            hard_exit(0)
        new_dir = out_dir.rstrip("/") + ".new"
        record_modules(files, root, new_dir, sample)
        old, new = digest_dir(out_dir), digest_dir(new_dir)
        diff = sorted(k for k in old.keys() | new.keys() if old.get(k) != new.get(k))
        print(f"modules : {len(old)} fichiers de référence, {len(diff)} différents", *diff[:10], sep="\n  ")
        hard_exit(1 if diff else 0)
    rates = os.path.join(out_dir, "advisor.json")
    advisor = [sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)), "advisor_rates.py")]
    extra = (["--limit", str(limit)] if limit else []) + (["-j", str(jobs)] if jobs else [])
    if mode == "record":
        record_decoder(files, root, out_dir, jobs)
        subprocess.run([*advisor, rates, *extra], check=True)
        record_tools(files, root, out_dir, sample)
        ok = True
    else:
        ok = check_decoder(files, root, out_dir, jobs)
        new_rates = rates + ".new"
        subprocess.run([*advisor, new_rates, *extra], check=True)
        ok &= subprocess.run([*advisor, "--compare", rates, new_rates]).returncode == 0
        ok &= check_tools(files, root, out_dir, sample, rtol)
    print("RÉFÉRENCES", "enregistrées" if mode == "record" else ("IDENTIQUES" if ok else "DIFFÉRENTES"))
    from opentunebox.gui.app import hard_exit  # PySide6 + pyqtgraph plantent à la sortie normale (L-019)
    hard_exit(0 if ok else 1)


if __name__ == "__main__":
    main()
