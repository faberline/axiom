# Tape

## Brief

Tape is Axiom's self-hosted stand-in for Google Cloud Pub/Sub: a
Raft-replicated, durable topic service where publishers append messages to a
topic and any number of subscriptions each receive every message, over tape's
own HTTP/1.1 + h2c API described by `/openapi.json`.

Cloud Pub/Sub is the feature-contract peer. [STATUS.md](STATUS.md) is the
checklist: one row per Pub/Sub surface, stating whether tape supports it
today, supports it with a named limit, or has not built it yet.
[ROADMAP.md](ROADMAP.md) names the outcome that closes each gap.

Today tape ships durable single-message publish, named pull subscriptions that
each advance one cumulative cursor, per-topic retention, Raft replication with
peer mTLS (one replication case currently red, see [STATUS](STATUS.md)), whole-journal backup and cold seed, a Kubernetes operator, and
generated TypeScript, Python, and Rust clients. Per-message acknowledgement,
competing subscribers, push delivery, seek and snapshots, ordering keys,
message filtering, and schema validation are ROADMAP outcomes, not shipped
behaviour.

**Boundaries.**

- [`relay`](../relay/README.md) is the single-queue work-queue broker
  (RabbitMQ/SQS-shaped): one job, one worker, lease, ack, delete. Use relay
  when exactly one worker should take each job; use tape when N independent
  subscriptions must each see every message.
- [`defer`](../defer/README.md) is the Cloud Tasks stand-in for scheduled
  HTTP dispatch. Tape's push delivery (a ROADMAP outcome) is tape's own
  outbound HTTPS, not a hand-off to defer.
- Tape does not speak the `pubsub.googleapis.com` wire protocol, does not
  serve gRPC, and does not offer streaming pull, exactly-once delivery, or
  export subscriptions. It publishes no performance claim against Kafka,
  JetStream, or any other broker; the only performance gate is tape against
  its own baseline.

## Primary workflow

1. Run one node. `--store` keeps the journal on disk; without it the node is
   in-memory. Auth is off by default, so the calls below need no token.

   ```bash
   cargo run -q -p tape --bin tape -- serve --bind 127.0.0.1:7137 --store /tmp/tape/journal.json
   ```

2. Create a subscription on a topic. Topics exist implicitly; the first
   append or subscription creates one.

   ```bash
   curl -sS -X POST http://127.0.0.1:7137/topics/orders/subscriptions \
     -H 'content-type: application/json' -d '{"name":"billing"}'
   ```

3. Publish a message. `key` is optional, `payload` is any JSON value, and the
   response carries the durable offset.

   ```bash
   curl -sS -X POST http://127.0.0.1:7137/topics/orders/append \
     -H 'content-type: application/json' \
     -d '{"key":"order-1","payload":{"total":42}}'
   ```

4. Pull from the subscription. The batch reports the `cursor` it read from,
   the `events`, and the advisory `next_offset`.

   ```bash
   curl -sS -X POST http://127.0.0.1:7137/topics/orders/subscriptions/billing/pull \
     -H 'content-type: application/json' -d '{"limit":10}'
   ```

5. Acknowledge by advancing the cursor to `next_offset`. Until this call, a
   repeated pull returns the same events.

   ```bash
   curl -sS -X POST http://127.0.0.1:7137/topics/orders/subscriptions/billing/ack \
     -H 'content-type: application/json' -d '{"offset":1}'
   ```

The same verbs exist offline against a journal file for local inspection:
`tape append`, `tape subscription create|list|show|pull|ack|delete`,
`tape replay`, and `tape checkpoint`, each taking `--store <path>`.

## Topics and subscriptions

A topic is an append-only, offset-numbered journal. A subscription is a named
cursor into one topic. That is the whole model today, and it maps onto Cloud
Pub/Sub as follows.

| Pub/Sub concept | Tape today | Gap owner in ROADMAP |
|---|---|---|
| Message | `{key?, payload, timestamp_ms?}` on publish; stored as `{topic, offset, timestamp_ms, key?, payload}` | `attributes-and-filters` adds an attribute map; `schema-validation` validates `payload` |
| Publish | `POST /topics/{topic}/append`, one message per request, acknowledged after the durable write | `resource-lifecycle-parity` adds batch publish |
| Subscription | `{topic, name}` plus one cumulative cursor | `subscription-ack-and-competing-subscribers` turns it into a configured resource with leases, retry policy, and dead-letter topic |
| Pull | `POST .../subscriptions/{name}/pull` returns events from the cursor; every caller sees the same window until an ack | The same outcome adds per-message ackIds, in-flight leases, and competing pullers |
| Ack | `POST .../subscriptions/{name}/ack` sets the cursor | The same outcome adds per-message ack, `modifyAckDeadline`, and nack |
| Push | Not built | `push-subscriptions` |
| Seek and snapshot | `GET /topics/{topic}/replay?from_offset=` and `?from_timestamp_ms=` read the journal directly | `seek-snapshot-and-retention` moves seek onto the subscription and adds snapshots |
| Retention | `PUT /topics/{topic}/retention` with `min_offset`, `max_age_seconds`, `protected_consumers` | The same outcome proves age-based expiry and narrows the public shape |
| Ordering keys | `key` is stored and returned; delivery does not order by it | `ordering-keys` |
| Filtering | Not built | `attributes-and-filters` |

