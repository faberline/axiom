"""Read typed gateway settings from Typer options that fall back to env vars."""

from typing import Annotated

import typer

app = typer.Typer(help="Service gateway configuration runner.")


@app.callback()
def main() -> None:
    """Main callback for multi-command CLI support."""


def parse_endpoints(value: str | None) -> list[str]:
    """Parse comma-separated endpoints from env string or CLI flag."""
    if not value or not value.strip():
        return []
    return [item.strip() for item in value.split(",") if item.strip()]


@app.command()
def serve(
    workers: Annotated[
        int,
        typer.Option(
            "--workers", "-w", envvar="GATEWAY_WORKERS", help="Worker process count"
        ),
    ] = 4,
    timeout: Annotated[
        float,
        typer.Option("--timeout", "-t", help="Request timeout in seconds"),
    ] = 30.0,
    endpoints: Annotated[
        str | None,
        typer.Option(
            "--endpoints",
            "-e",
            envvar="GATEWAY_ENDPOINTS",
            help="Comma-separated endpoints",
        ),
    ] = None,
    metrics: Annotated[
        bool,
        typer.Option(
            "--metrics/--no-metrics",
            envvar="GATEWAY_METRICS",
            help="Enable Prometheus metrics",
        ),
    ] = True,
) -> None:
    """Serve gateway with validated environment and CLI parameters."""
    if workers < 1:
        typer.secho(
            f"Error: workers must be >= 1, got {workers}", fg=typer.colors.RED, err=True
        )
        raise typer.Exit(code=2)
    if timeout <= 0.0:
        typer.secho(
            f"Error: timeout must be > 0, got {timeout}", fg=typer.colors.RED, err=True
        )
        raise typer.Exit(code=2)

    parsed_endpoints = parse_endpoints(endpoints)
    typer.echo(
        f"CONFIG: workers={workers} timeout={timeout:.1f} metrics={metrics} "
        f"endpoints={len(parsed_endpoints)}"
    )
