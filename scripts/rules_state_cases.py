#!/usr/bin/env python3
"""Cas de test de `rules.analyze` sur des journaux fabriqués, pour `rust/otb-advisor/src/rules.rs` (phase 5a-2b).

Usage : rules_state_cases.py SORTIE.json    (à exécuter avec `.venv/bin/python`)

Complète `rules_cases.py` (fonctions sans état) et la couche corpus (`otb-parity check-advisor`) : des vols courts fabriqués
(signal lisse, bruit réglable) font atteindre les branches que le corpus ne couvre pas ou mal. Chaque cas appelle l'étalon Python
`analyze(log, sel, profile)` et garde, de son rapport, **seulement** les constats produits par les parties portées en 5a-2b
(`KEYS`) : `delay_rules`, `filter_rules` et `filter_strategy` (5a-3) sont comparés ailleurs (corpus : `otb-parity check-advisor`, appels
directs : `rules_delay_cases.py`, `rules_rpm_cases.py`, `rules_notch_cases.py`).

Familles de cas :
- enregistrement : `log_rate_low` / `log_rate_low_filters` / `log_rate_harmonics` (deux actions), `gyro_unfilt_missing`, `rpm_missing`,
  `recording_gaps` (1 puis plusieurs trous, période qui exclut le trou), `crash` ;
- réponse indicielle : `step_few_windows` (aucun axe, un seul axe lu), `step_roll_pitch_gap` (écart > 0, < 0, égal au seuil, juste
  sous le seuil), mode Angle (gain F coupé) ;
- bruit : `dterm_noisy` (corpus seul, PTB seul, les deux), `gyro_noisy`, `gyro_noise_level` OK et INFO, `dterm_noise_level`, terme D nul
  (liste `d_db` vide), nul sur un seul axe, nul seulement dans la période, bande vide (NaN), seuils posés 1e-9 dB de part et d'autre de la mesure (les niveaux viennent d'une FFT : Rust et Python diffèrent de ~1e-14 dB, l'égalité exacte n'a pas de sens) ;
- moteurs : `motor_saturation` (et égalité), `motor_imbalance` (6 géométries, sous le seuil, égalité), `iterm_offset` roll / pitch / les deux
  (et égalité) ;
- `sliders_off`, `profile_provisional`, profil détecté ou imposé, `setpoint_delay` en mode Angle et en Acro ;
- erreurs : période hors du journal (Python lève ⇒ Rust lève).

Un seuil de profil peut être remplacé (`profile.over`) pour tomber exactement sur la mesure (égalités). Un flottant s'écrit en
hexadécimal. Les courbes sont des float32 (D-029), comme dans un vrai journal. Le test `rust/otb-advisor/tests/rules_state.rs` exige
des textes identiques au caractère près, les mêmes décisions dans le même ordre, et les mesures identiques au bit près (retards et niveaux
de bruit : tolérance mesurée).
"""
import dataclasses
import json
import math
import sys
from pathlib import Path

import numpy as np
from bf_filters_cases import hx
from flight_stats_cases import INPUTS, enc_log
from scipy.signal import lfilter

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tests"))
from synthetic import BASE_HEADER  # noqa: E402

from opentunebox.advisor.profiles import PROFILES  # noqa: E402
from opentunebox.advisor.rules import analyze  # noqa: E402
from opentunebox.log.flightlog import ANGLE_MODE_MASK, FlightLog  # noqa: E402
from opentunebox.log.settings import parse_settings  # noqa: E402

# Constats des parties portées en 5a-2b
KEYS = ("profile_provisional", "log_rate_low", "log_rate_low_filters", "log_rate_harmonics", "gyro_unfilt_missing", "rpm_missing",
        "recording_gaps", "crash", "step_ff_overshoot", "step_overshoot_strong", "step_overshoot_light", "step_soft", "step_ok",
        "loop_sensitivity_high", "loop_phase_margin_low", "step_few_windows", "step_roll_pitch_gap", "setpoint_delay", "dterm_noisy",
        "gyro_noisy", "gyro_noise_level", "dterm_noise_level", "motor_saturation", "motor_imbalance", "iterm_offset", "sliders_off")

INF = float("inf")


