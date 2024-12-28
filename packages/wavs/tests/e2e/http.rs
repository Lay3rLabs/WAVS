use std::collections::HashMap;

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use wavs::{
    apis::{
        dispatcher::{AllowedHostPermission, Permissions, Submit},
        ServiceID,
    },
    config::{Config, EthereumChainConfig},
    http::{
        handlers::service::{
            add::{AddServiceRequest, ServiceRequest},
            test::{TestAppRequest, TestAppResponse},
            upload::UploadServiceResponse,
        },
        types::TriggerRequest,
    },
    Digest,
};

pub struct HttpClient {
    inner: reqwest::Client,
    endpoint: String,
}

impl HttpClient {
    pub fn new(config: &Config) -> Self {
        let endpoint = format!("http://{}:{}", config.host, config.port);

        Self {
            inner: reqwest::Client::new(),
            endpoint,
        }
    }

    pub async fn get_config(&self) -> Result<Config> {
        self.inner
            .get(format!("{}/config", self.endpoint))
            .send()
            .await?
            .json()
            .await
            .map_err(|e| e.into())
    }

    pub async fn create_service(
        &self,
        id: ServiceID,
        digest: Digest,
        trigger: TriggerRequest,
        submit: Submit,
    ) -> Result<()> {
        let service = ServiceRequest {
            trigger,
            id,
            digest: digest.into(),
            permissions: Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
            },
            envs: Vec::new(),
            testable: Some(true),
            submit,
        };

        let body = serde_json::to_string(&AddServiceRequest {
            service,
            wasm_url: None,
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

    pub async fn register_service_on_aggregator(
        &self,
        service_manager_address: alloy::primitives::Address,
        service_id: ServiceID,
        chain_id: String,
        config: &Config,
    ) -> Result<()> {
        let chains: HashMap<String, EthereumChainConfig> = config.ethereum_chain_configs().unwrap();
        let chain_config = match chains.get(&chain_id) {
            Some(chain_config) => chain_config,
            None => panic!("Chain config not found for chain_id: {}", chain_id),
        };
        let aggregator_app_url = chain_config.aggregator_endpoint.clone().unwrap_or("http://127.0.0.1:8001".to_string());

        self.inner
            .post(format!("{}/add-service", aggregator_app_url))
            .header("Content-Type", "application/json")
            .json(
                &utils::aggregator::AddAggregatorServiceRequest::EthTrigger {
                    service_manager_address,
                    service_id: service_id.to_string(),
                },
            )
            .send()
            .await?;

        Ok(())
    }

    pub async fn test_service<D: DeserializeOwned>(
        &self,
        name: impl ToString,
        input: impl Serialize,
    ) -> Result<D> {
        let body = serde_json::to_string(&TestAppRequest {
            name: name.to_string(),
            input: Some(serde_json::to_value(input)?),
        })?;

        let response: TestAppResponse = self
            .inner
            .post(format!("{}/test", self.endpoint))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await?
            .json()
            .await?;

        Ok(serde_json::from_value(response.output)?)
    }

    pub async fn upload_wasm(&self, wasm_bytes: Vec<u8>) -> Result<Digest> {
        let response: UploadServiceResponse = self
            .inner
            .post(format!("{}/upload", self.endpoint))
            .body(wasm_bytes)
            .send()
            .await?
            .json()
            .await?;

        Ok(response.digest.into())
    }
}
