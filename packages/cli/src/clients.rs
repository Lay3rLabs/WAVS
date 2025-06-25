use alloy_provider::DynProvider;
use anyhow::{Context, Result};
use layer_climb::prelude::*;
use wavs_types::{
    AddServiceRequest, Digest, IWavsServiceManager::IWavsServiceManagerInstance, SaveServiceResponse, Service, ServiceID, SigningKeyResponse, UploadComponentResponse
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

    pub async fn create_service(
        &self,
        service: Service,
        save_service_args: Option<SetServiceUrlArgs>,
    ) -> Result<()> {
        if let Some(save_service) = save_service_args {
            self.set_service_url(
                save_service.provider,
                service.manager.evm_address_unchecked(),
                save_service.service_url,
            )
            .await?;
        }

        let body: String = serde_json::to_string(&AddServiceRequest {
            chain_name: service.manager.chain_name().clone(),
            address: Address::Evm(service.manager.evm_address_unchecked().into()),
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

        Ok(())
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
        let response:SaveServiceResponse = self
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

        Ok(format!("{}/service/{}", self.endpoint, response.hash))
    }

    pub async fn get_service_key(&self, service_id: ServiceID) -> Result<SigningKeyResponse> {
        self.inner
            .get(format!("{}/service-key/{service_id}", self.endpoint))
            .send()
            .await?
            .json()
            .await
            .map_err(|e| e.into())
    }
}
