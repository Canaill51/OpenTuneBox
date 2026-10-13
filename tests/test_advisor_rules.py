"""Fonctions pures de l'assistant (profils, règles), sans corpus : tournent aussi en intégration continue."""

from types import SimpleNamespace

import numpy as np
import pytest

from opentunebox.advisor import profiles


@pytest.mark.parametrize("name, key", [
    ("Meteor65", "whoop"),
    ("Air 65", "whoop"),
    ("Mobula7 whoop", "whoop"),
    ("Apex 5\"", "five"),
    ("Chimera 7 pouces", "seven"),
    ("Mario 10in", "large"),  # deux chiffres : pas « 0 pouce »
    ("3.5 inch", "micro"),
    ("Source One 1750", "five"),  # « 1750 » (capacité) ne contient pas un whoop « 75 »
    ("", "five"),
])
def test_detect_profile_from_craft_name(monkeypatch, name, key):
    """Sans fréquence moteur ni cellules, la taille vient du nom du drone ; 5 pouces par défaut."""
    monkeypatch.setattr(profiles, "hover_motor_hz", lambda _log: None)
    log = SimpleNamespace(cells=None, settings=SimpleNamespace(craft_name=name))
    prof, why = profiles.detect_profile(log)
    assert prof.key == key
    assert (name in why) if name and "1750" not in name else ("par défaut" in why)


def test_profile_key_by_motor_speed_and_cells():
    from opentunebox.advisor.profiles import profile_key

    assert [profile_key(h) for h in (480, 350, 300, 220, 165, 140, 100)] == \
        ["whoop", "whoop", "micro", "five", "five", "seven", "large"]
    # cellules : 1S = whoop ; petit drone en 2–4S au-dessus de 200 Hz ; rien pour le 6S (recouvrement)
    assert [profile_key(h, c) for h, c in ((306, 1), (237, 4), (195, 4), (255, 6), (242, 2))] == \
        ["whoop", "micro", "five", "five", "micro"]


def test_filter_delay_targets():
    from opentunebox.advisor.profiles import filter_delay_target_ms

    # graphique de Brian White : 1 ms à 2", 4 ms (gyro) et 3 ms (D) à 14"
    assert filter_delay_target_ms(5) == (1.75, 1.5)
    assert filter_delay_target_ms(14) == (4.0, 3.0)
    assert filter_delay_target_ms(None) is None and filter_delay_target_ms(1.6) is None


def test_filter_strategy_needs_two_stages():
    from opentunebox.advisor.rules import CONSEIL, Finding, filter_strategy

    rate = Finding("Filtres", CONSEIL, "Fréquence d'enregistrement trop basse", "", ["blackbox_sample_rate = 1/2"])
    rpm = Finding("Filtres", CONSEIL, "Poids du filtre RPM trop bas", "", ["rpm_filter_weights = 100,100,100"])
    assert filter_strategy([rate]) is None and filter_strategy([rpm]) is None  # le constat seul suffit
    s = filter_strategy([rpm, rate])
    assert s.actions[0].startswith("1. Enregistrement") and s.actions[1].startswith("2. Filtre RPM")


def test_log_rates_needed():
    from opentunebox.advisor.rules import log_rate_needed, pid_log_rate_needed

    # filtres : 8 kHz de gyro (PID 4 ou 8 kHz) → 2 kHz ; 3,2 kHz → 1,6 kHz ; whoop à 480 Hz → 3e harmonique visible
    assert log_rate_needed(4000) == 2000 and log_rate_needed(8000) == 2000 and log_rate_needed(3200) == 1600
    assert log_rate_needed(4000, 480) == 4000 and log_rate_needed(3200, 210) == 1600
    # séries PID : 1 kHz suffit (1,6 kHz pour une boucle à 3,2 kHz, faute de fraction plus proche)
    assert pid_log_rate_needed(8000) == 1000 and pid_log_rate_needed(4000) == 1000 and pid_log_rate_needed(3200) == 1600


def test_imbalance_interpretation():
    from opentunebox.advisor.rules import _imbalance_finding

    front = _imbalance_finding([20, 30, 20, 30], 20)
    assert "avant" in front.detail
    diag = _imbalance_finding([30, 20, 20, 30], 20)
    assert "diagonale" in diag.detail


