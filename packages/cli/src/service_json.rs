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
            // Check if the component exists
            if !self.components.contains_key(&workflow.component) {
                errors.push(format!(
                    "Workflow '{}' references non-existent component '{}'",
                    workflow_id, workflow.component
                ));
            }

            // Check if trigger is unset
            match &workflow.trigger {
                TriggerJson::Json(Json::Unset) => {
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
                            if let Err(err) = alloy::primitives::Address::parse_checksummed(
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
                SubmitJson::Json(Json::Unset) => {
                    errors.push(format!("Workflow '{}' has an unset submit", workflow_id));
                }
                SubmitJson::Submit(submit) => {
                    // Basic submit validation
                    match submit {
                        Submit::EthereumContract {
                            address,
                            chain_name: _,
                            max_gas,
                        } => {
                            // Validate Ethereum address format
                            if let Err(err) = alloy::primitives::Address::parse_checksummed(
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
                        }
                        Submit::None => {
                            // None submit type is always valid
                        }
                    }
                }
            }

            // Validate fuel limit
            if let Some(limit) = workflow.fuel_limit {
                if limit == 0 {
                    errors.push(format!(
                        "Workflow '{}' has a fuel limit of zero, which will prevent execution",
                        workflow_id
                    ));
                }
            }
        }

        errors
    }
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
