//! Canaveral Tasks - Task orchestration engine
//!
//! This crate provides parallel task execution across workspaces,
//! content-addressable caching, and smart test selection.

pub mod cache;
pub mod dag;
pub mod reporter;
pub mod scheduler;
pub mod task;
pub mod test_selection;

pub use cache::{CacheEntry, CacheKey, TaskCache};
pub use dag::{TaskDag, TaskNode};
pub use reporter::{TaskEvent, TaskReporter, TaskReporterRegistry};
pub use scheduler::{TaskResult, TaskScheduler, TaskStatus};
pub use task::{TaskCommand, TaskDefinition, TaskId};
pub use test_selection::{SelectedTest, SelectionReason, TestMap, TestSelector};