def test_noise_report_classifies_every_title():
    """Chaque constat de bruit ou de filtre de l'assistant a sa place dans le diagnostic de la fenêtre Spectre."""
    import re
    from pathlib import Path

    from opentunebox.advisor import noise, rules
    from opentunebox.advisor.rules import CONSEIL, Finding

    src = Path(rules.__file__).read_text(encoding="utf-8")
    titles = re.findall(r'Finding\("(Bruit|Filtres|Mécanique|Réponse)",\s*[^,]+,\s*f?"([^"{]*)', src)
    explicit = noise.CONDITION_TITLES + noise.DTERM_TITLES + noise.GYRO_TITLES + (noise.STRATEGY_TITLE,)
    for cat, title in titles:
        f = Finding(cat, CONSEIL, title, "")
        if cat in ("Réponse", "Mécanique") and not title.startswith(noise.GYRO_TITLES):
            assert noise.section_of(f) is None, title
        else:
            assert title.startswith(explicit), f"constat non rangé : {title}"
    assert noise.section_of(Finding("Filtres", CONSEIL, "Retard des filtres D", "")) == "dterm"
    assert noise.section_of(Finding("Filtres", CONSEIL, "Retard des filtres gyro", "")) == "gyro"
    assert noise.section_of(Finding("Enregistrement", CONSEIL, "Gyro non filtré absent", "")) == "conditions"


def test_static_notch_cutoff():
    """Coupure de gyro_notch1 tirée du Q nécessaire (bornes du notch dynamique), Q = f·c / (f² − c²) du firmware."""
    from opentunebox.advisor.rules import dyn_notch_q_need, static_notch_cutoff

    assert dyn_notch_q_need([200.0], [20.0], [15.0]) == pytest.approx(294, abs=1)  # arrondi à 250 : Q = 2,5
    c, measured = static_notch_cutoff(200.0, 20.0, 15.0)
    assert measured and c == 163  # 200 × (√26 − 1) / 5 = 163,96, arrondi vers le bas
    assert 200 * c / (200 ** 2 - c ** 2) == pytest.approx(2.5, abs=0.1)  # filterGetNotchQ (common/filter.c)
    assert static_notch_cutoff(200.0, 2.0, 30.0) == (179, True)  # raie fine : Q plafonné à 4,5 (0,89 × f)
    assert static_notch_cutoff(200.0, 200.0, 30.0) == (123, True)  # raie large : Q 1 au moins (0,62 × f)
    assert static_notch_cutoff(200.0, float("nan"), 15.0) == (140, False)  # largeur non mesurée : 0,7
    assert static_notch_cutoff(200.0, 20.0, 3.0) == (140, False)  # raie sous 4 dB dans le spectre moyen


def test_dyn_notch_q_need():
    """Q du notch qui ramène la raie à 4 dB du bruit voisin sur sa demi-largeur (notch SVF, Q × 100)."""
    import math

    from opentunebox.advisor.rules import _q_round, dyn_notch_q_need

    # raie à 200 Hz, 20 Hz de large, 15 dB : atténuation de 11 dB à ±10 Hz
    h = 10 ** (-11 / 20)
    q = dyn_notch_q_need([200.0], [20.0], [15.0])
    assert q == pytest.approx(100 * 200 * h / (20 * math.sqrt(1 - h * h)))
    assert _q_round(q) == 250
    assert dyn_notch_q_need([200.0], [20.0], [3.0]) == float("inf")  # déjà sous 4 dB : aucun besoin
    assert dyn_notch_q_need([200.0], [float("nan")], [15.0]) is None
    assert _q_round(40.0) == 100  # borne basse


# ---------------------------------------------------------------- seuils des profils (D-011)


