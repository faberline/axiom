# Loom

## Brief

Loom is the workflow scheduler in the Axiom service stack.

It owns workflow run state, dynamic DAG orchestration, runner selection,
timers, fair dispatch, and client control APIs. It coordinates through small
messages and references only: payload/result bytes live in `keep`, online task
delivery happens through `relay`, historical replay belongs to `tape`, and
delayed external callbacks can use `defer`.

## Boundaries

Loom is a control plane, never a data path.

```
client -> loom submit/status/result-ref + keep payload/result bytes
loom   -> relay publish task / observe ack + keep result refs
worker -> relay lease/ack/heartbeat + keep input/result bytes
```

- `loom` owns workflow state and orchestration decisions.
- `relay` owns online broker delivery and worker leasing.
- `keep` owns payload/result bytes and claim-check refs.
- `tape` owns replay/audit history when workflow events must be replayed later.
- `defer` owns delayed HTTP task dispatch; Loom may use it for timers/callbacks.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Workflow Orchestration | #116 | dynamic DAG ready-node scheduling and fan-in/fan-out |
| Workflow Data Model | #112 | WorkflowRun, Node, Stage, attempts, refs, and history records |
| State Durability | #110 | sharded raft-backed workflow state and crash recovery |
| Runner And Execution Selection | #164 | resident, k8s-job, and local runner metadata and dispatch |
| Client Control API | #165 | h2c/OpenAPI submit/status/result-ref surface |
| Worker Harness | #164 | reference worker harness over relay + keep |
| Fair Dispatch | #107 | weighted fairness, quota, and bounded materialization |
| Competitive Perf Gate | #127 | scheduler throughput and dormant-axis benchmark |
| CLI Interface | #165 | `loom` CLI for submit/status/worker/admin and agent docs |
| HTTP/2 API List | #165 | h2c/OpenAPI route inventory and contract tests |
| Kubernetes-Native Deployment | #165 | dedicated StatefulSet/operator topology |
| Long-Running Stability | #110 | soak, crash recovery, and bounded resource gates |
| Primary Replicas | #110 | raft-backed primary/replica topology |
| Security Hardening | #165 | authn/authz, tenant isolation, audit events, and secret rotation |
| Stateful Service Workload | #110 | projects the shared stateful-service workload baseline |

### CLI Interface

Loom ships an agent-drivable CLI that follows the repository CLI convention and
can operate the workflow control plane without bespoke scripts.

- Root WI: #165
- Surfaces: CLI: `loom llm`, `loom upgrade`, `loom issue`, `loom controller`,
  `loom worker`, and admin/debug verbs.
- Gate — behavior: pending CLI convention gate - required standard verbs,
  workflow submit/status ergonomics, worker harness wiring, and offline agent
  docs
- Source: `pending: apps/loom/tests/cli_contract.rs`
- Evidence: #541 — pending CLI convention gate

### HTTP/2 API List

Loom exposes a compact h2c/OpenAPI route list for workflow submission, run
control, status, result references, probes, metrics, and generated docs.

- Root WI: #165
- Surfaces: HTTP: `/healthz`, `/readyz`, `/metrics`, `/openapi.json`, `/docs`,
  workflow submit/status/control/result-ref routes.
- Gate — behavior: pending h2c/OpenAPI route-list gate - probes, metrics,
  OpenAPI, route inventory, validation, and status codes
- Source: `pending: apps/loom/tests/http_api.rs`
- Evidence: pending h2c/OpenAPI route-list gate

### Kubernetes-Native Deployment

Loom runs as a dedicated k8s-native workflow scheduler with operator-managed
state, probes, backup policy, rolling upgrades, and stable network identity.

- Root WI: #165
- Surfaces: K8s: dedicated StatefulSet/operator topology for workflow state,
  probes, metrics, backup/restore, PDBs, and runner integration.
- Gate — behavior: pending kustomize/operator render gate - CRD, operator,
  instance render, and kind dogfood
- Source: `pending: apps/loom/k8s`
- Evidence: pending k8s render/dogfood gates

### Long-Running Stability

Loom remains stable under long-running workflow load, restart cycles, and
backpressure without leaking resources or losing committed workflow
transitions.

- Root WI: #110
- Surfaces: Runtime: scheduler event loop, raft state, relay/keep client pools,
  compaction, snapshot, and recovery paths.
- Gate — stability: pending long-running gate - soak, restart, crash recovery,
  bounded memory, and backpressure behavior
- Source: `pending: apps/loom/tests/long_running_stability.rs`
- Evidence: pending long-running stability gate

### Primary Replicas

Loom replicates workflow state through a primary/replica topology so committed
workflow transitions survive leader failure and replica recovery.

- Root WI: #110
- Surfaces: Raft: workflow state primary/replica topology over `core/raft-core`
  and `core/raft-runtime`.
- Gate — stability: pending raft primary/replica gate - leader failover,
  replica catch-up, snapshot restore, and committed-transition safety
- Source: `pending: apps/loom/tests/primary_replicas.rs`
- Evidence: pending primary/replica failover gate

### Security Hardening

Loom protects workflow control APIs and worker integration with explicit tenant
authorization, auditability, network policy, and managed secret rotation.

- Root WI: #165
- Surfaces: HTTP/K8s: authn/authz, tenant/workflow authorization, network
  policy, audit events, secret rotation, and request limits.
- Gate — behavior: pending security gate - auth failure cases, tenant
  isolation, audit emission, secret rotation, and abuse limits
