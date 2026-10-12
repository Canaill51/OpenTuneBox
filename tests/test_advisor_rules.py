"""Fonctions pures de l'assistant (profils, règles), sans corpus : tournent aussi en intégration continue."""

from types import SimpleNamespace

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


def test_dyn_notch_q_need():
    """Q du notch qui ramène la raie à 3 dB du bruit voisin sur sa demi-largeur (notch SVF, Q × 100)."""
    import math

    from opentunebox.advisor.rules import _q_round, dyn_notch_q_need

    # raie à 200 Hz, 20 Hz de large, 15 dB : atténuation de 12 dB à ±10 Hz
    h = 10 ** (-12 / 20)
    q = dyn_notch_q_need([200.0], [20.0], [15.0])
    assert q == pytest.approx(100 * 200 * h / (20 * math.sqrt(1 - h * h)))
    assert _q_round(q) == 250
    assert dyn_notch_q_need([200.0], [20.0], [2.0]) == float("inf")  # déjà sous 3 dB : aucun besoin
    assert dyn_notch_q_need([200.0], [float("nan")], [15.0]) is None
    assert _q_round(40.0) == 100  # borne basse
