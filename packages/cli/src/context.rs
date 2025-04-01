use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    sync::Mutex,
};

use crate::{
    args::{CliArgs, CliSubmitKind},
    clients::{get_cosmos_client, get_eth_client},
    config::Config,
    deploy::CommandDeployResult,
};
use alloy::providers::Provider;
use anyhow::{Context, Result};
use layer_climb::signing::SigningClient;
use utils::{config::AnyChainConfig, eth_client::EthSigningClient};
use wavs_types::{ChainName, EthereumContractSubmission, Submit, Trigger};

use crate::{args::Command, deploy::Deployment};

pub struct CliContext {
    pub deployment: Mutex<Deployment>,
    pub config: Config,
    pub save_deployment: bool,
    pub quiet_results: bool,
    pub json: bool,
    _clients: HashMap<ChainName, AnyClient>,
}

enum AnyClient {
    Eth(EthSigningClient),
    Cosmos(SigningClient),
}

impl CliContext {
    pub async fn try_new(
        command: &Command,
        config: Config,
        deployment: Option<Deployment>,
    ) -> Result<Self> {
        let mut chains: HashSet<ChainName> = HashSet::new();

        let deployment = match deployment {
            None => Deployment::load(&config, command.args().json.unwrap_or_default())?,
            Some(deployment) => deployment,
        };

        match command {
            Command::DeployService {
                trigger_chain,
                submit_chain,
                submit,
                ..
            } => {
                if let Some(chain) = trigger_chain {
                    chains.insert(chain.clone());
                }

                // only add the submit chain if we'll actually use it
                match (submit_chain, submit) {
                    (_, CliSubmitKind::None) | (None, _) => {}
                    (Some(chain), _) => {
                        chains.insert(chain.clone());
                    }
                }
            }
            Command::DeployServiceRaw { service, .. } => {
                for workflow in service.workflows.values() {
                    match &workflow.trigger {
                        Trigger::EthContractEvent { chain_name, .. } => {
                            chains.insert(chain_name.clone());
                        }
                        Trigger::CosmosContractEvent { chain_name, .. } => {
                            chains.insert(chain_name.clone());
                        }
                        Trigger::BlockInterval { chain_name, .. } => {
                            chains.insert(chain_name.clone());
                        }
                        Trigger::Manual => {}
                    }

                    match &workflow.submit {
                        Submit::EthereumContract(EthereumContractSubmission {
                            chain_name, ..
                        }) => {
                            chains.insert(chain_name.clone());
                        }
                        Submit::Aggregator { .. } => {}
                        Submit::None => {}
                    }
                }
            }
            Command::UploadComponent { .. } => {}
            Command::Exec { .. } => {}
            Command::Service { .. } => {}
        }

        Self::new_chains(
            command.args(),
            chains.into_iter().collect(),
            config,
            Some(deployment),
        )
        .await
    }

    pub async fn new_chains(
        args: CliArgs,
        chains: Vec<ChainName>,
        config: Config,
        deployment: Option<Deployment>,
    ) -> Result<Self> {
        let json = args.json.unwrap_or_default();
        let deployment = match deployment {
            None => Deployment::load(&config, json)?,
            Some(deployment) => deployment,
        };

        let mut clients = HashMap::new();

        for chain_name in chains {
            let chain = config
                .chains
                .get_chain(&chain_name)?
                .context(format!("chain {chain_name} not found"))?;

            match chain {
                AnyChainConfig::Eth(eth_chain_config) => {
                    clients.insert(
                        chain_name,
                        AnyClient::Eth(get_eth_client(&config, eth_chain_config).await?),
                    );
                }
                AnyChainConfig::Cosmos(cosmos_chain_config) => {
                    clients.insert(
                        chain_name,
                        AnyClient::Cosmos(get_cosmos_client(&config, cosmos_chain_config).await?),
                    );
                }
            }
        }

        Ok(Self {
            config,
            deployment: Mutex::new(deployment),
            save_deployment: args.save_deployment.unwrap_or(true),
            quiet_results: args.quiet_results.unwrap_or_default(),
            json,
            _clients: clients,
        })
    }

    pub fn get_eth_client(&self, chain_name: &ChainName) -> Result<EthSigningClient> {
        match self
            ._clients
            .get(chain_name)
            .context(format!("chain {chain_name} not found"))?
        {
            AnyClient::Eth(client) => Ok(client.clone()),
            _ => Err(anyhow::anyhow!("expected eth client")),
        }
    }

    pub fn get_cosmos_client(&self, chain_name: &ChainName) -> Result<SigningClient> {
        match self
            ._clients
            .get(chain_name)
            .context(format!("chain {chain_name} not found"))?
        {
            AnyClient::Cosmos(client) => Ok(client.clone()),
            _ => Err(anyhow::anyhow!("expected cosmos client")),
        }
    }

    pub async fn address_exists_on_chain(
        &self,
        chain_name: &ChainName,
        address: layer_climb::prelude::Address,
    ) -> Result<bool> {
        Ok(
            match self
                ._clients
                .get(chain_name)
                .context(format!("chain {chain_name} not found"))?
            {
                AnyClient::Eth(client) => {
                    let address = address.try_into()?;

                    match client.provider.get_code_at(address).await {
                        Ok(addr) => **addr != alloy::primitives::Address::ZERO,
                        Err(_) => false,
                    }
                }
                AnyClient::Cosmos(client) => client.querier.contract_info(&address).await.is_ok(),
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
