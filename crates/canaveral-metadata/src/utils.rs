//! Utility functions for metadata management.
//!
//! This module provides common utility functions for working with app store
//! metadata, including text processing, locale handling, and keyword management.

use std::collections::HashSet;

/// List of common/recommended locales for Apple App Store.
///
/// These are the most commonly used locales that Apple supports for App Store
/// listings. Consider supporting at least `en-US` and a few major markets.
pub const APPLE_RECOMMENDED_LOCALES: &[&str] = &[
    "en-US", "en-GB", "en-AU", "en-CA", "de-DE", "fr-FR", "es-ES", "es-MX", "it-IT", "pt-BR",
    "pt-PT", "nl-NL", "ja", "ko", "zh-Hans", "zh-Hant", "ru", "tr", "ar", "he", "th", "vi", "id",
    "ms", "pl", "uk", "cs", "el", "hu", "ro", "sk", "da", "fi", "no", "sv",
];

/// List of common/recommended locales for Google Play Store.
///
/// These are the most commonly used locales that Google Play supports for
/// store listings. Consider supporting at least `en-US` and a few major markets.
pub const GOOGLE_PLAY_RECOMMENDED_LOCALES: &[&str] = &[
    "en-US", "en-GB", "en-AU", "en-IN", "de-DE", "fr-FR", "es-ES", "es-419", "it-IT", "pt-BR",
    "pt-PT", "nl-NL", "ja-JP", "ko-KR", "zh-CN", "zh-TW", "ru-RU", "tr-TR", "ar", "he-IL", "th",
    "vi", "id", "ms", "pl-PL", "uk", "cs-CZ", "el-GR", "hu-HU", "ro", "sk", "da-DK", "fi-FI",
    "nb-NO", "sv-SE", "hi-IN", "bn-BD", "ta-IN",
];

/// Auto-fix common text issues in metadata.
///
/// This function performs the following fixes:
/// - Trims leading and trailing whitespace
/// - Removes trailing whitespace from each line
/// - Preserves intentional line breaks
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::auto_fix_text;
///
/// let input = "  Hello World  \n  Line 2   \n";
/// let fixed = auto_fix_text(input);
/// assert_eq!(fixed, "Hello World\n  Line 2");
/// ```
pub fn auto_fix_text(text: &str) -> String {
    text.trim()
        .lines()
        .map(|line| line.trim_end()) // Remove trailing whitespace from each line
        .collect::<Vec<_>>()
        .join("\n")
}

/// Normalize a locale code to standard format.
///
/// Converts locale codes to the standard format: language-REGION (e.g., "en-US").
/// Handles various input formats like "en_US", "EN-us", "en_us".
/// Also handles script codes like zh-Hans, zh-Hant.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::normalize_locale;
///
/// assert_eq!(normalize_locale("en_US"), "en-US");
/// assert_eq!(normalize_locale("EN-us"), "en-US");
/// assert_eq!(normalize_locale("de"), "de");
/// assert_eq!(normalize_locale("zh-hans"), "zh-Hans");
/// ```
pub fn normalize_locale(locale: &str) -> String {
    // Replace underscores with hyphens and normalize
    let normalized = locale.replace('_', "-");
    let parts: Vec<&str> = normalized.split('-').collect();

    match parts.len() {
        1 => parts[0].to_lowercase(),
        2 => {
            let lang = parts[0].to_lowercase();
            let second = parts[1];

            // Check if it's a script (like Hans, Hant) - typically 4 chars
            if second.len() == 4 {
                // It's a script like Hans or Hant - title case it
                let script: String = second
                    .chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if i == 0 {
                            c.to_uppercase().next().unwrap()
                        } else {
                            c.to_lowercase().next().unwrap()
                        }
                    })
                    .collect();
                format!("{}-{}", lang, script)
            } else {
                // Treat as region - uppercase it
                format!("{}-{}", lang, second.to_uppercase())
            }
        }
        _ => {
            // Handle complex locales with more than 2 parts
            let lang = parts[0].to_lowercase();
            if parts.len() >= 2 {
                let script_or_region = parts[1];
                // Check if it's a script (like Hans, Hant) - typically 4 chars
                if script_or_region.len() == 4 {
                    // It's a script like Hans or Hant - title case it
                    let script: String = script_or_region
                        .chars()
                        .enumerate()
                        .map(|(i, c)| {
                            if i == 0 {
                                c.to_uppercase().next().unwrap()
                            } else {
                                c.to_lowercase().next().unwrap()
                            }
                        })
                        .collect();
                    format!("{}-{}", lang, script)
                } else {
                    // Treat as region
                    format!("{}-{}", lang, script_or_region.to_uppercase())
                }
            } else {
                lang
            }
        }
    }
}

