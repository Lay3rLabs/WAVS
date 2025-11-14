use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(feature = "ts-bindings")]
use ts_rs::TS;

use crate::{Component, ServiceManager, ServiceStatus, SignatureKind, Submit, Trigger, WorkflowId};

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ServiceJson {
    pub name: String,
    #[cfg_attr(feature = "ts-bindings", ts(type = "Record<string, WorkflowJson>"))]
    pub workflows: BTreeMap<WorkflowId, WorkflowJson>,
    pub status: ServiceStatus,
    pub manager: ServiceManagerJson,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct WorkflowJson {
    pub trigger: TriggerJson,
    pub component: ComponentJson,
    pub submit: SubmitJson,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
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

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", untagged)]
pub enum SubmitJson {
    Submit(Submit),
    Json(Json),
    AggregatorJson(AggregatorJson),
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AggregatorJson {
    Aggregator {
        url: String,
        component: ComponentJson,
        signature_kind: SignatureKind,
    },
}

impl Default for SubmitJson {
    fn default() -> Self {
        SubmitJson::Json(Json::Unset)
    }
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", untagged)]
pub enum ServiceManagerJson {
    Manager(ServiceManager),
    Json(Json),
}

impl Default for ServiceManagerJson {
    fn default() -> Self {
        ServiceManagerJson::Json(Json::Unset)
    }
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Json {
    Unset,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", untagged)]
pub enum ComponentJson {
    Component(Component),
    Json(Json),
}

impl ComponentJson {
    pub fn new(component: Component) -> Self {
        ComponentJson::Component(component)
    }

    pub fn new_unset() -> Self {
        ComponentJson::Json(Json::Unset)
    }

    pub fn is_unset(&self) -> bool {
        matches!(self, ComponentJson::Json(Json::Unset))
    }

    pub fn is_set(&self) -> bool {
        matches!(self, ComponentJson::Component(_))
    }

    pub fn as_component(&self) -> Option<&Component> {
        match self {
            ComponentJson::Component(component) => Some(component),
            ComponentJson::Json(Json::Unset) => None,
        }
    }

    pub fn as_component_mut(&mut self) -> Option<&mut Component> {
        match self {
            ComponentJson::Component(component) => Some(component),
            ComponentJson::Json(Json::Unset) => None,
        }
    }
}

impl Default for ComponentJson {
    fn default() -> Self {
        ComponentJson::Json(Json::Unset)
    }
}
