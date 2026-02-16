//! Smart test selection — find minimal test set for changed code

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use regex::Regex;
use tracing::{debug, info};

/// Reason a test was selected
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionReason {
    /// Test file itself was changed
    DirectChange,
    /// Test imports a changed source file
    ImportsChangedFile(PathBuf),
    /// Package dependency changed
    PackageDependency(String),
    /// Fallback: run full suite (couldn't determine coverage)
    FullSuiteFallback,
}

impl std::fmt::Display for SelectionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DirectChange => write!(f, "file changed directly"),
            Self::ImportsChangedFile(path) => {
                write!(f, "imports changed file {}", path.display())
            }
            Self::PackageDependency(pkg) => {
                write!(f, "dependency '{}' changed", pkg)
            }
            Self::FullSuiteFallback => write!(f, "full suite (couldn't determine coverage)"),
        }
    }
}

/// A test that was selected to run
#[derive(Debug, Clone)]
pub struct SelectedTest {
    /// Package name
    pub package: String,
    /// Test file path (None = run all tests for package)
    pub test_file: Option<PathBuf>,
    /// Why this test was selected
    pub reason: SelectionReason,
}

/// Maps source files to test files via import analysis
#[derive(Debug, Default)]
pub struct TestMap {
    /// source file -> set of test files that import it
    source_to_tests: HashMap<PathBuf, HashSet<PathBuf>>,
}

impl TestMap {
    /// Build a test map by scanning source and test files
    pub fn build(package_dir: &Path, package_type: &str) -> Self {
        debug!(
            path = %package_dir.display(),
            package_type,
            "building test map"
        );
        let mut map = Self::default();

        match package_type {
            "cargo" => map.scan_rust(package_dir),
            "npm" => map.scan_javascript(package_dir),
            "python" => map.scan_python(package_dir),
            _ => {} // Unknown type, fall back to full suite
        }

        debug!(
            mappings = map.source_to_tests.len(),
            "test map built"
        );
        map
    }

    /// Find test files that cover the given source files
    pub fn find_tests(&self, changed_files: &[PathBuf]) -> Vec<(PathBuf, PathBuf)> {
        let mut results = Vec::new();

        for changed_file in changed_files {
            if let Some(test_files) = self.source_to_tests.get(changed_file) {
                for test_file in test_files {
                    results.push((test_file.clone(), changed_file.clone()));
                }
            }
        }

        results
    }

