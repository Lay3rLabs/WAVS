use std::time::Duration;

use alloy_provider::DynProvider;
use anyhow::{Context, Result};
use wavs_types::{
    AddServiceRequest, ComponentDigest, DeleteServicesRequest, GetServiceKeyRequest,
    IWavsServiceManager::IWavsServiceManagerInstance, SaveServiceResponse, Service, ServiceManager,
    SigningKeyResponse, UploadComponentResponse,
};

use crate::command::deploy_service::SetServiceUrlArgs;

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
            .post(format!("{}/upload", self.endpoint))
            .body(wasm_bytes)
            .send()
            .await?
            .json()
            .await?;

        Ok(response.digest)
    }

    pub async fn create_service(
        &self,
        service_manager: ServiceManager,
        save_service_args: Option<SetServiceUrlArgs>,
    ) -> Result<Service> {
        if let Some(save_service) = save_service_args {
            self.set_service_url(
                save_service.provider,
                service_manager.evm_address_unchecked(),
                save_service.service_url,
            )
            .await?;
        }

        let body: String = serde_json::to_string(&AddServiceRequest {
            service_manager: service_manager.clone(),
        })?;

        let url = format!("{}/app", self.endpoint);
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

        let (chain_name, address) = match &service_manager {
            ServiceManager::Evm {
                chain_name,
                address,
            } => (chain_name.as_ref(), address.to_string()),
        };
        let service = self.get_service_from_node(chain_name, &address).await?;

        Ok(service)
    }

    pub async fn set_service_url(
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

    pub async fn save_service(&self, service: &Service) -> Result<String> {
        let body = serde_json::to_string(service)?;

        let url = format!("{}/save-service", self.endpoint);
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

        Ok(format!(
            "{}/service-by-hash/{}",
            self.endpoint, response.hash
        ))
    }

    pub async fn get_service_key(
        &self,
        service_manager: ServiceManager,
    ) -> Result<SigningKeyResponse> {
        let body = serde_json::to_string(&GetServiceKeyRequest { service_manager })?;

        let url = format!("{}/service-key", self.endpoint);
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

    pub async fn get_service_from_node(&self, chain_name: &str, address: &str) -> Result<Service> {
        let url = format!("{}/service", self.endpoint);

        let text = self.inner
            .get(&url)
            .query(&[("chain_name", chain_name), ("address", address)])
            .send()
            .await?
            .text()
            .await?;

        match serde_json::from_str(&text) {
            Ok(service) => Ok(service),
            Err(err) => Err(anyhow::anyhow!(
                "Failed to parse response as Service [{}]: {}",
                err,
                text
            )),
        }
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
                tracing::warn!("Waiting for service update: {}", service.id());

                let (chain_name, address) = match &service.manager {
                    ServiceManager::Evm {
                        chain_name,
                        address,
                    } => (chain_name.as_ref(), address.to_string()),
                };
                if let Ok(current_service) = self.get_service_from_node(chain_name, &address).await
                {
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

        let url = format!("{}/app", self.endpoint);
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
