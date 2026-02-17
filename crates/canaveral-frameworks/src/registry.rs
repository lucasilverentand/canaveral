//! Framework adapter registry
//!
//! Central registry for all framework adapters. Handles registration, lookup,
//! and detection of frameworks.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tracing::{debug, info, instrument, warn};

use crate::detection::DetectionResult;
use crate::error::{FrameworkError, Result};
use crate::traits::{BuildAdapter, DistributeAdapter, OtaAdapter, ScreenshotAdapter, TestAdapter};

/// Registry of framework adapters
pub struct FrameworkRegistry {
    /// Build adapters by ID
    build_adapters: HashMap<String, Arc<dyn BuildAdapter>>,

    /// Test adapters by ID
    test_adapters: HashMap<String, Arc<dyn TestAdapter>>,

    /// Screenshot adapters by ID
    screenshot_adapters: HashMap<String, Arc<dyn ScreenshotAdapter>>,

    /// Distribution adapters by ID
    distribute_adapters: HashMap<String, Arc<dyn DistributeAdapter>>,

    /// OTA adapters by ID
    ota_adapters: HashMap<String, Arc<dyn OtaAdapter>>,

    /// Ordered list of build adapter IDs for detection priority
    build_detection_order: Vec<String>,

    /// Ordered list of test adapter IDs for detection priority
    test_detection_order: Vec<String>,
}

