//! Common validation utilities shared across platforms.
//!
//! This module provides reusable validation functions for strings, URLs, locales, and more.

/// Validates that a string does not exceed a maximum character count.
///
/// Note: This counts Unicode characters (grapheme clusters approximately), not bytes.
///
/// # Arguments
///
/// * `value` - The string to validate
/// * `max_chars` - Maximum allowed character count
///
/// # Returns
///
/// `true` if the string is within the limit, `false` otherwise.
pub fn validate_length(value: &str, max_chars: usize) -> bool {
    char_count(value) <= max_chars
}

/// Counts the number of characters in a string.
///
/// This counts Unicode scalar values (chars), which is how most app stores count characters.
///
/// # Arguments
///
/// * `value` - The string to count characters in
///
/// # Returns
///
/// The number of characters.
pub fn char_count(value: &str) -> usize {
    value.chars().count()
}

/// Validates that a URL is properly formatted.
///
/// Checks for:
/// - Valid URL scheme (http or https)
/// - Valid host
/// - Overall URL structure
///
/// # Arguments
///
/// * `url` - The URL string to validate
///
/// # Returns
///
/// `true` if the URL is valid, `false` otherwise.
pub fn validate_url(url: &str) -> bool {
    let url = url.trim();

    // Must have a scheme
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }

    // Basic structure validation
    let parts: Vec<&str> = url.splitn(2, "://").collect();
    if parts.len() != 2 {
        return false;
    }

    let rest = parts[1];
    if rest.is_empty() {
        return false;
    }

    // Must have a host part
    let host_end = rest.find('/').unwrap_or(rest.len());
    let host_part = &rest[..host_end];

    // Host must not be empty
    if host_part.is_empty() {
        return false;
    }

    // Remove port if present
    let host = if let Some(port_idx) = host_part.rfind(':') {
        // Check if this is IPv6 (contains ']' before the colon)
        if host_part[..port_idx].ends_with(']') {
            // IPv6 with port
            &host_part[..port_idx]
        } else if host_part.contains('[') {
            // IPv6 without port
            host_part
        } else {
            // IPv4 or hostname with port
            &host_part[..port_idx]
        }
    } else {
        host_part
    };

    // Check for empty host after removing port
    if host.is_empty() {
        return false;
    }

    // Basic hostname validation
    // Must contain at least one dot or be localhost
    let is_valid_host = host == "localhost"
        || host.starts_with('[') // IPv6
        || (host.contains('.') && !host.starts_with('.') && !host.ends_with('.'));

    is_valid_host
}

/// Validates that a locale string is a valid BCP 47 language tag.
///
/// Accepts formats like:
/// - "en" (language only)
/// - "en-US" (language and region)
/// - "en_US" (underscore variant)
///
/// # Arguments
///
/// * `locale` - The locale string to validate
///
/// # Returns
///
/// `true` if the locale is valid, `false` otherwise.
pub fn validate_locale(locale: &str) -> bool {
    let locale = locale.trim();
    if locale.is_empty() {
        return false;
    }

    let parts: Vec<&str> = locale.split(&['-', '_'][..]).collect();

    // Language code (required)
    if parts.is_empty() {
        return false;
    }

    let language = parts[0];
    if language.len() < 2 || language.len() > 3 {
        return false;
    }
    if !language.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }

    // Region code (optional)
    if parts.len() > 1 {
        let region = parts[1];
        // Region can be 2 letters (country) or 3 digits (numeric region)
        if region.len() == 2 {
            if !region.chars().all(|c| c.is_ascii_alphabetic()) {
                return false;
            }
        } else if region.len() == 3 {
            if !region.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
        } else {
            return false;
        }
    }

    // For simplicity, we don't validate script or variant subtags
    // A full BCP 47 validator would be more complex
    true
}

/// Checks if a string has leading or trailing whitespace.
///
/// # Arguments
///
/// * `value` - The string to check
///
/// # Returns
///
/// `true` if the string has excess whitespace, `false` otherwise.
pub fn has_excess_whitespace(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }

    let first = value.chars().next();
    let last = value.chars().next_back();

    matches!(first, Some(c) if c.is_whitespace()) ||
    matches!(last, Some(c) if c.is_whitespace())
}

/// Checks if a string contains any newlines.
///
/// # Arguments
///
/// * `value` - The string to check
///
/// # Returns
///
/// `true` if the string contains newlines, `false` otherwise.
pub fn contains_newlines(value: &str) -> bool {
    value.contains('\n') || value.contains('\r')
}

/// Auto-fixes common text issues.
///
/// Performs the following fixes:
/// - Trims leading and trailing whitespace
/// - Normalizes Windows line endings (CRLF) to Unix (LF)
/// - Removes excessive blank lines (more than 2 consecutive newlines become 2)
///
/// # Arguments
///
/// * `value` - The string to fix
///
/// # Returns
///
/// A fixed version of the string.
pub fn auto_fix_text(value: &str) -> String {
    let mut result = value.trim().to_string();

    // Normalize line endings
    result = result.replace("\r\n", "\n");
    result = result.replace('\r', "\n");

    // Remove excessive blank lines
    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    result
}

/// Validates keywords format for Apple App Store.
///
/// Apple keywords should be comma-separated without spaces after commas.
///
/// # Arguments
///
/// * `keywords` - The keywords string to validate
///
/// # Returns
///
/// A tuple of (is_valid, has_spaces_after_commas).
pub fn validate_keywords_format(keywords: &str) -> (bool, bool) {
    let keywords = keywords.trim();

    if keywords.is_empty() {
        return (true, false);
    }

    // Check for spaces after commas (not ideal but not an error)
    let has_spaces = keywords.contains(", ");

    // Keywords shouldn't start or end with a comma
    let valid = !keywords.starts_with(',') && !keywords.ends_with(',');

    (valid, has_spaces)
}

