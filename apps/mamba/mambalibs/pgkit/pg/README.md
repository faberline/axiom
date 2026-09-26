# pgkit

## Brief

Native PostgreSQL toolkit core for Mamba-facing applications.

## Capabilities

### Capability Index

| Capability | Root WI | Impl | Verification | Maturity | Production | Notes |
|---|---:|---|---|---|---|---|
| Connection Pooling And Driver Access | - | implemented | verified | smoke | not_ready | async SQLx-backed connection and pooling primitives |
| Query Row And Type Mapping | - | implemented | verified | smoke | not_ready | parameterized CRUD, row extraction, and Mamba/PostgreSQL value conversion |
| Transaction And Migration Control | - | implemented | verified | smoke | not_ready | ACID transaction, savepoint, and up/down migration primitives |
| Schema And ORM Introspection | - | implemented | verified | smoke | not_ready | table metadata, constraints, relations, and ORM model mapping |
| Blocking Facade For Mamba Bindings | - | implemented | verified | smoke | not_ready | blocking wrappers consumed by the `mambalibs.pg` binding layer |
| Bulk Operations And Metrics | - | implemented | verified | smoke | not_ready | batch execution support, benchmark coverage, and operational metrics |

### Connection Pooling And Driver Access

ID: connection-pooling-and-driver-access
Type: RuntimeTool
Surfaces: Rust API: `pg::Connection` + `PoolConfig` - async PostgreSQL connection and pooling primitives
EC Dimensions: behavior: `cargo test -p pg test_pool` - pool configuration and connection lifecycle
Root WI: -
Status: verified
Required Verification: smoke
Promise:
pgkit provides SQLx-backed PostgreSQL connection and pooling primitives for Mamba-facing applications without moving SQL generation or serialization onto the Mamba heap.
Gate Inventory: `cargo test -p pg test_pool`

| Work Root | Kind | WI | Impl | Verification | Maturity | Gate / Evidence |
|---|---|---:|---|---|---|---|
| Pool configuration and connection lifecycle | epic | - | implemented | verified | smoke | `cargo test -p pg test_pool` |

### Query Row And Type Mapping

ID: query-row-and-type-mapping
Type: RuntimeTool
Surfaces: Rust API: `QueryBuilder` + row/type modules - parameterized CRUD, row extraction, and value conversion
EC Dimensions: behavior: `cargo test -p pg test_query_builder` - query builder and CRUD behavior; security: `cargo test -p pg test_security` - parameterized query safety
Root WI: -
Status: verified
Required Verification: smoke
Promise:
pgkit exposes parameterized SQL query construction, CRUD helpers, row representation, and Mamba-to-PostgreSQL type conversion as a Rust-native library surface.
Gate Inventory: `cargo test -p pg test_query_builder`; `cargo test -p pg test_row_crud`; `cargo test -p pg test_types`; `cargo test -p pg test_security`

| Work Root | Kind | WI | Impl | Verification | Maturity | Gate / Evidence |
|---|---|---:|---|---|---|---|
| Query builder and CRUD behavior | epic | - | implemented | verified | smoke | `cargo test -p pg test_query_builder`; `cargo test -p pg test_row_crud`; `cargo test -p pg test_types`; `cargo test -p pg test_security` |
| Type conversion and SQL injection guardrails | epic | - | implemented | verified | smoke | `cargo test -p pg test_types`; `cargo test -p pg test_security` |

### Transaction And Migration Control

ID: transaction-and-migration-control
Type: RuntimeTool
Surfaces: Rust API: transaction and migrate modules - ACID transactions, savepoints, and up/down migrations
EC Dimensions: behavior: `cargo test -p pg test_transaction` - transaction lifecycle; stability: `cargo test -p pg test_migration` - migration ordering and rollback behavior
Root WI: -
Status: verified
Required Verification: smoke
Promise:
pgkit manages PostgreSQL transactions and migrations with explicit transaction lifecycle, savepoint, and up/down migration contracts.
Gate Inventory: `cargo test -p pg test_transaction`; `cargo test -p pg test_migration`

| Work Root | Kind | WI | Impl | Verification | Maturity | Gate / Evidence |
|---|---|---:|---|---|---|---|
| Transaction lifecycle and savepoints | epic | - | implemented | verified | smoke | `cargo test -p pg test_transaction`; `cargo test -p pg test_migration` |
| Migration ordering and rollback behavior | epic | - | implemented | verified | smoke | `cargo test -p pg test_migration` |

### Schema And ORM Introspection

