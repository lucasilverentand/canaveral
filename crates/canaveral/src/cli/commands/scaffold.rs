//! Project scaffolding command

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Args, Subcommand};
use console::style;
use serde::Serialize;
use tracing::info;

use crate::cli::output::Ui;
use crate::cli::Cli;
use crate::scaffold::context::{
    AndroidBlock, ApiBlock, BillingConfig, Block, BlockType, DashboardBlock, ExpoBlock, IosBlock,
    PackageManager, ProjectContext, WebBlock,
};
use crate::scaffold::generator::{generate_block, generate_project};
use crate::scaffold::presets::{apply_preset, Preset};
use crate::scaffold::project_detector::{detect_project, save_scaffold_state};
use crate::scaffold::registry::{registry, validate_block_addition};

#[derive(Debug, Args)]
pub struct ScaffoldCommand {
    #[command(subcommand)]
    pub subcommand: Option<ScaffoldSubcommand>,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub legacy_args: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum ScaffoldSubcommand {
    New(ScaffoldNewCommand),
    Add(ScaffoldAddCommand),
    List(ScaffoldListCommand),
}

#[derive(Debug, Args)]
pub struct ScaffoldNewCommand {
    pub name: Option<String>,
    #[arg(long)]
    pub preset: Option<Preset>,
    #[arg(long)]
    pub package_manager: Option<PackageManager>,
    #[arg(short = 'y', long)]
    pub yes: bool,
    #[arg(long)]
    pub no_git: bool,
    #[arg(long)]
    pub no_install: bool,
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    #[arg(short, long)]
    pub force: bool,
    #[arg(long)]
    pub api: bool,
    #[arg(long)]
    pub dashboard: bool,
    #[arg(long)]
    pub web: bool,
    #[arg(long)]
    pub expo: bool,
    #[arg(long)]
    pub ios: bool,
    #[arg(long)]
    pub android: bool,
    #[arg(long)]
    pub db: bool,
    #[arg(long)]
    pub auth: bool,
}

#[derive(Debug, Args)]
pub struct ScaffoldAddCommand {
    pub block_type: BlockType,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(short = 'y', long)]
    pub yes: bool,
    #[arg(long)]
    pub api: Option<String>,
    #[arg(long)]
    pub e2e: Option<bool>,
    #[arg(long)]
    pub db: Option<bool>,
    #[arg(long)]
    pub no_rate_limit: bool,
    #[arg(long)]
    pub no_cors: bool,
    #[arg(long)]
    pub blog: Option<bool>,
    #[arg(long)]
    pub mdx: Option<bool>,
}

#[derive(Debug, Args)]
pub struct ScaffoldListCommand {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Serialize)]
struct ScaffoldCatalog {
    presets: Vec<&'static str>,
    blocks: Vec<&'static str>,
}

impl ScaffoldCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!("executing scaffold command");
        match &self.subcommand {
            Some(ScaffoldSubcommand::New(cmd)) => cmd.execute(cli),
            Some(ScaffoldSubcommand::Add(cmd)) => cmd.execute(cli),
            Some(ScaffoldSubcommand::List(cmd)) => cmd.execute(cli),
            None => {
                if !self.legacy_args.is_empty() {
                    anyhow::bail!(
                        "`canaveral scaffold <template>` was removed. Use `canaveral scaffold new` or `canaveral scaffold list`."
                    );
                }
                anyhow::bail!(
                    "Missing scaffold subcommand. Use `canaveral scaffold new`, `canaveral scaffold add`, or `canaveral scaffold list`."
                )
            }
        }
    }
}

