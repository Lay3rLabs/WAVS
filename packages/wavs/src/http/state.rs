use std::sync::Arc;

use anyhow::Context;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::multipart::{Form, Part};
use utils::storage::db::{DBError, RedbStorage, Table, JSON};
use wavs_types::ServiceID;

use crate::{apis::dispatcher::DispatchManager, config::Config, dispatcher::DispatcherError};

use super::ipfs::PinataResponse;

const SERVICES: Table<&str, JSON<wavs_types::Service>> = Table::new("services");

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub dispatcher: Arc<dyn DispatchManager<Error = DispatcherError>>,
    pub is_mock_chain_client: bool,
    pub http_client: reqwest::Client,
    pub storage: Arc<RedbStorage>,
}

impl HttpState {
    pub async fn new(
        config: Config,
        dispatcher: Arc<dyn DispatchManager<Error = DispatcherError>>,
        is_mock_chain_client: bool,
    ) -> anyhow::Result<Self> {
        if !config.data.exists() {
            std::fs::create_dir_all(&config.data).map_err(|err| {
                anyhow::anyhow!(
                    "Failed to create directory {} for http database: {}",
                    config.data.display(),
                    err
                )
            })?;
        }

        let storage = Arc::new(RedbStorage::new(config.data.join("http-db"))?);

        Ok(Self {
            config,
            dispatcher,
            is_mock_chain_client,
            http_client: reqwest::Client::new(),
            storage,
        })
    }

    pub fn load_service(&self, service_id: &ServiceID) -> anyhow::Result<wavs_types::Service> {
        match self.storage.get(SERVICES, service_id.as_ref()) {
            Ok(Some(service)) => Ok(service.value()),
            _ => Err(anyhow::anyhow!(
                "Service ID {service_id} has not been set on the http server",
            )),
        }
    }

    pub fn save_service(&self, service: &wavs_types::Service) -> Result<(), DBError> {
        self.storage.set(SERVICES, service.id.as_ref(), service)
    }

    pub async fn save_service_ipfs(&self, service: &wavs_types::Service) -> anyhow::Result<String> {
        // Serialize service to JSON string
        let service_json =
            serde_json::to_string(service).context("Failed to serialize service to JSON")?;

        // Convert JSON string to bytes for the file upload
        let service_bytes = service_json.into_bytes();

        // Set up Pinata V3 API endpoint and headers
        let pinata_url = "https://uploads.pinata.cloud/v3/files";
        let pinata_jwt = self
            .config
            .pinata_jwt
            .clone()
            .expect("Pinata JWT is not configured");

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", pinata_jwt))
                .context("Failed to create authorization header")?,
        );

        // Determine network type based on compile-time configuration
        // debug_assertions is automatically set in debug mode and disabled in release mode
        #[cfg(debug_assertions)]
        let network = "private";

        #[cfg(not(debug_assertions))]
        let network = "public";

        // Create a unique filename
        let filename = format!(
            "service-{}-{}.json",
            service.id,
            chrono::Utc::now().timestamp()
        );

        // Create multipart form
        let form = Form::new()
            .part("file", Part::bytes(service_bytes).file_name(filename))
            .text("network", network.to_string());

        // Make request to Pinata
        let client = reqwest::Client::new();
        let response = client
            .post(pinata_url)
            .headers(headers)
            .multipart(form)
            .send()
            .await
            .context("Failed to send request to Pinata V3 API")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            anyhow::bail!("Pinata V3 API returned error: {}", error_text);
        }

        // Parse response to get IPFS hash (CID)
        let pinata_response: PinataResponse = response
            .json()
            .await
            .context("Failed to parse Pinata V3 API response")?;

        tracing::debug!(
            "File uploaded to IPFS: id={}, name={}, size={}, created_at={}",
            pinata_response.data.id,
            pinata_response.data.name,
            pinata_response.data.size,
            pinata_response.data.created_at,
        );

        Ok(pinata_response.data.cid)
    }
}
