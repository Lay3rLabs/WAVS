use std::{collections::BTreeMap, num::NonZeroU64, str::FromStr};

use serde::{Deserialize, Serialize};
use utils::config::WAVS_ENV_PREFIX;
use wavs_types::{
    Component, ServiceID, ServiceManager, ServiceStatus, Submit, Timestamp, Trigger, WorkflowID,
};

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
                    if !key.starts_with(WAVS_ENV_PREFIX) {
                        errors.push(format!(
                "Workflow '{}' has environment variable '{}' that doesn't start with '{}'",
                workflow_id, key, WAVS_ENV_PREFIX
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
                        Trigger::EvmContractEvent {
                            address,
                            chain_name: _,
                            event_hash,
                        } => {
                            // Validate EVM address format
                            if let Err(err) = alloy_primitives::Address::parse_checksummed(
                                address.to_string(),
                                None,
                            ) {
                                errors.push(format!(
                                    "Workflow '{}' has an invalid EVM address format: {}",
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
                            schedule,
                            start_time,
                            end_time,
                        } => {
                            if let Err(err) = cron::Schedule::from_str(schedule) {
                                errors.push(format!(
                                    "Workflow '{}' has an invalid cron trigger schedule: {}",
                                    workflow_id, err
                                ));
                            }

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
                            start_block,
                            end_block,
                        } => {
                            if let Err(err) =
                                validate_block_interval_config(*start_block, *end_block)
                            {
                                errors.push(format!(
                                    "Workflow '{}' has an invalid block-interval trigger: {}",
                                    workflow_id, err
                                ));
                            }
                        }
                        Trigger::Manual => {
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
                        Submit::None => {
                            // None submit type is always valid
                        }
                        Submit::Aggregator {
                            url, evm_contracts, ..
                        } => {
                            if reqwest::Url::parse(url).is_err() {
                                errors.push(format!(
                                    "Workflow '{}' has an invalid URL: {}",
                                    workflow_id, url
                                ))
                            }

                            if evm_contracts.as_ref().map_or(true, |c| c.is_empty()) {
                                errors.push(format!("Workflow '{}' submits with aggregator, but no aggregator is defined", workflow_id));
                            }
                        }
                    }
                }
            }
            // Check if max_gas is reasonable if specified
            if let SubmitJson::Submit(Submit::Aggregator {
                evm_contracts: Some(contracts),
                ..
            }) = &workflow.submit
            {
                for evm_contract_submission in contracts {
                    if let Some(max_gas) = evm_contract_submission.max_gas {
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

pub fn validate_block_interval_config(
    start_block: Option<NonZeroU64>,
    end_block: Option<NonZeroU64>,
) -> Result<(), String> {
    // Ensure start_block <= end_block if both are provided
    if let (Some(start), Some(end)) = (start_block, end_block) {
        if start > end {
            return Err("start_block must be before or equal to end_block".to_string());
        }
    }

    Ok(())
}

pub fn validate_block_interval_config_on_chain(
    start_block: Option<NonZeroU64>,
    end_block: Option<NonZeroU64>,
    current_block: u64,
) -> Result<(), String> {
    validate_block_interval_config(start_block, end_block)?;

    if let Some(start) = start_block {
        if current_block > start.get() {
            return Err(format!("cannot start an interval in the past (current block is {}, explicit start_block is {})", current_block, start));
        }
    }

    if let Some(end) = end_block {
        if current_block > end.get() {
            return Err(format!(
                "cannot end an interval in the past (current block is {}, end_block is {})",
                current_block, end
            ));
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
