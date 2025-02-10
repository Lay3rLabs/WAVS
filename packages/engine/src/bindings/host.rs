use wavs_types::ChainName;

use crate::HostComponent;

impl super::world::host::Host for HostComponent {
    fn get_cosmos_chain_config(
        &mut self,
        chain_name: String,
    ) -> Option<super::world::host::CosmosChainConfig> {
        let chain_name = ChainName::new(chain_name).ok()?;

        self.chain_configs
            .cosmos
            .get(&chain_name)
            .cloned()
            .map(|config| config.into())
    }

    fn get_eth_chain_config(
        &mut self,
        chain_name: String,
    ) -> Option<super::world::host::EthChainConfig> {
        let chain_name = ChainName::new(chain_name).ok()?;

        self.chain_configs
            .eth
            .get(&chain_name)
            .cloned()
            .map(|config| config.into())
    }
}
