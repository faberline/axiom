# axiom

A monorepo of high-performance, Rust-built developer infrastructure. Each
project below is self-contained and ships its own README — follow the links for
details.

## Projects

| Project | What it is |
|---------|------------|
| [som-core](models/som-core/README.md) | SOM (Structured Outcome Model) Core - Model architecture and training pipeline. See [Architecture Design](models/som-core/docs/reference/architecture.md). |
| [cap](apps/cap/README.md) | `cap` keeps heavy local commands (`cargo test`, `uv run`, `pnpm build`, …) from eating the whole machine. |
| [vat](apps/vat/README.md) | `vat` is a headless local development test runner for the one operator Docker was never designed for: a coding/ML agent. |
| [courier](apps/courier/README.md) | `courier` is a stateless, GCP-hosted proxy that centralizes GitHub-issue access for every axiom CLI. |
| [loom](apps/loom/README.md) | Loom is the workflow scheduler in the Axiom service stack. |
| [preview](apps/preview/README.md) | `preview` manages MR-scoped UAT preview environments for GKE. |
| [tape](apps/tape/README.md) | Tape is the topic replay journal in the Axiom service stack. |
| [defer](apps/defer/README.md) | Defer is the Cloud Tasks-like delayed push-queue dispatch service in the Axiom stack. |
| [cube](apps/cube/README.md) | Cube is the OLAP service in the Axiom service stack. |
| [mesh](apps/mesh/README.md) | Mesh is the relationship/property-graph service in the Axiom service stack. |
| [beam](apps/beam/README.md) | Beam is the GPU vector database in the Axiom service stack. |
| [workbench](apps/workbench/README.md) | Terminal-first desktop workbench that launches Claude Code, Codex, or AGY through their native CLIs in a real PTY and adds read-only context beside the session — without owning vendor sessions or AW lifecycle state. |
| [arena](apps/arena/README.md) | N-target competitive comparison runner — fan one workload across targets, ratio + ratchet-gate, one agent-readable JSON report. |
| [guard](apps/guard/README.md) | Security posture gate for the cclab ecosystem. |
| [rig](apps/rig/README.md) | Declarative test-scenario harness engine for the cclab ecosystem. |
| [mamba](apps/mamba/README.md) | Force-typed Python compiler. |
| [sift](apps/sift/README.md) | Sift is the GCP/GKE-first operational event platform in the Axiom stack. |
| [lumen](apps/lumen/README.md) | A K8s-native, log-replicated search specialist. |
| [jet](apps/jet/README.md) | Jet is a Rust-native frontend toolchain. |
| [relay](apps/relay/README.md) | `relay` is the online single-cast pull work-queue broker in the Axiom stack (RabbitMQ/SQS-shaped): a producer publishes a task, a worker pulls (leases) it, runs it, and acks — each message is delivered exactly once to one of the competing consumers, then reclaimed (delete-on-ack). |
| [keep](apps/keep/README.md) | Cloud-native, multi-core key-value / claim-check store — the loom/relay data plane and a Redis / Dragonfly replacement. |
| [pgpool](apps/pgpool/README.md) | `pgpool` is the working app id for Axiom's Kubernetes-native PostgreSQL connection pooler. |
| [meter](apps/meter/README.md) | Local resource measurement for agent-driven Rust development. |

This table is hand-maintained. It used to be spliced in from a generator that no
longer exists, so adding a project under `apps/` adds no row on its own.

## Shared Libraries

Services and tools compose the internal libraries below instead of
reimplementing transport, auth, metrics, codegen, replication, durable local
storage, backup, or operator plumbing locally. Shared service capabilities
belong in `libs/*`; apps supply domain behavior and wiring. Libraries have no
user-facing CLI or release pipeline; those surfaces belong under `apps/`.

