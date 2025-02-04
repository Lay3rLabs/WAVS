use alloy::primitives::Address;
use anyhow::Result;
use rand::rngs::OsRng;
use utils::avs_client::AvsClientDeployer;
use wavs_types::ChainName;

use crate::{
    context::CliContext,
    deploy::{CommandDeployResult, Deployment},
};

pub struct DeployEigenServiceManager {
    pub args: DeployEigenServiceManagerArgs,
    pub address: Address,
}

#[derive(Clone)]
pub struct DeployEigenServiceManagerArgs {
    pub chain: ChainName,
    pub service_handler: Address,
    pub register_operator: bool,
}

impl std::fmt::Display for DeployEigenServiceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "New Eigenlayer service manager deployed")?;
        if self.args.register_operator {
            write!(f, " (and registered as an operator on it)")?;
        }
        write!(f, "\n\nAddress: {}", self.address)
    }
}

impl CommandDeployResult for DeployEigenServiceManager {
    fn update_deployment(&self, deployment: &mut Deployment) {
        deployment
            .eigen_service_managers
            .entry(self.args.chain.clone())
            .or_default()
            .push(self.address);
    }
}

impl DeployEigenServiceManager {
    pub async fn run(ctx: &CliContext, args: DeployEigenServiceManagerArgs) -> Result<Self> {
        let DeployEigenServiceManagerArgs {
            chain,
            service_handler,
            register_operator,
        } = args.clone();

        let deployment = ctx.deployment.lock().unwrap().clone();

        let core_contracts = match deployment.eigen_core.get(&chain) {
            Some(core_contracts) => core_contracts.clone(),
            None => {
                tracing::error!(
                    "Eigenlayer core contracts not deployed for chain {}, deploy those first!",
                    chain
                );
                return Err(anyhow::anyhow!("Eigenlayer core contracts not deployed"));
            }
        };

        let eigen_client = ctx.get_eth_client(&chain)?;

        let deployer = AvsClientDeployer::new(eigen_client.eth).core_addresses(core_contracts);

        let avs_client = deployer
            .deploy_service_manager(service_handler, None)
            .await?;

        if register_operator {
            avs_client.register_operator(&mut OsRng).await?;
        }

        let _self = Self {
            args,
            address: avs_client.service_manager,
        };

        _self.update_deployment(&mut ctx.deployment.lock().unwrap());

        Ok(_self)
    }
}
