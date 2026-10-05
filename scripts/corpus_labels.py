#!/usr/bin/env python3
"""Tableau d'étiquettes du corpus : une ligne par drone, à compléter dans un tableur (docs/assistant.md).

Usage : corpus_labels.py [--limit N]   (dossier de logs : variable OTB_LOGS ; écrit OTB_LOGS/corpus.csv)

Colonnes remplies par le script à chaque passage : drone, dossiers, sessions, origine, taille_detectee,
freq_moteur_hz, motor_kv (vide si absent ou égal au défaut 1960), cellules. Colonnes de Cédric, pré-remplies seulement à la création de la ligne
et jamais écrasées ensuite : etat (aide, origine, regle, inconnu), taille_reelle (pouces, ou « whoop »),
remarque. Les drones absents d'un passage gardent leur ligne (0 session après un passage complet).
Le script signale les noms de drone trouvés dans plusieurs dossiers : si ce sont des machines différentes,
ajouter le nom à SHARED_NAMES.
"""
import csv, hashlib, os, re, sys
from collections import Counter

import numpy as np

COLUMNS = ["drone", "dossiers", "sessions", "origine", "taille_detectee", "freq_moteur_hz", "motor_kv",
           "cellules", "etat", "taille_reelle", "remarque"]
USER_COLUMNS = ("etat", "taille_reelle", "remarque")
STATES = ("aide", "origine", "regle", "inconnu")
REFERENCE_STATES = ("regle", "origine")  # corpus de référence : drones réglés ou d'origine connus
HELP_FOLDERS = ("1. PTB", "3. BBLR FB")  # demandes d'aide (forums, groupes)
DEFAULT_KV = 1960  # motor_kv par défaut du firmware (pg/motor.c) : laissé vide, réglage probablement absent
CSV_NAME = "corpus.csv"
# noms donnés à plusieurs drones (confirmé par Cédric) : le drone est alors identifié par nom + dossier
SHARED_NAMES = {
    "Jax",  # Guigui : HORUS et Whisper25, 2026-10-05
    "Blueberry",  # 1. PTB/JL et JL/3.5, 2026-10-05
}
# noms trouvés dans plusieurs dossiers, mais d'un seul drone (confirmé par Cédric, 2026-10-05) : pas d'alerte
SAME_DRONE_NAMES = {"JeNo Ced", "JeNo CED", "M75 O4 CED", "AIR65 CED", "BABYDRONE"}


def logs_root():
    return os.environ.get("OTB_LOGS", os.path.join(os.path.expanduser("~"), "Documents", "Drones", "Logs"))


def drone_id(craft, relfile):
    """Identité d'un drone, la même pour la calibration : nom du drone, sinon « ? » + dossier du log ;
    nom + dossier pour un nom porté par plusieurs drones (SHARED_NAMES)."""
    name, folder = (craft or "").strip(), os.path.dirname(relfile)
    if not name:
        return "?" + folder
    return f"{name} · {folder}" if name in SHARED_NAMES else name


def log_files(root):
    """Logs du corpus (chemins absolus triés), sans les dossiers `__MACOSX` et sans doublons : un fichier
    au contenu identique à un autre (même log publié deux fois, copie renommée) n'est compté qu'une fois."""
    seen, out = set(), []
    for d, dirs, fs in os.walk(root):  # ordre fixe : dossiers triés, parcourus en profondeur
        dirs[:] = sorted(x for x in dirs if x != "__MACOSX")
        for f in sorted(fs):
            if not f.lower().endswith((".bbl", ".bfl")):
                continue
            path = os.path.join(d, f)
            h = hashlib.blake2b(digest_size=16)
            with open(path, "rb") as fh:
                for chunk in iter(lambda: fh.read(1 << 20), b""):
                    h.update(chunk)
            if h.digest() not in seen:
                seen.add(h.digest())
                out.append(path)
    return out


def guess_state(origins, texts):
    """État pré-rempli : dossier de demandes d'aide, puis mots des noms de fichiers et dossiers."""
    if any(o in HELP_FOLDERS for o in origins):
        return "aide"
    low = " ".join(texts).lower()
    if re.search(r"final|tuned|(?<![a-z])last(?![a-z])", low):
        return "regle"
    if "stock" in low:
        return "origine"
    return "inconnu"


def guess_size(texts):
    """Taille écrite dans un nom (drone, dossier, fichier : « 3.5 inch », « 5" », « Air65 »), sinon vide."""
    low = " ".join(texts).lower()
    if re.search(r"whoop|air ?65|meteor ?(?:65|75)|(?<!\d)(?:65|75) ?mm", low):
        return "whoop"
    m = re.search(r"(?<![\d.,])(\d{1,2}(?:[.,]\d)?) ?(?:\"|”|''|in(?![a-z])|inch|pouces?)", low)
    return m.group(1).replace(".", ",") if m else ""


def cell_count(settings, vbat):
    """Nombre de cellules par la formule du firmware (sensors/battery.c : tension / vbatmaxcellvoltage + 1),
    avec la tension au branchement (`vbatref`), sinon la première tension loggée."""
    v = settings.get_int("vbatref", 0) / 100.0
    if v <= 0 and vbat is not None and vbat.size:
        v = float(vbat[0])
    if v <= 0.5:
        return None
    cv = settings.get_ints("vbatcellvoltage")  # min, alerte, max (centièmes de volt)
    vmax = cv[2] / 100.0 if len(cv) == 3 and cv[2] > 0 else 4.30  # VBAT_CELL_VOLTAGE_DEFAULT_MAX
    return min(int(v / vmax) + 1, 24)


