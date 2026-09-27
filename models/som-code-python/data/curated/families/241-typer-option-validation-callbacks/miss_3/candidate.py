"""Validate Typer options with ranges, enum choices and a parameter callback."""

from __future__ import annotations

import json
from enum import StrEnum
from typing import Annotated

import typer

app = typer.Typer(help="Service launcher.")


class Format(StrEnum):
    """Output formats."""

    JSON = "json"
    TABLE = "table"


def validate_labels(value: list[str] | None) -> list[str] | None:
    """Reject labels that are not KEY=VALUE with a non-empty key."""
    for item in value or []:
        key, sep, _ = item.partition("=")
        if not sep:
            raise typer.BadParameter(f"expected KEY=VALUE, got {item!r}")
    return value


@app.command()
def describe(
    name: str,
    port: Annotated[int, typer.Option(min=1, max=65535)] = 8080,
    fmt: Annotated[Format, typer.Option("--format", case_sensitive=False)] = (
        Format.TABLE
    ),
    label: Annotated[
        list[str] | None,
        typer.Option("--label", "-l", callback=validate_labels),
    ] = None,
) -> None:
    """Describe service NAME with its port and labels."""
    labels = {k: v for k, _, v in (item.partition("=") for item in label or [])}
    if fmt is Format.JSON:
        payload = {"name": name, "port": port, "labels": labels}
        typer.echo(json.dumps(payload, sort_keys=True))
    else:
        pairs = ",".join(f"{k}={v}" for k, v in sorted(labels.items()))
        typer.echo(f"{name}\t{port}\t{pairs}")
