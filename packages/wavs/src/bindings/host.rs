use crate::engine::HostComponent;

impl super::world::host::Host for HostComponent {
    fn get_cosmos_chain_config(
        &mut self,
        chain_name: String,
    ) -> Option<super::world::host::CosmosChainConfig> {
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
        self.chain_configs
            .eth
            .get(&chain_name)
            .cloned()
            .map(|config| config.into())
    }
}
