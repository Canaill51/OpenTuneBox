#!/usr/bin/env python3
"""Cas de test de `noise.noise_report` sur des journaux fabriqués, pour `rust/otb-advisor/src/noise.rs` (phase 5a-4).

Usage : noise_cases.py SORTIE.json    (à exécuter avec `.venv/bin/python -I`)

Complète la couche corpus (`otb-parity check-advisor`, clés `noise|…`) : des vols courts fabriqués (modèle de `rules_state_cases.py`) font
atteindre les branches que le corpus ne couvre pas ou mal. Chaque cas appelle l'étalon Python `noise_report(log, sel, profile)` et garde
**tout** son diagnostic : bande, vol avec mouvements, niveaux de bruit par axe, hauteur des harmoniques, constats rangés par partie
(avec textes, actions, réglages), blocs de filtres (titre, réglages, observations, constats, verdict), stratégie. Un flottant s'écrit en
hexadécimal. Le test `rust/otb-advisor/tests/noise.rs` exige des textes identiques au caractère près et les mêmes décisions ; les niveaux
de bruit passent par une FFT (tolérance mesurée).

Familles de cas :
- filtre RPM : sans gyro non filtré, sans eRPM (avec et sans dshot_bidir), filtre coupé, 2 harmoniques, poids absents ou en nombre
  insuffisant, rpm_filter_q au maximum, Betaflight 4.4, journal trop lent (spectre replié) ;
- notch dynamique : coupé, boucle PID trop lente, sans filtre RPM, pics hors de la plage (vol avec mouvements) ou dans la plage,
  plus de pics que de notchs, raie fixe qui ne suit pas les moteurs (observation de la raie) ;
- passe-bas : filtres coupés / dynamiques / statiques, vol sans mouvement ;
- bruit : gyro et D bruités, terme D nul (axes sautés), gyro nul, bande vide, trous du journal, période explicite ;
- stratégie : celle de `rules` (plusieurs étapes), puis le repli (aucun changement, un seul réglage) ;
- erreurs : période hors du journal (Python lève, Rust aussi).
"""
import json
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))  # -I retire le dossier du script de sys.path
from bf_filters_cases import hx  # noqa: E402
from flight_stats_cases import INPUTS, enc_log  # noqa: E402
from rules_notch_cases import sweep  # noqa: E402
from rules_state_cases import build, combined, fd, hxl, noisy, rows, with_  # noqa: E402

from opentunebox.advisor import noise as NZ  # noqa: E402
from opentunebox.advisor.noise import noise_report  # noqa: E402
from opentunebox.advisor.rules import Finding  # noqa: E402
from opentunebox.advisor.profiles import PROFILES  # noqa: E402
from opentunebox.log.settings import LowpassSetting, parse_settings  # noqa: E402

CASES: list = []
SEEN: dict[str, int] = {}
VERDICTS: dict[str, int] = {}


def hdr(lg, **kw):
    """Même journal avec des réglages d'en-tête remplacés (`None` retire la clé)."""
    raw = {**lg.settings.raw, **kw}
    return with_(lg, settings=parse_settings({k: v for k, v in raw.items() if v is not None}))


def enc_block(b):
    if b is None:
        return None
    return {"title": b.title, "settings": b.settings, "observations": list(b.observations), "findings": [fd(f) for f in b.findings],
            "verdict": None if b.verdict is None else fd(b.verdict)}


def enc_noise(nr) -> dict:
    ax = lambda x: {"axis": x.axis, "before": hx(x.before_db), "after": hx(x.after_db), "hz": hx(x.peak_hz),  # noqa: E731
                    "drop": hx(x.reduction_db)}
    fl = lambda v: [fd(f) for f in v]  # noqa: E731
    return {"band": [hx(nr.band[0]), hx(nr.band[1])], "moving": bool(nr.moving), "has_unfiltered": bool(nr.has_unfiltered),
            "gyro": [ax(x) for x in nr.gyro], "dterm": [ax(x) for x in nr.dterm],
            "harmonics": None if nr.harmonics is None else [hxl(nr.harmonics[0]), hxl(nr.harmonics[1])],
            "conditions": fl(nr.conditions), "gyro_findings": fl(nr.gyro_findings), "dterm_findings": fl(nr.dterm_findings),
            "strategy": fl(nr.strategy), "gyro_blocks": [enc_block(b) for b in nr.gyro_blocks], "gyro_other": fl(nr.gyro_other),
            "dterm_block": enc_block(nr.dterm_block),
            "report_findings": [f.key for f in nr.report.findings]}


