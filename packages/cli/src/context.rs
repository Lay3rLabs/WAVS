use std::{fmt::Display, sync::Mutex};

use crate::{args::CliArgs, config::Config, deploy::CommandDeployResult};
use alloy_provider::Provider;
use anyhow::{Context, Result};
use layer_climb::prelude::*;
use utils::{config::AnyChainConfig, evm_client::EvmSigningClient};
use wavs_types::ChainName;

use crate::{args::Command, deploy::Deployment};

pub struct CliContext {
    pub deployment: Mutex<Deployment>,
    pub config: Config,
    pub save_deployment: bool,
    pub quiet_results: bool,
    pub json: bool,
}

impl CliContext {
    pub async fn try_new(
        command: &Command,
        config: Config,
        deployment: Option<Deployment>,
    ) -> Result<Self> {
        let deployment = match deployment {
            None => Deployment::load(&config, command.args().json.unwrap_or_default())?,
            Some(deployment) => deployment,
        };

        Self::new_deployment(command.args(), config, Some(deployment)).await
    }

    pub async fn new_deployment(
        args: CliArgs,
        config: Config,
        deployment: Option<Deployment>,
    ) -> Result<Self> {
        let json = args.json.unwrap_or_default();
        let deployment = match deployment {
            None => Deployment::load(&config, json)?,
            Some(deployment) => deployment,
        };

        Ok(Self {
            config,
            deployment: Mutex::new(deployment),
            save_deployment: args.save_deployment.unwrap_or(true),
            quiet_results: args.quiet_results.unwrap_or_default(),
            json,
        })
    }

    pub(crate) async fn new_evm_client(&self, chain_name: &ChainName) -> Result<EvmSigningClient> {
        let chain_config = self
            .config
            .chains
            .evm
            .get(chain_name)
            .context(format!("chain {chain_name} not found"))?
            .clone();

        let client_config = chain_config.signing_client_config(
            self.config
                .evm_credential
                .clone()
                .context("missing evm_credential")?,
        )?;

        let evm_client = EvmSigningClient::new(client_config).await?;

        Ok(evm_client)
    }

    pub async fn new_cosmos_client(&self, chain_name: &ChainName) -> Result<SigningClient> {
        let chain_config = self
            .config
            .chains
            .cosmos
            .get(chain_name)
            .context(format!("chain {chain_name} not found"))?
            .clone();

        let key_signer = KeySigner::new_mnemonic_str(
            self.config
                .cosmos_mnemonic
                .as_ref()
                .context("missing mnemonic")?,
            None,
        )?;

        let climb_chain_config: ChainConfig = chain_config.into();
        SigningClient::new(climb_chain_config, key_signer, None).await
    }

    pub async fn address_exists_on_chain(
        &self,
        chain_name: &ChainName,
        address: layer_climb::prelude::Address,
    ) -> Result<bool> {
        Ok(
            match self
                .config
                .chains
                .get_chain(chain_name)
                .ok()
                .flatten()
                .context(format!("chain {chain_name} not found"))?
            {
                AnyChainConfig::Evm(_) => {
                    let address = address.try_into()?;

                    match self
                        .new_evm_client(chain_name)
                        .await?
                        .provider
                        .get_code_at(address)
                        .await
                    {
                        Ok(addr) => **addr != alloy_primitives::Address::ZERO,
                        Err(_) => false,
                    }
                }
                AnyChainConfig::Cosmos(_) => self
                    .new_cosmos_client(chain_name)
                    .await?
                    .querier
                    .contract_info(&address)
                    .await
                    .is_ok(),
            },
        )
    }

    pub fn handle_deploy_result(&self, result: impl CommandDeployResult) -> Result<()> {
        let deployment = self.deployment.lock().unwrap();

        // save the updated deployment
        if self.save_deployment {
            if !self.config.data.exists() {
                std::fs::create_dir_all(&self.config.data)?;
            }
            let path = Deployment::path(&self.config);
            tracing::debug!("Saving deployment to {}", path.display());
            let file = std::fs::File::create(path)?;
            let writer = std::io::BufWriter::new(file);
            serde_json::to_writer(writer, &*deployment)?;
        }

        self.handle_display_result(result);

        Ok(())
    }

    pub fn handle_display_result(&self, result: impl Display) {
        if !self.quiet_results {
            tracing::info!("{}", result);
        }
    }
}
