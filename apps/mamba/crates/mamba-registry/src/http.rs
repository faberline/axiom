//! Canonical HTTP status code table.
//!
//! One authoritative source for both:
//! - mamba's `http.HTTPStatus` namespace (Python-side constants + phrases)
//! - binding crates that need the RFC 7231 reason phrase for a status
//!   code at generation / runtime time (e.g. `HTTPException(404).detail`
//!   defaulting to `"Not Found"`).
//!
//! Before this module existed, binding crates shipped their own inlined
//! phrase table (e.g. the `templates/mamba_binding/http_status_phrase.tera`
//! helper in the SDD codegen repo). Everyone is expected to route
//! through this table instead after PR-4 lands.
//!
//! # Source
//!
//! Codes and phrases match CPython 3.12's `http.HTTPStatus` enum, which
//! itself tracks IANA + RFC 7231. New entries should land here first;
//! mamba's Python-visible `http` module reads the same table.

/// `(code, snake_upper_name, reason_phrase)` triples — CPython 3.12 parity.
///
/// Sorted by status code so binary search is viable. The `name` column is
/// the `HTTPStatus` enum identifier (e.g. `"NOT_FOUND"`) and `phrase` is
/// the RFC-canonical reason line (e.g. `"Not Found"`).
const CANONICAL: &[(u16, &str, &str)] = &[
    (100, "CONTINUE", "Continue"),
    (101, "SWITCHING_PROTOCOLS", "Switching Protocols"),
    (102, "PROCESSING", "Processing"),
    (103, "EARLY_HINTS", "Early Hints"),
    (200, "OK", "OK"),
    (201, "CREATED", "Created"),
    (202, "ACCEPTED", "Accepted"),
    (
        203,
        "NON_AUTHORITATIVE_INFORMATION",
        "Non-Authoritative Information",
    ),
    (204, "NO_CONTENT", "No Content"),
    (205, "RESET_CONTENT", "Reset Content"),
    (206, "PARTIAL_CONTENT", "Partial Content"),
    (207, "MULTI_STATUS", "Multi-Status"),
    (208, "ALREADY_REPORTED", "Already Reported"),
    (226, "IM_USED", "IM Used"),
    (300, "MULTIPLE_CHOICES", "Multiple Choices"),
    (301, "MOVED_PERMANENTLY", "Moved Permanently"),
    (302, "FOUND", "Found"),
    (303, "SEE_OTHER", "See Other"),
    (304, "NOT_MODIFIED", "Not Modified"),
    (305, "USE_PROXY", "Use Proxy"),
    (307, "TEMPORARY_REDIRECT", "Temporary Redirect"),
    (308, "PERMANENT_REDIRECT", "Permanent Redirect"),
    (400, "BAD_REQUEST", "Bad Request"),
    (401, "UNAUTHORIZED", "Unauthorized"),
    (402, "PAYMENT_REQUIRED", "Payment Required"),
    (403, "FORBIDDEN", "Forbidden"),
    (404, "NOT_FOUND", "Not Found"),
    (405, "METHOD_NOT_ALLOWED", "Method Not Allowed"),
    (406, "NOT_ACCEPTABLE", "Not Acceptable"),
    (
        407,
        "PROXY_AUTHENTICATION_REQUIRED",
        "Proxy Authentication Required",
    ),
    (408, "REQUEST_TIMEOUT", "Request Timeout"),
    (409, "CONFLICT", "Conflict"),
    (410, "GONE", "Gone"),
    (411, "LENGTH_REQUIRED", "Length Required"),
    (412, "PRECONDITION_FAILED", "Precondition Failed"),
    (413, "REQUEST_ENTITY_TOO_LARGE", "Request Entity Too Large"),
    (414, "REQUEST_URI_TOO_LONG", "Request-URI Too Long"),
    (415, "UNSUPPORTED_MEDIA_TYPE", "Unsupported Media Type"),
    (
        416,
        "REQUESTED_RANGE_NOT_SATISFIABLE",
        "Requested Range Not Satisfiable",
    ),
    (417, "EXPECTATION_FAILED", "Expectation Failed"),
    (418, "IM_A_TEAPOT", "I'm a Teapot"),
    (421, "MISDIRECTED_REQUEST", "Misdirected Request"),
    (422, "UNPROCESSABLE_ENTITY", "Unprocessable Entity"),
    (423, "LOCKED", "Locked"),
    (424, "FAILED_DEPENDENCY", "Failed Dependency"),
    (425, "TOO_EARLY", "Too Early"),
    (426, "UPGRADE_REQUIRED", "Upgrade Required"),
    (428, "PRECONDITION_REQUIRED", "Precondition Required"),
    (429, "TOO_MANY_REQUESTS", "Too Many Requests"),
    (
        431,
        "REQUEST_HEADER_FIELDS_TOO_LARGE",
        "Request Header Fields Too Large",
    ),
    (
        451,
        "UNAVAILABLE_FOR_LEGAL_REASONS",
        "Unavailable For Legal Reasons",
    ),
    (500, "INTERNAL_SERVER_ERROR", "Internal Server Error"),
    (501, "NOT_IMPLEMENTED", "Not Implemented"),
    (502, "BAD_GATEWAY", "Bad Gateway"),
    (503, "SERVICE_UNAVAILABLE", "Service Unavailable"),
    (504, "GATEWAY_TIMEOUT", "Gateway Timeout"),
    (
        505,
        "HTTP_VERSION_NOT_SUPPORTED",
        "HTTP Version Not Supported",
    ),
    (506, "VARIANT_ALSO_NEGOTIATES", "Variant Also Negotiates"),
    (507, "INSUFFICIENT_STORAGE", "Insufficient Storage"),
    (508, "LOOP_DETECTED", "Loop Detected"),
    (510, "NOT_EXTENDED", "Not Extended"),
    (
        511,
        "NETWORK_AUTHENTICATION_REQUIRED",
        "Network Authentication Required",
    ),
];

