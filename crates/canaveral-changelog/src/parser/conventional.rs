//! Conventional Commits parser
//!
//! Parses commits following the Conventional Commits specification:
//! https://www.conventionalcommits.org/

use regex::Regex;
use std::sync::LazyLock;

use super::{CommitParser, ParserConfig};
use crate::types::{Footer, ParsedCommit};
use canaveral_git::CommitInfo;

/// Regex for parsing conventional commit messages
static CONVENTIONAL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<type>[a-zA-Z]+)(?:\((?P<scope>[^)]+)\))?(?P<breaking>!)?: (?P<description>.+)$",
    )
    .expect("Invalid regex")
});

/// Regex for parsing footer lines
static FOOTER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?P<token>[A-Za-z-]+|BREAKING CHANGE): (?P<value>.+)$").expect("Invalid regex")
});

/// Parser for Conventional Commits format
pub struct ConventionalParser {
    config: ParserConfig,
}

impl ConventionalParser {
    /// Create a new parser with default configuration
    pub fn new() -> Self {
        Self {
            config: ParserConfig::default(),
        }
    }

    /// Create a parser with custom configuration
    pub fn with_config(config: ParserConfig) -> Self {
        Self { config }
    }

    /// Parse a commit message
    fn parse_message(&self, message: &str, body: Option<&str>) -> Option<ParsedMessage> {
        let caps = CONVENTIONAL_REGEX.captures(message)?;

        let commit_type = caps.name("type")?.as_str().to_lowercase();
        let scope = caps.name("scope").map(|m| m.as_str().to_string());
        let breaking_marker = caps.name("breaking").is_some();
        let description = caps.name("description")?.as_str().to_string();

        // Parse footers from body
        let (body_text, footers) = if let Some(body) = body {
            self.parse_body(body)
        } else {
            (None, Vec::new())
        };

        // Check for breaking change in footers
        let breaking_in_footer = footers.iter().any(|f| {
            f.token.eq_ignore_ascii_case("BREAKING CHANGE")
                || f.token.eq_ignore_ascii_case("BREAKING-CHANGE")
        });

        Some(ParsedMessage {
            commit_type,
            scope,
            breaking: breaking_marker || breaking_in_footer,
            description,
            body: body_text,
            footers,
        })
    }

    /// Parse the body and extract footers
    fn parse_body(&self, body: &str) -> (Option<String>, Vec<Footer>) {
        let mut footers = Vec::new();
        let mut body_lines = Vec::new();
        let mut in_footer = false;

        for line in body.lines() {
            if let Some(caps) = FOOTER_REGEX.captures(line) {
                in_footer = true;
                footers.push(Footer {
                    token: caps.name("token").unwrap().as_str().to_string(),
                    value: caps.name("value").unwrap().as_str().to_string(),
                });
            } else if in_footer && line.starts_with(' ') {
                // Continuation of previous footer
                if let Some(last) = footers.last_mut() {
                    last.value.push('\n');
                    last.value.push_str(line.trim());
                }
            } else if !in_footer {
                body_lines.push(line);
            }
        }

        let body_text = if body_lines.is_empty() {
            None
        } else {
            Some(body_lines.join("\n").trim().to_string())
        };

        (body_text, footers)
    }
}

impl Default for ConventionalParser {
    fn default() -> Self {
        Self::new()
    }
}

struct ParsedMessage {
    commit_type: String,
    scope: Option<String>,
    breaking: bool,
    description: String,
    body: Option<String>,
    footers: Vec<Footer>,
}

impl CommitParser for ConventionalParser {
    fn parse(&self, commit: &CommitInfo) -> Option<ParsedCommit> {
        // Skip merge commits if configured
        if !self.config.include_merges && commit.message.starts_with("Merge ") {
            return None;
        }

        let parsed = self.parse_message(&commit.message, commit.body.as_deref())?;

        Some(ParsedCommit {
            hash: commit.hash.clone(),
            commit_type: parsed.commit_type,
            scope: parsed.scope,
            breaking: parsed.breaking,
            description: parsed.description,
            body: parsed.body,
            footers: parsed.footers,
            author: commit.author.clone(),
            timestamp: commit.timestamp,
        })
    }

