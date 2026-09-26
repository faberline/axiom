//! String utility functions for mamba-core
//!
//! Provides basic string validation utilities.

/// Check if a string consists entirely of alphanumeric characters (A-Z, a-z, 0-9).
///
/// # Arguments
/// * `s` - The string to validate
///
/// # Returns
/// * `true` if the string contains only alphanumeric characters or is empty
/// * `false` if the string contains any non-alphanumeric characters
///
/// # Examples
/// ```
/// use mamba_core::utils::is_alphanumeric;
///
/// assert!(is_alphanumeric("Alpha123"));
/// assert!(is_alphanumeric(""));
/// assert!(!is_alphanumeric("Alpha-123!"));
/// ```
pub fn is_alphanumeric(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_alphanumeric_string() {
        // Scenario: Valid Alphanumeric String
        assert!(is_alphanumeric("Alpha123"));
        assert!(is_alphanumeric("abc"));
        assert!(is_alphanumeric("123"));
        assert!(is_alphanumeric("ABC123xyz"));
    }

    #[test]
    fn test_string_with_special_characters() {
        // Scenario: String with Special Characters
        assert!(!is_alphanumeric("Alpha-123!"));
        assert!(!is_alphanumeric("hello world"));
        assert!(!is_alphanumeric("test@email.com"));
        assert!(!is_alphanumeric("path/to/file"));
    }

    #[test]
    fn test_empty_string() {
        // Scenario: Empty String Validation
        assert!(is_alphanumeric(""));
    }

    #[test]
    fn test_unicode_characters() {
        // Non-ASCII characters should return false
        assert!(!is_alphanumeric("café"));
        assert!(!is_alphanumeric("日本語"));
    }
}
