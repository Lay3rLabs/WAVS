use crate::digest::Digest;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    pub digest: Digest,
    pub trigger: Trigger,
    pub permissions: Permissions,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    #[serde(rename_all = "camelCase")]
    Cron { schedule: String },

    #[serde(rename_all = "camelCase")]
    Event {},

    #[serde(rename_all = "camelCase")]
    Queue {},
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_url_authorities: Vec<String>,
    // TODO more permissions
}