def build(fs=250.0, seconds=4.0, seed=0, g_sigma=0.05, d_sigma=0.01, moving=True, unfilt=True, rpm=True, motor_hz=220.0, flags=1,
          **header) -> FlightLog:
    """Vol fabriqué (modèle de `tests/synthetic.py:make_flightlog`) : consigne en paliers lissés par 4 PT1 à 8 Hz, gyro = consigne dans un
    PT1 à 30 Hz retardé de 8 ms plus un bruit blanc d'écart-type `g_sigma`, terme D = dérivée lisse plus un bruit blanc d'écart-type
    `d_sigma`, gaz 35–65 %, quatre moteurs, télémétrie RPM à `motor_hz` × (0,7 + 0,6 × gaz) si `rpm`. Chaque composante a son propre
    générateur : activer ou couper `unfilt` / `rpm` ne change pas les autres tableaux (ils sont écrits une seule fois dans le fichier).
    À 250 Hz : gyro 0,05 → −50 dB, 1,0 → −24 dB (échelle de 500 Hz, à peu près) ; D 0,5 → −30 dB, 2,0 → −18 dB."""
    rng = lambda k: np.random.default_rng(seed * 100 + k)  # noqa: E731
    n = int(seconds * fs)
    t = np.arange(n) / fs
    sp = np.zeros((3, n))
    if moving:
        r = rng(0)
        for ax in (0, 1):
            k = int(0.2 * fs)
            while k < n:
                hold, gap = int(r.uniform(0.2, 0.4) * fs), int(r.uniform(0.15, 0.4) * fs)
                sp[ax, k:k + hold] = r.choice([-1.0, 1.0]) * r.uniform(100, 400)
                k += hold + gap
    w = 2 * np.pi * 30.0 / fs
    k1 = w / (w + 1.0)
    ws = 2 * np.pi * 8.0 / fs
    ks = ws / (ws + 1.0)
    for ax in range(3):
        for _ in range(4):  # 4 PT1 à 8 Hz : consigne lisse, pas de bruit large bande hérité des marches
            sp[ax] = lfilter([ks], [1.0, -(1.0 - ks)], sp[ax])
    sp *= 3.0
    delay = int(round(0.008 * fs))
    smooth = np.zeros_like(sp)
    for ax in range(3):
        y = lfilter([k1], [1.0, -(1.0 - k1)], sp[ax])
        smooth[ax, delay:] = y[: n - delay]
    throttle = 50.0 + 15.0 * np.sin(2 * np.pi * 0.1 * t)
    mhz_mean = motor_hz * (0.7 + 0.6 * throttle / 100.0)
    gyro = smooth + rng(1).normal(0, g_sigma, (3, n)) + 0.02 * np.sin(2 * np.pi * np.cumsum(mhz_mean) / fs)
    gyro_unfilt = (gyro + rng(2).normal(0, 4 * g_sigma, (3, n)) + 6.0 * g_sigma * np.sin(4 * np.pi * np.cumsum(mhz_mean) / fs)
                   if unfilt else None)
    r = rng(3)
    motors = np.clip(np.vstack([throttle + r.normal(0, 1.0, n) for _ in range(4)]), 0, 100)
    r = rng(4)
    motor_hz_arr = np.vstack([mhz_mean + r.normal(0, 1.0, n) for _ in range(4)]) if rpm else None
    err = sp - smooth
    p_term = 0.032029 * 45 * err
    i_term = np.cumsum(0.244381 * 80 * err / fs, axis=1)
    i_term -= i_term.mean(axis=1, keepdims=True)
    d_term = -0.000529 * 40 * np.gradient(smooth, axis=1) * fs + rng(5).normal(0, d_sigma, (3, n))
    d_term[2] = 0.0
    f_term = 0.013754 * 1.2 * np.gradient(sp, axis=1) * fs
    rc = np.vstack([sp / 2.0, 1000.0 + 10.0 * throttle])
    raw = {**BASE_HEADER, **header}
    raw = {k: v for k, v in raw.items() if v is not None}
    f = lambda x: None if x is None else np.asarray(x, np.float32)  # noqa: E731
    return FlightLog(
        Path("synth.bbl"), 1, parse_settings(raw), t, fs, f(gyro), f(gyro_unfilt), f(sp), f(rc), f(p_term), f(i_term), f(d_term),
        f(f_term), f(throttle), f(motors), f(motor_hz_arr), np.zeros((0, n), np.float32), None, None, None,
        None if flags is None else np.full(n, flags, np.int32), np.zeros((0, 4)), {},
        frozenset(f"axis{k}[{a}]" for k in "PIDF" for a in range(3)))


