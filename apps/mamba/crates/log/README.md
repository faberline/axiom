# Cclab Log

## Brief

Cclab Log is the Rust structured-logging API surface for cclab crates.

It owns the logger facade, context binding, level filtering, sink contract,
file append delivery, UDP/TCP network delivery, and logging error types. The
current verification level is API behavior smoke. File rotation and retention
are still implementation gaps, so this README records the crate as
not production-ready logging behavior.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Structured Logger API | - | Logger API with context binding, level filtering, and sink fan-out smoke proof |
| Sink Contract And Configuration | - | Sink trait, console/file/network delivery, and configuration smoke proof; rotation/retention remain gaps |
| Log Error Contract | - | shared logging error enum and result alias smoke proof |

### Structured Logger API

Cclab Log exposes a structured logger API with bound context, level filtering,
convenience level methods, and fan-out to registered sinks.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `mamba_log::Logger`, `BoundLogger`, level methods,
  context binding
- Gate — behavior: `cargo test -p mamba-log` - logger context binding, level
  filtering, and sink fan-out behavior smoke
- Gate: `cargo test -p mamba-log`
- Evidence: `cargo test -p mamba-log`; apps/mamba/crates/log/src/logger.rs

### Sink Contract And Configuration

Cclab Log defines the sink contract and configuration types for console, file,
and network logging, with smoke-verified console formatting, file append, and
UDP/TCP network write behavior. File rotation and retention remain production
gaps.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `Sink`, `LogRecord`, `LogLevel`, `ConsoleSink`,
  `FileSink`, `NetworkSink`, `SinkConfig`
- Gate — behavior: `cargo test -p mamba-log` - sink config, level parsing, file
  append, UDP/TCP network write, and flush behavior smoke
- Gate: `cargo test -p mamba-log`
- Evidence: `cargo test -p mamba-log`; apps/mamba/crates/log/src/sink.rs

### Log Error Contract

Cclab Log provides a shared logging error contract for invalid levels, sink
failures, format failures, rotation failures, and I/O errors.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `LogError`, `Result`
- Gate — behavior: `cargo test -p mamba-log` - typed error display and I/O
  conversion behavior smoke
- Gate: `cargo test -p mamba-log`
- Evidence: `cargo test -p mamba-log`; apps/mamba/crates/log/src/error.rs
