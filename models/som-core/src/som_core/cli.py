"""Typer CLI for SOM core: environment check and the reference assembler."""

from __future__ import annotations

from pathlib import Path

import typer

from .assemble import DEFAULT_ISA, assemble as run_assemble, load_isa, load_ops_document, write_files
from .records import RecordError

app = typer.Typer(help="Snippet-Oriented Model (SOM) engine on Apple silicon.")


@app.command()
def verify() -> None:
    """Verify MLX environment and hardware."""
    import mlx.core as mx
    typer.echo(f"MLX version: {mx.__version__}")
    typer.echo(f"Default device: {mx.default_device()}")
    typer.echo("MLX environment verified successfully.")


@app.command()
def assemble(
    source: Path = typer.Argument(..., help="JSON with 'topology' and 'ops', or a family.json whose 'decompiled' block has both"),
    out: Path = typer.Option(..., "--out", "-o", help="Directory the assembled files are written under"),
    isa: Path = typer.Option(DEFAULT_ISA, "--isa", help="Snippet ISA directory (<library>/<id>.json)"),
) -> None:
    """Assemble the files an L3 operation list describes."""
    try:
        ops, topology = load_ops_document(source)
        files = run_assemble(ops, topology, load_isa(isa))
    except RecordError as exc:
        typer.echo(f"refused: {exc}", err=True)
        raise typer.Exit(2) from exc
    for path in write_files(files, out):
        typer.echo(str(path))


if __name__ == "__main__":
    app()
