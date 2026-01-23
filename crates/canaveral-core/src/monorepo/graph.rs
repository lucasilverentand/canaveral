//! Dependency graph for monorepo packages

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::error::{Result, WorkflowError};

use super::discovery::DiscoveredPackage;

/// A node in the dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageNode {
    /// Package name
    pub name: String,
    /// Package version
    pub version: String,
    /// Packages this package depends on
    pub dependencies: Vec<String>,
    /// Packages that depend on this package
    pub dependents: Vec<String>,
    /// Depth in the dependency tree (0 = no dependencies)
    pub depth: usize,
}

/// Dependency graph for workspace packages
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// Nodes indexed by package name
    nodes: HashMap<String, PackageNode>,
    /// Topologically sorted order (dependencies before dependents)
    sorted_order: Vec<String>,
    /// Circular dependencies detected
    cycles: Vec<Vec<String>>,
}

impl DependencyGraph {
    /// Build a dependency graph from discovered packages
    pub fn build(packages: &[DiscoveredPackage]) -> Result<Self> {
        let mut nodes: HashMap<String, PackageNode> = HashMap::new();

        // Create initial nodes
        for pkg in packages {
            nodes.insert(
                pkg.name.clone(),
                PackageNode {
                    name: pkg.name.clone(),
                    version: pkg.version.clone(),
                    dependencies: pkg.workspace_dependencies.clone(),
                    dependents: Vec::new(),
                    depth: 0,
                },
            );
        }

        // Build reverse dependency mapping (dependents)
        for pkg in packages {
            for dep in &pkg.workspace_dependencies {
                if let Some(dep_node) = nodes.get_mut(dep) {
                    dep_node.dependents.push(pkg.name.clone());
                }
            }
        }

        // Detect cycles and compute topological order
        let (sorted_order, cycles) = Self::topological_sort(&nodes)?;

        // Calculate depths
        for name in &sorted_order {
            if let Some(node) = nodes.get(name) {
                let max_dep_depth = node
                    .dependencies
                    .iter()
                    .filter_map(|dep| nodes.get(dep))
                    .map(|n| n.depth)
                    .max()
                    .unwrap_or(0);

                if let Some(node) = nodes.get_mut(name) {
                    node.depth = if node.dependencies.is_empty() {
                        0
                    } else {
                        max_dep_depth + 1
                    };
                }
            }
        }

        Ok(Self {
            nodes,
            sorted_order,
            cycles,
        })
    }

    /// Perform topological sort using Kahn's algorithm
    fn topological_sort(nodes: &HashMap<String, PackageNode>) -> Result<(Vec<String>, Vec<Vec<String>>)> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut sorted: Vec<String> = Vec::new();

        // Initialize in-degrees
        for (name, node) in nodes {
            let degree = node
                .dependencies
                .iter()
                .filter(|d| nodes.contains_key(*d))
                .count();
            in_degree.insert(name.clone(), degree);
            if degree == 0 {
                queue.push_back(name.clone());
            }
        }

