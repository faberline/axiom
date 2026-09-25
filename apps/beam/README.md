# Beam

## Brief

Beam is the GPU vector database in the Axiom service stack.

It owns vector-first storage, GPU ANN indexing, batch ingest, compaction/rebuild,
and vector query execution. It is intentionally separate from `lumen`: Lumen is
a mixed search service across exact, lexical, semantic, perceptual, and
duplicate search; Beam is a GPU-native vector service optimized for vector
indexes and GPU memory tiers.

## Boundaries

- `lumen` owns mixed search and ranking workflows.
- `beam` owns vector-first collections, GPU ANN indexes, and GPU batch
  query/ingest execution.
- `keep` can store large external payloads; Beam stores vectors and vector
  metadata needed for ANN.
- `cube` owns analytical aggregates; Beam owns nearest-neighbor retrieval.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| GPU Vector Index | #769 | GPU flat + IVF-flat + IVF-PQ ANN on wgpu/Metal, recall verified vs oracle; durable segments/memory-tiers/rebuild pending |
| Batch Ingest And Rebuild | #769 | batch upsert + CRUD + persistence (save/load); compaction present; offline rebuild pending |
| Vector Query API | #769 | kNN + metadata filters over REST (`/v1/collections/{c}/query`); recall gates via ivf_recall |
| HTTP/2 API List | #769 | h2c REST live (`beam serve`): health/collections/vectors/query; OpenAPI pending |
| Kubernetes-Native Deployment | #769 | dedicated StatefulSet/operator shape with GPU scheduling |
| Primary Replicas | #769 | raft-backed metadata and index lifecycle ownership |
| CLI Interface | #772 | shell + std verbs (llm/upgrade/issue) landed; vector verbs pending |
| Long-Running Stability | #769 | GPU index soak, rebuild, failover, and recovery gates |
| Security Hardening | #769 | collection authz, tenant isolation, audit, and secret rotation |
| Competitor Feature Parity | #769 | matrix landed (parity checklist vs Milvus/Qdrant/Faiss/cuVS/pgvector); features tracked F1-F5 |
| Competitor Performance | #769 | pinned baseline: beam-GPU beats faiss-CPU batched flat at n>=100k (1.06-2.3x, recall 1.000); loses single-query latency |
| Stateful Service Workload | #769 | baseline stateful service workload projection |

### Stateful Service Workload

Beam projects the shared stateful-service workload baseline.

- Root WI: #769
- Surfaces: Durable index, checkpoint state, and stateful replica lifecycle.
- Gate — behavior: pending stateful service workload projection
- Source: `pending: apps/beam/tests/persistence.rs`
- Evidence: pending stateful service workload projection

### CLI Interface

Beam ships an agent-drivable CLI for vector collection, ingest, query, index,
and admin workflows while following the repository-wide CLI convention.

- Root WI: #772
- Surfaces: CLI: `beam llm`, `beam upgrade`, `beam issue`, vector
  ingest/query/index, and admin/debug verbs.
- Gate — behavior: pending CLI convention gate - required standard verbs,
  vector workflow ergonomics, and offline agent docs
- Gate: passing: apps/beam/tests/cli_contract.rs (R1–R6, `cargo test -p beam`
  green)

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| beam-cli-shell | change | #772 | apps/beam/tests/cli_contract.rs (R1–R6 green) |
| beam-cli-convention-and-vector-verbs | change | #772 | apps/beam/tests/cli_contract.rs (R1–R6 green) |
| beam-vector-verbs | epic | #769 | pending vector ingest/query/index verbs |

### Long-Running Stability

Beam remains stable under sustained vector ingest, query, index rebuild, and
GPU memory pressure without serving corrupted index state.

- Root WI: #769
- Surfaces: Runtime: GPU index loader, batch ingest, compaction/rebuild worker,
  query executor, snapshot, and recovery paths.
- Gate — stability: pending long-running vector gate - soak, restart, GPU
  memory recovery, rebuild safety, bounded memory, and backpressure
