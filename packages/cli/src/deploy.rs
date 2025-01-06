use std::collections::{HashMap, HashSet};

use alloy::providers::Provider;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use utils::{
    eigen_client::{CoreAVSAddresses, EigenClient},
    layer_contract_client::LayerAddresses,
};
use wavs::apis::{ServiceID, WorkflowID};

use crate::{args::Command, config::Config};

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Deployment {
    // keyed by chain NAME (not chain id)
    pub eigen_core: HashMap<String, CoreAVSAddresses>,
    pub eth_services: HashMap<ServiceID, HashMap<WorkflowID, LayerAddresses>>,
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
        client: &EigenClient,
    ) -> Result<()> {
        // sanitize core
        {
            let mut to_remove = HashSet::new();
            for (chain, addresses) in self.eigen_core.iter() {
                if chain != &config.chain {
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

        if let Some(service_id) = match command {
            Command::DeployCore { .. } => None,
            Command::DeployService { .. } => None,
            Command::AddTask { service_id, .. } => Some(ServiceID::new(service_id)?),
            Command::Exec { .. } => None,
        } {
            let mut to_remove = HashSet::new();
            for (deployed_service_id, workflows) in self.eth_services.iter() {
                if *deployed_service_id != service_id {
                    continue;
                }

                for (deployed_workflow_id, addresses) in workflows.iter() {
                    for address in addresses.as_vec() {
                        if client.eth.provider.get_code_at(address).await?.0.is_empty() {
                            to_remove.insert((
                                deployed_service_id.clone(),
                                deployed_workflow_id.clone(),
                            ));
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
