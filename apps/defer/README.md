# Defer

## Brief

Defer is the Cloud Tasks-like delayed push-queue dispatch service in the Axiom
stack.

It owns scheduled execution, HTTP target dispatch, per-queue controls, rate
limits, leases/acks, retries, dead-letter queues, and dedupe keys. It is
intentionally separate from `relay`: Relay is the pull queue for worker-leased
delivery; Defer is the push queue for scheduled HTTP dispatch, where every item
has a target, schedule, attempt policy, and terminal outcome.

Task priority uses the same field shape as relay: an unsigned byte where `0` is
lowest, `255` is highest, and omitted priority defaults to `10`. Defer first
filters tasks by schedule/ETA and queue rate/concurrency limits; priority orders
only the due, dispatch-eligible candidates, with same-priority tasks remaining
FIFO by task creation order.

## Boundaries

- `relay` owns pull-queue broker delivery and opaque worker leases.
- `defer` owns scheduled task lifecycle, target dispatch, retry policy, DLQ, and
  rate/concurrency-limited push dispatch.
- `loom` may use Defer for timers/callbacks, but workflow state remains in Loom.
- `keep` may store large request/response bodies by reference.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Delayed Task Lifecycle | #766 | Raft-committed create/batch-create, ETA/priority, fenced lease, batch settle, retry, DLQ, cancel, inspect, snapshot and recovery |
| HTTP Dispatch And Retries | #766 | bounded concurrent real HTTP delivery with stable idempotency, optional HMAC signing, committed lease before side effect, committed settlement, and an executable ambiguous-outcome retry gate |
| Queue Rate Limits | #766 | committed per-queue pause/resume/disable, max-in-flight, dispatch budget, and token-bucket rate/burst state shared by all replicas |
| HTTP/2 API List | #766 | one HTTP/1.1+h2c service port, exact OpenAPI/docs/probes/metrics, queue/task/batch/dispatch/backup routes, and generated clients |
| Kubernetes-Native Deployment | #766 | layered base/overlays/components plus CRD/operator/instance, PDB/PVC/NetworkPolicy/observability, and disposable Kind lifecycle-recovery gate |
| Primary Replicas | #766 | every mutation is Raft committed; replicas apply identical scheduler bytes with committed executor/epoch fencing and durable restart recovery |
| Stateful Service Workload | #2170 | shared stateful-storage baseline composes Defer's Raft scheduler, snapshot/backup, authorization, and StatefulSet evidence without duplicating domain policy |
| CLI Interface | #766 | `defer` service/spec/client/queue/task/dispatch/backup/k8s/dockerfile plus standard `llm`/`upgrade`/`issue` verbs |
| Long-Running Stability | #766 | lease-expiry/two-cycle failover gates, PVC recovery, and 60-second fixed-state HTTP/Raft error/RSS/FD/thread/p99 plateau soak |
| Security Hardening | #766 | queue RBAC, audited live credential rotation, admission limits, signed targets, peer mTLS, restricted pods, NetworkPolicy, and secret projection |
| Competitor Feature Parity | #766 | explicit Google Cloud Tasks contract matrix and documented exclusions; no Celery/Sidekiq category mismatch |
| Competitor Performance | #766 | same-host Defer/Relay full durable lifecycle gate requires `Defer / Relay >= 0.8`; no unmeasured Cloud Tasks win is claimed |
| Defer App Source Root Taxonomy | #1217 | source-root routing uses `apps/defer` while preserving legacy TD identity |

### Defer App Source Root Taxonomy

Defer's app-facing source root resolves through `apps/defer` without changing
the GitHub label, AW project name, persistent branch convention, or TD bucket
identity.

- Root WI: #1217
- Surfaces: Repo taxonomy: root inventory, Cargo workspace routing, AW project
  config, project-local docs, scripts, tests, and generated evidence paths.
- Gate — behavior: verified repo taxonomy gate - defer resolves through
  apps/defer while tracker identity remains app:defer
- Gate: verified: capability structure and the open-work inventory, by two `aw`
  gates that were deleted with the binary
- Source:
  `verified: apps/defer/tech-design/logic/repo-taxonomy-move-defer-from-projects-defer-to-apps-defer.md`,
  `nothing re-runs them`,
  `verified: stale source-root scan permits only preserved TD-bucket references`