/// Check if two locale codes are equivalent.
///
/// Compares two locale codes after normalization, so "en-US" == "en_US" == "EN-us".
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::locales_equivalent;
///
/// assert!(locales_equivalent("en-US", "en_US"));
/// assert!(locales_equivalent("de-DE", "DE-de"));
/// assert!(!locales_equivalent("en-US", "en-GB"));
/// ```
pub fn locales_equivalent(a: &str, b: &str) -> bool {
    normalize_locale(a) == normalize_locale(b)
}

/// Get the language part of a locale code.
///
/// Extracts the language code from a locale, e.g., "en" from "en-US".
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::get_language_code;
///
/// assert_eq!(get_language_code("en-US"), "en");
/// assert_eq!(get_language_code("de"), "de");
/// assert_eq!(get_language_code("zh-Hans"), "zh");
/// ```
pub fn get_language_code(locale: &str) -> &str {
    if let Some(pos) = locale.find(|c| c == '-' || c == '_') {
        &locale[..pos]
    } else {
        locale
    }
}

/// Get the region part of a locale code.
///
/// Extracts the region code from a locale, e.g., "US" from "en-US".
/// Returns `None` if there is no region part.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::get_region_code;
///
/// assert_eq!(get_region_code("en-US"), Some("US"));
/// assert_eq!(get_region_code("de"), None);
/// assert_eq!(get_region_code("zh-Hans"), Some("Hans"));
/// ```
pub fn get_region_code(locale: &str) -> Option<&str> {
    if let Some(pos) = locale.find(|c| c == '-' || c == '_') {
        Some(&locale[pos + 1..])
    } else {
        None
    }
}

/// Parse a keywords string into a sorted, deduplicated list.
///
/// Keywords in app store metadata are typically comma-separated. This function
/// splits them, trims whitespace, removes duplicates, and returns a sorted list.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::parse_keywords;
///
/// let keywords = parse_keywords("app, utility, tool, app, helper");
/// assert_eq!(keywords, vec!["app", "helper", "tool", "utility"]);
/// ```
pub fn parse_keywords(keywords: &str) -> Vec<String> {
    let unique: HashSet<String> = keywords
        .split(',')
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();

    let mut sorted: Vec<String> = unique.into_iter().collect();
    sorted.sort();
    sorted
}

/// Join keywords back into a comma-separated string.
///
/// Formats a list of keywords into the standard comma-separated format used
/// by app stores.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::format_keywords;
///
/// let keywords = vec!["app".to_string(), "utility".to_string(), "tool".to_string()];
/// assert_eq!(format_keywords(&keywords), "app,utility,tool");
/// ```
pub fn format_keywords(keywords: &[String]) -> String {
    keywords.join(",")
}

/// Count characters in a string, handling Unicode properly.
///
/// This uses Rust's `char` count, which counts Unicode scalar values.
/// This is the correct way to count characters for app store validation.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::count_chars;
///
/// assert_eq!(count_chars("Hello"), 5);
/// assert_eq!(count_chars("Hello, World!"), 13);
/// // Unicode characters are counted correctly
/// assert_eq!(count_chars("\u{1F600}\u{1F601}"), 2);  // Two emoji
/// ```
pub fn count_chars(text: &str) -> usize {
    text.chars().count()
}

