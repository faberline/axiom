//! Format validation for common string patterns
//!
//! This module provides pre-compiled regex validators for common formats like
//! email, URL, UUID, IPv4/IPv6, hostname, phone, base64, slug, and date/time formats.

use once_cell::sync::Lazy;
use regex::Regex;

// ============================================================================
// Pre-compiled Regex Patterns
// ============================================================================

/// Email regex pattern (RFC 5322 simplified)
static EMAIL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap());

/// URL regex pattern (http/https)
static URL_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^https?://[^\s/$.?#].[^\s]*$").unwrap());

/// UUID regex pattern (v4)
static UUID_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-4[0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}$",
    )
    .unwrap()
});

/// ISO 8601 DateTime regex pattern
static DATETIME_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{1,9})?(Z|[+-]\d{2}:\d{2})$").unwrap()
});

/// Date regex pattern (YYYY-MM-DD)
static DATE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap());

/// Time regex pattern (HH:MM:SS with optional fractional seconds)
/// Note: This validates format only, not valid time ranges
static TIME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([01]\d|2[0-3]):([0-5]\d):([0-5]\d)(\.\d{1,9})?$").unwrap());

// ============================================================================
// New Format Patterns (Phase 1 Enhancement)
// ============================================================================

/// IPv4 regex pattern (with range validation in function)
static IPV4_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$").unwrap());

// Note: IPv6 validation uses std::net::Ipv6Addr for correctness
// This handles all edge cases including IPv4-mapped addresses properly

/// Hostname regex pattern (RFC 1123)
///
/// Note: This validates ASCII-only hostnames per RFC 1123.
/// For internationalized domain names (IDN), use `idn-hostname` format.
/// JSON Schema spec uses "hostname" for ASCII and "idn-hostname" for Unicode.
static HOSTNAME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?$").unwrap());

/// FQDN regex pattern (Fully Qualified Domain Name)
///
/// Note: This validates ASCII-only FQDNs per RFC 1123.
/// For internationalized domain names (IDN), consider adding `idn-hostname`.
static FQDN_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$").unwrap()
});

/// E.164 phone number format (+[country code][number], 8-15 digits total)
static PHONE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\+[1-9]\d{7,14}$").unwrap());

/// Base64 regex pattern (standard alphabet with optional padding)
static BASE64_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9+/]*={0,2}$").unwrap());

/// Slug regex pattern (URL-friendly identifier)
static SLUG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").unwrap());

// ============================================================================
// Format Validators
// ============================================================================

/// Validate email format
///
/// # Example
/// ```
/// use schema::formats::validate_email;
///
/// assert!(validate_email("user@example.com"));
/// assert!(!validate_email("invalid-email"));
/// ```
pub fn validate_email(value: &str) -> bool {
    EMAIL_REGEX.is_match(value)
}

/// Validate URL format (http/https)
///
/// # Example
/// ```
/// use schema::formats::validate_url;
///
/// assert!(validate_url("https://example.com"));
/// assert!(validate_url("http://localhost:8080/path"));
/// assert!(!validate_url("ftp://example.com"));
/// ```
pub fn validate_url(value: &str) -> bool {
    URL_REGEX.is_match(value)
}

/// Validate UUID format (v4)
///
/// # Example
/// ```
/// use schema::formats::validate_uuid;
///
/// assert!(validate_uuid("550e8400-e29b-41d4-a716-446655440000"));
/// assert!(!validate_uuid("not-a-uuid"));
/// ```
pub fn validate_uuid(value: &str) -> bool {
    UUID_REGEX.is_match(value)
}

/// Validate ISO 8601 DateTime format
///
/// # Example
/// ```
/// use schema::formats::validate_datetime;
///
/// assert!(validate_datetime("2024-01-19T12:00:00Z"));
/// assert!(validate_datetime("2024-01-19T12:00:00.123456789+08:00"));
/// assert!(!validate_datetime("2024-01-19 12:00:00"));
/// ```
pub fn validate_datetime(value: &str) -> bool {
    DATETIME_REGEX.is_match(value)
}

/// Validate date format (YYYY-MM-DD)
///
/// # Example
/// ```
/// use schema::formats::validate_date;
///
/// assert!(validate_date("2024-01-19"));
/// assert!(!validate_date("01/19/2024"));
/// ```
pub fn validate_date(value: &str) -> bool {
    DATE_REGEX.is_match(value)
}

