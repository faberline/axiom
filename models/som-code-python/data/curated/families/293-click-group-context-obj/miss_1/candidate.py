"""An inventory CLI whose click group shares one store object via ctx.obj."""

from __future__ import annotations

import json
from pathlib import Path

import click


class Inventory:
    """Item quantities persisted to a JSON file."""

    def __init__(self, path: Path, verbose: bool) -> None:
        self.path = path
        self.verbose = verbose
        self.items: dict[str, int] = {}
        if path.exists():
            self.items = json.loads(path.read_text(encoding="utf-8"))

    def add(self, name: str, qty: int) -> None:
        """Increase the quantity of name."""
        self.items[name] = self.items.get(name, 0) + qty

    def remove(self, name: str) -> None:
        """Delete name or raise ClickException if it is unknown."""
        if name not in self.items:
            raise click.ClickException(f"no such item: {name}")
        del self.items[name]

    def save(self) -> None:
        """Write the items back to the JSON file."""
        self.path.write_text(json.dumps(self.items, sort_keys=True), encoding="utf-8")


pass_inventory = click.make_pass_decorator(Inventory)


@click.group()
@click.option(
    "--store",
    type=click.Path(dir_okay=False, path_type=Path),
    envvar="INV_STORE",
    required=True,
)
@click.option("-v", "--verbose", is_flag=True)
@click.pass_context
def cli(ctx: click.Context, store: Path, verbose: bool) -> None:
    """Manage an inventory file."""
    inventory = Inventory(store, verbose)
    ctx.obj = inventory


@cli.command()
@click.argument("name")
@click.argument("qty", type=click.IntRange(min=1))
@pass_inventory
def add(inventory: Inventory, name: str, qty: int) -> None:
    """Add QTY of NAME."""
    inventory.add(name, qty)
    if inventory.verbose:
        click.echo(f"added {qty} {name}", err=True)


@cli.command()
@click.argument("name")
@pass_inventory
def remove(inventory: Inventory, name: str) -> None:
    """Remove NAME entirely."""
    inventory.remove(name)


@cli.command("list")
@pass_inventory
def list_items(inventory: Inventory) -> None:
    """Print each item and quantity, sorted by name."""
    for name, qty in sorted(inventory.items.items()):
        click.echo(f"{name}\t{qty}")
