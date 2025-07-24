use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_local::{coins_bip39::English, LocalSigner, MnemonicBuilder};
use anyhow::Result;

use crate::{
    evm_client::EvmSigningClient,
    test_utils::middleware::{
        MiddlewareServiceManagerAddresses, MiddlewareServiceManagerConfig, MiddlewareSetServiceUri,
    },
};

#[derive(Debug)]
pub struct MockServiceManager {
    pub deployer: LocalSigner<SigningKey>,
    pub deployer_key_hex: String,
    pub address: alloy_primitives::Address,
    pub all_addresses: MiddlewareServiceManagerAddresses,
    pub rpc_url: String,
}

impl MockServiceManager {
    // because the client will be used with the docker image
    // and we can't control or even know how the nonce gets used
    // we need to generate a random key and fund it from the wallet
    // otherwise we may try to run transactions in parallel with the same nonce
    pub async fn new(wallet_client: EvmSigningClient) -> Result<Self> {
        let deployer = MnemonicBuilder::<English>::default()
            .word_count(24)
            .build_random()?;

        wallet_client
            .transfer_funds(deployer.address(), "1")
            .await?;

        let deployer_key_hex = const_hex::encode(deployer.credential().to_bytes().to_vec());
        let rpc_url = wallet_client.config.endpoint.to_string();

        let all_addresses =
            MiddlewareServiceManagerAddresses::deploy(&rpc_url, &deployer_key_hex).await?;

        Ok(Self {
            deployer,
            rpc_url,
            address: all_addresses.address,
            all_addresses,
            deployer_key_hex,
        })
    }

    pub async fn set_service_uri(&self, uri: String) -> anyhow::Result<()> {
        MiddlewareSetServiceUri {
            rpc_url: self.rpc_url.clone(),
            service_manager_address: self.address,
            deployer_key_hex: self.deployer_key_hex.clone(),
            service_uri: uri,
        }
        .apply()
        .await
    }

    pub async fn configure(&self, config: &MiddlewareServiceManagerConfig) -> anyhow::Result<()> {
        config
            .apply(&self.rpc_url, &self.deployer_key_hex, &self.address)
            .await
    }
}
