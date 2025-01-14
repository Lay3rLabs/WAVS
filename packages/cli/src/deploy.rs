use std::collections::{HashMap, HashSet};

use alloy::{primitives::Address, providers::Provider};
use anyhow::Result;
use layer_climb::signing::SigningClient;
use serde::{Deserialize, Serialize};
use utils::{
    avs_client::AvsAddresses,
    eigen_client::{CoreAVSAddresses, EigenClient},
};
use utils::{ServiceID, WorkflowID};

use crate::{
    args::{CliSubmitKind, CliTriggerKind, Command},
    config::Config,
};

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Deployment {
    // keyed by chain NAME (not chain id)
    pub eigen_core: HashMap<String, CoreAVSAddresses>,
    pub eth_services: HashMap<ServiceID, HashMap<WorkflowID, EthService>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EthService {
    pub avs_addresses: AvsAddresses,
    pub trigger_address: layer_climb::prelude::Address,
    pub submit_kind: CliSubmitKind,
}

impl Deployment {
    pub fn load(config: &Config) -> Result<Self> {
        let path = Self::path(config);
        tracing::debug!("Loading deployment from {}", path.display());

        if !path.exists() {
            tracing::warn!("No deployment file found at {:?}, using default", path);
            return Ok(Self::default());
        }

        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let deployment = serde_json::from_reader(reader)?;

        Ok(deployment)
    }

    pub async fn sanitize(
        &mut self,
        command: &Command,
        config: &Config,
        eth_client: Option<&EigenClient>,
        _cosmos_client: Option<&SigningClient>,
    ) -> Result<()> {
        // sanitize core
        {
            let mut to_remove = HashSet::new();
            match (eth_client, config.eth_chain.as_ref()) {
                (Some(client), Some(eth_chain)) => {
                    for (chain, addresses) in self.eigen_core.iter() {
                        if chain != eth_chain {
                            continue;
                        }

                        for address in addresses.as_vec() {
                            if client.eth.provider.get_code_at(address).await?.0.is_empty() {
                                to_remove.insert(chain.clone());
                            }
                        }
                    }

                    for chain in to_remove.into_iter() {
                        tracing::warn!("Core addresses for {chain} are invalid, filtering out");
                        self.eigen_core.remove(&chain);
                    }
                }
                _ => {}
            }
        }

        if let Some(service_id) = match command {
            Command::DeployCore { .. } => None,
            Command::DeployService { .. } => None,
            Command::AddTask { service_id, .. } => Some(ServiceID::new(service_id)?),
            Command::Exec { .. } => None,
        } {
            let mut to_remove = HashSet::new();
            if let Some(client) = eth_client {
                for (deployed_service_id, workflows) in self.eth_services.iter() {
                    if *deployed_service_id != service_id {
                        continue;
                    }

                    for (deployed_workflow_id, addresses) in workflows.iter() {
                        for address in addresses.avs_addresses.as_vec() {
                            let submit_address_exists = !client.eth.provider.get_code_at(address).await?.0.is_empty();
                            let trigger_address_exists = match addresses.trigger_address {
                                layer_climb::prelude::Address::Eth(addr) => !client.eth.provider.get_code_at(addr.as_bytes().into()).await?.0.is_empty(),
                                _ => false,
                            };
                            if !submit_address_exists || !trigger_address_exists {
                                to_remove.insert((
                                    deployed_service_id.clone(),
                                    deployed_workflow_id.clone(),
                                ));
                            }
                        }
                    }
                }
            }

            for (service_id, workflow_id) in to_remove.into_iter() {
                tracing::warn!("Service addresses for service {service_id}, workflow {workflow_id} are invalid, filtering out");
                let service = self.eth_services.get_mut(&service_id).unwrap();
                service.remove(&workflow_id);
                if service.is_empty() {
                    self.eth_services.remove(&service_id);
                }
            }
        }

        Ok(())
    }

    pub fn save(&self, config: &Config) -> Result<()> {
        if !config.data.exists() {
            std::fs::create_dir_all(&config.data)?;
        }
        let path = Self::path(config);
        tracing::debug!("Saving deployment to {}", path.display());
        let file = std::fs::File::create(path)?;
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer(writer, self)?;

        Ok(())
    }

    pub fn path(config: &Config) -> std::path::PathBuf {
        config.data.join("deployments.json")
    }
}
