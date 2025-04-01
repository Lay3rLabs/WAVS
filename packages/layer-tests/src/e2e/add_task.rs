use std::time::Duration;

use alloy::providers::ext::AnvilApi;
use anyhow::{bail, Context, Result};
use wavs_cli::context::CliContext;
use wavs_types::{ChainName, EthereumContractSubmission, ServiceID, Submit, Trigger, WorkflowID};

use crate::{
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_eth_client::{SimpleEthSubmitClient, SimpleEthTriggerClient, TriggerId},
};

pub async fn add_task(
    ctx: &CliContext,
    service_id: String,
    workflow_id: Option<String>,
    input: Option<Vec<u8>>,
    result_timeout: Option<Duration>,
) -> Result<(TriggerId, Option<SignedData>)> {
    let service_id = ServiceID::new(service_id)?;
    let workflow_id = match workflow_id {
        Some(workflow_id) => WorkflowID::new(workflow_id)?,
        None => WorkflowID::default(),
    };

    let deployment = ctx.deployment.lock().unwrap().clone();

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

    let (is_trigger_time_based, trigger_id) = match workflow.trigger {
        Trigger::EthContractEvent {
            chain_name,
            address,
            event_hash: _,
        } => {
            let client = SimpleEthTriggerClient::new(ctx.get_eth_client(&chain_name)?, address);
            (
                false,
                client
                    .add_trigger(input.expect("on-chain triggers require input data"))
                    .await?,
            )
        }
        Trigger::CosmosContractEvent {
            chain_name,
            address,
            event_type: _,
        } => {
            let client =
                SimpleCosmosTriggerClient::new(ctx.get_cosmos_client(&chain_name)?, address);
            let trigger_id = client
                .add_trigger(input.expect("on-chain triggers require input data"))
                .await?;
            (false, TriggerId::new(trigger_id.u64()))
        }
        Trigger::BlockInterval {
            chain_name: _,
            n_blocks: _,
            ..
        } => {
            // Hardcoded id since the current flow expects it to come from the event
            (true, TriggerId::new(1337))
        }
        Trigger::Cron { .. } => (true, TriggerId::new(1338)),
        Trigger::Manual => unimplemented!(),
    };

    match workflow.submit {
        Submit::EthereumContract(EthereumContractSubmission {
            chain_name,
            address,
            max_gas: _,
        }) => {
            let result_timeout = match result_timeout {
                Some(timeout) => timeout,
                None => {
                    tracing::info!(
                        "Not waiting for task response on trigger {}, chain {}",
                        trigger_id,
                        chain_name
                    );
                    return Ok((trigger_id, None));
                }
            };

            Ok((
                trigger_id,
                Some(
                    wait_for_task_to_land(
                        ctx,
                        &chain_name,
                        address,
                        trigger_id,
                        result_timeout,
                        is_trigger_time_based,
                    )
                    .await?,
                ),
            ))
        }
        Submit::Aggregator { url } => {
            let result_timeout = match result_timeout {
                Some(timeout) => timeout,
                None => {
                    tracing::info!(
                        "Not waiting for task response on trigger {}, chain {}",
                        trigger_id,
                        url
                    );
                    return Ok((trigger_id, None));
                }
            };

            Ok((
                trigger_id,
                Some(
                    wait_for_task_to_land(
                        ctx,
                        service.manager.chain_name(),
                        service.manager.eth_address_unchecked(),
                        trigger_id,
                        result_timeout,
                        is_trigger_time_based,
                    )
                    .await?,
                ),
            ))
        }
        Submit::None => unimplemented!(),
    }
}

#[derive(Debug, Clone)]
pub struct SignedData {
    pub data: Vec<u8>,
    pub _signature: Vec<u8>,
}

pub async fn wait_for_task_to_land(
    ctx: &CliContext,
    chain_name: &ChainName,
    address: alloy::primitives::Address,
    trigger_id: TriggerId,
    result_timeout: Duration,
    is_trigger_time_based: bool,
) -> Result<SignedData> {
    let client = ctx.get_eth_client(chain_name)?;
    let provider = client.provider.clone();

    let submit_client = SimpleEthSubmitClient::new(client, address);

    tokio::time::timeout(result_timeout, async move {
        loop {
            if is_trigger_time_based {
                // if the trigger is time based we need to manually tell anvil
                // to move the block forward
                provider.evm_mine(None).await?;
            }
            match submit_client.trigger_validated(trigger_id).await {
                true => {
                    let data = submit_client.trigger_data(trigger_id).await?;

                    let _signature = submit_client.trigger_signature(trigger_id).await?;

                    return anyhow::Ok(SignedData { data, _signature });
                }
                false => {
                    tracing::debug!(
                        "Waiting for task response on trigger {}, chain {}",
                        trigger_id,
                        chain_name
                    );
                }
            }
            // still open, waiting...
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await?
}
