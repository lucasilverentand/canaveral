//! Git types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Information about a git commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    /// Commit hash (full)
    pub hash: String,
    /// Short hash (first 7 characters)
    pub short_hash: String,
    /// Commit message (first line)
    pub message: String,
    /// Full commit message body
    pub body: Option<String>,
    /// Author name
    pub author: String,
    /// Author email
    pub author_email: String,
    /// Commit timestamp
    pub timestamp: DateTime<Utc>,
}

impl CommitInfo {
    /// Create a new CommitInfo
    pub fn new(
        hash: impl Into<String>,
        message: impl Into<String>,
        author: impl Into<String>,
        author_email: impl Into<String>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        let hash = hash.into();
        let short_hash = hash.chars().take(7).collect();

        Self {
            hash,
            short_hash,
            message: message.into(),
            body: None,
            author: author.into(),
            author_email: author_email.into(),
            timestamp,
        }
    }

    /// Set the commit body
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Get the full message including body
    pub fn full_message(&self) -> String {
        match &self.body {
            Some(body) => format!("{}\n\n{}", self.message, body),
            None => self.message.clone(),
        }
    }
}

/// Information about a git tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    /// Tag name
    pub name: String,
    /// Commit hash the tag points to
    pub commit_hash: String,
    /// Tag message (for annotated tags)
    pub message: Option<String>,
    /// Tagger name (for annotated tags)
    pub tagger: Option<String>,
    /// Tag timestamp
    pub timestamp: Option<DateTime<Utc>>,
    /// Extracted version from tag name
    pub version: Option<String>,
}

impl TagInfo {
    /// Create a new TagInfo
    pub fn new(name: impl Into<String>, commit_hash: impl Into<String>) -> Self {
        let name = name.into();
        let version = extract_version(&name);

        Self {
            name,
            commit_hash: commit_hash.into(),
            message: None,
            tagger: None,
            timestamp: None,
            version,
        }
    }

    /// Set the tag message
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set the tagger
    pub fn with_tagger(mut self, tagger: impl Into<String>) -> Self {
        self.tagger = Some(tagger.into());
        self
    }

    /// Set the timestamp
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

/// Extract version from a tag name
fn extract_version(tag: &str) -> Option<String> {
    // Handle common tag formats: v1.0.0, 1.0.0, package@1.0.0, package-v1.0.0
    let tag = tag.strip_prefix('v').unwrap_or(tag);

    // Check for package@version format
    if let Some(pos) = tag.rfind('@') {
        let version_part = &tag[pos + 1..];
        let version = version_part.strip_prefix('v').unwrap_or(version_part);
        if looks_like_version(version) {
            return Some(version.to_string());
        }
    }

    // Check for package-vX.Y.Z format
    if let Some(pos) = tag.rfind("-v") {
        let version = &tag[pos + 2..];
        if looks_like_version(version) {
            return Some(version.to_string());
        }
    }

    // Check if the whole thing looks like a version
    if looks_like_version(tag) {
        return Some(tag.to_string());
    }

    None
}

/// Check if a string looks like a semantic version
fn looks_like_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 2 {
        return false;
    }

    // First part should be numeric
    parts[0].parse::<u64>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version() {
        assert_eq!(extract_version("v1.0.0"), Some("1.0.0".to_string()));
        assert_eq!(extract_version("1.0.0"), Some("1.0.0".to_string()));
        assert_eq!(extract_version("package@1.0.0"), Some("1.0.0".to_string()));
        assert_eq!(extract_version("package@v1.0.0"), Some("1.0.0".to_string()));
        assert_eq!(extract_version("pkg-v2.0.0"), Some("2.0.0".to_string()));
        assert_eq!(extract_version("not-a-version"), None);
    }

    #[test]
    fn test_commit_info() {
        let commit = CommitInfo::new(
            "abc1234567890",
            "feat: add feature",
            "Author",
            "author@example.com",
            Utc::now(),
        );
        assert_eq!(commit.short_hash, "abc1234");
        assert_eq!(commit.message, "feat: add feature");
    }
}
