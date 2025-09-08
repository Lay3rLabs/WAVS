mod cosmos;
mod evm;

use std::sync::Arc;

use cosmos::CosmosInstance;
use evm::EvmInstance;
use utils::{context::AppContext, telemetry::Metrics, test_utils::middleware::MiddlewareInstance};
use wavs::dispatcher::Dispatcher;

use super::config::Configs;

pub struct AppHandles {
    pub wavs_handle: std::thread::JoinHandle<()>,
    pub aggregator_handles: Vec<std::thread::JoinHandle<()>>,
    pub middleware_instance: MiddlewareInstance,
    _evm_chains: Vec<EvmInstance>,
    _cosmos_chains: Vec<CosmosInstance>,
}

impl AppHandles {
    pub fn start(ctx: &AppContext, configs: &Configs, metrics: Metrics) -> Self {
        let mut evm_chains = Vec::new();
        let mut cosmos_chains = Vec::new();

        for chain_config in configs.chains.evm_iter() {
            let handle = EvmInstance::spawn(ctx.clone(), configs, chain_config.clone());
            evm_chains.push(handle);
        }

        for chain_config in configs.chains.cosmos_iter() {
            let handle = CosmosInstance::spawn(ctx.clone(), configs, chain_config.clone());

            cosmos_chains.push(handle);
        }

        let dispatcher = Arc::new(Dispatcher::new(&configs.wavs, metrics.wavs).unwrap());

        let wavs_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = configs.wavs.clone();

            move || {
                wavs::run_server(ctx, config, dispatcher, metrics.http);
            }
        });

        let mut aggregator_handles = Vec::new();

        for config in &configs.aggregators {
            aggregator_handles.push(std::thread::spawn({
                let ctx = ctx.clone();
                let config = config.clone();
                move || {
                    wavs_aggregator::run_server(ctx, config);
                }
            }));
        }

        let middleware_instance = ctx
            .rt
            .block_on(async { MiddlewareInstance::new().await.unwrap() });

        Self {
            wavs_handle,
            aggregator_handles,
            middleware_instance,
            _evm_chains: evm_chains,
            _cosmos_chains: cosmos_chains,
        }
    }

    pub fn try_join(self) -> Vec<std::thread::Result<()>> {
        let mut results = Vec::new();
        results.push(self.wavs_handle.join());
        for handle in self.aggregator_handles {
            results.push(handle.join());
        }

        results
    }
}
