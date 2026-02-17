//! Human-readable release notes generation

use canaveral_core::config::ReleaseNotesConfig;
use canaveral_git::CommitInfo;
use tracing::{debug, info, instrument};

use crate::parser::{CommitParser, ConventionalParser};
use crate::types::ParsedCommit;

/// Generates structured, reader-friendly release notes from commits
pub struct ReleaseNotesGenerator {
    parser: Box<dyn CommitParser>,
    config: ReleaseNotesConfig,
}

impl ReleaseNotesGenerator {
    /// Create a new generator with default parser
    pub fn new(config: ReleaseNotesConfig) -> Self {
        Self {
            parser: Box::new(ConventionalParser::new()),
            config,
        }
    }

    /// Use a custom parser
    pub fn with_parser<P: CommitParser + 'static>(mut self, parser: P) -> Self {
        self.parser = Box::new(parser);
        self
    }

    /// Generate release notes from commits
    #[instrument(skip(self, commits), fields(commit_count = commits.len()))]
    pub fn generate(&self, version: &str, commits: &[CommitInfo]) -> ReleaseNotes {
        info!(
            version,
            commit_count = commits.len(),
            "generating release notes"
        );
        let parsed: Vec<ParsedCommit> = commits
            .iter()
            .filter_map(|c| self.parser.parse(c))
            .filter(|c| self.parser.should_include(c))
            .collect();

        let mut notes = ReleaseNotes {
            version: version.to_string(),
            headline: String::new(),
            breaking_changes: Vec::new(),
            features: Vec::new(),
            fixes: Vec::new(),
            other_changes: Vec::new(),
            contributors: Vec::new(),
            migration_guide: None,
        };

        // Categorize commits
        let mut contributors_set = std::collections::HashSet::new();
        let mut breaking_details = Vec::new();

        for commit in &parsed {
            contributors_set.insert(commit.author.clone());

            let entry = NoteEntry {
                description: commit.description.clone(),
                scope: commit.scope.clone(),
                hash: commit.hash.chars().take(7).collect(),
                author: commit.author.clone(),
            };

            if commit.breaking {
                let migration = commit
                    .body
                    .clone()
                    .or_else(|| {
                        commit
                            .footers
                            .iter()
                            .find(|f| f.token == "BREAKING CHANGE")
                            .map(|f| f.value.clone())
                    })
                    .unwrap_or_else(|| commit.description.clone());
                breaking_details.push(BreakingChange {
                    description: commit.description.clone(),
                    migration_guidance: migration,
                    scope: commit.scope.clone(),
                });
                notes.breaking_changes.push(entry);
            } else {
                match commit.commit_type.as_str() {
                    "feat" => notes.features.push(entry),
                    "fix" => notes.fixes.push(entry),
                    _ => {
                        if self.config.categorize {
                            notes.other_changes.push(entry);
                        }
                    }
                }
            }
        }

        // Generate headline
        notes.headline = self.generate_headline(&notes);

        // Collect contributors
        if self.config.include_contributors {
            notes.contributors = contributors_set.into_iter().collect();
            notes.contributors.sort();
        }

        // Generate migration guide
        if self.config.include_migration_guide && !breaking_details.is_empty() {
            notes.migration_guide = Some(self.generate_migration_guide(&breaking_details));
        }

        debug!(
            features = notes.features.len(),
            fixes = notes.fixes.len(),
            breaking = notes.breaking_changes.len(),
            contributors = notes.contributors.len(),
            "release notes generated"
        );

        notes
    }

    /// Format release notes as markdown
    #[instrument(skip(self, notes), fields(version = %notes.version))]
    pub fn format_markdown(&self, notes: &ReleaseNotes) -> String {
        let mut output = String::new();

        output.push_str(&format!("# Release {}\n\n", notes.version));

        if !notes.headline.is_empty() {
            output.push_str(&format!("{}\n\n", notes.headline));
        }

        // Breaking changes
        if !notes.breaking_changes.is_empty() {
            output.push_str("## Breaking Changes\n\n");
            for entry in &notes.breaking_changes {
                self.format_entry(&mut output, entry);
            }
            output.push('\n');
        }

        // Features
        if !notes.features.is_empty() {
            output.push_str("## New Features\n\n");
            for entry in &notes.features {
                self.format_entry(&mut output, entry);
            }
            output.push('\n');
        }

        // Bug fixes
        if !notes.fixes.is_empty() {
            output.push_str("## Bug Fixes\n\n");
            for entry in &notes.fixes {
                self.format_entry(&mut output, entry);
            }
            output.push('\n');
        }

        // Other changes
        if !notes.other_changes.is_empty() && self.config.categorize {
            output.push_str("## Other Changes\n\n");
            for entry in &notes.other_changes {
                self.format_entry(&mut output, entry);
            }
            output.push('\n');
        }

        // Migration guide
        if let Some(guide) = &notes.migration_guide {
            output.push_str("## Migration Guide\n\n");
            output.push_str(guide);
            output.push_str("\n\n");
        }

        // Contributors
        if !notes.contributors.is_empty() && self.config.include_contributors {
            output.push_str("## Contributors\n\n");
            for contributor in &notes.contributors {
                output.push_str(&format!("- {}\n", contributor));
            }
            output.push('\n');
        }

        output
    }

    /// Generate and format in one step
    #[instrument(skip(self, commits), fields(commit_count = commits.len()))]
    pub fn generate_formatted(&self, version: &str, commits: &[CommitInfo]) -> String {
        let notes = self.generate(version, commits);
        let output = self.format_markdown(&notes);
        debug!(output_len = output.len(), "release notes formatted");
        output
    }

    fn generate_headline(&self, notes: &ReleaseNotes) -> String {
        let mut parts = Vec::new();

        if !notes.breaking_changes.is_empty() {
            parts.push(format!(
                "{} breaking change{}",
                notes.breaking_changes.len(),
                if notes.breaking_changes.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        }
        if !notes.features.is_empty() {
            parts.push(format!(
                "{} new feature{}",
                notes.features.len(),
                if notes.features.len() == 1 { "" } else { "s" }
            ));
        }
        if !notes.fixes.is_empty() {
            parts.push(format!(
                "{} bug fix{}",
                notes.fixes.len(),
                if notes.fixes.len() == 1 { "" } else { "es" }
            ));
        }

        if parts.is_empty() {
            "Maintenance release.".to_string()
        } else {
            format!("This release includes {}.", parts.join(", "))
        }
    }

    fn format_entry(&self, output: &mut String, entry: &NoteEntry) {
        if let Some(scope) = &entry.scope {
            output.push_str(&format!(
                "- **{}**: {} ({})\n",
                scope, entry.description, entry.hash
            ));
        } else {
            output.push_str(&format!("- {} ({})\n", entry.description, entry.hash));
        }
    }

    fn generate_migration_guide(&self, changes: &[BreakingChange]) -> String {
        let mut guide = String::new();

        for (i, change) in changes.iter().enumerate() {
            if changes.len() > 1 {
                guide.push_str(&format!("### {}. ", i + 1));
            }

            if let Some(scope) = &change.scope {
                guide.push_str(&format!("{} ({})\n\n", change.description, scope));
            } else {
                guide.push_str(&format!("{}\n\n", change.description));
            }

            guide.push_str(&format!("{}\n\n", change.migration_guidance));
        }

        guide
    }
}

/// Structured release notes
#[derive(Debug, Clone)]
pub struct ReleaseNotes {
    /// Version string
    pub version: String,
    /// One-line summary
    pub headline: String,
    /// Breaking changes
    pub breaking_changes: Vec<NoteEntry>,
    /// New features
    pub features: Vec<NoteEntry>,
    /// Bug fixes
    pub fixes: Vec<NoteEntry>,
    /// Other notable changes
    pub other_changes: Vec<NoteEntry>,
    /// List of contributors
    pub contributors: Vec<String>,
    /// Migration guide for breaking changes
    pub migration_guide: Option<String>,
}

/// A single entry in the release notes
#[derive(Debug, Clone)]
pub struct NoteEntry {
    /// Description of the change
    pub description: String,
    /// Scope (component/module)
    pub scope: Option<String>,
    /// Short commit hash
    pub hash: String,
    /// Author name
    pub author: String,
}

/// A breaking change with migration guidance
#[derive(Debug, Clone)]
struct BreakingChange {
    description: String,
    migration_guidance: String,
    scope: Option<String>,
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
    fn test_generate_release_notes() {
        let config = ReleaseNotesConfig::default();
        let generator = ReleaseNotesGenerator::new(config);

        let commits = vec![
            make_commit("feat: add new login flow"),
            make_commit("fix: resolve crash on startup"),
            make_commit("feat(auth): add OAuth support"),
        ];

        let notes = generator.generate("1.2.0", &commits);

        assert_eq!(notes.version, "1.2.0");
        assert_eq!(notes.features.len(), 2);
        assert_eq!(notes.fixes.len(), 1);
        assert!(notes.breaking_changes.is_empty());
    }

    #[test]
    fn test_breaking_changes() {
        let config = ReleaseNotesConfig::default();
        let generator = ReleaseNotesGenerator::new(config);

        let commits = vec![make_commit("feat!: redesign API surface")];

        let notes = generator.generate("2.0.0", &commits);

        assert_eq!(notes.breaking_changes.len(), 1);
        assert!(notes.migration_guide.is_some());
    }

    #[test]
    fn test_format_markdown() {
        let config = ReleaseNotesConfig::default();
        let generator = ReleaseNotesGenerator::new(config);

        let commits = vec![
            make_commit("feat: add feature"),
            make_commit("fix: fix bug"),
        ];

        let formatted = generator.generate_formatted("1.0.0", &commits);

        assert!(formatted.contains("# Release 1.0.0"));
        assert!(formatted.contains("## New Features"));
        assert!(formatted.contains("## Bug Fixes"));
    }

    #[test]
    fn test_headline_generation() {
        let config = ReleaseNotesConfig::default();
        let generator = ReleaseNotesGenerator::new(config);

        let commits = vec![
            make_commit("feat: one feature"),
            make_commit("fix: one fix"),
            make_commit("fix: another fix"),
        ];

        let notes = generator.generate("1.0.0", &commits);

        assert!(notes.headline.contains("1 new feature"));
        assert!(notes.headline.contains("2 bug fixes"));
    }

    #[test]
    fn test_contributors() {
        let config = ReleaseNotesConfig {
            include_contributors: true,
            ..Default::default()
        };
        let generator = ReleaseNotesGenerator::new(config);

        let commits = vec![make_commit("feat: add feature")];

        let notes = generator.generate("1.0.0", &commits);

        assert!(!notes.contributors.is_empty());
        assert!(notes.contributors.contains(&"Test Author".to_string()));
    }

    #[test]
    fn test_empty_commits() {
        let config = ReleaseNotesConfig::default();
        let generator = ReleaseNotesGenerator::new(config);

        let notes = generator.generate("1.0.0", &[]);

        assert!(notes.features.is_empty());
        assert!(notes.fixes.is_empty());
        assert_eq!(notes.headline, "Maintenance release.");
    }
}
