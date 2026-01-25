//! CLI commands

mod build;
mod changelog;
mod completions;
mod doctor;
mod firebase;
mod init;
mod match_cmd;
mod metadata;
mod release;
mod screenshots;
mod signing;
mod signing_team;
mod status;
mod store;
mod test;
mod testflight;
mod validate;
mod version;

pub use build::BuildCommand;
pub use changelog::ChangelogCommand;
pub use completions::CompletionsCommand;
pub use doctor::DoctorCommand;
pub use firebase::FirebaseCommand;
pub use init::InitCommand;
pub use match_cmd::MatchCommand;
pub use metadata::MetadataCommand;
pub use release::ReleaseCommand;
pub use screenshots::ScreenshotsCommand;
pub use signing::SigningCommand;
pub use status::StatusCommand;
pub use store::StoreCommand;
pub use test::TestCommand;
pub use testflight::TestFlightCommand;
pub use validate::ValidateCommand;
pub use version::VersionCommand;
