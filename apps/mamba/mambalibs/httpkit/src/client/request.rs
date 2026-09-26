//! Client-side request adapter.
//!
//! The user-facing builder type lives in `mambalibs_http::http` (see `Request`); this
//! module converts that shape into the middleware/reqwest-facing form
//! (`ExtractedRequest`) where header order is preserved and the body is in a
//! flat representation reqwest can consume directly.

pub use crate::http::{Auth, MultipartField, Request, RequestBody};
pub use mamba_core::http::HttpMethod;

use super::error::{HttpError, HttpResult};
use std::time::Duration;

/// Back-compat alias for the user-facing builder. New code should prefer
/// `mambalibs_http::http::Request`, which IS the builder — chainable `.header`,
/// `.json`, `.bearer_auth`, ... methods live directly on it. The alias keeps
/// existing `mambalibs_http::client::RequestBuilder` imports compiling without
/// touching agentkit / cclab-qc.
pub type RequestBuilder = Request;

/// Convert the typed `HttpMethod` into reqwest's stringly method type.
fn to_reqwest_method(method: HttpMethod) -> reqwest::Method {
    match method {
        HttpMethod::Get => reqwest::Method::GET,
        HttpMethod::Post => reqwest::Method::POST,
        HttpMethod::Put => reqwest::Method::PUT,
        HttpMethod::Patch => reqwest::Method::PATCH,
        HttpMethod::Delete => reqwest::Method::DELETE,
        HttpMethod::Head => reqwest::Method::HEAD,
        HttpMethod::Options => reqwest::Method::OPTIONS,
    }
}

/// Internal, flat request shape used between middleware and the reqwest
/// transport. Header order is preserved (Vec instead of HashMap) so
/// middlewares appending auth/tracing headers behave predictably.
#[derive(Debug, Clone)]
pub struct ExtractedRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub query_params: Vec<(String, String)>,
    pub body: ExtractedBody,
    pub auth: ExtractedAuth,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum ExtractedBody {
    None,
    Json(serde_json::Value),
    Form(Vec<(String, String)>),
    Multipart(Vec<MultipartField>),
    Bytes(Vec<u8>),
    Text(String),
}

#[derive(Debug, Clone)]
pub enum ExtractedAuth {
    None,
    Basic { username: String, password: String },
    Bearer(String),
}

impl From<Request> for ExtractedRequest {
    fn from(req: Request) -> Self {
        let mut headers: Vec<(String, String)> = req.headers.into_iter().collect();

        // Serialize cookies into a single Cookie header on the client side
        // (server-side handlers fill `cookies` for the dispatcher; here we
        // only emit them when the caller built them via Request::cookie).
        if !req.cookies.is_empty() {
            let joined = req
                .cookies
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; ");
            headers.push(("cookie".to_string(), joined));
        }

        Self {
            method: req.method,
            url: req.url,
            headers,
            query_params: req.query_params.into_iter().collect(),
            body: match req.body {
                RequestBody::None => ExtractedBody::None,
                RequestBody::Json(v) => ExtractedBody::Json(v),
                RequestBody::Form(m) => ExtractedBody::Form(m.into_iter().collect()),
                RequestBody::Multipart(m) => ExtractedBody::Multipart(m),
                RequestBody::Bytes(b) => ExtractedBody::Bytes(b),
                RequestBody::Text(t) => ExtractedBody::Text(t),
            },
            auth: match req.auth {
                Auth::None => ExtractedAuth::None,
                Auth::Basic { username, password } => ExtractedAuth::Basic { username, password },
                Auth::Bearer(t) => ExtractedAuth::Bearer(t),
            },
            timeout_ms: req.timeout.map(|d| d.as_millis() as u64),
        }
    }
}

impl ExtractedRequest {
    /// Build a reqwest `RequestBuilder` from the extracted shape.
    pub fn build_reqwest(
        &self,
        client: &reqwest::Client,
        base_url: Option<&str>,
    ) -> HttpResult<reqwest::RequestBuilder> {
        let full_url = if let Some(base) = base_url {
            if self.url.starts_with("http://") || self.url.starts_with("https://") {
                self.url.clone()
            } else {
                let base = base.trim_end_matches('/');
                let path = if self.url.starts_with('/') {
                    self.url.clone()
                } else {
                    format!("/{}", self.url)
                };
                format!("{}{}", base, path)
            }
        } else {
            self.url.clone()
        };

        let mut builder = client.request(to_reqwest_method(self.method), &full_url);

        for (name, value) in &self.headers {
            builder = builder.header(name, value);
        }

        if !self.query_params.is_empty() {
            builder = builder.query(&self.query_params);
        }

        builder = match &self.body {
            ExtractedBody::None => builder,
            ExtractedBody::Json(value) => builder.json(value),
            ExtractedBody::Form(data) => {
                let form_data: Vec<(&str, &str)> =
                    data.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                builder.form(&form_data)
            }
            ExtractedBody::Multipart(fields) => {
                let mut form = reqwest::multipart::Form::new();
                for field in fields {
                    let mut part = reqwest::multipart::Part::bytes(field.data.clone());
                    if let Some(filename) = &field.filename {
                        part = part.file_name(filename.clone());
                    }
                    if let Some(mime) = &field.mime_type {
                        part = part.mime_str(mime).map_err(|e| {
                            HttpError::InvalidRequest(format!("invalid MIME type: {e}"))
                        })?;
                    }
                    form = form.part(field.name.clone(), part);
                }
                builder.multipart(form)
            }
            ExtractedBody::Bytes(data) => builder.body(data.clone()),
            ExtractedBody::Text(data) => builder.body(data.clone()),
        };

        builder = match &self.auth {
            ExtractedAuth::None => builder,
            ExtractedAuth::Basic { username, password } => {
                builder.basic_auth(username, Some(password))
            }
            ExtractedAuth::Bearer(token) => builder.bearer_auth(token),
        };

        if let Some(timeout_ms) = self.timeout_ms {
            builder = builder.timeout(Duration::from_millis(timeout_ms));
        }

        Ok(builder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_http_method_from_str() {
        assert_eq!(HttpMethod::from_str("GET").unwrap(), HttpMethod::Get);
        assert_eq!(HttpMethod::from_str("post").unwrap(), HttpMethod::Post);
        assert!(HttpMethod::from_str("INVALID").is_err());
    }

    #[test]
    fn test_request_builder_via_alias() {
        let request = RequestBuilder::new(HttpMethod::Post, "/api/users")
            .header("X-Custom", "value")
            .query("page", "1")
            .json(&serde_json::json!({"name": "Alice"}))
            .unwrap()
            .bearer_auth("token123");

        assert_eq!(request.method, HttpMethod::Post);
        assert_eq!(request.url, "/api/users");
        assert!(request.headers.contains_key("X-Custom"));
        assert!(request.headers.contains_key("Content-Type"));
        assert!(matches!(request.auth, Auth::Bearer(_)));
    }
}
