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
    p_rep = sub.add_parser("report", help="diagnostic de l'assistant de réglage")
    p_rep.add_argument("files", nargs="+")
    p_rep.add_argument("--profil", choices=["whoop", "micro", "five", "seven", "large"], default=None,
                       help="taille du drone (détectée automatiquement sinon)")
    args = ap.parse_args(argv)

    if args.cmd == "report":
        from .advisor.profiles import PROFILES
        from .advisor.rules import analyze
        from .analysis.period import auto_period

        for f in args.files:
            for lg in load(f):
                t0, t1 = auto_period(lg)
                rep = analyze(lg, lg.slice_time(t0, t1), PROFILES.get(args.profil) if args.profil else None)
                print(f"=== {lg.name} — {lg.settings.firmware}")
                print(f"    {rep.profile.label} ({rep.profile_reason}) ; période {t0:.1f}–{t1:.1f} s")
                for fd in rep.sorted():
                    print(f"  [{fd.severity.upper():9s}] {fd.category} — {fd.title}")
                    print(f"      {fd.detail}")
                    for a in fd.actions:
                        print(f"      → {a}")
                print()

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
