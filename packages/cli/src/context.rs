use std::collections::{HashMap, HashSet};

use crate::{
    client::{get_cosmos_client, get_eigen_client},
    config::Config,
};
use alloy::providers::Provider;
use layer_climb::signing::SigningClient;
use utils::{config::AnyChainConfig, eigen_client::EigenClient};
use wavs::apis::{ServiceID, WorkflowID};

use crate::{args::Command, deploy::Deployment};

pub struct ChainContext {
    pub deployment: Deployment,
    pub config: Config,
    _clients: HashMap<String, AnyClient>,
}

enum AnyClient {
    Eth(EigenClient),
    Cosmos(SigningClient),
}

impl ChainContext {
    pub async fn try_new(command: &Command, config: Config) -> Self {
        let mut chains = HashSet::new();

        let deployment = Deployment::load(&config).unwrap();

        match command {
            Command::DeployEigenCore { chain, .. } => {
                chains.insert(chain.to_string());
            }
            Command::DeployService {
                trigger_chain,
                submit_chain,
                ..
            } => {
                if let Some(chain) = trigger_chain {
                    chains.insert(chain.to_string());
                }

                if let Some(chain) = submit_chain {
                    chains.insert(chain.to_string());
                }
            }
            Command::AddTask {
                service_id,
                workflow_id,
                ..
            } => {
                let service_id = ServiceID::new(service_id).unwrap();
                let workflow_id = workflow_id.as_ref().map(|x| WorkflowID::new(x).unwrap());

                if let Some((chain, _)) =
                    deployment.get_trigger_info(&service_id, workflow_id.as_ref())
                {
                    chains.insert(chain);
                }

                if let Some((chain, _)) =
                    deployment.get_submit_info(&service_id, workflow_id.as_ref())
                {
                    chains.insert(chain);
                }
            }
            Command::Exec { .. } => {}
        }

        let mut clients = HashMap::new();

        for chain_name in chains {
            let chain = config
                .chains
                .get_chain(&chain_name)
                .unwrap()
                .unwrap_or_else(|| panic!("chain {chain_name} not found"));

            match chain {
                AnyChainConfig::Eth(eth_chain_config) => {
                    clients.insert(
                        chain_name,
                        AnyClient::Eth(get_eigen_client(&config, eth_chain_config.into()).await),
                    );
                }
                AnyChainConfig::Cosmos(cosmos_chain_config) => {
                    clients.insert(
                        chain_name,
                        AnyClient::Cosmos(get_cosmos_client(&config, cosmos_chain_config).await),
                    );
                }
            }
        }

        Self {
            config,
            deployment,
            _clients: clients,
        }
    }

    pub fn save_deployment(&mut self) {
        if !self.config.data.exists() {
            std::fs::create_dir_all(&self.config.data).unwrap();
        }
        let path = Deployment::path(&self.config);
        tracing::debug!("Saving deployment to {}", path.display());
        let file = std::fs::File::create(path).unwrap();
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer(writer, &self.deployment).unwrap();
    }

    pub fn get_eth_client(&self, chain_name: &str) -> EigenClient {
        match self
            ._clients
            .get(chain_name)
            .unwrap_or_else(|| panic!("chain {chain_name} not found"))
        {
            AnyClient::Eth(client) => client.clone(),
            _ => panic!("expected eth client"),
        }
    }

    pub fn get_cosmos_client(&self, chain_name: &str) -> SigningClient {
        match self
            ._clients
            .get(chain_name)
            .unwrap_or_else(|| panic!("chain {chain_name} not found"))
        {
            AnyClient::Cosmos(client) => client.clone(),
            _ => panic!("expected cosmos client"),
        }
    }

    pub async fn address_exists_on_chain(
        &self,
        chain_name: &str,
        address: layer_climb::prelude::Address,
    ) -> bool {
        match self
            ._clients
            .get(chain_name)
            .unwrap_or_else(|| panic!("chain {chain_name} not found"))
        {
            AnyClient::Eth(client) => {
                let address = address.try_into().unwrap();

                match client.eth.provider.get_code_at(address).await {
                    Ok(addr) => **addr != alloy::primitives::Address::ZERO,
                    Err(_) => false,
                }
            }
            AnyClient::Cosmos(client) => client.querier.contract_info(&address).await.is_ok(),
        }
    }
}
