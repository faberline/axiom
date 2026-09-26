# Cclab Qc Mamba

## Brief

Cclab QC Mamba is the Mamba native binding for pytest-like test decorator
metadata.

It registers the `cclab.qc` module through the shared Mamba registry and
exposes native-call entrypoints for fixture metadata, a mark namespace sentinel,
raises context metadata, and parametrize metadata. The crate owns the Mamba ABI
surface for those primitives; downstream runner discovery/execution consumes
the metadata outside this binding.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Mamba QC Decorator Binding | - | exposes pytest-like fixture, mark, raises, and parametrize metadata primitives for Mamba |

### Mamba QC Decorator Binding

Cclab QC Mamba exposes pytest-like test decorator primitives to Mamba scripts
through the `cclab.qc` native module, including fixture metadata, a mark
namespace sentinel, raises context metadata, and parametrize metadata for the
downstream test runner to consume.

- Root WI: none; this capability predates the tracker.
- Surfaces: Mamba module: `cclab.qc`; Native ABI: `mb_qc_fixture`,
  `mb_qc_mark`, `mb_qc_raises`, `mb_qc_parametrize`; Rust module registrar:
  `QcMambaModule`
- Gate — behavior: `cargo test -p qc-mamba`
- Gate: `cargo test -p qc-mamba`
- Source: `apps/mamba/crates/qc-mamba/src/lib.rs`,
  `apps/mamba/crates/qc-mamba/src/methods.rs`
- Evidence: `cargo test -p qc-mamba`; apps/mamba/crates/qc-mamba/src/lib.rs;
  apps/mamba/crates/qc-mamba/src/methods.rs
