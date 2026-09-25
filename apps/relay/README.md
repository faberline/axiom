# relay

## Brief

`relay` is the online **single-cast pull work-queue broker** in the Axiom stack
(RabbitMQ/SQS-shaped): a producer publishes a task, a worker **pulls** (leases)
it, runs it, and acks — each message is delivered exactly once to one of the
competing consumers, then reclaimed (**delete-on-ack**). It owns work-queue
leasing with lease-expiry redelivery, dead-lettering, priority, short delayed
visibility, h2c/OpenAPI transport, and raft-backed HA. Payloads stay opaque JSON
so higher-level systems (loom, worker runtimes) use relay without relay learning
their domain model.

Relay is the pull side of Axiom's queue family; [`defer`](../defer) is the push
side for scheduled HTTP dispatch. Relay is deliberately **not** a Kafka/pub-sub
platform: durable topic replay/retention is [`tape`](../tape)'s job, and
push/ETA task dispatch is [`defer`](../defer)'s. Concurrency is
**client-driven** — more workers = more throughput; relay has no server-side
rate/concurrency governor (that is `defer`). Tenancy is a deployment concern —
relay is single-tenant per deployment; run one per tenant (k8s namespace). See
the boundary notes in the ecosystem docs.

Priority is shared with `defer` as an unsigned byte: `0` is lowest, `255` is
highest, the default is `10`, and higher values lease first. In relay, priority
orders only entries that are already visible in the work queue; long-horizon ETA,
rate limiting, and HTTP target dispatch remain Defer concerns.

Relay ships as **one binary**: bare `relay` serves (h2c broker; raft-runtime
auto-mode HA — see [`HA.md`](HA.md)), and the same bin carries the offline
`spec`/`spec gen [--target <profile>]`, `backup`, `k8s crd|operator|instance`, `dockerfile render`,
and shared `llm`/`upgrade`/`issue` verbs (WIs #1204-#1209, the
service-archetype convergence).

## Capabilities

Each capability states its promise and names the gate that verifies it. A
promise with no gate line under it is not claimed.

### Domain

#### Durable ordered log

Append messages in per-subject/shard order, deduplicate idempotent retries on
message id, and recover committed log state across restarts and segment
rotation. Delete-on-ack: segments are reclaimed once every entry in them is
acked, not by wall-clock age or size.

- Gate: `cargo test -p relay --test relay_core --test durable --test segments`

#### Work queue lifecycle

Competing-worker queue semantics with epoch-fenced leases, heartbeat extension,
idempotent ack, expired-lease reclaim, and redelivery liveness.

- Gate: `cargo test -p relay --test work_queue_api --test reconciler`

#### Raft HA and primary/replica topology

A raft-backed HA path on the shared `core/raft-runtime` driver: the leader owns
writes, followers replicate committed state, and failover preserves the
ordered-log and work-queue API contract. It converges in process, persists hard
state and the applied-index floor, serves through real h2c nodes, and is
dogfooded through a Kubernetes kind failover smoke. Auto-mode takes no flags —
downward-API env (`REPLICAS_PER_SHARD` > 1) flips replica mode.

- Gate: `cargo test -p relay --test raft_core --test raft_persistence --test raft_cluster`
- Dogfood: `apps/relay/scripts/kind-failover-smoke.sh`
- Source: `apps/relay/src/raft.rs`, [`HA.md`](HA.md)

### Interface

#### CLI

One runnable binary with a stable process entrypoint — serve by default, both
locally and as the Kubernetes raft node — plus offline spec and deploy verbs and
a served OpenAPI contract for non-Rust clients.

Alongside it, the mandatory shared `cli-std` surface every ecosystem CLI owes,
backed by build-stamp provenance and kept out of relay's domain verbs:

- `relay llm` — offline agent self-doc topics (outline, http-api, operations)
- `relay upgrade [--check]` — shared self-update through `cli-std`
- `relay issue search|view|create` — tracker surface scoped to `app:relay`

Gate: `cargo test -p relay --test worker_loop --test spec_cli --test deploy_cli --test raft_config --test raft_cluster`

#### HTTP/OpenAPI worker protocol

A polyglot h2c/OpenAPI contract so non-Rust workers can publish, consume, lease,
heartbeat, and ack. The served `/openapi.json` and the offline `relay spec` are
twins of the same contract; `apps/relay/docs/worker-protocol.md` is the endpoint
summary.

- Gate: `cargo test -p relay --test http2_transport --test worker_loop --test spec_cli`

#### Standard operational endpoints

The one-port operational surface the service trait requires — probes, metrics
scrape, live spec, and Swagger UI stay always-on and auth-exempt on the serve
port, via `service_http::standard_probe_routes`:

- `GET /healthz`, `GET /readyz` (503 once draining), `GET /metrics`,
  `GET /openapi.json`, `GET /docs`
- `relay spec` is the offline twin when no server is running

