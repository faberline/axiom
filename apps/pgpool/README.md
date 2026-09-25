# Pgpool

## Brief

`pgpool` is the working app id for Axiom's Kubernetes-native PostgreSQL
connection pooler. The final product name is not settled yet; until then the
repository path, crate name, binary name, and tracker label use `pgpool` as a
stable implementation placeholder.

The app owns the database-pooling data plane: frontend PostgreSQL TCP
admission, backend connection budgeting, drain-aware shutdown, pool health, and
standard operational HTTP endpoints. Cloud SQL, AlloyDB, and other platform
connectors stay explicit adapters above this core rather than being baked into
the pooler runtime.

Current implementation slice: `apps/pgpool` is a Rust workspace crate and
binary with PostgreSQL wire handling, bounded session/transaction pooling, a
single-owner dense-buffer readiness reactor for the transaction data plane, a
served admin plane, live remote-PostgreSQL capacity discovery, global endpoint
quota/drain models, and layered Pgpool CRD/operator/instance artifacts. Shared
runtime dependencies remain wired through `server-lifecycle`, `server-tcp`,
`server-http`, `metrics-prometheus`, and `service-k8s`; provider authentication
and broader external EC gates remain separate work roots.

## Boundaries

- `pgpool` owns Postgres-compatible pooling and proxy admission.
- `server-lifecycle`, `server-tcp`, and `server-http` own generic server runtime
  mechanics; `pgpool` composes them instead of duplicating accept loops,
  connection budgets, h2c serving, drain, or tracing.
- Platform adapters such as Cloud SQL Proxy or AlloyDB endpoint discovery stay
  optional integration layers, not required runtime dependencies.
- Application services should connect to `pgpool` over the PostgreSQL wire
  protocol and inspect operations through the admin HTTP surface.

## Capabilities

A promise with no gate under it is not claimed.

