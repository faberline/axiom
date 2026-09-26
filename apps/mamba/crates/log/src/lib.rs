//! # mamba-log
//!
//! Rust-powered structured logging for Python, replacing loguru.
//! Uses tracing + tokio for async file I/O with multiple sinks.

pub mod error;
pub mod logger;
pub mod sink;

pub use error::LogError;
pub use logger::Logger;
pub use sink::{ConsoleSink, FileSink, NetworkSink, Sink, SinkConfig};
