# Client Transport Policy Example

This repo-root example proves the shared client transport policy with real
components:

- `lumen`, a [faberline/lumen](https://github.com/faberline/lumen) git
  dependency, supplies its actual OpenAPI document.
- A real Lumen router is served over the shared HTTP/1.1 + h2c server path.
- `core/openapi-codegen` generates TypeScript, Python, and Rust clients.
- `core/transport-h2c` supplies the logarithmic HTTP/2 connection-count heuristic.

Run it:

```bash
cargo run -p axiom-client-transport-policy-example
cargo test -p axiom-client-transport-policy-example
```

The tests intentionally run generated clients against a real Lumen server
instead of a toy fixture, so transport-policy drift in the shared libraries or
Lumen's integration surface is visible from one root-level command.
