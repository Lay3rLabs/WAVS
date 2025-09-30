use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use utoipa::ToSchema;
use wavs_types::ChainKey;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthStatus {
    pub timestamp: DateTime<Utc>,
    pub chains: HashMap<ChainKey, ChainHealthResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChainHealthResult {
    Healthy,
    Unhealthy { error: String },
    Unknown,
}

impl HealthStatus {
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            chains: HashMap::new(),
        }
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