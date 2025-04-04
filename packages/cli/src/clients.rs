use anyhow::{Context, Result};
use layer_climb::prelude::*;
use utils::{
    config::{CosmosChainConfig, EthereumChainConfig},
    eth_client::{EthClientBuilder, EthSigningClient},
};
use wavs_types::{
    AddServiceRequest, AllowedHostPermission, ComponentSource, Digest, Permissions, Service,
    ServiceConfig, ServiceID, ServiceManager, Submit, Trigger, UploadComponentResponse,
};

use crate::config::Config;

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

    pub async fn create_service_simple(
        &self,
        trigger: Trigger,
        submit: Submit,
        source: ComponentSource,
        config: ServiceConfig,
        manager: ServiceManager,
    ) -> Result<Service> {
        let mut service = Service::new_simple(
            ServiceID::new(uuid::Uuid::now_v7().as_simple().to_string())?,
            None,
            trigger,
            source,
            submit,
            Some(config),
            manager,
        );

        for component in service.components.values_mut() {
            component.permissions = Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
            }
        }

        self.create_service_raw(service.clone()).await?;

        Ok(service)
    }

    pub async fn create_service_raw(&self, service: Service) -> Result<()> {
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

        // TODO - deprecate this old add-service endpoint, instead
        // broadcast the service uri to the nodes by way of the service manager metadata
        // but for now, we'll support both until all the dust settles

        let body = serde_json::to_string(&AddServiceRequest { service })?;

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
