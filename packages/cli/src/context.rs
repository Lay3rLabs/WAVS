use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::sync::Arc;
use utils::config::{
    ChainConfigs, CosmosChainConfig, EthereumChainConfig, OptionalWavsChainConfig,
};

use crate::args::{ChainKind, CliArgs};

#[derive(Clone)]
pub struct CliContext {
    inner: Arc<CliContextInner>,
}

impl std::ops::Deref for CliContext {
    type Target = CliContextInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct CliContextInner {
    pub args: CliArgs,
    pub chain_config: CliChainConfig,
}

#[allow(dead_code)]
pub enum CliChainConfig {
    Cosmos(CosmosChainConfig),
    Eth(EthereumChainConfig),
}

impl CliContext {
    pub fn new() -> Result<Self> {
        let args = CliArgs::parse();

        #[derive(Debug, Deserialize)]
        struct PartialWavsConfig {
            pub chains: ChainConfigs,
            #[serde(flatten)]
            pub chain_config_override: OptionalWavsChainConfig,
        }

        let config =
            std::fs::read_to_string(&args.wavs_config).expect("Could not read config file");
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
                    .map(CliChainConfig::Cosmos),
                ChainKind::Eth => chains
                    .eth
                    .get(&args.chain)
                    .cloned()
                    .map(CliChainConfig::Eth),
            })
            .unwrap_or_else(|| match chains.cosmos.get(&args.chain).cloned() {
                Some(chain) => Some(CliChainConfig::Cosmos(chain)),
                None => chains
                    .eth
                    .get(&args.chain)
                    .cloned()
                    .map(CliChainConfig::Eth),
            })
            .context(format!("No chain config found for: {}", args.chain))?;

        Ok(Self {
            inner: Arc::new(CliContextInner { args, chain_config }),
        })
    }
}