impl ScaffoldListCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let reg = registry();
        let mut blocks: Vec<&'static str> = reg
            .all_specs()
            .into_iter()
            .map(|spec| match spec.block_type {
                BlockType::Api => "api",
                BlockType::Dashboard => "dashboard",
                BlockType::Web => "web",
                BlockType::Expo => "expo",
                BlockType::Ios => "ios",
                BlockType::Android => "android",
            })
            .collect();
        blocks.sort_unstable();

        let catalog = ScaffoldCatalog {
            presets: vec!["fullstack"],
            blocks,
        };

        if self.json {
            println!("{}", serde_json::to_string_pretty(&catalog)?);
            return Ok(());
        }

        ui.header("Scaffold presets:");
        for preset in &catalog.presets {
            println!("  {}", style(preset).cyan());
        }
        ui.blank();
        ui.header("Scaffold blocks:");
        for block in &catalog.blocks {
            println!("  {}", style(block).green());
        }

        Ok(())
    }
}

impl ScaffoldNewCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let cwd = std::env::current_dir()?;
        let project_name = match &self.name {
            Some(name) => name.clone(),
            None if self.yes => "my-project".to_string(),
            None => ui.input("Project name", "my-project")?,
        };

        let slug = slugify(&project_name);
        if slug.is_empty() {
            anyhow::bail!("Project name must contain at least one alphanumeric character");
        }

        let output_dir = resolve_output_dir(&cwd, self.output.as_ref(), &slug);
        if output_dir.exists() && !self.force {
            anyhow::bail!(
                "Output directory already exists: {}. Use --force to overwrite.",
                output_dir.display()
            );
        }

        let package_manager = match self.package_manager {
            Some(pm) => pm,
            None if self.yes => PackageManager::Bun,
            None => match ui.select("Package manager", &["bun", "pnpm", "npm"], 0)? {
                0 => PackageManager::Bun,
                1 => PackageManager::Pnpm,
                _ => PackageManager::Npm,
            },
        };

        let mut ctx = ProjectContext::new(project_name.clone(), slug);
        ctx.package_manager = package_manager;

        if let Some(preset) = self.preset {
            apply_preset(&mut ctx, preset);
        } else if self.yes {
            apply_preset(&mut ctx, Preset::Fullstack);
        }

        apply_block_overrides(self, &mut ctx);
        apply_package_overrides(self, &mut ctx);

        if !self.yes {
            prompt_business_choices(&ui, &mut ctx)?;
            if self.preset.is_none() && !any_block_flag_set(self) {
                prompt_block_selection(&ui, &mut ctx)?;
            }
        }

        ctx.apply_derivations();
        validate_api_links(&ctx)?;

        let generated = generate_project(&ctx, &output_dir)?;
        save_scaffold_state(&output_dir, &ctx)?;

        let do_git = if self.no_git {
            false
        } else if self.yes {
            true
        } else {
            ui.confirm("Initialize git repository?", true)?
        };

        let do_install = if self.no_install {
            false
        } else if self.yes {
            true
        } else {
            ui.confirm("Install dependencies?", true)?
        };

        if do_git {
            run_shell_command(&output_dir, "git init")?;
        }
        if do_install {
            run_shell_command(&output_dir, ctx.package_manager.install_cmd())?;
        }

        ui.success(&format!(
            "Scaffolded project at {}",
            ui.fmt_path(&output_dir.display())
        ));
        ui.hint(&format!("{} files created", generated.len()));
        ui.blank();
        ui.header("Next steps:");
        ui.hint(&format!("cd {}", ui.fmt_path(&output_dir.display())));
        if !do_install {
            ui.hint(ctx.package_manager.install_cmd());
        }
        ui.hint(&format!("{} dev", ctx.package_manager.run_prefix()));

        Ok(())
    }
}

impl ScaffoldAddCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let cwd = std::env::current_dir()?;
        let detected = detect_project(&cwd)?.ok_or_else(|| {
            anyhow::anyhow!("No scaffolded project found. Run `canaveral scaffold new` first.")
        })?;

        let root = detected.root_dir;
        let mut ctx = detected.context.ok_or_else(|| {
            anyhow::anyhow!("Project detected but scaffold state is unavailable or invalid")
        })?;

        let reg = registry();
        let spec = reg
            .get(self.block_type)
            .ok_or_else(|| anyhow::anyhow!("Unknown block type"))?;

        let block_name = self
            .name
            .clone()
            .unwrap_or_else(|| spec.default_name.to_string());
        let block = build_block_from_flags(self, block_name, &ctx)?;

        validate_block_addition(&ctx, &block)?;

        let generated = generate_block(&ctx, &block, &root)?;
        ctx.blocks.push(block);
        ctx.apply_derivations();
        save_scaffold_state(&root, &ctx)?;

        ui.success(&format!(
            "Added {} block",
            style(format!("{:?}", self.block_type).to_lowercase()).cyan()
        ));
        ui.hint(&format!("{} files created", generated.len()));

        Ok(())
    }
}

