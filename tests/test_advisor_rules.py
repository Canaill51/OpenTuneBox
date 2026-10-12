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
