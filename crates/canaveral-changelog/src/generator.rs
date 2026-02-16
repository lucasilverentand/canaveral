//! Changelog generation

use std::collections::HashMap;

use canaveral_core::config::ChangelogConfig;
use canaveral_git::CommitInfo;
use tracing::{debug, info, instrument};

use crate::formatter::{ChangelogFormatter, MarkdownFormatter};
use crate::parser::{CommitParser, ConventionalParser};
use crate::types::{ChangelogEntry, ParsedCommit, Section};

/// Changelog generator
pub struct ChangelogGenerator {
    parser: Box<dyn CommitParser>,
    formatter: Box<dyn ChangelogFormatter>,
    config: ChangelogConfig,
}

impl ChangelogGenerator {
    /// Create a new generator with default parser and formatter
    pub fn new(config: ChangelogConfig) -> Self {
        Self {
            parser: Box::new(ConventionalParser::new()),
            formatter: Box::new(MarkdownFormatter::new()),
            config,
        }
    }

    /// Use a custom parser
    pub fn with_parser<P: CommitParser + 'static>(mut self, parser: P) -> Self {
        self.parser = Box::new(parser);
        self
    }

    /// Use a custom formatter
    pub fn with_formatter<F: ChangelogFormatter + 'static>(mut self, formatter: F) -> Self {
        self.formatter = Box::new(formatter);
        self
    }

    /// Generate a changelog entry from commits
    #[instrument(skip(self, commits), fields(commit_count = commits.len()))]
    pub fn generate(&self, version: &str, commits: &[CommitInfo]) -> ChangelogEntry {
        info!(version, commit_count = commits.len(), "generating changelog entry");
        let mut entry = ChangelogEntry::new(version);

        // Parse commits
        let parsed: Vec<ParsedCommit> = commits
            .iter()
            .filter_map(|c| self.parser.parse(c))
            .filter(|c| self.parser.should_include(c))
            .collect();

        // Group commits by type
        let mut grouped: HashMap<String, Vec<ParsedCommit>> = HashMap::new();
        let mut breaking = Vec::new();

        for commit in parsed {
            if commit.breaking {
                breaking.push(commit.clone());
            }

            grouped
                .entry(commit.commit_type.clone())
                .or_default()
                .push(commit);
        }

        // Create sections based on config
        for (commit_type, commits) in grouped {
            if let Some(type_config) = self.config.types.get(&commit_type) {
                if !type_config.hidden {
                    let mut section = Section::new(&type_config.section);
                    for commit in commits {
                        section.add_commit(commit);
                    }
                    entry.add_section(section);
                }
            } else {
                // Unknown type - add with default section name
                let section_name = match commit_type.as_str() {
                    "feat" => "Features",
                    "fix" => "Bug Fixes",
                    "docs" => "Documentation",
                    "perf" => "Performance",
                    _ => continue, // Skip unknown types
                };

                let mut section = Section::new(section_name);
                for commit in commits {
                    section.add_commit(commit);
                }
                entry.add_section(section);
            }
        }

        // Add breaking changes
        for commit in breaking {
            entry.add_breaking_change(commit);
        }

        debug!(
            section_count = entry.sections.len(),
            breaking_count = entry.breaking_changes.len(),
            "changelog sections built"
        );

        // Sort sections by a defined order
        entry.sections.sort_by(|a, b| {
            let order = |s: &str| match s {
                "Features" => 0,
                "Bug Fixes" => 1,
                "Performance" => 2,
                "Documentation" => 3,
                _ => 99,
            };
            order(&a.title).cmp(&order(&b.title))
        });

        entry
    }

    /// Format a changelog entry to string
    pub fn format(&self, entry: &ChangelogEntry) -> String {
        self.formatter.format(entry, &self.config)
    }

    /// Generate and format in one step
    #[instrument(skip(self, commits), fields(commit_count = commits.len()))]
    pub fn generate_formatted(&self, version: &str, commits: &[CommitInfo]) -> String {
        let entry = self.generate(version, commits);
        let output = self.format(&entry);
        debug!(output_len = output.len(), "changelog formatted");
        output
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
    fn test_generate_changelog() {
        let config = ChangelogConfig::default();
        let generator = ChangelogGenerator::new(config);

        let commits = vec![
            make_commit("feat: add new feature"),
            make_commit("fix: fix bug"),
            make_commit("chore: update deps"),
        ];

        let entry = generator.generate("1.0.0", &commits);

        assert_eq!(entry.version, "1.0.0");
        assert!(!entry.sections.is_empty());
    }

    #[test]
    fn test_breaking_changes() {
        let config = ChangelogConfig::default();
        let generator = ChangelogGenerator::new(config);

        let commits = vec![make_commit("feat!: breaking feature")];

        let entry = generator.generate("2.0.0", &commits);

        assert!(!entry.breaking_changes.is_empty());
    }

    #[test]
    fn test_format_changelog() {
        let config = ChangelogConfig::default();
        let generator = ChangelogGenerator::new(config);

        let commits = vec![
            make_commit("feat: add feature"),
            make_commit("fix: fix bug"),
        ];

        let formatted = generator.generate_formatted("1.0.0", &commits);

        assert!(formatted.contains("1.0.0"));
        assert!(formatted.contains("Features") || formatted.contains("feat"));
    }
}
