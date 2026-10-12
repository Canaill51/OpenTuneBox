"""Ligne de commande `opentunebox info` / `report`, sans corpus."""

import pytest

from opentunebox.cli import main


def test_cli_requires_a_command():
    with pytest.raises(SystemExit):
        main([])
    with pytest.raises(SystemExit):
        main(["report", "x.bbl", "--profil", "geant"])  # taille inconnue refusée par argparse


@pytest.mark.parametrize("cmd", ["info", "report"])
def test_cli_unreadable_file_is_reported_not_raised(tmp_path, capsys, cmd):
    """Un fichier qui n'est pas un log est signalé en une ligne, par les deux commandes, sans trace d'erreur."""
    bad = tmp_path / "pas-un-log.bbl"
    bad.write_bytes(b"\x00\x01rien de bon ici" * 50)
    assert main([cmd, str(bad)]) == 0
    out = capsys.readouterr().out
    assert "illisible" in out or "aucune session exploitable" in out
    assert "Traceback" not in out