The prefix is the map: `server-*` is protocol runtime, `transport-*` is wire
transport, `service-*` is an app-integrated capability, and narrower
`storage-*`, `metrics-*`, `peer-*`, and `raft-*` families name their owned
mechanism directly. Directory, Cargo package, and Rust crate identities move
together; see [Shared-library naming grammar](CONTRIBUTING.md#shared-library-naming-grammar).

| Library | What it is |
|---------|------------|
| [build-stamp](libs/build-stamp/Cargo.toml) | Shared `build.rs` stamping for service CLIs: git short SHA, build epoch, and target triple. |
| [claim-token](libs/claim-token/Cargo.toml) | Scoped claim-check access tokens; issuers sign bounded payload/key access and services verify the scope. |
| [cli-std](libs/cli-std/Cargo.toml) | Shared implementation for the required `llm`, `upgrade`, and `issue` CLI convention. |
| [compass](libs/compass/README.md) | Code-intelligence engine for navigation, analysis, refactoring, and watch workflows. |
| [transport-h2c](libs/transport-h2c/Cargo.toml) | Shared HTTP/2 cleartext transport: client, pool, frame-level manager, connection sizing, and optional per-connection HTTP/1.1+h2c protocol handling. |
| [openapi-codegen](libs/openapi-codegen/Cargo.toml) | Typed TypeScript, Python, and Rust API client generation from OpenAPI 3.0/3.1 documents. |
| [server-lifecycle](libs/server-lifecycle/Cargo.toml) | Protocol-neutral server lifecycle: bind config, shutdown/drain, readiness signals, connection budgets, and metrics hooks. |
| [server-tcp](libs/server-tcp/Cargo.toml) | Shared TCP accept/runtime layer for raw protocols, proxies, and poolers, built on server-lifecycle. |
| [server-http](libs/server-http/Cargo.toml) | Shared listener-level HTTP runtime: composes server-tcp admission/lifecycle with transport-h2c per-connection HTTP/1.1+h2c handling. |
| [service-k8s](libs/service-k8s/Cargo.toml) | Kubernetes service integration: reconcile controller, leader election, workload rendering, stateful capacity planning, and resize primitives. |
| [raft-core](libs/raft-core/Cargo.toml) | Transport- and storage-agnostic, step-driven Raft consensus core. |
| [raft-runtime](libs/raft-runtime/Cargo.toml) | Shared Raft runtime over h2c peer transport with apply, topology, snapshots, compaction, and read-your-write propose. |
| [service-auth](libs/service-auth/Cargo.toml) | Shared request-auth middleware: extract, verify, reject, and inject verified identity into service handlers. |
| [service-backup](libs/service-backup/Cargo.toml) | Shared backup contract: destination and policy schema, sink trait, local and S3-compatible sinks, and runner primitive. |
| [service-executor](libs/service-executor/README.md) | Generic bounded asynchronous executor mechanics; services retain committed ownership, fencing, retry, and outcome policy. |
| [storage-durable](libs/storage-durable/Cargo.toml) | Shared durable local storage primitives: fsync policy, atomic replace, CRC-framed append logs, and sequence-named snapshot stores. |
| [service-observability](libs/service-observability/Cargo.toml) | Protocol-neutral service observability composition: logging, stable identity, optional OTLP export, metric-provider semantics, and lifecycle counters. |
| [service-http](libs/service-http/Cargo.toml) | Standard HTTP service policy shell: probes, lifecycle adapters, OpenAPI/docs routes, request-context propagation, and shared errors. |
| [metrics-prometheus](libs/metrics-prometheus/Cargo.toml) | Lock-free Prometheus primitives and text encoder for service metrics. |
| [peer-tls](libs/peer-tls/Cargo.toml) | Peer mTLS material loading and rustls server/client config builders. |
| [surface](libs/surface/Cargo.toml) | Renderer-neutral UI element model shared by Jet WASM, native readers, renderers, and parity tools. |
| [ui-runtime](libs/ui-runtime/Cargo.toml) | Renderer-neutral component runtime: hooks, fiber storage, mount, flush, and update scheduling. |

## Install

Each binary ships a `curl | sh` installer that downloads the right prebuilt
binary from GitHub Releases and drops it on your `PATH` (default
`$HOME/.local/bin`). Self-update later with `<binary> upgrade`. Projects without
an installer yet are marked _coming soon_.

| Project | Binary | Install |
|---------|--------|---------|
| [arena](apps/arena/README.md) | `arena` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/arena/install.sh \| sh` |
| [cap](apps/cap/README.md) | `cap` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/cap/install.sh \| sh` |
| [courier](apps/courier/README.md) | `courier` | _coming soon_ |
| [guard](apps/guard/README.md) | `guard` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/guard/install.sh \| sh` |
| [jet](apps/jet/README.md) | `jet` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/jet/install.sh \| sh` |
| [beam](apps/beam/README.md) | `beam` | _coming soon_ |
| [cube](apps/cube/README.md) | `cube` | _coming soon_ |
| [defer](apps/defer/README.md) | `defer` | _coming soon_ |
| [keep](apps/keep/README.md) | `keep` | _coming soon_ |
| [loom](apps/loom/README.md) | `loom` | _coming soon_ |
| [lumen](apps/lumen/README.md) | `lumen` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/lumen/install.sh \| sh` |
| [mamba](apps/mamba/README.md) | `mamba` | _coming soon_ |
| [meter](apps/meter/README.md) | `meter` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/meter/install.sh \| sh` |
| [pgpool](apps/pgpool/README.md) | `pgpool` | _coming soon_ |
| [preview](apps/preview/README.md) | `preview` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/preview/install.sh \| sh` |
| [sift](apps/sift/README.md) | `sift` | _coming soon_ |
| [relay](apps/relay/README.md) | `relay` | _coming soon_ |
| [rig](apps/rig/README.md) | `rig` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/rig/install.sh \| sh` |
| [tape](apps/tape/README.md) | `tape` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/tape/install.sh \| sh` |
| [vat](apps/vat/README.md) | `vat` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/vat/install.sh \| sh` |

## Runtime Evidence Loop

The runtime tools are intentionally split by responsibility:

- `vat` prepares and runs the local environment.
- `rig` drives requests, queries, and workload traffic.
- `meter measure` observes a running executable or service from the outside and
  records cpu time, wall time, peak RSS, and optional stack samples under
  `.meter/`.
- `meter profile` folds embedded/source-aware profiling data, such as phase
  breakdowns emitted by code that uses meter APIs.
- `arena` compares collected benchmark results across targets.
- `guard` turns static and runtime security evidence into one posture report.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the repo-wide authoring contract:
how to shape files, paths, and names so the tree stays legible to agents and
tooling, plus the shared **service archetype** (a common service baseline with
StatefulSet and Deployment workload profiles, HTTP/2 + OpenAPI, k8s-native)
and the **CLI convention** every
binary follows (`llm` / `upgrade` / `issue`).

## License

MIT
