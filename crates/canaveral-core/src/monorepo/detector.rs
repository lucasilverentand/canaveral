//! Workspace detector trait and registry for pluggable workspace detection

use std::path::Path;

use tracing::{debug, info};

use crate::error::Result;

use super::workspace::Workspace;

/// Trait for workspace detectors
pub trait WorkspaceDetector: Send + Sync {
    /// Detector name (e.g., "cargo", "pnpm", "nx")
    fn name(&self) -> &'static str;
    /// Try to detect a workspace at the given path
    fn detect(&self, path: &Path) -> Result<Option<Workspace>>;
}

/// Registry of workspace detectors, tried in order
pub struct WorkspaceDetectorRegistry {
    detectors: Vec<Box<dyn WorkspaceDetector>>,
}

impl WorkspaceDetectorRegistry {
    /// Create a registry with all built-in detectors
    pub fn new() -> Self {
        Self {
            detectors: vec![
                Box::new(CargoDetector),
                Box::new(PnpmDetector),
                Box::new(LernaDetector),
                Box::new(NxDetector),
                Box::new(TurboDetector),
                Box::new(NpmYarnDetector),
                Box::new(PythonDetector),
            ],
        }
    }

    /// Create an empty registry with no detectors
    pub fn empty() -> Self {
        Self {
            detectors: Vec::new(),
        }
    }

    /// Register an additional detector
    pub fn register(&mut self, detector: Box<dyn WorkspaceDetector>) {
        self.detectors.push(detector);
    }

    /// Try each detector in order, returning the first match
    pub fn detect(&self, path: &Path) -> Result<Option<Workspace>> {
        debug!(path = %path.display(), detectors = self.detectors.len(), "running workspace detection");
        for detector in &self.detectors {
            if let Some(ws) = detector.detect(path)? {
                info!(
                    detector = detector.name(),
                    workspace_type = %ws.workspace_type,
                    path = %path.display(),
                    "workspace detected"
                );
                return Ok(Some(ws));
            }
        }
        debug!(path = %path.display(), "no workspace detected");
        Ok(None)
    }

    /// Get all registered detectors
    pub fn all(&self) -> &[Box<dyn WorkspaceDetector>] {
        &self.detectors
    }

    /// Get names of all registered detectors
    pub fn names(&self) -> Vec<&'static str> {
        self.detectors.iter().map(|d| d.name()).collect()
    }
}

impl Default for WorkspaceDetectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Detects Cargo workspaces
pub struct CargoDetector;

impl WorkspaceDetector for CargoDetector {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn detect(&self, path: &Path) -> Result<Option<Workspace>> {
        Workspace::detect_cargo(path)
    }
}

/// Detects pnpm workspaces
pub struct PnpmDetector;

impl WorkspaceDetector for PnpmDetector {
    fn name(&self) -> &'static str {
        "pnpm"
    }

    fn detect(&self, path: &Path) -> Result<Option<Workspace>> {
        Workspace::detect_pnpm(path)
    }
}

/// Detects Lerna monorepos
pub struct LernaDetector;

impl WorkspaceDetector for LernaDetector {
    fn name(&self) -> &'static str {
        "lerna"
    }

    fn detect(&self, path: &Path) -> Result<Option<Workspace>> {
        Workspace::detect_lerna(path)
    }
}

/// Detects Nx monorepos
pub struct NxDetector;

impl WorkspaceDetector for NxDetector {
    fn name(&self) -> &'static str {
        "nx"
    }

    fn detect(&self, path: &Path) -> Result<Option<Workspace>> {
        Workspace::detect_nx(path)
    }
}

/// Detects Turborepo workspaces
pub struct TurboDetector;

impl WorkspaceDetector for TurboDetector {
    fn name(&self) -> &'static str {
        "turbo"
    }

    fn detect(&self, path: &Path) -> Result<Option<Workspace>> {
        Workspace::detect_turbo(path)
    }
}

/// Detects npm or Yarn workspaces
pub struct NpmYarnDetector;

impl WorkspaceDetector for NpmYarnDetector {
    fn name(&self) -> &'static str {
        "npm_yarn"
    }

    fn detect(&self, path: &Path) -> Result<Option<Workspace>> {
        Workspace::detect_npm_yarn(path)
    }
}

/// Detects Python monorepos
pub struct PythonDetector;

impl WorkspaceDetector for PythonDetector {
    fn name(&self) -> &'static str {
        "python"
    }

    fn detect(&self, path: &Path) -> Result<Option<Workspace>> {
        Workspace::detect_python(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = WorkspaceDetectorRegistry::empty();
        assert!(registry.all().is_empty());
        assert!(registry.names().is_empty());

        let temp = tempfile::TempDir::new().unwrap();
        let result = registry.detect(temp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_default_has_builtins() {
        let registry = WorkspaceDetectorRegistry::default();
        assert_eq!(registry.all().len(), 7);
    }

    #[test]
    fn test_detector_names() {
        let registry = WorkspaceDetectorRegistry::new();
        let names = registry.names();
        assert_eq!(
            names,
            vec!["cargo", "pnpm", "lerna", "nx", "turbo", "npm_yarn", "python"]
        );
    }

    #[test]
    fn test_register_custom_detector() {
        struct CustomDetector;
        impl WorkspaceDetector for CustomDetector {
            fn name(&self) -> &'static str {
                "custom"
            }
            fn detect(&self, _path: &Path) -> Result<Option<Workspace>> {
                Ok(None)
            }
        }

        let mut registry = WorkspaceDetectorRegistry::empty();
        registry.register(Box::new(CustomDetector));
        assert_eq!(registry.all().len(), 1);
        assert_eq!(registry.names(), vec!["custom"]);
    }
}
