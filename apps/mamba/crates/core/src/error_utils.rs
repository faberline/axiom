//! Error sanitization utilities shared across cclab crates
//!
//! This module provides common utilities for error handling and sanitization
//! that are used by multiple crates.
//!
//! # Security Features
//! - Removes connection strings from error messages
//! - Redacts credentials and authentication details
//! - Categorizes errors for better handling

use regex::Regex;
use std::sync::OnceLock;

/// Error categories for better error handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Connection-related errors (network, DNS, etc.)
    Connection,
    /// Authentication/authorization errors
    Authentication,
    /// Timeout errors
    Timeout,
    /// Validation errors (invalid input)
    Validation,
    /// Query/operation errors
    Operation,
    /// Unknown/uncategorized errors
    Unknown,
}

// ============================================================================
// Regex patterns (compiled once)
// ============================================================================

/// Regex for matching MongoDB connection strings
fn connection_string_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"mongodb(\+srv)?://[^\s]+").expect("valid regex"))
}

/// Regex for matching credentials in URLs
fn credentials_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"://[^:]+:[^@]+@").expect("valid regex"))
}

/// Regex for matching IP addresses
fn ip_address_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(:\d+)?\b").expect("valid regex")
    })
}

/// Regex for matching URL credentials (user:password@)
fn url_creds_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"https?://[^@:]+:[^@]+@").expect("valid regex"))
}

/// Regex for matching internal IP addresses
fn internal_ip_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\b(10\.|172\.(1[6-9]|2[0-9]|3[01])\.|192\.168\.)\d+\.\d+\b")
            .expect("valid regex")
    })
}

/// Regex for matching auth headers and tokens
fn auth_header_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)(authorization:\s*bearer|bearer|api[_-]?key|token)\s*[:=]?\s*\S+")
            .expect("valid regex")
    })
}

// ============================================================================
// Error Sanitization
// ============================================================================

/// Sanitizes an error message by removing sensitive information.
///
/// # Arguments
/// * `error_msg` - The original error message
/// * `debug_mode` - If true, returns the original message (for development)
///
/// # Returns
/// Sanitized error message safe for production logging
///
/// # Security
/// Removes:
/// - Connection strings (mongodb://..., mongodb+srv://..., https://...)
/// - Credentials (username:password)
/// - IP addresses and ports
/// - Authentication tokens and headers
///
/// # Example
/// ```
/// use mamba_core::error_utils::sanitize_error;
///
/// let error = "Failed to connect to mongodb://user:pass@localhost:27017/mydb";
/// let sanitized = sanitize_error(error, false);
/// assert!(!sanitized.contains("user:pass"));
/// assert!(!sanitized.contains("localhost"));
/// ```
pub fn sanitize_error(error_msg: &str, debug_mode: bool) -> String {
    if debug_mode {
        return error_msg.to_string();
    }

    let mut sanitized = error_msg.to_string();

    // Remove MongoDB connection strings
    sanitized = connection_string_regex()
        .replace_all(&sanitized, "[CONNECTION_STRING_REDACTED]")
        .to_string();

    // Remove URL credentials
    sanitized = url_creds_regex()
        .replace_all(&sanitized, "https://[REDACTED]@")
        .to_string();

    // Remove credentials from URLs
    sanitized = credentials_regex()
        .replace_all(&sanitized, "://[CREDENTIALS_REDACTED]@")
        .to_string();

    // Remove internal IP addresses
    sanitized = internal_ip_regex()
        .replace_all(&sanitized, "[INTERNAL_IP]")
        .to_string();

    // Remove regular IP addresses
    sanitized = ip_address_regex()
        .replace_all(&sanitized, "[IP_REDACTED]")
        .to_string();

    // Remove auth headers and tokens
    sanitized = auth_header_regex()
        .replace_all(&sanitized, "$1: [REDACTED]")
        .to_string();

    // Remove common sensitive patterns
    sanitized = sanitized
        .replace("username", "[USERNAME_REDACTED]")
        .replace("password", "[PASSWORD_REDACTED]")
        .replace("auth", "[AUTH_REDACTED]");

    sanitized
}

