use anyhow::{Context, Result};
use serde::Deserialize;
use std::sync::Arc;
use utils::config::{AnyChainConfig, ChainConfigs, OptionalWavsChainConfig};

use crate::args::WavsArgs;

#[derive(Clone)]
pub struct WavsContext {
    inner: Arc<WavsContextInner>,
}

impl std::ops::Deref for WavsContext {
    type Target = WavsContextInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct WavsContextInner {
    pub args: WavsArgs,
    pub chain_config: AnyChainConfig,
}

impl WavsContext {
    pub fn new(args: WavsArgs) -> Result<Self> {
        #[derive(Debug, Deserialize)]
        struct PartialWavsConfig {
            pub chains: ChainConfigs,
            #[serde(flatten)]
            pub chain_config_override: OptionalWavsChainConfig,
        }

        let config =
            std::fs::read_to_string(&args.config_filepath).context("Could not read config file")?;
        let config: PartialWavsConfig =
            toml::from_str(&config).context("Could not parse config file")?;

        let chains = config
            .chains
            .merge_overrides(&config.chain_config_override)?;

        let chain_config = chains
            .get_chain(&args.chain)?
            .context(format!("No chain config found for {}", args.chain))?;

        Ok(Self {
            inner: Arc::new(WavsContextInner { args, chain_config }),
        })
    }
}
