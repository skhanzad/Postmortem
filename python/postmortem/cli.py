from __future__ import annotations

import argparse
import sys

from postmortem import __version__


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="postmortem")
    parser.add_argument("--version", action="version", version=f"postmortem {__version__}")
    sub = parser.add_subparsers(dest="command")
    check = sub.add_parser("check", help="check changes for test tampering")
    check.add_argument("--base", help="base ref to compare against")
    args = parser.parse_args(argv)

    if args.command == "check":
        print("No checks implemented yet.")
        return 0
    parser.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main())