/// Validate time format (HH:MM:SS with optional fractional seconds)
///
/// # Example
/// ```
/// use schema::formats::validate_time;
///
/// assert!(validate_time("12:00:00"));
/// assert!(validate_time("23:59:59.999"));
/// assert!(!validate_time("25:00:00"));
/// ```
pub fn validate_time(value: &str) -> bool {
    TIME_REGEX.is_match(value)
}

// ============================================================================
// New Format Validators (Phase 1 Enhancement)
// ============================================================================

/// Validate IPv4 address format with range checking
///
/// # Example
/// ```
/// use schema::formats::validate_ipv4;
///
/// assert!(validate_ipv4("192.168.1.1"));
/// assert!(validate_ipv4("0.0.0.0"));
/// assert!(validate_ipv4("255.255.255.255"));
/// assert!(!validate_ipv4("256.1.1.1"));
/// assert!(!validate_ipv4("1.2.3"));
/// ```
pub fn validate_ipv4(value: &str) -> bool {
    IPV4_REGEX.captures(value).map_or(false, |caps| {
        (1..=4).all(|i| {
            caps.get(i)
                .and_then(|m| m.as_str().parse::<u8>().ok())
                .is_some()
        })
    })
}

/// Validate IPv6 address format
///
/// Uses std::net::Ipv6Addr for correct parsing of all IPv6 forms
/// including IPv4-mapped addresses with proper octet range validation.
///
/// # Example
/// ```
/// use schema::formats::validate_ipv6;
///
/// assert!(validate_ipv6("2001:0db8:85a3:0000:0000:8a2e:0370:7334"));
/// assert!(validate_ipv6("2001:db8:85a3::8a2e:370:7334"));
/// assert!(validate_ipv6("::1"));
/// assert!(validate_ipv6("fe80::1"));
/// assert!(!validate_ipv6("not-an-ipv6"));
/// assert!(!validate_ipv6("::ffff:999.999.999.999")); // Invalid IPv4-mapped
/// ```
pub fn validate_ipv6(value: &str) -> bool {
    use std::net::Ipv6Addr;
    // Strip zone ID if present (e.g., fe80::1%eth0)
    let addr = value.split('%').next().unwrap_or(value);
    addr.parse::<Ipv6Addr>().is_ok()
}

/// Validate hostname format (RFC 1123)
///
/// # Example
/// ```
/// use schema::formats::validate_hostname;
///
/// assert!(validate_hostname("localhost"));
/// assert!(validate_hostname("my-server"));
/// assert!(validate_hostname("server1"));
/// assert!(!validate_hostname("-invalid"));
/// assert!(!validate_hostname("invalid-"));
/// ```
pub fn validate_hostname(value: &str) -> bool {
    if value.is_empty() || value.len() > 63 {
        return false;
    }
    HOSTNAME_REGEX.is_match(value)
}

/// Validate Fully Qualified Domain Name (FQDN) format
///
/// # Example
/// ```
/// use schema::formats::validate_fqdn;
///
/// assert!(validate_fqdn("example.com"));
/// assert!(validate_fqdn("sub.domain.example.org"));
/// assert!(!validate_fqdn("localhost"));
/// assert!(!validate_fqdn("-invalid.com"));
/// ```
pub fn validate_fqdn(value: &str) -> bool {
    if value.is_empty() || value.len() > 253 {
        return false;
    }
    FQDN_REGEX.is_match(value)
}

/// Validate E.164 phone number format
///
/// Accepts phone numbers starting with + followed by country code and number
/// (8-15 digits total after +).
///
/// # Example
/// ```
/// use schema::formats::validate_phone;
///
/// assert!(validate_phone("+14155551234"));
/// assert!(validate_phone("+886912345678"));
/// assert!(!validate_phone("14155551234")); // Missing +
/// assert!(!validate_phone("+1234567")); // Too short
/// ```
pub fn validate_phone(value: &str) -> bool {
    PHONE_REGEX.is_match(value)
}

/// Validate base64 encoded string format
///
/// # Example
/// ```
/// use schema::formats::validate_base64;
///
/// assert!(validate_base64("SGVsbG8gV29ybGQ="));
/// assert!(validate_base64("YWJjZA=="));
/// assert!(validate_base64("YWJj"));
/// assert!(!validate_base64("not valid base64!"));
/// ```
pub fn validate_base64(value: &str) -> bool {
    if value.is_empty() {
        return true; // Empty string is valid base64
    }
    // Check length is multiple of 4 or has valid padding
    let len = value.len();
    if len % 4 != 0 {
        return false;
    }
    BASE64_REGEX.is_match(value)
}