/// Suggests optimized keywords by removing spaces after commas.
///
/// # Arguments
///
/// * `keywords` - The keywords string to optimize
///
/// # Returns
///
/// Optimized keywords string.
pub fn optimize_keywords(keywords: &str) -> String {
    keywords
        .split(',')
        .map(|k| k.trim())
        .filter(|k| !k.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

/// Checks if a string is empty or contains only whitespace.
///
/// # Arguments
///
/// * `value` - The string to check
///
/// # Returns
///
/// `true` if the string is empty or whitespace-only, `false` otherwise.
pub fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

/// Validates that a string contains only allowed characters.
///
/// # Arguments
///
/// * `value` - The string to validate
/// * `allowed` - A function that returns true for allowed characters
///
/// # Returns
///
/// `true` if all characters are allowed, `false` otherwise.
pub fn validate_chars<F>(value: &str, allowed: F) -> bool
where
    F: Fn(char) -> bool,
{
    value.chars().all(allowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_length() {
        assert!(validate_length("hello", 10));
        assert!(validate_length("hello", 5));
        assert!(!validate_length("hello", 4));

        // Test with Unicode characters
        assert!(validate_length("caf\u{e9}", 4)); // café is 4 chars
        assert!(!validate_length("caf\u{e9}", 3));

        // Test with emoji
        assert!(validate_length("hello \u{1F600}", 7)); // 6 chars + emoji
    }

    #[test]
    fn test_char_count() {
        assert_eq!(char_count("hello"), 5);
        assert_eq!(char_count("caf\u{e9}"), 4);
        assert_eq!(char_count(""), 0);
        assert_eq!(char_count("\u{1F600}"), 1); // emoji
    }

    #[test]
    fn test_validate_url() {
        // Valid URLs
        assert!(validate_url("https://example.com"));
        assert!(validate_url("https://example.com/path"));
        assert!(validate_url("http://example.com"));
        assert!(validate_url("https://sub.example.com/path?query=1"));
        assert!(validate_url("https://localhost:8080"));
        assert!(validate_url("http://localhost"));

        // Invalid URLs
        assert!(!validate_url("example.com"));
        assert!(!validate_url("ftp://example.com"));
        assert!(!validate_url("https://"));
        assert!(!validate_url("https:///path"));
        assert!(!validate_url(""));
        assert!(!validate_url("not a url"));
    }

    #[test]
    fn test_validate_locale() {
        // Valid locales
        assert!(validate_locale("en"));
        assert!(validate_locale("en-US"));
        assert!(validate_locale("en_US"));
        assert!(validate_locale("de-DE"));
        assert!(validate_locale("ja"));
        assert!(validate_locale("zh-CN"));
        assert!(validate_locale("pt-BR"));

        // Invalid locales
        assert!(!validate_locale(""));
        assert!(!validate_locale("x"));
        assert!(!validate_locale("english"));
        assert!(!validate_locale("en-USA")); // Region must be 2 chars
        assert!(!validate_locale("123"));
    }

    #[test]
    fn test_has_excess_whitespace() {
        assert!(!has_excess_whitespace("hello"));
        assert!(!has_excess_whitespace("hello world"));
        assert!(has_excess_whitespace(" hello"));
        assert!(has_excess_whitespace("hello "));
        assert!(has_excess_whitespace(" hello "));
        assert!(has_excess_whitespace("\thello"));
        assert!(has_excess_whitespace("hello\n"));
        assert!(!has_excess_whitespace(""));
    }

    #[test]
    fn test_contains_newlines() {
        assert!(!contains_newlines("hello"));
        assert!(contains_newlines("hello\nworld"));
        assert!(contains_newlines("hello\r\nworld"));
        assert!(contains_newlines("hello\rworld"));
    }

    #[test]
    fn test_auto_fix_text() {
        assert_eq!(auto_fix_text("  hello  "), "hello");
        assert_eq!(auto_fix_text("hello\r\nworld"), "hello\nworld");
        assert_eq!(auto_fix_text("a\n\n\n\nb"), "a\n\nb");
        assert_eq!(auto_fix_text("  a\r\n\r\n\r\nb  "), "a\n\nb");
    }

    #[test]
    fn test_validate_keywords_format() {
        assert_eq!(validate_keywords_format("a,b,c"), (true, false));
        assert_eq!(validate_keywords_format("a, b, c"), (true, true));
        assert_eq!(validate_keywords_format(",a,b"), (false, false));
        assert_eq!(validate_keywords_format("a,b,"), (false, false));
        assert_eq!(validate_keywords_format(""), (true, false));
    }

    #[test]
    fn test_optimize_keywords() {
        assert_eq!(optimize_keywords("a, b, c"), "a,b,c");
        assert_eq!(optimize_keywords("  a  ,  b  ,  c  "), "a,b,c");
        assert_eq!(optimize_keywords("a,b,c"), "a,b,c");
        assert_eq!(optimize_keywords("a,,b,c,"), "a,b,c");
    }

    #[test]
    fn test_is_blank() {
        assert!(is_blank(""));
        assert!(is_blank("   "));
        assert!(is_blank("\t\n"));
        assert!(!is_blank("a"));
        assert!(!is_blank(" a "));
    }
}
