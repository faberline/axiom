"""A Typer wc clone reading files or stdin and reporting partial failure."""

from __future__ import annotations

from pathlib import Path
from typing import Annotated

import typer

app = typer.Typer(help="Text utilities.")


@app.callback()
def main() -> None:
    """Text utilities."""


@app.command()
def wc(
    paths: Annotated[list[Path] | None, typer.Argument()] = None,
    lines_only: Annotated[bool, typer.Option("--lines", "-l")] = False,
) -> None:
    """Count newlines and words in PATHS, or stdin when none or '-' is given."""
    sources = paths or [Path("-")]
    failed = False
    total_lines = total_words = 0
    for path in sources:
        try:
            if str(path) == "-":
                text = typer.get_text_stream("stdin").read()
            else:
                text = path.read_text(encoding="utf-8")
        except OSError as exc:
            typer.echo(f"wc: {path}: {exc.strerror}", err=True)
            failed = True
            continue
        n_lines, n_words = text.count("\n"), len(text.split())
        total_lines += n_lines
        total_words += n_words
        typer.echo(
            f"{n_lines}\t{path}" if lines_only else f"{n_lines}\t{n_words}\t{path}"
        )
    if len(sources) > 1:
        typer.echo(
            f"{total_lines}\ttotal"
            if lines_only
            else f"{total_lines}\t{total_words}\ttotal"
        )
    if failed:
        raise typer.Exit(code=1)
