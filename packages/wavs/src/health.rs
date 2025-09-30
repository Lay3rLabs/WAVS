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

pub type SharedHealthStatus = Arc<RwLock<HealthStatus>>;

pub fn create_shared_health_status() -> SharedHealthStatus {
    Arc::new(RwLock::new(HealthStatus::new()))
}

pub async fn update_health_status(
    health_status: &SharedHealthStatus,
    chain_configs: &utils::config::ChainConfigs,
) -> Result<(), anyhow::Error> {
    let chains = chain_configs.all_chain_keys()?;
    let mut any_unhealthy = false;
    let mut chain_results = HashMap::new();

    // run all health checks without holding the lock
    for chain in chains {
        let config = chain_configs.get_chain(&chain).unwrap();
        let health_result = match utils::health::health_check_single_chain(&chain, &config).await {
            Ok(()) => ChainHealthResult::Healthy,
            Err(err) => {
                any_unhealthy = true;
                ChainHealthResult::Unhealthy {
                    error: err.to_string(),
                }
            }
        };
        chain_results.insert(chain, health_result);
    }

    // update the status with all results at once
    if let Ok(mut status) = health_status.write() {
        status.timestamp = chrono::Utc::now().timestamp() as u64;
        status.chains = chain_results;
    }

    if any_unhealthy {
        Err(anyhow::anyhow!("One or more chains are unhealthy"))
    } else {
        Ok(())
    }
}
