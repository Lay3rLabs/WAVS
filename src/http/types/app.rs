use serde::{Deserialize, Serialize};

use crate::{
    apis::{dispatcher::Permissions, Trigger},
    Digest,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub name: String,
    // TODO - probably make a different struct for request vs. response
    // i.e. the request shouldn't contain this field at all
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    pub digest: Digest,
    pub trigger: Trigger,
    pub permissions: Permissions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub envs: Vec<(String, String)>,
    pub testable: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Copy)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Active,
    Failed,
    MissingWasm,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    //#[error("invalid CRON frequency")]
    //InvalidCronFrequency,
}
