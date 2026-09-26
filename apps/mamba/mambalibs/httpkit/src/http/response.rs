//! Unified HTTP response type.
//!
//! Server handlers construct one; clients receive one. Client-side metadata
//! (`latency_ms`, `final_url`, `version`) defaults to zero/empty on
//! server-constructed responses.

use super::cookie::Cookie;
use mamba_core::http::{HttpResponseLike, HttpStatus};
use std::collections::HashMap;
use std::time::Duration;

use crate::client::error::{HttpError, HttpResult};

#[derive(Debug, Clone)]
pub struct Response {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub cookies: Vec<Cookie>,
    /// Content-Type the server is announcing. Mirrors the `content-type`
    /// header but kept as a separate field for the server-builder ergonomics
    /// (HTMLResponse / JSONResponse / ...).
    pub media_type: String,

    // Client-side metrics (zero on server-constructed Response).
    pub latency_ms: u64,
    pub final_url: String,
    pub version: String,
}

impl Response {
    pub fn new(status_code: u16) -> Self {
        Self {
            status_code,
            headers: HashMap::new(),
            body: Vec::new(),
            cookies: Vec::new(),
            media_type: String::new(),
            latency_ms: 0,
            final_url: String::new(),
            version: String::new(),
        }
    }

    pub fn status(&self) -> HttpStatus {
        HttpStatus(self.status_code)
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.status_code)
    }

    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.status_code)
    }

    pub fn is_redirect(&self) -> bool {
        (300..400).contains(&self.status_code)
    }

    pub fn text(&self) -> HttpResult<String> {
        String::from_utf8(self.body.clone())
            .map_err(|e| HttpError::ResponseError(format!("Invalid UTF-8 in response: {}", e)))
    }

    pub fn json(&self) -> HttpResult<serde_json::Value> {
        serde_json::from_slice(&self.body)
            .map_err(|e| HttpError::Json(format!("Failed to parse JSON: {}", e)))
    }

    pub fn json_as<T: serde::de::DeserializeOwned>(&self) -> HttpResult<T> {
        serde_json::from_slice(&self.body)
            .map_err(|e| HttpError::Json(format!("Failed to deserialize JSON: {}", e)))
    }

    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    pub fn content_length(&self) -> usize {
        self.body.len()
    }

    pub fn latency(&self) -> Duration {
        Duration::from_millis(self.latency_ms)
    }

    pub fn protocol_version(&self) -> &str {
        &self.version
    }

    pub fn is_http2(&self) -> bool {
        self.version == "HTTP/2"
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        let lower = name.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == lower)
            .map(|(_, v)| v.as_str())
    }

    pub fn content_type(&self) -> Option<&str> {
        self.header("content-type")
    }

    pub fn is_json(&self) -> bool {
        self.content_type()
            .map(|ct| ct.contains("application/json"))
            .unwrap_or(false)
    }
}

impl HttpResponseLike for Response {
    fn status_code(&self) -> u16 {
        self.status_code
    }

    fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    fn body_bytes(&self) -> &[u8] {
        &self.body
    }
}
