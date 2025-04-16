use std::time::Duration;

use alloy_provider::{ext::AnvilApi, Provider};
use anyhow::{bail, Context, Result};
use utils::eth_client::EthSigningClient;
use wavs_types::{Envelope, EthereumContractSubmission, ServiceID, Submit, Trigger, WorkflowID};

use crate::{
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_eth_client::{SimpleEthSubmitClient, SimpleEthTriggerClient, TriggerId},
};

use super::clients::Clients;

pub async fn add_task(
    clients: &Clients,
    service_id: String,
    workflow_id: Option<String>,
    input: Option<Vec<u8>>,
    submit_client: EthSigningClient,
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
        Trigger::EthContractEvent {
            chain_name,
            address,
            event_hash: _,
        } => {
            let eth_client = clients.get_eth_client(&chain_name);
            let client = SimpleEthTriggerClient::new(eth_client, address);

            client
                .add_trigger(input.expect("on-chain triggers require input data"))
                .await?
        }
        Trigger::CosmosContractEvent {
            chain_name,
            address,
            event_type: _,
        } => {
            let client =
                SimpleCosmosTriggerClient::new(clients.get_cosmos_client(&chain_name), address);
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
        Submit::EthereumContract(EthereumContractSubmission {
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
                        .await?,
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
                        service.manager.eth_address_unchecked(),
                        trigger_id,
                        submit_start_block,
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
    pub envelope: Envelope,
    pub signature: Vec<u8>,
}

pub async fn wait_for_task_to_land(
    eth_submit_client: EthSigningClient,
    address: alloy_primitives::Address,
    trigger_id: TriggerId,
    submit_start_block: u64,
) -> Result<SignedData> {
    let submit_client = SimpleEthSubmitClient::new(eth_submit_client, address);

    tokio::time::timeout(Duration::from_secs(5), async move {
        loop {
            if submit_client.eth.provider.get_block_number().await? == submit_start_block {
                submit_client.eth.provider.evm_mine(None).await?;
            }
            match submit_client.trigger_validated(trigger_id).await {
                true => {
                    let data = submit_client.trigger_data(trigger_id).await?;

                    let envelope = submit_client.trigger_envelope(trigger_id).await?;

                    let signature = submit_client.trigger_signature(trigger_id).await?;

                    return anyhow::Ok(SignedData {
                        data,
                        signature,
                        envelope,
                    });
                }
                false => {
                    tracing::debug!("Waiting for task response on trigger {}", trigger_id,);
                }
            }

            // still open, waiting...
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await?
}
