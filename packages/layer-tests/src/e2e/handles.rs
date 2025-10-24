mod cosmos;
mod evm;

use std::sync::Arc;

use cosmos::CosmosInstance;
use evm::EvmInstance;
use utils::{
    context::AppContext,
    telemetry::Metrics,
    test_utils::middleware::{EvmMiddlewareType, MiddlewareInstance},
};
use wavs::dispatcher::Dispatcher;

use super::config::Configs;

pub struct AppHandles {
    pub wavs_handle: std::thread::JoinHandle<()>,
    pub aggregator_handles: Vec<std::thread::JoinHandle<()>>,
    pub evm_middleware_instance: Option<MiddlewareInstance>,
    pub cosmos_middleware_instance: Option<MiddlewareInstance>,
    _evm_chains: Vec<EvmInstance>,
    _cosmos_chains: Vec<CosmosInstance>,
}

impl AppHandles {
    pub fn start(
        ctx: &AppContext,
        configs: &Configs,
        metrics: Metrics,
        middleware_type: EvmMiddlewareType,
    ) -> Self {
        let mut evm_chains = Vec::new();
        let mut cosmos_chains = Vec::new();

        {
            let chains = configs.chains.read().unwrap();
            for chain_config in chains.evm_iter() {
                let handle = EvmInstance::spawn(ctx.clone(), configs, chain_config.clone());
                evm_chains.push(handle);
            }

            for chain_config in chains.cosmos_iter() {
                let handle = CosmosInstance::spawn(ctx.clone(), configs, chain_config.clone());

                cosmos_chains.push(handle);
            }
        }

        let dispatcher = Arc::new(Dispatcher::new(&configs.wavs, metrics.wavs).unwrap());

        let wavs_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = configs.wavs.clone();

            move || {
                let health_status = wavs::health::SharedHealthStatus::new();
                wavs::run_server(ctx, config, dispatcher, metrics.http, health_status);
            }
        });

        let mut aggregator_handles = Vec::new();

        for config in &configs.aggregators {
            aggregator_handles.push(std::thread::spawn({
                let ctx = ctx.clone();
                let config = config.clone();
                move || {
                    let meter = opentelemetry::global::meter("aggregator_test");
                    let metrics = utils::telemetry::AggregatorMetrics::new(meter);
                    wavs_aggregator::run_server(ctx, config, metrics);
                }
            }));
        }

        let evm_middleware_instance =
            if evm_chains.is_empty() {
                None
            } else {
                Some(ctx.rt.block_on(async {
                    MiddlewareInstance::new_evm(middleware_type).await.unwrap()
                }))
            };

        let cosmos_middleware_instance = if cosmos_chains.is_empty() {
            None
        } else {
            Some(
                ctx.rt
                    .block_on(async { MiddlewareInstance::new_cosmos().await.unwrap() }),
            )
        };

        Self {
            wavs_handle,
            aggregator_handles,
            evm_middleware_instance,
            cosmos_middleware_instance,
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
