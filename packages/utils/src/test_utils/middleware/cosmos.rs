use anyhow::Result;
use layer_climb::pool::SigningClientPool;

#[derive(Clone)]
pub struct CosmosMiddleware {
    pool: SigningClientPool,
}

impl CosmosMiddleware {
    pub fn new(pool: SigningClientPool) -> Self {
        Self { pool }
    }

    pub async fn deploy_service_manager(&self) -> Result<()> {
        todo!()
    }

    pub async fn set_service_manager_uri(&self) -> Result<()> {
        todo!()
    }

    pub async fn configure_service_manager(&self) -> Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use bip39::Mnemonic;
    use deadpool::managed::Pool;
    use layer_climb::{pool::SigningClientPoolManager, prelude::TxSigner};
    use layer_climb_cli::handle::CosmosInstance;
    use rand::prelude::*;
    use wavs_types::{CosmosChainConfig, CosmosChainConfigBuilder};

    use crate::init_tracing_tests;

    use super::*;

    #[tokio::test]
    async fn cosmos_middleware_works() {
        init_tracing_tests();

        let mut rng = rand::rng();

        let entropy: [u8; 32] = rng.random();
        let mnemonic = Mnemonic::from_entropy(&entropy).unwrap().to_string();

        let (_instance, chain_config) = start_chain(&mnemonic).await;

        let pool_manager =
            SigningClientPoolManager::new_mnemonic(mnemonic, chain_config.into(), None, None);

        let pool = SigningClientPool::new(Pool::builder(pool_manager).max_size(8).build().unwrap());

        let middleware = CosmosMiddleware::new(pool);

        middleware.deploy_service_manager().await.unwrap();
    }

    async fn start_chain(mnemonic: &str) -> (CosmosInstance, CosmosChainConfig) {
        let cosmos_port = 9321;
        let rpc_endpoint = format!("http://127.0.0.1:{}", cosmos_port);

        let chain_config = CosmosChainConfigBuilder {
            rpc_endpoint: Some(rpc_endpoint),
            grpc_endpoint: None,
            gas_price: 0.025,
            gas_denom: "ucosm".to_string(),
            bech32_prefix: "wasm".to_string(),
            faucet_endpoint: None,
        }
        .build("wasmd".parse().unwrap());

        let climb_chain_config: layer_climb::prelude::ChainConfig =
            chain_config.clone().to_chain_config();

        let signer = layer_climb::prelude::KeySigner::new_mnemonic_str(mnemonic, None).unwrap();

        let addr = climb_chain_config
            .address_from_pub_key(&signer.public_key().await.unwrap())
            .unwrap();

        let instance = layer_climb_cli::handle::CosmosInstance::new(climb_chain_config, vec![addr]);

        tracing::info!(
            "Setting up Cosmos chain: {}",
            instance.chain_config.chain_id
        );
        instance.setup().unwrap();

        tracing::info!("Starting Cosmos chain: {}", instance.chain_config.chain_id);
        instance.run().unwrap();

        tracing::info!(
            "Waiting for block on Cosmos chain: {}",
            instance.chain_config.chain_id
        );
        instance.wait_for_block().await.unwrap();

        (instance, chain_config)
    }
}