- Source: `pending: apps/loom/tests/security_hardening.rs`
- Evidence: pending security hardening gate

### Workflow Orchestration

Loom schedules dynamic workflow DAGs by selecting ready nodes, publishing work
through Relay, observing completion, and advancing workflow state
deterministically.

- Root WI: #116
- Surfaces: HTTP: workflow submit/status/control routes; Scheduler: ready-node
  frontier, dynamic fan-out/fan-in, and fan-in barriers.
- Gate — behavior: pending workflow orchestration conformance gate - DAG
  transitions, retry, fan-in, fan-out, and completion state
- Source: `pending: apps/loom/tests/workflow_orchestration.rs`
- Evidence: pending workflow orchestration gate

### Workflow Data Model

Loom defines the durable workflow model used by clients, controllers, runners,
and status/history views.

- Root WI: #112
- Surfaces: Rust/API schema: WorkflowRun, Node, Stage, Attempt, ResultRef,
  InputRef, and workflow history records.
- Gate — behavior: pending data-model conformance gate - serialization,
  versioning, validation, and compatibility
- Source: `pending: apps/loom/tests/workflow_data_model.rs`
- Evidence: pending data-model conformance gate

### State Durability

Loom persists workflow state before acknowledgement and survives process or
leader failure without losing committed workflow transitions.

- Root WI: #110
- Surfaces: Raft: sharded workflow state machine over `core/raft-core` and
  `core/raft-runtime`; Snapshot: service-owned workflow state snapshots.
- Gate — stability: pending raft durability gate - crash recovery, snapshot
  restore, and failover without committed workflow loss
- Source: `pending: apps/loom/tests/raft_workflow_state.rs`
- Evidence: pending raft durability/failover gate

### Runner And Execution Selection

Loom selects the runner class for each task, while Relay routes delivery and
workers own actual execution.

- Root WI: #164
- Surfaces: Scheduler: runner metadata and dispatch policy for resident,
  k8s-job, and local runners.
- Gate — behavior: pending runner-selection gate - runner routing, fallback,
  retry, and status attribution
- Source: `pending: apps/loom/tests/runner_selection.rs`
- Evidence: pending runner-selection gate

### Client Control API

Loom exposes a thin h2c/OpenAPI control API for submitting workflows, checking
status, and obtaining result refs without moving payload bytes.

- Root WI: #165
- Surfaces: HTTP: `POST /runs`, `GET /runs/{id}`,
  `POST /runs/{id}/nodes/{node}/complete`, `/healthz`, `/readyz`, `/metrics`,
  `/openapi.json`, `/docs`.
- Gate — behavior: pending h2c/OpenAPI API gate - submit, status, result-ref,
  validation, probes, metrics, and OpenAPI
- Source: `pending: apps/loom/tests/client_control_api.rs`
- Evidence: pending client-control API gate

### Worker Harness

Loom ships a reference worker harness while preserving the polyglot boundary:
workers talk to Relay and Keep, not to Loom.

- Root WI: #164
- Surfaces: CLI: `loom worker` reference harness; Worker protocol: relay
  lease/heartbeat/ack plus keep input/result refs.
- Gate — behavior: pending worker-harness gate - lease, keep IO, ack exactly
  once, heartbeat, and retry boundaries
- Source: `pending: apps/loom/tests/worker_harness.rs`
- Evidence: pending worker-harness gate

### Fair Dispatch

Loom schedules ready work fairly across tenants, workflows, and runner classes
without materializing unbounded dormant branches.

- Root WI: #107
- Surfaces: Scheduler: weighted fair share, quota, namespace/workflow priority,
  and lazy bounded materialization.
- Gate — behavior: pending fairness conformance gate - quota, priority,
  starvation prevention, and bounded materialization
- Source: `pending: apps/loom/tests/fair_dispatch.rs`
- Evidence: pending fairness conformance gate

### Competitive Perf Gate

Loom keeps workflow scheduling performance tied to repeatable meter/vat gates
instead of anecdotal benchmark runs.

- Root WI: #127
- Surfaces: Meter/Vat: scheduler throughput and dormant-axis benchmark;
  Benchmark: Temporal/Celery-style comparison harnesses.
- Gate — efficiency: pending loom scheduler meter gate - frontier throughput,
  memory, dormant branches, and dispatch latency
- Source: `pending: apps/loom/vat.toml`, `pending: apps/loom/benchmark`
- Evidence: pending meter/vat benchmark gate

### Stateful Service Workload

Loom projects the shared stateful-service workload baseline without a duplicate
service implementation. Its durable run store, stable StatefulSet identity,
raft primary/replica topology, and snapshot/backup path are verified by the
linked capability roots.

- Root WI: #110
- Surfaces: Raft: sharded workflow state primary/replica topology over
  `core/raft-core` and `core/raft-runtime`. K8s: dedicated StatefulSet/operator
  topology for workflow state under `apps/loom/k8s/`.
- Gate — behavior: the `stateful_storage` profile had its shared baseline
  resolved by the `aw` capability gate, which was deleted with the binary
- Gate — stability: raft failover, replica catch-up, snapshot restore, and
  committed-transition safety
- Source: `apps/loom/tests/integration/`
- Evidence: the `aw` capability gate, deleted with the binary; composes
  Workflow Orchestration, State Durability, Primary Replicas, and
  Kubernetes-Native Deployment without duplicating their claims
