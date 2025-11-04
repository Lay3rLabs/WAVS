use std::time::Duration;

use alloy_provider::DynProvider;
use anyhow::{Context, Result};
use layer_climb::{prelude::CosmosAddr, signing::SigningClient};
use wavs_types::{
    contracts::cosmwasm::service_manager::ServiceManagerExecuteMessages, AddServiceRequest,
    ChainKey, ComponentDigest, DeleteServicesRequest, DevTriggerStreamsInfo, GetSignerRequest,
    IWavsServiceManager::IWavsServiceManagerInstance, SaveServiceResponse, Service, ServiceManager,
    SignerResponse, UploadComponentResponse,
};

use crate::command::deploy_service::SetServiceUriArgs;

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

    pub async fn upload_component(&self, wasm_bytes: Vec<u8>) -> Result<ComponentDigest> {
        let response: UploadComponentResponse = self
            .inner
            .post(format!("{}/dev/components", self.endpoint))
            .body(wasm_bytes)
            .send()
            .await?
            .json()
            .await?;

        Ok(response.digest)
    }

    pub async fn register_aggregator_service(
        &self,
        service_manager: &ServiceManager,
    ) -> Result<()> {
        use wavs_types::aggregator::RegisterServiceRequest;

        self.inner
            .post(format!("{}/services", self.endpoint))
            .json(&RegisterServiceRequest {
                service_manager: service_manager.clone(),
            })
            .send()
            .await?;

        Ok(())
    }

    pub async fn create_service(
        &self,
        service_manager: ServiceManager,
        save_service_args: Option<SetServiceUriArgs>,
    ) -> Result<Service> {
        if let Some(save_service) = save_service_args {
            match save_service {
                SetServiceUriArgs::Evm {
                    provider,
                    service_uri,
                } => {
                    let address = service_manager.address().try_into()?;
                    self.evm_set_service_url(provider, address, service_uri.to_string())
                        .await?;
                }
                SetServiceUriArgs::Cosmos {
                    client,
                    service_uri,
                } => {
                    let address = service_manager.address().try_into()?;
                    self.cosmos_set_service_url(client, address, service_uri.to_string())
                        .await?;
                }
            }
        }

        let body: String = serde_json::to_string(&AddServiceRequest {
            service_manager: service_manager.clone(),
        })?;

        let url = format!("{}/services", self.endpoint);
        let response = self
            .inner
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", url))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<Failed to read response body>".to_string());

            anyhow::bail!("{} from {}: {}", status, url, error_text);
        }

        let (chain, address) = match &service_manager {
            ServiceManager::Evm { chain, address } => (chain, address.to_string()),
            ServiceManager::Cosmos { chain, address } => (chain, address.to_string()),
        };
        let service = self.get_service_from_node(chain, &address).await?;

        Ok(service)
    }

    pub async fn evm_set_service_url(
        &self,
        provider: DynProvider,
        service_manager_address: alloy_primitives::Address,
        service_url: String,
    ) -> Result<()> {
        let contract = IWavsServiceManagerInstance::new(service_manager_address, provider);
        contract
            .setServiceURI(service_url)
            .send()
            .await?
            .watch()
            .await?;

        Ok(())
    }

    pub async fn cosmos_set_service_url(
        &self,
        client: SigningClient,
        service_manager_address: CosmosAddr,
        service_uri: String,
    ) -> Result<()> {
        client
            .contract_execute(
                &service_manager_address.into(),
                &ServiceManagerExecuteMessages::WavsSetServiceUri { service_uri },
                vec![],
                None,
            )
            .await?;

        Ok(())
    }

    pub async fn save_service(&self, service: &Service) -> Result<String> {
        let body = serde_json::to_string(service)?;

        let url = format!("{}/dev/services", self.endpoint);
        let response: SaveServiceResponse = self
            .inner
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", url))?
            .json()
            .await
            .with_context(|| format!("Failed to parse response from {}", url))?;

        Ok(format!("{}/dev/services/{}", self.endpoint, response.hash))
    }

    pub async fn dev_add_service_direct(&self, service: &Service) -> Result<()> {
        let body = serde_json::to_string(service)?;

        let url = format!("{}/dev/services", self.endpoint);
        let response: SaveServiceResponse = self
            .inner
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", url))?
            .json()
            .await
            .with_context(|| format!("Failed to parse response from {}", url))?;

        let url = format!("{}/dev/services/{}", self.endpoint, response.hash);

        let res = self
            .inner
            .post(&url)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", url))?;

        if !res.status().is_success() {
            let status = res.status();
            let error_text = res
                .text()
                .await
                .unwrap_or_else(|_| "<Failed to read response body>".to_string());

            Err(anyhow::anyhow!("{} from {}: {}", status, url, error_text))
        } else {
            Ok(())
        }
    }

    pub async fn get_service_signer(
        &self,
        service_manager: ServiceManager,
    ) -> Result<SignerResponse> {
        let body = serde_json::to_string(&GetSignerRequest { service_manager })?;

        let url = format!("{}/services/signer", self.endpoint);
        let text = self
            .inner
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?
            .text()
            .await?;

        match serde_json::from_str(&text) {
            Ok(response) => Ok(response),
            Err(_) => {
                // If the response is not JSON, return it as an error
                Err(anyhow::anyhow!(
                    "Failed to parse response as SigningKeyResponse: {}",
                    text
                ))
            }
        }
    }

    pub async fn get_service_from_node(&self, chain: &ChainKey, address: &str) -> Result<Service> {
        let url = format!("{}/services/{chain}/{address}", self.endpoint);

        let text = self.inner.get(&url).send().await?.text().await?;

        match serde_json::from_str(&text) {
            Ok(service) => Ok(service),
            Err(err) => Err(anyhow::anyhow!(
                "Failed to parse response as Service [{}]: {}",
                err,
                text
            )),
        }
    }

    pub async fn get_trigger_streams_info(&self) -> Result<DevTriggerStreamsInfo> {
        let url = format!("{}/dev/trigger-streams", self.endpoint);

        let text = self.inner.get(&url).send().await?.text().await?;

        serde_json::from_str::<DevTriggerStreamsInfo>(&text).map_err(|err| {
            anyhow::anyhow!(
                "Failed to parse response as DevTriggerStreamsInfoResponse [{}]: {}",
                err,
                text
            )
        })
    }

    pub async fn wait_for_service_update(
        &self,
        service: &Service,
        timeout: Option<Duration>,
    ) -> Result<()> {
        // wait until WAVS sees the new service
        let service_hash = service.hash()?;
        tokio::time::timeout(timeout.unwrap_or(Duration::from_secs(30)), async {
            loop {
                tracing::warn!(service.name = %service.name, service.manager = ?service.manager, "Waiting for service update: {} [{:?}]", service.name, service.manager);

                let (chain, address) = match &service.manager {
                    ServiceManager::Evm { chain, address } => (chain, address.to_string()),
                    ServiceManager::Cosmos { chain, address} => (chain, address.to_string())
                };
                if let Ok(current_service) = self.get_service_from_node(chain, &address).await {
                    if current_service.hash()? == service_hash {
                        break Ok(());
                    }
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await?
    }

    pub async fn delete_service(&self, service_managers: Vec<ServiceManager>) -> Result<()> {
        let body: String = serde_json::to_string(&DeleteServicesRequest { service_managers })?;

        let url = format!("{}/services", self.endpoint);
        let response = self
            .inner
            .delete(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", url))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<Failed to read response body>".to_string());

            anyhow::bail!("{} from {}: {}", status, url, error_text);
        }

        Ok(())
    }
}