fn apply_block_overrides(cmd: &ScaffoldNewCommand, ctx: &mut ProjectContext) {
    if !any_block_flag_set(cmd) {
        return;
    }
    ctx.blocks.clear();
    if cmd.api {
        ctx.blocks.push(Block::Api(ApiBlock {
            name: "main".to_string(),
            db: true,
            rate_limit: true,
            cors: true,
        }));
    }
    if cmd.dashboard {
        ctx.blocks.push(Block::Dashboard(DashboardBlock {
            name: "app".to_string(),
            api: Some("main".to_string()),
            e2e: true,
        }));
    }
    if cmd.web {
        ctx.blocks.push(Block::Web(WebBlock {
            name: "marketing".to_string(),
            blog: false,
            mdx: false,
        }));
    }
    if cmd.expo {
        ctx.blocks.push(Block::Expo(ExpoBlock {
            name: "expo".to_string(),
            api: Some("main".to_string()),
            e2e: true,
        }));
    }
    if cmd.ios {
        ctx.blocks.push(Block::Ios(IosBlock {
            name: "ios".to_string(),
            api: Some("main".to_string()),
            e2e: true,
        }));
    }
    if cmd.android {
        ctx.blocks.push(Block::Android(AndroidBlock {
            name: "android".to_string(),
            api: Some("main".to_string()),
            e2e: true,
        }));
    }
}

fn apply_package_overrides(cmd: &ScaffoldNewCommand, ctx: &mut ProjectContext) {
    if cmd.db {
        ctx.packages.db = true;
    }
    if cmd.auth {
        ctx.packages.auth = true;
        ctx.auth.enabled = true;
    }
}

fn prompt_business_choices(ui: &Ui, ctx: &mut ProjectContext) -> anyhow::Result<()> {
    ctx.auth.workspaces = ui.confirm("Users belong to workspaces/organizations?", false)?;
    ctx.auth.roles = ui.confirm("Enable role-based access?", false)?;

    let providers = ui.multi_select("Social login providers", &["google", "github", "apple"])?;
    ctx.auth.social_providers = providers
        .into_iter()
        .filter_map(|idx| match idx {
            0 => Some("google".to_string()),
            1 => Some("github".to_string()),
            2 => Some("apple".to_string()),
            _ => None,
        })
        .collect();

    let billing = ui.select("Billing model", &["none", "stripe", "iap", "both"], 0)?;
    ctx.billing = match billing {
        1 => BillingConfig {
            stripe: true,
            iap: false,
        },
        2 => BillingConfig {
            stripe: false,
            iap: true,
        },
        3 => BillingConfig {
            stripe: true,
            iap: true,
        },
        _ => BillingConfig {
            stripe: false,
            iap: false,
        },
    };
    Ok(())
}

