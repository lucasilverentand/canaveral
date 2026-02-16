//! Git hooks management command

use std::path::PathBuf;
use std::process::Command;

use clap::{Args, Subcommand};
use console::style;
use tracing::info;

use canaveral_core::config::Config;
use canaveral_git::hooks::{self, GitHookType};

use crate::cli::Cli;

/// Git hook management (install, uninstall, run, status)
#[derive(Debug, Args)]
pub struct HooksCommand {
    #[command(subcommand)]
    pub subcommand: HooksSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum HooksSubcommand {
    /// Install git hooks into .git/hooks/
    Install {
        /// Install only a specific hook (commit-msg, pre-commit, pre-push)
        #[arg(long)]
        hook: Option<String>,
    },
    /// Uninstall canaveral-managed git hooks
    Uninstall,
    /// Run a hook (called by git, not typically invoked directly)
    Run {
        /// Hook name to run (commit-msg, pre-commit, pre-push)
        name: String,
        /// Separator
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Show which hooks are installed
    Status,
}

impl HooksCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let repo_root = find_repo_root()?;

        match &self.subcommand {
            HooksSubcommand::Install { hook } => self.install(&repo_root, hook.as_deref(), cli),
            HooksSubcommand::Uninstall => self.uninstall(&repo_root, cli),
            HooksSubcommand::Run { name, args } => self.run(&repo_root, name, args),
            HooksSubcommand::Status => self.status(&repo_root, cli),
        }
    }

    fn install(&self, repo_root: &PathBuf, hook: Option<&str>, cli: &Cli) -> anyhow::Result<()> {
        info!("installing git hooks");

        if let Some(name) = hook {
            let hook_type = parse_hook_type(name)?;
            hooks::install_hook(repo_root, hook_type)?;
            if !cli.quiet {
                println!(
                    "{} Installed {} hook",
                    style("✓").green().bold(),
                    style(name).cyan()
                );
            }
        } else {
            hooks::install_all(repo_root)?;
            if !cli.quiet {
                println!(
                    "{} Installed all git hooks (commit-msg, pre-commit, pre-push)",
                    style("✓").green().bold()
                );
            }
        }

        Ok(())
    }

    fn uninstall(&self, repo_root: &PathBuf, cli: &Cli) -> anyhow::Result<()> {
        info!("uninstalling git hooks");
        hooks::uninstall_all(repo_root)?;

        if !cli.quiet {
            println!(
                "{} Uninstalled all canaveral git hooks",
                style("✓").green().bold()
            );
        }
        Ok(())
    }

    fn run(&self, repo_root: &PathBuf, name: &str, args: &[String]) -> anyhow::Result<()> {
        info!(hook = name, "running git hook");

        match name {
            "commit-msg" => self.run_commit_msg(repo_root, args),
            "pre-commit" => self.run_script_hook(repo_root, "pre_commit"),
            "pre-push" => self.run_script_hook(repo_root, "pre_push"),
            _ => anyhow::bail!("Unknown hook: {name}"),
        }
    }

