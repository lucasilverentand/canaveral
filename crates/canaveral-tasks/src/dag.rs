//! Task DAG construction and management

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use canaveral_core::monorepo::graph::DependencyGraph;

use crate::task::{TaskDefinition, TaskId};

/// A node in the task execution DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    /// Task identifier
    pub id: TaskId,
    /// The task definition
    pub definition: TaskDefinition,
    /// Tasks that must complete before this one
    pub dependencies: HashSet<TaskId>,
    /// Tasks waiting on this one
    pub dependents: HashSet<TaskId>,
    /// Execution wave (tasks in the same wave can run in parallel)
    pub wave: usize,
}

/// Directed acyclic graph of tasks to execute
#[derive(Debug, Clone)]
pub struct TaskDag {
    /// All nodes in the DAG
    nodes: HashMap<TaskId, TaskNode>,
    /// Tasks grouped by execution wave (wave 0 runs first, then wave 1, etc.)
    waves: Vec<Vec<TaskId>>,
    /// Topologically sorted task order
    sorted_order: Vec<TaskId>,
}

impl TaskDag {
    /// Build a task DAG from a dependency graph and task definitions.
    ///
    /// Expands the package-level dependency graph into a task-level graph.
    /// For example, if "test" depends_on "build" within a package, and package B
    /// depends on package A, then:
    ///   B:test -> B:build -> A:build (if build has depends_on_packages)
    ///   A:test -> A:build
    #[instrument(skip_all, fields(packages = packages.len(), target_tasks = target_tasks.len()))]
    pub fn build(
        package_graph: &DependencyGraph,
        pipeline: &HashMap<String, TaskDefinition>,
        target_tasks: &[String],
        packages: &[String],
    ) -> Result<Self, DagError> {
        let mut nodes: HashMap<TaskId, TaskNode> = HashMap::new();

        // Create task nodes for each package × task combination
        for pkg in packages {
            for task_name in target_tasks {
                let definition = pipeline
                    .get(task_name)
                    .ok_or_else(|| DagError::TaskNotFound(task_name.clone()))?;

                let id = TaskId::new(pkg, task_name);
                nodes.insert(
                    id.clone(),
                    TaskNode {
                        id: id.clone(),
                        definition: definition.clone(),
                        dependencies: HashSet::new(),
                        dependents: HashSet::new(),
                        wave: 0,
                    },
                );
            }
        }

        // Wire up dependencies
        for pkg in packages {
            for task_name in target_tasks {
                let id = TaskId::new(pkg, task_name);
                let definition = pipeline.get(task_name).unwrap();

                let mut deps = HashSet::new();

                // Same-package task dependencies (e.g., test depends_on build)
                for dep_task in &definition.depends_on {
                    if target_tasks.contains(dep_task) {
                        let dep_id = TaskId::new(pkg, dep_task);
                        if nodes.contains_key(&dep_id) {
                            deps.insert(dep_id);
                        }
                    }
                }

                // Cross-package dependencies (same task in dependency packages)
                if definition.depends_on_packages {
                    let pkg_deps = package_graph.get_dependencies(pkg);
                    for dep_pkg in &pkg_deps {
                        if packages.contains(dep_pkg) {
                            let dep_id = TaskId::new(dep_pkg, task_name);
                            if nodes.contains_key(&dep_id) {
                                deps.insert(dep_id);
                            }
                        }
                    }
                }

                if let Some(node) = nodes.get_mut(&id) {
                    node.dependencies = deps;
                }
            }
        }

        // Build reverse dependency map (dependents)
        let all_deps: Vec<(TaskId, HashSet<TaskId>)> = nodes
            .iter()
            .map(|(id, node)| (id.clone(), node.dependencies.clone()))
            .collect();

        for (id, deps) in &all_deps {
            for dep in deps {
                if let Some(dep_node) = nodes.get_mut(dep) {
                    dep_node.dependents.insert(id.clone());
                }
            }
        }

        // Topological sort
        let sorted_order = Self::topological_sort(&nodes)?;

        // Compute waves
        let waves = Self::compute_waves(&nodes, &sorted_order);

        // Set wave numbers on nodes
        for (wave_idx, wave_tasks) in waves.iter().enumerate() {
            for task_id in wave_tasks {
                if let Some(node) = nodes.get_mut(task_id) {
                    node.wave = wave_idx;
                }
            }
        }

        info!(
            task_count = nodes.len(),
            wave_count = waves.len(),
            "task DAG built"
        );

        Ok(Self {
            nodes,
            waves,
            sorted_order,
        })
    }

