use alloy::primitives::Address;
use anyhow::Result;
use rand::rngs::OsRng;
use utils::{avs_client::AvsClientDeployer, types::ChainName};

use crate::context::CliContext;

pub struct DeployEigenServiceManager {
    pub address: Address,
}

pub struct DeployEigenServiceManagerArgs {
    pub chain: ChainName,
    pub service_handler: Address,
    pub register_operator: bool,
}

impl DeployEigenServiceManager {
    pub async fn run(
        ctx: &CliContext,
        DeployEigenServiceManagerArgs {
            chain,
            service_handler,
            register_operator,
        }: DeployEigenServiceManagerArgs,
    ) -> Result<Self> {
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

        Ok(Self {
            address: avs_client.service_manager,
        })
    }
}
