use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use wavs_types::{Service, ServiceManager};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SavedRegistry {
    pub chain_id: u64,
    pub chain_key: String,
    pub rpc_url: String,
    pub address: String,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    pub wavs_home: Option<PathBuf>,
    #[serde(default)]
    pub saved_registries: Vec<SavedRegistry>,
    #[serde(default)]
    pub saved_service_managers: Vec<ServiceManager>,
    #[serde(default)]
    pub saved_services: Vec<Service>,
    #[serde(default)]
    pub mcp_enabled: bool,
    #[serde(default)]
    pub mcp_auto_start: bool,
    #[serde(default)]
    pub mcp_token: Option<String>,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
}
