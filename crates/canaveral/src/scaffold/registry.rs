use std::collections::HashMap;

use super::context::{Block, BlockType, ProjectContext};
use super::errors::ScaffoldError;

#[derive(Debug, Clone)]
pub struct BlockSpec {
    pub block_type: BlockType,
    pub app_prefix: &'static str,
    pub default_name: &'static str,
    pub singleton: bool,
}

#[derive(Debug, Clone)]
pub struct BlockRegistry {
    specs: HashMap<BlockType, BlockSpec>,
}

impl BlockRegistry {
    pub fn get(&self, block_type: BlockType) -> Option<&BlockSpec> {
        self.specs.get(&block_type)
    }

    pub fn all_specs(&self) -> Vec<&BlockSpec> {
        let mut values: Vec<&BlockSpec> = self.specs.values().collect();
        values.sort_by_key(|spec| spec.default_name);
        values
    }
}

pub fn registry() -> BlockRegistry {
    let mut specs = HashMap::new();
    specs.insert(
        BlockType::Api,
        BlockSpec {
            block_type: BlockType::Api,
            app_prefix: "api",
            default_name: "main",
            singleton: false,
        },
    );
    specs.insert(
        BlockType::Dashboard,
        BlockSpec {
            block_type: BlockType::Dashboard,
            app_prefix: "dashboard",
            default_name: "app",
            singleton: false,
        },
    );
    specs.insert(
        BlockType::Web,
        BlockSpec {
            block_type: BlockType::Web,
            app_prefix: "web",
            default_name: "marketing",
            singleton: false,
        },
    );
    specs.insert(
        BlockType::Expo,
        BlockSpec {
            block_type: BlockType::Expo,
            app_prefix: "app",
            default_name: "expo",
            singleton: true,
        },
    );
    specs.insert(
        BlockType::Ios,
        BlockSpec {
            block_type: BlockType::Ios,
            app_prefix: "ios",
            default_name: "ios",
            singleton: true,
        },
    );
    specs.insert(
        BlockType::Android,
        BlockSpec {
            block_type: BlockType::Android,
            app_prefix: "android",
            default_name: "android",
            singleton: true,
        },
    );

    BlockRegistry { specs }
}

pub fn validate_block_addition(ctx: &ProjectContext, block: &Block) -> Result<(), ScaffoldError> {
    let reg = registry();
    let block_type = block.block_type();
    let name = block.name();

    if let Some(spec) = reg.get(block_type) {
        if spec.singleton
            && ctx
                .blocks
                .iter()
                .any(|existing| existing.block_type() == block_type)
        {
            return Err(ScaffoldError::SingletonBlock(
                format!("{:?}", block_type).to_lowercase(),
            ));
        }
    }

    if ctx
        .blocks
        .iter()
        .any(|existing| existing.block_type() == block_type && existing.name() == name)
    {
        return Err(ScaffoldError::DuplicateBlock(
            format!("{:?}", block_type).to_lowercase(),
            name.to_string(),
        ));
    }

    if let Some(api) = block.api_target() {
        if !ctx.has_api_named(api) {
            return Err(ScaffoldError::InvalidApiLink(api.to_string()));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scaffold::context::{ApiBlock, DashboardBlock};

    #[test]
    fn duplicate_block_rejected() {
        let mut ctx = ProjectContext::new("p".to_string(), "p".to_string());
        ctx.blocks.push(Block::Api(ApiBlock {
            name: "main".to_string(),
            db: true,
            rate_limit: true,
            cors: true,
        }));

        let dup = Block::Api(ApiBlock {
            name: "main".to_string(),
            db: false,
            rate_limit: true,
            cors: true,
        });

        assert!(validate_block_addition(&ctx, &dup).is_err());
    }

    #[test]
    fn invalid_api_link_rejected() {
        let ctx = ProjectContext::new("p".to_string(), "p".to_string());
        let block = Block::Dashboard(DashboardBlock {
            name: "admin".to_string(),
            api: Some("missing".to_string()),
            e2e: false,
        });
        assert!(validate_block_addition(&ctx, &block).is_err());
    }
}
