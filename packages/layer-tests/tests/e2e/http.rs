use anyhow::Result;
use serde::de::DeserializeOwned;
use utils::config::EthereumChainConfig;
use wavs::{
    apis::{
        dispatcher::{AllowedHostPermission, ComponentWorld, Permissions, ServiceConfig, Submit},
        trigger::{Trigger, TriggerData},
        ServiceID,
    },
    config::Config,
    http::handlers::service::{
        add::{AddServiceRequest, ServiceRequest},
        test::{TestAppRequest, TestAppResponse},
        upload::UploadServiceResponse,
    },
    Digest,
};

#[derive(Clone)]
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
        trigger: Trigger,
        submit: Submit,
        world: ComponentWorld,
    ) -> Result<()> {
        let service = ServiceRequest {
            trigger,
            id,
            world,
            digest: digest.into(),
            permissions: Permissions {
                allowed_http_hosts: AllowedHostPermission::All,
                file_system: true,
            },
            testable: Some(true),
            config: ServiceConfig::default(),
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
        chain_name: &str,
        service_manager_address: alloy::primitives::Address,
        chain_config: &EthereumChainConfig,
    ) -> Result<()> {
        let aggregator_app_url = chain_config.aggregator_endpoint.clone().unwrap();

        self.inner
            .post(format!("{}/add-service", aggregator_app_url))
            .header("Content-Type", "application/json")
            .json(
                &utils::aggregator::AddAggregatorServiceRequest::EthTrigger {
                    service_manager_address,
                },
            )
            .send()
            .await?;

        Ok(())
    }

    pub async fn test_service<D: DeserializeOwned>(
        &self,
        name: impl ToString,
        input: TriggerData,
    ) -> Result<D> {
        let body = serde_json::to_string(&TestAppRequest {
            name: name.to_string(),
            input,
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