/// Validate URL-friendly slug format
///
/// Slugs are lowercase alphanumeric strings with hyphens separating words.
///
/// # Example
/// ```
/// use schema::formats::validate_slug;
///
/// assert!(validate_slug("hello-world"));
/// assert!(validate_slug("my-awesome-post-123"));
/// assert!(validate_slug("simple"));
/// assert!(!validate_slug("Hello-World")); // Uppercase
/// assert!(!validate_slug("-starts-with-hyphen"));
/// assert!(!validate_slug("ends-with-hyphen-"));
/// ```
pub fn validate_slug(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    SLUG_REGEX.is_match(value)
}

/// Validate JSON string format
///
/// Checks if the string is valid JSON.
///
/// # Example
/// ```
/// use schema::formats::validate_json;
///
/// assert!(validate_json(r#"{"key": "value"}"#));
/// assert!(validate_json(r#"[1, 2, 3]"#));
/// assert!(validate_json(r#""string""#));
/// assert!(validate_json("123"));
/// assert!(validate_json("true"));
/// assert!(validate_json("null"));
/// assert!(!validate_json("{invalid json}"));
/// ```
pub fn validate_json(value: &str) -> bool {
    // R2: When sonic-rs is available, use it for direct JSON validation
    // from byte slices — faster than the hand-written state machine.
    #[cfg(feature = "sonic")]
    {
        sonic_rs::from_str::<sonic_rs::Value>(value).is_ok()
    }
    #[cfg(not(feature = "sonic"))]
    {
        // Fallback: simple JSON validation using a basic parser approach
        let value = value.trim();
        if value.is_empty() {
            return false;
        }
        validate_json_value(value.as_bytes(), 0).map_or(false, |pos| pos == value.len())
    }
}

// Simple JSON parser for validation (only used when sonic feature is disabled)
#[cfg(not(feature = "sonic"))]
fn validate_json_value(bytes: &[u8], start: usize) -> Option<usize> {
    let pos = skip_whitespace(bytes, start);
    if pos >= bytes.len() {
        return None;
    }

    match bytes[pos] {
        b'{' => validate_json_object(bytes, pos),
        b'[' => validate_json_array(bytes, pos),
        b'"' => validate_json_string(bytes, pos),
        b't' => validate_json_literal(bytes, pos, b"true"),
        b'f' => validate_json_literal(bytes, pos, b"false"),
        b'n' => validate_json_literal(bytes, pos, b"null"),
        b'-' | b'0'..=b'9' => validate_json_number(bytes, pos),
        _ => None,
    }
}

#[cfg(not(feature = "sonic"))]
fn skip_whitespace(bytes: &[u8], start: usize) -> usize {
    let mut pos = start;
    while pos < bytes.len() && matches!(bytes[pos], b' ' | b'\t' | b'\n' | b'\r') {
        pos += 1;
    }
    pos
}

#[cfg(not(feature = "sonic"))]
fn validate_json_object(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start + 1; // Skip '{'
    pos = skip_whitespace(bytes, pos);

    if pos < bytes.len() && bytes[pos] == b'}' {
        return Some(pos + 1);
    }

    loop {
        pos = skip_whitespace(bytes, pos);
        if pos >= bytes.len() || bytes[pos] != b'"' {
            return None;
        }
        pos = validate_json_string(bytes, pos)?;
        pos = skip_whitespace(bytes, pos);
        if pos >= bytes.len() || bytes[pos] != b':' {
            return None;
        }
        pos = skip_whitespace(bytes, pos + 1);
        pos = validate_json_value(bytes, pos)?;
        pos = skip_whitespace(bytes, pos);
        if pos >= bytes.len() {
            return None;
        }
        match bytes[pos] {
            b'}' => return Some(pos + 1),
            b',' => pos += 1,
            _ => return None,
        }
    }
}