/// Truncate text to a maximum number of characters, adding ellipsis if truncated.
///
/// If the text is longer than `max_chars`, it will be truncated and "..." will
/// be appended. The final string will be at most `max_chars` characters long
/// (including the ellipsis).
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::truncate_with_ellipsis;
///
/// assert_eq!(truncate_with_ellipsis("Hello, World!", 10), "Hello, ...");
/// assert_eq!(truncate_with_ellipsis("Short", 10), "Short");
/// ```
pub fn truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        text.to_string()
    } else if max_chars <= 3 {
        text.chars().take(max_chars).collect()
    } else {
        let truncated: String = text.chars().take(max_chars - 3).collect();
        format!("{}...", truncated)
    }
}

/// Check if a locale is in the recommended list for Apple App Store.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::is_apple_recommended_locale;
///
/// assert!(is_apple_recommended_locale("en-US"));
/// assert!(!is_apple_recommended_locale("xy-ZZ"));
/// ```
pub fn is_apple_recommended_locale(locale: &str) -> bool {
    let normalized = normalize_locale(locale);
    APPLE_RECOMMENDED_LOCALES
        .iter()
        .any(|l| normalize_locale(l) == normalized)
}

/// Check if a locale is in the recommended list for Google Play Store.
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::utils::is_google_play_recommended_locale;
///
/// assert!(is_google_play_recommended_locale("en-US"));
/// assert!(!is_google_play_recommended_locale("xy-ZZ"));
/// ```
pub fn is_google_play_recommended_locale(locale: &str) -> bool {
    let normalized = normalize_locale(locale);
    GOOGLE_PLAY_RECOMMENDED_LOCALES
        .iter()
        .any(|l| normalize_locale(l) == normalized)
}

/// Get locales that are recommended but not yet supported by the app.
///
/// Returns a list of Apple recommended locales that are not in the provided list.
pub fn get_missing_apple_locales(supported: &[&str]) -> Vec<&'static str> {
    let supported_normalized: HashSet<String> =
        supported.iter().map(|l| normalize_locale(l)).collect();

    APPLE_RECOMMENDED_LOCALES
        .iter()
        .filter(|l| !supported_normalized.contains(&normalize_locale(l)))
        .copied()
        .collect()
}

/// Get locales that are recommended but not yet supported by the app.
///
/// Returns a list of Google Play recommended locales that are not in the provided list.
pub fn get_missing_google_play_locales(supported: &[&str]) -> Vec<&'static str> {
    let supported_normalized: HashSet<String> =
        supported.iter().map(|l| normalize_locale(l)).collect();

    GOOGLE_PLAY_RECOMMENDED_LOCALES
        .iter()
        .filter(|l| !supported_normalized.contains(&normalize_locale(l)))
        .copied()
        .collect()
}

