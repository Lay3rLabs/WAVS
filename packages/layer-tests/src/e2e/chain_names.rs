use utils::config::ChainConfigs;
use wavs_types::ChainName;

/// Structure to hold the different chain names for test configuration
#[derive(Debug, Default, Clone)]
pub struct ChainNames {
    pub evm: Vec<ChainName>,
    pub evm_aggregator: Vec<(ChainName, String)>,
    pub cosmos: Vec<ChainName>,
}

impl ChainNames {
    /// Create a new ChainNames by categorizing chains from the config
    pub fn from_config(chain_configs: &ChainConfigs) -> Self {
        let mut chain_names = Self::default();

        // Categorize EVM chains
        for (chain_name, chain) in chain_configs.evm.iter() {
            if chain.aggregator_endpoint.is_some() {
                chain_names.evm_aggregator.push((
                    chain_name.clone(),
                    chain
                        .aggregator_endpoint
                        .clone()
                        .expect("Aggregator URL is expected"),
                ));
            } else {
                chain_names.evm.push(chain_name.clone());
            }
        }

        // Collect Cosmos chains
        chain_names.cosmos = chain_configs.cosmos.keys().cloned().collect::<Vec<_>>();

        chain_names
    }
}
