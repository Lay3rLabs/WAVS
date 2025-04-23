use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use wavs_types::{
    Aggregator, Component, EthereumContractSubmission, ServiceID, ServiceManager, ServiceStatus,
    Submit, Timestamp, Trigger, WorkflowID,
};

pub const ENV_PREFIX: &str = "WAVS_ENV_";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ServiceJson {
    pub id: ServiceID,
    pub name: String,
    pub workflows: BTreeMap<WorkflowID, WorkflowJson>,
    pub status: ServiceStatus,
    pub manager: ServiceManagerJson,
}

impl ServiceJson {
    /// Validates the service configuration
    /// Returns a Vec<String> containing any validation errors found
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Basic service validation
        if self.name.is_empty() {
            errors.push("Service name cannot be empty".to_string());
        }

        for (workflow_id, workflow) in &self.workflows {
            // Check if component is unset
            if workflow.component.is_unset() {
                errors.push(format!("Workflow '{}' has an unset component", workflow_id));
            } else {
                let component = workflow
                    .component
                    .as_component()
                    .expect("Component is unset and not validated beforehand");

                // Validate fuel limit
                if let Some(limit) = component.fuel_limit {
                    if limit == 0 {
                        errors.push(format!(
                            "Workflow '{}' has a fuel limit of zero, which will prevent execution",
                            workflow_id
                        ));
                    }
                }

                // Validate env_keys have the correct prefix
                for key in &component.env_keys {
                    if !key.starts_with(ENV_PREFIX) {
                        errors.push(format!(
                "Workflow '{}' has environment variable '{}' that doesn't start with '{}'",
                workflow_id, key, ENV_PREFIX
            ));
                    }
                }
            }

            // Check if trigger is unset
            match &workflow.trigger {
                TriggerJson::Json(_) => {
                    errors.push(format!("Workflow '{}' has an unset trigger", workflow_id));
                }
                TriggerJson::Trigger(trigger) => {
                    // Basic trigger validation
                    match trigger {
                        Trigger::CosmosContractEvent { event_type, .. } => {
                            // Validate event type
                            if event_type.is_empty() {
                                errors.push(format!(
                                    "Workflow '{}' has an empty event type in Cosmos trigger",
                                    workflow_id
                                ));
                            }
                        }
                        Trigger::EthContractEvent {
                            address,
                            chain_name: _,
                            event_hash,
                        } => {
                            // Validate Ethereum address format
                            if let Err(err) = alloy_primitives::Address::parse_checksummed(
                                address.to_string(),
                                None,
                            ) {
                                errors.push(format!(
                                    "Workflow '{}' has an invalid Ethereum address format: {}",
                                    workflow_id, err
                                ));
                            }

                            // Validate event hash (should be 32 bytes)
                            if event_hash.as_slice().len() != 32 {
                                errors.push(format!(
                                                        "Workflow '{}' has an invalid event hash length: expected 32 bytes but got {} bytes",
                                                        workflow_id, event_hash.as_slice().len()
                                                    ));
                            }
                        }
                        Trigger::Cron {
                            schedule: _,
                            start_time,
                            end_time,
                        } => {
                            if let Err(err) = validate_cron_config(*start_time, *end_time) {
                                errors.push(format!(
                                    "Workflow '{}' has an invalid cron trigger: {}",
                                    workflow_id, err
                                ));
                            }
                        }
                        Trigger::BlockInterval {
                            chain_name: _,
                            n_blocks: _,
                        }
                        | Trigger::Manual => {
                            // Manual and block interval triggers are valid
                        }
                    }
                }
            }

            // Check if submit is unset
            match &workflow.submit {
                SubmitJson::Json(_) => {
                    errors.push(format!("Workflow '{}' has an unset submit", workflow_id));
                }
                SubmitJson::Submit(submit) => {
                    // Basic submit validation
                    match submit {
                        Submit::EthereumContract(EthereumContractSubmission {
                            address,
                            max_gas,
                            chain_name: _,
                        }) => {
                            // Validate Ethereum address format
                            if let Err(err) = alloy_primitives::Address::parse_checksummed(
                                address.to_string(),
                                None,
                            ) {
                                errors.push(format!(
                                    "Workflow '{}' has an invalid Ethereum address format in submit action: {}",
                                    workflow_id, err
                                ));
                            }

                            // Check if max_gas is reasonable if specified
                            if let Some(gas) = max_gas {
                                if *gas == 0 {
                                    errors.push(format!(
                                        "Workflow '{}' has max_gas of zero, which will prevent transactions",
                                        workflow_id
                                    ));
                                }
                            }

                            if !workflow.aggregators.is_empty() {
                                errors.push(format!("Workflow '{}' submits with eth contract, but it has an aggregator defined", workflow_id));
                            }
                        }
                        Submit::None => {
                            // None submit type is always valid
                            if !workflow.aggregators.is_empty() {
                                errors.push(format!(
                                    "Workflow '{}' has no submit, but it has an aggregator defined",
                                    workflow_id
                                ));
                            }
                        }
                        Submit::Aggregator { url } => {
                            if reqwest::Url::parse(url).is_err() {
                                errors.push(format!(
                                    "Workflow '{}' has an invalid URL: {}",
                                    workflow_id, url
                                ))
                            }

                            if workflow.aggregators.is_empty() {
                                errors.push(format!("Workflow '{}' submits with aggregator, but no aggregator is defined", workflow_id));
                            }
                        }
                    }
                }
            }
            // Check if max_gas is reasonable if specified
            for aggregator in &workflow.aggregators {
                match aggregator {
                    Aggregator::Ethereum(ethereum_contract_submission) => {
                        if let Some(max_gas) = ethereum_contract_submission.max_gas {
                            if max_gas == 0 {
                                errors.push(format!(
                                    "Workflow aggregator '{}' has max_gas of zero, which will prevent transactions",
                                    workflow_id
                                ));
                            }
                        }
                    }
                }
            }
        }

        if matches!(&self.manager, ServiceManagerJson::Json(_)) {
            errors.push("Service has an unset service manager".to_owned());
        }

        errors
    }
}

pub fn validate_cron_config(
    start_time: Option<Timestamp>,
    end_time: Option<Timestamp>,
) -> Result<(), String> {
    // Ensure start_time <= end_time if both are provided
    if let (Some(start), Some(end)) = (start_time, end_time) {
        if start > end {
            return Err("start_time must be before or equal to end_time".to_string());
        }
    }

    // Ensure end_time is in the future
    if let Some(end) = end_time {
        let now = Timestamp::now();
        if end < now {
            return Err("end_time must be in the future".to_string());
        }
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct WorkflowJson {
    pub trigger: TriggerJson,
    pub component: ComponentJson,
    pub submit: SubmitJson,
    /// If submit is `Submit::Aggregator`, this is
    /// the required data for the aggregator to submit this workflow
    pub aggregators: Vec<Aggregator>,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Json {
    Unset,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", untagged)]
#[allow(clippy::large_enum_variant)]
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
