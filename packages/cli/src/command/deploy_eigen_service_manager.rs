use alloy::primitives::Address;
use anyhow::Result;
use rand::rngs::OsRng;
use utils::avs_client::AvsClientDeployer;

use crate::context::CliContext;

pub struct DeployEigenServiceManager {
    pub address: Address,
}

pub struct DeployEigenServiceManagerArgs {
    pub chain: String,
    pub payload_handler: Address,
    pub register_operator: bool,
}

impl DeployEigenServiceManager {
    pub async fn run(
        ctx: &CliContext,
        DeployEigenServiceManagerArgs {
            chain,
            payload_handler,
            register_operator,
        }: DeployEigenServiceManagerArgs,
    ) -> Result<Self> {
        let eigen_client = ctx.get_eth_client(&chain)?;
        let core_contracts = eigen_client.deploy_core_contracts().await?;
        let deployer = AvsClientDeployer::new(eigen_client.eth).core_addresses(core_contracts);

        let avs_client = deployer
            .deploy_service_manager(payload_handler, None)
            .await?;

        if register_operator {
            avs_client.register_operator(&mut OsRng).await?;
        }

        Ok(Self {
            address: avs_client.service_manager,
        })
    }
}
