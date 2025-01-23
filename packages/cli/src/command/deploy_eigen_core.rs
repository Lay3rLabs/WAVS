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

        let deployment = ctx.deployment.lock().unwrap().clone();

        let core_contracts = match deployment.eigen_core.get(&chain) {
            Some(core_contracts) => {
                match ctx
                    .address_exists_on_chain(&chain, core_contracts.delegation_manager.into())
                    .await?
                {
                    true => {
                        tracing::warn!("Core contracts already deployed for chain {}", chain);
                        Some(core_contracts.clone())
                    }
                    false => {
                        tracing::warn!("Core contracts already deployed for chain {}, but service manager not found.. redeploying", chain);
                        None
                    }
                }
            }
            None => None,
        };

        let (core_contracts, fresh) = match core_contracts {
            Some(core_contracts) => (core_contracts, false),
            None => {
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

                (core_contracts, true)
            }
        };

        let mut deployment = deployment;
        if fresh {
            if !deployment.eigen_core.is_empty() {
                tracing::warn!("Overwriting deployment due to outdated core contracts");
            }
            deployment = Deployment::default();
        }
        deployment.eigen_core.insert(chain, core_contracts.clone());

        *ctx.deployment.lock().unwrap() = deployment;

        Ok(Self {
            addresses: core_contracts,
        })
    }
}
