use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScaffoldError {
    #[error("Template root not found. Expected templates under templates/scaffold")]
    TemplateRootNotFound,
    #[error("Template file not found: {0}")]
    TemplateFileMissing(String),
    #[error("Unknown block type: {0}")]
    UnknownBlock(String),
    #[error("Invalid API link '{0}': no API block with that name")]
    InvalidApiLink(String),
    #[error("A {0} block named '{1}' already exists")]
    DuplicateBlock(String, String),
    #[error("Block type '{0}' is singleton and already exists")]
    SingletonBlock(String),
}
