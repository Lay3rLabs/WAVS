use std::convert::TryInto;
use std::sync::Arc;

use utils::{
    storage::{
        db::{handles, WavsDb},
        fs::FileStorage,
    },
    telemetry::HttpMetrics,
};
use wavs_types::{Service, ServiceDigest, ServiceId};

use crate::{config::Config, dispatcher::Dispatcher, health::SharedHealthStatus};

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub dispatcher: Arc<Dispatcher<FileStorage>>,
    pub is_mock_chain_client: bool,
    pub http_client: reqwest::Client,
    pub storage: WavsDb,
    pub metrics: HttpMetrics,
    pub health_status: SharedHealthStatus,
}

impl HttpState {
    pub async fn new(
        config: Config,
        dispatcher: Arc<Dispatcher<FileStorage>>,
        is_mock_chain_client: bool,
        metrics: HttpMetrics,
        health_status: SharedHealthStatus,
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

        let storage = WavsDb::new()?;

        Ok(Self {
            config,
            dispatcher,
            is_mock_chain_client,
            http_client: reqwest::Client::new(),
            storage,
            metrics,
            health_status,
        })
    }

    pub fn load_service(&self, service_id: &ServiceId) -> anyhow::Result<wavs_types::Service> {
        match self.dispatcher.services.get(service_id) {
            Ok(service) => Ok(service),
            _ => Err(anyhow::anyhow!(
                "Service ID {service_id} has not been set on the http server",
            )),
        }
    }

    pub fn load_service_by_hash(
        &self,
        service_hash: &ServiceDigest,
    ) -> anyhow::Result<wavs_types::Service> {
        let key: [u8; 32] = service_hash
            .as_ref()
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid service hash length"))?;
        match self.storage.get(&handles::SERVICES_BY_HASH, key) {
            Ok(Some(service)) => Ok(service),
            Ok(None) => Err(anyhow::anyhow!(
                "Service Hash {} has not been set on the http server",
                service_hash
            )),
            Err(e) => Err(anyhow::anyhow!("Failed to load service by hash: {}", e)),
        }
    }
    pub fn save_service_by_hash(&self, service: &Service) -> anyhow::Result<ServiceDigest> {
        let service_hash = service.hash()?;
        let key: [u8; 32] = service_hash
            .as_ref()
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid service hash length"))?;
        self.storage
            .set(&handles::SERVICES_BY_HASH, key, service.clone())?;
        Ok(service_hash)
    }
}