def test_profile_key_boundaries():
    from opentunebox.advisor.profiles import FIVE_HZ, MICRO_HZ, SEVEN_HZ, SMALL_PACK_HZ, WHOOP_HZ, profile_key

    assert (WHOOP_HZ, MICRO_HZ, FIVE_HZ, SEVEN_HZ, SMALL_PACK_HZ) == (345, 290, 160, 115, 200)
    assert profile_key(345) == "whoop" and profile_key(344.9) == "micro"
    assert profile_key(290) == "micro" and profile_key(289.9) == "five"
    assert profile_key(160) == "five" and profile_key(159.9) == "seven"
    assert profile_key(115) == "seven" and profile_key(114.9) == "large"
    assert profile_key(200, 4) == "micro" and profile_key(199.9, 4) == "five" and profile_key(250, 5) == "five"
    assert profile_key(100, 1) == "whoop"


def test_profiles_snapshot():
    """Instantané des seuils de chaque profil : une modification volontaire passe par la compétence
    `assistant-seuils` et docs/assistant.md (« Calibration »), et met ce test à jour."""
    import dataclasses

    from opentunebox.advisor.profiles import PROFILES

    expected = {
        "whoop": dict(hover_hz=(345, 900), resonance_hz=(250, 600), latency_ms=(13, 20), gyro_delay_max_ms=1.5,
                      dterm_delay_max_ms=2.5, gyro_noise_high_db=-28.0, dterm_noise_high_db=-14.7, low_band_high_db=-13.9,
                      motor_left_high_db=-32.2, imbalance_high_pct=13.5, iterm_offset_high=33.0, saturation_high_pct=1.5,
                      roll_pitch_gap_high=0.12, resonance_high_db=7.9, provisional=True, nominal_inch=None),
        "micro": dict(hover_hz=(200, 345), resonance_hz=(180, 450), latency_ms=(10, 21), gyro_delay_max_ms=1.8,
                      dterm_delay_max_ms=2.6, gyro_noise_high_db=-29.4, dterm_noise_high_db=-14.8, low_band_high_db=-13.9,
                      motor_left_high_db=-32.5, imbalance_high_pct=13.5, iterm_offset_high=33.0, saturation_high_pct=1.5,
                      roll_pitch_gap_high=0.12, resonance_high_db=7.9, provisional=True, nominal_inch=3),
        "five": dict(hover_hz=(160, 290), resonance_hz=(120, 350), latency_ms=(10, 22), gyro_delay_max_ms=1.9,
                     dterm_delay_max_ms=2.7, gyro_noise_high_db=-28.3, dterm_noise_high_db=-13.1, low_band_high_db=-15.7,
                     motor_left_high_db=-28.4, imbalance_high_pct=14.0, iterm_offset_high=32, saturation_high_pct=1.5,
                     roll_pitch_gap_high=0.11, resonance_high_db=6.8, provisional=False, nominal_inch=5),
        "seven": dict(hover_hz=(115, 160), resonance_hz=(80, 220), latency_ms=(15, 24), gyro_delay_max_ms=2.6,
                      dterm_delay_max_ms=2.8, gyro_noise_high_db=-31.4, dterm_noise_high_db=-17.5, low_band_high_db=-21.7,
                      motor_left_high_db=-31.8, imbalance_high_pct=11.2, iterm_offset_high=31, saturation_high_pct=1.5,
                      roll_pitch_gap_high=0.08, resonance_high_db=8.2, provisional=False, nominal_inch=7),
        "large": dict(hover_hz=(50, 125), resonance_hz=(40, 160), latency_ms=(18, 30), gyro_delay_max_ms=3.5,
                      dterm_delay_max_ms=3.1, gyro_noise_high_db=-31.0, dterm_noise_high_db=-17.0, low_band_high_db=-22.0,
                      motor_left_high_db=-32.0, imbalance_high_pct=13.5, iterm_offset_high=33.0, saturation_high_pct=1.5,
                      roll_pitch_gap_high=0.12, resonance_high_db=7.9, provisional=True, nominal_inch=8),
    }
    assert list(PROFILES) == list(expected)
    for key, p in PROFILES.items():
        got = {k: v for k, v in dataclasses.asdict(p).items() if k not in ("key", "label")}
        assert got == expected[key], key
        assert p.key == key and p.label
        assert p.hover_hz[0] < p.hover_hz[1] and p.resonance_hz[0] < p.resonance_hz[1] and p.latency_ms[0] < p.latency_ms[1]
        assert p.gyro_noise_high_db < 0 and p.dterm_noise_high_db < 0 and p.low_band_high_db < 0 and p.motor_left_high_db < 0
        assert p.provisional == (key in {"whoop", "micro", "large"})


