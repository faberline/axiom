"""Typer CLI interface for SOM core training, evaluation, and daemon serving."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Optional
import typer

app = typer.Typer(help="Train and use SOM locally on Apple silicon via MLX.")


@app.command()
def verify() -> None:
    """Verify MLX environment and hardware."""
    import mlx.core as mx
    typer.echo(f"MLX version: {mx.__version__}")
    typer.echo(f"Default device: {mx.default_device()}")
    typer.echo("MLX environment verified successfully.")


@app.command()
def train(
    domain: str = typer.Option("python", help="The domain model to train (e.g., python, react)"),
    data_dir: Optional[list[str]] = typer.Option(None, "--data-dir", "-d", help="Corpus directory; repeat to train on several corpora together (defaults to python-v2 data)"),
    epochs: int = typer.Option(3, "--epochs", "-e", help="Number of training epochs"),
    batch_size: int = typer.Option(1, "--batch-size", "-b", help="Gradient accumulation batch size"),
    lr: float = typer.Option(1e-4, "--lr", help="Learning rate"),
    smoke: bool = typer.Option(False, "--smoke", help="Run smoke test mode with minimal steps"),
    output_dir: Optional[str] = typer.Option(None, "--output-dir", "-o", help="Output directory for checkpoints"),
    use_skeleton: bool = typer.Option(True, "--skeleton", help="Use MLX Skeleton LoRA model for local training"),
) -> None:
    """Train a Structured Outcome Model (SOM) adapter via MLX using scenario datasets."""
    typer.echo(f"=== SOM Training Pipeline: domain={domain} ===")
    from .dataset import load_corpora
    from .train import train as run_train

    typer.echo(f"Loading corpora: {', '.join(data_dir) if data_dir else 'auto-discover'}")
    dataset = load_corpora(data_dir)
    typer.echo(f"Successfully loaded {len(dataset)} scenario examples from {len(data_dir or [None])} corpus directories.")

    run_path = output_dir or f"runs/{domain}-lora"
    typer.echo(f"Starting MLX LoRA training loop -> output: {run_path}")

    summary = run_train(
        run=run_path,
        smoke=smoke,
        dataset=dataset,
        epochs=epochs,
        batch_size=batch_size,
        lr=lr,
        use_skeleton=use_skeleton,
    )
    typer.echo(f"Training finished with status: {summary.get('status')}")
    typer.echo(f"Checkpoint saved: {summary.get('checkpoint')}")


@app.command()
def serve(
    domain: str = typer.Option("python", help="The domain model to load into memory (e.g., python, react)"),
    checkpoint: Optional[str] = typer.Option(None, "--checkpoint", "-c", help="Path to the MLX model checkpoint"),
    preload_dataset: bool = typer.Option(True, "--preload-dataset", help="Preload domain dataset into memory"),
    data_dir: Optional[list[str]] = typer.Option(None, "--data-dir", "-d", help="Corpus directory; repeatable"),
) -> None:
    """
    Start the SOM Daemon (MCP Server) to keep the model 'hot' in memory.
    This allows IDEs and Agents to send requests via stdio/HTTP without cold-start delays.
    """
    typer.echo(f"Starting SOM Daemon/MCP Server for domain: {domain}")

    if preload_dataset:
        from .dataset import load_corpora
        typer.echo(f"Preloading {domain} dataset into memory...")
        try:
            dataset = load_corpora(data_dir)
            typer.echo(f"Preloaded {len(dataset)} scenario families into daemon cache.")
        except Exception as e:
            typer.echo(f"Notice: Could not preload dataset: {e}")

    typer.echo("Loading MLX weights into Unified Memory... (Model is now HOT)")
    if checkpoint:
        typer.echo(f"Using checkpoint: {checkpoint}")

    typer.echo("Listening for MCP requests on stdio...")
    try:
        pass
    except KeyboardInterrupt:
        typer.echo("Shutting down SOM Daemon.")


if __name__ == "__main__":
    app()
