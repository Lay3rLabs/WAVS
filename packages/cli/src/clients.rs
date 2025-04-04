use anyhow::{Context, Result};
use layer_climb::prelude::*;
use utils::{
    config::{CosmosChainConfig, EthereumChainConfig},
    eth_client::{EthClientBuilder, EthSigningClient},
};
use wavs_types::{
    AddServiceRequest, Digest, IWavsServiceManager::IWavsServiceManagerInstance, Service,
    UploadComponentResponse,
};

use crate::{config::Config, context::CliContext};

pub async fn get_eth_client(
    config: &Config,
    chain_config: EthereumChainConfig,
) -> Result<EthSigningClient> {
    let client_config = chain_config.to_client_config(None, config.eth_mnemonic.clone(), None);

    let eth_client = EthClientBuilder::new(client_config).build_signing().await?;

    Ok(eth_client)
}

pub async fn get_cosmos_client(
    config: &Config,
    chain_config: CosmosChainConfig,
) -> Result<SigningClient> {
    let key_signer = KeySigner::new_mnemonic_str(
        config
            .cosmos_mnemonic
            .as_ref()
            .context("missing mnemonic")?,
        None,
    )?;

    let climb_chain_config: ChainConfig = chain_config.into();
    SigningClient::new(climb_chain_config, key_signer, None).await
}

#[derive(Clone)]
pub struct HttpClient {
    inner: reqwest::Client,
    endpoint: String,
}

impl HttpClient {
    pub fn new(endpoint: String) -> Self {
        Self {
            inner: reqwest::Client::new(),
            endpoint,
        }
    }

    pub async fn get_config(&self) -> Result<serde_json::Value> {
        self.inner
            .get(format!("{}/config", self.endpoint))
            .send()
            .await?
            .json()
            .await
            .map_err(|e| e.into())
    }

    pub async fn upload_component(&self, wasm_bytes: Vec<u8>) -> Result<Digest> {
        let response: UploadComponentResponse = self
            .inner
            .post(format!("{}/upload", self.endpoint))
            .body(wasm_bytes)
            .send()
            .await?
            .json()
            .await?;

        Ok(response.digest.into())
    }

    pub async fn create_service_raw(
        &self,
        ctx: &CliContext,
        index: u32,
        service: Service,
    ) -> Result<()> {
        let body = serde_json::to_string(&service)?;

        self.inner
            .post(format!("{}/save-service", self.endpoint))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?
            .error_for_status()?;

        let service_uri = format!("{}/service/{}", self.endpoint, service.id);
        tracing::info!("Service URI: {}", service_uri);

        let client = ctx
            .new_eth_client(service.manager.chain_name(), index, true)
            .await?;
        let contract = IWavsServiceManagerInstance::new(
            service.manager.eth_address_unchecked(),
            client.provider,
        );
        contract
            .setServiceURI(service_uri)
            .send()
            .await?
            .watch()
            .await?;

        let body: String = serde_json::to_string(&AddServiceRequest {
            chain_name: service.manager.chain_name().clone(),
            address: Address::Eth(service.manager.eth_address_unchecked().into()),
        })?;

        self.inner
            .post(format!("{}/app", self.endpoint))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}
