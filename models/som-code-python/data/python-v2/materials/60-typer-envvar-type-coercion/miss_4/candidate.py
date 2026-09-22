from typing import List, Optional
import typer

app = typer.Typer(help="Service gateway configuration runner.")


@app.callback()
def main() -> None:
    """Main callback for multi-command CLI support."""
    pass


def parse_endpoints(value: Optional[str]) -> List[str]:
    """Parse comma-separated endpoints from env string or CLI flag."""
    if not value or not value.strip():
        return []
    return [item.strip() for item in value.split(",") if item.strip()]


@app.command()
def serve(
    workers: int = typer.Option(
        4, "--workers", "-w", envvar="GATEWAY_WORKERS", help="Worker process count"
    ),
    timeout: float = typer.Option(
        30.0, "--timeout", "-t", envvar="GATEWAY_TIMEOUT", help="Request timeout in seconds"
    ),
    endpoints: Optional[str] = typer.Option(
        None, "--endpoints", "-e", envvar="GATEWAY_ENDPOINTS", help="Comma-separated endpoints"
    ),
    metrics: bool = typer.Option(
        False, "--metrics/--no-metrics", envvar="GATEWAY_METRICS", help="Enable Prometheus metrics"
    ),
) -> None:
    """Serve gateway with validated environment and CLI parameters."""
    if workers < 1:
        typer.secho(f"Error: workers must be >= 1, got {workers}", fg=typer.colors.RED, err=True)
        raise typer.Exit(code=2)
    if timeout <= 0.0:
        typer.secho(f"Error: timeout must be > 0, got {timeout}", fg=typer.colors.RED, err=True)
        raise typer.Exit(code=2)

    parsed_endpoints = parse_endpoints(endpoints)
    typer.echo(
        f"CONFIG: workers={workers} timeout={timeout:.1f} metrics={metrics} endpoints={len(parsed_endpoints)}"
    )
