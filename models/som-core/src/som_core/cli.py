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


@app.command()
def lock(
    model: str = typer.Argument(..., help="Hugging Face repo id of the base model"),
    lock_path: Path = typer.Option(None, "--lock", help="Lock file to write (default: sources.lock.json)"),
) -> None:
    """Pin a base model's current revision and every file's sha256 in a lock file."""
    from .train import LOCK, write_lock

    pinned = write_lock(model, lock_path or LOCK)
    typer.echo(f"{lock_path or LOCK}: {pinned['model']}@{pinned['revision']} ({len(pinned['files'])} files)")


@app.command()
def train(
    out: Path = typer.Option(..., "--out", "-o", help="Directory for the adapter and training_summary.json"),
    data_dir: list[Path] = typer.Option(None, "--data-dir", help="Corpus directory; repeat for several (default: som-code-python curated, then user)"),
    layer_weight: list[str] = typer.Option(None, "--layer-weight", help="layer=weight, e.g. user=3"),
    iters: int = typer.Option(600, help="Optimizer steps"),
    batch_size: int = typer.Option(1, help="Pairs per step"),
    learning_rate: float = typer.Option(1e-4, help="Adam learning rate"),
    lora_layers: int = typer.Option(16, help="Transformer blocks, counted from the top, that get LoRA"),
    rank: int = typer.Option(16, help="LoRA rank"),
    eval_every: int = typer.Option(100, help="Steps between per-layer validation passes"),
    max_seq_length: int = typer.Option(4096, help="Token cap per pair"),
    lock_path: Path = typer.Option(None, "--lock", help="Base-model lock file (default: sources.lock.json)"),
    task: str = typer.Option("layers", help="layers (L1/L2/L3 records) or dsl (SOM prompt -> DSL)"),
    prompts_dir: Path = typer.Option(None, "--prompts-dir", help="dsl: flagship SOM prompts, <family>.json each"),
    val_ratio: float = typer.Option(0.1, help="dsl: share of non-holdout families held out for validation"),
    dropout: bool = typer.Option(True, "--dropout/--no-dropout", help="dsl: add a copy of each prompt with optional tags removed"),
    dropout_copies: int = typer.Option(1, "--dropout-copies", help="dsl: dropout copies per prompt, each its own draw"),
    revisions: bool = typer.Option(False, "--revisions/--no-revisions", help="dsl: add near miss -> fix: why_wrong -> gold retry turns"),
    retries: bool = typer.Option(False, "--retries/--no-retries", help="dsl: add rejected attempt -> the loop's own feedback -> gold retry turns"),
    swaps: bool = typer.Option(False, "--swaps/--no-swaps", help="dsl: add prompt and gold with the raises message words swapped alike"),
    fix_retries: bool = typer.Option(False, "--fix-retries/--no-fix-retries", help="dsl: add the retry after a fix: revision that still lacks the named call or keyword"),
    train_fraction: float = typer.Option(1.0, help="dsl: share of training families kept, nested across fractions"),
    val_layer: list[str] = typer.Option(None, "--val-layer", help="Validate, and pick the checkpoint, on this layer only; repeatable"),
    docstrings: bool = typer.Option(True, "--docstrings/--no-docstrings", help="dsl: keep function and class docstrings in completions and retry attempts"),
    order_seed: int = typer.Option(None, "--order-seed", help="Seed for the batch order only; the split and the pairs keep the default seed"),
) -> None:
    """LoRA-train the L1/L2/L3 layers, or SOM prompt -> DSL, on the locked base model."""
    from .train import TrainError, train as run_train

    try:
        summary = run_train(
            out, data_dir or None, layer_weight or None, iters=iters, batch_size=batch_size,
            learning_rate=learning_rate, lora_layers=lora_layers, rank=rank, eval_every=eval_every,
            max_seq_length=max_seq_length, **({"lock_path": lock_path} if lock_path else {}), task=task,
            prompts_dir=prompts_dir, val_ratio=val_ratio, dropout=dropout, dropout_copies=dropout_copies, revisions=revisions, retries=retries, swaps=swaps, fix_retries=fix_retries,
            train_fraction=train_fraction, val_layers=tuple(val_layer) if val_layer else None, docstrings=docstrings, order_seed=order_seed, log=typer.echo,
        )
    except TrainError as exc:
        typer.echo(f"refused: {exc}", err=True)
        raise typer.Exit(2) from exc
    typer.echo(f"validation loss fell: {summary['val_fell']}")


@app.command(name="eval")
def eval_(
    adapter: Path = typer.Argument(..., help="Directory `som train --out` wrote"),
    out: Path = typer.Option(..., "--out", "-o", help="Directory for generated programs and eval_summary.json"),
    isa: Path = typer.Option(DEFAULT_ISA, "--isa", help="Snippet ISA directory (<library>/<id>.json)"),
    limit: int = typer.Option(0, help="Judge only the first N held-out families (0 = all)"),
    teacher_forced: bool = typer.Option(False, "--teacher-forced", help="Also run L2 and L3 from each family's gold upstream records"),
) -> None:
    """Generate every held-out family from its caption, assemble it, and run its fixture."""
    from .evaluate import evaluate
    from .train import TrainError

    try:
        report = evaluate(adapter, out, isa, limit or None, teacher_forced, log=typer.echo)
    except TrainError as exc:
        typer.echo(f"refused: {exc}", err=True)
        raise typer.Exit(2) from exc
    _echo_rate("chain", report["chain"])
    if "teacher_forced" in report:
        typer.echo(f"l2|gold-l1 stages: {report['teacher_forced']['l2']['stages']}")
        _echo_rate("l3|gold-l1,l2", report["teacher_forced"]["l3"])


def _echo_rate(name: str, part: dict) -> None:
    rate = part["fixture_pass_rate"]
    typer.echo(f"{name} stages: {part['stages']}")
    typer.echo(
        f"{name} fixture pass rate: {part['fixture_passed']} / {part['fixture_denominator']}"
        + (f" ({rate:.1%})" if rate is not None else "")
        + f"  quality-clean: {part['quality_clean']}"
    )


if __name__ == "__main__":
    app()
