//! Package detection

use std::path::Path;

use tracing::{debug, instrument};

use canaveral_core::error::Result;
use canaveral_core::types::PackageInfo;

use crate::registry::AdapterRegistry;

/// Detect packages in a directory
#[instrument(skip_all, fields(path = %path.display()))]
pub fn detect_packages(path: &Path) -> Result<Vec<PackageInfo>> {
    let registry = AdapterRegistry::new();
    let packages = detect_packages_with_registry(path, &registry)?;
    debug!(count = packages.len(), "detected packages");
    Ok(packages)
}

/// Detect packages using a custom registry
pub fn detect_packages_with_registry(
    path: &Path,
    registry: &AdapterRegistry,
) -> Result<Vec<PackageInfo>> {
    let mut packages = Vec::new();

    for adapter in registry.all() {
        if adapter.detect(path) {
            if let Ok(info) = adapter.get_info(path) {
                packages.push(info);
            }
        }
    }

    Ok(packages)
}

/// Detect packages recursively in a directory tree
#[instrument(skip_all, fields(path = %path.display(), max_depth))]
pub fn detect_packages_recursive(path: &Path, max_depth: usize) -> Result<Vec<PackageInfo>> {
    let registry = AdapterRegistry::new();
    let mut packages = Vec::new();

    detect_recursive_inner(path, &registry, 0, max_depth, &mut packages)?;

    debug!(count = packages.len(), "detected packages recursively");
    Ok(packages)
}

fn detect_recursive_inner(
    path: &Path,
    registry: &AdapterRegistry,
    current_depth: usize,
    max_depth: usize,
    packages: &mut Vec<PackageInfo>,
) -> Result<()> {
    if current_depth > max_depth {
        return Ok(());
    }

    // Check current directory
    if let Ok(found) = detect_packages_with_registry(path, registry) {
        packages.extend(found);
    }

    // Recurse into subdirectories
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                // Skip common non-package directories
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.')
                    || name_str == "node_modules"
                    || name_str == "target"
                    || name_str == "__pycache__"
                    || name_str == "venv"
                    || name_str == ".venv"
                    || name_str == "dist"
                    || name_str == "build"
                {
                    continue;
                }

                detect_recursive_inner(
                    &entry_path,
                    registry,
                    current_depth + 1,
                    max_depth,
                    packages,
                )?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_empty_dir() {
        let temp = TempDir::new().unwrap();
        let packages = detect_packages(temp.path()).unwrap();
        assert!(packages.is_empty());
    }

    #[test]
    fn test_detect_npm_package() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test", "version": "1.0.0"}"#,
        )
        .unwrap();

        let packages = detect_packages(temp.path()).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "test");
        assert_eq!(packages[0].package_type, "npm");
    }

    #[test]
    fn test_detect_cargo_package() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            r#"
[package]
name = "test-crate"
version = "0.1.0"
"#,
        )
        .unwrap();

        let packages = detect_packages(temp.path()).unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "test-crate");
        assert_eq!(packages[0].package_type, "cargo");
    }
}
