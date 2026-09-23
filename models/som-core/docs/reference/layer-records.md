# Layer records

The records the four SOM layers pass down, and the operation vocabulary the
assembler executes. `src/som_core/records.py` enforces this document;
`models/som-code-python/scripts/decompile_gold.py` measures L2 and L3 from
each gold program, and the corpus author writes L1 by hand. Each layer model
is trained to generate the next record from the one above it. No record
holds alternatives to choose between.

## Where the records live

| Record | Field in `family.json` | Produced by | Drift |
|---|---|---|---|
| Prompt | `caption` | author | the curation check grounds it in the gold |
| L1 plan | `plan` | author | the curation check grounds it in `decompiled` |
| L2 topology | `decompiled.topology` | decompiler, from the gold AST | re-measured on every curation run |
| L3 operation list | `decompiled.ops` | decompiler, from the gold source | re-measured on every curation run |
| Snippet coverage | `decompiled.coverage` | decompiler | re-measured on every curation run |

## L1 plan

```json
{"intent": "Create an item over HTTP and persist it",
 "target": "Item",
 "constraints": ["Return 201 with the stored item", "Return 422 when price is not positive"]}
```

- `intent`: one imperative sentence naming the feature, not its code.
- `target`: the domain entity or resource the feature is about.
- `constraints`: one or more observable requirements. Every exception the
  gold raises and every status code it declares appears in at least one
  constraint, by name or number.

## L2 topology

```json
{"files": [{"path": "candidate.py",
            "blocks": [{"name": "engine", "kind": "constant", "depends_on": []},
                       {"name": "create_item", "kind": "route", "depends_on": ["ItemCreate", "get_db"]}]}]}
```

- A file is one module; `path` is relative to the program root and ends in
  `.py`. Paths are unique.
- A block is one top-level statement other than an import. `name` is the
  class or function name, the first assigned name for an assignment, and
  for any other statement the dotted callee of its expression (or its
  statement keyword) with `#2`, `#3` appended on repeats. Names are unique
  within a file.
- `kind` is `class`, `function`, `route` (a function a decorator registers
  under an HTTP method), `command` (a CLI entry point), `constant` (an
  assignment), or `statement`.
- `depends_on` lists the blocks, in the same file or another, whose names the
  block's code reads, in first-reference order; a block in another file is
  written `<path>:<name>`. Every entry names a declared block.
- Scope: at most five files and twenty blocks. A larger topology is refused
  as `SCOPE_TOO_LARGE`.

## L3 operation list

An ordered list; each operation is an object with `op` and its fields.

| `op` | Fields | Effect |
|---|---|---|
| `CREATE_FILE` | `path`, `docstring` (string or null) | Starts a file, with its module docstring. Must precede every other operation on that path. |
| `ADD_IMPORT` | `path`, `stmt`, `group` | Adds one import statement, verbatim. `group` is the index of the blank-line-separated import paragraph it belongs to, starting at 0. |
| `INSERT_SNIPPET` | `path`, `block`, `id`, `params`, `blank_before` | Renders snippet `id` from the ISA with `params` (every `{{name}}` in the template must be given, and no extra key) as block `block`. |
| `INSERT_BLOCK` | `path`, `block`, `source`, `blank_before` | Writes `source` verbatim as block `block`: the ISA has no snippet for it. |

- Every `block` is declared in the topology under the same `path`, and every
  declared block is inserted exactly once, in topology order.
- `blank_before` is the number of blank lines before the block, 0 to 2.
- A snippet's `imports` are advisory for the generator; the list itself must
  carry every import the file needs as `ADD_IMPORT`.
- `decompiled.coverage` is `{"snippet_blocks": n, "literal_blocks": m}`.
  The share of `INSERT_SNIPPET` is how much of the corpus the ISA can
  express; `INSERT_BLOCK` is the gap the corpus project's ISA v2 closes.

## Assembly

The reference assembler (`som assemble`) writes each file as: the module
docstring, when present, in triple double quotes; one blank line; the
imports, a blank line between groups; then each block preceded by its
`blank_before` blank lines; a single trailing newline. A family is proven when its assembled file has
the same `ast.dump` as its gold and the family's fixture passes on it; the
corpus project's round-trip gate checks both for every family.
