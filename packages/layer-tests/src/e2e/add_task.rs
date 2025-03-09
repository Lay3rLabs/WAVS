use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use anyhow::{bail, Result};
use utils::avs_client::{layer_service_aggregator::WavsServiceAggregator, SignedData};
use wavs_cli::context::CliContext;
use wavs_types::{ChainName, ServiceID, Submit, Trigger, WorkflowID};

use crate::{
    example_cosmos_client::SimpleCosmosTriggerClient,
    example_eth_client::{SimpleEthSubmitClient, SimpleEthTriggerClient, TriggerId},
};

pub async fn add_task(
    ctx: &CliContext,
    service_id: String,
    workflow_id: Option<String>,
    input: Vec<u8>,
    result_timeout: Option<Duration>,
    is_aggregator: bool,
) -> Result<(TriggerId, Option<SignedData>)> {
    let service_id = ServiceID::new(service_id)?;
    let workflow_id = match workflow_id {
        Some(workflow_id) => WorkflowID::new(workflow_id)?,
        None => WorkflowID::default(),
    };

    let deployment = ctx.deployment.lock().unwrap().clone();

    let workflow = match deployment.services.get(&service_id) {
        Some(service) => match service.workflows.get(&workflow_id) {
            Some(workflow) => workflow.clone(),
            None => {
                bail!(
                    "Service contracts not deployed for service {} and workflow {}, deploy those first!",
                    service_id,
                    workflow_id
                );
            }
        },
        None => {
            bail!(
                "Service contracts not deployed for service {}, deploy those first!",
                service_id
            );
        }
    };

    let trigger_id = match workflow.trigger {
        Trigger::EthContractEvent {
            chain_name,
            address,
            event_hash: _,
        } => {
            let client = SimpleEthTriggerClient::new(ctx.get_eth_client(&chain_name)?.eth, address);
            client.add_trigger(input).await?
        }
        Trigger::CosmosContractEvent {
            chain_name,
            address,
            event_type: _,
        } => {
            let client =
                SimpleCosmosTriggerClient::new(ctx.get_cosmos_client(&chain_name)?, address);
            let trigger_id = client.add_trigger(input).await?;
            TriggerId::new(trigger_id.u64())
        }
        Trigger::BlockInterval {
            chain_name,
            trigger_name,
            n_blocks,
        } => {
            let mut hasher = DefaultHasher::new();
            chain_name.hash(&mut hasher);
            trigger_name.hash(&mut hasher);
            n_blocks.hash(&mut hasher);
            TriggerId::new(hasher.finish())
        }
        Trigger::Manual => unimplemented!(),
    };

    match workflow.submit {
        Submit::EthereumContract {
            chain_name,
            address,
            max_gas: _,
        } => {
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
                        is_aggregator,
                        result_timeout,
                    )
                    .await?,
                ),
            ))
        }
        Submit::None => unimplemented!(),
    }
}

pub async fn wait_for_task_to_land(
    ctx: &CliContext,
    chain_name: &ChainName,
    address: alloy::primitives::Address,
    trigger_id: TriggerId,
    is_aggregator: bool,
    result_timeout: Duration,
) -> Result<SignedData> {
    let client = ctx.get_eth_client(chain_name)?;

    let address = match is_aggregator {
        false => address,
        true => {
            let contract = WavsServiceAggregator::new(address, client.eth.provider.clone());
            contract.getHandler().call().await?._0
        }
    };

    let submit_client = SimpleEthSubmitClient::new(client.eth, address);

    tokio::time::timeout(result_timeout, async move {
        loop {
            match submit_client.trigger_validated(trigger_id).await {
                true => {
                    let data = submit_client.trigger_data(trigger_id).await?;

                    let signature = submit_client.trigger_signature(trigger_id).await?;

                    return anyhow::Ok(SignedData { data, signature });
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
