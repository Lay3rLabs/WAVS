use std::time::Duration;

use anyhow::Result;
use utils::{
    avs_client::{ServiceManagerClient, SignedData},
    types::{Submit, Trigger},
    ServiceID, WorkflowID,
};

use crate::{
    clients::{
        example_cosmos_client::SimpleCosmosTriggerClient,
        example_eth_client::{SimpleEthSubmitClient, SimpleEthTriggerClient, TriggerId},
    },
    context::CliContext,
    util::ComponentInput,
};

/// Add Task is specific for the example trigger contracts, mostly just for testing
pub struct AddTask {
    pub signed_data: Option<SignedData>,
}

impl std::fmt::Display for AddTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Task added!")?;

        match &self.signed_data {
            Some(signed_data) => {
                write!(
                    f,
                    "\n\nSignature (hex encoded): \n{}",
                    hex::encode(&signed_data.signature)
                )?;
                write!(
                    f,
                    "\n\nResult (hex encoded): \n{}",
                    hex::encode(&signed_data.data)
                )?;
                if let Ok(s) = std::str::from_utf8(&signed_data.data) {
                    write!(f, "\n\nResult (utf8): \n{}", s)?;
                }
            }
            None => {
                write!(f, "\n\nNot watching for the result")?;
            }
        }

        Ok(())
    }
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
            None => WorkflowID::default(),
        };

        let workflow = match deployment.services.get(&service_id) {
            Some(service) => match service.workflows.get(&workflow_id) {
                Some(workflow) => workflow.clone(),
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

        let trigger_id = match workflow.trigger {
            Trigger::EthContractEvent {
                chain_name,
                address,
                event_hash: _,
            } => {
                let client =
                    SimpleEthTriggerClient::new(ctx.get_eth_client(&chain_name)?.eth, address);
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
            Trigger::Manual => unimplemented!(),
        };

        match workflow.submit {
            Submit::EigenContract {
                chain_name,
                service_manager: service_manager_address,
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
                        return Ok(Some(Self { signed_data: None }));
                    }
                };

                let eigen_client = ctx.get_eth_client(&chain_name)?;

                let service_manager =
                    ServiceManagerClient::new(eigen_client.eth.clone(), service_manager_address);

                let submit_client = SimpleEthSubmitClient::new(
                    ctx.get_eth_client(&chain_name)?.eth,
                    service_manager.handler_address().await?,
                );

                tracing::info!("service manager address: {}", service_manager_address);

                if submit_client.get_service_manager_address().await? != service_manager_address {
                    return Err(anyhow::anyhow!("service manager address mismatch"));
                }

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
            Submit::None => unimplemented!(),
        }
    }
}