The baseline capabilities selected by aw.toml's `service` umbrella profile
(plus `cli_facing`, `competitive_replacement`, `kubernetes_native`,
`long_running`, and `network_exposed`) are mandatory for this pooler class.
They do not replace pgpool's product capabilities; the PostgreSQL pooler core
and the platform adapter boundary remain first-class domain roots.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Working-Name App Scaffold | - | crate/bin/README/AW metadata and route inventory are present under `apps/pgpool` |
| Shared Server Substrate Adoption | - | runtime plan composes `server-lifecycle`, `server-tcp`, and `server-http` types |
| PostgreSQL Pooler Core | 1282 | domain: frontend pg wire parser, backend pool, transaction/session modes |
| Platform Adapter Boundary | 1283 | live PostgreSQL capacity discovery is provider/role typed; provider auth remains outside core runtime |
| CLI Interface | 1282 | mandatory baseline: single `pgpool` bin with runtime-plan/spec verbs; serve entrypoint remains open |
| CLI Standard Surface | - | mandatory baseline: shared `cli-std` llm/upgrade/issue surface with build-stamp provenance |
| Chainable Output Conformance | - | mandatory baseline: `runtime-plan` emits `next:`; raw spec streams stay unwrapped |
| Competitor Feature Parity | 1285 | mandatory baseline: PgBouncer/Odyssey/pgcat transaction-pooling replacement breadth |
| Competitor Performance | 1285 | fixed local ABBA harness has six eligible pgpool wins vs PgBouncer (#1753); vat-isolated ratchet remains open |
| EC Gates Configured | 1285 | mandatory baseline: aw.toml EC inventory, vat meter/guard runners, external-contracts evidence |
| HTTP/2 API List | 1282 | mandatory baseline: offline `pgpool spec` admin route inventory; served contract remains open |
| Kubernetes-Native Deployment | 1284 | PgpoolSpec CRD/operator/instance render, shared Deployment composition, quota admission, and drain behavior are covered; image artifact work remains |
| Long-Running Stability | 1282 | mandatory baseline: backend reuse without leaks, graceful drain, restart safety |
| Security Hardening | 1286 | mandatory baseline: frontend auth passthrough, TLS posture, admin-plane exposure gates |
| Standard Operational Endpoints | 1282 | mandatory baseline: one-port `/healthz`, `/readyz`, `/metrics`, `/openapi.json`, `/docs`; offline twin exists |

### Working-Name App Scaffold

Hold `pgpool` as a stable working app id — crate, binary, README capability
map, and AW metadata live under `apps/pgpool` — so pooler work roots can land
before the final product name is settled, without renaming churn.

- Root WI: none; this capability predates the tracker.
- Surfaces: CLI: `pgpool runtime-plan` - offline shared-runtime plan for the
  data and admin planes.; Config: `apps/pgpool/aw.toml` - project registration,
  capability profile traits, and workspace test gate.
- Gate — behavior: `cargo test -p pgpool --test cli_contract` - compiled-binary
  contract for the scaffold surface
- Source: `apps/pgpool/tests/cli_contract.rs`, `apps/pgpool/aw.toml`,
  `apps/pgpool/src/bin/pgpool.rs`
- Evidence: apps/pgpool/tests/cli_contract.rs

### Shared Server Substrate Adoption

`pgpool` starts from the shared service substrate instead of inventing a local
accept loop or HTTP admin server. The TCP data-plane listener uses `server-tcp`
concepts, the admin listener uses `server-http`/h2c concepts, and connection
limits/readiness/drain are represented by `server-lifecycle`.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `RuntimePlan` - composes `server-lifecycle`
  bind/budget/drain, `server-tcp` socket options, and `server-http` h2c
  options.; CLI: `pgpool runtime-plan` - JSON plan naming the shared libs.
- Gate — behavior: `cargo test -p pgpool` - runtime plan composes shared
  substrate types instead of local reinventions
- Source: `apps/pgpool/src/lib.rs`, `apps/pgpool/tests/cli_contract.rs`
- Evidence: apps/pgpool/src/lib.rs

### PostgreSQL Pooler Core

Provide a high-throughput PostgreSQL pooler with bounded frontend admission,
backend connection reuse, transaction/session pool modes, graceful drain, and
clear observability before platform-specific adapters are added.

- Root WI: #1282
- Surfaces: TCP: `0.0.0.0:6432` - PostgreSQL wire protocol frontend admission
  for application clients.; Rust API: `RuntimePlan` - pool mode, frontend
  budget, and backend budget configuration.
- Gate — behavior: pending pg wire parser and pool lifecycle conformance gates
  - startup/auth passthrough, transaction/session pooling, drain
- Gate: apps/pgpool/tests/wire_codec.rs
  (`cargo test -p pgpool --test wire_codec`)
- Gate: apps/pgpool/tests/session_proxy.rs
  (`cargo test -p pgpool --test proxy --test session_proxy`)
- Gate: apps/pgpool/tests/pool_modes.rs
  (`cargo test -p pgpool --test pool --test pool_modes`)
- Source: `apps/pgpool/tests/proxy.rs`, `apps/pgpool/tests/pool.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| pg-wire-frontend-protocol | epic | 1287 | apps/pgpool/tests/wire_codec.rs; apps/pgpool/tech-design/logic/pg-wire-message-codec-frontend-backend-frames.md |
| backend-pool-and-reuse | epic | 1289 | apps/pgpool/tests/pool.rs; apps/pgpool/tests/pool_modes.rs; apps/pgpool/tech-design/logic/backend-pool-connection-reuse-and-transaction-session-pool-modes.md |
| transaction-session-pool-modes | epic | 1289 | apps/pgpool/tests/pool.rs; apps/pgpool/tests/pool_modes.rs; apps/pgpool/tech-design/logic/backend-pool-connection-reuse-and-transaction-session-pool-modes.md |
| transaction-readiness-reactor | change | 1753 | apps/pgpool/tech-design/logic/p0-dense-buffer-readiness-reactor.md; apps/pgpool/tests/pool_modes.rs; apps/pgpool/benchmarks/pgbouncer-transaction-pooling/run.sh |
| serve-entrypoint-and-drain | epic | 1288 | apps/pgpool/tests/proxy.rs; apps/pgpool/tests/session_proxy.rs; apps/pgpool/tech-design/logic/session-mode-proxy-with-auth-passthrough-and-serve-entrypoint.md |

### Platform Adapter Boundary

Keep Cloud SQL Proxy, AlloyDB endpoint discovery, and other platform connectors
as explicit adapters above the pooler core: the core runtime never embeds
platform SDKs, and adapters only supply backend endpoints and auth material
through a stable seam.

- Root WI: #1283
- Surfaces: Rust API: backend endpoint/auth adapter seam - Cloud SQL, AlloyDB,
  and plain-Postgres backends supply endpoints and auth material above the core
  runtime.
- Gate — behavior: pending adapter seam conformance gates - core runtime stays
  adapter-free
- Gate: adapters compose from outside
- Source:
  `apps/pgpool/tests/connection_discovery.rs - live PostgreSQL runtime discovery integration gate`,
  `apps/pgpool/src/platform/discovery.rs - provider/role typed adapter seam and runtime-lower-bound logic`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| backend-adapter-seam | epic | 1283 | pending: adapter seam contract tests |
| runtime-connection-limit-discovery | change | 1570 | apps/pgpool/tests/connection_discovery.rs; apps/pgpool/src/platform/discovery.rs; apps/pgpool/tech-design/semantic/pgpool-runtime-connection-limit-discovery.md |

### CLI Interface

Expose pgpool as one runnable binary with a stable process entrypoint — serve
by default once the pooler core lands — plus offline runtime-plan and spec
verbs for agents and operators.

- Root WI: #1282
- Surfaces: CLI: `pgpool` - single bin; `runtime-plan` and `spec` verbs today,
  serve-by-default data/admin plane entrypoint planned.
- Gate — behavior: `cargo test -p pgpool --test cli_contract` - compiled-binary
  help/verb contract
- Source: `apps/pgpool/tests/cli_contract.rs`, `apps/pgpool/src/bin/pgpool.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| offline-plan-and-spec-verbs | change | - | apps/pgpool/tests/cli_contract.rs |
| serve-by-default-entrypoint | epic | 1288 | apps/pgpool/tests/cli_contract.rs (`help_and_llm_workflow_topic_mention_serve`); apps/pgpool/src/bin/pgpool.rs |

### CLI Standard Surface

Ship the mandatory shared `cli-std` surface (llm/upgrade/issue) every ecosystem
CLI owes, backed by build-stamp provenance, without blurring it into pgpool's
domain verbs.

- Root WI: none; this capability predates the tracker.
- Surfaces: CLI: `pgpool llm` - offline agent self-doc topics (outline,
  workflow, api, boundaries).; CLI: `pgpool upgrade` - shared self-update and
  `--check` surface through `cli-std`.; CLI:
  `pgpool issue search|view|create|comment` - shared tracker surface scoped to
  `project:pgpool`.
- Gate — behavior: `cargo test -p pgpool --test cli_contract` -
  llm/upgrade/issue appear in the compiled binary help contract
- Source: `apps/pgpool/src/bin/pgpool.rs`, `apps/pgpool/tests/cli_contract.rs`,
  `core/cli-std/src`
- Evidence: apps/pgpool/tests/cli_contract.rs

### Chainable Output Conformance

Keep pgpool's CLI outputs chainable per the CLI convention: raw artifact
streams (spec renders) stay unwrapped bytes, while operational verbs carry
explicit `next:`/terminal markers.

- Root WI: none; this capability predates the tracker.
- Surfaces: CLI: `pgpool spec --format openapi|openapi-yaml|json-schema|routes`
  - raw artifact streams that intentionally stay unwrapped bytes.; CLI:
  `pgpool runtime-plan` - operational output carrying a runnable `next:` step.
- Gate — behavior: `cargo test -p pgpool --test cli_contract` - runtime-plan
  emits `next: pgpool spec --format routes`
- Gate: spec stdout stays raw parseable bytes
- Source: `apps/pgpool/tests/cli_contract.rs`, `apps/pgpool/src/bin/pgpool.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| next-marker-on-runtime-plan | change | - | apps/pgpool/tests/cli_contract.rs |
| raw-spec-streams-stay-unwrapped | change | - | apps/pgpool/tests/cli_contract.rs |

### Competitor Feature Parity

Cover the baseline connection-pooler functions pgpool needs to replace
PgBouncer, Odyssey, and pgcat for Axiom workloads: transaction and session
pooling, bounded admission, drain, and pool observability.

- Root WI: #1285
- Surfaces: TCP: PostgreSQL wire frontend - transaction/session pooling
  workflows PgBouncer-class poolers cover.; HTTP: admin pool/stats/drain routes
  - operational parity with pooler admin consoles.
- Gate — behavior: pending pooler parity conformance gates - transaction
  pooling, session pooling, drain, and stats parity vs PgBouncer/Odyssey/pgcat
- Source: `pending: parity conformance matrix vs PgBouncer/Odyssey/pgcat`
- Evidence: pending: parity conformance gates

### Competitor Performance

Tie pgpool's performance claims to repeatable pooled-connection throughput and
latency tests under a vat-isolated meter gate, with the external PgBouncer /
Odyssey / pgcat comparison as advisory dogfood until promoted.

- Root WI: #1285
- Surfaces: Harness:
  `apps/pgpool/benchmarks/pgbouncer-transaction-pooling/run.sh` - fixed
  counterbalanced PgBouncer transaction-pooling comparison.; Meter/Vat: meter
  diagnostics are executable while `apps/pgpool/vat.toml#meter-perf` remains
  pending for an isolated ratchet.
- Gate — efficiency: fixed 64-client, 16-backend, simple-protocol release ABBA
  comparison with complete-client/error validation
- Gate: pending vat promotion to an enforced ratchet
- Source:
  `apps/pgpool/tests/pgbouncer_benchmark.rs - hermetic profile/verdict contract`,
  `apps/pgpool/benchmarks/pgbouncer-transaction-pooling/run.sh - six eligible release wins recorded on #1753, including the default transaction engine`,
  `pending: apps/pgpool/vat.toml meter-perf promotion to an enforced isolated ratchet`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| vat-meter-throughput-gate | epic | 1285 | pending: vat meter-perf runner |
| external-pooler-comparison | change | 1753 | fixed local ABBA runner; six eligible pgpool wins vs PgBouncer with both orders unanimous |

### EC Gates Configured

Keep pgpool's service-trait EC baseline explicit and runnable: aw.toml owns the
EC inventory, vat owns the meter/guard runners, and external-contracts/ carries
the evidence contracts each gate closes against.

- Root WI: #1285
- Surfaces: Config: `apps/pgpool/aw.toml` - AW EC inventory and generated
  dispatch commands (pending).; Config: pending `apps/pgpool/vat.toml` -
  vat-managed meter/guard runners.
- Gate — behavior: pending a phase-1 project at `apps/pgpool/e2e/` - no
  black-box case exists yet for the pooler capability set
- Source: `pending: aw.toml EC inventory`,
  `pending: vat meter/guard runners and external-contracts evidence`
- Evidence: pending: aw ec gen --verify

### HTTP/2 API List

Publish pgpool's admin HTTP surface as a compact route inventory — standard
operational endpoints plus `/pools`, `/pools/{pool}/stats`, and `/drain` — with
the offline `pgpool spec` twin matching the served contract once the admin
plane runs.

- Root WI: #1282
- Surfaces: CLI: `pgpool spec --format routes|openapi|openapi-yaml|json-schema`
  - offline admin API inventory and OpenAPI twin.; HTTP: served `/openapi.json`
  and admin routes on the running admin plane, matching the offline twin
  byte-for-byte.
- Gate — behavior: `cargo test -p pgpool` - offline route inventory names the
  standard and pool admin endpoints
- Gate: served-vs-offline conformance proven by `tests/admin_plane.rs`
- Gate: apps/pgpool/tests/admin_plane.rs
  (`served_contract_matches_offline_spec`, AC3)
- Source: `apps/pgpool/src/spec.rs`, `apps/pgpool/tests/cli_contract.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| offline-route-and-openapi-inventory | change | - | apps/pgpool/src/spec.rs; apps/pgpool/tests/cli_contract.rs |
| served-contract-matches-offline-spec | epic | 1290 | apps/pgpool/tests/admin_plane.rs (`served_contract_matches_offline_spec`); apps/pgpool/tech-design/logic/served-admin-plane-with-drain-aware-readiness.md |

### Kubernetes-Native Deployment

Ship pgpool as a Kubernetes-native pooler: CRD/operator/instance render verbs,
image fixtures rendered from the binary, and pod lifecycle behavior (readiness
flip plus graceful drain) proven in a kind smoke path.

- Root WI: #1284
- Surfaces: CLI: `pgpool k8s crd render`, `pgpool k8s operator render|run`, and
  `pgpool k8s instance render` - layered deployment artifact verbs per the
  service CLI convention.; K8s: namespaced Pgpool CRD, leader-elected operator,
  live endpoint discovery plus pre-apply quota admission, instance profiles,
  and shared stateless Deployment/ClusterIP/PDB composition.
- Gate — behavior: `cargo test -p pgpool --test operator --test cli_contract` -
  CRD/operator/instance artifacts and shared Deployment children render
  deterministically from the binary and typed CR
- Source:
  `apps/pgpool/tests/operator.rs - CRD structural schema, owned stateless render, readiness, budget-status, and operator asset gates`,
  `apps/pgpool/tests/cli_contract.rs - layered k8s CLI render contract`,
  `apps/pgpool/src/k8s/control.rs - deterministic quota admission and drain-before-release reconciliation model`,
  `real kind API-server smoke - generated CRD, Pgpool CR, RBAC, and operator Deployment admitted successfully`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| crd-operator-instance-render | epic | 1284 | apps/pgpool/tests/operator.rs; apps/pgpool/tests/cli_contract.rs; apps/pgpool/tech-design/semantic/pgpool-crd-operator-control-plane.md |
| kind-drain-readiness-smoke | epic | 1284 | pending: kind smoke script |
| stateless-deployment-instance | change | 1561 | apps/pgpool/src/k8s/instance.rs; negative stateful-boundary tests in the same source unit |
| global-endpoint-quota-allocation | change | 1571 | apps/pgpool/src/k8s/budget.rs; apps/pgpool/tech-design/semantic/pgpool-global-endpoint-quota-allocation.md |
| drain-safe-control-plane-status | change | 1573 | apps/pgpool/src/k8s/control.rs; apps/pgpool/tech-design/semantic/pgpool-drain-safe-control-plane-status.md |
| crd-operator-control-plane | change | 1575 | apps/pgpool/src/operator; apps/pgpool/tests/operator.rs; apps/pgpool/tech-design/semantic/pgpool-crd-operator-control-plane.md |

### Long-Running Stability

Run as a long-lived pooler without leaking backend connections or file
descriptors, dropping in-flight transactions on drain, or corrupting pool state
across backend restarts and rolling deploys.

- Root WI: #1282
- Surfaces: CLI: pending `pgpool` serve process - durable pooler with
  drain-aware shutdown.; TCP/HTTP: frontend admission and admin plane surviving
  backend restarts and rolling deploys.
- Gate — stability: pending long-run and drain conformance gates - backend
  reuse without connection/fd leaks, drain without dropped in-flight
  transactions, restart safety
- Gate: apps/pgpool/tests/pool_modes.rs
  (`churn_100_cycles_holds_backend_count_stable_no_leak`)
- Gate: apps/pgpool/tests/pool.rs
  (`dropped_lease_without_explicit_release_does_not_leak_capacity_slot`) —
  bounded-cycle proof, not a true long-run soak
- Source: `pending: drain and backend-restart conformance tests`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| pool-leak-and-reuse-longrun | epic | 1289 | bounded-cycle proof, not a true long-run soak: apps/pgpool/tests/pool_modes.rs (`churn_100_cycles_holds_backend_count_stable_no_leak`); apps/pgpool/tests/pool.rs (`dropped_lease_without_explicit_release_does_not_leak_capacity_slot`) |
| drain-and-backend-restart-safety | epic | 1289 | pending: drain conformance tests |

### Security Hardening

Keep pgpool safe as a network-exposed credential-carrying proxy: auth
passthrough without credential persistence, explicit TLS posture on both
frontend and backend links, malformed wire-frame rejection, and a gated admin
plane before production readiness.

- Root WI: #1286
- Surfaces: TCP: PostgreSQL frontend auth passthrough - client credentials
  verified against the backend, never stored.; HTTP: admin plane exposure
  posture - probes stay tokenless, mutating admin verbs gated.; Env: pending
  TLS material configuration for frontend and backend links.
- Gate — security: pending guard scan and negative gates - auth passthrough,
  TLS posture, malformed-frame rejection, admin exposure
- Source: `pending: vat guard-security runner`,
  `pending: auth passthrough and malformed-frame negative tests`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| auth-passthrough-and-tls-posture | epic | 1286 | pending: negative gates |
| guard-static-runtime-evidence | epic | 1286 | pending: vat guard-security runner |

### Standard Operational Endpoints

Expose the standard one-port operational surface the service trait requires —
probes, metrics scrape, live spec, and Swagger UI stay always-on on the admin
port, with readiness flipping on drain and `pgpool spec` as the offline twin.

- Root WI: #1282
- Surfaces: HTTP: `/healthz`, `/readyz`, `/metrics`, `/openapi.json`, `/docs` -
  one-port operational surface, served on `RuntimePlan.admin_bind` via
  `server_http::serve_h2c_with_options`.; CLI: `pgpool spec` - offline OpenAPI
  evidence for the same contract when no server is running.
- Gate — behavior: `cargo test -p pgpool` - offline inventory carries the five
  standard endpoints
- Gate: served conformance proven by `tests/admin_plane.rs`
- Gate: apps/pgpool/tests/admin_plane.rs (`all_routes_respond_on_h2c_and_http1`
  AC1, `drain_flips_readyz_and_process_exits_cleanly` AC2,
  `metrics_exposes_prometheus_pool_gauges` AC4)
- Source: `apps/pgpool/src/spec.rs`

| Work Root | Kind | WI | Gate / Evidence |
|---|---|---:|---|
| offline-standard-endpoint-inventory | change | - | apps/pgpool/src/spec.rs |
| served-probes-and-drain-flip | epic | 1290 | apps/pgpool/tests/admin_plane.rs (`all_routes_respond_on_h2c_and_http1`, `drain_flips_readyz_and_process_exits_cleanly`, `metrics_exposes_prometheus_pool_gauges`); apps/pgpool/tech-design/logic/served-admin-plane-with-drain-aware-readiness.md |