- Evidence: TD merged; source-root migration smoke checks passed

### CLI Interface

Defer ships an agent-drivable CLI for delayed task lifecycle, queue control,
and admin workflows while following the repository-wide CLI convention.

- Root WI: #766
- Surfaces: CLI: `defer llm`, `defer upgrade`, `defer issue`, queue/task
  create/status/cancel, and admin/debug verbs.
- Gate — behavior: `cargo test -p defer --test cli_contract` - required
  standard verbs, task lifecycle ergonomics, exact spec/render output, and
  offline agent docs
- Source: `apps/defer/tests/cli_contract.rs`
- Evidence: apps/defer/tests/cli_contract.rs

### Long-Running Stability

Defer remains stable under sustained scheduled-task load, retries, DLQ writes,
and restart cycles without losing committed tasks or duplicating terminal acks.
The 2026-07-17 default run crossed the 1,024-entry proposal-cache/snapshot
cadence before measuring 60 seconds: 1,043 fixed-state operations, zero errors,
RSS 41,584 -> 42,192 KiB (1%), FD 18 -> 18, threads 12 -> 12, and task-read p99
2 -> 1 ms.

- Root WI: #766
- Surfaces: Runtime: priority-ready set, committed queue-control/rate bucket,
  fenced lease store, retry scheduler, DLQ state, Raft snapshot/recovery, and
  bounded HTTP dispatcher.
- Gate — stability:
  `cargo test -p defer --test task_lifecycle --test rate_limits --test raft_scheduler`
- Gate — dogfood: `DEFER_SOAK_AUTOSTART=1 bash apps/defer/scripts/soak.sh`
- Gate: Kubernetes: `bash apps/defer/scripts/kind-e2e.sh`
- Source: `apps/defer/tests/task_lifecycle.rs`,
  `apps/defer/tests/rate_limits.rs`,
  `apps/defer/tests/raft_scheduler.rs (same-directory node restart, snapshot recovery, and repeated failover)`,
  `apps/defer/scripts/soak.sh`, `apps/defer/scripts/kind-e2e.sh`,
  `core/service-observability/scripts/soak-metrics.sh`
- Evidence: lifecycle/raft tests plus bounded soak and operator/PVC Kind
  recovery

### Security Hardening

Defer protects task control and target dispatch with explicit authorization,
signed delivery, auditability, network policy, and managed secret rotation.

- Root WI: #766
- Surfaces: HTTP/K8s: queue/task authn/authz, signed target dispatch, tenant
  isolation, network policy, audit events, secret rotation, and request limits.
- Gate — security: shared audited queue RBAC and credential rotation, HMAC
  target signing, real peer mTLS, bounded admission, restricted K8s security
  contexts, read-only secret projection, and NetworkPolicy
- Source: `apps/defer/tests/http_api.rs`, `apps/defer/tests/service_auth.rs`,
  `apps/defer/tests/http_dispatch.rs`, `apps/defer/tests/service_admission.rs`,
  `apps/defer/tests/raft_peer_mtls.rs`, `apps/defer/tests/direct_k8s_assets.rs`
- Evidence: auth rotation/audit, target signing, admission, peer mTLS, and K8s
  negative/static gates

### Competitor Feature Parity

Defer keeps an explicit delayed-task feature matrix against established task
dispatch systems, with comparison scope changed only when product requirements
change.

- Root WI: #766
- Surfaces: Docs/Test: delayed HTTP push-task feature matrix against Google
  Cloud Tasks; Celery and Sidekiq are excluded because they are worker
  frameworks, not managed HTTP push queues.
- Gate — behavior: schedule, target delivery, terminal success, retry/DLQ,
  rate/burst/concurrency, cancel/inspect, dedupe/idempotency, and target
  authentication
- Source: `apps/defer/benchmarks/competitor-feature-matrix.md`
- Evidence: apps/defer/benchmarks/competitor-feature-matrix.md

### Competitor Performance

Defer bounds the cost of its additional ETA, permit, retry, DLQ, and terminal
state machinery against the sibling Relay work queue. Cloud Tasks remains the
external feature-contract peer; no Cloud performance claim is made without a
real queue, public target, declared region, and equivalent network conditions.

