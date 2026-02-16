//! Tag operations

use chrono::{TimeZone, Utc};
use regex::Regex;
use semver::Version;
use tracing::{debug, info, instrument};

use crate::repository::{GitRepo, Result};
use crate::types::TagInfo;
use canaveral_core::error::GitError;

impl GitRepo {
    /// Get all tags
    #[instrument(skip(self))]
    pub fn tags(&self) -> Result<Vec<TagInfo>> {
        let mut tags = Vec::new();

        self.repo.tag_foreach(|oid, name| {
            let name = String::from_utf8_lossy(name)
                .trim_start_matches("refs/tags/")
                .to_string();

            // Try to get the commit this tag points to
            if let Ok(commit) = self.repo.find_commit(oid) {
                tags.push(TagInfo::new(&name, commit.id().to_string()));
            } else if let Ok(tag) = self.repo.find_tag(oid) {
                // Annotated tag
                let target_id = tag.target_id();
                let mut tag_info = TagInfo::new(&name, target_id.to_string());

                if let Some(msg) = tag.message() {
                    tag_info = tag_info.with_message(msg);
                }

                if let Some(tagger) = tag.tagger() {
                    if let Some(name) = tagger.name() {
                        tag_info = tag_info.with_tagger(name);
                    }
                    let timestamp = Utc
                        .timestamp_opt(tagger.when().seconds(), 0)
                        .single()
                        .unwrap_or_else(Utc::now);
                    tag_info = tag_info.with_timestamp(timestamp);
                }

                tags.push(tag_info);
            }

            true
        })?;

        debug!(count = tags.len(), "listed all tags");
        Ok(tags)
    }

    /// Get tags matching a pattern
    pub fn tags_matching(&self, pattern: &str) -> Result<Vec<TagInfo>> {
        let regex = Regex::new(pattern).map_err(|e| GitError::NoTags(e.to_string()))?;

        let all_tags = self.tags()?;
        let matching: Vec<_> = all_tags
            .into_iter()
            .filter(|t| regex.is_match(&t.name))
            .collect();

        Ok(matching)
    }

    /// Find the latest tag by semantic version
    #[instrument(skip(self), fields(pattern))]
    pub fn find_latest_tag(&self, pattern: Option<&str>) -> Result<Option<TagInfo>> {
        let tags = match pattern {
            Some(p) => self.tags_matching(p)?,
            None => self.tags()?,
        };

        // Filter to tags with valid versions and sort by version
        let mut versioned_tags: Vec<_> = tags
            .into_iter()
            .filter_map(|t| {
                t.version
                    .as_ref()
                    .and_then(|v| Version::parse(v).ok())
                    .map(|v| (t, v))
            })
            .collect();

        versioned_tags.sort_by(|a, b| b.1.cmp(&a.1));

        let result = versioned_tags.into_iter().next().map(|(t, _)| t);
        debug!(latest = ?result.as_ref().map(|t| &t.name), "found latest tag");
        Ok(result)
    }

    /// Find a specific tag by name
    pub fn find_tag(&self, name: &str) -> Result<Option<TagInfo>> {
        let tag_ref = format!("refs/tags/{}", name);

        match self.repo.find_reference(&tag_ref) {
            Ok(reference) => {
                let target = reference.peel_to_commit()?;
                Ok(Some(TagInfo::new(name, target.id().to_string())))
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(e) => Err(GitError::Git2(e)),
        }
    }

    /// Create a lightweight tag
    #[instrument(skip(self), fields(name, annotated = message.is_some()))]
    pub fn create_tag(&self, name: &str, message: Option<&str>) -> Result<TagInfo> {
        // Check if tag already exists
        if self.find_tag(name)?.is_some() {
            return Err(GitError::TagExists(name.to_string()));
        }

        let head = self.head_commit()?;

        if let Some(msg) = message {
            // Create annotated tag
            let sig = self.repo.signature()?;
            self.repo
                .tag(name, head.as_object(), &sig, msg, false)?;
        } else {
            // Create lightweight tag
            self.repo.tag_lightweight(name, head.as_object(), false)?;
        }

        info!(name, annotated = message.is_some(), "created tag");
        Ok(TagInfo::new(name, head.id().to_string()))
    }

    /// Delete a tag
    #[instrument(skip(self), fields(name))]
    pub fn delete_tag(&self, name: &str) -> Result<()> {
        self.repo.tag_delete(name)?;
        info!(name, "deleted tag");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use std::path::Path;
    use tempfile::TempDir;

    fn setup_repo_with_tag() -> (TempDir, GitRepo) {
        let temp = TempDir::new().unwrap();
        let repo = Repository::init(temp.path()).unwrap();

        // Create initial commit
        let sig = Signature::now("Test", "test@example.com").unwrap();

        std::fs::write(temp.path().join("file.txt"), "content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        // Create a tag
        let commit = repo.find_commit(oid).unwrap();
        repo.tag_lightweight("v1.0.0", commit.as_object(), false)
            .unwrap();

        let git_repo = GitRepo::open(temp.path()).unwrap();
        (temp, git_repo)
    }

    #[test]
    fn test_list_tags() {
        let (_temp, repo) = setup_repo_with_tag();
        let tags = repo.tags().unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v1.0.0");
    }

    #[test]
    fn test_find_tag() {
        let (_temp, repo) = setup_repo_with_tag();
        let tag = repo.find_tag("v1.0.0").unwrap();
        assert!(tag.is_some());
        assert_eq!(tag.unwrap().version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_find_latest_tag() {
        let (_temp, repo) = setup_repo_with_tag();
        let tag = repo.find_latest_tag(None).unwrap();
        assert!(tag.is_some());
    }

    #[test]
    fn test_create_tag() {
        let (_temp, repo) = setup_repo_with_tag();
        let tag = repo.create_tag("v2.0.0", Some("Release 2.0")).unwrap();
        assert_eq!(tag.name, "v2.0.0");
    }

    #[test]
    fn test_tag_already_exists() {
        let (_temp, repo) = setup_repo_with_tag();
        let result = repo.create_tag("v1.0.0", None);
        assert!(matches!(result, Err(GitError::TagExists(_))));
    }
}
