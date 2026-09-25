# Mesh

## Brief

Mesh is the relationship/property-graph service in the Axiom service stack.
It owns typed node/edge storage with properties, and traversal/path query
over that graph. Like `lumen`, mesh is log-driven and derived: writes fold
through a raft-replicated log into a separate, rebuildable local index. The
caller owns the system of record; mesh never becomes the durable owner of
relationship data.

It is intentionally separate from its siblings: `beam` owns vector ANN
search, `lumen` owns lexical/semantic/perceptual search and dedup, and
`cube` owns OLAP-style columnar aggregation. Mesh owns the graph shape:
nodes, typed edges, properties, and traversal.

## Contributing

Repo-wide authoring rules live in
[../../CONTRIBUTING.md](../../CONTRIBUTING.md).

## Capabilities

A promise with no gate under it is not claimed.

The capability headings and tables below were machine-readable input for the
`aw` capability verb, which was deleted with the binary. Nothing reads them
now, so they are a record of what was claimed rather than a contract a tool
still enforces.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Property Graph Core | #1969 | typed node/edge storage with properties |
| Traversal & Path Query | #1969 | neighbor/path/pattern traversal execution |
| Log-Driven Derived Index | #1969 | raft log fold into a rebuildable local index; never system of record |
| HTTP/2 API List | #1969 | h2c/OpenAPI endpoint inventory |
| Kubernetes-Native Deployment | #1969 | dedicated StatefulSet/operator shape |
| Primary Replicas | #1969 | raft-backed shard ownership |
| CLI Interface | #1969 | `mesh` CLI for graph write/query/admin and agent docs — standard llm/upgrade/issue verbs plus placeholder domain verbs landed (#1970); real graph write/query verbs still pending |
| Long-Running Stability | #1969 | write/traversal soak and recovery gates |
| Security Hardening | #1969 | collection authz, tenant isolation, audit, and secret rotation |
| Competitor Feature Parity | #1969 | Neo4j/JanusGraph/DGraph/Neptune-style feature matrix |
| Competitor Performance | #1969 | pinned graph query baseline, rerun only on scope change |

### CLI Interface

Mesh ships an agent-drivable CLI for node/edge writes, traversal/path query,
and admin workflows while following the repository-wide CLI convention.

- Root WI: #1969
- Surfaces: CLI: `mesh llm`, `mesh upgrade`, `mesh issue`, graph
  node/edge/query verbs, and admin/debug verbs.
- Gate — behavior: pending CLI convention gate - required standard verbs, graph
  workflow ergonomics, and offline agent docs
- Gate: passing: apps/mesh/tests/cli_contract.rs (`cargo test -p mesh`)

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| mesh-cli-convention-and-graph-verbs | epic | #1969 | pending CLI convention gate |
| mesh-cli-shell-scaffold | change | #1970 | `cargo test -p mesh` (5/5 passing); apps/mesh/tests/cli_contract.rs |

### Long-Running Stability

Mesh remains stable under sustained write and traversal load without corrupting
the derived index or losing committed log entries.

- Root WI: #1969
- Surfaces: Runtime: node/edge writer, traversal executor, shard rebalancer,
  snapshot, and recovery paths.
- Gate — stability: pending long-running graph gate - soak, restart,
  index-rebuild recovery, bounded memory, and backpressure behavior
- Source: `pending: apps/mesh/tests/long_running_stability.rs`
- Evidence: pending long-running graph gate

### Security Hardening

Mesh protects graph collections and query APIs with explicit authorization,
auditability, network policy, and managed secret rotation.

- Root WI: #1969
- Surfaces: HTTP/K8s: collection/query authn/authz, tenant/collection
  isolation, network policy, audit events, secret rotation, and request limits.
- Gate — behavior: pending security gate - auth failure cases, collection
  isolation, audit emission, secret rotation, and abuse limits
- Source: `pending: apps/mesh/tests/security_hardening.rs`
- Evidence: pending security hardening gate

### Competitor Feature Parity

Mesh keeps an explicit graph feature matrix against established graph systems,
with comparison scope changed only when product requirements change.

- Root WI: #1969
- Surfaces: Docs/Test: graph feature matrix against Neo4j, JanusGraph, DGraph,
  and Neptune-style graph services.
