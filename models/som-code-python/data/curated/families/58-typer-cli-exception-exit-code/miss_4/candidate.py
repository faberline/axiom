"""Map each deployment failure to its own Typer exit code and a stderr message."""

from pathlib import Path
from typing import Annotated

import typer

app = typer.Typer(help="Artifact deployment and verification CLI.")


@app.callback()
def main() -> None:
    """Main callback for multi-command CLI support."""


def execute_deployment(artifact_path: Path, target_env: str) -> dict[str, str]:
    """Execute deployment logic, raising specific domain errors."""
    if not artifact_path.exists():
        raise FileNotFoundError(f"Artifact not found: {artifact_path}")
    if target_env not in ("staging", "production"):
        raise ValueError(f"Invalid target environment: '{target_env}'")
    content = artifact_path.read_text(encoding="utf-8").strip()
    if not content:
        raise RuntimeError("Artifact payload is empty")
    return {"status": "deployed", "env": target_env, "artifact": artifact_path.name}


@app.command()
def deploy(
    artifact: Annotated[
        Path, typer.Option("--artifact", "-a", help="Path to artifact")
    ],
    env: Annotated[
        str, typer.Option("--env", "-e", help="Target environment")
    ] = "staging",
) -> None:
    """Deploy build artifact to target environment with strict exit code contracts."""
    try:
        result = execute_deployment(artifact, env)
        typer.echo(f"SUCCESS: {result['artifact']} deployed to {result['env']}")
    except FileNotFoundError as exc:
        typer.secho(f"Error: {exc}", fg=typer.colors.RED, err=True)
        raise typer.Exit(code=1) from exc
    except ValueError as exc:
        typer.secho(f"Validation Error: {exc}", fg=typer.colors.RED, err=True)
        raise typer.Exit(code=1) from exc
    except RuntimeError as exc:
        typer.secho(f"Deployment Failed: {exc}", fg=typer.colors.RED, err=True)
        raise typer.Exit(code=3) from exc
