//! Unified HTTP wire-level types shared by `httpkit`'s server and client sides.
//!
//! # Why this module exists
//!
//! Historically httpkit had three parallel HTTP shapes:
//!
//! - `mambalibs_http::request_response::{Request, Response, Cookie}` — Python-facing,
//!   schema-driven codegen, simple primitives. Designed before mamba was
//!   strong-typed; it lifted pydantic-style validators into Rust so CPython
//!   could call them at native speed.
//! - `mambalibs_http::client::{RequestBuilder, HttpResponse}` — rich client-side
//!   builder with enum body / auth.
//! - `mambalibs_http::protocol::Native*Head` — internal streaming-server primitives.
//!
//! With mamba being strongly-typed and capable of running Python validators
//! at native speed, the schema-driven Rust layer no longer has a job to do —
//! payload validation lives in user-written Python compiled by mamba
//! (forthcoming `httpkit.BaseModel` + `Field`), so httpkit's Rust side only
//! needs one expressive wire type pair.
//!
//! This module is that pair. `request_response::*` is kept as a back-compat
//! shim for older generated request/response bindings and will be retired once
//! the mamba-native dataclass/schema layer ships.

pub mod auth;
pub mod body;
pub mod cookie;
pub mod request;
pub mod response;

pub use auth::Auth;
pub use body::{MultipartField, RequestBody};
pub use cookie::Cookie;
pub use request::Request;
pub use response::Response;

// Re-export shared HTTP scalars hosted in mamba-core.
pub use mamba_core::http::{HttpMethod, HttpRequestLike, HttpResponseLike, HttpStatus};
