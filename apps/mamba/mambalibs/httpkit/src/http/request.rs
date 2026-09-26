//! Unified HTTP request type.
//!
//! Used on both the server side (router fills `path_params`, handler reads
//! `headers`/`body`/`cookies`) and the client side (builder methods produce a
//! `Request` that the client transport serializes onto the wire).
//!
//! On the server side `auth` / `timeout` carry the default `Auth::None` and
//! `None`; on the client side `path_params` is the empty map. Sharing one
//! struct keeps the FastAPI-like "Request 兩向使用" surface and lets
//! middleware operate on the same shape regardless of direction.

use super::auth::Auth;
use super::body::{MultipartField, RequestBody};
use super::cookie::Cookie;
use mamba_core::http::HttpMethod;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;

use crate::client::error::{HttpError, HttpResult};

#[derive(Debug, Clone)]
pub struct Request {
    pub method: HttpMethod,
    /// Absolute URL on the client side, path-only on the server side.
    pub url: String,
    pub headers: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
    pub cookies: Vec<Cookie>,
    pub body: RequestBody,
    /// Populated by the router on the server side; empty on the client side.
    pub path_params: HashMap<String, String>,
    /// Client-side only; ignored on the server side.
    pub auth: Auth,
    /// Per-request timeout override (client-side).
    pub timeout: Option<Duration>,
}

impl Request {
    pub fn new(method: HttpMethod, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: HashMap::new(),
            query_params: HashMap::new(),
            cookies: Vec::new(),
            body: RequestBody::None,
            path_params: HashMap::new(),
            auth: Auth::None,
            timeout: None,
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Get, url)
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Post, url)
    }

    pub fn put(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Put, url)
    }

    pub fn patch(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Patch, url)
    }

    pub fn delete(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Delete, url)
    }

    pub fn head(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Head, url)
    }

    pub fn options(url: impl Into<String>) -> Self {
        Self::new(HttpMethod::Options, url)
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    pub fn headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers.extend(headers);
        self
    }

    pub fn query(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.query_params.insert(name.into(), value.into());
        self
    }

    pub fn query_params(mut self, params: HashMap<String, String>) -> Self {
        self.query_params.extend(params);
        self
    }

    pub fn cookie(mut self, cookie: Cookie) -> Self {
        self.cookies.push(cookie);
        self
    }

    pub fn json<T: Serialize>(mut self, body: &T) -> HttpResult<Self> {
        let value = serde_json::to_value(body)
            .map_err(|e| HttpError::Json(format!("Failed to serialize JSON: {}", e)))?;
        self.body = RequestBody::Json(value);
        self.headers
            .insert("Content-Type".to_string(), "application/json".to_string());
        Ok(self)
    }

    pub fn json_value(mut self, body: serde_json::Value) -> Self {
        self.body = RequestBody::Json(body);
        self.headers
            .insert("Content-Type".to_string(), "application/json".to_string());
        self
    }

    pub fn form(mut self, data: HashMap<String, String>) -> Self {
        self.body = RequestBody::Form(data);
        self.headers.insert(
            "Content-Type".to_string(),
            "application/x-www-form-urlencoded".to_string(),
        );
        self
    }

    pub fn multipart(mut self, fields: Vec<MultipartField>) -> Self {
        self.body = RequestBody::Multipart(fields);
        self
    }

    pub fn bytes(mut self, data: Vec<u8>) -> Self {
        self.body = RequestBody::Bytes(data);
        self
    }

    pub fn text(mut self, data: impl Into<String>) -> Self {
        self.body = RequestBody::Text(data.into());
        self.headers
            .insert("Content-Type".to_string(), "text/plain".to_string());
        self
    }

    pub fn basic_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.auth = Auth::Basic {
            username: username.into(),
            password: password.into(),
        };
        self
    }

    pub fn bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.auth = Auth::Bearer(token.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn timeout_secs(mut self, secs: f64) -> Self {
        self.timeout = Some(Duration::from_secs_f64(secs));
        self
    }
}
