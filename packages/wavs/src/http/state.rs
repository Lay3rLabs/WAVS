use std::sync::Arc;

use utils::{
    storage::{
        db::{RedbStorage, Table, JSON},
        fs::FileStorage,
    },
    telemetry::HttpMetrics,
};
use wavs_types::ServiceID;

use crate::{config::Config, dispatcher::Dispatcher};

const SERVICES: Table<&str, JSON<wavs_types::Service>> = Table::new("services");

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub dispatcher: Arc<Dispatcher<FileStorage>>,
    pub is_mock_chain_client: bool,
    pub http_client: reqwest::Client,
    pub storage: Arc<RedbStorage>,
    pub metrics: HttpMetrics,
}

impl HttpState {
    pub async fn new(
        config: Config,
        dispatcher: Arc<Dispatcher<FileStorage>>,
        is_mock_chain_client: bool,
        metrics: HttpMetrics,
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
            metrics,
        })
    }

    pub fn load_service(&self, service_id: &ServiceID) -> anyhow::Result<wavs_types::Service> {
        match self.storage.get(SERVICES, service_id.as_ref()) {
            Ok(Some(service)) => Ok(service.value()),
            _ => Err(anyhow::anyhow!(
                "Service Hash {service_id} has not been set on the http server",
            )),
        }
    }

    pub fn save_service(
        &self,
        service: &wavs_types::Service,
    ) -> anyhow::Result<()> {
        self.storage.set(SERVICES, service.id.as_ref(), service)?;

        Ok(())
    }
}
