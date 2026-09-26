# Cclab Core

## Brief

Cclab Core is the small shared Rust support crate for ecosystem-wide error,
HTTP, and utility contracts.

It owns the common `DataBridgeError` / `Result` surface, production-safe error
sanitization helpers, and lightweight HTTP request/response abstractions used
by higher-level cclab crates.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Unified Error Contract | - | shared `DataBridgeError` and `Result` contract for cclab crates |
| Sanitized Error Reporting | - | redacts connection strings, credentials, internal IPs, and tokens before production logging |
| HTTP Helper Contracts | - | shared HTTP method, status, request, and response helper traits |

### Unified Error Contract

Cclab Core gives Rust crates one shared error/result contract with stable
display text, serde conversion, retry classification, and constraint violation
classification.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `mamba_core::DataBridgeError`, `Result`, `is_retryable`,
  `is_constraint_violation`
- Gate — behavior: `cargo test -p mamba-core` - shared error display,
  conversion, retry, and constraint behavior
- Gate: `cargo test -p mamba-core`
- Evidence: `cargo test -p mamba-core`

### Sanitized Error Reporting

Cclab Core provides production-safe error sanitization and categorization so
crates can log failures without leaking connection strings, credentials,
internal IPs, or auth tokens.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `sanitize_error`, `sanitize_error_message`,
  `categorize_error`, `ErrorCategory`
- Gate — security: `cargo test -p mamba-core` - connection string, credential,
  IP, and token redaction behavior
- Gate: `cargo test -p mamba-core`
- Evidence: `cargo test -p mamba-core`

### HTTP Helper Contracts

Cclab Core provides lightweight HTTP helper types and traits that let ecosystem
crates share method parsing, status classification, header lookup, and
response/request behavior.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `HttpMethod`, `HttpStatus`, `HttpRequestLike`,
  `HttpResponseLike`
- Gate — behavior: `cargo test -p mamba-core` - HTTP method parsing, status
  classification, and trait helper behavior
- Gate: `cargo test -p mamba-core`
- Evidence: `cargo test -p mamba-core`
