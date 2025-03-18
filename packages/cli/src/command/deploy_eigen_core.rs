use anyhow::Result;
use utils::eigen_client::CoreAVSAddresses;
use wavs_types::{ChainName, Submit, Trigger};

use crate::{
    context::CliContext,
    deploy::{CommandDeployResult, Deployment},
};

pub struct DeployEigenCore {
    pub args: DeployEigenCoreArgs,
    pub addresses: CoreAVSAddresses,
}

impl std::fmt::Display for DeployEigenCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "New Eigenlayer core addresses deployed")?;
        if self.args.register_operator {
            write!(f, " (and registered as an operator on them)")?;
        }
        write!(f, "\n\n{:#?}", self.addresses)
    }
}

impl CommandDeployResult for DeployEigenCore {
    fn update_deployment(&self, deployment: &mut Deployment) {
        let chain = &self.args.chain;

        if deployment.eigen_core.contains_key(chain) {
            tracing::warn!("Overwriting old deployment");
            let mut deleted_services = false;

            for services in deployment.services.values_mut() {
                services.workflows.retain(|_, workflow| {
                    if let Some(chain_name) = match &workflow.trigger {
                        Trigger::EthContractEvent { chain_name, .. } => Some(chain_name),
                        Trigger::CosmosContractEvent { chain_name, .. } => Some(chain_name),
                        Trigger::BlockInterval { chain_name, .. } => Some(chain_name),
                        Trigger::Manual => None,
                    } {
                        if chain_name != chain {
                            deleted_services = true;
                            return false;
                        }
                    }
                    if let Some(chain_name) = match &workflow.submit {
                        Submit::EthereumContract { chain_name, .. } => Some(chain_name),
                        Submit::None => None,
                    } {
                        if chain_name != chain {
                            deleted_services = true;
                            return false;
                        }
                    }

                    true
                });
            }

            deployment
                .services
                .retain(|_, service| !service.workflows.is_empty());

            if deleted_services {
                tracing::warn!("Deleted old services");
            }
        }

        if deployment.eigen_service_managers.contains_key(chain) {
            tracing::warn!("Deleted old service managers");
            deployment.eigen_service_managers.remove(chain);
        }

        deployment
            .eigen_core
            .insert(chain.clone(), self.addresses.clone());
    }
}

pub struct DeployEigenCoreArgs {
    pub register_operator: bool,
    pub chain: ChainName,
}

impl DeployEigenCore {
    pub async fn run(ctx: &CliContext, args: DeployEigenCoreArgs) -> Result<Self> {
        tracing::info!("Deploying Eigenlayer core on {}", args.chain);
        let eigen_client = ctx.get_eth_client(&args.chain)?;

        let core_contracts = eigen_client.deploy_core_contracts().await?;

        if args.register_operator {
            eigen_client
                .register_operator(&core_contracts)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to register operator: {:?}", e);
                    e
                })?;
        }

        let _self = Self {
            args,
            addresses: core_contracts,
        };

        _self.update_deployment(&mut ctx.deployment.lock().unwrap());

        Ok(_self)
    }
}
