"""Parse boolean click options from environment variables with explicit flags."""

from typing import Any

import click


@click.group()
def cli() -> None:
    """DevOps release pipeline controller."""


@cli.command("sync")
@click.option(
    "--dry-run",
    type=str,
    callback=lambda c, p, v: v in ("true", "1"),
    default="false",
    envvar="SYNC_DRY_RUN",
    help="Perform simulation without live changes (env: SYNC_DRY_RUN)",
)
@click.option(
    "--force/--no-force",
    default=False,
    envvar="SYNC_FORCE",
    help="Bypass confirmation prompt (env: SYNC_FORCE)",
)
@click.option(
    "--retries",
    type=click.IntRange(0, 10),
    default=3,
    envvar="SYNC_RETRIES",
    help="Number of retry attempts (env: SYNC_RETRIES)",
)
def sync_command(dry_run: bool, force: bool, retries: int) -> dict[str, Any]:
    """Execute pipeline sync with environment variable overrides."""
    payload = {"dry_run": dry_run, "force": force, "retries": retries}
    click.echo(f"STATUS: dry_run={dry_run} force={force} retries={retries}")
    return payload