def test_rule_constants_snapshot():
    """Seuils communs des règles et du diagnostic des séries (docs/assistant.md, « Origine des seuils »)."""
    from opentunebox.advisor import log_check, rules, step_tuning

    assert (rules.SENSITIVITY_HIGH_DB, rules.PHASE_MARGIN_LOW_DEG, rules.D_DOMINANT_RATIO) == (7.0, 35.0, 1.0)
    assert (rules.STEP_PEAK_HIGH, rules.STEP_PEAK_HIGH_FF, rules.STEP_PEAK_FLAT) == ((1.15, 1.25), (1.4, 1.55), 1.03)
    assert (rules.STEP_PEAK_SOFT, rules.STEP_MIN_WINDOWS, rules.STEP_SURE_WINDOWS) == (1.01, 20, 200)
    assert (rules.DTERM_PTB_MAX_DB, rules.DTERM_PTB_IDEAL_DB, rules.GYRO_PTB_MAX_DB) == (-10.0, -20.0, -30.0)
    assert (rules.RPM_Q_STEP, rules.RPM_Q_MAX, rules.RPM_Q_MIN) == (100, 1000, 250)
    assert (rules.RPM_WEIGHT_MARGIN_DB, rules.RPM_WEIGHT_GAIN_MIN, rules.HARMONIC_VISIBLE_DB) == (3.0, 0.5, 4.0)
    assert (rules.DYN_NOTCH_Q_RPM, rules.DYN_NOTCH_Q_STEP, rules.DYN_NOTCH_Q_LOW) == ((300, 450), 50, 100)
    assert (rules.DYN_NOTCH_MIN_HZ, rules.DYN_NOTCH_MARGIN_HZ) == (20, 20)
    assert not hasattr(rules, "DYN_NOTCH_MIN_STEP_HZ")  # relèvement de dyn_notch_min_hz retiré (gain mesuré ≈ 0)
    assert (rules.LOG_RATE_MIN_HZ, rules.PID_LOG_RATE_MIN_HZ, rules.PID_LOG_RATE_FLOOR_HZ) == (1600.0, 1000.0, 500.0)
    assert rules.STRATEGY_MIN_STAGES == 2 and rules.LOG_RATE_FRACTIONS == (1, 2, 4, 8, 16)
    assert (step_tuning.SERIES_STEP, step_tuning.STEP_TOO_FINE, step_tuning.I_SLIDER_RANGE) == (0.2, 0.15, (0.15, 0.35))
    assert (step_tuning.MIN_WINDOWS, step_tuning.SURE_WINDOWS) == (20, 200)
    assert (step_tuning.PEAK_FLAT_MAX, step_tuning.PEAK_PLATEAU, step_tuning.PEAK_PLATEAU_MAX, step_tuning.PITCH_PEAK_GAP) == \
        (1.05, 0.02, 1.1, 0.02)
    assert (step_tuning.RIPPLE_EARLY, step_tuning.RIPPLE_PLATEAU, step_tuning.RIPPLE_HIGH, step_tuning.RIPPLE_STRONG,
            step_tuning.RIPPLE_GROWTH) == (0.03, 0.05, 0.065, 0.1, 0.02)
    assert (step_tuning.LATENCY_PLATEAU_PCT, step_tuning.LATENCY_GAP_OK_MS, step_tuning.LATENCY_GAP_MAX_MS,
            step_tuning.WAVES_RATIO_HIGH) == (4.0, 1.0, 2.0, 2.5)
    assert (log_check.MIN_VERSION, log_check.MIN_DURATION_S, log_check.LOG_RATE_DPS, log_check.LOG_ANGLE_LIMIT) == \
        ((4, 4), 15.0, 200, 30)
    assert (log_check.ANGLE_SHARE_MIN, log_check.AXES_MIXED_MIN, log_check.DYN_DAMPING_MAX, log_check.D_DELAY_PTB_OFFSET_MS) == \
        (0.9, 0.4, 0.05, 0.5)
    assert log_check.ROLE_ITEMS == {"filters": ("log_rate", "gyro_unfilt", "debug_mode", "rpm"),
                                    "pid": ("pid_log_rate", "windows", "axes_mixed", "i_slider", "dyn_damping")}
    assert log_check.USAGE_ROLES == {"noise": "filters", "delay": "filters", "series": "pid", "ff_series": "pid"}


