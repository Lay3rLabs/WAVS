use anyhow::Result;
use utils::eigen_client::CoreAVSAddresses;

use crate::{
    context::CliContext,
    deploy::{ServiceSubmitInfo, ServiceTriggerInfo},
};

pub struct DeployEigenCore {
    pub addresses: CoreAVSAddresses,
}

pub struct DeployEigenCoreArgs {
    pub register_operator: bool,
    pub chain: String,
}

impl DeployEigenCore {
    pub async fn run(
        ctx: &CliContext,
        DeployEigenCoreArgs {
            register_operator,
            chain,
        }: DeployEigenCoreArgs,
    ) -> Result<Self> {
        let eigen_client = ctx.get_eth_client(&chain)?;

        let core_contracts = eigen_client.deploy_core_contracts().await?;

        if register_operator {
            eigen_client
                .register_operator(&core_contracts)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to register operator: {:?}", e);
                    e
                })?;
        }

        let deployment = &mut *ctx.deployment.lock().unwrap();
        if deployment.eigen_core.contains_key(&chain) {
            tracing::warn!("Overwriting old deployment");
            let mut deleted_services = false;

            for workflows in deployment.services.values_mut() {
                workflows.retain(|_, workflow| {
                    if let Some(chain_name) = match &workflow.trigger {
                        ServiceTriggerInfo::EthSimpleContract { chain_name, .. } => {
                            Some(chain_name)
                        }
                        ServiceTriggerInfo::CosmosSimpleContract { chain_name, .. } => {
                            Some(chain_name)
                        }
                    } {
                        if *chain_name != chain {
                            deleted_services = true;
                            return false;
                        }
                    }
                    if let Some(chain_name) = match &workflow.submit {
                        ServiceSubmitInfo::EigenLayer { chain_name, .. } => Some(chain_name),
                    } {
                        if *chain_name != chain {
                            deleted_services = true;
                            return false;
                        }
                    }

                    true
                });
            }

            deployment
                .services
                .retain(|_, workflows| !workflows.is_empty());

            if deleted_services {
                tracing::warn!("Deleted old services");
            }
        }

        deployment.eigen_core.insert(chain, core_contracts.clone());

        Ok(Self {
            addresses: core_contracts,
        })
    }
}
