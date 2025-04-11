use alloy::providers::Provider;
use anyhow::Result;
use layer_climb::prelude::*;
use wavs_types::{
    AddServiceRequest, Digest, IWavsServiceManager::IWavsServiceManagerInstance, Service,
    ServiceID, SigningKeyResponse, UploadComponentResponse,
};

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

    pub async fn create_service_raw<T: Provider>(
        &self,
        provider: T,
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

        let contract =
            IWavsServiceManagerInstance::new(service.manager.eth_address_unchecked(), provider);
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
