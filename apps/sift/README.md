# Sift

## Brief

Sift is one SRE product for logs, metrics, and traces.

Users see one Rust binary, API, CLI, MCP server, container image, version, and
Kubernetes resource. A small installation can run the `all` role. A larger GKE
installation can run the same binary as `agent`, `gateway`, `query`, `store`,
`control`, or `operator` roles.

Sift is Agent First. Phase one has no Web UI. It exposes read-only tools for
agents and operators. It does not expose the old generic event, profile, audit,
evaluation, or separate error-report APIs.

Sift owns its storage engines. It does not require Loki, Mimir, Tempo,
ClickHouse, or another external database. GCS is an optional long-term segment
and backup store for GKE.

The product direction follows these useful parts of mature products:

| Product | Sift direction |
|---|---|
| [Datadog](https://docs.datadoghq.com/getting_started/tagging/unified_service_tagging/) | Use one service, environment, and version identity across every signal. |
| [Grafana Cloud](https://grafana.com/docs/grafana-cloud/) | Give users one product while each signal keeps a suitable internal engine. |
| [New Relic](https://docs.newrelic.com/docs/nrql/get-started/introduction-nrql-new-relics-query-language/) | Give users one versioned query entry point. |
| [Elastic Observability](https://www.elastic.co/docs/solutions/observability) | Separate hot data, archived data, retention, alerts, and SLO work. |
| [Honeycomb](https://docs.honeycomb.io/investigate/query/build/) | Make trace-led diagnosis and high-cardinality evidence easy to retrieve. |

## Primary workflow

1. Start Sift with a writable data volume at `/var/lib/sift`.
2. Send OTLP logs, metrics, and traces to the Sift gateway.
3. Send Prometheus Remote Write 1.0 metrics when a Prometheus client is used.
4. Query one signal or correlate several signals through the Sift API, CLI, or MCP tools.
5. Keep recent data on the Sift volume and commit immutable Parquet segments to GCS when GCS is configured.

## Signal ingest

Sift accepts the official OpenTelemetry Protocol, called OTLP.

| Transport | Endpoint | Phase-one contract |
|---|---|---|
| OTLP/HTTP | `POST /v1/logs` | JSON, Protobuf, gzip, and partial success |
| OTLP/HTTP | `POST /v1/metrics` | JSON, Protobuf, gzip, and partial success |
| OTLP/HTTP | `POST /v1/traces` | JSON, Protobuf, gzip, and partial success |
| OTLP/gRPC | port `4317` | Official generated logs, metrics, and traces services with gzip |
| Prometheus | `POST /prometheus/api/v1/write` | Stable Remote Write 1.0 with Snappy block compression |

Remote Write 2.0 is experimental and is rejected. The removed `/v1/events`
and `/v1/events:write` routes return `404`.

The external Remote Write gate pins Prometheus compliance commit
`67b8327a2e93dc28f64d4b21bbce00b362f565d5` and a digest-pinned Go image. It
runs only the official RW1 receiver compatibility cases because the remaining
upstream receiver suite targets RW2. The Rust `prometheus_api` E2E also requires
a successful durable write and query readback because the upstream suite does
not check stored data.

Prometheus queries reject selectors above 1,000 candidate series. Range
queries also reject more than 11,000 evaluation steps or more than 1,000,000
series-step evaluations. The metric projection checks candidate cardinality
before it loads points. It skips disk chunks outside the requested time window
and rejects a query before it scans more than 250,000 source points or
materializes more than 64 MiB of source metric data.

The gateway checks request size, schema, project scope, quotas, and sensitive
fields before it forwards an accepted batch to the store role. The store only
acknowledges a replicated write after the Raft quorum has made it durable.

## Query and agent surface

The main product API is:

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/api/v1/query` | Run a versioned logs, metrics, or traces query. |
| `POST` | `/api/v1/logs/tail` | Wait for new matching logs for a bounded time. |
| `GET` | `/api/v1/traces/{trace_id}` | Read one trace with explicit gap diagnostics. |
| `POST` | `/api/v1/correlate` | Find related signals by trace, span, service, exemplar, or time. |
| `GET` | `/api/v1/services` | List known services and their environments and versions. |
| `GET` | `/api/v1/queries/{query_id}` | Read a persisted asynchronous query job. |
| `GET` | `/prometheus/api/v1/query` | Run an instant PromQL query. |
| `GET` | `/prometheus/api/v1/query_range` | Run a range PromQL query. |

`QueryRequestV1` is a versioned JSON abstract syntax tree. An abstract syntax
tree is a structured query, not a free-form query string. It supports project,
environment, time range, signal type, Boolean filters, equality, membership,
existence, ranges, text, regular expressions, limits, cursors, and sync or
async execution. Metric queries add a function, step, and group fields. Trace
queries add service, operation, duration, status, and attribute fields.

Responses include `data`, `next_cursor`, `watermark`, `partial`, `warnings`,
`stats`, and an optional `query_id`. Expensive async jobs are saved under
`query-jobs` and survive a process restart.

The MCP endpoint is `/mcp`. The same Rust binary also supports:

```sh
sift mcp serve --stdio --endpoint http://127.0.0.1:7380
```

Phase one exposes only these read-only MCP tools:

- `sift_query`
- `sift_get_trace`
- `sift_correlate`
- `sift_list_services`
- `sift_tail_logs`

The CLI and MCP clients call the same Sift API and use the same JSON schemas.

## Durable storage

Every normal Sift mode is durable by default. The data directory order is:

1. `--data-dir`
2. `SIFT_DATA_DIR`
3. `/var/lib/sift`

Sift does not fall back to the current directory or `/tmp`. It refuses to start
when the data root is not writable. `--ephemeral` creates a temporary root only
for the local `all` role. Production roles and production environments reject
that flag.

The fixed layout is:

```text
/var/lib/sift/
├── layout.json
├── control/
├── wal/
│   ├── logs/
│   ├── metrics/
│   └── traces/
├── segments/
│   ├── logs/
│   ├── metrics/
│   └── traces/
├── indexes/
├── snapshots/
├── archive-cache/
├── gateway-spool/
├── query-jobs/
├── agent/
└── tmp/
```

`layout.json` stores the format version, cluster ID, node ID, and role. A
process lock prevents two processes from opening one root. Directories use
mode `0700`. Data files use mode `0600`. Temporary files stay on the same file
system so Sift can use an atomic rename.

Sift checks the WAL, segment manifest, segments, and indexes when it starts.
It can rebuild an index from committed segments. It cannot replace a missing
WAL or committed segment with an index. A Sift 0.1.1 legacy root is refused and
is not changed. This version has no automatic legacy migration.

Each signal has its own WAL and immutable segment tree. A Raft batch is bounded
by 10 milliseconds or 1 MiB. Three-voter roles require two durable voters for
an acknowledgement. A committed GCS archive writes Snappy-compressed Apache
Parquet objects first. It then writes content-addressed 64 KiB catalog pages
and a fixed-size V10 root manifest last. Cleanup uses a paged plan and a durable
cursor. A restart resumes at the next object. Only the committed root permits
WAL compaction. A failed archive keeps the WAL. Local capacity limits return
retryable backpressure before the volume is exhausted.

Retention scans at most 64 archive segments per generation. The manifest keeps
the next catalog key and one fixed cutoff. A caught-up voter applies the small
source/target delta. Only an empty, behind, or inconsistent voter downloads the
full archive.

Sift guarantees exact event-ID idempotency for six hours after its stored
`acknowledged_at` time. The leader selects this Raft decision time before the
proposal. Every voter stores the same value. `AppendResult` returns this value.
A network response can arrive later and does not change the stored decision
time. A retry in the six-hour period returns the original cursor and timestamp
across a process restart or voter failover. Sift makes no duplicate-suppression
promise after six hours. This limit does not change telemetry retention.
Telemetry stays queryable for up to 180 days under the fixed hot and archive
policy.

## Deployment model

The one Sift binary supports these roles:

| Role | Work |
|---|---|
| `all` | Run a persistent single-process product for local use, Docker, and small installations. |
| `agent` | Collect local files, standard input, or Kubernetes CRI logs and keep checkpoints. |
| `gateway` | Authenticate and forward HTTP and OTLP/gRPC ingest, query, and MCP traffic. |
| `query` | Serve query traffic from the durable store source of truth. |
| `store` | Own the logs, metrics, and traces WAL, segments, projections, and Raft group. |
| `control` | Own durable cluster control state in its own Raft group. |
| `operator` | Reconcile the Sift Kubernetes resource. |

The checked-in Docker image sets `SIFT_DATA_DIR=/var/lib/sift`, declares that
path as a volume, and runs Sift as a non-root user. The example uses a named
volume:

```sh
docker compose -f apps/sift/compose.yaml up --build
```

Removing the container does not remove the `sift-data` volume.

The Kubernetes operator creates gateway and query Deployments, store and
control StatefulSets, an agent DaemonSet, Services, PVCs, disruption budgets,
and a NetworkPolicy. Each stateful or local-state role mounts a PVC at
`/var/lib/sift`. The agent keeps its checkpoint under `/var/lib/sift/agent`.
The gateway and query roles keep their spool and job state under the same root.

The store and control roles use one fixed three-voter topology. Their dedicated
Raft listener is port `7381` and requires mutual TLS. The Secret named by
`spec.peerTlsSecret` must contain `tls.crt`, `tls.key`, and `ca.crt`. Its server
certificate must cover the pod DNS names below the store and control headless
Services.

Kubernetes delegated authentication has two cluster prerequisites. The Sift
operator creates one per-instance `system:auth-delegator` ClusterRoleBinding.
Its own ClusterRole allows ClusterRoleBinding apply and `bind` on only that
built-in role. A finalizer removes the binding when auth is disabled or the
Sift resource is deleted. The operator also discovers the ready
`default/kubernetes` endpoint and limits review calls to that exact IP. Sift
fails closed when either step fails.

GKE deployments require Dataplane V2 with
[FQDN Network Policy](https://docs.cloud.google.com/kubernetes-engine/docs/how-to/fqdn-network-policies)
enabled. Sift uses it to allow the store and backup roles to reach only
`storage.googleapis.com` on port 443. Each policy selects one Sift instance and
one role. Removing a `gs://` destination removes its policy. Standard
NetworkPolicy rules allow only the discovered Kubernetes API endpoint, the GKE
metadata server, cluster DNS, and the named Sift role peers.

## Security

Static token mode reads a role registry from a mounted Secret. Kubernetes mode
checks each bearer token with TokenReview. It then checks the requested project
and operation with SubjectAccessReview. Cross-project access is rejected.

The agent and backup jobs use an audience-bound projected ServiceAccount token.
They read the token file for every request so token rotation does not require a
restart. Peer traffic uses mutual TLS on a port that is separate from the
public API. The `/mcp` transport checks allowed Host and Origin values.

The gateway applies project scope and redaction before durable storage. Sift
does not print bearer tokens or private key material in normal errors.

## Current limits

This implementation is a phase-one foundation. These limits are important:

- PromQL supports metric selectors and `sum`, `avg`, `min`, `max`, `count`, and `rate`. It is not a full PromQL implementation.
- Logs support structured search, bounded text search, facets, context, and tail. They do not yet have production-scale load evidence.
- Traces support trace reads, search filters, gaps, and critical paths. Tail sampling and a complete service dependency engine are not complete.
- Local and emulated GCS archive, outage, lifecycle, and fresh-volume restore tests pass. A live GCS recovery drill has not run for this candidate.
- Three local voters pass a mutual-TLS leader-loss test. The required 30-minute GKE MVP run has not run for this candidate.
- The fixed 30-day hot and 180-day total retention worker is implemented. It keeps WAL during a GCS outage, rewrites mixed-age segments, removes expired blobs, and resumes safe cleanup after restart. Exact event-ID idempotency is a separate six-hour window.
- The 10,000-item-per-second MVP target has not been proven on GKE. The later 100,000-item-per-second and 24-hour production gates also remain unproven.
- Alerts, saved queries, SLOs, incidents, on-call, runbooks, synthetics, RUM, profiles, and cost analysis are later phases.

Do not use a green unit or local integration test as evidence for a live GKE or
production performance claim.

## Contract discovery

The `apps/sift` source owns the product boundary, Rust binary, signal engines,
public schemas, deployment files, and end-to-end tests.

An `apps/<name>` source is another Axiom application with its own product
boundary. A `core/<name>` source is a shared Rust contract from
[faberline/core](https://github.com/faberline/core) that Sift composes.
An `external:<name>` source is a protocol or file format maintained outside
this repository. Sift owns the adapter and compatibility tests for every
external contract that it adopts.

The runtime OpenAPI document is available at `/openapi.json`. Each test listed
below is an explicit Cargo test target under `apps/sift/e2e`.

## Capabilities

### Capability index

| Capability | ID | User promise | Sources |
|---|---|---|---|
| Signal ingest | `signal-ingest` | Send logs, metrics, and traces through standard telemetry protocols. | `apps/sift`<br>`external:opentelemetry`<br>`external:prometheus` |
| Unified investigation | `unified-investigation` | Query and correlate every phase-one signal through one product API. | `apps/sift`<br>`core/index-text` |
| Durable local data | `durable-local-data` | Keep accepted data in one private and versioned persistent root. | `apps/sift`<br>`core/storage-durable` |
| Replicated availability | `replicated-availability` | Keep acknowledged data after one voter fails. | `apps/sift`<br>`core/raft-core`<br>`core/raft-runtime`<br>`core/peer-tls` |
| Archive and restore | `archive-and-restore` | Commit immutable Parquet archives and restore their exact events. | `apps/sift`<br>`core/storage-segment`<br>`core/service-projection`<br>`core/service-backup`<br>`external:apache-parquet` |
| Agent and CLI access | `agent-and-cli-access` | Let people and agents use the same read-only product schemas. | `apps/sift`<br>`core/cli-std`<br>`external:model-context-protocol` |
| Kubernetes operation | `kubernetes-operation` | Run one Sift product as secure role-based GKE workloads. | `apps/sift`<br>`core/service-k8s`<br>`core/service-auth` |
| Structured log collection | `structured-log-collection` | Resume file and CRI collection without silently skipping bytes. | `apps/sift`<br>`core/service-observability` |

### Signal ingest

- ID: `signal-ingest`
- Promise: Accept official OTLP logs, metrics, and traces plus stable Prometheus Remote Write 1.0.
- Sources:
  - `apps/sift` owns validation, limits, partial success, routing, and durable admission.
  - `external:opentelemetry` defines the official OTLP messages and collector services.
  - `external:prometheus` defines the stable Remote Write 1.0 wire contract.
- Gate: `bash apps/sift/test.sh --test otlp_gcp_ingest`
- Gate: `bash apps/sift/test.sh --test otlp_grpc`
- Gate: `bash apps/sift/test.sh --test prometheus_api`
- Gate: `bash apps/sift/e2e/prometheus_compliance.sh`

### Unified investigation

- ID: `unified-investigation`
- Promise: Give logs, metrics, and traces one versioned query, correlation, service, and async-job surface.
- Sources:
  - `apps/sift` owns the public query AST, correlation rules, projections, and response contract.
  - `core/index-text` supplies the embedded and rebuildable log index primitives.
- Gate: `bash apps/sift/test.sh --test text_index_migration`
- Gate: `bash apps/sift/test.sh --test phase_one_api`
- Gate: `bash apps/sift/test.sh --test trace_store`

### Durable local data

- ID: `durable-local-data`
- Promise: Refuse unsafe roots and recover accepted signal data from private WAL and segments.
- Sources:
  - `apps/sift` owns the fixed layout, signal WAL, segments, lock, and startup checks.
  - `core/storage-durable` supplies framed durable logs and torn-tail recovery behavior.
- Gate: `bash apps/sift/test.sh --test persistent_data_dir`
- Gate: `bash apps/sift/test.sh --test durable_signal_wal`
- Gate: `bash apps/sift/test.sh --test local_backpressure`
- Gate: `bash apps/sift/test.sh --test bounded_dedupe_window`

### Replicated availability

- ID: `replicated-availability`
- Promise: Acknowledge replicated batches through a three-voter quorum and keep them after leader loss.
- Sources:
  - `apps/sift` maps Sift batches into the shared Raft state machine and role topology.
  - `core/raft-core` supplies the consensus log and quorum rules.
  - `core/raft-runtime` supplies durable host, transport, apply, and snapshot behavior.
  - `core/peer-tls` supplies required mutual TLS for dedicated peer traffic.
- Gate: `bash apps/sift/test.sh --test raft_batch`
- Gate: `bash apps/sift/test.sh --test raft_failover`
- Gate: `bash apps/sift/test.sh --test bounded_raft_state`
- Gate: `bash apps/sift/test.sh --test raft_archive_checkpoint`

### Archive and restore

- ID: `archive-and-restore`
- Promise: Write immutable signal segments before the manifest, preserve WAL on archive failure, and make every voter adopt the same retained prefix before Raft compaction.
- Sources:
  - `apps/sift` owns archive manifests, segment hashes, commit order, WAL compaction, and restore checks.
  - `core/storage-segment` supplies immutable paged catalogs and bounded catalog readers.
  - `core/service-projection` rebuilds typed projections when the retained source generation changes.
  - `core/service-backup` supplies destination, upload, fetch, and retention behavior.
  - `external:apache-parquet` defines the immutable columnar segment file format.
- Gate: `bash apps/sift/test.sh --test gcs_archive`
- Gate: `bash apps/sift/test.sh --test cold_query_archive`
- Gate: `bash apps/sift/test.sh --test retention_lifecycle`
- Gate: `bash apps/sift/test.sh --test paged_archive_gc`
- Gate: `bash apps/sift/test.sh --test raft_archive_checkpoint`
- Gate: `bash apps/sift/test.sh --test live_backup`

### Agent and CLI access

- ID: `agent-and-cli-access`
- Promise: Expose the same phase-one contracts through HTTP, CLI, and read-only MCP tools.
- Sources:
  - `apps/sift` owns the CLI commands, OpenAPI schema, MCP tools, and client calls.
  - `core/cli-std` supplies common terminal output and operational command behavior.
  - `external:model-context-protocol` defines the MCP transports and tool exchange contract.
- Gate: `bash apps/sift/test.sh --test cli_contract`
- Gate: `bash apps/sift/test.sh --test mcp_surface`

### Kubernetes operation

- ID: `kubernetes-operation`
- Promise: Render durable role workloads with project authorization, peer TLS, and PVC recovery boundaries.
- Sources:
  - `apps/sift` owns the Sift resource, operator adapter, role images, and network policy.
  - `core/service-k8s` supplies the shared reconcile and workload rendering framework.
  - `core/service-auth` supplies TokenReview, SubjectAccessReview, projected tokens, and fail-closed decisions.
- Gate: `bash apps/sift/test.sh --test deployment_cli`
- Gate: `bash apps/sift/test.sh --test persistent_deployment`
- Gate: `bash apps/sift/test.sh --test kubernetes_auth`

### Structured log collection

- ID: `structured-log-collection`
- Promise: Collect JSONL and CRI logs with durable checkpoints, bounded retry, quarantine, and byte-loss evidence.
- Sources:
  - `apps/sift` owns source discovery, framing, retry, checkpoints, quarantine, and ingest mapping.
  - `core/service-observability` defines the structured service-log schema used by Axiom applications.
- Gate: `bash apps/sift/test.sh --test collector_cri`
- Gate: `bash apps/sift/test.sh --test structured_stdout_collector_e2e`

## Supporting documents

- [High availability operations](HA.md) explains role deployment, peer TLS, backup, and recovery limits.
- [Structured stdout observability](observability/structured-stdout.md) explains application and collector ownership.
- [Agent context](llms.txt) lists the supported build, test, and safety commands.
