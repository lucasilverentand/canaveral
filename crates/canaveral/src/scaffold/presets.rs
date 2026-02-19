use super::context::{
    AndroidBlock, ApiBlock, Block, DashboardBlock, ExpoBlock, IosBlock, ProjectContext, WebBlock,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Preset {
    Fullstack,
}

impl Preset {
    pub fn as_str(self) -> &'static str {
        match self {
            Preset::Fullstack => "fullstack",
        }
    }
}

pub fn apply_preset(ctx: &mut ProjectContext, preset: Preset) {
    match preset {
        Preset::Fullstack => {
            ctx.preset = Some("fullstack".to_string());
            ctx.auth.enabled = true;
            ctx.auth.roles = true;

            ctx.packages.db = true;
            ctx.packages.auth = true;
            ctx.packages.email = true;
            ctx.packages.ui = true;
            ctx.packages.ui_native = true;
            ctx.packages.api_client = true;
            ctx.packages.utils = true;
            ctx.packages.types = true;
            ctx.packages.config = true;
            ctx.packages.test_utils = true;

            ctx.tooling.biome = true;
            ctx.tooling.turbo = true;
            ctx.tooling.github = true;
            ctx.tooling.typescript = true;
            ctx.tooling.tailwind = true;
            ctx.tooling.localflare = true;

            ctx.blocks = vec![
                Block::Api(ApiBlock {
                    name: "auth".to_string(),
                    db: true,
                    rate_limit: true,
                    cors: true,
                }),
                Block::Api(ApiBlock {
                    name: "user".to_string(),
                    db: true,
                    rate_limit: true,
                    cors: true,
                }),
                Block::Api(ApiBlock {
                    name: "admin".to_string(),
                    db: true,
                    rate_limit: true,
                    cors: true,
                }),
                Block::Dashboard(DashboardBlock {
                    name: "admin".to_string(),
                    api: Some("admin".to_string()),
                    e2e: true,
                }),
                Block::Web(WebBlock {
                    name: "marketing".to_string(),
                    blog: false,
                    mdx: false,
                }),
                Block::Expo(ExpoBlock {
                    name: "expo".to_string(),
                    api: Some("user".to_string()),
                    e2e: true,
                }),
                Block::Ios(IosBlock {
                    name: "ios".to_string(),
                    api: Some("user".to_string()),
                    e2e: true,
                }),
                Block::Android(AndroidBlock {
                    name: "android".to_string(),
                    api: Some("user".to_string()),
                    e2e: true,
                }),
            ];
        }
    }

    ctx.apply_derivations();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullstack_sets_blocks_and_preset() {
        let mut ctx = ProjectContext::new("p".to_string(), "p".to_string());
        apply_preset(&mut ctx, Preset::Fullstack);
        assert_eq!(ctx.preset.as_deref(), Some("fullstack"));
        assert!(!ctx.blocks.is_empty());
    }
}
