# Snippet ISA

What a generator author gets from the snippet instruction set: a fixed
vocabulary of verified code templates that the L4 assembler will execute.
This area spans the README capability `snippet-isa`.

## Snippet ISA v1

- Problem: none open as shipped; the limits below belong to the outcome in
  this area.
- Who: generator authors; corpus authors adding a snippet.
- Promise: `data/snippets/<library>/<id>.json` is one instruction: an
  `id`, a `description`, the `imports` it needs, and a Mustache `template`
  with named parameters. The verifier checks every required snippet exists,
  that parameters are consistent, assembles the whole set in dependency order
  into one module, and compiles it.
- Limits today: the required set is eight snippets across FastAPI, Pydantic,
  and SQLAlchemy; the verifier is the only consumer, so no family's gold has
  been expressed as an operation list over the ISA.
- Non-goals: an assembler, which is the engine's L4 outcome.
- Neighbours: none; first section of the area.
- Status rows: `snippet-isa`.

## Snippet ISA v2

- Problem: The oracle families use libraries the ISA does not carry, so the
  gold of most families cannot be assembled from it.
- Who: generator authors; the L3 optimizer, whose vocabulary this is.
- Promise: The ISA covers the libraries the families use, every family's
  gold can be expressed as an operation list over it, and a coverage report
  names no family whose gold uses a library the ISA lacks.
- Non-goals: the assembler; a snippet for a library no family uses.
- Open: whether version 2 replaces version 1 in place or ships beside it
  under `data/snippets/v2/` until the assembler exists.
- Neighbours: [Snippet ISA v1](#snippet-isa-v1) above; the decompiler
  outcome in [data-pipeline.md](data-pipeline.md), whose L3 records are what
  the coverage report reads.
- Outcome: `snippet-isa-v2`. Tracking: Not assigned.

## Non-goals in this area

- No snippet for a platform without a Python oracle; see ROADMAP
  `platform-pilot-families`.
