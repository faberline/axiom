"""Share global Typer options with a subcommand group through ctx.obj."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Annotated

import typer

ALLOWED_KEYS = frozenset({"region", "output"})

app = typer.Typer(help="Cloud CLI.")
config_app = typer.Typer(help="Inspect and edit configuration.")
app.add_typer(config_app, name="config")


@dataclass(frozen=True)
class State:
    """Global options resolved once in the root callback."""

    verbose: bool
    profile: str


@app.callback()
def main(
    ctx: typer.Context,
    verbose: Annotated[bool, typer.Option("--verbose", "-v")] = False,
    profile: Annotated[str, typer.Option()] = "default",
) -> None:
    """Resolve global options for every subcommand."""
    ctx.obj = State(verbose=verbose, profile=profile)


@config_app.command("show")
def show(ctx: typer.Context) -> None:
    """Print the active profile."""
    state: State = ctx.obj
    if state.verbose:
        typer.echo(f"[debug] profile={state.profile}", err=True)
    typer.echo(f"profile: {state.profile}")


@config_app.command("set")
def set_value(ctx: typer.Context, key: str, value: str) -> None:
    """Set KEY to VALUE in the active profile."""
    state: State = ctx.obj
    if key not in ALLOWED_KEYS:
        raise typer.BadParameter(f"unknown key {key!r}", param_hint="KEY")
    if state.verbose:
        typer.echo(f"[debug] writing {key}", err=True)
    typer.echo(f"{state.profile}.{key} = {value}")
