use std::convert::TryInto;
use std::sync::Arc;

use utils::{
    storage::{db::WavsDb, fs::FileStorage},
    telemetry::HttpMetrics,
};
use wavs_types::{Service, ServiceDigest, ServiceId};

use crate::{
    config::Config, dispatcher::Dispatcher, health::SharedHealthStatus, log_buffer::LogBuffer,
};

#[derive(Clone)]
pub struct HttpState {
    pub config: Config,
    pub dispatcher: Arc<Dispatcher<FileStorage>>,
    pub is_mock_chain_client: bool,
    pub http_client: reqwest::Client,
    pub db_storage: WavsDb,
    pub metrics: HttpMetrics,
    pub health_status: SharedHealthStatus,
    pub log_buffer: LogBuffer,
}

impl HttpState {
    pub async fn new(
        config: Config,
        dispatcher: Arc<Dispatcher<FileStorage>>,
        is_mock_chain_client: bool,
        metrics: HttpMetrics,
        health_status: SharedHealthStatus,
        log_buffer: LogBuffer,
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

        let db_storage = dispatcher.db_storage.clone();

        // Load previously saved service definitions from disk so GET /dev/services/{hash} works after restart
        let services_dir = config.data.join("dev_services");
        if services_dir.exists() {
            match std::fs::read_dir(&services_dir) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
                            continue;
                        }
                        let Ok(bytes) = std::fs::read(entry.path()) else {
                            continue;
                        };
                        let Ok(service) = serde_json::from_slice::<Service>(&bytes) else {
                            continue;
                        };
                        let Ok(hash) = service.hash() else { continue };
                        let key: [u8; 32] = match hash.as_ref().try_into() {
                            Ok(k) => k,
                            Err(_) => continue,
                        };
                        if let Err(e) = db_storage.services_by_hash.insert(key, service) {
                            tracing::warn!("Failed to load service from disk: {:?}", e);
                        }
                    }
                }
                Err(e) => tracing::warn!("Failed to read dev_services directory: {:?}", e),
            }
        }

        Ok(Self {
            config,
            db_storage,
            dispatcher,
            is_mock_chain_client,
            http_client: reqwest::Client::new(),
            metrics,
            health_status,
            log_buffer,
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
        if let Some(service) = self.db_storage.services_by_hash.get_cloned(&key) {
            Ok(service)
        } else {
            Err(anyhow::anyhow!(
                "Service Hash {} has not been set on the http server",
                service_hash
            ))
        }
    }
    pub async fn save_service_by_hash(&self, service: &Service) -> anyhow::Result<ServiceDigest> {
        let service_hash = service.hash()?;
        let key: [u8; 32] = service_hash
            .as_ref()
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid service hash length"))?;
        self.db_storage
            .services_by_hash
            .insert(key, service.clone())?;
        // Persist to disk so service definitions survive node restarts (atomic write)
        let services_dir = self.config.data.join("dev_services");
        tokio::fs::create_dir_all(&services_dir).await?;
        let dest = services_dir.join(format!("{service_hash}.json"));
        let json = serde_json::to_vec(service)?;
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let tmp = tempfile::NamedTempFile::new_in(&services_dir)?;
            std::fs::write(tmp.path(), &json)?;
            tmp.persist(dest)?;
            Ok(())
        })
        .await??;
        Ok(service_hash)
    }
}
