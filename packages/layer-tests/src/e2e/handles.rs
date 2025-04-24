mod cosmos;
mod evm;

use std::sync::Arc;

use cosmos::CosmosInstance;
use evm::EvmInstance;
use utils::context::AppContext;
use wavs::{dispatcher::CoreDispatcher, metrics::Metrics};

use super::config::Configs;

pub struct AppHandles {
    pub wavs_handle: std::thread::JoinHandle<()>,
    pub aggregator_handle: Option<std::thread::JoinHandle<()>>,
    _evm_chains: Vec<EvmInstance>,
    _cosmos_chains: Vec<CosmosInstance>,
}

impl AppHandles {
    pub fn start(ctx: &AppContext, configs: &Configs) -> Self {
        let mut evm_chains = Vec::new();
        let mut cosmos_chains = Vec::new();

        for chain_config in configs.chains.evm.values() {
            let handle = EvmInstance::spawn(ctx.clone(), configs, chain_config.clone());
            evm_chains.push(handle);
        }

        for chain_config in configs.chains.cosmos.values() {
            let handle = CosmosInstance::spawn(ctx.clone(), configs, chain_config.clone());

            cosmos_chains.push(handle);
        }

        let dispatcher = Arc::new(CoreDispatcher::new_core(&configs.wavs, wavs_metrics).unwrap());

        let wavs_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = configs.wavs.clone();

            move || {
                wavs::run_server(ctx, config, dispatcher, http_metrics);
            }
        });

        let aggregator_handle = configs.aggregator.clone().map(|config| {
            std::thread::spawn({
                let ctx = ctx.clone();
                move || {
                    wavs_aggregator::run_server(ctx, config);
                }
            })
        });

        Self {
            wavs_handle,
            aggregator_handle,
            _evm_chains: evm_chains,
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
