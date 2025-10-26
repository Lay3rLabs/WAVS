mod cosmos;
mod evm;

use std::{collections::HashMap, sync::Arc};

use cosmos::CosmosInstance;
use evm::EvmInstance;
use utils::{
    context::AppContext,
    telemetry::Metrics,
    test_utils::middleware::{
        cosmos::CosmosMiddleware,
        evm::{EvmMiddleware, EvmMiddlewareType},
    },
};
use wavs::dispatcher::Dispatcher;
use wavs_types::{ChainKey, ChainKeyNamespace};

use crate::e2e::clients::Clients;

use super::config::Configs;

pub struct AppHandles {
    pub wavs_handle: std::thread::JoinHandle<()>,
    pub aggregator_handles: Vec<std::thread::JoinHandle<()>>,
    pub evm_middleware: Option<EvmMiddleware>,
    pub cosmos_middlewares: CosmosMiddlewares,
    _evm_chains: Vec<EvmInstance>,
    _cosmos_chains: Vec<CosmosInstance>,
}

pub type CosmosMiddlewares = Arc<HashMap<ChainKey, CosmosMiddleware>>;

impl AppHandles {
    pub fn start(
        ctx: &AppContext,
        configs: &Configs,
        clients: &Clients,
        metrics: Metrics,
        evm_middleware_type: EvmMiddlewareType,
    ) -> Self {
        let mut evm_chains = Vec::new();
        let mut cosmos_chains = Vec::new();

        let mut cosmos_middlewares = HashMap::new();
        {
            let chains = configs.chains.read().unwrap();
            for chain_config in chains.evm_iter() {
                let handle = EvmInstance::spawn(ctx.clone(), configs, chain_config.clone());
                evm_chains.push(handle);
            }

            for chain_config in chains.cosmos_iter() {
                let handle = CosmosInstance::spawn(ctx.clone(), configs, chain_config.clone());

                let chain_key = ChainKey {
                    namespace: ChainKeyNamespace::COSMOS.parse().unwrap(),
                    id: chain_config.chain_id,
                };
                let pool = clients.cosmos_client_pools.get(&chain_key).unwrap_or_else(||
                    panic!("Cosmos client pool must exist for chain {}since the chain configs are derived from it", chain_key)
                );
                let middleware = CosmosMiddleware::new(pool.clone());

                cosmos_middlewares.insert(chain_key, middleware);
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

        let evm_middleware = if evm_chains.is_empty() {
            None
        } else {
            Some(EvmMiddleware::new(evm_middleware_type).unwrap())
        };

        Self {
            wavs_handle,
            aggregator_handles,
            evm_middleware,
            cosmos_middlewares: Arc::new(cosmos_middlewares),
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
