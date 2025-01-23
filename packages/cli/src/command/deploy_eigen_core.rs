use anyhow::Result;
use utils::eigen_client::CoreAVSAddresses;

use crate::{context::CliContext, deploy::Deployment};

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
        if !deployment.eigen_core.is_empty() {
            tracing::warn!("Overwriting old deployment");
        }

        *deployment = Deployment::default();

        deployment.eigen_core.insert(chain, core_contracts.clone());

        Ok(Self {
            addresses: core_contracts,
        })
    }
}
