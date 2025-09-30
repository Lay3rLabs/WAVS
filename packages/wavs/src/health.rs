use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use utoipa::ToSchema;
use wavs_types::ChainKey;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthStatus {
    pub timestamp: u64,
    pub chains: HashMap<ChainKey, ChainHealthResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChainHealthResult {
    Healthy,
    Unhealthy { error: String },
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self {
            timestamp: chrono::Utc::now().timestamp() as u64,
            chains: HashMap::new(),
        }
    }
}

impl HealthStatus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_healthy(&self) -> bool {
        self.chains
            .values()
            .all(|result| matches!(result, ChainHealthResult::Healthy))
    }
}

#[derive(Clone)]
pub struct SharedHealthStatus(Arc<RwLock<HealthStatus>>);

impl Default for SharedHealthStatus {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(HealthStatus::new())))
    }
}

impl SharedHealthStatus {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn update(&self, chain_configs: &utils::config::ChainConfigs) {
        let chains = match chain_configs.all_chain_keys() {
            Ok(chains) => chains,
            Err(_) => return,
        };

        let mut chain_results = HashMap::new();

        // run all health checks without holding the lock
        for chain in chains {
            let config = chain_configs.get_chain(&chain).unwrap();
            let health_result =
                match utils::health::health_check_single_chain(&chain, &config).await {
                    Ok(()) => {
                        tracing::info!("Chain [{chain}] is healthy");
                        ChainHealthResult::Healthy
                    }
                    Err(err) => {
                        tracing::warn!("Chain [{chain}] is unhealthy: {err}");
                        ChainHealthResult::Unhealthy {
                            error: err.to_string(),
                        }
                    }
                };
            chain_results.insert(chain, health_result);
        }

        // update the status with all results at once
        if let Ok(mut status) = self.0.write() {
            status.timestamp = chrono::Utc::now().timestamp() as u64;
            status.chains = chain_results;
        }
    }

    pub fn any_failing(&self) -> bool {
        if let Ok(status) = self.0.read() {
            !status.is_healthy()
        } else {
            false
        }
    }

    pub fn read(&self) -> std::sync::LockResult<std::sync::RwLockReadGuard<'_, HealthStatus>> {
        self.0.read()
    }
}
