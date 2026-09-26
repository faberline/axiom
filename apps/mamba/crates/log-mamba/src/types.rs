//! Opaque types for the `log-mamba` FFI layer.

/// A Mamba-visible logger handle.
///
/// Stores the logger name; log calls use `eprintln!` with JSON-structured output.
#[derive(Debug, Clone)]
pub struct MbLogger {
    /// Logger name (e.g. module or component name).
    pub name: String,
}

impl MbLogger {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