- Root WI: #766
- Surfaces: Test: identical single-voter fsync-always durable enqueue ->
  committed lease -> committed ack workload in Defer and Relay, with
  throughput, p50/p95/p99, CPU, process RSS, disk bytes, amplification, and
  error counts.
- Gate — efficiency:
  `cargo test --release -p defer --test relay_performance_ceiling -- --ignored --nocapture`
  requires Defer throughput to remain at least 80% of Relay under the declared
  workload
- Source: `apps/defer/tests/relay_performance_ceiling.rs`,
  `apps/defer/benchmarks/competitor-feature-matrix.md`,
  `apps/defer/benchmarks/relay-performance-ceiling.md`
- Evidence: release-mode Defer/Relay ceiling gate (`minimum_ratio = 0.8`);
  apps/defer/benchmarks/relay-performance-ceiling.md

### Delayed Task Lifecycle

Defer manages delayed task state from creation through terminal success,
failure, cancellation, or dead-letter handoff. Task priority is `u8`
(`0..=255`, default `10`); it is applied only after the task is due and the
queue can dispatch more work.

- Root WI: #766
- Surfaces: Rust/Raft: `DeferScheduler` and `DeferRaft`; HTTP: queue/task
  single and batch routes, status, cancel, and dispatch.
- Gate — behavior: lifecycle/rate-limit/Raft scheduler tests cover ordering,
  batch atomicity, cancellation, fenced leases, settlement, failover, and
  terminal states