Every HTTP request is correlatable end to end: the shared `service-http` trace
layer honors a valid W3C `traceparent` when present and creates a local root
trace when absent, with `trace_id`/`span_id`/`parent_span_id`/`trace_flags`
flowing into every request span and structured log line. Every response carries
a `Server-Timing: app;dur=<ms>` baseline (the shared
`service-http::server_timing` contract, #2490).

- Gate: `cargo test -p relay --test http2_transport --test auth --test spec_cli`

#### Chainable output

Raw artifact streams stay unwrapped bytes — `relay spec`,
`relay k8s crd|operator|instance render`, `relay dockerfile render`. File-writing
and admin verbs (`--out`, `relay backup`, `relay issue …`,
`relay upgrade --check`) carry an explicit `next:` or a terminal marker.

- Gate: `cargo test -p relay --test deploy_cli --test spec_cli`

### Operations

#### Kubernetes-native deployment

An operator-rendered StatefulSet with downward-API env auto-mode reads, offline
CRD/operator/instance/dockerfile render verbs whose committed files are
byte-equal fixtures, and a kind failover path for deployment dogfood.

- Gate: `cargo test -p relay --test deploy_cli --test operator --test direct_k8s_assets --test raft_config`
- Dogfood: `apps/relay/scripts/kind-failover-smoke.sh`
- Assets: `apps/relay/k8s`

#### Long-running stability

Run as a long-lived broker without losing committed entries, leaking stuck
leases forever, or corrupting recovery state across restarts, segment rotation,
graceful drains, and leader failover — with a backup/restore path for the rest.
The cluster gate recovers the first stopped node from its durable engine/Raft
state before committing through a second leader loss.

Measured 2026-07-17, default 60-second run: 5,622 fixed-state operations, zero
errors, RSS 14,352 → 14,352 KiB, FD 14 → 14, threads 11 → 11, inspect p99
1 → 1 ms.

- Gate: `cargo test -p relay --test durable --test segments --test reconciler --test raft_persistence --test raft_cluster --test backup`
- Soak: `RELAY_SOAK_AUTOSTART=1 bash apps/relay/scripts/soak.sh`

#### Security hardening

Opaque payload boundaries, the shared bearer-token contract on the data plane
(probes exempt), fail-fast peer-TLS material validation, untrusted
peer-certificate rejection, last-known-good credential rotation, and explicit
negative gates for request limits and network policy.

On the wire that means queue-scoped bearer RBAC with audited live registry
rotation and bounded 429 admission; a dedicated reloadable mTLS listener for
peers; and read-only Secret projection, restricted pods, PDB, and ingress
NetworkPolicy in Kubernetes. The request-body cap (#2556) returns 413 with the
structured `payload_too_large` envelope, and a refused publish never reaches the
engine.

- Behavior: `bash apps/relay/scripts/ec-evidence.sh security-behavior`
- Boundaries: `bash apps/relay/scripts/ec-evidence.sh security-boundaries`
- Stability: `bash apps/relay/scripts/ec-evidence.sh security-stability`
- Static + runtime scan: `cd apps/relay && vat run guard-security`

#### Stateful service workload

The stateful production workload composes the shared storage, backup, raft,
peer-security, and Kubernetes mechanisms — `core/raft-runtime`,
`core/service-backup`, `core/service-auth`, `core/service-k8s` — while product
policy stays in the domain sections above. This is an integration map, not a
second copy of those contracts.

### Measurement

#### Competitive feature parity

Cover the baseline single-cast work-queue broker functions Relay needs to
compete with RabbitMQ, NATS JetStream, Redis Streams, and Dragonfly in Axiom
workloads. Redpanda/Kafka-class replay journals are `tape` competitors, not
relay competitors.

- Gate: `cargo test -p relay --test relay_core --test work_queue_api --test worker_loop --test raft_core --test raft_persistence --test raft_cluster`

#### Performance

Performance claims stay tied to a release-mode, fsync-always durable
measurement whose report is parsed independently and fails closed on missing or
zero observations — a fixed 2,000-message / 128-byte / batch-100 envelope.
RabbitMQ, NATS JetStream, Redis Streams, and Dragonfly comparisons are
**advisory dogfood**, not a required gate, until equivalent real-service
calibration is promoted. Claims remain workload-scoped.

- Behavior: `bash apps/relay/scripts/ec-evidence.sh performance-behavior`
- Efficiency: `bash apps/relay/scripts/ec-evidence.sh performance-efficiency`
- Throughput ratchet: `cd apps/relay && vat run meter-perf`
- Comparison harness: `cargo run -p relay --release --example bench_compare -- --backend <target>`
  (advisory ratio wrapper: `apps/arena/examples/relay-vs-rabbitmq-nats-redis.toml`)

Last real Relay/RabbitMQ/NATS bulk lifecycle calibration: 2026-07-17, recorded
in `apps/relay/docs/perf-gate.md`.