#[cfg(not(feature = "sonic"))]
fn validate_json_array(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start + 1; // Skip '['
    pos = skip_whitespace(bytes, pos);

    if pos < bytes.len() && bytes[pos] == b']' {
        return Some(pos + 1);
    }

    loop {
        pos = validate_json_value(bytes, pos)?;
        pos = skip_whitespace(bytes, pos);
        if pos >= bytes.len() {
            return None;
        }
        match bytes[pos] {
            b']' => return Some(pos + 1),
            b',' => pos = skip_whitespace(bytes, pos + 1),
            _ => return None,
        }
    }
}

#[cfg(not(feature = "sonic"))]
fn validate_json_string(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start + 1; // Skip opening '"'
    while pos < bytes.len() {
        match bytes[pos] {
            b'"' => return Some(pos + 1),
            b'\\' => {
                pos += 1;
                if pos >= bytes.len() {
                    return None;
                }
                match bytes[pos] {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => pos += 1,
                    b'u' => {
                        pos += 1;
                        let code_point = parse_unicode_escape(bytes, pos)?;
                        pos += 4;

                        // Check for surrogate pairs (U+D800 to U+DFFF)
                        if (0xD800..=0xDBFF).contains(&code_point) {
                            // High surrogate - must be followed by low surrogate
                            if pos + 6 > bytes.len()
                                || bytes[pos] != b'\\'
                                || bytes[pos + 1] != b'u'
                            {
                                return None; // Lone high surrogate
                            }
                            pos += 2;
                            let low = parse_unicode_escape(bytes, pos)?;
                            if !(0xDC00..=0xDFFF).contains(&low) {
                                return None; // Not a valid low surrogate
                            }
                            pos += 4;
                        } else if (0xDC00..=0xDFFF).contains(&code_point) {
                            // Lone low surrogate is invalid
                            return None;
                        }
                    }
                    _ => return None,
                }
            }
            0x00..=0x1F => return None, // Control characters not allowed
            _ => pos += 1,
        }
    }
    None
}

#[cfg(not(feature = "sonic"))]
fn parse_unicode_escape(bytes: &[u8], pos: usize) -> Option<u16> {
    if pos + 4 > bytes.len() {
        return None;
    }
    let hex_str = std::str::from_utf8(&bytes[pos..pos + 4]).ok()?;
    u16::from_str_radix(hex_str, 16).ok()
}

#[cfg(not(feature = "sonic"))]
fn validate_json_number(bytes: &[u8], start: usize) -> Option<usize> {
    let mut pos = start;
    if pos < bytes.len() && bytes[pos] == b'-' {
        pos += 1;
    }
    if pos >= bytes.len() {
        return None;
    }
    if bytes[pos] == b'0' {
        pos += 1;
    } else if bytes[pos].is_ascii_digit() {
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
    } else {
        return None;
    }
    // Fraction
    if pos < bytes.len() && bytes[pos] == b'.' {
        pos += 1;
        if pos >= bytes.len() || !bytes[pos].is_ascii_digit() {
            return None;
        }
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
    }
    // Exponent
    if pos < bytes.len() && (bytes[pos] == b'e' || bytes[pos] == b'E') {
        pos += 1;
        if pos < bytes.len() && (bytes[pos] == b'+' || bytes[pos] == b'-') {
            pos += 1;
        }
        if pos >= bytes.len() || !bytes[pos].is_ascii_digit() {
            return None;
        }
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
    }
    Some(pos)
}