    /// Topological sort using Kahn's algorithm
    #[instrument(skip_all, fields(node_count = nodes.len()))]
    fn topological_sort(nodes: &HashMap<TaskId, TaskNode>) -> Result<Vec<TaskId>, DagError> {
        let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
        let mut queue: VecDeque<TaskId> = VecDeque::new();
        let mut sorted: Vec<TaskId> = Vec::new();

        for (id, node) in nodes {
            let degree = node
                .dependencies
                .iter()
                .filter(|d| nodes.contains_key(*d))
                .count();
            in_degree.insert(id.clone(), degree);
            if degree == 0 {
                queue.push_back(id.clone());
            }
        }

        while let Some(id) = queue.pop_front() {
            sorted.push(id.clone());

            if let Some(node) = nodes.get(&id) {
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

        if sorted.len() != nodes.len() {
            let in_sorted: HashSet<_> = sorted.iter().collect();
            let cyclic: Vec<String> = nodes
                .keys()
                .filter(|id| !in_sorted.contains(id))
                .map(|id| id.to_string())
                .collect();
            return Err(DagError::CyclicDependency(cyclic.join(", ")));
        }

        Ok(sorted)
    }

    /// Compute execution waves (groups of tasks that can run in parallel)
    #[instrument(skip_all, fields(node_count = nodes.len()))]
    fn compute_waves(nodes: &HashMap<TaskId, TaskNode>, sorted: &[TaskId]) -> Vec<Vec<TaskId>> {
        let mut wave_map: HashMap<TaskId, usize> = HashMap::new();

        for id in sorted {
            if let Some(node) = nodes.get(id) {
                let wave = node
                    .dependencies
                    .iter()
                    .filter_map(|dep| wave_map.get(dep))
                    .max()
                    .map(|w| w + 1)
                    .unwrap_or(0);
                wave_map.insert(id.clone(), wave);
            }
        }

        let max_wave = wave_map.values().max().copied().unwrap_or(0);
        let mut waves: Vec<Vec<TaskId>> = vec![Vec::new(); max_wave + 1];

        for id in sorted {
            if let Some(&wave) = wave_map.get(id) {
                waves[wave].push(id.clone());
            }
        }

        waves
    }

    /// Get all task nodes
    pub fn nodes(&self) -> &HashMap<TaskId, TaskNode> {
        &self.nodes
    }

    /// Get a specific task node
    pub fn get(&self, id: &TaskId) -> Option<&TaskNode> {
        self.nodes.get(id)
    }

    /// Get execution waves
    pub fn waves(&self) -> &[Vec<TaskId>] {
        &self.waves
    }

    /// Get the total number of tasks
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the DAG is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get topologically sorted order
    pub fn sorted(&self) -> &[TaskId] {
        &self.sorted_order
    }

    /// Get a human-readable summary of the execution plan
    pub fn execution_plan(&self) -> String {
        let mut plan = String::new();
        for (i, wave) in self.waves.iter().enumerate() {
            plan.push_str(&format!("Wave {} ({} tasks):\n", i, wave.len()));
            for id in wave {
                if let Some(node) = self.nodes.get(id) {
                    let cmd = node
                        .definition
                        .command
                        .as_deref()
                        .unwrap_or("<framework adapter>");
                    let deps: Vec<String> =
                        node.dependencies.iter().map(|d| d.to_string()).collect();
                    if deps.is_empty() {
                        plan.push_str(&format!("  {} -> {}\n", id, cmd));
                    } else {
                        plan.push_str(&format!(
                            "  {} -> {} (after: {})\n",
                            id,
                            cmd,
                            deps.join(", ")
                        ));
                    }
                }
            }
        }
        plan
    }
}

/// Errors during DAG construction
#[derive(Debug, thiserror::Error)]
pub enum DagError {
    /// Cyclic dependency detected
    #[error("Cyclic dependency detected among tasks: {0}")]
    CyclicDependency(String),

    /// Task not found in pipeline
    #[error("Task '{0}' not found in pipeline configuration")]
    TaskNotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use canaveral_core::monorepo::discovery::DiscoveredPackage;

    fn create_test_graph() -> DependencyGraph {
        let packages = vec![
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
                name: "app".to_string(),
                version: "1.0.0".to_string(),
                path: "packages/app".into(),
                manifest_path: "packages/app/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["core".to_string(), "utils".to_string()],
            },
        ];
        DependencyGraph::build(&packages).unwrap()
    }

    fn create_pipeline() -> HashMap<String, TaskDefinition> {
        let mut pipeline = HashMap::new();
        pipeline.insert(
            "build".to_string(),
            TaskDefinition::new("build")
                .with_command("npm run build")
                .with_depends_on_packages(true),
        );
        pipeline.insert(
            "test".to_string(),
            TaskDefinition::new("test")
                .with_command("npm test")
                .with_depends_on("build"),
        );
        pipeline.insert(
            "lint".to_string(),
            TaskDefinition::new("lint").with_command("npm run lint"),
        );
        pipeline
    }

    #[test]
    fn test_build_dag() {
        let graph = create_test_graph();
        let pipeline = create_pipeline();
        let packages = vec!["core".to_string(), "utils".to_string(), "app".to_string()];

        let dag = TaskDag::build(&graph, &pipeline, &["build".to_string()], &packages).unwrap();

        assert_eq!(dag.len(), 3); // one build per package
        assert!(!dag.waves().is_empty());
    }

    #[test]
    fn test_dag_waves() {
        let graph = create_test_graph();
        let pipeline = create_pipeline();
        let packages = vec!["core".to_string(), "utils".to_string(), "app".to_string()];

        let dag = TaskDag::build(&graph, &pipeline, &["build".to_string()], &packages).unwrap();

        // core:build should be in wave 0 (no deps)
        let core_build = TaskId::new("core", "build");
        let core_node = dag.get(&core_build).unwrap();
        assert_eq!(core_node.wave, 0);

        // utils:build should be in wave 1 (depends on core:build)
        let utils_build = TaskId::new("utils", "build");
        let utils_node = dag.get(&utils_build).unwrap();
        assert_eq!(utils_node.wave, 1);

        // app:build should be in wave 2 (depends on core:build and utils:build)
        let app_build = TaskId::new("app", "build");
        let app_node = dag.get(&app_build).unwrap();
        assert_eq!(app_node.wave, 2);
    }

    #[test]
    fn test_dag_with_multiple_tasks() {
        let graph = create_test_graph();
        let pipeline = create_pipeline();
        let packages = vec!["core".to_string(), "utils".to_string()];

        let dag = TaskDag::build(
            &graph,
            &pipeline,
            &["build".to_string(), "test".to_string()],
            &packages,
        )
        .unwrap();

        // 2 packages × 2 tasks = 4 nodes
        assert_eq!(dag.len(), 4);

        // core:test depends on core:build (same-package dep)
        let core_test = TaskId::new("core", "test");
        let core_test_node = dag.get(&core_test).unwrap();
        assert!(core_test_node
            .dependencies
            .contains(&TaskId::new("core", "build")));
    }

    #[test]
    fn test_dag_task_not_found() {
        let graph = create_test_graph();
        let pipeline = create_pipeline();
        let packages = vec!["core".to_string()];

        let result = TaskDag::build(&graph, &pipeline, &["nonexistent".to_string()], &packages);

        assert!(result.is_err());
    }

    #[test]
    fn test_execution_plan_output() {
        let graph = create_test_graph();
        let pipeline = create_pipeline();
        let packages = vec!["core".to_string(), "utils".to_string()];

        let dag = TaskDag::build(&graph, &pipeline, &["build".to_string()], &packages).unwrap();
        let plan = dag.execution_plan();

        assert!(plan.contains("Wave 0"));
        assert!(plan.contains("core:build"));
        assert!(plan.contains("utils:build"));
    }

    #[test]
    fn test_independent_tasks_same_wave() {
        let graph = create_test_graph();
        let pipeline = create_pipeline();
        let packages = vec!["core".to_string()];

        // lint has no depends_on and no depends_on_packages
        let dag = TaskDag::build(
            &graph,
            &pipeline,
            &["build".to_string(), "lint".to_string()],
            &packages,
        )
        .unwrap();

        let build_node = dag.get(&TaskId::new("core", "build")).unwrap();
        let lint_node = dag.get(&TaskId::new("core", "lint")).unwrap();

        // Both should be in wave 0 since lint doesn't depend on build
        assert_eq!(build_node.wave, 0);
        assert_eq!(lint_node.wave, 0);
    }
}
