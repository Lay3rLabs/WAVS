use anyhow::{Context, Result};
use utils::config::ChainConfigs;
use wavs_types::ChainName;

/// Structure to hold the different chain names for test configuration
#[derive(Debug, Default, Clone)]
pub struct ChainNames {
    pub evm: Vec<ChainName>,
    pub cosmos: Vec<ChainName>,
}

impl ChainNames {
    /// Create a new ChainNames by categorizing chains from the config
    pub fn from_config(chain_configs: &ChainConfigs) -> Self {
        let mut chain_names = Self::default();

        // Categorize EVM chains
        for chain_name in chain_configs.evm.keys() {
            chain_names.evm.push(chain_name.clone());
        }

        // Collect Cosmos chains
        chain_names.cosmos = chain_configs.cosmos.keys().cloned().collect::<Vec<_>>();

        chain_names
    }

    // Get the primary EVM chain with error if not found
    pub fn primary_evm(&self) -> Result<&ChainName> {
        self.evm
            .first()
            .context("Primary EVM chain required but not found")
    }

    // Get the secondary EVM chain with error if not found
    pub fn secondary_evm(&self) -> Result<&ChainName> {
        self.evm
            .get(1)
            .context("Secondary EVM chain required but not found")
    }

    // Get the primary Cosmos chain with error if not found
    pub fn primary_cosmos(&self) -> Result<&ChainName> {
        self.cosmos
            .first()
            .context("Cosmos chain required but not found")
    }
}