def noisy(lg: FlightLog, g=0.0, d=0.0, tone_d=0.0, tone_hz=100.0, seed=7) -> FlightLog:
    """Même vol avec du bruit ajouté au gyro (écart-type `g`) et au terme D (`d`, plus une raie `tone_d` à `tone_hz`) : seuls ces deux
    tableaux changent."""
    r = np.random.default_rng(seed)
    out = lg
    if g:
        out = with_(out, gyro=(lg.gyro + r.normal(0, g, lg.gyro.shape)).astype(np.float32))
    if d or tone_d:
        dt = lg.d_term + r.normal(0, d, lg.d_term.shape) + tone_d * np.sin(2 * np.pi * tone_hz * lg.t)
        dt[2] = 0.0
        out = with_(out, d_term=dt.astype(np.float32))
    return out


def with_(lg: FlightLog, **kw) -> FlightLog:
    return dataclasses.replace(lg, **kw)


def rows(lg: FlightLog, name: str, edit) -> FlightLog:
    """Copie de `lg` dont le tableau `name` (float32) est modifié en place par `edit(tableau)`."""
    a = getattr(lg, name).copy()
    edit(a)
    return with_(lg, **{name: a})


# ---------------------------------------------------------------- encodage

def fd(f) -> dict:
    return {"key": f.key, "category": f.category, "severity": f.severity, "title": f.title, "detail": f.detail,
            "actions": list(f.actions), "axis": f.axis, "settings": list(f.settings)}


def hxl(v) -> list:
    return [hx(x) for x in v]


def enc_measures(r, lg) -> dict:
    m = r.measures
    d = m["delays"]
    s = m["stats"]
    return {
        "hover_motor_hz": None if m["hover_motor_hz"] is None else hx(m["hover_motor_hz"]),
        "log_rate_needed_hz": hxl(m["log_rate_needed_hz"]), "aliased": bool(m["aliased"]),
        "steps": {ax: hxl([getattr(mt, k) for k in ("peak", "peak_ms", "latency_ms", "rise_ms", "settling_ms", "overshoot_pct", "ripple",
                                                    "ripple_std", "ripple_ms", "waves", "waves_ratio")])
                  for ax, mt in m["steps"].items()},
        "delays": {"gyro": hx(d.gyro_ms), "dterm": hx(d.dterm_ms), "sp": hxl(d.sp_gyro_ms), "stick": hx(d.stick_p95_dps)},
        "gyro_noise_db": hxl(m["gyro_noise_db"]), "dterm_noise_db": hxl(m["dterm_noise_db"]), "low_band_db": hxl(m["low_band_db"]),
        "gyro_peak_ptb_db": hx(m["gyro_peak_ptb_db"]), "dterm_peak_ptb_db": hx(m["dterm_peak_ptb_db"]),
        "stats": {"duration_s": hx(s.duration_s), "sat": hx(s.motor_saturation_pct), "imb": hx(s.motor_imbalance_pct),
                  "motors": [hxl([mm.mean, mm.sd, mm.max]) for mm in s.motors]},
        "pid_error_sd": hxl(m["pid_error_sd"]), "iterm_mean": hxl(m["iterm_mean"]),
        "sliders": {"values": [[k, hx(v)] for k, v in m["sliders"].values.items()], "from_header": bool(m["sliders"].from_header),
                    "consistent": bool(m["sliders"].consistent)},
    }


CASES: list = []
SEEN: dict[str, int] = {}