ID: schema-and-orm-introspection
Type: RuntimeTool
Surfaces: Rust API: schema and orm modules - table metadata, constraints, relations, and ORM model mapping
EC Dimensions: behavior: `cargo test -p pg test_schema` - schema introspection and ORM metadata contracts
Root WI: -
Status: verified
Required Verification: smoke
Promise:
pgkit can inspect PostgreSQL schema metadata and map it into ORM-facing table, constraint, relation, and model contracts.
Gate Inventory: `cargo test -p pg test_schema`; `cargo test -p pg test_orm`; `cargo test -p pg test_relations`; `cargo test -p pg test_constraints`

| Work Root | Kind | WI | Impl | Verification | Maturity | Gate / Evidence |
|---|---|---:|---|---|---|---|
| Schema metadata and constraints | epic | - | implemented | verified | smoke | `cargo test -p pg test_schema`; `cargo test -p pg test_orm`; `cargo test -p pg test_relations`; `cargo test -p pg test_constraints` |
| ORM relation mapping | epic | - | implemented | verified | smoke | `cargo test -p pg test_orm`; `cargo test -p pg test_relations` |

### Blocking Facade For Mamba Bindings

ID: blocking-facade-for-mamba-bindings
Type: RuntimeTool
Surfaces: Rust API: blocking facade helpers - sync wrappers consumed by `mambalibs.pg` binding code
EC Dimensions: behavior: `cargo test -p pg test_blocking_facade_shape` - async/blocking API shape parity
Root WI: -
Status: verified
Required Verification: smoke
Promise:
pgkit exposes blocking wrapper shapes that let the sibling `mambalibs.pg` binding reuse the async PostgreSQL core from synchronous Mamba-facing calls.
Gate Inventory: `cargo test -p pg test_blocking`; `cargo test -p pg test_async_blocking_parity`; `cargo test -p pg test_blocking_facade_shape`

| Work Root | Kind | WI | Impl | Verification | Maturity | Gate / Evidence |
|---|---|---:|---|---|---|---|
| Async/blocking API shape parity | epic | - | implemented | verified | smoke | `cargo test -p pg test_blocking`; `cargo test -p pg test_async_blocking_parity`; `cargo test -p pg test_blocking_facade_shape` |

### Bulk Operations And Metrics

ID: bulk-operations-and-metrics
Type: RuntimeTool
Surfaces: Rust API: bulk driver and metrics modules - batch execution support and operational counters
EC Dimensions: behavior: `cargo test -p pg test_bulk_ops` - bulk operation behavior; efficiency: `cargo test -p pg test_benchmark` - benchmark harness coverage
Root WI: -
Status: verified
Required Verification: smoke
Promise:
pgkit includes bulk operation support and metrics hooks for tracking PostgreSQL core behavior and performance-sensitive paths.
Gate Inventory: `cargo test -p pg test_bulk_ops`; `cargo test -p pg test_benchmark`; apps/mamba/mambalibs/pgkit/pg/benches/main.rs

| Work Root | Kind | WI | Impl | Verification | Maturity | Gate / Evidence |
|---|---|---:|---|---|---|---|
| Bulk operation behavior | epic | - | implemented | verified | smoke | `cargo test -p pg test_bulk_ops`; `cargo test -p pg test_benchmark`; apps/mamba/mambalibs/pgkit/pg/benches/main.rs |
| Benchmark and metric evidence | epic | - | implemented | verified | smoke | `cargo test -p pg test_benchmark`; apps/mamba/mambalibs/pgkit/pg/benches/main.rs |

## Overview

`apps/mamba/mambalibs/pgkit/pg` owns pgkit's source/logic layer: PostgreSQL driver
access, ORM/query primitives, schema migration, validation, metrics, and the
blocking facade used by language interfaces. Mamba-facing bindings live in the
sibling `apps/mamba/mambalibs/pgkit/binding` crate and expose this core as
`mambalibs.pg`.

- **Zero Mamba heap pressure**: SQL generation and serialization stay in Rust
- **Native async I/O**: Tokio + SQLx drive PostgreSQL access
- **Sync facade**: Binding layers reuse explicit blocking wrappers
- **Parallel processing**: For bulk operations using Rayon
- **Security-first**: Input validation to prevent SQL injection
- **Type safety**: Full type mapping between Mamba values and PostgreSQL

## Architecture

```
Mamba source layer (models, queries, migrations)
          |
     Mamba interface layer (mambalibs.pg)
          |
   Native pgkit core (projects/pgkit/pg/src/)
          |
        SQLx (PostgreSQL driver)
```

## Modules

- **connection**: Connection pooling and management
- **types**: Type mapping between Mamba values and PostgreSQL
- **row**: Row representation for query results
- **query**: Type-safe SQL query builder
- **transaction**: Transaction management with ACID guarantees
- **migration**: Database migration management
- **schema**: Schema introspection utilities

## Type Mapping