impl FrameworkRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            build_adapters: HashMap::new(),
            test_adapters: HashMap::new(),
            screenshot_adapters: HashMap::new(),
            distribute_adapters: HashMap::new(),
            ota_adapters: HashMap::new(),
            build_detection_order: Vec::new(),
            test_detection_order: Vec::new(),
        }
    }

    /// Create a registry with all built-in adapters
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();

        // Register all built-in framework adapters
        crate::frameworks::register_all(&mut registry);

        registry
    }

    // -------------------------------------------------------------------------
    // Build Adapters
    // -------------------------------------------------------------------------

    /// Register a build adapter
    pub fn register_build<A: BuildAdapter + 'static>(&mut self, adapter: A) {
        let id = adapter.id().to_string();
        debug!(adapter_id = %id, "registering build adapter");
        self.build_detection_order.push(id.clone());
        self.build_adapters.insert(id, Arc::new(adapter));
    }

    /// Get a build adapter by ID
    pub fn get_build(&self, id: &str) -> Option<Arc<dyn BuildAdapter>> {
        self.build_adapters.get(id).cloned()
    }

    /// Get all build adapter IDs
    pub fn build_adapter_ids(&self) -> Vec<&str> {
        self.build_adapters.keys().map(|s| s.as_str()).collect()
    }

    /// Detect build adapters for a project
    pub fn detect_build(&self, path: &Path) -> Vec<DetectionResult> {
        debug!(path = %path.display(), "detecting build frameworks");
        let mut results: Vec<_> = self
            .build_detection_order
            .iter()
            .filter_map(|id| {
                let adapter = self.build_adapters.get(id)?;
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

        results.sort_by(|a, b| b.detection.cmp(&a.detection));
        if !results.is_empty() {
            info!(
                detected_count = results.len(),
                best = %results[0].adapter_name,
                confidence = results[0].detection.confidence(),
                "build framework detection complete"
            );
        }
        results
    }

    /// Detect and return the best build adapter
    pub fn detect_build_best(&self, path: &Path) -> Option<Arc<dyn BuildAdapter>> {
        self.detect_build(path)
            .first()
            .and_then(|r| self.get_build(&r.adapter_id))
    }

    /// Resolve a build adapter - by ID or auto-detect
    #[instrument(skip(self), fields(path = %path.display(), adapter_id))]
    pub fn resolve_build(
        &self,
        path: &Path,
        adapter_id: Option<&str>,
    ) -> Result<Arc<dyn BuildAdapter>> {
        if let Some(id) = adapter_id {
            self.get_build(id).ok_or_else(|| FrameworkError::Context {
                context: "adapter resolution".to_string(),
                message: format!("Unknown build adapter: {}", id),
            })
        } else {
            let detections = self.detect_build(path);

            if detections.is_empty() {
                return Err(FrameworkError::NoFrameworkDetected {
                    path: path.to_path_buf(),
                    supported: self.build_adapter_ids().join(", "),
                });
            }

            // Check for ambiguity
            if detections.len() > 1 {
                let first = &detections[0];
                let second = &detections[1];

                // If confidence is too close, it's ambiguous
                if first.detection.confidence() < second.detection.confidence() + 20 {
                    warn!(
                        first = %first.adapter_name,
                        second = %second.adapter_name,
                        "ambiguous framework detection"
                    );
                    return Err(FrameworkError::AmbiguousFramework {
                        frameworks: detections.iter().map(|d| d.adapter_name.clone()).collect(),
                    });
                }
            }

            self.get_build(&detections[0].adapter_id)
                .ok_or_else(|| FrameworkError::Context {
                    context: "adapter resolution".to_string(),
                    message: "Detected adapter not found in registry".to_string(),
                })
        }
    }

    // -------------------------------------------------------------------------
    // Test Adapters
    // -------------------------------------------------------------------------

    /// Register a test adapter
    pub fn register_test<A: TestAdapter + 'static>(&mut self, adapter: A) {
        let id = adapter.id().to_string();
        self.test_detection_order.push(id.clone());
        self.test_adapters.insert(id, Arc::new(adapter));
    }

    /// Get a test adapter by ID
    pub fn get_test(&self, id: &str) -> Option<Arc<dyn TestAdapter>> {
        self.test_adapters.get(id).cloned()
    }

    /// Detect test adapters for a project
    pub fn detect_test(&self, path: &Path) -> Vec<DetectionResult> {
        let mut results: Vec<_> = self
            .test_detection_order
            .iter()
            .filter_map(|id| {
                let adapter = self.test_adapters.get(id)?;
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

        results.sort_by(|a, b| b.detection.cmp(&a.detection));
        results
    }

    /// Resolve a test adapter
    pub fn resolve_test(
        &self,
        path: &Path,
        adapter_id: Option<&str>,
    ) -> Result<Arc<dyn TestAdapter>> {
        if let Some(id) = adapter_id {
            self.get_test(id).ok_or_else(|| FrameworkError::Context {
                context: "adapter resolution".to_string(),
                message: format!("Unknown test adapter: {}", id),
            })
        } else {
            self.detect_test(path)
                .first()
                .and_then(|r| self.get_test(&r.adapter_id))
                .ok_or_else(|| FrameworkError::Context {
                    context: "adapter resolution".to_string(),
                    message: "No test adapter found for project".to_string(),
                })
        }
    }

    /// Get all test adapter IDs
    pub fn test_adapter_ids(&self) -> Vec<&str> {
        self.test_adapters.keys().map(|s| s.as_str()).collect()
    }

    /// Get all registered test adapters (for iteration)
    pub fn test_adapters(&self) -> Vec<&dyn TestAdapter> {
        self.test_adapters.values().map(|a| a.as_ref()).collect()
    }

    /// Get a test adapter by ID (returns reference for runner compatibility)
    pub fn get_test_adapter(&self, id: &str) -> Option<&dyn TestAdapter> {
        self.test_adapters.get(id).map(|a| a.as_ref())
    }

    // -------------------------------------------------------------------------
    // Screenshot Adapters
    // -------------------------------------------------------------------------

    /// Register a screenshot adapter
    pub fn register_screenshot<A: ScreenshotAdapter + 'static>(&mut self, adapter: A) {
        let id = adapter.id().to_string();
        self.screenshot_adapters.insert(id, Arc::new(adapter));
    }

    /// Get a screenshot adapter by ID
    pub fn get_screenshot(&self, id: &str) -> Option<Arc<dyn ScreenshotAdapter>> {
        self.screenshot_adapters.get(id).cloned()
    }

    // -------------------------------------------------------------------------
    // Distribution Adapters
    // -------------------------------------------------------------------------

    /// Register a distribution adapter
    pub fn register_distribute<A: DistributeAdapter + 'static>(&mut self, adapter: A) {
        let id = adapter.id().to_string();
        self.distribute_adapters.insert(id, Arc::new(adapter));
    }

    /// Get a distribution adapter by ID
    pub fn get_distribute(&self, id: &str) -> Option<Arc<dyn DistributeAdapter>> {
        self.distribute_adapters.get(id).cloned()
    }

    /// Get all distribution adapter IDs
    pub fn distribute_adapter_ids(&self) -> Vec<&str> {
        self.distribute_adapters
            .keys()
            .map(|s| s.as_str())
            .collect()
    }

    // -------------------------------------------------------------------------
    // OTA Adapters
    // -------------------------------------------------------------------------

    /// Register an OTA adapter
    pub fn register_ota<A: OtaAdapter + 'static>(&mut self, adapter: A) {
        let id = adapter.id().to_string();
        self.ota_adapters.insert(id, Arc::new(adapter));
    }

    /// Get an OTA adapter by ID
    pub fn get_ota(&self, id: &str) -> Option<Arc<dyn OtaAdapter>> {
        self.ota_adapters.get(id).cloned()
    }

    /// Detect OTA adapters for a project
    pub fn detect_ota(&self, path: &Path) -> Vec<DetectionResult> {
        self.ota_adapters
            .values()
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
            .collect()
    }
}

impl Default for FrameworkRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = FrameworkRegistry::new();
        assert!(registry.build_adapter_ids().is_empty());
    }

    #[test]
    fn test_detection_empty_registry() {
        let registry = FrameworkRegistry::new();
        let temp = tempfile::TempDir::new().unwrap();

        let results = registry.detect_build(temp.path());
        assert!(results.is_empty());
    }
}