# Chaque seuil de profil lu par analyze() : mesure du log synthétique prise comme seuil ± δ → constat présent ou
# absent, et seuil cité dans le texte quand la règle le fait. (motor_left, imbalance, resonance ne le citent pas.)
THRESHOLD_CASES = [
    ("gyro_noise_high_db", lambda m: max(m["gyro_noise_db"]), "Gyro filtré bruité", 0.1, "{:.1f}"),
    ("low_band_high_db", lambda m: max(m["low_band_db"]), "Oscillations basses fréquences (20–80 Hz)", 0.1, "{:.1f}"),
    ("motor_left_high_db", lambda m: m["motor_harmonic_left_db"], "Bruit moteur mal éliminé", 0.1, None),
    ("saturation_high_pct", lambda m: m["stats"].motor_saturation_pct, "Moteurs souvent saturés", 0.1, "{:.1f}"),
    ("imbalance_high_pct", lambda m: m["stats"].motor_imbalance_pct, "Moteurs déséquilibrés", 0.1, None),
    ("iterm_offset_high", lambda m: abs(m["iterm_mean"][0]), "Terme I décentré en roll", 0.5, "{:.0f}"),
    ("resonance_high_db", lambda m: m["frame_resonance"][1], "Raie fixe", 0.1, None),
    ("roll_pitch_gap_high", lambda m: abs(m["steps"]["Pitch"].peak - m["steps"]["Roll"].peak),
     "Roll et pitch se comportent différemment", 0.002, "{:.2f}"),
]


@pytest.mark.parametrize("field, measure, title, delta, fmt", THRESHOLD_CASES, ids=[c[0] for c in THRESHOLD_CASES])
def test_profile_threshold_drives_rule(synthetic_log, field, measure, title, delta, fmt):
    from dataclasses import replace

    from opentunebox.advisor.profiles import PROFILES
    from opentunebox.advisor.rules import analyze

    base = PROFILES["five"]
    value = measure(analyze(synthetic_log, profile=base).measures)
    assert np.isfinite(value), field

    def findings(threshold):
        return [f for f in analyze(synthetic_log, profile=replace(base, **{field: threshold})).findings
                if f.title.startswith(title)]

    assert not findings(value + delta), f"{field} : constat présent au-dessus du seuil"
    hit = findings(value - delta)
    assert hit, f"{field} : constat absent sous le seuil"
    if fmt:
        assert fmt.format(value - delta) in hit[0].detail, f"{field} : seuil non cité"


def test_dterm_noise_rule_corpus_and_ptb_branches(synthetic_log):
    from dataclasses import replace

    from opentunebox.advisor.profiles import PROFILES
    from opentunebox.advisor.rules import DTERM_PTB_MAX_DB, analyze

    base = PROFILES["five"]
    rep = analyze(synthetic_log, profile=replace(base, dterm_noise_high_db=100.0))
    assert rep.measures["dterm_peak_ptb_db"] > DTERM_PTB_MAX_DB
    f = next(x for x in rep.findings if x.title == "Terme D bruité")
    assert "PIDtoolbox" in f.detail and "seuil" not in f.detail  # branche repère PTB seule
    quiet = replace(synthetic_log, d_term=synthetic_log.d_term * 0.002)
    rep = analyze(quiet, profile=base)
    assert rep.measures["dterm_peak_ptb_db"] < DTERM_PTB_MAX_DB
    d = max(rep.measures["dterm_noise_db"])
    hit = [x for x in analyze(quiet, profile=replace(base, dterm_noise_high_db=d - 0.1)).findings if x.title == "Terme D bruité"]
    assert hit and f"seuil {d - 0.1:.1f} dB" in hit[0].detail and "PIDtoolbox" not in hit[0].detail
    assert not [x for x in analyze(quiet, profile=replace(base, dterm_noise_high_db=d + 0.1)).findings
                if x.title == "Terme D bruité"]
    assert any(x.title == "Bruit du terme D" for x in analyze(quiet, profile=replace(base, dterm_noise_high_db=d + 0.1)).findings)


