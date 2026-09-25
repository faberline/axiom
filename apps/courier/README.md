# courier

## Brief

`courier` is a stateless, GCP-hosted proxy that centralizes GitHub-issue
access for every axiom CLI. It holds the real GitHub credential server-side
and forwards `issue search/view/create/comment` calls to `api.github.com`,
so individual dev machines and CI runners authenticate with a shared
`courier` bearer token instead of each needing their own GitHub credential.
GitHub remains the source of truth for issue data — `courier` stores nothing
of its own.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| GitHub Issues Proxy | #1332 | forwards search/view/create/comment to GitHub with a server-held credential |

### GitHub Issues Proxy

Every axiom CLI can search/view/create/comment on GitHub issues by
authenticating to `courier` with a shared bearer token, without holding a
personal GitHub credential. Every HTTP request is correlatable end to end: W3C
`traceparent` is honored when present and a local root trace is created when
absent, with the ids flowing into every request span and structured log line.
Server-Timing per-response latency attribution (the shared
`service-http::server_timing` contract) is not yet wired into courier's HTTP
stack — that lands in a separate #2490 adoption batch.

- Root WI: #1332
- Surfaces: HTTP: `GET /v1/issues/{owner}/{name}`,
  `GET /v1/issues/{owner}/{name}/{number}`, `POST /v1/issues/{owner}/{name}`,
  `POST /v1/issues/{owner}/{name}/{number}/comments`, `/healthz`, `/readyz`,
  `/metrics`, `/openapi.json`, `/docs` (`service_http::standard_probe_routes`);
  CLI: `courier llm|upgrade|issue`.; Logs: structured stdout with per-request
  trace correlation — the shared `service-http` trace layer
  (`service_http::trace_layer()`) accepts a valid W3C version-00 `traceparent`
  (invalid input is treated as absent) and generates a fresh local root context
  otherwise, so every request span and log line carries
  `trace_id`/`span_id`/`parent_span_id`/`trace_flags`.; HTTP: Server-Timing
  response attribution — shared `service-http::server_timing` contract
  (`Server-Timing: app;dur=` per-response latency), wiring pending (#2490
  adoption batch).
- Gate — behavior: `cargo test -p courier` - proxy forwarding, auth, and repo
  allow-list coverage
- Gate: `cargo test -p courier`
- Gate: trace-context accept/generate passing via `bash scripts/faberline-core-test.sh service-http`
  (faberline/core crates/service-http/src/transport.rs) — courier wires
  `service_http::trace_layer()` in apps/courier/src/http/mod.rs, no
  courier-owned trace-context test exists yet
- Source: `apps/courier/src/http`
- Evidence: apps/courier/src/http
