# axiom

A monorepo of high-performance, Rust-built developer infrastructure. Each
project below is self-contained and ships its own README — follow the links for
details.

## Projects

| Project | What it is |
|---------|------------|
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
| [mamba](apps/mamba/README.md) | Force-typed Python compiler. |
| [sift](apps/sift/README.md) | Sift is the GCP/GKE-first operational event platform in the Axiom stack. |
| [jet](apps/jet/README.md) | Jet is a Rust-native frontend toolchain. |
| [relay](apps/relay/README.md) | `relay` is the online single-cast pull work-queue broker in the Axiom stack (RabbitMQ/SQS-shaped): a producer publishes a task, a worker pulls (leases) it, runs it, and acks — each message is delivered exactly once to one of the competing consumers, then reclaimed (delete-on-ack). |
| [keep](apps/keep/README.md) | Cloud-native, multi-core key-value / claim-check store — the loom/relay data plane and a Redis / Dragonfly replacement. |
| [pgpool](apps/pgpool/README.md) | `pgpool` is the working app id for Axiom's Kubernetes-native PostgreSQL connection pooler. |
| [meter](apps/meter/README.md) | Local resource measurement for agent-driven Rust development. |

This table is hand-maintained. It used to be spliced in from a generator that no
longer exists, so adding a project under `apps/` adds no row on its own.

## Shared Libraries

Services and tools compose the shared Rust libraries in
[faberline/core](https://github.com/faberline/core) instead of reimplementing
transport, auth, metrics, codegen, replication, durable local storage, backup,
or operator plumbing locally. Each library is a `crates/<name>` package there;
member manifests here depend on it by git tag (`tag = "v0.4.14"`), and READMEs
name it `core/<name>`. Run a library's own tests at the pinned commit with
`bash scripts/faberline-core-test.sh <name>`.

`lumen` and `rig` moved to [faberline/lumen](https://github.com/faberline/lumen)
and [faberline/rig](https://github.com/faberline/rig); consumers here pin them
by git revision.

The library naming grammar still applies; see
[Shared-library naming grammar](CONTRIBUTING.md#shared-library-naming-grammar).

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
| [mamba](apps/mamba/README.md) | `mamba` | _coming soon_ |
| [meter](apps/meter/README.md) | `meter` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/meter/install.sh \| sh` |
| [pgpool](apps/pgpool/README.md) | `pgpool` | _coming soon_ |
| [preview](apps/preview/README.md) | `preview` | `curl -fsSL https://raw.githubusercontent.com/chrischeng-c4/axiom/main/apps/preview/install.sh \| sh` |
| [sift](apps/sift/README.md) | `sift` | _coming soon_ |
| [relay](apps/relay/README.md) | `relay` | _coming soon_ |
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