        // Process nodes
        while let Some(name) = queue.pop_front() {
            sorted.push(name.clone());

            if let Some(node) = nodes.get(&name) {
                for dependent in &node.dependents {
                    if let Some(degree) = in_degree.get_mut(dependent) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        // Detect cycles (nodes not in sorted order have cycles)
        let mut cycles = Vec::new();
        if sorted.len() != nodes.len() {
            let in_sorted: HashSet<_> = sorted.iter().collect();
            let cyclic_nodes: Vec<_> = nodes
                .keys()
                .filter(|n| !in_sorted.contains(n))
                .cloned()
                .collect();

            // Find actual cycles
            for start in &cyclic_nodes {
                if let Some(cycle) = Self::find_cycle(nodes, start, &cyclic_nodes) {
                    if !cycles.iter().any(|c: &Vec<String>| {
                        c.len() == cycle.len() && cycle.iter().all(|n| c.contains(n))
                    }) {
                        cycles.push(cycle);
                    }
                }
            }
        }

        Ok((sorted, cycles))
    }

    /// Find a cycle starting from a given node
    fn find_cycle(
        nodes: &HashMap<String, PackageNode>,
        start: &str,
        cyclic_nodes: &[String],
    ) -> Option<Vec<String>> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut path: Vec<String> = Vec::new();

        fn dfs(
            nodes: &HashMap<String, PackageNode>,
            current: &str,
            start: &str,
            visited: &mut HashSet<String>,
            path: &mut Vec<String>,
            cyclic_nodes: &[String],
        ) -> bool {
            if visited.contains(current) {
                return current == start && path.len() > 1;
            }

            if !cyclic_nodes.contains(&current.to_string()) {
                return false;
            }

            visited.insert(current.to_string());
            path.push(current.to_string());

            if let Some(node) = nodes.get(current) {
                for dep in &node.dependencies {
                    if dfs(nodes, dep, start, visited, path, cyclic_nodes) {
                        return true;
                    }
                }
            }

            path.pop();
            false
        }

        if dfs(nodes, start, start, &mut visited, &mut path, cyclic_nodes) {
            Some(path)
        } else {
            None
        }
    }

    /// Get packages in topologically sorted order (dependencies first)
    pub fn sorted(&self) -> &[String] {
        &self.sorted_order
    }

    /// Get packages in reverse topological order (dependents first)
    pub fn reverse_sorted(&self) -> Vec<String> {
        let mut reversed = self.sorted_order.clone();
        reversed.reverse();
        reversed
    }

    /// Check if there are any circular dependencies
    pub fn has_cycles(&self) -> bool {
        !self.cycles.is_empty()
    }

    /// Get detected circular dependencies
    pub fn cycles(&self) -> &[Vec<String>] {
        &self.cycles
    }

    /// Get a package node
    pub fn get(&self, name: &str) -> Option<&PackageNode> {
        self.nodes.get(name)
    }

    /// Get all packages that depend on a given package (direct dependents)
    pub fn get_dependents(&self, name: &str) -> HashSet<String> {
        self.nodes
            .get(name)
            .map(|n| n.dependents.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all packages that a given package depends on (direct dependencies)
    pub fn get_dependencies(&self, name: &str) -> HashSet<String> {
        self.nodes
            .get(name)
            .map(|n| n.dependencies.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all packages transitively affected by a change to the given package
    pub fn get_affected(&self, name: &str) -> HashSet<String> {
        let mut affected = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        queue.push_back(name.to_string());

        while let Some(current) = queue.pop_front() {
            if affected.contains(&current) {
                continue;
            }
            affected.insert(current.clone());

            if let Some(node) = self.nodes.get(&current) {
                for dependent in &node.dependents {
                    if !affected.contains(dependent) {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }

        // Remove the original package from affected
        affected.remove(name);
        affected
    }

    /// Get all transitive dependencies of a package
    pub fn get_all_dependencies(&self, name: &str) -> HashSet<String> {
        let mut deps = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        queue.push_back(name.to_string());

        while let Some(current) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&current) {
                for dep in &node.dependencies {
                    if !deps.contains(dep) {
                        deps.insert(dep.clone());
                        queue.push_back(dep.clone());
                    }
                }
            }
        }

        deps
    }

    /// Validate that the graph has no cycles
    pub fn validate(&self) -> Result<()> {
        if self.has_cycles() {
            let cycle_desc: Vec<String> = self
                .cycles
                .iter()
                .map(|c| c.join(" -> "))
                .collect();

            return Err(WorkflowError::ValidationFailed(format!(
                "Circular dependencies detected: {}",
                cycle_desc.join("; ")
            ))
            .into());
        }
        Ok(())
    }

    /// Get the maximum depth of the dependency tree
    pub fn max_depth(&self) -> usize {
        self.nodes.values().map(|n| n.depth).max().unwrap_or(0)
    }

    /// Get packages at a specific depth level
    pub fn packages_at_depth(&self, depth: usize) -> Vec<&str> {
        self.nodes
            .values()
            .filter(|n| n.depth == depth)
            .map(|n| n.name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_packages() -> Vec<DiscoveredPackage> {
        vec![
            DiscoveredPackage {
                name: "core".to_string(),
                version: "1.0.0".to_string(),
                path: "packages/core".into(),
                manifest_path: "packages/core/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec![],
            },
            DiscoveredPackage {
                name: "utils".to_string(),
                version: "1.0.0".to_string(),
                path: "packages/utils".into(),
                manifest_path: "packages/utils/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["core".to_string()],
            },
            DiscoveredPackage {
                name: "cli".to_string(),
                version: "1.0.0".to_string(),
                path: "packages/cli".into(),
                manifest_path: "packages/cli/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["core".to_string(), "utils".to_string()],
            },
        ]
    }

    #[test]
    fn test_build_graph() {
        let packages = create_packages();
        let graph = DependencyGraph::build(&packages).unwrap();

        assert!(!graph.has_cycles());
        assert_eq!(graph.sorted().len(), 3);
    }

    #[test]
    fn test_topological_order() {
        let packages = create_packages();
        let graph = DependencyGraph::build(&packages).unwrap();

        let sorted = graph.sorted();

        // Core should come before utils and cli
        let core_pos = sorted.iter().position(|n| n == "core").unwrap();
        let utils_pos = sorted.iter().position(|n| n == "utils").unwrap();
        let cli_pos = sorted.iter().position(|n| n == "cli").unwrap();

        assert!(core_pos < utils_pos);
        assert!(core_pos < cli_pos);
        assert!(utils_pos < cli_pos);
    }

    #[test]
    fn test_dependents() {
        let packages = create_packages();
        let graph = DependencyGraph::build(&packages).unwrap();

        let core_dependents = graph.get_dependents("core");
        assert!(core_dependents.contains("utils"));
        assert!(core_dependents.contains("cli"));

        let utils_dependents = graph.get_dependents("utils");
        assert!(utils_dependents.contains("cli"));
        assert!(!utils_dependents.contains("core"));
    }

    #[test]
    fn test_affected_packages() {
        let packages = create_packages();
        let graph = DependencyGraph::build(&packages).unwrap();

        // If core changes, utils and cli are affected
        let affected = graph.get_affected("core");
        assert!(affected.contains("utils"));
        assert!(affected.contains("cli"));

        // If cli changes, nothing else is affected
        let affected = graph.get_affected("cli");
        assert!(affected.is_empty());
    }

    #[test]
    fn test_depth_calculation() {
        let packages = create_packages();
        let graph = DependencyGraph::build(&packages).unwrap();

        assert_eq!(graph.get("core").unwrap().depth, 0);
        assert_eq!(graph.get("utils").unwrap().depth, 1);
        assert_eq!(graph.get("cli").unwrap().depth, 2);
        assert_eq!(graph.max_depth(), 2);
    }

    #[test]
    fn test_cycle_detection() {
        let packages = vec![
            DiscoveredPackage {
                name: "a".to_string(),
                version: "1.0.0".to_string(),
                path: "a".into(),
                manifest_path: "a/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["b".to_string()],
            },
            DiscoveredPackage {
                name: "b".to_string(),
                version: "1.0.0".to_string(),
                path: "b".into(),
                manifest_path: "b/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["c".to_string()],
            },
            DiscoveredPackage {
                name: "c".to_string(),
                version: "1.0.0".to_string(),
                path: "c".into(),
                manifest_path: "c/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["a".to_string()],
            },
        ];

        let graph = DependencyGraph::build(&packages).unwrap();
        assert!(graph.has_cycles());
        assert!(graph.validate().is_err());
    }

    #[test]
    fn test_packages_at_depth() {
        let packages = create_packages();
        let graph = DependencyGraph::build(&packages).unwrap();

        let depth_0 = graph.packages_at_depth(0);
        assert!(depth_0.contains(&"core"));

        let depth_1 = graph.packages_at_depth(1);
        assert!(depth_1.contains(&"utils"));

        let depth_2 = graph.packages_at_depth(2);
        assert!(depth_2.contains(&"cli"));
    }
}
