use anyhow::{Context, Result};
use serde::Deserialize;
use std::sync::Arc;
use utils::config::{
    ChainConfigs, CosmosChainConfig, EthereumChainConfig, OptionalWavsChainConfig,
};

use crate::args::{ChainKind, WavsArgs};

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
    pub chain_config: WavsChainConfig,
}

#[allow(dead_code)]
pub enum WavsChainConfig {
    Cosmos(CosmosChainConfig),
    Eth(EthereumChainConfig),
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
            std::fs::read_to_string(&args.config_filepath).expect("Could not read config file");
        let config: PartialWavsConfig =
            toml::from_str(&config).expect("Could not parse config file");

        let chains = config
            .chains
            .merge_overrides(&config.chain_config_override)?;

        let chain_config = args
            .chain_kind
            .map(|kind| match kind {
                ChainKind::Cosmos => chains
                    .cosmos
                    .get(&args.chain)
                    .cloned()
                    .map(WavsChainConfig::Cosmos),
                ChainKind::Eth => chains
                    .eth
                    .get(&args.chain)
                    .cloned()
                    .map(WavsChainConfig::Eth),
            })
            .unwrap_or_else(|| match chains.cosmos.get(&args.chain).cloned() {
                Some(chain) => Some(WavsChainConfig::Cosmos(chain)),
                None => chains
                    .eth
                    .get(&args.chain)
                    .cloned()
                    .map(WavsChainConfig::Eth),
            })
            .context(format!("No chain config found for: {}", args.chain))?;

        Ok(Self {
            inner: Arc::new(WavsContextInner { args, chain_config }),
        })
    }
}
