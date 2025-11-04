use anyhow::{Context, Result};
use wavs_types::{ChainConfigs, ChainKey, ChainKeyNamespace};

/// Structure to hold the different chain keys for test configuration
#[derive(Debug, Default, Clone)]
pub struct ChainKeys {
    pub evm: Vec<ChainKey>,
    pub cosmos: Vec<ChainKey>,
}

impl ChainKeys {
    /// Create a new ChainNames by categorizing chains from the config
    pub fn from_config(chain_configs: &ChainConfigs) -> Self {
        Self {
            evm: chain_configs.chain_keys(ChainKeyNamespace::EVM.parse().unwrap()),
            cosmos: chain_configs.chain_keys(ChainKeyNamespace::COSMOS.parse().unwrap()),
        }
    }

    // Get the primary EVM chain with error if not found
    pub fn primary_evm(&self) -> Result<&ChainKey> {
        self.evm
            .first()
            .context("Primary EVM chain required but not found")
    }

    // Get the secondary EVM chain with error if not found
    pub fn secondary_evm(&self) -> Result<&ChainKey> {
        self.evm
            .get(1)
            .context("Secondary EVM chain required but not found")
    }

    // Get the primary Cosmos chain with error if not found
    pub fn primary_cosmos(&self) -> Result<&ChainKey> {
        self.cosmos
            .first()
            .context("Cosmos chain required but not found")
    }
}
