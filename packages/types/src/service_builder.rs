use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(feature = "ts-bindings")]
use ts_rs::TS;

use crate::{Component, ServiceManager, ServiceStatus, SignatureKind, Submit, Trigger, WorkflowId};

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ServiceBuilder {
    pub name: String,
    #[cfg_attr(feature = "ts-bindings", ts(type = "Record<string, WorkflowBuilder>"))]
    pub workflows: BTreeMap<WorkflowId, WorkflowBuilder>,
    pub status: ServiceStatus,
    pub manager: ServiceManagerBuilder,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct WorkflowBuilder {
    pub trigger: TriggerBuilder,
    pub component: ComponentBuilder,
    pub submit: SubmitBuilder,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", untagged)]
pub enum TriggerBuilder {
    Trigger(Trigger),
    Builder(Builder),
}

impl Default for TriggerBuilder {
    fn default() -> Self {
        TriggerBuilder::Builder(Builder::Unset)
    }
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", untagged)]
#[allow(clippy::large_enum_variant)]
pub enum SubmitBuilder {
    Submit(Submit),
    Builder(Builder),
    AggregatorBuilder(AggregatorBuilder),
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AggregatorBuilder {
    Aggregator {
        url: String,
        component: ComponentBuilder,
        signature_kind: SignatureKind,
    },
}

impl Default for SubmitBuilder {
    fn default() -> Self {
        SubmitBuilder::Builder(Builder::Unset)
    }
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", untagged)]
pub enum ServiceManagerBuilder {
    Manager(ServiceManager),
    Builder(Builder),
}

impl Default for ServiceManagerBuilder {
    fn default() -> Self {
        ServiceManagerBuilder::Builder(Builder::Unset)
    }
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Builder {
    Unset,
}

#[cfg_attr(feature = "ts-bindings", derive(TS))]
#[cfg_attr(feature = "ts-bindings", ts(export))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", untagged)]
#[allow(clippy::large_enum_variant)]
pub enum ComponentBuilder {
    Component(Component),
    Builder(Builder),
}

impl ComponentBuilder {
    pub fn new(component: Component) -> Self {
        ComponentBuilder::Component(component)
    }

    pub fn new_unset() -> Self {
        ComponentBuilder::Builder(Builder::Unset)
    }

    pub fn is_unset(&self) -> bool {
        matches!(self, ComponentBuilder::Builder(Builder::Unset))
    }

    pub fn is_set(&self) -> bool {
        matches!(self, ComponentBuilder::Component(_))
    }

    pub fn as_component(&self) -> Option<&Component> {
        match self {
            ComponentBuilder::Component(component) => Some(component),
            ComponentBuilder::Builder(Builder::Unset) => None,
        }
    }

    pub fn as_component_mut(&mut self) -> Option<&mut Component> {
        match self {
            ComponentBuilder::Component(component) => Some(component),
            ComponentBuilder::Builder(Builder::Unset) => None,
        }
    }
}

impl Default for ComponentBuilder {
    fn default() -> Self {
        ComponentBuilder::Builder(Builder::Unset)
    }
}
