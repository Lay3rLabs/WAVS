use std::time::Duration;

use anyhow::Result;
use utils::avs_client::SignedData;
use wavs::apis::{ServiceID, WorkflowID};

use crate::{
    clients::{
        example_cosmos_client::SimpleCosmosTriggerClient,
        example_eth_client::{SimpleEthSubmitClient, SimpleEthTriggerClient, TriggerId},
    },
    context::CliContext,
    deploy::{ServiceSubmitInfo, ServiceTriggerInfo},
    util::ComponentInput,
};

pub struct AddTask {
    pub signed_data: Option<SignedData>,
}

pub struct AddTaskArgs {
    pub service_id: String,
    pub workflow_id: Option<String>,
    pub input: ComponentInput,
    pub result_timeout: Option<Duration>,
}

impl AddTask {
    pub async fn run(
        ctx: &CliContext,
        AddTaskArgs {
            service_id,
            workflow_id,
            input,
            result_timeout,
        }: AddTaskArgs,
    ) -> Result<Option<Self>> {
        let deployment = ctx.deployment.lock().unwrap().clone();

        let input = input.decode()?;

        let service_id = ServiceID::new(service_id)?;
        let workflow_id = match workflow_id {
            Some(workflow_id) => WorkflowID::new(workflow_id)?,
            None => WorkflowID::new("default")?,
        };

        let service = match deployment.services.get(&service_id) {
            Some(workflows) => match workflows.get(&workflow_id) {
                Some(service) => service.clone(),
                None => {
                    tracing::error!(
                        "Service contracts not deployed for service {} and workflow {}, deploy those first!",
                        service_id,
                        workflow_id
                    );
                    return Ok(None);
                }
            },
            None => {
                tracing::error!(
                    "Service contracts not deployed for service {}, deploy those first!",
                    service_id
                );
                return Ok(None);
            }
        };

        let trigger_id = match service.trigger {
            ServiceTriggerInfo::EthSimpleContract {
                chain_name,
                address,
            } => {
                let client = SimpleEthTriggerClient::new(
                    ctx.get_eth_client(&chain_name)?.eth,
                    address.try_into()?,
                );
                client.add_trigger(input).await?
            }
            ServiceTriggerInfo::CosmosSimpleContract {
                chain_name,
                address,
            } => {
                let client =
                    SimpleCosmosTriggerClient::new(ctx.get_cosmos_client(&chain_name)?, address);
                let trigger_id = client.add_trigger(input).await?;
                TriggerId::new(trigger_id.u64())
            }
        };

        match service.submit {
            ServiceSubmitInfo::EigenLayer {
                chain_name,
                avs_addresses,
            } => {
                let result_timeout = match result_timeout {
                    Some(timeout) => timeout,
                    None => {
                        tracing::info!(
                            "Not waiting for task response on trigger {}, chain {}",
                            trigger_id,
                            chain_name
                        );
                        return Ok(Some(Self { signed_data: None }));
                    }
                };
                let submit_client = SimpleEthSubmitClient::new(
                    ctx.get_eth_client(&chain_name)?.eth,
                    avs_addresses.service_manager,
                );

                let signed_data = tokio::time::timeout(result_timeout, async move {
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
                .await?;

                Ok(Some(Self {
                    signed_data: Some(signed_data?),
                }))
            }
        }
    }
}
