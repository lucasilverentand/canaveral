//! Shell completions generation command

use std::io;

use clap::{Args, CommandFactory, ValueEnum};
use clap_complete::{generate, Shell};
use tracing::info;

use crate::cli::Cli;

/// Generate shell completions
#[derive(Debug, Args)]
pub struct CompletionsCommand {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: ShellType,

    /// Output to file instead of stdout
    #[arg(short, long)]
    pub output: Option<std::path::PathBuf>,
}

/// Supported shell types
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ShellType {
    /// Bash shell
    Bash,
    /// Zsh shell
    Zsh,
    /// Fish shell
    Fish,
    /// PowerShell
    #[value(name = "powershell")]
    PowerShell,
    /// Elvish shell
    Elvish,
}

impl From<ShellType> for Shell {
    fn from(shell: ShellType) -> Self {
        match shell {
            ShellType::Bash => Shell::Bash,
            ShellType::Zsh => Shell::Zsh,
            ShellType::Fish => Shell::Fish,
            ShellType::PowerShell => Shell::PowerShell,
            ShellType::Elvish => Shell::Elvish,
        }
    }
}

impl CompletionsCommand {
    /// Execute the completions command
    pub fn execute(&self, cli: &crate::cli::Cli) -> anyhow::Result<()> {
        info!(shell = ?self.shell, "executing completions command");
        let mut cmd = Cli::command();
        let shell: Shell = self.shell.into();

        if let Some(ref output_path) = self.output {
            let mut file = std::fs::File::create(output_path)?;
            generate(shell, &mut cmd, "canaveral", &mut file);

            if !cli.quiet {
                println!("Completions written to {}", output_path.display());
            }
        } else {
            generate(shell, &mut cmd, "canaveral", &mut io::stdout());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_type_conversion() {
        assert!(matches!(Shell::from(ShellType::Bash), Shell::Bash));
        assert!(matches!(Shell::from(ShellType::Zsh), Shell::Zsh));
        assert!(matches!(Shell::from(ShellType::Fish), Shell::Fish));
        assert!(matches!(
            Shell::from(ShellType::PowerShell),
            Shell::PowerShell
        ));
        assert!(matches!(Shell::from(ShellType::Elvish), Shell::Elvish));
    }
}
