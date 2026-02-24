//! Configuration system for Canaveral

pub mod changelog;
pub mod ci;
pub mod defaults;
pub mod git;
pub mod hooks_cfg;
mod loader;
pub mod metadata_cfg;
pub mod pr;
pub mod publishing;
pub mod release_notes;
mod root;
pub mod signing;
pub mod stores;
pub mod tasks;
pub mod tools;
pub mod validation;
pub mod versioning;

#[cfg(test)]
mod types;

pub use changelog::*;
pub use ci::*;
pub use defaults::*;
pub use git::*;
pub use hooks_cfg::*;
pub use loader::*;
pub use metadata_cfg::*;
pub use pr::*;
pub use publishing::*;
pub use release_notes::*;
pub use root::*;
pub use signing::*;
pub use stores::*;
pub use tasks::*;
pub use tools::*;
pub use validation::*;
pub use versioning::*;