- Source: `pending: apps/beam/tests/long_running_stability.rs`
- Evidence: pending long-running vector gate

### Security Hardening

Beam protects vector collections and GPU query APIs with explicit
authorization, auditability, network policy, and managed secret rotation.

- Root WI: #769
- Surfaces: HTTP/K8s: collection/query authn/authz, tenant/collection
  isolation, network policy, audit events, secret rotation, and request limits.
- Gate — behavior: pending security gate - auth failure cases, collection
  isolation, audit emission, secret rotation, and abuse limits
- Source: `pending: apps/beam/tests/security_hardening.rs`
- Evidence: pending security hardening gate

### Competitor Feature Parity

Beam keeps an explicit GPU/vector feature matrix against established vector
systems, with comparison scope changed only when product requirements change.

- Root WI: #769
- Surfaces: Docs/Test: GPU/vector database feature matrix against Milvus,
  Qdrant, Faiss-style, and GPU ANN services.
- Gate — behavior: pending competitor feature gate - collection lifecycle,
  vector ingest, ANN index build/load, filters, recall diagnostics, rebuild,
  and GPU scheduling
- Source:
  `landed: apps/beam/benchmark/competitor-feature-matrix.md (parity checklist + gaps)`
- Evidence: pending competitor feature gate

### Competitor Performance

Beam maintains a pinned competitor performance baseline and reruns external
benchmarks only when the comparison scope changes or a human explicitly asks.

- Root WI: #769
- Surfaces: Meter/Vat: vector ingest throughput, index build time, query
  p50/p95, recall, GPU memory pressure, and rebuild cost.
- Gate — efficiency: pending competitor performance gate - pinned external
  baseline and Beam-owned vector measurements
- Source:
  `landed: apps/beam/benchmark/competitor-performance-baseline.md (beam-GPU beats faiss-CPU batched flat at n>=100k`,
  `independently re-verified)`, `pending: apps/beam/meter-beam-query.toml`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| gpu-vector-competitor-performance-baseline | epic | #769 | pending competitor performance gate |
| four-pillar-throughput-saturation-ddd | change | #769 | pending throughput architecture |
| competitive-throughput-ddd-zero-cost | change | #769 | pending throughput architecture |
| competitive-throughput-gate-saturate-four-pillars | change | #769 | pending throughput architecture |
| competitive-throughput-out-of-core-scaling | change | #769 | pending throughput architecture |
| competitive-throughput-gpu-scaling | change | #769 | pending throughput architecture |

### GPU Vector Index

Beam manages GPU-native vector indexes with explicit memory-tier and rebuild
semantics rather than treating vector search as a Lumen side path.

- Root WI: #769
- Surfaces: GPU engine: vector collection shards, ANN index build/load, GPU
  memory tier, and host spill policy.
- Gate — behavior: pending vector index conformance gate - build/load/search
  correctness
- Gate — efficiency: pending GPU meter gate - throughput, memory, and latency
  floors
- Source:
  `passing: apps/beam/tests/gpu_matches_cpu.rs (GPU flat top-k == CPU oracle, L2/Dot/Cosine)`,
  `passing: apps/beam/tests/ivf_recall.rs (IVF-flat exact at full probe`,
  `recall grows with nprobe`, `GPU ADC == CPU ADC`, `scaling)`,
  `pending: apps/beam/tests/gpu_vector_index.rs (durable index build/load/rebuild lifecycle)`,
  `pending: apps/beam/meter-beam-gpu.toml`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| gpu-flat-knn-wgpu-metal | change | #769 | apps/beam/tests/gpu_matches_cpu.rs (Apple M1 Max/Metal, recall 1.000) |
| gpu-ivf-pq-ann | change | #769 | apps/beam/tests/ivf_recall.rs (IVF-flat 2.7× faster lossless; IVF-PQ ADC exact, recall tunable) |
| gpu-ann-index-lifecycle | epic | #769 | pending durable segments, memory tiers, rebuild, and GPU meter gates |

### Batch Ingest And Rebuild