def case(label, lg, sel=None, base=None, over=None):
    """Un cas : `analyze(lg, sel, profil)` de l'étalon. `base` : clé du profil imposé (sinon détecté) ; `over` : seuils remplacés."""
    over = over or {}
    profile = None
    if base is not None:
        profile = dataclasses.replace(PROFILES[base], **over)
    rec = {"label": label, "log": {**enc_log(lg), "raw": dict(lg.settings.raw), "columns": sorted(lg.pid_columns)},
           "sel": None if sel is None else list(sel),
           "profile": None if base is None else {"base": base, "over": {k: ([hx(v[0]), hx(v[1])] if isinstance(v, tuple) else hx(v))
                                                                          for k, v in over.items()}}}
    try:
        r = analyze(lg, None if sel is None else slice(*sel), profile)
    except Exception as e:  # noqa: BLE001 (l'erreur est elle-même la valeur attendue)
        rec |= {"error": True, "out": None}
        print(f"  {label} : Python lève {type(e).__name__}", file=sys.stderr)
        CASES.append(rec)
        return None
    kept = [f for f in r.findings if f.key in KEYS]
    print(f"  {label} : " + ", ".join(f"{f.key}" for f in kept if f.key not in ("step_ok", "setpoint_delay")), file=sys.stderr)
    srt = [next(i for i, g in enumerate(kept) if g is f) for f in r.sorted() if f.key in KEYS]  # rang dans `findings`
    for f in kept:
        SEEN[f.key] = SEEN.get(f.key, 0) + 1
    rec |= {"error": False, "out": {
        "profile": r.profile.key, "reason": r.profile_reason, "period": [hx(r.period[0]), hx(r.period[1])],
        "findings": [fd(f) for f in kept], "sorted": srt, "steps": [st.axis for st in r.steps],
        "measures": enc_measures(r, lg)}}
    CASES.append(rec)
    return r


def measure(lg, base="five", sel=None):
    """Mesures de l'étalon (pour poser un seuil exactement sur elles)."""
    return analyze(lg, None if sel is None else slice(*sel), PROFILES[base]).measures


def up(x):
    return math.nextafter(x, INF)


def down(x):
    return math.nextafter(x, -INF)