Two tape-only surfaces predate the subscription model and remain public for
now: journal replay (`GET /topics/{topic}/replay`, `/replay/stream`) and
consumer checkpoints (`GET|PUT /topics/{topic}/consumers/{consumer}/checkpoint`).
Subscriptions are built on them. They leave the public API in the
`seek-snapshot-and-retention` outcome, not before, because they are the
readback oracle for the current acceptance scripts.

## Contract discovery

| Need | Source of truth |
|---|---|
| Route inventory | `tape spec --format routes`, or `GET /openapi.json` on a running node. The committed [`clients/openapi.json`](clients/openapi.json) is byte-equal to live generation; a gate refuses drift. |
| Request and response schemas | `tape spec --format json-schema`, or the `components` section of the OpenAPI document. |
| CLI surface | `tape --help`; `tape llm` prints the agent-facing topic index. |
| Support state per Pub/Sub surface | [STATUS.md](STATUS.md) |
| Future outcomes and non-goals | [ROADMAP.md](ROADMAP.md) |
| Typed clients | [`clients/README.md`](clients/README.md) |
| Serve flags, environment, ports, probes, runbooks | [`docs/deployment-handoff.md`](docs/deployment-handoff.md) |
| Kubernetes custom resource | `tape k8s crd render`; the operator and instance layers come from `tape k8s operator render` and `tape k8s instance render`. |
| Backup destinations | `tape backup --help` lists the accepted schemes: `file://`, `s3://` (feature `backup`), and `gs://`. |

## Capabilities

Every entry below is a tape product capability. The list has no primary and
secondary classes.