def read_labels(path):
    """{drone: ligne} depuis le tableau (séparateur « ; » ou « , »), ou {} s'il n'existe pas."""
    if not os.path.exists(path):
        return {}
    with open(path, encoding="utf-8-sig", newline="") as fh:
        text = fh.read()
    delim = ";" if text.split("\n", 1)[0].count(";") >= text.split("\n", 1)[0].count(",") else ","
    rows = csv.DictReader(text.splitlines(), delimiter=delim)
    return {r["drone"]: {k: (v or "").strip() for k, v in r.items() if k} for r in rows if r.get("drone")}


def merge(old, new, full=False):
    """Lignes à écrire : colonnes automatiques à jour, colonnes de Cédric conservées. Après un passage
    complet (`full`), un drone absent garde sa ligne avec 0 session (à effacer à la main)."""
    out = {d: {**row, "sessions": 0} if full and d not in new else row for d, row in old.items()}
    for d, row in new.items():
        out[d] = {**row, **{k: old[d].get(k, "") for k in USER_COLUMNS}} if d in old else row
    return sorted(out.values(), key=lambda r: (r.get("origine", ""), r["drone"].lower()))


def write_labels(path, rows):
    with open(path, "w", encoding="utf-8-sig", newline="") as fh:  # BOM : accents corrects dans Excel
        w = csv.DictWriter(fh, COLUMNS, delimiter=";", extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)


def scan(root, limit=None):
    """{drone: ligne} pour tous les logs du dossier (sessions d'au moins 8 s, comme la calibration)."""
    from opentunebox.advisor.profiles import detect_profile, profile_key_from_hz
    from opentunebox.analysis.motors import hover_motor_hz
    from opentunebox.log.flightlog import load

    files = log_files(root)[:limit]
    acc = {}
    for i, f in enumerate(files):
        rel = os.path.relpath(f, root)
        print(f"[{i + 1}/{len(files)}] {rel}", file=sys.stderr)
        try:
            logs = load(f)
        except Exception as e:
            print("ERR", rel, repr(e)[:150], file=sys.stderr)
            continue
        for lg in logs:
            if lg.duration < 8:
                continue
            a = acc.setdefault(drone_id(lg.settings.craft_name, rel), {
                "dirs": set(), "origins": set(), "names": set(), "sessions": 0, "hz": [], "kv": [],
                "cells": [], "profiles": []})
            a["dirs"].add(os.path.dirname(rel))
            a["origins"].add(rel.split(os.sep, 1)[0] if os.sep in rel else "")
            a["names"].add(os.path.basename(rel))
            a["sessions"] += 1
            hz = hover_motor_hz(lg)
            if hz is not None:
                a["hz"].append(hz)
            if lg.settings.motor_kv > 0 and lg.settings.motor_kv != DEFAULT_KV:
                a["kv"].append(lg.settings.motor_kv)
            n = cell_count(lg.settings, lg.vbat)
            if n:
                a["cells"].append(n)
            a["profiles"].append(detect_profile(lg)[0].key)
    rows = {}
    for d, a in acc.items():
        hz = float(np.median(a["hz"])) if a["hz"] else None
        named = [d.lstrip("?").split(" · ")[0], *a["dirs"]]
        rows[d] = {
            "drone": d, "dossiers": " | ".join(sorted(a["dirs"])), "sessions": a["sessions"],
            "origine": " | ".join(sorted(a["origins"])),
            "taille_detectee": profile_key_from_hz(hz) if hz else Counter(a["profiles"]).most_common(1)[0][0],
            "freq_moteur_hz": f"{hz:.0f}" if hz else "",
            "motor_kv": Counter(a["kv"]).most_common(1)[0][0] if a["kv"] else "",
            "cellules": Counter(a["cells"]).most_common(1)[0][0] if a["cells"] else "",
            "etat": guess_state(a["origins"], [*named, *a["names"]]),
            "taille_reelle": guess_size([*named, *a["names"]]), "remarque": "",
        }
    return rows


def shared_name_candidates(rows):
    """Noms trouvés dans plusieurs dossiers sans lien de parenté : peut-être plusieurs machines."""
    out = []
    for d, row in rows.items():
        dirs = row["dossiers"].split(" | ")
        inside = lambda a, b: a == b or a.startswith(b + os.sep)  # noqa: E731
        if not d.startswith("?") and d not in SAME_DRONE_NAMES and any(not inside(a, b) and not inside(b, a) for a in dirs for b in dirs):
            out.append(f"{d} : {row['dossiers']}")
    return out


if __name__ == "__main__":
    limit = int(sys.argv[sys.argv.index("--limit") + 1]) if "--limit" in sys.argv else None
    root = logs_root()
    path = os.path.join(root, CSV_NAME)
    old = read_labels(path)
    new = scan(root, limit)
    rows = merge(old, new, full=limit is None)
    write_labels(path, rows)
    shared = shared_name_candidates(new)
    if shared:
        print("Noms trouvés dans plusieurs dossiers (machines différentes ? → SHARED_NAMES) :\n  "
              + "\n  ".join(shared))
    count = Counter(r["etat"] or "(vide)" for r in rows)
    print(f"{path} : {len(rows)} drones ({len(rows) - len(old)} nouveaux) ; états : "
          + ", ".join(f"{k} {v}" for k, v in count.most_common()))