- Source: `apps/defer/tests/task_lifecycle.rs`,
  `apps/defer/tests/rate_limits.rs`, `apps/defer/tests/raft_scheduler.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| delayed-task-state-machine | epic | #766 | apps/defer/tests/task_lifecycle.rs |
| due-task-priority-ordering | epic | #766 | apps/defer/tests/task_lifecycle.rs |

### HTTP Dispatch And Retries

Defer dispatches tasks to HTTP targets with bounded retries, dedupe keys, and
explicit dead-letter behavior. If a target accepts the effect while the worker
loses its committed fence before settlement, Defer reports `LostOwnership` and
retries with the same idempotency key; only the fenced retry may commit the
terminal outcome.

- Root WI: #766
- Surfaces: Rust API: `HttpDispatcher::dispatch_batch`; Raft: committed
  `LeaseDue` and `SettleBatch`; HTTP: real target
  method/headers/body/idempotency/signature and manual dispatch route.
- Gate — behavior:
  `cargo test -p defer --test http_dispatch --test http_api --test rate_limits`
  - real delivery, signing, retry/DLQ, batch settlement, and lost-ownership
  fencing
- Source: `apps/defer/tests/task_lifecycle.rs`,
  `apps/defer/tests/http_dispatch.rs`, `apps/defer/tests/http_api.rs`
- Evidence: real HTTP delivery, signing, stable idempotency, retry/DLQ,
  committed batch settlement, and accepted-HTTP/lost-fence retry proof

### Queue Rate Limits

Defer enforces Cloud Tasks-style per-queue controls, rate limits, and
concurrency limits before dispatching tasks to external targets.

- Root WI: #766
- Surfaces: Rust/Raft/HTTP: `QueuePolicy`, committed queue
  configuration/control, max dispatch per tick/second, burst, max-in-flight,
  snapshot, and priority among eligible tasks.
- Gate — behavior:
  `cargo test -p defer --test rate_limits --test task_lifecycle` - per-queue
  dispatch budget/concurrency/rate enforcement, pause/resume/disable controls,
  policy update isolation, and priority-preserving dispatch among eligible
  tasks
- Source: `apps/defer/tests/rate_limits.rs`,
  `apps/defer/tests/task_lifecycle.rs`
- Evidence: apps/defer/tests/rate_limits.rs; apps/defer/tests/task_lifecycle.rs

### HTTP/2 API List

Defer exposes a compact h2c/OpenAPI API list for queue/task lifecycle and
operator workflows. Every HTTP request is correlatable end to end: W3C
`traceparent` is honored when present and a local root trace is created when
absent, with the ids flowing into every request span and structured log line.
Server-Timing per-response latency attribution (the shared
`service-http::server_timing` contract) is wired into defer's HTTP stack: every
response carries a `Server-Timing: app;dur=<ms>` baseline (#2490).

- Root WI: #766
- Surfaces: HTTP: `/healthz`, `/readyz`, `/metrics`, `/openapi.json`, `/docs`,
  queue/task/admin routes.; Logs: structured stdout with per-request trace
  correlation — the shared `service-http` trace layer
  (`service_http::trace_layer()`) accepts a valid W3C version-00 `traceparent`
  (invalid input is treated as absent) and generates a fresh local root context
  otherwise, so every request span and log line carries
  `trace_id`/`span_id`/`parent_span_id`/`trace_flags`.; HTTP: Server-Timing
  response attribution — shared `service-http::server_timing` contract
  (`Server-Timing: app;dur=` per-response latency) on every response.
- Gate — behavior: `cargo test -p defer --test http_api --test cli_contract` -
  live HTTP/1.1+h2c probes/metrics/OpenAPI, domain routes, offline spec twin,
  and client generation
- Source: `apps/defer/tests/http_api.rs`, `apps/defer/tests/cli_contract.rs`
- Evidence: apps/defer/tests/http_api.rs; apps/defer/tests/cli_contract.rs

### Kubernetes-Native Deployment

Defer runs as a dedicated k8s-native task dispatch service with
operator-managed queues, rate limits, storage, backup policy, and lifecycle.

- Root WI: #766
- Surfaces: K8s: dedicated StatefulSet/operator topology for queues, timers,
  storage, probes, backups, and PDBs.
- Gate — behavior: direct Kustomize and CRD/operator/instance render tests
- Gate — stability: disposable Kind operator/PVC queue-task lifecycle recovery
- Source: `apps/defer/k8s`, `apps/defer/tests/direct_k8s_assets.rs`,
  `apps/defer/tests/operator.rs`, `apps/defer/scripts/kind-e2e.sh`
- Evidence: layered assets, operator conformance, and Kind PVC lifecycle
  recovery

### Primary Replicas

Defer replicates task state and timer ownership through raft so failover
preserves scheduled tasks and terminal outcomes. The integration gate restarts
the first failed primary from durable state, waits for task-state convergence,
then removes the current leader and completes another full create/lease/ack
lifecycle through the surviving quorum.

- Root WI: #766
- Surfaces: Raft: delayed task state machine over `core/raft-core` and
  `core/raft-runtime`.
- Gate — stability:
  `cargo test -p defer --test raft_scheduler --test raft_peer_mtls` - committed
  task recovery, same-directory recovered-node catch-up followed by a second
  leader loss, fencing, snapshots, and authenticated peers
- Source: `apps/defer/tests/raft_scheduler.rs`,
  `apps/defer/tests/raft_peer_mtls.rs`
- Evidence: three-voter failover, durable replay/snapshot recovery, and real
  peer mTLS

### Stateful Service Workload

Defer composes its stateful production workload from shared Raft, backup,
authorization, and Kubernetes mechanisms while keeping delayed-task lifecycle,
dispatch, rate-limit, failover, and recovery policy in the existing domain
roots. This root is an integration map, not a second implementation of those
contracts.

- Root WI: #2170
- Surfaces: Shared mechanisms: `core/raft-runtime`, `core/service-backup`,
  `core/service-auth`, and `core/service-k8s`; Defer integration: Raft-backed
  scheduler state, admin snapshot backup, queue-scoped authorization, and
  operator-managed StatefulSet storage.
- Gate — behavior: the `stateful_storage` profile resolved its common workload
  baseline under the `aw` capability gate, which was deleted with the binary
- Gate: stability and security remain owned by the linked Defer capability
  roots and their executable gates
- Gate: the `aw` capability gate, deleted with the binary
- Source: `the rows below are what still runs`,
  `apps/defer/tests/direct_k8s_assets.rs`,
  `apps/defer/tests/raft_scheduler.rs`, `apps/defer/tests/service_auth.rs`,
  `apps/defer/src/raft.rs`, `apps/defer/src/bin/defer.rs`,
  `apps/defer/src/operator`
- Evidence: Composes Primary Replicas, Kubernetes-Native Deployment, Security
  Hardening, Backup & Restore, and Long-Running Stability from shared libraries
  without duplicating Defer domain behavior.
