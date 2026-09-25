# Tape generated clients

This directory holds tape's committed OpenAPI contract and the pinned client
target policy. The clients themselves are generated on demand and are not
committed.

## Contract

`openapi.json` is a committed snapshot of `tape spec --format openapi`, the
same document a running node serves at `GET /openapi.json`. A gate refuses any
drift between the two, so client generation works offline against this file.

```bash
cargo run -q -p tape --bin tape -- spec --format openapi > apps/tape/clients/openapi.json
```

Regenerate and commit the snapshot in the same change that alters a route or a
schema field. The subscription, push, and seek outcomes in
[ROADMAP.md](../ROADMAP.md) each move this file.

## Generate

Clients are generated in-binary by `tape spec gen` through the shared
`core/openapi-codegen` crate from faberline/core. No Makefile, `node`, or external generator is
involved; a `cargo` toolchain is the only requirement.

```bash
cargo run -q -p tape --bin tape -- spec gen --lang ts --out apps/tape/clients/ts
cargo run -q -p tape --bin tape -- spec gen --lang py --out apps/tape/clients/py
cargo run -q -p tape --bin tape -- spec gen --lang rust --out apps/tape/clients/rust
```

`codegen.toml` pins the default target for each language. Every generated
client includes `.openapi-codegen.json` naming the exact target contract; pass
`--target <profile>` only for an explicit override.

## Language matrix

| Language | Generated form | Transport | Auth input | Current limits |
|---|---|---|---|---|
| TypeScript | Typed fetch client with request and response types per route | HTTP/1.1 or h2c to the serve port | Bearer token header when `--auth required` | Pull and ack expose the cumulative cursor; no ackIds, push, or seek types until those outcomes land |
| Python | Typed client module with dataclass-style request and response models | HTTP/1.1 or h2c to the serve port | Bearer token header when `--auth required` | Same route scope as TypeScript; the streaming replay route is served as a framed body the client does not decode |
| Rust | Crate with a typed client over `reqwest` | HTTP/1.1 or h2c to the serve port | Bearer token header when `--auth required` | Same route scope as TypeScript; consumers pin the generator output in their own repository |

## Connect

- Base URL: the serve port, `127.0.0.1:7137` by default or whatever `--bind`
  and `TAPE_BIND` set. In Kubernetes use the `tape` ClusterIP Service for
  load-balanced calls; the per-pod names go through the headless Service.
- Token: with `--auth required`, send `Authorization: Bearer <token>` from a
  registry entry that grants `read`, `write`, or `admin` on the topic. Probes
  and the OpenAPI document stay tokenless.
- Transport: HTTP/1.1 and cleartext HTTP/2 share the port; a client may use
  either without a TLS handshake. TLS termination is the deployment's job.

## Current boundaries

- Generated clients cover exactly the routes in `openapi.json`. Routes that
  the seek outcome retires will disappear from the clients when the snapshot
  is regenerated.
- The clients carry no retry, backoff, or idempotency logic; append is
  at-least-once from the caller's side and a retried append can land twice.
- Nothing here speaks the Cloud Pub/Sub client libraries' protocol. A caller
  migrating from Pub/Sub swaps the client, not only the endpoint.

## Verification

```bash
cargo test -p tape --test spec_generated_clients --test spec_route_parity
```

The first target generates all three clients and checks their route scope.
The second checks that the served routes, the spec inventory, and the
committed `openapi.json` agree byte for byte.

## Supporting documents

- [README.md](../README.md) for the product workflow and capabilities.
- [STATUS.md](../STATUS.md) for the support state of each route family.
- [Deployment handoff](../docs/deployment-handoff.md) for serve flags, ports,
  and environment.