| Mamba Type | PostgreSQL Type | ExtractedValue Variant |
|------------|-----------------|------------------------|
| None | NULL | Null |
| bool | BOOLEAN | Bool |
| int (small) | INTEGER | Int |
| int (large) | BIGINT | BigInt |
| float | DOUBLE PRECISION | Double |
| str | TEXT | String |
| bytes | BYTEA | Bytes |
| uuid.UUID | UUID | Uuid |
| datetime.date | DATE | Date |
| datetime.time | TIME | Time |
| datetime.datetime (naive) | TIMESTAMP | Timestamp |
| datetime.datetime (aware) | TIMESTAMPTZ | TimestampTz |
| dict/list | JSONB | Json |
| list[T] | ARRAY | Array |
| Decimal | NUMERIC | Decimal |

## Features

- Connection pooling with configurable min/max connections
- Parameterized queries to prevent SQL injection
- Transaction support with savepoints
- Schema introspection (tables, columns, indexes, foreign keys)
- Migration management (up/down migrations)
- Comprehensive error handling with type conversion

## Status

**Version**: 0.1.0-alpha (Pre-release)

### Implemented
- Connection pooling and management ✅
- Type mapping between Mamba values and PostgreSQL ✅
- Basic CRUD operations (insert, fetch, update, delete) ✅
- Query builder with WHERE, ORDER BY, LIMIT, OFFSET ✅
- Transaction support with isolation levels ✅
- Raw SQL execution ✅
- Schema introspection (tables, columns, indexes, foreign keys) ✅
- Migration management (up/down migrations) ✅
- Advanced query features (JOINs) ✅

### Roadmap (Future Releases)
- Subquery support
- Bulk operations optimization with Rayon
- Connection pool metrics and monitoring
- Prepared statement caching
- Advanced migration features (rollback chains, dry-run)

## Usage

Rust callers depend on the core crate:

```toml
[dependencies]
pg = { path = "projects/pgkit/pg" }
```

Mamba callers import through the interface crate:

```python
from mambalibs.pg import connect, execute, Session
from mambalibs.pg.migrate import MigrationRunner, Migration
```

## Quick Start

```rust
use pg::{Connection, PoolConfig, QueryBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create connection pool
    let conn = Connection::new(
        "postgresql://user:pass@localhost/db",
        PoolConfig::default(),
    )
    .await?;

    // Insert data
    let query = QueryBuilder::new("users")
        .insert(vec![
            ("name", "Alice".into()),
            ("email", "alice@example.com".into()),
        ]);
    let _ = query;

    // Query data
    let query = QueryBuilder::new("users")
        .where_eq("name", "Alice")
        .limit(10);
    let _ = query;
    let rows: Vec<()> = Vec::new();

    println!("Found {} users", rows.len());
    Ok(())
}
```

## Dependencies

- **sqlx** 0.8: Async PostgreSQL driver with connection pooling
- **tokio**: Async runtime
- **serde**: Serialization framework
- **uuid**: UUID support
- **chrono**: Date/time handling
- **rust_decimal**: Precise decimal arithmetic

## Development

```bash
# Check compilation
cargo check -p pg

# Run unit tests (no database required)
cargo test -p pg

# Run integration tests (requires PostgreSQL)
cargo test -p pg --test test_transaction

# Run all tests including ignored ones
cargo test -p pg -- --ignored

# Lint
cargo clippy -p pg
```

### PostgreSQL Setup (macOS)

Using Homebrew:

```bash
# Install PostgreSQL
brew install postgresql@15

# Start PostgreSQL service
brew services start postgresql@15

# Create test database
createdb test_db

# Verify connection
psql -d test_db -c "SELECT version();"
```

Set the database URL environment variable:

```bash
# Default connection (local socket)
export DATABASE_URL="postgresql://localhost/test_db"

# Or with explicit credentials
export DATABASE_URL="postgresql://username:password@localhost:5432/test_db"
```

### PostgreSQL Setup (Docker)

```bash
# Start PostgreSQL container
docker run -d --name postgres-test \
  -p 5432:5432 \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=test_db \
  postgres:15

# Set connection URL
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/test_db"

# Run tests
cargo test -p pg --test test_transaction

# Cleanup
docker stop postgres-test && docker rm postgres-test
```

### Running Migration Tests

The migration integration tests require a running PostgreSQL database. They are marked with `#[ignore]` to prevent them from running in CI without a database.

```bash
# Start PostgreSQL (using Docker)
docker run -d --name postgres-test -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres:15

# Create test database
docker exec -it postgres-test psql -U postgres -c "CREATE DATABASE test_db;"

# Run migration tests
export POSTGRES_URL="postgresql://postgres:postgres@localhost/test_db"
cargo test -p pg --test test_migration -- --ignored

# Clean up
docker stop postgres-test && docker rm postgres-test
```

Each test creates its own migration table with a unique name (e.g., `_test_migrations_apply`) to avoid conflicts when running tests in parallel. Tables are automatically cleaned up after each test.

## License

MIT
