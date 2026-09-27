"""Validate and normalize click options with parameter callbacks."""

from __future__ import annotations

import json
import re

import click

TAG_PATTERN = re.compile(r"[a-z][a-z0-9-]{0,19}")


def parse_target(
    _ctx: click.Context, _param: click.Parameter, value: str
) -> tuple[str, int]:
    """Turn HOST:PORT into a (host, port) pair or raise BadParameter."""
    host, sep, port = value.rpartition(":")
    if not sep or not host:
        raise click.BadParameter("expected HOST:PORT")
    if not port.isdigit() or not 0 <= int(port) <= 65535:
        raise click.BadParameter("port must be between 1 and 65535")
    return host, int(port)


def parse_tags(
    _ctx: click.Context, _param: click.Parameter, value: tuple[str, ...]
) -> tuple[str, ...]:
    """Lower-case, validate and de-duplicate tags, keeping first-seen order."""
    seen: list[str] = []
    for raw in value:
        normalized = raw.strip().lower()
        if not TAG_PATTERN.fullmatch(normalized):
            raise click.BadParameter(f"invalid tag {raw!r}")
        if normalized not in seen:
            seen.append(normalized)
    return tuple(seen)


@click.command()
@click.option("--target", required=True, callback=parse_target)
@click.option("--tag", "tags", multiple=True, callback=parse_tags)
@click.option(
    "--timeout",
    type=click.FloatRange(min=0, min_open=True),
    default=5.0,
    show_default=True,
)
def main(target: tuple[str, int], tags: tuple[str, ...], timeout: float) -> None:
    """Print the validated options as JSON."""
    host, port = target
    payload = {"host": host, "port": port, "tags": list(tags), "timeout": timeout}
    click.echo(json.dumps(payload))
