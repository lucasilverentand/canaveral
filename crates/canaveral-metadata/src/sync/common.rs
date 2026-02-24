//! Shared utilities for metadata sync adapters.
//!
//! Contains helpers that are used by both Apple and Google Play sync
//! implementations to avoid duplication.

use crate::{Locale, Result};
use chrono::{Duration, Utc};
use std::sync::RwLock;
use tracing::warn;

/// Default retry delay in milliseconds.
pub const DEFAULT_RETRY_DELAY_MS: u64 = 1000;

/// Default maximum number of retries for rate-limited requests.
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Compare two optional strings, returning true if they differ.
///
/// Trims whitespace before comparing. Empty strings and None are
/// considered equivalent.
pub fn strings_differ(local: Option<&str>, remote: Option<&str>) -> bool {
    match (local, remote) {
        (Some(l), Some(r)) => l.trim() != r.trim(),
        (Some(l), None) => !l.trim().is_empty(),
        (None, Some(r)) => !r.trim().is_empty(),
        (None, None) => false,
    }
}

/// Compare two required strings, returning true if they differ.
///
/// Trims whitespace before comparing.
pub fn strings_differ_required(local: &str, remote: &str) -> bool {
    local.trim() != remote.trim()
}

/// Parse a locale string (e.g. "en-US", "de-DE") into a [`Locale`].
pub fn parse_locale(locale_str: &str) -> Result<Locale> {
    Locale::new(locale_str)
}

/// Generic token cache with expiry tracking.
///
/// Used by both Apple (JWT) and Google Play (OAuth2 access token) sync
/// implementations to cache authentication tokens with a safety buffer
/// before expiration.
pub struct TokenCache {
    inner: RwLock<Option<CachedToken>>,
}

struct CachedToken {
    token: String,
    expires_at: chrono::DateTime<Utc>,
}

impl TokenCache {
    /// Create a new empty token cache.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(None),
        }
    }

    /// Get a cached token if it's still valid.
    ///
    /// Returns `None` if no token is cached or if the cached token
    /// will expire within the given buffer duration.
    pub fn get(&self, buffer: Duration) -> Option<String> {
        let cache = self.inner.read().unwrap();
        cache.as_ref().and_then(|cached| {
            if Utc::now() < cached.expires_at - buffer {
                Some(cached.token.clone())
            } else {
                None
            }
        })
    }

    /// Store a token with its expiration time.
    pub fn set(&self, token: String, expires_at: chrono::DateTime<Utc>) {
        let mut cache = self.inner.write().unwrap();
        *cache = Some(CachedToken { token, expires_at });
    }
}

impl Default for TokenCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the `Retry-After` header value from an HTTP response,
/// falling back to a default delay.
pub fn parse_retry_after(headers: &reqwest::header::HeaderMap, default_seconds: u64) -> u64 {
    headers
        .get("Retry-After")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default_seconds)
}

/// Log a rate-limit warning with retry count.
pub fn log_rate_limit_warning(retry_after: u64, attempt: u32, max_retries: u32) {
    warn!(
        "Rate limited, waiting {} seconds before retry ({}/{})",
        retry_after,
        attempt + 1,
        max_retries
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strings_differ_both_none() {
        assert!(!strings_differ(None, None));
    }

    #[test]
    fn test_strings_differ_empty_vs_none() {
        assert!(!strings_differ(Some(""), None));
        assert!(!strings_differ(None, Some("")));
    }

    #[test]
    fn test_strings_differ_equal() {
        assert!(!strings_differ(Some("hello"), Some("hello")));
    }

    #[test]
    fn test_strings_differ_whitespace_trimming() {
        assert!(!strings_differ(Some(" hello "), Some("hello")));
    }

    #[test]
    fn test_strings_differ_actual_difference() {
        assert!(strings_differ(Some("hello"), Some("world")));
        assert!(strings_differ(Some("hello"), None));
        assert!(strings_differ(None, Some("world")));
    }

    #[test]
    fn test_strings_differ_required_equal() {
        assert!(!strings_differ_required("hello", "hello"));
        assert!(!strings_differ_required(" hello ", "hello"));
    }

    #[test]
    fn test_strings_differ_required_different() {
        assert!(strings_differ_required("hello", "world"));
    }

    #[test]
    fn test_token_cache_empty() {
        let cache = TokenCache::new();
        assert!(cache.get(Duration::minutes(5)).is_none());
    }

    #[test]
    fn test_token_cache_set_and_get() {
        let cache = TokenCache::new();
        let expires_at = Utc::now() + Duration::hours(1);
        cache.set("my-token".to_string(), expires_at);

        assert_eq!(
            cache.get(Duration::minutes(5)),
            Some("my-token".to_string())
        );
    }

    #[test]
    fn test_token_cache_expired() {
        let cache = TokenCache::new();
        // Token that expired 1 minute ago
        let expires_at = Utc::now() - Duration::minutes(1);
        cache.set("expired-token".to_string(), expires_at);

        assert!(cache.get(Duration::minutes(5)).is_none());
    }

    #[test]
    fn test_token_cache_within_buffer() {
        let cache = TokenCache::new();
        // Token expires in 3 minutes, but buffer is 5 minutes
        let expires_at = Utc::now() + Duration::minutes(3);
        cache.set("almost-expired".to_string(), expires_at);

        assert!(cache.get(Duration::minutes(5)).is_none());
    }

    #[test]
    fn test_parse_locale_valid() {
        assert!(parse_locale("en-US").is_ok());
        assert!(parse_locale("de-DE").is_ok());
        assert!(parse_locale("ja").is_ok());
    }
}
