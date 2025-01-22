mod cosmos;
mod eth;

use std::sync::Arc;

use cosmos::CosmosInstance;
use eth::EthereumInstance;
use utils::context::AppContext;
use wavs::dispatcher::CoreDispatcher;

use super::config::Configs;

pub struct AppHandles {
    pub wavs_handle: std::thread::JoinHandle<()>,
    pub aggregator_handle: Option<std::thread::JoinHandle<()>>,
    _eth_chains: Vec<EthereumInstance>,
    _cosmos_chains: Vec<CosmosInstance>,
}

impl AppHandles {
    pub fn start(ctx: &AppContext, configs: &Configs) -> Self {
        let mut eth_chains = Vec::new();
        let mut cosmos_chains = Vec::new();

        for chain_config in configs.chains.eth.values() {
            let handle = EthereumInstance::spawn(configs, chain_config.clone());
            eth_chains.push(handle);
        }

        for chain_config in configs.chains.cosmos.values() {
            let handle = CosmosInstance::spawn(ctx.clone(), configs, chain_config.clone());

            cosmos_chains.push(handle);
        }

        let dispatcher = Arc::new(CoreDispatcher::new_core(&configs.wavs).unwrap());

        let wavs_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = configs.wavs.clone();
            move || {
                wavs::run_server(ctx, config, dispatcher);
            }
        });

        let aggregator_handle = configs.aggregator.clone().map(|config| {
            std::thread::spawn({
                let ctx = ctx.clone();
                move || {
                    aggregator::run_server(ctx, config);
                }
            })
        });

        Self {
            wavs_handle,
            aggregator_handle,
            _eth_chains: eth_chains,
            _cosmos_chains: cosmos_chains,
        }
    }

    pub fn join(self) {
        self.wavs_handle.join().unwrap();
        if let Some(handle) = self.aggregator_handle {
            handle.join().unwrap();
        }
    }
}
