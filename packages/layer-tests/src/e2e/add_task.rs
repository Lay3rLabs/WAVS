use std::time::Duration;

use alloy_provider::{ext::AnvilApi, Provider};
use anyhow::{bail, Context, Result};
use utils::evm_client::EvmSigningClient;
use wavs_types::{EvmContractSubmission, ServiceID, Submit, Trigger, WorkflowID};

use crate::{
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_evm_client::{
        example_submit::ISimpleSubmit::SignedData, SimpleEvmSubmitClient, SimpleEvmTriggerClient,
        TriggerId,
    },
};

use super::clients::Clients;

pub async fn add_task(
    clients: &Clients,
    service_id: String,
    workflow_id: Option<String>,
    input: Option<Vec<u8>>,
    submit_client: EvmSigningClient,
    submit_start_block: u64,
    task_should_land_on_chain: bool,
) -> Result<(TriggerId, Option<SignedData>)> {
    let service_id = ServiceID::new(service_id)?;
    let workflow_id = match workflow_id {
        Some(workflow_id) => WorkflowID::new(workflow_id)?,
        None => WorkflowID::default(),
    };

    let deployment = clients.cli_ctx.deployment.lock().unwrap().clone();

    let service = deployment
        .services
        .get(&service_id)
        .context(format!("Service not found for {}", service_id))?;

    let workflow = match service.workflows.get(&workflow_id) {
        Some(workflow) => workflow.clone(),
        None => {
            bail!(
                "Service contracts not deployed for service {} and workflow {}, deploy those first!",
                service_id,
                workflow_id
            );
        }
    };

    let trigger_id = match workflow.trigger {
        Trigger::EvmContractEvent {
            chain_name,
            address,
            event_hash: _,
        } => {
            let evm_client = clients.get_evm_client(&chain_name);
            let client = SimpleEvmTriggerClient::new(evm_client, address);

            client
                .add_trigger(input.expect("on-chain triggers require input data"))
                .await?
        }
        Trigger::CosmosContractEvent {
            chain_name,
            address,
            event_type: _,
        } => {
            let client = SimpleCosmosTriggerClient::new(
                clients.get_cosmos_client(&chain_name).await,
                address,
            );
            let trigger_id = client
                .add_trigger(input.expect("on-chain triggers require input data"))
                .await?;

            TriggerId::new(trigger_id.u64())
        }
        Trigger::BlockInterval {
            chain_name: _,
            n_blocks: _,
            ..
        } => {
            // Hardcoded id since the current flow expects it to come from the event
            TriggerId::new(1337)
        }
        Trigger::Cron { .. } => TriggerId::new(1338),
        Trigger::Manual => unimplemented!(),
    };

    match workflow.submit {
        Submit::EvmContract(EvmContractSubmission {
            chain_name,
            address,
            max_gas: _,
        }) => {
            if !task_should_land_on_chain {
                tracing::info!(
                    "Not waiting for task response on trigger {}, chain {}",
                    trigger_id,
                    chain_name
                );
                return Ok((trigger_id, None));
            }

            Ok((
                trigger_id,
                Some(
                    wait_for_task_to_land(submit_client, address, trigger_id, submit_start_block)
                        .await,
                ),
            ))
        }
        Submit::Aggregator { url } => {
            if !task_should_land_on_chain {
                tracing::info!(
                    "Not waiting for task response on trigger {}, chain {}",
                    trigger_id,
                    url
                );
                return Ok((trigger_id, None));
            }

            Ok((
                trigger_id,
                Some(
                    wait_for_task_to_land(
                        submit_client,
                        service.manager.evm_address_unchecked(),
                        trigger_id,
                        submit_start_block,
                    )
                    .await,
                ),
            ))
        }
        Submit::None => unimplemented!(),
    }
}

// The new optimized version of wait_for_task_to_land with exponential backoff
pub async fn wait_for_task_to_land(
    evm_submit_client: EvmSigningClient,
    address: alloy_primitives::Address,
    trigger_id: TriggerId,
    submit_start_block: u64,
) -> SignedData {
    let submit_client = SimpleEvmSubmitClient::new(evm_submit_client.clone(), address);

    tokio::time::timeout(Duration::from_secs(5), async move {
        let mut backoff = 50; // milliseconds
        let max_backoff = 500; // maximum backoff in milliseconds

        loop {
            // Force mining if block hasn't advanced
            if submit_client
                .evm_client
                .provider
                .get_block_number()
                .await
                .unwrap()
                == submit_start_block
            {
                submit_client
                    .evm_client
                    .provider
                    .evm_mine(None)
                    .await
                    .unwrap();
            }

            match submit_client.trigger_validated(trigger_id).await {
                true => {
                    return submit_client.signed_data(trigger_id).await.unwrap();
                }
                false => {
                    tracing::debug!("Waiting for task response on trigger {}", trigger_id);
                }
            }

            // Exponential backoff with capping
            tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
            backoff = std::cmp::min(backoff * 2, max_backoff);
        }
    })
    .await
    .unwrap()
}
