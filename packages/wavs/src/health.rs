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
    let result = utils::health::health_check_chains_query(chain_configs).await;

    if let Ok(mut status) = health_status.write() {
        let health_result = match &result {
            Ok(()) => ChainHealthResult::Healthy,
            Err(err) => ChainHealthResult::Unhealthy {
                error: err.to_string(),
            },
        };
        for chain in chains {
            status.chains.insert(chain.clone(), health_result.clone());
        }
    }

    result
}
