# Cube

## Brief

Cube is the OLAP service in the Axiom service stack.

It owns columnar ingest, scan/filter/project, group-by aggregates, rollups,
partition pruning, and analytical query execution. It is intentionally separate
from `lumen`: Lumen is a low-latency search and dedup index; Cube is a
columnar analytical service for aggregations, dimensional queries, and retained
measurement/event facts.

## Boundaries

- `lumen` owns search, ranking, duplicate detection, and vector/text retrieval.
- `cube` owns columnar facts, analytical scans, grouping, rollups, and time
  partitions.
- `tape` may feed Cube from replayed topics, but Cube owns analytical storage.
- `meter` and `arena` can write facts into Cube, but measurement semantics stay
  with those tools.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Columnar Ingest | #767 | append facts into partitioned columnar storage |
| Analytical Query API | #767 | scan/filter/project/group-by/aggregate over facts |
| Rollups And Partitions | #767 | time partition pruning and materialized rollups |
| HTTP/2 API List | #767 | h2c/OpenAPI endpoint inventory |
| Kubernetes-Native Deployment | #767 | dedicated StatefulSet/operator shape |
| Primary Replicas | #767 | raft-backed metadata and partition ownership |
| CLI Interface | #767 | `cube` CLI for ingest/query/admin and agent docs |
| Long-Running Stability | #767 | ingest/query/rollup soak and recovery gates |
| Security Hardening | #767 | table authz, tenant isolation, audit, and secret rotation |
| Competitor Feature Parity | #767 | ClickHouse/DuckDB-style OLAP feature matrix |
| Competitor Performance | #767 | pinned OLAP query baseline, rerun only on scope change |

### CLI Interface

Cube ships an agent-drivable CLI for ingest, query, rollup, and admin workflows
while following the repository-wide CLI convention.

- Root WI: #767
- Surfaces: CLI: `cube llm`, `cube upgrade`, `cube issue`, table
  ingest/query/rollup, and admin/debug verbs.
- Gate — behavior: pending CLI convention gate - required standard verbs,
  analytical workflow ergonomics, and offline agent docs
- Source: `pending: apps/cube/tests/cli_contract.rs`
- Evidence: pending CLI convention gate

### Long-Running Stability

Cube remains stable under sustained ingest, analytical query, and rollup load
without corrupting partitions or losing committed metadata.

- Root WI: #767
- Surfaces: Runtime: ingest writer, query executor, rollup refresher, partition
  compactor, snapshot, and recovery paths.
- Gate — stability: pending long-running OLAP gate - soak, restart, rollup
  recovery, bounded memory, and backpressure behavior
- Source: `pending: apps/cube/tests/long_running_stability.rs`
- Evidence: pending long-running OLAP gate

### Security Hardening

Cube protects analytical tables and query APIs with explicit authorization,
auditability, network policy, and managed secret rotation.

- Root WI: #767
- Surfaces: HTTP/K8s: table/query authn/authz, tenant/table isolation, network
  policy, audit events, secret rotation, and request limits.
- Gate — behavior: pending security gate - auth failure cases, table isolation,
  audit emission, secret rotation, and abuse limits
- Source: `pending: apps/cube/tests/security_hardening.rs`
- Evidence: pending security hardening gate

### Competitor Feature Parity

Cube keeps an explicit OLAP feature matrix against established analytical
systems, with comparison scope changed only when product requirements change.

- Root WI: #767
- Surfaces: Docs/Test: OLAP feature matrix against ClickHouse, DuckDB, and
  cloud warehouse-style analytical services.
- Gate — behavior: pending competitor feature gate - ingest, schema,
  scan/filter/project, aggregates, rollups, partition pruning, and retention
- Source: `pending: apps/cube/benchmark/competitor-feature-matrix.md`
- Evidence: pending competitor feature gate

### Competitor Performance

Cube maintains a pinned competitor performance baseline and reruns external
benchmarks only when the comparison scope changes or a human explicitly asks.

- Root WI: #767
- Surfaces: Meter/Vat: ingest throughput, scan/filter/group-by latency, rollup
  refresh cost, and partition pruning efficiency.
- Gate — efficiency: pending competitor performance gate - pinned external
  baseline and Cube-owned OLAP measurements
- Source: `pending: apps/cube/benchmark/competitor-performance-baseline.md`,
  `pending: apps/cube/meter-cube-query.toml`
- Evidence: pending competitor performance gate

### Columnar Ingest

Cube accepts fact batches into columnar storage with explicit schema,
partition, and durability semantics.

- Root WI: #767
- Surfaces: HTTP: `/v1/tables/{table}/ingest` - append fact batches into
  partitioned columnar storage.
- Gate — behavior: pending ingest conformance gate - schema evolution,
  partition routing, and durable append
- Source: `pending: apps/cube/tests/columnar_ingest.rs`
- Evidence: pending ingest conformance gate

### Analytical Query API

Cube answers OLAP-style queries over retained fact tables without depending on
Lumen search indexes.

- Root WI: #767
- Surfaces: HTTP: `/v1/query` - scan, filter, project, group-by, aggregate,
  sort, and page analytical results.
- Gate — behavior: pending analytical query gate - filters, group-by,
  aggregates, pagination, and deterministic result ordering
- Gate — efficiency: pending meter query gate - retained latency/resource
  floors
- Source: `pending: apps/cube/tests/query_api.rs`,
  `pending: apps/cube/meter-cube-query.toml`
- Evidence: pending analytical query gate

### Rollups And Partitions

Cube manages time partitions and materialized rollups so analytical queries can
stay bounded as data grows.

- Root WI: #767
- Surfaces: HTTP/Admin: rollup definitions, refresh jobs, retention windows,
  and partition pruning.
- Gate — behavior: pending rollup gate - rollup freshness, invalidation,
  partition pruning, and retention behavior
- Source: `pending: apps/cube/tests/rollups_partitions.rs`
- Evidence: pending rollup/partition gate

### HTTP/2 API List

Cube exposes a compact h2c/OpenAPI API list for table ingest, analytical query,
rollup, and operator workflows.

- Root WI: #767
- Surfaces: HTTP: `/healthz`, `/readyz`, `/metrics`, `/openapi.json`, `/docs`,
  table ingest/query/admin routes.
- Gate — behavior: pending h2c/OpenAPI route-list gate - probes, metrics,
  OpenAPI, and route inventory
- Source: `pending: apps/cube/tests/http_api.rs`
- Evidence: pending h2c/OpenAPI route-list gate

### Kubernetes-Native Deployment

Cube runs as a dedicated k8s-native OLAP service with operator-managed storage,
backup policy, and partition/rollup lifecycle.

- Root WI: #767
- Surfaces: K8s: dedicated StatefulSet/operator topology for partitions,
  storage, probes, backups, and PDBs.
- Gate — behavior: pending kustomize/operator render gate - CRD, operator, and
  instance render
- Gate — stability: pending kind analytical dogfood
- Source: `pending: apps/cube/k8s`
- Evidence: pending k8s render/dogfood gates

### Primary Replicas

Cube replicates metadata and partition/rollup ownership through raft so query
and ingest control state survives failover.

- Root WI: #767
- Surfaces: Raft: metadata, partition ownership, and rollup job state over
  `core/raft-core` and `core/raft-runtime`.
- Gate — stability: pending raft OLAP failover gate - metadata and partition
  ownership survive failover
- Source: `pending: apps/cube/tests/raft_metadata.rs`
- Evidence: pending raft metadata failover gate
