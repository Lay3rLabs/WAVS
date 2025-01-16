use std::sync::Arc;

use alloy::node_bindings::AnvilInstance;
use utils::{
    config::{ChainConfigs, ConfigBuilder, CosmosChainConfig, EthereumChainConfig},
    context::AppContext,
    filesystem::workspace_path,
};
use wavs::dispatcher::CoreDispatcher;

use super::{config::Configs, cosmos::IcTestHandle, matrix::TestMatrix};

pub struct AppHandles {
    pub eth_chains: Vec<AnvilInstance>,
    pub cosmos_chains: Vec<Option<IcTestHandle>>,
    pub wavs_handle: std::thread::JoinHandle<()>,
    pub aggregator_handle: std::thread::JoinHandle<()>,
}

impl AppHandles {
    pub fn start(
        ctx: &AppContext,
        configs: &Configs,
        eth_chains: Vec<AnvilInstance>,
        cosmos_chains: Vec<Option<IcTestHandle>>,
    ) -> Self {
        let dispatcher = Arc::new(CoreDispatcher::new_core(&configs.wavs).unwrap());

        let wavs_handle = std::thread::spawn({
            let dispatcher = dispatcher.clone();
            let ctx = ctx.clone();
            let config = configs.wavs.clone();
            move || {
                wavs::run_server(ctx, config, dispatcher);
            }
        });

        let aggregator_handle = std::thread::spawn({
            let config = configs.aggregator.clone();
            let ctx = ctx.clone();
            move || {
                aggregator::run_server(ctx, config);
            }
        });

        Self {
            wavs_handle,
            aggregator_handle,
            eth_chains,
            cosmos_chains,
        }
    }

    pub fn join(mut self) {
        self.wavs_handle.join().unwrap();
        self.aggregator_handle.join().unwrap();
    }
}
