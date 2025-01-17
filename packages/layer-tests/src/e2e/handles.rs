use std::sync::Arc;

use alloy::node_bindings::AnvilInstance;
use utils::context::AppContext;
use wavs::dispatcher::CoreDispatcher;

use super::{config::Configs, cosmos::IcTestHandle};

pub struct AppHandles {
    pub _eth_chains: Vec<AnvilInstance>,
    pub _cosmos_chains: Vec<Option<IcTestHandle>>,
    pub wavs_handle: std::thread::JoinHandle<()>,
    pub aggregator_handle: Option<std::thread::JoinHandle<()>>,
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
