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
    Unknown,
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
