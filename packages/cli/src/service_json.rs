use std::{num::NonZeroU64, str::FromStr};

use alloy_primitives::Address;
use cron::Schedule;
use wavs_types::{
    AggregatorBuilder, ServiceBuilder, ServiceManagerBuilder, SolanaEventFilter, Submit,
    SubmitBuilder, Timestamp, Trigger, TriggerBuilder, WAVS_ENV_PREFIX,
};

pub trait ServiceJsonExt {
    fn validate(&self) -> Vec<String>;
}

impl ServiceJsonExt for ServiceBuilder {
    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if self.name.is_empty() {
            errors.push("Service name cannot be empty".to_string());
        }

        for (workflow_id, workflow) in &self.workflows {
            if workflow.component.is_unset() {
                errors.push(format!("Workflow '{}' has an unset component", workflow_id));
            } else if let Some(component) = workflow.component.as_component() {
                if let Some(limit) = component.fuel_limit {
                    if limit == 0 {
                        errors.push(format!(
                            "Workflow '{}' has a fuel limit of zero, which will prevent execution",
                            workflow_id
                        ));
                    }
                }

                for key in &component.env_keys {
                    if !key.starts_with(WAVS_ENV_PREFIX) {
                        errors.push(format!(
                            "Workflow '{}' has environment variable '{}' that doesn't start with '{}'",
                            workflow_id, key, WAVS_ENV_PREFIX
                        ));
                    }
                }
            }

            match &workflow.trigger {
                TriggerBuilder::Builder(_) => {
                    errors.push(format!("Workflow '{}' has an unset trigger", workflow_id));
                }
                TriggerBuilder::Trigger(trigger) => match trigger {
                    Trigger::CosmosContractEvent { event_type, .. } => {
                        if event_type.is_empty() {
                            errors.push(format!(
                                "Workflow '{}' has an empty event type in Cosmos trigger",
                                workflow_id
                            ));
                        }
                    }
                    Trigger::EvmContractEvent {
                        address,
                        chain: _,
                        event_hash,
                    } => {
                        if let Err(err) = Address::parse_checksummed(address.to_string(), None) {
                            errors.push(format!(
                                "Workflow '{}' has an invalid EVM address format: {}",
                                workflow_id, err
                            ));
                        }

                        if event_hash.as_slice().len() != 32 {
                            errors.push(format!(
                                "Workflow '{}' has an invalid event hash length: expected 32 bytes but got {} bytes",
                                workflow_id,
                                event_hash.as_slice().len()
                            ));
                        }
                    }
                    Trigger::Cron {
                        schedule,
                        start_time,
                        end_time,
                    } => {
                        if let Err(err) = Schedule::from_str(schedule) {
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
                        chain: _,
                        n_blocks: _,
                        start_block,
                        end_block,
                    } => {
                        if let Err(err) = validate_block_interval_config(*start_block, *end_block) {
                            errors.push(format!(
                                "Workflow '{}' has an invalid block-interval trigger: {}",
                                workflow_id, err
                            ));
                        }
                    }
                    Trigger::HypercoreAppend { feed_key } => {
                        if feed_key.trim().is_empty() {
                            errors.push(format!(
                                "Workflow '{}' has an empty feed_key in Hypercore trigger",
                                workflow_id
                            ));
                        }
                    }
                    Trigger::SolanaProgramEvent {
                        chain: _,
                        program_id,
                        filter,
                        commitment: _,
                    } => {
                        // The `SolanaAddress` newtype already enforces the
                        // 32-byte invariant at deserialize time, so we
                        // only need to validate the filter here. We do
                        // re-check the address length defensively in case
                        // a future loosened constructor lets a wrong-
                        // length address through.
                        if program_id.as_bytes().len() != 32 {
                            errors.push(format!(
                                "Workflow '{}' has a Solana program_id that is not 32 bytes",
                                workflow_id
                            ));
                        }

                        match filter {
                            SolanaEventFilter::Discriminator(disc) => {
                                if disc.len() != 8 {
                                    errors.push(format!(
                                        "Workflow '{}' has a Solana discriminator of {} bytes; Anchor convention is exactly 8",
                                        workflow_id,
                                        disc.len()
                                    ));
                                }
                            }
                            SolanaEventFilter::LogContains(needle) => {
                                if needle.is_empty() {
                                    errors.push(format!(
                                        "Workflow '{}' has an empty Solana log-contains filter",
                                        workflow_id
                                    ));
                                }
                            }
                        }
                    }
                    Trigger::Manual | Trigger::AtProtoEvent { .. } => {}
                },
            }

            match &workflow.submit {
                SubmitBuilder::Builder(_) => {
                    errors.push(format!("Workflow '{}' has an unset submit", workflow_id));
                }
                SubmitBuilder::AggregatorBuilder(aggregator_json) => match aggregator_json {
                    AggregatorBuilder::Aggregator {
                        component,
                        signature_kind: _,
                    } => {
                        if component.is_unset() {
                            errors.push(format!(
                                "Workflow '{}' has an unset aggregator component",
                                workflow_id
                            ));
                        }
                    }
                },
                SubmitBuilder::Submit(Submit::None) => {}
                SubmitBuilder::Submit(Submit::Aggregator { component, .. }) => {
                    if let Some(limit) = component.fuel_limit {
                        if limit == 0 {
                            errors.push(format!(
                                "Workflow '{}' has an aggregator component with a fuel limit of zero, which will prevent execution",
                                workflow_id
                            ));
                        }
                    }

                    for key in &component.env_keys {
                        if !key.starts_with(WAVS_ENV_PREFIX) {
                            errors.push(format!(
                                "Workflow '{}' has aggregator component environment variable '{}' that doesn't start with '{}'",
                                workflow_id, key, WAVS_ENV_PREFIX
                            ));
                        }
                    }
                }
            }
        }

        if matches!(&self.manager, ServiceManagerBuilder::Builder(_)) {
            errors.push("Service has an unset service manager".to_owned());
        }

        errors
    }
}

