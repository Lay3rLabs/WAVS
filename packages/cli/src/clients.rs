use alloy_provider::DynProvider;
use anyhow::{Context, Result};
use layer_climb::prelude::*;
use reqwest::{Response, StatusCode};
use wavs_types::{
    AddServiceRequest, Digest, IWavsServiceManager::IWavsServiceManagerInstance, Service,
    ServiceID, SigningKeyResponse, UploadComponentResponse,
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

    /// Helper method to extract error messages from HTTP responses
    async fn handle_error_response(response: Response, context: &str) -> Result<Response> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let error_body = match response.text().await {
            Ok(body) => {
                // Try to parse as JSON for structured error messages
                match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(json) => {
                        if let Some(error_msg) = json.get("error").and_then(|e| e.as_str()) {
                            format!("{}: {}", status, error_msg)
                        } else if let Some(message) = json.get("message").and_then(|m| m.as_str()) {
                            format!("{}: {}", status, message)
                        } else {
                            format!("{}: {}", status, body)
                        }
                    }
                    Err(_) => format!("{}: {}", status, body),
                }
            }
            Err(_) => format!("{}", status),
        };

        anyhow::bail!("{}: {}", context, error_body)
    }

    /// Helper method to parse JSON responses with appropriate error context
    async fn parse_json<T: serde::de::DeserializeOwned>(
        response: Response,
        context: &str,
    ) -> Result<T> {
        response
            .json()
            .await
            .with_context(|| format!("Failed to parse {} response", context))
    }

    pub async fn get_config(&self) -> Result<serde_json::Value> {
        let response = self
            .inner
            .get(format!("{}/config", self.endpoint))
            .send()
            .await?;

        let response = Self::handle_error_response(response, "Failed to get config").await?;
        Self::parse_json(response, "config").await
    }

    pub async fn upload_component(&self, wasm_bytes: Vec<u8>) -> Result<Digest> {
        let response = self
            .inner
            .post(format!("{}/upload", self.endpoint))
            .body(wasm_bytes)
            .send()
            .await?;

        let response = Self::handle_error_response(response, "Failed to upload component").await?;
        let upload_response: UploadComponentResponse =
            Self::parse_json(response, "upload component").await?;

        Ok(upload_response.digest.into())
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

        let response = self
            .inner
            .post(format!("{}/app", self.endpoint))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        Self::handle_error_response(response, "Server error").await?;
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

        let response = self
            .inner
            .post(format!("{}/save-service", self.endpoint))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?;

        Self::handle_error_response(response, "Failed to save service").await?;
        Ok(format!("{}/service/{}", self.endpoint, service.id))
    }

    pub async fn get_service_key(&self, service_id: ServiceID) -> Result<SigningKeyResponse> {
        let response = self
            .inner
            .get(format!("{}/service-key/{service_id}", self.endpoint))
            .send()
            .await?;

        let response = Self::handle_error_response(response, "Failed to get service key").await?;
        Self::parse_json(response, "service key").await
    }
}
