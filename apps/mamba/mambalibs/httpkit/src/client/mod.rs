//! `mambalibs_http::client` — high-performance async HTTP client.
//!
//! Connection-pooled async HTTP client with automatic latency measurement,
//! request building, and middleware. The native API toolkit owns both
//! server-side route registration and client-side request execution.
//!
//! Mamba bindings are exposed through the sibling `mambalibs-http-binding`
//! crate under `mambalibs.http`.

pub mod client;
pub mod config;
pub mod error;
pub mod middleware;
pub mod request;
pub mod response;

pub use client::HttpClient;
pub use config::{HttpClientConfig, HttpProtocolPolicy};
pub use error::{HttpError, HttpResult};
pub use middleware::{
    AuthMiddleware, LoggingMiddleware, Middleware, MiddlewareChain, MiddlewareFuture,
    MiddlewareResult, Next, RetryConfig, RetryMiddleware,
};
// `RequestBuilder` is now a type alias for the unified `mambalibs_http::http::Request`;
// `HttpResponse` for `mambalibs_http::http::Response`. Old imports still work.
pub use request::{HttpMethod, RequestBuilder};
pub use response::HttpResponse;

pub use mamba_core::http::{HttpRequestLike, HttpResponseLike, HttpStatus};