fn prompt_block_selection(ui: &Ui, ctx: &mut ProjectContext) -> anyhow::Result<()> {
    let selected = ui.multi_select(
        "Select blocks",
        &["api", "dashboard", "web", "expo", "ios", "android"],
    )?;

    if selected.is_empty() {
        return Ok(());
    }

    ctx.blocks.clear();
    if selected.contains(&0) {
        ctx.blocks.push(Block::Api(ApiBlock {
            name: "main".to_string(),
            db: true,
            rate_limit: true,
            cors: true,
        }));
    }
    if selected.contains(&1) {
        ctx.blocks.push(Block::Dashboard(DashboardBlock {
            name: "app".to_string(),
            api: Some("main".to_string()),
            e2e: true,
        }));
    }
    if selected.contains(&2) {
        ctx.blocks.push(Block::Web(WebBlock {
            name: "marketing".to_string(),
            blog: false,
            mdx: false,
        }));
    }
    if selected.contains(&3) {
        ctx.blocks.push(Block::Expo(ExpoBlock {
            name: "expo".to_string(),
            api: Some("main".to_string()),
            e2e: true,
        }));
    }
    if selected.contains(&4) {
        ctx.blocks.push(Block::Ios(IosBlock {
            name: "ios".to_string(),
            api: Some("main".to_string()),
            e2e: true,
        }));
    }
    if selected.contains(&5) {
        ctx.blocks.push(Block::Android(AndroidBlock {
            name: "android".to_string(),
            api: Some("main".to_string()),
            e2e: true,
        }));
    }
    Ok(())
}

fn build_block_from_flags(
    cmd: &ScaffoldAddCommand,
    name: String,
    ctx: &ProjectContext,
) -> anyhow::Result<Block> {
    let api_link = cmd.api.as_deref().and_then(|v| {
        if v.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(v.to_string())
        }
    });

    let block = match cmd.block_type {
        BlockType::Api => Block::Api(ApiBlock {
            name,
            db: cmd.db.unwrap_or(true),
            rate_limit: !cmd.no_rate_limit,
            cors: !cmd.no_cors,
        }),
        BlockType::Dashboard => Block::Dashboard(DashboardBlock {
            name,
            api: api_link.or_else(|| first_api(ctx)),
            e2e: cmd.e2e.unwrap_or(true),
        }),
        BlockType::Web => Block::Web(WebBlock {
            name,
            blog: cmd.blog.unwrap_or(false),
            mdx: cmd.mdx.unwrap_or(false),
        }),
        BlockType::Expo => Block::Expo(ExpoBlock {
            name,
            api: api_link.or_else(|| first_api(ctx)),
            e2e: cmd.e2e.unwrap_or(true),
        }),
        BlockType::Ios => Block::Ios(IosBlock {
            name,
            api: api_link.or_else(|| first_api(ctx)),
            e2e: cmd.e2e.unwrap_or(true),
        }),
        BlockType::Android => Block::Android(AndroidBlock {
            name,
            api: api_link.or_else(|| first_api(ctx)),
            e2e: cmd.e2e.unwrap_or(true),
        }),
    };
    Ok(block)
}

fn first_api(ctx: &ProjectContext) -> Option<String> {
    ctx.blocks.iter().find_map(|block| match block {
        Block::Api(api) => Some(api.name.clone()),
        _ => None,
    })
}

fn validate_api_links(ctx: &ProjectContext) -> anyhow::Result<()> {
    for block in &ctx.blocks {
        if let Some(api) = block.api_target() {
            if !ctx.has_api_named(api) {
                anyhow::bail!("Block '{}' references missing API '{}'.", block.name(), api);
            }
        }
    }
    Ok(())
}

fn any_block_flag_set(cmd: &ScaffoldNewCommand) -> bool {
    cmd.api || cmd.dashboard || cmd.web || cmd.expo || cmd.ios || cmd.android
}

fn resolve_output_dir(cwd: &Path, output: Option<&PathBuf>, name: &str) -> PathBuf {
    output
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            }
        })
        .unwrap_or_else(|| cwd.join(name))
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars().map(|c| c.to_ascii_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if (ch == '-' || ch == '_' || ch.is_whitespace()) && !slug.is_empty() && !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn run_shell_command(cwd: &Path, command: &str) -> anyhow::Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .status()?;
    if !status.success() {
        anyhow::bail!(
            "Command failed ({}): {}",
            status.code().unwrap_or(1),
            command
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_normalizes_project_names() {
        assert_eq!(slugify("My Project"), "my-project");
        assert_eq!(slugify("abc___DEF"), "abc-def");
    }
}