- Gate — behavior: pending competitor feature gate - node/edge model, property
  schema, traversal/path query, indexing, and retention
- Source: `pending: apps/mesh/benchmark/competitor-feature-matrix.md`
- Evidence: pending competitor feature gate

### Competitor Performance

Mesh maintains a pinned competitor performance baseline and reruns external
benchmarks only when the comparison scope changes or a human explicitly asks.

- Root WI: #1969
- Surfaces: Meter/Vat: write throughput, traversal/path-query latency,
  index-rebuild cost, and shard rebalance efficiency.
- Gate — efficiency: pending competitor performance gate - pinned external
  baseline and Mesh-owned graph measurements
- Source: `pending: apps/mesh/benchmark/competitor-performance-baseline.md`,
  `pending: apps/mesh/meter-mesh-query.toml`
- Evidence: pending competitor performance gate

### Property Graph Core

Mesh stores typed nodes and edges with properties as a caller-owned
relationship graph, with explicit schema and durability semantics.

- Root WI: #1969
- Surfaces: HTTP: `/v1/collections/{collection}/nodes`,
  `/v1/collections/{collection}/edges` - typed node/edge upsert and delete with
  properties.
- Gate — behavior: pending graph write conformance gate - schema/type
  validation, edge endpoint integrity, and durable append
- Source: `pending: apps/mesh/tests/property_graph_core.rs`
- Evidence: pending graph write conformance gate

### Traversal & Path Query

Mesh answers neighbor, pattern, and path queries over the stored graph without
depending on Lumen search indexes or Cube analytical storage.

- Root WI: #1969
- Surfaces: HTTP: `/v1/collections/{collection}/traverse`,
  `/v1/collections/{collection}/path` - neighbor expansion, filtered traversal,
  and shortest/bounded path query.
- Gate — behavior: pending traversal query gate - depth/filter bounds, pattern
  match, path result correctness, and deterministic ordering
- Gate — efficiency: pending meter traversal gate - retained latency/resource
  floors
- Source: `pending: apps/mesh/tests/traversal_and_path_query.rs`,
  `pending: apps/mesh/meter-mesh-query.toml`
- Evidence: pending traversal query gate

### Log-Driven Derived Index

Mesh treats its graph index as derived and rebuildable from the replicated log
at all times; it never becomes the durable owner of relationship data.

- Root WI: #1969
- Surfaces: Raft/Storage: durable append log via `core/storage-durable`, folded
  into a separate rebuildable local graph index; the caller remains system of
  record.
- Gate — behavior: pending derived-index gate - log replay determinism and full
  index rebuild from log
- Gate — stability: pending rebuild-under-load gate
- Source: `pending: apps/mesh/tests/log_driven_derived_index.rs`
- Evidence: pending derived-index gate

### HTTP/2 API List

Mesh exposes a compact h2c/OpenAPI API list for graph write, traversal/path
query, and operator workflows.

- Root WI: #1969
- Surfaces: HTTP: `/healthz`, `/readyz`, `/metrics`, `/openapi.json`, `/docs`,
  node/edge/query/admin routes.
- Gate — behavior: pending h2c/OpenAPI route-list gate - probes, metrics,
  OpenAPI, and route inventory
- Source: `pending: apps/mesh/tests/http_api.rs`
- Evidence: pending h2c/OpenAPI route-list gate

### Kubernetes-Native Deployment

Mesh runs as a dedicated k8s-native graph service with operator-managed
storage, backup policy, and shard lifecycle.

- Root WI: #1969
- Surfaces: K8s: dedicated StatefulSet/operator topology for shards, storage,
  probes, backups, and PDBs.
- Gate — behavior: pending kustomize/operator render gate - CRD, operator, and
  instance render
- Gate — stability: pending kind graph dogfood
- Source: `pending: apps/mesh/k8s`
- Evidence: pending k8s render/dogfood gates

### Primary Replicas

Mesh replicates shard ownership through raft so write and traversal control
state survives failover.

- Root WI: #1969
- Surfaces: Raft: shard ownership, membership, and rebalance job state over
  `core/raft-core` and `core/raft-runtime`.
- Gate — stability: pending raft graph failover gate - shard ownership and
  derived index survive failover
- Source: `pending: apps/mesh/tests/raft_shard_ownership.rs`
- Evidence: pending raft shard failover gate