/// RFC reason phrase for `code`, or `"Unknown"` for unregistered codes.
///
/// The fallback matches CPython's behaviour — `HTTPStatus(499).phrase`
/// would be `"Unknown"` on a code that isn't in the enum. Binding crates
/// use this as the default value for `HTTPException.detail` when the
/// caller didn't supply one:
///
/// ```ignore
/// let detail = user_detail.unwrap_or_else(|| {
///     mamba_registry::http::status_phrase(status_code).to_string()
/// });
/// ```
pub fn status_phrase(code: u16) -> &'static str {
    lookup(code)
        .map(|&(_, _, phrase)| phrase)
        .unwrap_or("Unknown")
}

/// Enum identifier for `code` (e.g. `404` → `Some("NOT_FOUND")`), `None`
/// if the code isn't registered. Used by mamba's Python-visible
/// `http.HTTPStatus(code).name` expression.
pub fn status_name(code: u16) -> Option<&'static str> {
    lookup(code).map(|&(_, name, _)| name)
}

/// Iterate every registered `(code, name, phrase)`. Used by mamba's
/// stdlib bootstrap to populate `http.HTTPStatus.{OK,NOT_FOUND,…}`
/// without duplicating the table.
pub fn canonical_codes() -> &'static [(u16, &'static str, &'static str)] {
    CANONICAL
}

/// Binary search over `CANONICAL`. Ok for cold-path uses; callers that
/// hit this in a hot loop should cache. The table is sorted by code.
fn lookup(code: u16) -> Option<&'static (u16, &'static str, &'static str)> {
    CANONICAL
        .binary_search_by_key(&code, |(c, _, _)| *c)
        .ok()
        .map(|i| &CANONICAL[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_phrases_match_cpython() {
        assert_eq!(status_phrase(200), "OK");
        assert_eq!(status_phrase(404), "Not Found");
        assert_eq!(status_phrase(418), "I'm a Teapot");
        assert_eq!(status_phrase(500), "Internal Server Error");
    }

    #[test]
    fn unknown_code_yields_unknown_phrase() {
        assert_eq!(status_phrase(499), "Unknown");
        assert_eq!(status_phrase(999), "Unknown");
        assert_eq!(status_phrase(0), "Unknown");
    }

    #[test]
    fn known_names_match_cpython() {
        assert_eq!(status_name(200), Some("OK"));
        assert_eq!(status_name(404), Some("NOT_FOUND"));
        assert_eq!(status_name(422), Some("UNPROCESSABLE_ENTITY"));
        assert_eq!(status_name(499), None);
    }

    #[test]
    fn canonical_table_sorted_and_unique() {
        let codes: Vec<u16> = canonical_codes().iter().map(|(c, _, _)| *c).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        assert_eq!(codes, sorted, "CANONICAL must be sorted by status code");
        let mut dedup = sorted.clone();
        dedup.dedup();
        assert_eq!(
            dedup.len(),
            sorted.len(),
            "CANONICAL must have unique codes"
        );
    }

    #[test]
    fn canonical_table_covers_core_codes() {
        let codes: std::collections::HashSet<u16> =
            canonical_codes().iter().map(|(c, _, _)| *c).collect();
        for expected in [100, 200, 301, 400, 404, 418, 500] {
            assert!(codes.contains(&expected), "missing status code {expected}");
        }
    }
}
