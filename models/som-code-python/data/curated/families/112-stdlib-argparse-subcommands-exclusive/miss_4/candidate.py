"""Inventory command line with subcommands, exclusive format flags and exit codes."""

import argparse
from collections.abc import Sequence
from typing import TextIO

EXIT_USAGE = 2


def positive_int(text: str) -> int:
    """Parse a strictly positive integer for argparse."""
    try:
        value = int(text)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(f"not a positive integer: {text}") from exc
    if value < 1:
        raise argparse.ArgumentTypeError(f"not a positive integer: {text}")
    return value


def build_parser() -> argparse.ArgumentParser:
    """Build the parser with required export and count subcommands."""
    parser = argparse.ArgumentParser(prog="inv")
    commands = parser.add_subparsers(dest="command", required=True)
    export = commands.add_parser("export")
    export.add_argument("path")
    formats = export.add_mutually_exclusive_group()
    formats.add_argument("--json", action="store_true")
    formats.add_argument("--csv", action="store_true")
    export.add_argument("--limit", type=positive_int, default=10)
    commands.add_parser("count")
    return parser


def main(argv: Sequence[str], out: TextIO) -> int:
    """Run the command line and return its exit code instead of exiting."""
    try:
        args = build_parser().parse_args(argv)
    except SystemExit as exc:
        return exc.code if isinstance(exc.code, int) else EXIT_USAGE
    if args.command == "count":
        print("count", file=out)
        return 0
    fmt = "csv" if args.csv else "json"
    print(f"export {args.path} as {fmt} limit {args.limit}", file=out)
    return 0
