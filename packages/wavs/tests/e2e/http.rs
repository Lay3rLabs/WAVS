use anyhow::Result;
use layer_climb::prelude::Address;
use serde::{de::DeserializeOwned, Serialize};
use wavs::{
    apis::{
        dispatcher::{AllowedHostPermission, Permissions, Submit},
        ServiceID,
    },
    config::Config,
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
        task_queue_addr: Address,
        task_queue_erc1271: Address,
        submit: Submit,
    ) -> Result<()> {
        let service = ServiceRequest {
            trigger: TriggerRequest::eth_queue(task_queue_addr, task_queue_erc1271),
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