/// Convenience wrapper that always sanitizes (production mode).
///
/// Use this when you don't have access to a config or always want sanitization.
pub fn sanitize_error_message(error_msg: &str) -> String {
    sanitize_error(error_msg, false)
}

// ============================================================================
// Error Categorization
// ============================================================================

/// Categorizes an error based on its message.
///
/// # Arguments
/// * `error_msg` - The error message to categorize
///
/// # Returns
/// ErrorCategory indicating the type of error
pub fn categorize_error(error_msg: &str) -> ErrorCategory {
    let lowercase = error_msg.to_lowercase();

    if lowercase.contains("connection")
        || lowercase.contains("network")
        || lowercase.contains("dns")
    {
        ErrorCategory::Connection
    } else if lowercase.contains("auth")
        || lowercase.contains("unauthorized")
        || lowercase.contains("permission")
    {
        ErrorCategory::Authentication
    } else if lowercase.contains("timeout") || lowercase.contains("timed out") {
        ErrorCategory::Timeout
    } else if lowercase.contains("invalid") || lowercase.contains("validation") {
        ErrorCategory::Validation
    } else if lowercase.contains("query") || lowercase.contains("operation") {
        ErrorCategory::Operation
    } else {
        ErrorCategory::Unknown
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_mongodb_connection_string() {
        let error = "Failed to connect to mongodb://user:pass@localhost:27017/mydb";
        let sanitized = sanitize_error(error, false);
        assert!(!sanitized.contains("mongodb://"));
        assert!(!sanitized.contains("user:pass"));
        assert!(sanitized.contains("[CONNECTION_STRING_REDACTED]"));
    }

    #[test]
    fn test_sanitize_https_credentials() {
        let error = "Connection failed to https://user:password@api.example.com/path";
        let sanitized = sanitize_error(error, false);
        assert!(!sanitized.contains("password"));
        assert!(sanitized.contains("[REDACTED]"));
    }

    #[test]
    fn test_sanitize_internal_ip() {
        let error = "Cannot connect to 192.168.1.100:8080";
        let sanitized = sanitize_error(error, false);
        assert!(sanitized.contains("[INTERNAL_IP]") || sanitized.contains("[IP_REDACTED]"));
        assert!(!sanitized.contains("192.168.1.100"));
    }

    #[test]
    fn test_sanitize_auth_header() {
        let error = "Request failed with Authorization: Bearer secret-token-123";
        let sanitized = sanitize_error(error, false);
        assert!(!sanitized.contains("secret-token-123"));
        assert!(sanitized.contains("[REDACTED]"));
    }

    #[test]
    fn test_debug_mode_preserves_details() {
        let error = "Failed to connect to mongodb://user:pass@localhost:27017/mydb";
        let sanitized = sanitize_error(error, true);
        assert_eq!(sanitized, error);
    }

    #[test]
    fn test_categorize_connection_error() {
        assert_eq!(
            categorize_error("Connection refused"),
            ErrorCategory::Connection
        );
        assert_eq!(
            categorize_error("Network timeout"),
            ErrorCategory::Connection
        );
        assert_eq!(
            categorize_error("DNS resolution failed"),
            ErrorCategory::Connection
        );
    }

    #[test]
    fn test_categorize_auth_error() {
        assert_eq!(
            categorize_error("Authentication failed"),
            ErrorCategory::Authentication
        );
        assert_eq!(
            categorize_error("Unauthorized access"),
            ErrorCategory::Authentication
        );
    }

    #[test]
    fn test_categorize_timeout_error() {
        assert_eq!(
            categorize_error("Operation timed out"),
            ErrorCategory::Timeout
        );
    }

    #[test]
    fn test_categorize_validation_error() {
        assert_eq!(
            categorize_error("Invalid field name"),
            ErrorCategory::Validation
        );
    }

    #[test]
    fn test_categorize_unknown_error() {
        assert_eq!(
            categorize_error("Something went wrong"),
            ErrorCategory::Unknown
        );
    }
}