    /// Scan Rust source files for test relationships
    fn scan_rust(&mut self, dir: &Path) {
        let src_dir = dir.join("src");
        if !src_dir.exists() {
            return;
        }

        // In Rust, tests are typically in the same file (mod tests) or in tests/
        // For same-file tests: any change to the file means run that file's tests
        let test_dir = dir.join("tests");

        // Map src files to themselves (inline tests)
        if let Ok(entries) = glob::glob(&src_dir.join("**/*.rs").to_string_lossy()) {
            for entry in entries.flatten() {
                let relative = entry.strip_prefix(dir).unwrap_or(&entry).to_path_buf();
                self.source_to_tests
                    .entry(relative.clone())
                    .or_default()
                    .insert(relative);
            }
        }

        // Map src files to integration test files that use them
        if test_dir.exists() {
            if let Ok(test_entries) = glob::glob(&test_dir.join("**/*.rs").to_string_lossy()) {
                let use_re = Regex::new(r"use\s+(?:crate|super)::(\w+)").unwrap();

                for test_entry in test_entries.flatten() {
                    if let Ok(content) = std::fs::read_to_string(&test_entry) {
                        let relative_test = test_entry.strip_prefix(dir).unwrap_or(&test_entry).to_path_buf();

                        for cap in use_re.captures_iter(&content) {
                            if let Some(module_name) = cap.get(1) {
                                let module = module_name.as_str();
                                // Try to find the source file for this module
                                let candidates = vec![
                                    PathBuf::from(format!("src/{}.rs", module)),
                                    PathBuf::from(format!("src/{}/mod.rs", module)),
                                ];

                                for candidate in candidates {
                                    if dir.join(&candidate).exists() {
                                        self.source_to_tests
                                            .entry(candidate)
                                            .or_default()
                                            .insert(relative_test.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Scan JavaScript/TypeScript source files for test relationships
    fn scan_javascript(&mut self, dir: &Path) {
        let import_re = Regex::new(
            r#"(?:import\s+.*?from\s+['"]([^'"]+)['"]|require\s*\(\s*['"]([^'"]+)['"]\s*\))"#,
        )
        .unwrap();

        // Find test files
        let test_patterns = [
            "**/*.test.js",
            "**/*.test.ts",
            "**/*.test.tsx",
            "**/*.spec.js",
            "**/*.spec.ts",
            "**/*.spec.tsx",
            "__tests__/**/*.js",
            "__tests__/**/*.ts",
            "__tests__/**/*.tsx",
        ];

        let mut test_files = Vec::new();
        for pattern in &test_patterns {
            let full = dir.join(pattern).to_string_lossy().to_string();
            if let Ok(paths) = glob::glob(&full) {
                for path in paths.flatten() {
                    test_files.push(path);
                }
            }
        }

        // For each test file, find its imports
        for test_path in &test_files {
            if let Ok(content) = std::fs::read_to_string(test_path) {
                let test_dir_path = test_path.parent().unwrap_or(dir);
                let relative_test = test_path.strip_prefix(dir).unwrap_or(test_path).to_path_buf();

                for cap in import_re.captures_iter(&content) {
                    let import_path = cap
                        .get(1)
                        .or_else(|| cap.get(2))
                        .map(|m| m.as_str())
                        .unwrap_or("");

                    // Only resolve relative imports
                    if import_path.starts_with('.') {
                        let resolved = test_dir_path.join(import_path);
                        // Try common extensions
                        let extensions = ["", ".js", ".ts", ".tsx", ".jsx", "/index.js", "/index.ts"];
                        for ext in &extensions {
                            let candidate = PathBuf::from(format!("{}{}", resolved.display(), ext));
                            if candidate.exists() {
                                let relative_src = candidate
                                    .strip_prefix(dir)
                                    .unwrap_or(&candidate)
                                    .to_path_buf();
                                self.source_to_tests
                                    .entry(relative_src)
                                    .or_default()
                                    .insert(relative_test.clone());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Scan Python source files for test relationships
    fn scan_python(&mut self, dir: &Path) {
        let import_re = Regex::new(r"(?:from\s+(\S+)\s+import|import\s+(\S+))").unwrap();

        // Find test files
        let test_patterns = ["**/test_*.py", "**/*_test.py", "tests/**/*.py"];

        let mut test_files = Vec::new();
        for pattern in &test_patterns {
            let full = dir.join(pattern).to_string_lossy().to_string();
            if let Ok(paths) = glob::glob(&full) {
                for path in paths.flatten() {
                    test_files.push(path);
                }
            }
        }

        // For each test file, find its imports
        for test_path in &test_files {
            if let Ok(content) = std::fs::read_to_string(test_path) {
                let relative_test = test_path.strip_prefix(dir).unwrap_or(test_path).to_path_buf();

                for cap in import_re.captures_iter(&content) {
                    let module = cap
                        .get(1)
                        .or_else(|| cap.get(2))
                        .map(|m| m.as_str())
                        .unwrap_or("");

                    // Convert module path to file path
                    let module_path = module.replace('.', "/");
                    let candidates = vec![
                        PathBuf::from(format!("{}.py", module_path)),
                        PathBuf::from(format!("{}/__init__.py", module_path)),
                        PathBuf::from(format!("src/{}.py", module_path)),
                        PathBuf::from(format!("src/{}/__init__.py", module_path)),
                    ];

                    for candidate in candidates {
                        if dir.join(&candidate).exists() {
                            self.source_to_tests
                                .entry(candidate)
                                .or_default()
                                .insert(relative_test.clone());
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Smart test selector
pub struct TestSelector;

impl TestSelector {
    /// Select tests to run based on changed files
    pub fn select(
        packages: &[(String, PathBuf, String)], // (name, path, type)
        changed_files: &HashMap<String, Vec<PathBuf>>, // package -> changed files
        dependency_changes: &HashSet<String>, // packages changed via dependencies
    ) -> Vec<SelectedTest> {
        info!(
            packages = packages.len(),
            changed_packages = changed_files.len(),
            dependency_changes = dependency_changes.len(),
            "selecting tests"
        );
        let mut selected = Vec::new();

        for (pkg_name, pkg_path, pkg_type) in packages {
            // Check if this package has direct file changes
            if let Some(files) = changed_files.get(pkg_name) {
                let test_map = TestMap::build(pkg_path, pkg_type);
                let test_matches = test_map.find_tests(files);

                if test_matches.is_empty() {
                    // Changed files but no specific test mapping — check if any changed
                    // file is a test file itself
                    let has_test_files = files.iter().any(|f| is_test_file(f, pkg_type));

                    if has_test_files {
                        for file in files {
                            if is_test_file(file, pkg_type) {
                                selected.push(SelectedTest {
                                    package: pkg_name.clone(),
                                    test_file: Some(file.clone()),
                                    reason: SelectionReason::DirectChange,
                                });
                            }
                        }
                    } else {
                        // Can't determine which tests — run full suite
                        selected.push(SelectedTest {
                            package: pkg_name.clone(),
                            test_file: None,
                            reason: SelectionReason::FullSuiteFallback,
                        });
                    }
                } else {
                    for (test_file, changed_file) in test_matches {
                        selected.push(SelectedTest {
                            package: pkg_name.clone(),
                            test_file: Some(test_file),
                            reason: SelectionReason::ImportsChangedFile(changed_file),
                        });
                    }
                }
            } else if dependency_changes.contains(pkg_name) {
                // Package itself didn't change, but a dependency did
                selected.push(SelectedTest {
                    package: pkg_name.clone(),
                    test_file: None,
                    reason: SelectionReason::PackageDependency(pkg_name.clone()),
                });
            }
        }

        // Deduplicate
        let mut seen = HashSet::new();
        selected.retain(|t| {
            let key = (t.package.clone(), t.test_file.clone());
            seen.insert(key)
        });

        info!(selected_count = selected.len(), "tests selected");
        selected
    }
}

/// Check if a file is a test file based on naming conventions
fn is_test_file(path: &Path, package_type: &str) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let path_str = path.to_string_lossy();

    match package_type {
        "cargo" => {
            path_str.contains("/tests/") || path_str.starts_with("tests/") || name == "tests.rs"
        }
        "npm" => {
            name.contains(".test.") || name.contains(".spec.") || path_str.contains("__tests__")
        }
        "python" => name.starts_with("test_") || name.ends_with("_test.py"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_reason_display() {
        assert_eq!(SelectionReason::DirectChange.to_string(), "file changed directly");
        assert_eq!(
            SelectionReason::PackageDependency("core".to_string()).to_string(),
            "dependency 'core' changed"
        );
    }

    #[test]
    fn test_is_test_file_rust() {
        assert!(is_test_file(Path::new("tests/integration.rs"), "cargo"));
        assert!(is_test_file(Path::new("src/tests.rs"), "cargo"));
        assert!(!is_test_file(Path::new("src/lib.rs"), "cargo"));
    }

    #[test]
    fn test_is_test_file_js() {
        assert!(is_test_file(Path::new("src/app.test.js"), "npm"));
        assert!(is_test_file(Path::new("src/app.spec.ts"), "npm"));
        assert!(is_test_file(
            Path::new("__tests__/app.js"),
            "npm"
        ));
        assert!(!is_test_file(Path::new("src/app.js"), "npm"));
    }

    #[test]
    fn test_is_test_file_python() {
        assert!(is_test_file(Path::new("test_app.py"), "python"));
        assert!(is_test_file(Path::new("app_test.py"), "python"));
        assert!(!is_test_file(Path::new("app.py"), "python"));
    }

    #[test]
    fn test_empty_selection() {
        let packages: Vec<(String, PathBuf, String)> = vec![];
        let changed_files = HashMap::new();
        let dep_changes = HashSet::new();

        let selected = TestSelector::select(&packages, &changed_files, &dep_changes);
        assert!(selected.is_empty());
    }

    #[test]
    fn test_dependency_change_selection() {
        let packages = vec![(
            "app".to_string(),
            PathBuf::from("/tmp/nonexistent/app"),
            "npm".to_string(),
        )];
        let changed_files = HashMap::new();
        let mut dep_changes = HashSet::new();
        dep_changes.insert("app".to_string());

        let selected = TestSelector::select(&packages, &changed_files, &dep_changes);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].package, "app");
        assert!(matches!(
            selected[0].reason,
            SelectionReason::PackageDependency(_)
        ));
    }
}
