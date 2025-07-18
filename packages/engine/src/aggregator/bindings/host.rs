use wavs_types::ChainName;

use crate::aggregator::AggregatorHostComponent;

use super::world::host::Host;
use super::world::wavs::types::core::LogLevel;

impl Host for AggregatorHostComponent {
    fn get_cosmos_chain_config(
        &mut self,
        chain_name: String,
    ) -> Option<super::world::wavs::types::chain::CosmosChainConfig> {
        let chain_name = ChainName::new(chain_name).ok()?;

        self.chain_configs
            .cosmos
            .get(&chain_name)
            .cloned()
            .map(|config| config.into())
    }

    fn get_evm_chain_config(
        &mut self,
        chain_name: String,
    ) -> Option<super::world::wavs::types::chain::EvmChainConfig> {
        let chain_name = ChainName::new(chain_name).ok()?;

        self.chain_configs
            .evm
            .get(&chain_name)
            .cloned()
            .map(|config| config.into())
    }

    fn config_var(&mut self, key: String) -> Option<String> {
        // Aggregator components get config from the aggregator component, not the main workflow
        self.aggregator_component.config.get(&key).cloned()
    }

    fn log(&mut self, level: LogLevel, message: String) {
        let digest = self.aggregator_component.source.digest();

        (self.inner_log)(&self.service.id(), &self.workflow_id, digest, level.into(), message);
    }
}
