use alloy_primitives::Address;
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::{coins_bip39::English, LocalSigner, MnemonicBuilder};
use anyhow::Result;

use crate::{
    evm_client::EvmSigningClient,
    test_utils::middleware::{
        MiddlewareInstance, MiddlewareServiceManager, MiddlewareServiceManagerConfig,
    },
};

pub struct MockServiceManager {
    #[allow(dead_code)]
    deployer: LocalSigner<SigningKey>,
    middleware_instance: MiddlewareInstance,
    service_manager: MiddlewareServiceManager,
}

impl MockServiceManager {
    // because the client will be used with the docker image
    // and we can't control or even know how the nonce gets used
    // we need to generate a random key and fund it from the wallet
    // otherwise we may try to run transactions in parallel with the same nonce
    pub async fn new(
        middleware_instance: MiddlewareInstance,
        wallet_client: EvmSigningClient,
    ) -> Result<Self> {
        let deployer = MnemonicBuilder::<English>::default()
            .word_count(24)
            .build_random()?;

        tracing::debug!("Starting transfer_funds for deployer: {:?}", deployer.address());
        wallet_client
            .transfer_funds(deployer.address(), "1")
            .await?;
        tracing::debug!("Completed transfer_funds for deployer: {:?}", deployer.address());

        let deployer_key_hex = const_hex::encode(deployer.credential().to_bytes());
        let rpc_url = wallet_client.config.endpoint.to_string();

        tracing::debug!("Starting deploy_service_manager for deployer: {:?}", deployer.address());
        let service_manager = middleware_instance
            .deploy_service_manager(rpc_url, deployer_key_hex)
            .await?;
        tracing::debug!("Completed deploy_service_manager for deployer: {:?}, address: {}", deployer.address(), service_manager.address);

        Ok(Self {
            deployer,
            service_manager,
            middleware_instance,
        })
    }

    pub fn address(&self) -> Address {
        self.service_manager.address
    }

    pub async fn set_service_uri(&self, uri: String) -> anyhow::Result<()> {
        self.middleware_instance
            .set_service_manager_uri(&self.service_manager, &uri)
            .await?;

        Ok(())
    }

    pub async fn configure(&self, config: &MiddlewareServiceManagerConfig) -> anyhow::Result<()> {
        self.middleware_instance
            .configure_service_manager(&self.service_manager, config)
            .await
    }
}
