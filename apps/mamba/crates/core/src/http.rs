//! Shared HTTP types for cclab ecosystem.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// HTTP request methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    /// Returns the method as a string slice.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for HttpMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" => Ok(Self::Delete),
            "HEAD" => Ok(Self::Head),
            "OPTIONS" => Ok(Self::Options),
            _ => Err(format!("Invalid HTTP method: {}", s)),
        }
    }
}

/// HTTP status code wrapper with helper methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HttpStatus(pub u16);

impl HttpStatus {
    // Common status codes
    pub const OK: Self = Self(200);
    pub const CREATED: Self = Self(201);
    pub const NO_CONTENT: Self = Self(204);
    pub const BAD_REQUEST: Self = Self(400);
    pub const UNAUTHORIZED: Self = Self(401);
    pub const FORBIDDEN: Self = Self(403);
    pub const NOT_FOUND: Self = Self(404);
    pub const METHOD_NOT_ALLOWED: Self = Self(405);
    pub const CONFLICT: Self = Self(409);
    pub const UNPROCESSABLE_ENTITY: Self = Self(422);
    pub const INTERNAL_SERVER_ERROR: Self = Self(500);
    pub const BAD_GATEWAY: Self = Self(502);
    pub const SERVICE_UNAVAILABLE: Self = Self(503);

    /// Returns the status code as u16.
    pub fn code(&self) -> u16 {
        self.0
    }

    /// Returns true if this is a success status (2xx).
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.0)
    }

    /// Returns true if this is a client error status (4xx).
    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.0)
    }

    /// Returns true if this is a server error status (5xx).
    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.0)
    }

    /// Returns true if this is a redirect status (3xx).
    pub fn is_redirect(&self) -> bool {
        (300..400).contains(&self.0)
    }

    /// Returns true if this is an informational status (1xx).
    pub fn is_informational(&self) -> bool {
        (100..200).contains(&self.0)
    }
}

impl From<u16> for HttpStatus {
    fn from(code: u16) -> Self {
        Self(code)
    }
}

impl From<HttpStatus> for u16 {
    fn from(status: HttpStatus) -> Self {
        status.0
    }
}

/// Trait for types that represent HTTP responses.
pub trait HttpResponseLike {
    /// Returns the HTTP status code.
    fn status_code(&self) -> u16;

    /// Returns the response headers.
    fn headers(&self) -> &HashMap<String, String>;

    /// Returns the response body as bytes.
    fn body_bytes(&self) -> &[u8];

    /// Returns the HTTP status.
    fn status(&self) -> HttpStatus {
        HttpStatus(self.status_code())
    }

    /// Returns true if this is a success response (2xx).
    fn is_success(&self) -> bool {
        self.status().is_success()
    }

    /// Returns true if this is a client error response (4xx).
    fn is_client_error(&self) -> bool {
        self.status().is_client_error()
    }

    /// Returns true if this is a server error response (5xx).
    fn is_server_error(&self) -> bool {
        self.status().is_server_error()
    }

    /// Gets a header value by name (case-insensitive).
    fn header(&self, name: &str) -> Option<&str> {
        self.headers().get(&name.to_lowercase()).map(|s| s.as_str())
    }

    /// Returns the Content-Type header value.
    fn content_type(&self) -> Option<&str> {
        self.header("content-type")
    }

    /// Returns the Content-Length header value.
    fn content_length(&self) -> Option<usize> {
        self.header("content-length").and_then(|s| s.parse().ok())
    }
}

/// Trait for types that represent HTTP requests.
pub trait HttpRequestLike {
    /// Returns the HTTP method.
    fn method(&self) -> HttpMethod;

    /// Returns the request URL.
    fn url(&self) -> &str;

    /// Returns the request headers.
    fn headers(&self) -> &HashMap<String, String>;

    /// Returns the request body as bytes, if present.
    fn body_bytes(&self) -> Option<&[u8]>;

    /// Gets a header value by name (case-insensitive).
    fn header(&self, name: &str) -> Option<&str> {
        self.headers().get(&name.to_lowercase()).map(|s| s.as_str())
    }

    /// Returns the Content-Type header value.
    fn content_type(&self) -> Option<&str> {
        self.header("content-type")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_method_as_str() {
        assert_eq!(HttpMethod::Get.as_str(), "GET");
        assert_eq!(HttpMethod::Post.as_str(), "POST");
        assert_eq!(HttpMethod::Put.as_str(), "PUT");
        assert_eq!(HttpMethod::Patch.as_str(), "PATCH");
        assert_eq!(HttpMethod::Delete.as_str(), "DELETE");
        assert_eq!(HttpMethod::Head.as_str(), "HEAD");
        assert_eq!(HttpMethod::Options.as_str(), "OPTIONS");
    }

    #[test]
    fn test_http_method_from_str() {
        assert_eq!(HttpMethod::from_str("GET").unwrap(), HttpMethod::Get);
        assert_eq!(HttpMethod::from_str("get").unwrap(), HttpMethod::Get);
        assert_eq!(HttpMethod::from_str("Get").unwrap(), HttpMethod::Get);
        assert_eq!(HttpMethod::from_str("POST").unwrap(), HttpMethod::Post);
        assert!(HttpMethod::from_str("INVALID").is_err());
    }

    #[test]
    fn test_http_status_helpers() {
        assert!(HttpStatus::OK.is_success());
        assert!(HttpStatus::CREATED.is_success());
        assert!(!HttpStatus::OK.is_client_error());
        assert!(!HttpStatus::OK.is_server_error());

        assert!(HttpStatus::BAD_REQUEST.is_client_error());
        assert!(HttpStatus::NOT_FOUND.is_client_error());
        assert!(!HttpStatus::BAD_REQUEST.is_success());

        assert!(HttpStatus::INTERNAL_SERVER_ERROR.is_server_error());
        assert!(!HttpStatus::INTERNAL_SERVER_ERROR.is_success());

        assert!(HttpStatus(301).is_redirect());
        assert!(HttpStatus(100).is_informational());
    }

    #[test]
    fn test_http_status_conversion() {
        let status = HttpStatus::from(404);
        assert_eq!(status.code(), 404);
        assert!(status.is_client_error());

        let code: u16 = HttpStatus::OK.into();
        assert_eq!(code, 200);
    }
}
