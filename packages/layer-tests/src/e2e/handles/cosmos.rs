use layer_climb::prelude::*;
use utils::{config::CosmosChainConfig, context::AppContext};

use crate::e2e::config::Configs;

pub struct CosmosInstance {
    _inner: layer_climb_cli::handle::CosmosInstance,
}

impl CosmosInstance {
    pub fn spawn(ctx: AppContext, configs: &Configs, chain_config: CosmosChainConfig) -> Self {
        let mnemonic = configs.cli.cosmos_mnemonic.as_ref().unwrap();

        let chain_config: layer_climb::prelude::ChainConfig =
            chain_config.clone().to_chain_config();
        let signer = layer_climb::prelude::KeySigner::new_mnemonic_str(mnemonic, None).unwrap();

        let addr = ctx.rt.block_on(async {
            chain_config
                .address_from_pub_key(&signer.public_key().await.unwrap())
                .unwrap()
        });

        let instance = layer_climb_cli::handle::CosmosInstance::new(chain_config, vec![addr]);

        tracing::info!(
            "Setting up Cosmos chain: {}",
            instance.chain_config.chain_id
        );
        instance.setup().unwrap();

        tracing::info!("Starting Cosmos chain: {}", instance.chain_config.chain_id);
        instance.run().unwrap();

        ctx.rt.block_on(async {
            tracing::info!(
                "Waiting for block on Cosmos chain: {}",
                instance.chain_config.chain_id
            );
            instance.wait_for_block().await.unwrap();
        });

        Self { _inner: instance }
    }
}

impl Drop for CosmosInstance {
    fn drop(&mut self) {
        tracing::info!(
            "Stopping Cosmos chain: {}",
            self._inner.chain_config.chain_id
        );
    }
}