#[cfg(not(feature = "sonic"))]
fn validate_json_literal(bytes: &[u8], start: usize, expected: &[u8]) -> Option<usize> {
    if bytes.len() >= start + expected.len() && &bytes[start..start + expected.len()] == expected {
        Some(start + expected.len())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_validation() {
        // Valid emails
        assert!(validate_email("user@example.com"));
        assert!(validate_email("test.user+tag@subdomain.example.co.uk"));
        assert!(validate_email("admin@localhost.local"));

        // Invalid emails
        assert!(!validate_email("invalid-email"));
        assert!(!validate_email("@example.com"));
        assert!(!validate_email("user@"));
        assert!(!validate_email("user@.com"));
        assert!(!validate_email("user@example"));
    }

    #[test]
    fn test_url_validation() {
        // Valid URLs
        assert!(validate_url("https://example.com"));
        assert!(validate_url("http://localhost:8080"));
        assert!(validate_url(
            "https://sub.domain.example.com/path?query=value"
        ));

        // Invalid URLs
        assert!(!validate_url("ftp://example.com"));
        assert!(!validate_url("not-a-url"));
        assert!(!validate_url("://example.com"));
    }

    #[test]
    fn test_uuid_validation() {
        // Valid UUIDs (v4)
        assert!(validate_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(validate_uuid("6ba7b810-9dad-41d1-80b4-00c04fd430c8"));

        // Invalid UUIDs
        assert!(!validate_uuid("not-a-uuid"));
        assert!(!validate_uuid("550e8400-e29b-11d4-a716-446655440000")); // Not v4
        assert!(!validate_uuid("550e8400e29b41d4a716446655440000")); // Missing hyphens
    }

    #[test]
    fn test_datetime_validation() {
        // Valid DateTimes
        assert!(validate_datetime("2024-01-19T12:00:00Z"));
        assert!(validate_datetime("2024-01-19T12:00:00+08:00"));
        assert!(validate_datetime("2024-01-19T12:00:00.123456789Z"));

        // Invalid DateTimes
        assert!(!validate_datetime("2024-01-19 12:00:00"));
        assert!(!validate_datetime("2024-01-19T12:00:00"));
        assert!(!validate_datetime("01/19/2024 12:00:00"));
    }

    #[test]
    fn test_date_validation() {
        // Valid dates
        assert!(validate_date("2024-01-19"));
        assert!(validate_date("2000-12-31"));

        // Invalid dates
        assert!(!validate_date("01/19/2024"));
        assert!(!validate_date("2024-1-19"));
        assert!(!validate_date("24-01-19"));
    }

    #[test]
    fn test_time_validation() {
        // Valid times
        assert!(validate_time("12:00:00"));
        assert!(validate_time("23:59:59"));
        assert!(validate_time("00:00:00.123456789"));

        // Invalid times
        assert!(!validate_time("25:00:00"));
        assert!(!validate_time("12:00"));
        assert!(!validate_time("12:00:00 PM"));
    }

    // ========================================================================
    // New Format Validator Tests (Phase 1)
    // ========================================================================

    #[test]
    fn test_ipv4_validation() {
        // Valid IPv4
        assert!(validate_ipv4("192.168.1.1"));
        assert!(validate_ipv4("0.0.0.0"));
        assert!(validate_ipv4("255.255.255.255"));
        assert!(validate_ipv4("10.0.0.1"));
        assert!(validate_ipv4("172.16.0.1"));

        // Invalid IPv4
        assert!(!validate_ipv4("256.1.1.1"));
        assert!(!validate_ipv4("1.2.3"));
        assert!(!validate_ipv4("1.2.3.4.5"));
        assert!(!validate_ipv4("192.168.1.256"));
        assert!(!validate_ipv4("abc.def.ghi.jkl"));
        assert!(!validate_ipv4(""));
    }

    #[test]
    fn test_ipv6_validation() {
        // Valid IPv6
        assert!(validate_ipv6("2001:0db8:85a3:0000:0000:8a2e:0370:7334"));
        assert!(validate_ipv6("2001:db8:85a3::8a2e:370:7334"));
        assert!(validate_ipv6("::1"));
        assert!(validate_ipv6("fe80::1"));
        assert!(validate_ipv6("::"));
        assert!(validate_ipv6("2001:db8::"));
        // Valid IPv4-mapped IPv6
        assert!(validate_ipv6("::ffff:192.168.1.1"));
        assert!(validate_ipv6("::ffff:127.0.0.1"));

        // Invalid IPv6
        assert!(!validate_ipv6("not-an-ipv6"));
        assert!(!validate_ipv6("192.168.1.1")); // IPv4 not valid as IPv6
        assert!(!validate_ipv6(""));
        // Invalid IPv4-mapped IPv6 (octets out of range)
        assert!(!validate_ipv6("::ffff:999.999.999.999"));
        assert!(!validate_ipv6("::ffff:256.1.1.1"));
    }

    #[test]
    fn test_hostname_validation() {
        // Valid hostnames
        assert!(validate_hostname("localhost"));
        assert!(validate_hostname("my-server"));
        assert!(validate_hostname("server1"));
        assert!(validate_hostname("a"));
        assert!(validate_hostname("a1b2c3"));

        // Invalid hostnames
        assert!(!validate_hostname("-invalid"));
        assert!(!validate_hostname("invalid-"));
        assert!(!validate_hostname(""));
        assert!(!validate_hostname("a".repeat(64).as_str())); // Too long
    }

    #[test]
    fn test_fqdn_validation() {
        // Valid FQDNs
        assert!(validate_fqdn("example.com"));
        assert!(validate_fqdn("sub.domain.example.org"));
        assert!(validate_fqdn("my-site.co.uk"));
        assert!(validate_fqdn("a.bc"));

        // Invalid FQDNs
        assert!(!validate_fqdn("localhost")); // No TLD
        assert!(!validate_fqdn("-invalid.com"));
        assert!(!validate_fqdn("invalid-.com"));
        assert!(!validate_fqdn(""));
    }

    #[test]
    fn test_phone_validation() {
        // Valid phone numbers (E.164)
        assert!(validate_phone("+14155551234"));
        assert!(validate_phone("+886912345678"));
        assert!(validate_phone("+12345678"));

        // Invalid phone numbers
        assert!(!validate_phone("14155551234")); // Missing +
        assert!(!validate_phone("+1234567")); // Too short
        assert!(!validate_phone("+01234567890")); // Leading 0 after +
        assert!(!validate_phone("+1-415-555-1234")); // Contains hyphens
        assert!(!validate_phone(""));
    }

    #[test]
    fn test_base64_validation() {
        // Valid base64
        assert!(validate_base64("SGVsbG8gV29ybGQ=")); // "Hello World"
        assert!(validate_base64("YWJjZA==")); // "abcd"
        assert!(validate_base64("YWJj")); // "abc" (no padding needed)
        assert!(validate_base64("")); // Empty is valid

        // Invalid base64
        assert!(!validate_base64("not valid base64!"));
        assert!(!validate_base64("abc")); // Length not multiple of 4
        assert!(!validate_base64("ab=c")); // Padding in wrong place
    }

    #[test]
    fn test_slug_validation() {
        // Valid slugs
        assert!(validate_slug("hello-world"));
        assert!(validate_slug("my-awesome-post-123"));
        assert!(validate_slug("simple"));
        assert!(validate_slug("123"));
        assert!(validate_slug("a1b2c3"));

        // Invalid slugs
        assert!(!validate_slug("Hello-World")); // Uppercase
        assert!(!validate_slug("-starts-with-hyphen"));
        assert!(!validate_slug("ends-with-hyphen-"));
        assert!(!validate_slug("double--hyphen"));
        assert!(!validate_slug("has spaces"));
        assert!(!validate_slug(""));
    }

    #[test]
    fn test_json_validation() {
        // Valid JSON
        assert!(validate_json(r#"{"key": "value"}"#));
        assert!(validate_json(r#"[1, 2, 3]"#));
        assert!(validate_json(r#""string""#));
        assert!(validate_json("123"));
        assert!(validate_json("-123.456"));
        assert!(validate_json("1e10"));
        assert!(validate_json("true"));
        assert!(validate_json("false"));
        assert!(validate_json("null"));
        assert!(validate_json(r#"{"nested": {"key": [1, 2, 3]}}"#));
        assert!(validate_json("[]"));
        assert!(validate_json("{}"));

        // Invalid JSON
        assert!(!validate_json("{invalid json}"));
        assert!(!validate_json(""));
        assert!(!validate_json("{"));
        assert!(!validate_json("[1, 2,]")); // Trailing comma
        assert!(!validate_json("{'single': 'quotes'}")); // Single quotes
    }

    #[test]
    fn test_json_unicode_surrogates() {
        // Valid: properly paired surrogates (emoji 😀 = U+1F600 = \uD83D\uDE00)
        assert!(validate_json(r#""\uD83D\uDE00""#));

        // Valid: regular Unicode escape (not a surrogate)
        assert!(validate_json(r#""\u0041""#)); // 'A'
        assert!(validate_json(r#""\u4E2D""#)); // '中'

        // Invalid: lone high surrogate
        assert!(!validate_json(r#""\uD800""#));
        assert!(!validate_json(r#""\uDBFF""#));

        // Invalid: lone low surrogate
        assert!(!validate_json(r#""\uDC00""#));
        assert!(!validate_json(r#""\uDFFF""#));

        // Invalid: high surrogate not followed by low surrogate
        assert!(!validate_json(r#""\uD83D\u0041""#)); // high + regular
        assert!(!validate_json(r#""\uD83Dabc""#)); // high + not escape

        // Invalid: low surrogate followed by high (wrong order)
        assert!(!validate_json(r#""\uDC00\uD800""#));
    }
}