Beam ingests vectors in batches and rebuilds/promotes ANN indexes without
serving partially corrupted index state.

- Root WI: #769
- Surfaces: HTTP/Admin: batch ingest, segment compaction, background rebuild,
  and index promotion controls.
- Gate — behavior: pending ingest/rebuild gate - idempotent batch ingest,
  compaction, rebuild, and promotion
- Source: `pending: apps/beam/tests/batch_ingest_rebuild.rs`
- Evidence: pending ingest/rebuild conformance gate

### Vector Query API

Beam serves vector nearest-neighbor queries with explicit recall and latency
gates, keeping lexical/perceptual/duplicate search outside its scope.

- Root WI: #769
- Surfaces: HTTP: `/v1/collections/{collection}/query` - nearest-neighbor
  search, metadata filters, top-k, and recall diagnostics.
- Gate — behavior: pending vector query gate - top-k, filters, recall fixtures,
  and deterministic pagination
- Gate — efficiency: pending GPU query gate - p50/p95/top-k throughput
- Source: `pending: apps/beam/tests/vector_query.rs`,
  `pending: apps/beam/meter-beam-query.toml`
- Evidence: pending vector query and GPU query gates

### HTTP/2 API List

Beam exposes a compact h2c/OpenAPI API list for vector collection, index,
ingest, query, and operator workflows. Every HTTP request is correlatable end
to end: W3C `traceparent` is honored when present and a local root trace is
created when absent, with the ids flowing into every request span and
structured log line. Server-Timing per-response latency attribution (the shared
`service-http::server_timing` contract) is not yet wired into beam's HTTP stack
— that lands in a separate #2490 adoption batch.

- Root WI: #769
- Surfaces: HTTP: `/healthz`, `/readyz`, `/metrics`, `/openapi.json`, `/docs`,
  vector collection/index/query routes.; Logs: structured stdout with
  per-request trace correlation — the shared `service-http` trace layer
  (`service_http::trace_layer()`) accepts a valid W3C version-00 `traceparent`
  (invalid input is treated as absent) and generates a fresh local root context
  otherwise, so every request span and log line carries
  `trace_id`/`span_id`/`parent_span_id`/`trace_flags`.; HTTP: Server-Timing
  response attribution — shared `service-http::server_timing` contract
  (`Server-Timing: app;dur=` per-response latency), wiring pending (#2490
  adoption batch).
- Gate — behavior: pending h2c/OpenAPI route-list gate - probes, metrics,
  OpenAPI, and route inventory
- Gate: passing (trace-context accept/generate): `bash scripts/faberline-core-test.sh service-http`
  (faberline/core crates/service-http/src/transport.rs) — beam wires
  `service_http::trace_layer()` in apps/beam/src/service.rs
- Source: `pending: apps/beam/tests/http_api.rs`,
  `no beam-owned trace-context test exists yet`
- Evidence: pending h2c/OpenAPI route-list gate

### Kubernetes-Native Deployment

Beam runs as a dedicated k8s-native GPU vector service with operator-managed
GPU scheduling, storage, backup policy, and index lifecycle.

- Root WI: #769
- Surfaces: K8s: dedicated StatefulSet/operator topology for GPU nodes,
  storage, probes, backups, PDBs, and scheduling constraints.
- Gate — behavior: pending kustomize/operator render gate - CRD, operator, and
  GPU instance render
- Gate — stability: pending kind/GPU deployment dogfood
- Source: `pending: apps/beam/k8s`
- Evidence: pending k8s render/dogfood gates

### Primary Replicas

Beam replicates collection metadata and index lifecycle ownership through raft
while GPU index bytes remain service-owned data-plane state.

- Root WI: #769
- Surfaces: Raft: collection metadata, shard ownership, and index lifecycle
  state over `core/raft-core` and `core/raft-runtime`.
- Gate — stability: pending raft vector failover gate - metadata and index
  lifecycle state survive failover
- Source: `pending: apps/beam/tests/raft_metadata.rs`
- Evidence: pending raft metadata failover gate