def case(label, lg, sel=None, base="five"):
    """Un cas : `noise_report(lg, sel, profil)` de l'étalon (`base` : clé du profil imposé, sinon détecté)."""
    profile = None if base is None else PROFILES[base]
    rec = {"label": label, "log": {**enc_log(lg), "raw": dict(lg.settings.raw), "columns": sorted(lg.pid_columns)},
           "sel": None if sel is None else list(sel), "profile": base}
    try:
        nr = noise_report(lg, None if sel is None else slice(*sel), profile)
    except Exception as e:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        rec |= {"error": True, "out": None}
        print(f"  {label} : Python lève {type(e).__name__}: {e}", file=sys.stderr)
        CASES.append(rec)
        return None
    for fs in (nr.conditions, nr.gyro_findings, nr.dterm_findings, nr.strategy):
        for f in fs:
            SEEN[f.key] = SEEN.get(f.key, 0) + 1
    blocks = [*nr.gyro_blocks, nr.dterm_block]
    vs = [b.verdict.key for b in blocks if b.verdict is not None]
    for v in vs:
        VERDICTS[v] = VERDICTS.get(v, 0) + 1
    print(f"  {label} : verdicts {vs}, stratégie {[f.key for f in nr.strategy]}, autres {[f.key for f in nr.gyro_other]}", file=sys.stderr)
    rec |= {"error": False, "out": enc_noise(nr)}
    CASES.append(rec)
    return nr