A capability can have several sources. `apps/tape` supplies tape-specific
behaviour and composition. `core/<name>` supplies a reusable mechanism from
[faberline/core](https://github.com/faberline/core).
`external:<name>` supplies an outside runtime or provisioned contract. Each
source below states its direct contribution.

### Capability index

| Capability | ID | User promise | Sources |
|---|---|---|---|
| Publish and durable write | `topic-publish` | Append one message to a topic and receive its offset only after the write is durable. | `apps/tape`, `core/storage-durable`, `core/raft-runtime` |
| Pull subscriptions | `pull-subscriptions` | Create named subscriptions on a topic and pull messages from each subscription's own cursor. | `apps/tape` |
| Topic retention | `topic-retention` | Bound a topic's journal by an offset floor without dropping messages a protected consumer still needs. | `apps/tape` |
| Replicated availability | `replicated-availability` | Run a Raft group whose members replicate, forward, fail over, and rejoin over mutually authenticated peer links. | `apps/tape`, `core/raft-runtime`, `core/peer-tls` |
| Backup and seed | `backup-and-seed` | Export a whole-journal snapshot to a sink and restore it into an empty node. | `apps/tape`, `core/service-backup`, `core/storage-durable` |
| Security hardening | `security-hardening` | Gate data-plane routes by per-topic grants, bound request admission, and keep management audit redacted. | `apps/tape`, `core/service-auth`, `core/service-http`, `core/peer-tls` |
| Kubernetes-native deployment | `kubernetes-native-deployment` | Reconcile a `Tape` custom resource, or apply the direct-install base, into stable Kubernetes workloads. | `apps/tape`, `core/service-k8s`, `external:kubernetes` |
| Operations and observability | `operations-observability` | Expose health, readiness, metrics, traces, and graceful drain on one port. | `apps/tape`, `core/service-http`, `core/metrics-prometheus` |
| API, CLI, and clients | `api-cli-clients` | Publish one discoverable HTTP contract and generate typed clients from it. | `apps/tape`, `core/service-http`, `core/transport-h2c`, `core/openapi-codegen`, `core/cli-std` |
| Local performance ceiling | `local-performance-ceiling` | Keep append, replay, and checkpoint latency inside tape's own release-mode budget. | `apps/tape` |

### Publish and durable write

- ID: `topic-publish`
- Promise: Accept one JSON message per request, fsync it through the WAL, and
  answer with its offset; refuse oversized bodies with 413; keep reads serving
  and answer 507 when the volume is full.
- Sources:
  - [`apps/tape`](./) defines the message envelope, the journal, the WAL frame
    format, the group-commit fsync, and the storage-full degraded mode.
  - [`core/storage-durable`](https://github.com/faberline/core/blob/v0.4.13/crates/storage-durable/README.md) provides
    durable files, atomic replacement, fsync, and framed logs.
  - [`core/raft-runtime`](https://github.com/faberline/core/blob/v0.4.13/crates/raft-runtime/README.md) orders and
    replicates the append when the node runs in a group.
- Gate: `cargo test -p tape --test http_transport --test durable_write_path --test durable_crash_recovery`
- Gate: `cargo test -p tape --lib`

### Pull subscriptions

- ID: `pull-subscriptions`
- Promise: Create, list, show, and delete named subscriptions on a topic; pull
  a bounded window from the subscription cursor; advance the cursor with an
  explicit ack; report per-subscription lag.
- Sources:
  - [`apps/tape`](./) defines the subscription resource, the pull window, the
    ack semantics, the lag gauge, and the offline `--store` verbs.
- Gate: `cargo test -p tape --test cli_contract --test http_transport`
- Gate: `cargo test -p tape --test provision_topics_via_spec`

### Topic retention

- ID: `topic-retention`
- Promise: Set and read a per-topic retention policy whose floor never passes
  a protected consumer's checkpoint, and let a backfill append land behind the
  live head without moving any consumer.
- Sources:
  - [`apps/tape`](./) defines the retention policy, the protected floor, and
    the backfill offsets.
- Gate: `cargo test -p tape --test retention_backfill`

### Replicated availability

- ID: `replicated-availability`
- Promise: Elect a leader among three members, replicate every append,
  forward writes from followers, catch a fresh member up by snapshot, survive
  a leader kill without committed loss, and restart a single member onto its
  applied floor. One follower-forwarding case is currently red against the
  shared runtime's JSON publish handler; the [STATUS](STATUS.md) row
  `raft-replication` names it and the `deterministic-failover` outcome owns
  the repair.
- Sources:
  - [`apps/tape`](./) defines the replicated command set, the applied-floor
    recovery, and the peer mTLS listener that keeps raft routes off the public
    router.
  - [`core/raft-runtime`](https://github.com/faberline/core/blob/v0.4.13/crates/raft-runtime/README.md) hosts the group,
    the log, snapshots, and forwarding.
  - [`core/peer-tls`](https://github.com/faberline/core/blob/v0.4.13/crates/peer-tls/README.md) provides the mutual TLS
    material and verification for peer links.
- Gate: `cargo test -p tape --test raft_cluster --test raft_persistence --test raft_peer_mtls`
- Gate: `cargo test -p tape --test raft_failover`

### Backup and seed

- ID: `backup-and-seed`
- Promise: Stream a whole-journal snapshot from `GET /admin/backup`, ship it
  with `tape backup` to a `file://`, `s3://`, or `gs://` destination, and
  restore it with `--bootstrap-seed-uri` into an empty data directory only.
- Sources:
  - [`apps/tape`](./) defines the snapshot route, the backup verb, the
    seed-only-into-empty rule, and the redacted backup audit.
  - [`core/service-backup`](https://github.com/faberline/core/blob/v0.4.13/crates/service-backup/README.md) provides the
    destination sinks and retention pruning.
  - [`core/storage-durable`](https://github.com/faberline/core/blob/v0.4.13/crates/storage-durable/README.md) provides
    the atomic restore write.
- Gate: `cargo test -p tape --features backup --test backup --test backup_destination_docs`
- Gate: `cargo test -p tape --test bootstrap --test seed_ha_bootstrap`

### Security hardening

- ID: `security-hardening`
- Promise: Require per-topic read, write, and admin grants when `--auth
  required`, keep probes tokenless, classify append as write admission, and
  keep the backup audit redacted and off the hot data-plane routes.
- Sources:
  - [`apps/tape`](./) defines the grant model, the admission classification,
    and the audit record.
  - [`core/service-auth`](https://github.com/faberline/core/blob/v0.4.13/crates/service-auth/README.md) provides the
    bearer-token registry and grant evaluation.
  - [`core/service-http`](https://github.com/faberline/core/blob/v0.4.13/crates/service-http/README.md) provides the
    shared router shell, error envelope, and admission hooks.
  - [`core/peer-tls`](https://github.com/faberline/core/blob/v0.4.13/crates/peer-tls/README.md) provides the peer
    identity plane.
- Gate: `cargo test -p tape --test service_auth --test service_admission --test audit_contract`
- Gate: `cargo test -p tape --test raft_peer_mtls`

### Kubernetes-native deployment

- ID: `kubernetes-native-deployment`
- Promise: Render and reconcile a `Tape` custom resource into a StatefulSet,
  Services, PodDisruptionBudget, ConfigMap, backup CronJob, and observability
  pair with status conditions; ship a direct-install base for a durable
  singleton; provision topics and subscriptions declaratively.
- Sources:
  - [`apps/tape`](./) defines the CRD, defaults, topology policy, conditions,
    render verbs, direct-install base, and provisioning.
  - [`core/service-k8s`](https://github.com/faberline/core/blob/v0.4.13/crates/service-k8s/README.md) provides reusable
    reconciliation, leader election, workload, Service, and status mechanisms.
  - `external:kubernetes` stores desired state and runs the workload,
    network, lease, RBAC, and Secret contracts.
- Gate: `cargo test -p tape --features operator --test operator --test operator_render_provision_topics`
- Gate: `cargo test -p tape --test deploy_cli --test direct_k8s_assets --test network_policy_assets --test observability_assets`
- Gate: `bash apps/tape/scripts/kind-e2e.sh`
- Gate: `bash acceptance/gcp/scripts/verify-tape.sh`

Release 0.4.11 (2026-07-25, digest-pinned multi-arch GHCR image) passed four
GKE acceptance runs on 2026-07-23 and 2026-07-24. That is historical
evidence; it does not define the current contract.

### Operations and observability

- ID: `operations-observability`
- Promise: Serve `/healthz`, `/readyz`, `/metrics`, `/openapi.json`, and
  `/docs` on the data-plane port; flip readiness to 503 on drain; emit
  request counters, latency sums, topic offset and subscription lag gauges;
  export OTLP traces; survive repeated restarts without losing history.
- Sources:
  - [`apps/tape`](./) defines the tape metric families, the drain window, and
    the stateful restart behaviour.
  - [`core/service-http`](https://github.com/faberline/core/blob/v0.4.13/crates/service-http/README.md) provides the
    probe routes, `Server-Timing`, structured logs, and OTLP wiring.
  - [`core/metrics-prometheus`](https://github.com/faberline/core/blob/v0.4.13/crates/metrics-prometheus/README.md)
    provides the Prometheus text exposition.
- Gate: `cargo test -p tape --test http_transport --test shared_otlp_tracing --test long_running_stability`
- Gate: `cargo test -p tape --test rig_stateful_adapter`

### API, CLI, and clients

- ID: `api-cli-clients`
- Promise: Serve exactly the routes the spec lists, over HTTP/1.1 and h2c on
  one port; generate TypeScript, Python, and Rust clients scoped to those
  routes; ship the standard CLI surface.
- Sources:
  - [`apps/tape`](./) defines the route inventory, the OpenAPI document, and
    the CLI verbs.
  - [`core/service-http`](https://github.com/faberline/core/blob/v0.4.13/crates/service-http/README.md) provides the
    router shell and error envelope.
  - [`core/transport-h2c`](https://github.com/faberline/core/blob/v0.4.13/crates/transport-h2c/README.md) provides the
    cleartext HTTP/2 listener shared with HTTP/1.1.
  - [`core/openapi-codegen`](https://github.com/faberline/core/blob/v0.4.13/crates/openapi-codegen/README.md) provides
    the in-binary client generator.
  - [`core/cli-std`](https://github.com/faberline/core/blob/v0.4.13/crates/cli-std/README.md) provides the standard
    command set and output conventions.
- Gate: `cargo test -p tape --test spec_route_parity --test spec_generated_clients --test cli_contract`

### Local performance ceiling

- ID: `local-performance-ceiling`
- Promise: Keep append, replay, and checkpoint inside the release-mode budget
  measured against tape's own baseline, show durable append throughput rising
  with connection count, and never claim a win over another broker.
- Sources:
  - [`apps/tape`](./) defines the benchmark, the budget, and the
    `tape-bench` CLI.
- Gate: `cargo test --release -p tape --test tape_perf_gate`

## Supporting documents

| Document | Use it for |
|---|---|
| [Product requirements](docs/product/README.md) | What tape promises per capability area, one section per shipped capability or ROADMAP outcome; epics are carved from it |
| [STATUS.md](STATUS.md) | Current support per Cloud Pub/Sub surface, with limits and gates |
| [ROADMAP.md](ROADMAP.md) | Future outcomes and explicit non-goals |
| [Generated clients](clients/README.md) | Client generation, language matrix, connection inputs, and current limits |
| [Deployment handoff](docs/deployment-handoff.md) | Images, serve flags, environment, HTTP surface, smoke sequence, backup and restore runbooks |
| [Node drain and PodDisruptionBudget](docs/runbooks/node-drain-and-pdb.md) | Unblocking a drain on the direct-install singleton and verifying recovery |
| [Benchmark history](docs/benchmarks-scale.md) | The local performance gate and the retired peer-broker calibrations |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Tape edit rules and required verification commands |

Historical evidence and implementation planning do not define the current
product contract. Use the capability gates and the published runtime
contracts.