def main():
    five = "five"
    base = build()  # 250 Hz, 4 s, très calme : log_rate_low, log_rate_harmonics (PID à 8 kHz), fenêtres de la réponse indicielle
    n0 = base.n

    # ---- enregistrement
    case("base 250 Hz, profil détecté", base)
    case("base 250 Hz, profil five", base, base=five)
    case("période explicite", base, sel=(125, 800), base=five)
    case("période dépassant le journal", base, sel=(150, n0 + 800), base=five)
    case("période hors du journal : Python lève", base, sel=(n0 + 5, n0 + 9), base=five)
    case("période vide au début : Python lève", base, sel=(0, 0), base=five)
    # 1 kHz : 950 Hz = 0,95 × 1 kHz est le seuil de log_rate_low ; PID à 1 kHz puis à 8 kHz
    short = build(fs=1000.0, seconds=1.2, motor_hz=170.0, looptime="1000")  # 3 × 170 = 510 Hz > fs / 2 à 1 kHz, pas à 1,1 kHz
    case("1 kHz, PID 1 kHz, moteurs rapides : harmoniques, boucle PID trop lente", short, base=five)
    case("1,1 kHz, PID 1 kHz : aucun constat de fréquence", with_(short, fs=1100.0), base=five)
    short8 = with_(short, settings=parse_settings({**short.settings.raw, "looptime": "125"}))
    case("1 kHz, PID 8 kHz : log_rate_low_filters + harmoniques à enregistrer plus vite", short8, base=five)
    case("fs = 950 pile (seuil 0,95 × 1 kHz, `<` strict)", with_(short8, fs=950.0), base=five)
    case("fs juste sous 950 : log_rate_low", with_(short8, fs=down(950.0)), base=five)
    case("1,8 kHz, PID 8 kHz : sous le seuil des filtres (0,95 × 2 kHz = 1,9 kHz)", with_(short8, fs=1800.0), base=five)
    case("1,9 kHz pile : pas de constat de filtres", with_(short8, fs=1900.0), base=five)
    # 3 × fréquence moteur = fs / 2 exactement (moteurs à 150,0 Hz partout, fs 900 Hz) : `>` strict, pas de constat ; un ulp sous : constat
    const_hz = rows(short, "motor_hz", lambda a: a.__setitem__(slice(None), 150.0))
    case("harmoniques : 3 × fréquence moteur = fs / 2 pile", with_(const_hz, fs=900.0), base=five)
    case("harmoniques : fs un ulp sous 900 Hz", with_(const_hz, fs=down(900.0)), base=five)
    # boucle PID à 900 Hz, fs 890 Hz : 3 × 150 = 450 > 445, harm_hz = 900 est entre fs et 1,05 × fs (934,5) : « boucle PID trop lente »
    slow_pid = with_(const_hz, fs=890.0, settings=parse_settings({**const_hz.settings.raw, "looptime": "1111"}))
    case("harmoniques : harm_hz entre fs et 1,05 × fs", slow_pid, base=five)
    case("sans gyro non filtré", build(unfilt=False), base=five)
    case("sans eRPM", build(rpm=False), base=five)
    case("sans gyro non filtré ni eRPM", build(unfilt=False, rpm=False), base=five)
    t1 = base.t.copy()
    t1[600:] += 0.5
    case("un trou", with_(base, t=t1), base=five)
    t3 = base.t.copy()
    t3[200:] += 0.2
    t3[500:] += 0.3
    t3[750:] += 0.4
    case("trois trous", with_(base, t=t3), base=five)
    case("trou hors de la période", with_(base, t=t1), sel=(0, 500), base=five)
    # fs = 256 Hz (puissance de 2) : les temps sont exacts en binaire, un pas de 4 périodes vaut pile le seuil 4 / fs (`>` strict)
    tb = np.arange(n0) / 256.0
    tb[600:] += 3.0 / 256.0
    case("trou de 4 périodes de pas pile (fs 256 Hz) : aucun trou", with_(base, fs=256.0, t=tb), base=five)
    tb2 = np.arange(n0) / 256.0
    tb2[600:] += 3.0 / 256.0 + 1e-9
    case("trou de 4 périodes de pas + 1e-9 s (fs 256 Hz) : un trou", with_(base, fs=256.0, t=tb2), base=five)
    t5 = base.t.copy()
    t5[600:] += 3.5 / base.fs
    case("trou de 4,5 périodes de pas : un trou", with_(base, t=t5), base=five)
    cr = rows(base, "gyro", lambda a: a.__setitem__((0, slice(500, 530)), 2500.0))
    case("crash (gyro au-delà de 1 900 °/s)", cr, base=five)
    cr2 = rows(base, "gyro", lambda a: a.__setitem__((0, slice(500, 510)), 2500.0))
    case("pic de gyro trop court pour un crash", cr2, base=five)

    # ---- réponse indicielle
    still = build(moving=False)
    case("sans mouvement : step_few_windows", still, base=five)
    case("sans mouvement, profil détecté", still)
    roll_only = rows(base, "setpoint", lambda a: a.__setitem__((1, slice(None)), 0.0))
    roll_only = rows(roll_only, "gyro", lambda a: a.__setitem__((1, slice(None)), 0.0))
    case("roll seul en mouvement : aucun constat d'écart", roll_only, base=five)
    # écart roll / pitch : seuil à 0 (tout écart non nul se déclare), puis pile sur l'écart mesuré, puis juste dessous
    st = measure(base)["steps"]
    d0 = st["Pitch"].peak - st["Roll"].peak
    gap = abs(d0)
    print(f"  écart pitch − roll de la base : {d0!r}", file=sys.stderr)
    case("écart roll / pitch, seuil 0", base, base=five, over={"roll_pitch_gap_high": 0.0})
    case("écart roll / pitch, seuil = écart (pas de constat)", base, base=five, over={"roll_pitch_gap_high": gap})
    case("écart roll / pitch, juste sous l'écart", base, base=five, over={"roll_pitch_gap_high": down(gap)})
    case("écart roll / pitch, seuil du profil", base, base=five)

    def swap01(a):
        a[[0, 1]] = a[[1, 0]]

    sw = base
    for nm in ("setpoint", "gyro", "rc_command", "p_term", "i_term", "d_term", "f_term"):
        sw = rows(sw, nm, swap01)
    case("axes échangés (écart de signe contraire), seuil 0", sw, base=five, over={"roll_pitch_gap_high": 0.0})
    case("axes échangés, seuil = écart", sw, base=five, over={"roll_pitch_gap_high": gap})
    angle = build(flags=1 | ANGLE_MODE_MASK)
    case("journal en mode Angle", angle, base=five)
    case("modes de vol non enregistrés", build(flags=None), base=five)
    case("termes PID non enregistrés (boucle approchée)", with_(base, pid_columns=frozenset()), base=five)

    # ---- bruit (le gyro et le terme D reçoivent du bruit en plus : un seul tableau change)
    case("bruit moyen : gyro OK", noisy(base, g=0.2), base=five)
    gmax = max(measure(base)["gyro_noise_db"])
    # niveaux issus d'une FFT : Rust et Python diffèrent de ~1e-14 dB, un seuil posé à l'ulp près n'a pas de sens ; marge de 1e-9 dB
    case("gyro : seuil 1e-9 dB au-dessus de la mesure (pas de constat bruité)", base, base=five, over={"gyro_noise_high_db": gmax + 1e-9})
    case("gyro : seuil 1e-9 dB sous la mesure (bruité)", base, base=five, over={"gyro_noise_high_db": gmax - 1e-9})
    g1 = noisy(base, g=1.0)
    print(f"  gyro σ=1 : {measure(g1)['gyro_noise_db']}, sommet {measure(g1)['gyro_peak_ptb_db']:.2f}", file=sys.stderr)
    case("gyro_noise_level INFO (sommet au-dessus du repère, moyenne sous le seuil)", g1, base=five, over={"gyro_noise_high_db": -20.0})
    case("gyro bruité", g1, base=five)
    d2 = noisy(base, d=2.0)
    case("D bruité : corpus seul", d2, base=five, over={"dterm_noise_high_db": -20.0})
    case("D bruité : corpus et PTB", noisy(base, d=10.0), base=five)
    for tone in (300.0,):
        dt = noisy(base, tone_d=tone)
        r = measure(dt)
        print(f"  raie D {tone} à 100 Hz : moyenne {r['dterm_noise_db']}, sommet {r['dterm_peak_ptb_db']:.2f}", file=sys.stderr)
        case(f"D : raie de {tone} à 100 Hz", dt, base=five)
    dmax = max(measure(d2)["dterm_noise_db"])
    case("D : seuil 1e-9 dB au-dessus de la mesure (pas de constat corpus)", d2, base=five, over={"dterm_noise_high_db": dmax + 1e-9})
    case("D : seuil 1e-9 dB sous la mesure", d2, base=five, over={"dterm_noise_high_db": dmax - 1e-9})
    case("terme D nul partout : d_db vide", rows(base, "d_term", lambda a: a.__setitem__(slice(None), 0.0)), base=five)
    case("terme D nul sur le pitch seulement", rows(base, "d_term", lambda a: a.__setitem__((1, slice(None)), 0.0)), base=five)
    case("terme D nul sur le roll seulement", rows(base, "d_term", lambda a: a.__setitem__((0, slice(None)), 0.0)), base=five)

    def d_outside(a):
        a[0, :] = 0.0
        a[0, :150] = 0.5 * np.sin(np.arange(150))

    case("terme D nul dans la période seulement (non nul ailleurs)", rows(base, "d_term", d_outside), sel=(200, 900), base=five)
    # signaux constants : le niveau est le plancher EPS de `to_db`, exact des deux côtés (FFT de zéros) : seuils posés pile sur ce niveau
    silent = rows(base, "gyro", lambda a: a.__setitem__(slice(None), 0.0))
    silent = rows(silent, "d_term", lambda a: a.__setitem__((slice(0, 2), slice(None)), 1.0))
    ms = measure(silent)
    c_g, c_d = max(ms["gyro_noise_db"]), max(ms["dterm_noise_db"])
    print(f"  signaux constants : niveaux {c_g!r} {c_d!r}", file=sys.stderr)
    case("signaux constants, seuils = niveaux : ni gyro ni D bruités", silent, base=five,
         over={"gyro_noise_high_db": c_g, "dterm_noise_high_db": c_d})
    case("signaux constants, seuils un ulp sous les niveaux : gyro et D bruités", silent, base=five,
         over={"gyro_noise_high_db": down(c_g), "dterm_noise_high_db": down(c_d)})
    # retard consigne → gyro : seul le lacet bouge, roll et pitch sont NaN : aucun constat (le lacet ne compte pas)
    def to_yaw(a):
        a[2] = a[0]
        a[0] = 0.0
        a[1] = 0.0

    yaw = rows(rows(base, "setpoint", to_yaw), "gyro", to_yaw)
    case("seul le lacet bouge : pas de retard consigne → gyro", yaw, base=five)
    nb = build(fs=150.0, seconds=4.0)
    case("bande de bruit vide (fs 150 Hz : 80 Hz au-delà de 0,95 × Nyquist)", nb, base=five)

    # ---- moteurs
    sat = rows(base, "motors", lambda a: a.__setitem__((0, slice(None, None, 10)), 100.0))
    s_pct = measure(sat)["stats"].motor_saturation_pct
    case("saturation des moteurs", sat, base=five)
    case("saturation : seuil = mesure", sat, base=five, over={"saturation_high_pct": s_pct})
    case("saturation : seuil juste sous la mesure", sat, base=five, over={"saturation_high_pct": down(s_pct)})
    offsets = {"avant": (0, 30, 0, 30), "arrière": (30, 0, 30, 0), "gauche": (0, 0, 30, 30), "droite": (30, 30, 0, 0),
               "diagonale M1/M4": (30, 0, 0, 30), "diagonale M2/M3": (0, 30, 30, 0), "un seul moteur": (40, 0, 0, 0),
               "sous le seuil": (3, 0, 0, 0)}
    for nm, off in offsets.items():
        lg = rows(base, "motors", lambda a, off=off: a.__iadd__(np.array(off, np.float32)[:, None]))
        case(f"déséquilibre des moteurs : {nm}", lg, base=five)
    imb_lg = rows(base, "motors", lambda a: a.__iadd__(np.array((0, 30, 0, 30), np.float32)[:, None]))
    imb = measure(imb_lg)["stats"].motor_imbalance_pct
    case("déséquilibre : seuil = mesure", imb_lg, base=five, over={"imbalance_high_pct": imb})
    case("déséquilibre : seuil juste sous la mesure", imb_lg, base=five, over={"imbalance_high_pct": down(imb)})
    for nm, off in (("roll", (60, 0, 0)), ("pitch", (0, -60, 0)), ("roll et pitch", (-70, 55, 0)), ("lacet seul", (0, 0, 90)),
                    ("sous le seuil", (20, -20, 0))):
        lg = rows(base, "i_term", lambda a, off=off: a.__iadd__(np.array(off, np.float32)[:, None]))
        case(f"terme I décentré : {nm}", lg, base=five)
    i_lg = rows(base, "i_term", lambda a: a.__iadd__(np.array((60, 0, 0), np.float32)[:, None]))
    im = measure(i_lg)["iterm_mean"]
    case("terme I : seuil = |moyenne| (pas de constat)", i_lg, base=five, over={"iterm_offset_high": abs(im[0])})
    case("terme I : seuil juste sous |moyenne|", i_lg, base=five, over={"iterm_offset_high": down(abs(im[0]))})

    # ---- curseurs, profils, retard consigne
    case("curseurs désactivés (PID saisis à la main)", build(simplified_pids_mode="0"), base=five)
    case("curseurs désactivés, D max (convention 2025.12)",
         build(simplified_pids_mode="0", d_min=None, d_max="40,42,0", **{"Firmware revision": "Betaflight 2025.12.0 (abcd1234) STM32H743"}),
         base=five)
    for k in ("whoop", "micro", "seven", "large"):
        case(f"profil imposé : {k}", base, base=k)
    case("profil provisoire et seuil de D", d2, base="micro")
    case("retard consigne → gyro en Acro", base, base=five)
    case("retard consigne → gyro, journal en mode Angle", angle, base=five)

    out = {"keys": list(KEYS), "cases": CASES, "inputs": INPUTS}
    text = json.dumps(out, separators=(",", ":"))
    Path(sys.argv[1]).write_text(text)
    print(len(CASES), "cas,", sum(1 for c in CASES if c["error"]), "erreurs de Python,", len(INPUTS), "entrées,",
          f"{len(text) / 1e6:.2f} Mo", file=sys.stderr)
    print("constats vus :", dict(sorted(SEEN.items())), file=sys.stderr)
    # étudiés par `rules_cases.py` (textes) et par le corpus (taux) : un vol fabriqué ne vise pas leurs seuils de réponse indicielle
    absent_ok = {"step_ff_overshoot", "step_overshoot_strong", "step_overshoot_light", "step_soft", "loop_phase_margin_low"}
    missing = [k for k in KEYS if k not in SEEN and k not in absent_ok]
    print("constats jamais produits :", missing, file=sys.stderr)
    if missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
