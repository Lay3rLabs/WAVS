use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use utils::{avs_client::AvsAddresses, eigen_client::CoreAVSAddresses};
use wavs::apis::{ServiceID, WorkflowID};

use crate::config::Config;

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Deployment {
    // keyed by chain name (not necessarily the same as chainId)
    pub eigen_core: HashMap<String, CoreAVSAddresses>,
    pub services: HashMap<ServiceID, HashMap<WorkflowID, ServiceInfo>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServiceInfo {
    pub trigger: ServiceTriggerInfo,
    pub submit: ServiceSubmitInfo,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceTriggerInfo {
    EthSimpleContract {
        chain_name: String,
        address: layer_climb::prelude::Address,
    },

    CosmosSimpleContract {
        chain_name: String,
        address: layer_climb::prelude::Address,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceSubmitInfo {
    EigenLayer {
        chain_name: String,
        avs_addresses: AvsAddresses,
    },
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

    pub fn path(config: &Config) -> std::path::PathBuf {
        config.data.join("deployments.json")
    }

    // for now - all of our triggers use the same pattern of chain+address
    // this will change in the future
    pub fn get_trigger_info(
        &self,
        service_id: &ServiceID,
        workflow_id: Option<&WorkflowID>,
    ) -> Option<(String, layer_climb::prelude::Address)> {
        let service = self.services.get(service_id)?;
        let workflow = match workflow_id {
            Some(workflow_id) => service.get(workflow_id)?,
            None => service.values().next()?,
        };

        let any_trigger_info = workflow.trigger.clone();

        match any_trigger_info {
            ServiceTriggerInfo::EthSimpleContract {
                chain_name,
                address,
            } => Some((chain_name, address)),
            ServiceTriggerInfo::CosmosSimpleContract {
                chain_name,
                address,
            } => Some((chain_name, address)),
        }
    }

    // for now - all of our submits use the same pattern of chain+avs_addresses
    // this will change in the future
    pub fn get_submit_info(
        &self,
        service_id: &ServiceID,
        workflow_id: Option<&WorkflowID>,
    ) -> Option<(String, AvsAddresses)> {
        let service = self.services.get(service_id)?;
        let workflow = match workflow_id {
            Some(workflow_id) => service.get(workflow_id)?,
            None => service.values().next()?,
        };

        let any_submit_info = workflow.submit.clone();

        match any_submit_info {
            ServiceSubmitInfo::EigenLayer {
                chain_name,
                avs_addresses,
            } => Some((chain_name, avs_addresses)),
        }
    }
}
