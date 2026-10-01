"""Ligne de commande : `opentunebox info LOG.bbl`, `opentunebox report LOG.bbl`."""

from __future__ import annotations

import argparse
import sys

from .log.flightlog import load


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="opentunebox", description="Analyse de logs Blackbox Betaflight")
    sub = ap.add_subparsers(dest="cmd", required=True)
    p_info = sub.add_parser("info", help="résumé des sessions d'un ou plusieurs logs")
    p_info.add_argument("files", nargs="+")
    args = ap.parse_args(argv)

    if args.cmd == "info":
        for f in args.files:
            try:
                logs = load(f)
            except Exception as e:  # noqa: BLE001
                print(f"{f} : illisible ({e})")
                continue
            if not logs:
                print(f"{f} : aucune session exploitable")
            for lg in logs:
                print(lg.summary())
                print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
