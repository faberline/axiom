# Contributing to pgkit

## Brief

How to change `apps/mamba/mambalibs/pgkit`. What it promises lives in
[README.md](README.md); mamba's own engineering doctrine and working-tree
discipline live in [apps/mamba/CONTRIBUTING.md](../../CONTRIBUTING.md);
repository-wide authoring and verification rules live in the root
[CONTRIBUTING.md](../../../../CONTRIBUTING.md).

There is no phase ladder here. `leg.leg_root` resolves `apps/<project>` and
nothing else, so a change to this crate is authored and committed directly
rather than driven through `e2e -> unit -> logic`.

## Verification

`apps/mamba/mambalibs/pgkit` is a container of 3 crates rather than one, so
the gate names all of them.

| Gate | Command |
|---|---|
| unit + colocated tests | `cargo test -p pg -p pg-cli -p pgkit-binding` |
