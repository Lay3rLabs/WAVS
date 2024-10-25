use serde::{Deserialize, Serialize};

use crate::Digest;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    pub digest: Digest,
    pub trigger: Trigger,
    pub permissions: Permissions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub envs: Vec<(String, String)>,
    pub testable: Option<bool>,
}

impl App {
    pub fn _validate(&self) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    #[serde(rename_all = "camelCase")]
    Cron { schedule: String },

    #[serde(rename_all = "camelCase")]
    Event {},

    #[serde(rename_all = "camelCase")]
    Queue {
        task_queue_addr: String,
        hd_index: u32,
        poll_interval: u64,
    },
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Active,
    Failed,
    MissingWasm,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    // TODO
    //#[serde(default, skip_serializing_if = "Vec::is_empty")]
    //pub allowed_url_authorities: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    //#[error("invalid CRON frequency")]
    //InvalidCronFrequency,
}
