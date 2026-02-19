use std::path::{Path, PathBuf};

use anyhow::Context;

use super::context::{
    AndroidBlock, ApiBlock, Block, DashboardBlock, ExpoBlock, IosBlock, PackageManager,
    ProjectContext, WebBlock,
};

pub const STATE_FILE: &str = "canaveral.scaffold.json";

#[derive(Debug, Clone)]
pub struct DetectedProject {
    pub root_dir: PathBuf,
    pub context: Option<ProjectContext>,
}

pub fn load_scaffold_state(root_dir: &Path) -> anyhow::Result<Option<ProjectContext>> {
    let path = root_dir.join(STATE_FILE);
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed reading {}", path.display()))?;
    let parsed: ProjectContext = serde_json::from_str(&content)
        .with_context(|| format!("failed parsing {}", path.display()))?;
    Ok(Some(parsed))
}

pub fn save_scaffold_state(root_dir: &Path, context: &ProjectContext) -> anyhow::Result<()> {
    let path = root_dir.join(STATE_FILE);
    let content = serde_json::to_string_pretty(context)?;
    std::fs::write(&path, content).with_context(|| format!("failed writing {}", path.display()))
}

pub fn detect_project(start_dir: &Path) -> anyhow::Result<Option<DetectedProject>> {
    if let Some(root_dir) = find_project_root(start_dir) {
        if let Some(context) = load_scaffold_state(&root_dir)? {
            return Ok(Some(DetectedProject {
                root_dir,
                context: Some(context),
            }));
        }
    }

    if let Some(root_dir) = find_project_root(start_dir) {
        let mut inferred = infer_context(&root_dir)?;
        inferred.apply_derivations();
        return Ok(Some(DetectedProject {
            root_dir,
            context: Some(inferred),
        }));
    }

    Ok(None)
}

fn find_project_root(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir.to_path_buf();
    loop {
        if current.join(STATE_FILE).exists() {
            return Some(current);
        }

        if current.join("package.json").exists()
            && (current.join("turbo.json").exists()
                || current.join("pnpm-workspace.yaml").exists()
                || current.join("apps").exists())
        {
            return Some(current);
        }

        if !current.pop() {
            break;
        }
    }
    None
}

fn infer_context(root_dir: &Path) -> anyhow::Result<ProjectContext> {
    let project_name = root_dir
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("project")
        .to_string();

    let mut ctx = ProjectContext::new(project_name.clone(), project_name);
    ctx.package_manager = detect_package_manager(root_dir);

    let apps_dir = root_dir.join("apps");
    if !apps_dir.exists() {
        return Ok(ctx);
    }

    for entry in std::fs::read_dir(&apps_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dirname = entry.file_name().to_string_lossy().to_string();
        let package_json = path.join("package.json");
        if !package_json.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&package_json)?;
        let value: serde_json::Value = serde_json::from_str(&content)?;
        let deps = value.get("dependencies").cloned().unwrap_or_default();

        if deps.get("hono").is_some() {
            ctx.blocks.push(Block::Api(ApiBlock {
                name: dirname.trim_start_matches("api-").to_string(),
                db: true,
                rate_limit: true,
                cors: true,
            }));
            continue;
        }

        if deps.get("astro").is_some() {
            ctx.blocks.push(Block::Web(WebBlock {
                name: dirname.trim_start_matches("web-").to_string(),
                blog: false,
                mdx: false,
            }));
            continue;
        }

        if deps.get("expo").is_some() {
            ctx.blocks.push(Block::Expo(ExpoBlock {
                name: dirname.trim_start_matches("app-").to_string(),
                api: None,
                e2e: false,
            }));
            continue;
        }

        if dirname.starts_with("dashboard-") {
            ctx.blocks.push(Block::Dashboard(DashboardBlock {
                name: dirname.trim_start_matches("dashboard-").to_string(),
                api: None,
                e2e: false,
            }));
            continue;
        }

        if dirname == "ios" || dirname.starts_with("ios-") {
            ctx.blocks.push(Block::Ios(IosBlock {
                name: dirname.trim_start_matches("ios-").to_string(),
                api: None,
                e2e: false,
            }));
            continue;
        }

        if dirname == "android" || dirname.starts_with("android-") {
            ctx.blocks.push(Block::Android(AndroidBlock {
                name: dirname.trim_start_matches("android-").to_string(),
                api: None,
                e2e: false,
            }));
        }
    }

    Ok(ctx)
}

fn detect_package_manager(root_dir: &Path) -> PackageManager {
    if root_dir.join("bun.lockb").exists() {
        return PackageManager::Bun;
    }
    if root_dir.join("pnpm-lock.yaml").exists() {
        return PackageManager::Pnpm;
    }
    if root_dir.join("package-lock.json").exists() {
        return PackageManager::Npm;
    }
    PackageManager::Bun
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn state_file_has_priority() {
        let temp = TempDir::new().expect("tempdir");
        let root = temp.path();

        std::fs::write(root.join("package.json"), "{}")
            .expect("write package.json for detector root conditions");
        std::fs::write(root.join("turbo.json"), "{}")
            .expect("write turbo.json for monorepo detection");

        let ctx = ProjectContext::new("a".to_string(), "a".to_string());
        save_scaffold_state(root, &ctx).expect("save scaffold state");

        let detected = detect_project(root)
            .expect("detect project should not fail")
            .expect("project should be detected");

        assert!(detected.context.is_some());
        assert_eq!(detected.root_dir, root.to_path_buf());
    }

    #[test]
    fn state_round_trip_serialization() {
        let temp = TempDir::new().expect("tempdir");
        let root = temp.path();
        let mut ctx = ProjectContext::new("demo".to_string(), "demo".to_string());
        ctx.blocks.push(Block::Api(ApiBlock {
            name: "main".to_string(),
            db: true,
            rate_limit: true,
            cors: true,
        }));

        save_scaffold_state(root, &ctx).expect("save state");
        let loaded = load_scaffold_state(root)
            .expect("load state")
            .expect("state should exist");

        assert_eq!(loaded.project_slug, "demo");
        assert_eq!(loaded.blocks.len(), 1);
    }
}