    fn run_commit_msg(&self, repo_root: &PathBuf, args: &[String]) -> anyhow::Result<()> {
        let msg_file = args
            .first()
            .ok_or_else(|| anyhow::anyhow!("commit-msg hook requires a message file argument"))?;

        let message = std::fs::read_to_string(msg_file)?;
        let first_line = message.lines().next().unwrap_or("").trim();

        let config = load_config(repo_root)?;
        let hook_cfg = &config.git_hooks.commit_msg;

        // Allow WIP commits if configured
        if hook_cfg.allow_wip
            && (first_line.starts_with("WIP")
                || first_line.starts_with("wip")
                || first_line.eq_ignore_ascii_case("wip"))
        {
            return Ok(());
        }

        // Check max subject length
        if first_line.len() > hook_cfg.max_subject_length {
            anyhow::bail!(
                "Commit subject is {} characters, max allowed is {}",
                first_line.len(),
                hook_cfg.max_subject_length
            );
        }

        // Validate conventional commits format
        if hook_cfg.conventional_commits {
            use canaveral_changelog::ConventionalParser;
            use canaveral_changelog::CommitParser;
            use canaveral_git::CommitInfo;
            use chrono::Utc;

            let commit = CommitInfo::new("0000000", first_line, "", "", Utc::now());
            let parser = ConventionalParser::new();

            match parser.parse(&commit) {
                Some(parsed) => {
                    // Check allowed types if specified
                    if !hook_cfg.allowed_types.is_empty()
                        && !hook_cfg.allowed_types.contains(&parsed.commit_type)
                    {
                        anyhow::bail!(
                            "Commit type '{}' is not in allowed types: {}",
                            parsed.commit_type,
                            hook_cfg.allowed_types.join(", ")
                        );
                    }
                }
                None => {
                    anyhow::bail!(
                        "Commit message does not follow Conventional Commits format.\n\
                         Expected: <type>[optional scope]: <description>\n\
                         Example:  feat(auth): add login endpoint"
                    );
                }
            }
        }

        Ok(())
    }

    fn run_script_hook(&self, repo_root: &PathBuf, config_key: &str) -> anyhow::Result<()> {
        let config = load_config(repo_root)?;

        let hook_cfg = match config_key {
            "pre_commit" => &config.git_hooks.pre_commit,
            "pre_push" => &config.git_hooks.pre_push,
            _ => anyhow::bail!("Unknown script hook config key: {config_key}"),
        };

        if hook_cfg.commands.is_empty() {
            return Ok(());
        }

        for cmd_str in &hook_cfg.commands {
            info!(command = %cmd_str, "running hook command");

            let status = Command::new("sh")
                .arg("-c")
                .arg(cmd_str)
                .current_dir(repo_root)
                .status()?;

            if !status.success() {
                let code = status.code().unwrap_or(1);
                anyhow::bail!(
                    "Hook command failed (exit {}): {}",
                    code,
                    cmd_str
                );
            }
        }

        Ok(())
    }

    fn status(&self, repo_root: &PathBuf, cli: &Cli) -> anyhow::Result<()> {
        let statuses = hooks::status(repo_root);

        if !cli.quiet {
            println!("{}", style("Git hook status:").bold());
            for s in &statuses {
                let icon = if s.installed {
                    style("✓").green().bold()
                } else {
                    style("✗").red()
                };
                let state = if s.installed {
                    "installed"
                } else {
                    "not installed"
                };
                let backup_note = if s.has_backup { " (backup exists)" } else { "" };
                println!("  {icon} {:<12} {state}{backup_note}", s.hook_type.filename());
            }
        }

        Ok(())
    }
}

fn parse_hook_type(name: &str) -> anyhow::Result<GitHookType> {
    match name {
        "commit-msg" => Ok(GitHookType::CommitMsg),
        "pre-commit" => Ok(GitHookType::PreCommit),
        "pre-push" => Ok(GitHookType::PrePush),
        _ => anyhow::bail!(
            "Unknown hook type: '{name}'. Valid types: commit-msg, pre-commit, pre-push"
        ),
    }
}

fn find_repo_root() -> anyhow::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let mut path = cwd.as_path();
    loop {
        if path.join(".git").exists() {
            return Ok(path.to_path_buf());
        }
        path = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Not inside a git repository"))?;
    }
}

fn load_config(repo_root: &PathBuf) -> anyhow::Result<Config> {
    let yaml_path = repo_root.join("canaveral.yaml");
    let toml_path = repo_root.join("canaveral.toml");

    if yaml_path.exists() {
        let content = std::fs::read_to_string(&yaml_path)?;
        Ok(serde_yaml::from_str(&content)?)
    } else if toml_path.exists() {
        let content = std::fs::read_to_string(&toml_path)?;
        Ok(toml::from_str(&content)?)
    } else {
        // Return defaults if no config file
        Ok(Config::default())
    }
}
