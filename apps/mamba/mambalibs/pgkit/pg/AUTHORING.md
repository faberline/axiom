# pg — Author Guide

## Async/Sync Parity Invariant

`pg` is **async-first with a sync facade**. Every public
IO-touching function in `driver/`, `migrate/`, and (when it lands)
`orm/` is `async fn` over `sqlx`. The `driver/blocking/` directory is
the sync surface, and every function there is a thin pass-through that
drives the async surface via `self.runtime.block_on(...)` with no
branching, retry, or business logic.

The invariant exists to prevent the SQLAlchemy `AsyncSession` failure
mode — a sync codebase with greenlet-shimmed async surface that suffers
from lazy-load surprises, `selectin`/`run_sync` workarounds, and
divergent semantics between the two surfaces. Two enforcement tests
live under `projects/pgkit/pg/tests/`:

  * `test_async_blocking_parity.rs` — every public `async fn` has a
    public blocking sibling and vice versa. A small `EXEMPT_*` list
    captures legitimately one-sided fns (constructors, getters,
    borrow helpers) with a reviewer-approved reason.
  * `test_blocking_facade_shape.rs` — every blocking facade fn body
    contains a `block_on` call (unless the fn is on an
    `ALLOWLIST` of facade-internal extensions) and contains no
    `if` / `match` / `while` / `for` / `loop` at the facade level.

Adding a public async fn requires adding its blocking sibling in the
same change. Adding a blocking fn requires adding (or pointing to)
its async sibling. Removing either side of a pair is a breaking
design-contract change and requires explicit reviewer approval
recorded in the TD.

## Layer Boundaries

Per `.aw/tech-design/projects/pg/specs/pg-mod-boundary.md`,
`pg` is split into three layers, each modeled on a Python
pgkit analog:

  * `driver/`   — psycopg / asyncpg equivalent (Connection,
                  Transaction, low-level executor)
  * `orm/`      — SQLAlchemy equivalent (schema, query builder,
                  validation; Session not yet implemented)
  * `migrate/`  — Alembic equivalent (Migration value object +
                  MigrationRunner)

Cross-layer dependencies flow downward only:
`migrate → driver`, `orm → driver`. `driver/` must not depend on
`orm/` or `migrate/`.

When the `orm/` layer grows into a `Session` / Unit-of-Work surface,
the same parity invariant binds: the Session API is designed
**async-native first**, and a sync `BlockingSession` facade is added
in the same change. No `AsyncSession` retrofit pattern is permitted.
