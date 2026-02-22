use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;

use super::context::{Block, PackageManager, ProjectContext};
use super::errors::ScaffoldError;
use super::registry::registry;
use super::templates::{list_template_files, load_template, render_template, template_root};

pub fn generate_project(
    context: &ProjectContext,
    target_dir: &Path,
) -> anyhow::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(target_dir)?;

    let mut written = Vec::new();

    written.extend(generate_root(context, target_dir)?);
    written.extend(generate_packages(context, target_dir)?);

    for block in &context.blocks {
        written.extend(generate_block(context, block, target_dir)?);
    }

    Ok(written)
}

pub fn generate_block(
    context: &ProjectContext,
    block: &Block,
    target_dir: &Path,
) -> anyhow::Result<Vec<PathBuf>> {
    let reg = registry();
    let spec = reg
        .get(block.block_type())
        .ok_or_else(|| ScaffoldError::UnknownBlock(format!("{:?}", block.block_type())))?;

    let template_root = template_root()?;
    let block_template_dir = template_root.join("blocks").join(spec.app_prefix);

    if !block_template_dir.exists() {
        return Err(
            ScaffoldError::TemplateFileMissing(format!("blocks/{}", spec.app_prefix)).into(),
        );
    }

    let mut vars = base_vars(context);
    vars.insert("block_name".to_string(), block.name().to_string());
    vars.insert(
        "app_dir".to_string(),
        format!("apps/{}-{}", spec.app_prefix, block.name()),
    );

    if let Some(api) = block.api_target() {
        vars.insert("api_block_name".to_string(), api.to_string());
    } else {
        vars.insert("api_block_name".to_string(), "none".to_string());
    }

    let mut written = Vec::new();
    for relative in list_template_files(&block_template_dir)? {
        let source_rel = format!("blocks/{}/{}", spec.app_prefix, relative.display());
        let source = load_template(&source_rel)?;
        let rendered = render_template(&source, &vars);

        let output_rel = relative.to_string_lossy().replace(
            "__APP_DIR__",
            &format!("apps/{}-{}", spec.app_prefix, block.name()),
        );

        let output_path = target_dir.join(output_rel);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&output_path, rendered)
            .with_context(|| format!("failed writing {}", output_path.display()))?;
        written.push(output_path);
    }

    Ok(written)
}

fn generate_root(context: &ProjectContext, target_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    let vars = base_vars(context);

    for file in [
        "root/package.json",
        "root/turbo.json",
        "root/.gitignore",
        "root/README.md",
    ] {
        let content = load_template(file)?;
        let rendered = render_template(&content, &vars);
        let filename = file
            .strip_prefix("root/")
            .ok_or_else(|| anyhow::anyhow!("invalid root template path"))?;
        let path = target_dir.join(filename);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, rendered)?;
        written.push(path);
    }

    Ok(written)
}

fn generate_packages(context: &ProjectContext, target_dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let flags = [
        ("db", context.packages.db),
        ("auth", context.packages.auth),
        ("email", context.packages.email),
        ("ui", context.packages.ui),
        ("ui-native", context.packages.ui_native),
        ("api-client", context.packages.api_client),
        ("utils", context.packages.utils),
        ("types", context.packages.types),
        ("config", context.packages.config),
        ("test-utils", context.packages.test_utils),
    ];

    let mut written = Vec::new();
    for (pkg, enabled) in flags {
        if !enabled {
            continue;
        }

        let template_dir = format!("packages/{pkg}");
        let root = template_root()?.join(&template_dir);
        if !root.exists() {
            continue;
        }

        let mut vars = base_vars(context);
        vars.insert("package_name".to_string(), pkg.to_string());

        for relative in list_template_files(&root)? {
            let source_rel = format!("{template_dir}/{}", relative.display());
            let source = load_template(&source_rel)?;
            let rendered = render_template(&source, &vars);
            let output_rel = relative
                .to_string_lossy()
                .replace("__PACKAGE_DIR__", &format!("packages/{pkg}"));
            let output_path = target_dir.join(output_rel);
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&output_path, rendered)?;
            written.push(output_path);
        }
    }

    Ok(written)
}

fn base_vars(context: &ProjectContext) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("project_name".to_string(), context.project_name.clone());
    vars.insert("project_slug".to_string(), context.project_slug.clone());
    vars.insert(
        "package_manager".to_string(),
        match context.package_manager {
            PackageManager::Bun => "bun",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Npm => "npm",
        }
        .to_string(),
    );
    vars.insert(
        "package_manager_spec".to_string(),
        match context.package_manager {
            PackageManager::Bun => "bun@1.0.0".to_string(),
            PackageManager::Pnpm => "pnpm@9.0.0".to_string(),
            PackageManager::Npm => "npm@10.0.0".to_string(),
        },
    );
    vars.insert(
        "run_dev".to_string(),
        format!("{} dev", context.package_manager.run_prefix()),
    );
    vars
}
