use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Bun,
    Pnpm,
    Npm,
}

impl PackageManager {
    pub fn install_cmd(self) -> &'static str {
        match self {
            Self::Bun => "bun install",
            Self::Pnpm => "pnpm install",
            Self::Npm => "npm install",
        }
    }

    pub fn run_prefix(self) -> &'static str {
        match self {
            Self::Npm => "npm run",
            Self::Bun => "bun",
            Self::Pnpm => "pnpm",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum BlockType {
    Api,
    Dashboard,
    Web,
    Expo,
    Ios,
    Android,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub workspaces: bool,
    pub roles: bool,
    pub social_providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingConfig {
    pub stripe: bool,
    pub iap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagesConfig {
    pub db: bool,
    pub auth: bool,
    pub email: bool,
    pub ui: bool,
    pub ui_native: bool,
    pub api_client: bool,
    pub utils: bool,
    pub types: bool,
    pub config: bool,
    pub test_utils: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolingConfig {
    pub biome: bool,
    pub turbo: bool,
    pub github: bool,
    pub typescript: bool,
    pub tailwind: bool,
    pub localflare: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiBlock {
    pub name: String,
    pub db: bool,
    pub rate_limit: bool,
    pub cors: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardBlock {
    pub name: String,
    pub api: Option<String>,
    pub e2e: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebBlock {
    pub name: String,
    pub blog: bool,
    pub mdx: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpoBlock {
    pub name: String,
    pub api: Option<String>,
    pub e2e: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IosBlock {
    pub name: String,
    pub api: Option<String>,
    pub e2e: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AndroidBlock {
    pub name: String,
    pub api: Option<String>,
    pub e2e: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Block {
    Api(ApiBlock),
    Dashboard(DashboardBlock),
    Web(WebBlock),
    Expo(ExpoBlock),
    Ios(IosBlock),
    Android(AndroidBlock),
}

impl Block {
    pub fn block_type(&self) -> BlockType {
        match self {
            Block::Api(_) => BlockType::Api,
            Block::Dashboard(_) => BlockType::Dashboard,
            Block::Web(_) => BlockType::Web,
            Block::Expo(_) => BlockType::Expo,
            Block::Ios(_) => BlockType::Ios,
            Block::Android(_) => BlockType::Android,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Block::Api(v) => &v.name,
            Block::Dashboard(v) => &v.name,
            Block::Web(v) => &v.name,
            Block::Expo(v) => &v.name,
            Block::Ios(v) => &v.name,
            Block::Android(v) => &v.name,
        }
    }

    pub fn api_target(&self) -> Option<&str> {
        match self {
            Block::Dashboard(v) => v.api.as_deref(),
            Block::Expo(v) => v.api.as_deref(),
            Block::Ios(v) => v.api.as_deref(),
            Block::Android(v) => v.api.as_deref(),
            _ => None,
        }
    }

    pub fn uses_e2e(&self) -> bool {
        match self {
            Block::Dashboard(v) => v.e2e,
            Block::Expo(v) => v.e2e,
            Block::Ios(v) => v.e2e,
            Block::Android(v) => v.e2e,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectContext {
    pub version: String,
    pub preset: Option<String>,
    pub project_name: String,
    pub project_slug: String,
    pub package_manager: PackageManager,
    pub auth: AuthConfig,
    pub billing: BillingConfig,
    pub packages: PackagesConfig,
    pub tooling: ToolingConfig,
    pub blocks: Vec<Block>,
}

impl ProjectContext {
    pub fn new(project_name: String, project_slug: String) -> Self {
        Self {
            version: "1.0.0".to_string(),
            preset: None,
            project_name,
            project_slug,
            package_manager: PackageManager::Bun,
            auth: AuthConfig {
                enabled: false,
                workspaces: false,
                roles: false,
                social_providers: Vec::new(),
            },
            billing: BillingConfig {
                stripe: false,
                iap: false,
            },
            packages: PackagesConfig {
                db: false,
                auth: false,
                email: false,
                ui: false,
                ui_native: false,
                api_client: false,
                utils: true,
                types: true,
                config: true,
                test_utils: false,
            },
            tooling: ToolingConfig {
                biome: true,
                turbo: true,
                github: true,
                typescript: true,
                tailwind: false,
                localflare: false,
            },
            blocks: Vec::new(),
        }
    }

    pub fn has_api_named(&self, name: &str) -> bool {
        self.blocks.iter().any(|block| match block {
            Block::Api(api) => api.name == name,
            _ => false,
        })
    }

    pub fn apply_derivations(&mut self) {
        let mut has_api = false;
        let mut has_dashboard_or_web = false;
        let mut has_native = false;
        let mut has_frontend = false;
        let mut has_e2e = false;
        let mut has_api_with_db = false;

        for block in &self.blocks {
            match block {
                Block::Api(api) => {
                    has_api = true;
                    has_api_with_db |= api.db;
                }
                Block::Dashboard(_) | Block::Web(_) => {
                    has_frontend = true;
                    has_dashboard_or_web = true;
                }
                Block::Expo(_) | Block::Ios(_) | Block::Android(_) => {
                    has_frontend = true;
                    has_native = true;
                }
            }
            has_e2e |= block.uses_e2e();
        }

        self.packages.db = self.packages.db || has_api_with_db;
        self.packages.auth = self.packages.auth || self.auth.enabled;
        self.packages.email = self.packages.email || self.packages.auth;
        self.packages.ui = self.packages.ui || has_dashboard_or_web;
        self.packages.ui_native = self.packages.ui_native || has_native;
        self.packages.api_client = self.packages.api_client || (has_api && has_frontend);
        self.packages.test_utils = self.packages.test_utils || has_e2e;

        self.tooling.tailwind = self.tooling.tailwind || has_dashboard_or_web;
        self.tooling.localflare = self.tooling.localflare || has_api;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_enables_expected_packages() {
        let mut ctx = ProjectContext::new("a".to_string(), "a".to_string());
        ctx.blocks.push(Block::Api(ApiBlock {
            name: "main".to_string(),
            db: true,
            rate_limit: true,
            cors: true,
        }));
        ctx.blocks.push(Block::Web(WebBlock {
            name: "marketing".to_string(),
            blog: false,
            mdx: false,
        }));

        ctx.apply_derivations();

        assert!(ctx.packages.db);
        assert!(ctx.packages.ui);
        assert!(ctx.packages.api_client);
        assert!(ctx.tooling.localflare);
    }
}
