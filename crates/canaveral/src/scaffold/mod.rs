pub mod context;
pub mod errors;
pub mod generator;
pub mod presets;
pub mod project_detector;
pub mod registry;
pub mod templates;

pub use context::{
    AndroidBlock, ApiBlock, AuthConfig, BillingConfig, Block, BlockType, DashboardBlock, ExpoBlock,
    IosBlock, PackageManager, PackagesConfig, ProjectContext, ToolingConfig, WebBlock,
};
pub use generator::{generate_block, generate_project};
pub use presets::{apply_preset, Preset};
pub use project_detector::{detect_project, load_scaffold_state, save_scaffold_state};
pub use registry::{registry, validate_block_addition};