def test_delay_thresholds_and_provisional(synthetic_log):
    from dataclasses import replace

    from opentunebox.advisor.profiles import PROFILES
    from opentunebox.advisor.rules import OK, analyze

    base = PROFILES["five"]
    dl = analyze(synthetic_log, profile=base).measures["delays"]
    assert dl.moving

    def gyro_delay(threshold):
        return [f for f in analyze(synthetic_log, profile=replace(base, gyro_delay_max_ms=threshold)).findings
                if f.title in ("Retard des filtres gyro", "Filtrage gyro peut-être trop prudent")]

    assert [f.severity for f in gyro_delay(dl.gyro_ms + 0.1)] == [OK]
    assert all(f.severity != OK for f in gyro_delay(dl.gyro_ms - 0.1)) and gyro_delay(dl.gyro_ms - 0.1)
    ok_d = [f for f in analyze(synthetic_log, profile=replace(base, dterm_delay_max_ms=dl.dterm_ms + 0.1)).findings
            if f.title == "Retard des filtres D" and f.severity == OK]
    assert ok_d and f"{dl.dterm_ms + 0.1:.1f}" in ok_d[0].detail
    titles = {f.title for f in analyze(synthetic_log, profile=replace(base, provisional=True)).findings}
    assert "Seuils provisoires pour cette taille" in titles
    assert "Seuils provisoires pour cette taille" not in {f.title for f in analyze(synthetic_log, profile=base).findings}


# ---------------------------------------------------------------- règles pures


def test_lowpass_setup_rules():
    from opentunebox.advisor.rules import ATTENTION, CONSEIL, INFO, _lowpass_setup_rules
    from opentunebox.log.settings import parse_settings

    def rules_for(**raw):
        base = {"looptime": "125", "pid_process_denom": "1", "gyro_lpf2_type": "0", "gyro_lpf2_static_hz": "500",
                "dterm_lpf1_type": "0", "dterm_lpf1_dyn_hz": "75,150", "dterm_lpf2_type": "0", "dterm_lpf2_static_hz": "150"}
        base.update(raw)
        return {f.title: f for f in _lowpass_setup_rules(parse_settings(base))}

    assert rules_for() == {}
    f = rules_for(pid_process_denom="4", gyro_lpf2_static_hz="0")["LPF2 du gyro coupé : risque de repliement"]
    assert f.severity == ATTENTION and "500" in f.actions[0]  # boucle PID à 2 kHz
    f = rules_for(pid_process_denom="2", gyro_lpf2_static_hz="0")["LPF2 du gyro coupé : risque de repliement"]
    assert f.severity == CONSEIL and "1000" in f.actions[0]  # 4 kHz
    assert "LPF2 du gyro coupé : risque de repliement" not in rules_for(gyro_lpf2_static_hz="0")  # PID = gyro
    assert rules_for(dterm_lpf2_static_hz="0")["Filtrage du D insuffisant"].severity == ATTENTION
    assert "Filtrage du D insuffisant" not in rules_for(dterm_lpf2_static_hz="0", dterm_lpf1_type="2")  # PT2 seul
    assert "Filtrage du D insuffisant" not in rules_for(dterm_lpf2_static_hz="0", dterm_lpf1_type="1")  # biquad seul
    f = rules_for(dterm_lpf1_dyn_hz="0,0", dterm_lpf1_static_hz="90")["LPF1 du D en mode statique"]
    assert f.severity == INFO and "dterm_lpf1_dyn_min_hz = 90, dterm_lpf1_dyn_max_hz = 180" in f.actions[0]
    assert rules_for(gyro_lpf1_type="1", gyro_lpf1_static_hz="300")["Biquad sur le LPF1 du gyro"].severity == INFO  # 1 = BIQUAD
    assert "Biquad sur le LPF1 du gyro" not in rules_for(gyro_lpf1_type="2", gyro_lpf1_static_hz="300")  # 2 = PT2


