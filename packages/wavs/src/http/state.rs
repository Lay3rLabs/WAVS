use std::convert::TryInto;
use std::sync::Arc;

use utils::{
    storage::{db::WavsDb, fs::FileStorage},
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
        if let Some(service) = self.storage.services_by_hash.get_cloned(&key) {
            Ok(service)
        } else {
            Err(anyhow::anyhow!(
                "Service Hash {} has not been set on the http server",
                service_hash
            ))
        }
    }
    pub fn save_service_by_hash(&self, service: &Service) -> anyhow::Result<ServiceDigest> {
        let service_hash = service.hash()?;
        let key: [u8; 32] = service_hash
            .as_ref()
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid service hash length"))?;
        self.storage.services_by_hash.insert(key, service.clone())?;
        Ok(service_hash)
    }
}
