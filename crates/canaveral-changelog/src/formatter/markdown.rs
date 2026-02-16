//! Markdown changelog formatter

use canaveral_core::config::ChangelogConfig;
use tracing::{debug, instrument};

use super::ChangelogFormatter;
use crate::types::ChangelogEntry;

/// Markdown changelog formatter
pub struct MarkdownFormatter {
    /// Include compare link between versions
    pub include_compare_link: bool,
    /// Repository URL for links
    pub repo_url: Option<String>,
}

impl MarkdownFormatter {
    /// Create a new markdown formatter
    pub fn new() -> Self {
        Self {
            include_compare_link: true,
            repo_url: None,
        }
    }

    /// Set repository URL for links
    pub fn with_repo_url(mut self, url: impl Into<String>) -> Self {
        self.repo_url = Some(url.into());
        self
    }
}

impl Default for MarkdownFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangelogFormatter for MarkdownFormatter {
    #[instrument(skip(self, entry, config), fields(version = %entry.version, section_count = entry.sections.len()))]
    fn format(&self, entry: &ChangelogEntry, config: &ChangelogConfig) -> String {
        let mut output = String::new();

        // Version header
        let date_str = entry.date.format("%Y-%m-%d").to_string();
        output.push_str(&format!("## [{}] - {}\n\n", entry.version, date_str));

        // Breaking changes
        if !entry.breaking_changes.is_empty() {
            output.push_str("### ⚠ BREAKING CHANGES\n\n");
            for commit in &entry.breaking_changes {
                output.push_str(&format!("- {}", commit.description));
                if let Some(scope) = &commit.scope {
                    output.push_str(&format!(" ({})", scope));
                }
                if config.include_hashes {
                    output.push_str(&format!(" ({})", &commit.hash[..7]));
                }
                output.push('\n');
            }
            output.push('\n');
        }

        // Sections
        for section in &entry.sections {
            if section.is_empty() {
                continue;
            }

            output.push_str(&format!("### {}\n\n", section.title));

            for commit in &section.commits {
                output.push_str(&format!("- {}", commit.description));

                if let Some(scope) = &commit.scope {
                    output.push_str(&format!(" ({})", scope));
                }

                if config.include_hashes {
                    let short_hash = &commit.hash[..7.min(commit.hash.len())];
                    if let Some(repo_url) = &self.repo_url {
                        output.push_str(&format!(
                            " ([{}]({}/commit/{}))",
                            short_hash, repo_url, commit.hash
                        ));
                    } else {
                        output.push_str(&format!(" ({})", short_hash));
                    }
                }

                if config.include_authors {
                    output.push_str(&format!(" - {}", commit.author));
                }

                output.push('\n');
            }

            output.push('\n');
        }

        // Notes
        if let Some(notes) = &entry.notes {
            output.push_str(&format!("### Notes\n\n{}\n\n", notes));
        }

        debug!(output_len = output.len(), "markdown changelog formatted");
        output
    }

    fn extension(&self) -> &'static str {
        "md"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ParsedCommit, Section};
    use chrono::Utc;

    #[test]
    fn test_format_basic() {
        let formatter = MarkdownFormatter::new();
        let config = ChangelogConfig::default();

        let mut entry = ChangelogEntry::new("1.0.0");

        let mut section = Section::new("Features");
        section.add_commit(ParsedCommit {
            hash: "abc1234567890".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            breaking: false,
            description: "add new feature".to_string(),
            body: None,
            footers: vec![],
            author: "Test".to_string(),
            timestamp: Utc::now(),
        });
        entry.add_section(section);

        let output = formatter.format(&entry, &config);

        assert!(output.contains("## [1.0.0]"));
        assert!(output.contains("### Features"));
        assert!(output.contains("add new feature"));
    }

    #[test]
    fn test_format_with_scope() {
        let formatter = MarkdownFormatter::new();
        let config = ChangelogConfig::default();

        let mut entry = ChangelogEntry::new("1.0.0");

        let mut section = Section::new("Bug Fixes");
        section.add_commit(ParsedCommit {
            hash: "def4567890abc".to_string(),
            commit_type: "fix".to_string(),
            scope: Some("parser".to_string()),
            breaking: false,
            description: "handle edge case".to_string(),
            body: None,
            footers: vec![],
            author: "Test".to_string(),
            timestamp: Utc::now(),
        });
        entry.add_section(section);

        let output = formatter.format(&entry, &config);

        assert!(output.contains("(parser)"));
    }

    #[test]
    fn test_format_breaking_changes() {
        let formatter = MarkdownFormatter::new();
        let config = ChangelogConfig::default();

        let mut entry = ChangelogEntry::new("2.0.0");
        entry.add_breaking_change(ParsedCommit {
            hash: "break123456789".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            breaking: true,
            description: "remove deprecated API".to_string(),
            body: None,
            footers: vec![],
            author: "Test".to_string(),
            timestamp: Utc::now(),
        });

        let output = formatter.format(&entry, &config);

        assert!(output.contains("BREAKING CHANGES"));
        assert!(output.contains("remove deprecated API"));
    }

    #[test]
    fn test_format_with_repo_url() {
        let formatter = MarkdownFormatter::new().with_repo_url("https://github.com/test/repo");
        let config = ChangelogConfig::default();

        let mut entry = ChangelogEntry::new("1.0.0");

        let mut section = Section::new("Features");
        section.add_commit(ParsedCommit {
            hash: "abc1234567890".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            breaking: false,
            description: "feature".to_string(),
            body: None,
            footers: vec![],
            author: "Test".to_string(),
            timestamp: Utc::now(),
        });
        entry.add_section(section);

        let output = formatter.format(&entry, &config);

        assert!(output.contains("https://github.com/test/repo/commit/"));
    }
}