def test_rpm_weight_rules_and_version_gate(monkeypatch):
    """Poids RPM : raie encore visible avec un poids réduit ou plein ; poids plus fort que nécessaire proposé
    seulement à partir de Betaflight 4.5 (rpm_filter_weights n'existe pas avant)."""
    from opentunebox.advisor import rules
    from opentunebox.log.settings import parse_settings

    levels = {"raw": np.array([20.0, -1.0, -1.0]), "left": np.array([0.0, 0.0, 0.0])}
    monkeypatch.setattr(rules, "rpm_harmonic_levels", lambda _log, _sel: (levels["raw"], levels["left"]))

    def titles(version="4.5.1", weights="100,100,100", **kw):
        s = parse_settings({"Firmware revision": f"Betaflight {version} (x) STM32F7X2", "rpm_filter_harmonics": "3",
                            "rpm_filter_weights": weights, "rpm_filter_q": "500"})
        return {f.title: f for f in rules._rpm_weight_rules(None, slice(None), s, **kw)}

    f = titles()["Filtre RPM plus fort que nécessaire"]
    assert f.actions[0].startswith("rpm_filter_weights = 100, 30, 30")  # −1 dB + 3 dB de marge → 0,21 → 0,3
    assert "Filtre RPM plus fort que nécessaire" not in titles("4.4.3")
    assert "Filtre RPM plus fort que nécessaire" not in titles(lighten=False)
    levels["left"] = np.array([0.0, 5.0, 0.0])
    assert "Poids du filtre RPM trop bas" in titles(weights="100,80,100")
    f = titles()["Raie moteur encore visible après le filtre RPM"]
    assert "rpm_filter_q 500 → 400" in f.actions[1]
    levels["left"] = np.array([5.0, 0.0, 0.0])
    assert "Raie moteur encore visible après le filtre RPM" in titles(h1_flagged=False)
    assert "Raie moteur encore visible après le filtre RPM" not in titles(h1_flagged=True)  # déjà « Bruit moteur mal éliminé »


def test_filter_targets_checks():
    from opentunebox.advisor.log_check import D_DELAY_PTB_OFFSET_MS, FilterTargets

    ok = FilterTargets(-30.0, -10.0, 1.75, 1.5 + D_DELAY_PTB_OFFSET_MS, (1.75, 1.5))
    assert ok.reached() is True and [c[1] for c in ok.checks()] == [True, True, True, True]
    assert FilterTargets(-30.0, -10.0, 1.75, 2.01, (1.75, 1.5)).reached() is False
    assert FilterTargets(-29.9, -10.0, 1.75, 2.0, (1.75, 1.5)).reached() is False
    assert FilterTargets(float("nan"), -10.0, 1.75, 2.0, (1.75, 1.5)).reached() is None
    assert len(FilterTargets(-30.0, -10.0, 1.75, 2.0, None).checks()) == 2  # whoop : pas d'objectif de retard
    assert FilterTargets(-30.0, float("nan"), 1.75, 2.0, None).reached() is None


def test_dyn_notch_q_too_high_needs_a_passing_line():
    """« Q trop haut » seulement si une raie à suivre dépasse encore HARMONIC_VISIBLE_DB dans le gyro filtré
    (choix de Cédric, 2026-10-13) : sinon le Q actuel suffit déjà, quoi qu'en dise le calcul."""
    from opentunebox.advisor import rules
    s = SimpleNamespace(dyn_notch_q=500)
    dn = {"q_need": 360.0, "peaks": [240.0], "widths": [14.0], "heights": [16.0], "left": [2.0], "count": 2}
    title = "Notch dynamique : Q trop haut pour les raies à suivre"
    assert not any(f.title == title for f in rules._dyn_notch_q_n_rules(s, dn, False, False, False, [], False))
    dn["left"] = [6.2]
    f = next(f for f in rules._dyn_notch_q_n_rules(s, dn, False, False, False, [], False) if f.title == title)
    assert f.actions[0].startswith("dyn_notch_q 500 → 350")
    assert "240 Hz (+6 dB)" in f.detail
