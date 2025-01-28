use std::{collections::HashMap, fmt::Display};

use alloy::primitives::Address;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use utils::{eigen_client::CoreAVSAddresses, types::ChainName, ServiceID, WorkflowID};

use crate::config::Config;

// Commands that return a type which update the deployment should implement this
pub trait CommandDeployResult: Display {
    fn update_deployment(&self, deployment: &mut Deployment);
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct Deployment {
    pub eigen_core: HashMap<ChainName, CoreAVSAddresses>,
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
        chain_name: ChainName,
        event_hash: [u8; 32],
        address: layer_climb::prelude::Address,
    },

    CosmosSimpleContract {
        chain_name: ChainName,
        event_type: String,
        address: layer_climb::prelude::Address,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceSubmitInfo {
    EigenLayer {
        chain_name: ChainName,
        service_manager_address: Address,
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
    ) -> Option<ServiceTriggerInfo> {
        let service = self.services.get(service_id)?;
        let workflow = match workflow_id {
            Some(workflow_id) => service.get(workflow_id)?,
            None => service.values().next()?,
        };

        Some(workflow.trigger.clone())
    }

    // for now - all of our submits use the same pattern of chain+avs_addresses
    // this will change in the future
    pub fn get_submit_info(
        &self,
        service_id: &ServiceID,
        workflow_id: Option<&WorkflowID>,
    ) -> Option<(ChainName, Address)> {
        let service = self.services.get(service_id)?;
        let workflow = match workflow_id {
            Some(workflow_id) => service.get(workflow_id)?,
            None => service.values().next()?,
        };

        let any_submit_info = workflow.submit.clone();

        match any_submit_info {
            ServiceSubmitInfo::EigenLayer {
                chain_name,
                service_manager_address,
            } => Some((chain_name, service_manager_address)),
        }
    }
}