/// Sanitize text for use in metadata fields.
///
/// Removes or replaces characters that might cause issues in app store metadata:
/// - Removes control characters (except newlines and tabs)
/// - Normalizes various dash types to standard hyphen
/// - Normalizes various quote types to standard quotes
pub fn sanitize_text(text: &str) -> String {
    text.chars()
        .filter_map(|c| {
            match c {
                // Allow regular printable characters, newlines, and tabs
                '\n' | '\t' | '\r' => Some(c),
                // Filter out other control characters
                c if c.is_control() => None,
                // Normalize various dashes to hyphen
                '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}' => {
                    Some('-')
                }
                // Normalize various quotes
                '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => Some('\''),
                '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => Some('"'),
                // Keep everything else
                _ => Some(c),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_fix_text() {
        assert_eq!(auto_fix_text("  hello  "), "hello");
        assert_eq!(auto_fix_text("hello  \nworld  "), "hello\nworld");
        assert_eq!(auto_fix_text("  line1  \n  line2  \n"), "line1\n  line2");
    }

    #[test]
    fn test_normalize_locale() {
        assert_eq!(normalize_locale("en-US"), "en-US");
        assert_eq!(normalize_locale("en_US"), "en-US");
        assert_eq!(normalize_locale("EN-us"), "en-US");
        assert_eq!(normalize_locale("de"), "de");
        assert_eq!(normalize_locale("zh-hans"), "zh-Hans");
        assert_eq!(normalize_locale("zh-HANT"), "zh-Hant");
    }

    #[test]
    fn test_locales_equivalent() {
        assert!(locales_equivalent("en-US", "en_US"));
        assert!(locales_equivalent("en-US", "EN-us"));
        assert!(locales_equivalent("de-DE", "de_de"));
        assert!(!locales_equivalent("en-US", "en-GB"));
    }

    #[test]
    fn test_get_language_code() {
        assert_eq!(get_language_code("en-US"), "en");
        assert_eq!(get_language_code("de"), "de");
        assert_eq!(get_language_code("zh-Hans"), "zh");
        assert_eq!(get_language_code("pt_BR"), "pt");
    }

    #[test]
    fn test_get_region_code() {
        assert_eq!(get_region_code("en-US"), Some("US"));
        assert_eq!(get_region_code("de"), None);
        assert_eq!(get_region_code("zh-Hans"), Some("Hans"));
        assert_eq!(get_region_code("pt_BR"), Some("BR"));
    }

    #[test]
    fn test_parse_keywords() {
        let keywords = parse_keywords("app, utility, tool, app, helper");
        assert_eq!(keywords, vec!["app", "helper", "tool", "utility"]);

        let empty = parse_keywords("");
        assert!(empty.is_empty());

        let single = parse_keywords("single");
        assert_eq!(single, vec!["single"]);
    }

    #[test]
    fn test_format_keywords() {
        let keywords = vec![
            "app".to_string(),
            "utility".to_string(),
            "tool".to_string(),
        ];
        assert_eq!(format_keywords(&keywords), "app,utility,tool");

        let empty: Vec<String> = vec![];
        assert_eq!(format_keywords(&empty), "");
    }

    #[test]
    fn test_count_chars() {
        assert_eq!(count_chars("Hello"), 5);
        assert_eq!(count_chars(""), 0);
        assert_eq!(count_chars("\u{1F600}\u{1F601}"), 2); // Two emoji (grinning faces)
        assert_eq!(count_chars("cafe\u{0301}"), 5); // cafe with combining accent
    }

    #[test]
    fn test_truncate_with_ellipsis() {
        assert_eq!(truncate_with_ellipsis("Hello, World!", 10), "Hello, ...");
        assert_eq!(truncate_with_ellipsis("Short", 10), "Short");
        assert_eq!(truncate_with_ellipsis("Exact len!", 10), "Exact len!");
        assert_eq!(truncate_with_ellipsis("AB", 3), "AB");
        assert_eq!(truncate_with_ellipsis("ABCD", 3), "ABC");
    }

    #[test]
    fn test_is_apple_recommended_locale() {
        assert!(is_apple_recommended_locale("en-US"));
        assert!(is_apple_recommended_locale("en_US"));
        assert!(is_apple_recommended_locale("de-DE"));
        assert!(!is_apple_recommended_locale("xy-ZZ"));
    }

    #[test]
    fn test_is_google_play_recommended_locale() {
        assert!(is_google_play_recommended_locale("en-US"));
        assert!(is_google_play_recommended_locale("ja-JP"));
        assert!(!is_google_play_recommended_locale("xy-ZZ"));
    }

    #[test]
    fn test_get_missing_locales() {
        let supported = vec!["en-US", "de-DE"];
        let missing_apple = get_missing_apple_locales(&supported);
        assert!(!missing_apple.contains(&"en-US"));
        assert!(!missing_apple.contains(&"de-DE"));
        assert!(missing_apple.contains(&"fr-FR"));

        let missing_google = get_missing_google_play_locales(&supported);
        assert!(!missing_google.contains(&"en-US"));
        assert!(missing_google.contains(&"ja-JP"));
    }

    #[test]
    fn test_sanitize_text() {
        // Normal text passes through
        assert_eq!(sanitize_text("Hello World"), "Hello World");

        // Newlines and tabs preserved
        assert_eq!(sanitize_text("Hello\nWorld\tTest"), "Hello\nWorld\tTest");

        // Various dashes normalized
        assert_eq!(sanitize_text("a\u{2013}b"), "a-b"); // en-dash
        assert_eq!(sanitize_text("a\u{2014}b"), "a-b"); // em-dash

        // Various quotes normalized
        assert_eq!(sanitize_text("\u{201C}Hello\u{201D}"), "\"Hello\""); // smart quotes
        assert_eq!(sanitize_text("\u{2018}test\u{2019}"), "'test'"); // smart single quotes
    }
}
