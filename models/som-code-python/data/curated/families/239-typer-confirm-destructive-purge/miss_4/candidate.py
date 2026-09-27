"""A Typer purge command that lists, confirms, then deletes matching files."""

from __future__ import annotations

from pathlib import Path
from typing import Annotated

import typer

app = typer.Typer(help="Snapshot housekeeping.")


@app.callback()
def main() -> None:
    """Snapshot housekeeping commands."""


@app.command()
def purge(
    directory: Annotated[Path, typer.Argument(exists=True, file_okay=False)],
    pattern: Annotated[str, typer.Option(help="Glob to match")] = "*.snap",
    yes: Annotated[bool, typer.Option("--yes", "-y", help="Skip prompt")] = False,
    dry_run: Annotated[bool, typer.Option("--dry-run")] = False,
) -> None:
    """Delete files in DIRECTORY matching PATTERN after confirmation."""
    targets = sorted(p for p in directory.glob(pattern))
    if not targets:
        typer.echo("nothing to purge")
        raise typer.Exit()
    for path in targets:
        typer.echo(path.name)
    if dry_run:
        typer.echo(f"would delete {len(targets)} files")
        return
    if not yes:
        typer.confirm(f"Delete {len(targets)} files?", abort=True)
    for path in targets:
        path.unlink()
    typer.echo(f"deleted {len(targets)} files")
