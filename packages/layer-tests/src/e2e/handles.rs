mod cosmos;
mod evm;

use std::{collections::HashMap, sync::Arc};

use cosmos::CosmosInstance;
use evm::EvmInstance;
use utils::{
    context::AppContext,
    telemetry::Metrics,
    test_utils::middleware::{
        cosmos::{CosmosMiddleware, CosmosMiddlewareKind},
        evm::{EvmMiddleware, EvmMiddlewareType},
    },
};
use wavs::dispatcher::Dispatcher;
use wavs_types::{ChainKey, ChainKeyNamespace};

use super::config::Configs;

pub struct AppHandles {
    /// One handle per WAVS operator instance
    pub wavs_handles: Vec<std::thread::JoinHandle<()>>,
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

            for (index, chain_config) in chains.cosmos_iter().enumerate() {
                let handle =
                    CosmosInstance::spawn(ctx.clone(), configs, chain_config.clone(), index);

                let chain_key = ChainKey {
                    namespace: ChainKeyNamespace::COSMOS.parse().unwrap(),
                    id: chain_config.chain_id.clone(),
                };
                let middleware = ctx
                    .rt
                    .block_on(CosmosMiddleware::new(
                        chain_config.clone(),
                        CosmosMiddlewareKind::Mock,
                        configs.mnemonics.cosmos_middleware[index].to_string(),
                    ))
                    .unwrap();

                cosmos_middlewares.insert(chain_key, middleware);
                cosmos_chains.push(handle);
            }
        }

        // Spawn one WAVS instance per operator
        let mut wavs_handles = Vec::with_capacity(configs.num_operators());

        for (operator_index, wavs_config) in configs.wavs_configs.iter().enumerate() {
            // Each operator gets its own dispatcher and metrics
            // Note: For now, we share the same metrics instance - in the future we may want
            // to have separate metrics per operator
            let dispatcher = Arc::new(Dispatcher::new(wavs_config, metrics.wavs.clone()).unwrap());

            let wavs_handle = std::thread::spawn({
                let dispatcher = dispatcher.clone();
                let ctx = ctx.clone();
                let config = wavs_config.clone();
                let http_metrics = metrics.http.clone();

                move || {
                    tracing::info!(
                        "Starting WAVS operator {} on port {}",
                        operator_index,
                        config.port
                    );
                    let health_status = wavs::health::SharedHealthStatus::new();
                    wavs::run_server(ctx, config, dispatcher, http_metrics, health_status);
                }
            });

            wavs_handles.push(wavs_handle);
        }

        let evm_middleware = if evm_chains.is_empty() {
            None
        } else {
            Some(EvmMiddleware::new(evm_middleware_type).unwrap())
        };

        Self {
            wavs_handles,
            evm_middleware,
            cosmos_middlewares: Arc::new(cosmos_middlewares),
            _evm_chains: evm_chains,
            _cosmos_chains: cosmos_chains,
        }
    }

    pub fn try_join(self) -> Vec<std::thread::Result<()>> {
        let mut results = Vec::new();
        for handle in self.wavs_handles {
            results.push(handle.join());
        }
        results
    }
}
