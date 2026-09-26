# Cclab Mamba Registry

## Brief

Cclab Mamba Registry is the Rust registry and ABI bridge for native Mamba
modules.

It lets binding crates self-register importable Mamba modules, share the
`MbValue` conversion boundary, and call runtime-provided object, exception,
HTTP status, and async helpers without depending on private Mamba layouts.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Native Module Auto Registration | - | link-time registration surface for native Mamba modules |
| MbValue Conversion ABI | - | NaN-boxed Mamba value ABI plus Rust conversion traits and native handles |
| Runtime Bridge Helpers | - | object ops, exception helpers, HTTP status table, and shared tokio runtime bridge |

### Native Module Auto Registration

Cclab Mamba Registry lets Rust binding crates self-register native Mamba
modules, exported symbols, and module values at link time so the Mamba runtime
can discover importable native modules without a hand-maintained central table.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `MambaModule`, `ModuleRegistrar`, `RuntimeSymbol`,
  `RuntimeValue`, `MAMBA_MODULES`, `rt_sym!`, `all_modules`, `find_module`
- Gate — behavior: `cargo test -p mamba-registry` - module slice,
  registrar, symbol, and integration smoke behavior
- Gate: `cargo test -p mamba-registry`
- Evidence: `cargo test -p mamba-registry`

### MbValue Conversion ABI

Cclab Mamba Registry provides the shared `MbValue` ABI and conversion traits
that let Rust binding crates move primitive values, collections, optional
values, strings, and opaque native handles across the Mamba runtime boundary.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `MbValue`, `FromMbValue`, `IntoMbValue`, `MbConvError`,
  native wrapping helpers
- Gate — behavior: `cargo test -p mamba-registry` - scalar roundtrip,
  conversion, overflow, collection, option, and native handle behavior
- Gate: `cargo test -p mamba-registry`
- Evidence: `cargo test -p mamba-registry`

### Runtime Bridge Helpers

Cclab Mamba Registry gives binding crates a shared runtime bridge for object
operations, exception propagation, canonical HTTP status metadata, and
process-wide async execution without depending on private Mamba runtime
layouts.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `ObjectOps`, `set_object_ops`, `ops`, `raise_*`,
  `http::status_phrase`, `runtime::handle`
- Gate — behavior: `cargo test -p mamba-registry` - ops table, exception,
  HTTP status, and runtime helper behavior
- Gate: `cargo test -p mamba-registry`
- Evidence: `cargo test -p mamba-registry`
