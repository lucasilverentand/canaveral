//! Framework detection system
//!
//! Automatically detects which framework a project uses based on file presence,
//! configuration files, and project structure.

use std::path::Path;

use tracing::{debug, instrument};

use crate::error::Result;
use crate::traits::BuildAdapter;

/// Detection result from an adapter
#[derive(Debug, Clone)]
pub enum Detection {
    /// Framework definitely not present
    No,
    /// Framework might be present with confidence 0-100
    Maybe(u8),
    /// Framework definitely present with confidence 0-100
    Yes(u8),
}

impl Detection {
    /// Create a confident detection (80-100)
    pub fn confident(confidence: u8) -> Self {
        Self::Yes(confidence.min(100))
    }

    /// Create a possible detection (40-79)
    pub fn possible(confidence: u8) -> Self {
        Self::Maybe(confidence.min(100))
    }

    /// Get the confidence score (0-100)
    pub fn confidence(&self) -> u8 {
        match self {
            Self::No => 0,
            Self::Maybe(c) | Self::Yes(c) => *c,
        }
    }

    /// Check if detected (Maybe or Yes)
    pub fn detected(&self) -> bool {
        !matches!(self, Self::No)
    }

    /// Check if confident (Yes with high confidence)
    pub fn is_confident(&self) -> bool {
        matches!(self, Self::Yes(c) if *c >= 80)
    }
}

impl PartialEq for Detection {
    fn eq(&self, other: &Self) -> bool {
        self.confidence() == other.confidence()
    }
}

impl Eq for Detection {}

impl PartialOrd for Detection {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Detection {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.confidence().cmp(&other.confidence())
    }
}

/// Framework detector that manages multiple adapters
pub struct FrameworkDetector {
    adapters: Vec<Box<dyn BuildAdapter>>,
}

impl FrameworkDetector {
    /// Create a new detector with the given adapters
    pub fn new(adapters: Vec<Box<dyn BuildAdapter>>) -> Self {
        Self { adapters }
    }

    /// Detect frameworks at the given path
    #[instrument(skip(self), fields(path = %path.display(), adapter_count = self.adapters.len()))]
    pub fn detect(&self, path: &Path) -> Vec<DetectionResult> {
        debug!(path = %path.display(), adapter_count = self.adapters.len(), "detecting frameworks");
        let mut results: Vec<_> = self
            .adapters
            .iter()
            .filter_map(|adapter| {
                let detection = adapter.detect(path);
                if detection.detected() {
                    Some(DetectionResult {
                        adapter_id: adapter.id().to_string(),
                        adapter_name: adapter.name().to_string(),
                        detection,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort by confidence (highest first)
        results.sort_by(|a, b| b.detection.cmp(&a.detection));
        results
    }

    /// Detect and return the best match, if any
    pub fn detect_best(&self, path: &Path) -> Option<DetectionResult> {
        self.detect(path).into_iter().next()
    }

    /// Detect and return the best match, or error if ambiguous
    pub fn detect_unambiguous(&self, path: &Path) -> Result<Option<DetectionResult>> {
        let results = self.detect(path);

        if results.is_empty() {
            return Ok(None);
        }

        if results.len() == 1 {
            return Ok(results.into_iter().next());
        }

        // Check if top result is significantly more confident than second
        let first = &results[0];
        let second = &results[1];

        if first.detection.confidence() >= second.detection.confidence() + 20 {
            return Ok(Some(results.into_iter().next().unwrap()));
        }

        // Ambiguous - multiple frameworks with similar confidence
        Err(crate::error::FrameworkError::AmbiguousFramework {
            frameworks: results.iter().map(|r| r.adapter_name.clone()).collect(),
        })
    }
}

/// Result of framework detection
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub adapter_id: String,
    pub adapter_name: String,
    pub detection: Detection,
}

// -----------------------------------------------------------------------------
// Detection helpers
// -----------------------------------------------------------------------------

/// Check if a file exists at the path
pub fn file_exists(path: &Path, filename: &str) -> bool {
    path.join(filename).exists()
}

/// Check if any of the files exist at the path
pub fn any_file_exists(path: &Path, filenames: &[&str]) -> bool {
    filenames.iter().any(|f| file_exists(path, f))
}

/// Check if all files exist at the path
pub fn all_files_exist(path: &Path, filenames: &[&str]) -> bool {
    filenames.iter().all(|f| file_exists(path, f))
}

/// Check if a directory exists at the path
pub fn dir_exists(path: &Path, dirname: &str) -> bool {
    path.join(dirname).is_dir()
}

/// Check if file contains a pattern
pub fn file_contains(path: &Path, filename: &str, pattern: &str) -> bool {
    let file_path = path.join(filename);
    if let Ok(content) = std::fs::read_to_string(file_path) {
        content.contains(pattern)
    } else {
        false
    }
}

/// Check if package.json has a dependency
pub fn has_npm_dependency(path: &Path, package: &str) -> bool {
    let package_json = path.join("package.json");
    if let Ok(content) = std::fs::read_to_string(package_json) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            // Check dependencies, devDependencies, peerDependencies
            for key in ["dependencies", "devDependencies", "peerDependencies"] {
                if let Some(deps) = json.get(key).and_then(|v| v.as_object()) {
                    if deps.contains_key(package) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check pubspec.yaml for a dependency
pub fn has_pubspec_dependency(path: &Path, package: &str) -> bool {
    let pubspec = path.join("pubspec.yaml");
    if let Ok(content) = std::fs::read_to_string(pubspec) {
        // Simple check - could use yaml parser for more accuracy
        content.contains(&format!("{}:", package)) || content.contains(&format!("{} :", package))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_confidence() {
        assert_eq!(Detection::No.confidence(), 0);
        assert_eq!(Detection::Maybe(50).confidence(), 50);
        assert_eq!(Detection::Yes(90).confidence(), 90);
    }

    #[test]
    fn test_detection_ordering() {
        let mut detections = vec![
            Detection::Maybe(40),
            Detection::Yes(90),
            Detection::No,
            Detection::Maybe(60),
        ];

        detections.sort();

        assert_eq!(detections[0].confidence(), 0);
        assert_eq!(detections[3].confidence(), 90);
    }

    #[test]
    fn test_detection_is_confident() {
        assert!(!Detection::No.is_confident());
        assert!(!Detection::Maybe(70).is_confident());
        assert!(!Detection::Yes(70).is_confident());
        assert!(Detection::Yes(80).is_confident());
        assert!(Detection::Yes(100).is_confident());
    }

    #[test]
    fn test_file_exists_helpers() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("pubspec.yaml"), "name: test").unwrap();
        std::fs::create_dir(temp.path().join("lib")).unwrap();

        assert!(file_exists(temp.path(), "pubspec.yaml"));
        assert!(!file_exists(temp.path(), "package.json"));
        assert!(dir_exists(temp.path(), "lib"));
        assert!(!dir_exists(temp.path(), "src"));
    }
}
