# Snippet ISA

What a generator author gets from the snippet instruction set: a fixed
vocabulary of verified code templates that the L4 assembler will execute.
This area spans the README capability `snippet-isa`.

## Snippet ISA

- Problem: none open as shipped; the limits below belong to the outcome in
  this area.
- Who: generator authors, whose L3 model emits `INSERT_SNIPPET`; corpus
  authors adding a snippet.
- Promise: `data/snippets/<library>/<id>.json` is one instruction: an
  `id`, a `description`, the `imports` it needs, and a `template` of literal
  text with `{{name}}` placeholders and one-level list sections, so a model
  class or a route's keyword list is one snippet with a list parameter. Every
  snippet is cut from gold text and used by at least two families'
  operation lists; no template line is only a placeholder, so a snippet
  cannot hide an arbitrary body; and the verifier reports how many corpus
  blocks the ISA expresses.
- Limits today: see STATUS `snippet-isa` for the measured coverage; the
  remaining literal blocks are route and function bodies with branching
  logic, exception handlers, CLI commands, test instrumentation, and the
  code of libraries only one family uses.
- Non-goals: the assembler, which `models/som-core` owns; nested sections or
  multi-line placeholders.
- Neighbours: the decompiler outcome in [data-pipeline.md](data-pipeline.md),
  whose L3 records are what the verifier counts.
- Status rows: `snippet-isa`.

## Snippet long tail

- Problem: most corpus blocks are still `INSERT_BLOCK` literals, so the L3
  model learns them as free code rather than as instructions.
- Who: generator authors; corpus authors extending the ISA.
- Promise: the idioms behind route handlers with branching bodies, exception
  handlers, CLI commands, and async code become snippets wherever two
  families share them, raising the verifier's coverage without a
  placeholder-only line.
- Non-goals: a snippet used by one family; a new template syntax.
- Open: whether an idiom only one family uses today earns a second family,
  or stays a literal block.
- Neighbours: [Snippet ISA](#snippet-isa) above.
- Outcome: `snippet-long-tail`. Tracking: Not assigned.

## Non-goals in this area

- No snippet for a platform without a Python oracle; see ROADMAP
  `platform-pilot-families`.
