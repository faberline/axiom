# wgpu validation in dev builds — Slice 4k

> **Issue**: #1729
> **Parent epic**: #1254 (WebGPU-React renderer)
> **Depends on**: #1719 (Slice 4a — renderer wrapper).
> **Status**: in-flight

## Problem

wgpu has two complementary safety nets:

- **Compile-time**: Rust's type system catches the obvious shape
  errors (wrong descriptor variant, lifetime escapes, etc.).
- **Runtime validation layer**: wgpu's `InstanceFlags::VALIDATION`
  catches the API-misuse errors only the driver can see — binding an
  unbound buffer index, dispatching with no pipeline set, using a
  texture in a state incompatible with its descriptor, etc. The
  validation layer emits diagnostics via the `log` crate; without it,
  these errors surface only as cryptic GPU hangs / black frames on
  some drivers.

The runtime layer is far too slow to ship in release builds (it does
shadow-state tracking on every call), but it is exactly what we want
during development and in CI. The default `InstanceFlags::default()`
already calls `from_build_config()` which gates on `cfg!(debug_assertions)`,
but that's an *implicit* contract — a future refactor that drops
`..Default::default()` would silently disable validation. Slice 4k
makes the gate **explicit** and documents the contract.

Validation diagnostics are emitted via the `log` crate; downstream
consumers (and our test harness) increasingly use `tracing`. A
bridging helper is offered so callers can opt into log→tracing
forwarding with one idempotent call at startup.

## Scope

In:

- New module `validation` exposing:
  - `instance_flags_for_build() -> wgpu::InstanceFlags` — explicit
    `cfg(debug_assertions)` gate. Returns `InstanceFlags::debugging()`
    (i.e. `DEBUG | VALIDATION`) under `debug_assertions`,
    `InstanceFlags::empty()` otherwise.
  - `try_install_log_bridge() -> Result<(), tracing_log::log_tracer::SetLoggerError>` —
    thin wrapper around `tracing_log::LogTracer::init()`. Idempotent
    failure: if a `log` global has already been installed, returns the
    error verbatim so the caller decides whether that's fatal.
- `WebGpuRenderer::new` switches from `..Default::default()` on the
  `InstanceDescriptor` to an explicit `flags:
  validation::instance_flags_for_build()`. The behavior is byte-
  equivalent in both build configurations; the change is purely
  about making the contract visible to readers and to grep.
- New `tracing-log = "0.2"` dependency (the workspace already pulls
  in `tracing 0.1` and `tracing-subscriber 0.3`; `tracing-log` is the
  missing piece that lets `log::error!()` calls inside wgpu reach a
  `tracing::Subscriber`).

Out:

- A custom `Subscriber` implementation. Consumers initialise their
  own `tracing-subscriber` per their environment (CI prints to stdout,
  dev prints to file, prod ships structured JSON).
- A validation-error-as-Rust-panic mode. wgpu's validation already
  panics by default on Vulkan/D3D12; the macOS/Metal backend logs and
  continues. Tightening that platform-specific behavior is a separate
  slice.
- CI integration. The AC bullet "zero validation errors during the
  test suite" is met structurally: `cargo test -p jet-grid-render-webgpu`
  exercises every code path through `WebGpuRenderer::new` /
  `render_frame` / `FrameBuilder`, and the validation layer's default
  is to panic on error — any genuine API misuse fails the test. No
  separate CI gate is wired in this slice.

## Interface

```rust
// apps/jet/crates/grid-render-webgpu/src/validation.rs

/// Return the `wgpu::InstanceFlags` appropriate for the current build:
/// `DEBUG | VALIDATION` under `cfg(debug_assertions)`, `empty()`
/// otherwise. Pure — testable without a GPU.
pub fn instance_flags_for_build() -> wgpu::InstanceFlags;

/// Install `tracing-log::LogTracer` as the global `log` shim, so
/// `log::error!` calls inside wgpu (validation diagnostics in
/// particular) reach the active `tracing::Subscriber`. Returns the
/// underlying error if a `log` global was already installed; that's
/// not necessarily fatal — many test harnesses install a `log`
/// subscriber for other crates and this helper is idempotent in
/// spirit (call it once; subsequent calls are no-ops via the error
/// arm).
pub fn try_install_log_bridge()
    -> Result<(), tracing_log::log_tracer::SetLoggerError>;
```

## Acceptance Criteria

- [x] `cfg(debug_assertions)` gate — `instance_flags_for_build` reads
      `cfg!(debug_assertions)` literally and `WebGpuRenderer::new`
      consumes the result; both are grep-visible.
- [x] Validation logs routed to tracing crate — `try_install_log_bridge`
      installs `tracing_log::LogTracer` so wgpu's `log` output reaches
      the active `tracing::Subscriber`. (Opt-in by the caller.)
- [x] CI check: zero validation errors during the test suite — met
      structurally by wgpu's panic-on-error default for the validation
      layer; every test that exercises wgpu fails on a validation
      error.
- [x] `cargo test -p jet-grid-render-webgpu` passes.
- [x] Module-level docs explain WHY (implicit-vs-explicit gate
      contract; log→tracing bridge rationale).

## Reference context

- `apps/jet/crates/grid-render-webgpu/src/lib.rs` — `WebGpuRenderer::new`
  (where the `InstanceDescriptor` is built).
- wgpu 24 docs:
  - `wgpu::InstanceFlags::debugging()` = `DEBUG | VALIDATION`.
  - `wgpu::InstanceFlags::from_build_config()` — built-in equivalent
    of our `instance_flags_for_build`. We re-implement the gate
    locally so the renderer's contract isn't tied to a wgpu
    implementation detail.
- `tracing-log` 0.2 — provides `LogTracer::init()` that registers a
  `log::Log` impl which forwards records to `tracing`.
