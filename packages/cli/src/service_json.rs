use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use wavs_types::{
    Component, ComponentID, ServiceConfig, ServiceID, ServiceStatus, Submit, Trigger, WorkflowID,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ServiceJson {
    pub id: ServiceID,
    pub name: String,
    pub components: BTreeMap<ComponentID, Component>,
    pub workflows: BTreeMap<WorkflowID, WorkflowJson>,
    pub status: ServiceStatus,
    pub config: ServiceConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct WorkflowJson {
    pub trigger: TriggerJson,
    pub component: ComponentID,
    pub submit: SubmitJson,
    pub fuel_limit: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", untagged)]
pub enum TriggerJson {
    Trigger(Trigger),
    Json(Json),
}

impl Default for TriggerJson {
    fn default() -> Self {
        TriggerJson::Json(Json::Unset)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", untagged)]
pub enum SubmitJson {
    Submit(Submit),
    Json(Json),
}

impl Default for SubmitJson {
    fn default() -> Self {
        SubmitJson::Json(Json::Unset)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Json {
    Unset,
}