    fn should_include(&self, commit: &ParsedCommit) -> bool {
        // Check exclusions first
        if self.config.exclude_types.contains(&commit.commit_type) {
            return false;
        }

        // If include_types is empty, include everything not excluded
        // If include_types is non-empty, only include those types
        if self.config.include_types.is_empty() {
            true
        } else {
            self.config.include_types.contains(&commit.commit_type)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_commit(message: &str) -> CommitInfo {
        CommitInfo::new(
            "abc1234567890",
            message,
            "Test Author",
            "test@example.com",
            Utc::now(),
        )
    }

    #[test]
    fn test_parse_simple_feat() {
        let parser = ConventionalParser::new();
        let commit = make_commit("feat: add new feature");
        let parsed = parser.parse(&commit).unwrap();

        assert_eq!(parsed.commit_type, "feat");
        assert_eq!(parsed.description, "add new feature");
        assert!(parsed.scope.is_none());
        assert!(!parsed.breaking);
    }

    #[test]
    fn test_parse_with_scope() {
        let parser = ConventionalParser::new();
        let commit = make_commit("fix(parser): handle edge case");
        let parsed = parser.parse(&commit).unwrap();

        assert_eq!(parsed.commit_type, "fix");
        assert_eq!(parsed.scope, Some("parser".to_string()));
        assert_eq!(parsed.description, "handle edge case");
    }

    #[test]
    fn test_parse_breaking_change_marker() {
        let parser = ConventionalParser::new();
        let commit = make_commit("feat!: breaking change");
        let parsed = parser.parse(&commit).unwrap();

        assert!(parsed.breaking);
    }

    #[test]
    fn test_parse_breaking_with_scope() {
        let parser = ConventionalParser::new();
        let commit = make_commit("refactor(core)!: major refactoring");
        let parsed = parser.parse(&commit).unwrap();

        assert_eq!(parsed.commit_type, "refactor");
        assert_eq!(parsed.scope, Some("core".to_string()));
        assert!(parsed.breaking);
    }

    #[test]
    fn test_parse_non_conventional() {
        let parser = ConventionalParser::new();
        let commit = make_commit("Just a regular commit message");
        let parsed = parser.parse(&commit);

        assert!(parsed.is_none());
    }

    #[test]
    fn test_parse_with_body() {
        let parser = ConventionalParser::new();
        let mut commit = make_commit("feat: add feature");
        commit.body = Some("This is the body\n\nWith multiple paragraphs.".to_string());

        let parsed = parser.parse(&commit).unwrap();
        assert!(parsed.body.is_some());
    }

    #[test]
    fn test_parse_with_footer() {
        let parser = ConventionalParser::new();
        let mut commit = make_commit("feat: add feature");
        commit.body = Some("Body text\n\nRefs: #123\nFixes: #456".to_string());

        let parsed = parser.parse(&commit).unwrap();
        assert_eq!(parsed.footers.len(), 2);
        assert_eq!(parsed.footers[0].token, "Refs");
        assert_eq!(parsed.footers[0].value, "#123");
    }

    #[test]
    fn test_breaking_change_footer() {
        let parser = ConventionalParser::new();
        let mut commit = make_commit("feat: add feature");
        commit.body = Some("BREAKING CHANGE: This breaks everything".to_string());

        let parsed = parser.parse(&commit).unwrap();
        assert!(parsed.breaking);
    }

    #[test]
    fn test_should_include_with_excludes() {
        let parser = ConventionalParser::with_config(
            ParserConfig::default().exclude_type("chore"),
        );

        let mut commit = make_commit("feat: feature");
        let parsed = parser.parse(&commit).unwrap();
        assert!(parser.should_include(&parsed));

        commit = make_commit("chore: cleanup");
        let parsed = parser.parse(&commit).unwrap();
        assert!(!parser.should_include(&parsed));
    }
}