def main():
    base = build()
    n0 = base.n

    # ---- filtre RPM
    case("base 250 Hz", base)
    case("profil détecté", base, base=None)
    case("sans gyro non filtré", build(unfilt=False))
    case("sans eRPM", build(rpm=False))
    case("sans eRPM, dshot_bidir activé", build(rpm=False, dshot_bidir="1"))
    case("sans gyro non filtré ni eRPM", build(unfilt=False, rpm=False))
    case("filtre RPM coupé (eRPM présent)", build(rpm_filter_harmonics="0", rpm_filter_weights=None))
    case("2 harmoniques, 2 poids", build(rpm_filter_harmonics="2", rpm_filter_weights="100,80"))
    case("3 harmoniques, 1 seul poids déclaré", build(rpm_filter_weights="90"))
    case("1 harmonique", build(rpm_filter_harmonics="1", rpm_filter_weights="100"))
    case("rpm_filter_q au maximum", build(rpm_filter_q="1000"))
    case("Betaflight 4.4", build(**{"Firmware revision": "Betaflight 4.4.2 (abcd1234) STM32F7X2"}))
    case("Betaflight 4.5 pile", build(**{"Firmware revision": "Betaflight 4.5.0 (abcd1234) STM32F7X2"}))
    case("version inconnue", build(**{"Firmware revision": "Betaflight (abcd1234)"}))
    slow = build(fs=1000.0, seconds=1.2, motor_hz=170.0, looptime="1000")
    case("journal trop lent : spectre replié", slow)
    case("journal trop lent, version 4.4", hdr(slow, **{"Firmware revision": "Betaflight 4.4.2 (abcd1234) STM32F7X2"}))

    # ---- notch dynamique (la boucle PID à 2 kHz le met en service)
    comb = combined()
    case("combiné : bruit moteur, oscillations 20–80 Hz, notch dynamique", comb)
    case("combiné, 1 notch", combined(count="1"))
    case("combiné, notch coupé (dyn_notch_count = 0)", combined(count="0"))
    case("combiné, plage du notch à 0", hdr(comb, dyn_notch_max_hz="0"))
    case("combiné, boucle PID à 1 kHz : notch inactif", hdr(comb, pid_process_denom="2"))
    case("combiné, sans eRPM", with_(comb, motor_hz=None))
    case("combiné, sans gyro non filtré", with_(comb, gyro_unfilt=None))
    case("combiné, filtre RPM coupé : notch et filtre RPM coupés", hdr(comb, rpm_filter_harmonics="0", rpm_filter_weights=None,
                                                                        dyn_notch_count="0"))
    case("combiné, filtre RPM coupé, notch actif", hdr(comb, rpm_filter_harmonics="0", rpm_filter_weights=None))
    for lo, hi in (("200", "300"), ("20", "35"), ("150", "1000")):
        case(f"combiné, plage {lo}–{hi} Hz", hdr(comb, dyn_notch_min_hz=lo, dyn_notch_max_hz=hi))
    case("combiné, sans mouvement, plage 200–300", hdr(build(fs=2000.0, seconds=3.0, motor_hz=43.0, looptime="500", moving=False),
                                                       dyn_notch_min_hz="200", dyn_notch_max_hz="300"))
    case("combiné, 7 notchs, plage 100–600", hdr(comb, dyn_notch_count="7"))
    # raie faible à 300 Hz dans le gyro non filtré seulement : pics à suivre sans constat (sous le seuil de la raie fixe), dont un sous la
    # plage du notch (« pic en dehors de la plage » en vol avec mouvements, simple détail sans mouvement)
    for amp in (0.1, 0.15):
        flat = build(fs=2000.0, seconds=3.0, motor_hz=43.0, looptime="500")
        u = flat.gyro_unfilt.astype(np.float64)
        u[:2] += amp * np.sin(2 * np.pi * 300.0 * flat.t)
        line = with_(flat, gyro_unfilt=u.astype(np.float32))
        for lo, hi, cnt in (("100", "250", "1"), ("100", "600", "3"), ("100", "600", "1"), ("50", "600", "1")):
            case(f"raie faible {amp}, {cnt} notch(s), plage {lo}–{hi} Hz", hdr(line, dyn_notch_min_hz=lo, dyn_notch_max_hz=hi,
                                                                              dyn_notch_count=cnt))
    still_flat = build(fs=2000.0, seconds=3.0, motor_hz=43.0, looptime="500", moving=False)
    u = still_flat.gyro_unfilt.astype(np.float64)
    u[:2] += 0.1 * np.sin(2 * np.pi * 300.0 * still_flat.t)
    case("raie faible, sans mouvement, pic hors plage", hdr(with_(still_flat, gyro_unfilt=u.astype(np.float32)), dyn_notch_min_hz="100",
                                                           dyn_notch_max_hz="250", dyn_notch_count="1"))
    # raie fixe qui ne suit pas les moteurs, gaz qui balayent la plage (400 Hz : l'observation de la raie du notch)
    for nm, kw in (("raie fixe 150 Hz + harmoniques", dict(line=(150.0, 1.0), harm={1: 0.5, 2: 0.4})),
                   ("raie fixe 90 Hz", dict(line=(90.0, 1.0))), ("harmoniques seules", dict(harm={1: 1.0, 2: 0.5, 3: 0.3}))):
        sw = sweep(**kw)
        case(f"balayage des gaz, {nm}", sw)
        case(f"balayage des gaz, {nm}, notch à 2 kHz et 150–600 Hz", hdr(sw, looptime="500", dyn_notch_min_hz="150"))

    # ---- passe-bas, bruit, stratégie
    case("sans mouvement", build(moving=False))
    case("sans mouvement ni eRPM", build(moving=False, rpm=False))
    case("passe-bas coupés", build(gyro_lpf1_dyn_hz="0,0", gyro_lpf2_static_hz="0", dterm_lpf1_static_hz="0",
                                   dterm_lpf1_dyn_hz="0,0", dterm_lpf2_static_hz="0"))
    case("passe-bas BIQUAD statiques", build(gyro_lpf1_type="2", gyro_lpf1_dyn_hz="0,0", gyro_lpf1_static_hz="300", gyro_lpf2_type="1",
                                             dterm_lpf1_type="2", dterm_lpf1_dyn_hz="0,0", dterm_lpf1_static_hz="100"))
    g1 = noisy(base, g=1.0)
    case("gyro bruité", g1)
    case("D bruité", noisy(base, d=10.0))
    case("gyro et D bruités", noisy(base, g=1.0, d=10.0))
    case("gyro bruité, sans eRPM ni gyro non filtré", noisy(build(unfilt=False, rpm=False), g=1.0, d=10.0))
    case("raie dans le D", noisy(base, tone_d=300.0))
    case("terme D nul partout", rows(base, "d_term", lambda a: a.__setitem__(slice(None), 0.0)))
    case("terme D nul sur le roll", rows(base, "d_term", lambda a: a.__setitem__((0, slice(None)), 0.0)))
    case("terme D nul sur le pitch", rows(base, "d_term", lambda a: a.__setitem__((1, slice(None)), 0.0)))

    def d_outside(a):
        a[0, :] = 0.0
        a[0, :150] = 0.5 * np.sin(np.arange(150))

    case("terme D nul dans la période seulement", rows(base, "d_term", d_outside), sel=(200, 900))
    case("gyro nul sur le lacet", rows(base, "gyro", lambda a: a.__setitem__((2, slice(None)), 0.0)))
    case("gyro nul partout", rows(base, "gyro", lambda a: a.__setitem__(slice(None), 0.0)))
    case("gyro non filtré nul", rows(base, "gyro_unfilt", lambda a: a.__setitem__(slice(None), 0.0)))
    case("bande de bruit vide (fs 150 Hz)", build(fs=150.0, seconds=4.0))
    t1 = base.t.copy()
    t1[600:] += 0.5
    case("un trou", with_(base, t=t1))
    t3 = base.t.copy()
    t3[200:] += 0.2
    t3[500:] += 0.3
    t3[750:] += 0.4
    case("trois trous, période explicite", with_(base, t=t3), sel=(100, 900))
    case("période explicite", base, sel=(125, 800))
    case("période dépassant le journal", base, sel=(150, n0 + 800))
    case("période hors du journal : Python lève", base, sel=(n0 + 5, n0 + 9))
    case("période vide : Python lève", base, sel=(0, 0))
    for k in ("whoop", "micro", "seven", "large"):
        case(f"profil imposé : {k}", noisy(base, g=0.5, d=3.0), base=k)

    # appels directs : rangement de chaque clé et de chaque catégorie, texte des passe-bas, repères de niveau
    keys = [*NZ.CONDITION_KEYS, *NZ.DTERM_KEYS, *NZ.GYRO_KEYS, NZ.STRATEGY_KEY, "step_ok", "crash", "motor_saturation", "unknown_key"]
    sections = [[k, c, NZ.section_of(Finding(k, c, "info", "t", "d"))]
                for k in keys for c in ("Enregistrement", "Bruit", "Filtres", "Mécanique", "Réponse", "Moteurs", "Réglage", "Inconnue")]
    lps = [LowpassSetting(), LowpassSetting("PT1", 150, 0, 0), LowpassSetting("BIQUAD", 0, 75, 150), LowpassSetting("PT2", 90, 100, 200),
           LowpassSetting("PT3", 0, 0, 300), LowpassSetting("PT1", 0, 250, 500, 3)]
    lowpass = [[{"type": lp.type, "static_hz": lp.static_hz, "dyn_min_hz": lp.dyn_min_hz, "dyn_max_hz": lp.dyn_max_hz, "expo": lp.expo},
                NZ.lowpass_text(lp)] for lp in lps]
    levels = [[k, NZ.level_note(k)] for k in ("gyro", "dterm", "autre")]
    out = {"cases": CASES, "inputs": INPUTS, "sections": sections, "lowpass": lowpass, "levels": levels,
           "keys": {"rpm": list(NZ.RPM_KEYS), "notch": list(NZ.NOTCH_KEYS), "gyro_lowpass": list(NZ.GYRO_LOWPASS_KEYS)}}
    text = json.dumps(out, separators=(",", ":"))
    Path(sys.argv[1]).write_text(text)
    print(len(CASES), "cas,", sum(1 for c in CASES if c["error"]), "erreurs de Python,", f"{len(text) / 1e6:.2f} Mo", file=sys.stderr)
    print("constats vus :", dict(sorted(SEEN.items())), file=sys.stderr)
    print("verdicts vus :", dict(sorted(VERDICTS.items())), file=sys.stderr)
    required = ("rpm_block_no_rpm", "rpm_block_off", "rpm_block_unmeasurable", "rpm_block_ok", "notch_block_ok", "notch_block_both_off",
                "notch_block_unjudged", "notch_block_peak_outside", "lowpass_block_ok")
    required_strat = ("filter_no_change", "filter_single_step", "filter_strategy")
    missing = [k for k in required if k not in VERDICTS] + [k for k in required_strat if k not in SEEN]
    print("jamais produits :", missing, file=sys.stderr)
    if missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
