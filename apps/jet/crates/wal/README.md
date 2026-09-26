# Cclab Wal

## Brief

Cclab Wal is the shared Rust write-ahead-log implementation for cclab storage
engines.

It owns the reusable WAL entry/header format, CRC-backed corruption detection,
buffered append and fsync behavior, file rotation, typed replay, WAL discovery,
and old-segment cleanup helpers. The public surface is a Rust library API used
by higher-level storage crates.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Entry Format And Corruption Detection | - | WAL header/version, JSON entry encoding, length prefix, checksum, and corruption errors |
| Durable Writer And Rotation | - | append, buffered flush/fsync, position tracking, flush interval, and file rotation |
| Replay Reader And File Retention | - | replay reader, iterator, WAL file discovery, and cleanup helpers |

### Entry Format And Corruption Detection

Cclab Wal defines a reusable WAL file and entry format with versioned headers,
JSON-encoded typed operations, length prefixes, CRC32 checksums, and explicit
corruption errors.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `jet_wal::{WalEntry, WalHeader, WalError, Result}`
- Gate — behavior: `cargo test -p jet-wal` - header roundtrip, entry
  roundtrip, checksum validation, invalid magic/version/corruption errors
- Gate: `cargo test -p jet-wal`
- Evidence: `cargo test -p jet-wal`

### Durable Writer And Rotation

Cclab Wal writes typed storage operations to durable WAL files with buffered
appends, explicit flush/fsync control, position tracking, flush interval
checks, and size-based rotation.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `jet_wal::{WalWriter, WalConfig}`
- Gate — behavior: `cargo test -p jet-wal` - writer creation, append
  position, flush/fsync state, rotation, and file creation
- Gate: `cargo test -p jet-wal`
- Evidence: `cargo test -p jet-wal`

### Replay Reader And File Retention

Cclab Wal replays typed operations from WAL files through pull and iterator
APIs, discovers WAL files in timestamp order, and exposes cleanup helpers for
old WAL segments.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API:
  `jet_wal::{WalReader, find_wal_files, cleanup_old_wal_files}`
- Gate — behavior: `cargo test -p jet-wal` - empty replay, multi-entry
  replay, iterator replay, WAL file discovery, and retention cleanup helpers
- Gate: `cargo test -p jet-wal`
- Evidence: `cargo test -p jet-wal`