pub fn validate_cron_config(
    start_time: Option<Timestamp>,
    end_time: Option<Timestamp>,
) -> Result<(), String> {
    if let (Some(start), Some(end)) = (start_time, end_time) {
        if start > end {
            return Err("start_time must be before or equal to end_time".to_string());
        }
    }

    {
        if let Some(end) = end_time {
            let now = Timestamp::now();
            if end < now {
                return Err("end_time must be in the future".to_string());
            }
        }
    }

    Ok(())
}

pub fn validate_block_interval_config(
    start_block: Option<NonZeroU64>,
    end_block: Option<NonZeroU64>,
) -> Result<(), String> {
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
            return Err(format!(
                "cannot start an interval in the past (current block is {}, explicit start_block is {})",
                current_block, start
            ));
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

#[cfg(test)]
mod solana_validator_tests {
    use super::*;
    use wavs_types::{
        Component, ComponentBuilder, ComponentDigest, ComponentSource, ServiceBuilder,
        SolanaAddress, SolanaCommitment, SolanaEventFilter, WorkflowBuilder, WorkflowId,
    };

    fn service_with_solana_trigger(filter: SolanaEventFilter) -> ServiceBuilder {
        let program_id = SolanaAddress::from_base58("11111111111111111111111111111111").unwrap();
        let trigger = Trigger::solana_program_event(
            "solana:devnet",
            program_id,
            filter,
            SolanaCommitment::Confirmed,
        );

        let workflow = WorkflowBuilder {
            component: ComponentBuilder::new(Component::new(ComponentSource::Digest(
                ComponentDigest::hash([0u8; 32]),
            ))),
            trigger: TriggerBuilder::Trigger(trigger),
            submit: SubmitBuilder::Submit(Submit::None),
        };

        let mut workflows = std::collections::BTreeMap::new();
        workflows.insert(WorkflowId::new("workflow-test").unwrap(), workflow);

        ServiceBuilder {
            name: "test-service".to_string(),
            workflows,
            status: wavs_types::ServiceStatus::Active,
            manager: ServiceManagerBuilder::default(),
        }
    }

    #[test]
    fn solana_discriminator_must_be_eight_bytes() {
        // Anchor convention is exactly 8 bytes.
        let bad = service_with_solana_trigger(SolanaEventFilter::Discriminator(vec![1, 2, 3]));
        let errors = bad.validate();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("Anchor convention is exactly 8")),
            "expected discriminator-length error in {errors:?}"
        );
    }

    #[test]
    fn solana_log_contains_must_be_nonempty() {
        let bad = service_with_solana_trigger(SolanaEventFilter::LogContains(String::new()));
        let errors = bad.validate();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("empty Solana log-contains filter")),
            "expected empty-needle error in {errors:?}"
        );
    }

    #[test]
    fn solana_eight_byte_discriminator_is_accepted() {
        let good = service_with_solana_trigger(SolanaEventFilter::Discriminator(vec![0u8; 8]));
        let errors = good.validate();
        // Other unrelated validation errors are fine (e.g. unset
        // service-manager); we only care that the trigger itself doesn't
        // contribute one.
        assert!(
            !errors.iter().any(|e| e.contains("Solana discriminator")),
            "did not expect discriminator error in {errors:?}"
        );
    }

    #[test]
    fn solana_log_contains_nonempty_is_accepted() {
        let good = service_with_solana_trigger(SolanaEventFilter::LogContains("Transfer".into()));
        let errors = good.validate();
        assert!(
            !errors
                .iter()
                .any(|e| e.contains("empty Solana log-contains filter")),
            "did not expect log-contains error in {errors:?}"
        );
    }
}
